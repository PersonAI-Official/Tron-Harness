impl ChatService {
    pub async fn send_message(
        &self,
        session_code: &str,
        command: SendChatMessageCommand,
        resolved_model: AgentModelConfig,
        user_code: Option<String>,
        locale: Option<String>,
        llm_runtime: Option<LlmRuntimeService>,
        agent: AgentInvocationService,
    ) -> Result<ChatSendResult, ChatSendError> {
        self.send_message_inner(
            session_code,
            command,
            resolved_model,
            user_code,
            locale,
            llm_runtime,
            agent,
            None,
            None,
        )
        .await
    }

    pub async fn send_message_stream(
        &self,
        session_code: &str,
        command: SendChatMessageCommand,
        resolved_model: AgentModelConfig,
        user_code: Option<String>,
        locale: Option<String>,
        llm_runtime: Option<LlmRuntimeService>,
        agent: AgentInvocationService,
        sink: ModelStreamSink,
    ) -> Result<ChatSendResult, ChatSendError> {
        self.send_message_inner(
            session_code,
            command,
            resolved_model,
            user_code,
            locale,
            llm_runtime,
            agent,
            Some(sink),
            None,
        )
        .await
    }

    pub(crate) async fn send_background_message(
        &self,
        session_code: &str,
        command: SendChatMessageCommand,
        resolved_model: AgentModelConfig,
        user_code: Option<String>,
        locale: String,
        agent: AgentInvocationService,
        presentation: &crate::application::agent::AgentAguiStreamService,
        admission_signal: Option<tokio::sync::oneshot::Sender<()>>,
    ) -> Result<ChatSendResult, ChatSendError> {
        let _watch = presentation.watch(session_code);
        self.send_message_inner(
            session_code,
            command,
            resolved_model,
            user_code,
            Some(locale),
            None,
            agent,
            Some(std::sync::Arc::new(|_| {})),
            admission_signal,
        )
        .await
    }

    async fn send_message_inner(
        &self,
        session_code: &str,
        command: SendChatMessageCommand,
        resolved_model: AgentModelConfig,
        user_code: Option<String>,
        locale: Option<String>,
        llm_runtime: Option<LlmRuntimeService>,
        agent: AgentInvocationService,
        sink: Option<ModelStreamSink>,
        admission_signal: Option<tokio::sync::oneshot::Sender<()>>,
    ) -> Result<ChatSendResult, ChatSendError> {

        let artifact_actions = command
            .client_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.artifacts.as_ref())
            .map(|capabilities| capabilities.actions.clone())
            .unwrap_or_else(|| crate::domain::chat::ArtifactAction::PORTABLE.to_vec());
        eliteforge_agent_core::validate_annotations(&command.annotations).map_err(|_| ChatSendError::Chat(ChatError::AnnotationsInvalid))?;
        let annotations = command.annotations;
        let content = normalized_optional(command.content).unwrap_or_default();
        let context_compaction = content.trim().eq_ignore_ascii_case("/compact");
        let attachments =
            normalize_chat_attachments(command.attachments).map_err(ChatSendError::Chat)?;
        if content.is_empty() && attachments.is_empty() && annotations.is_empty() && command.resume.is_empty() {
            return Err(ChatSendError::Chat(ChatError::MessageRequired));
        }
        let session = self
            .repository()
            .get_session(session_code)
            .map_err(ChatSendError::Chat)?;
        if session.status != crate::domain::chat::ChatSessionStatus::Active {
            return Err(ChatSendError::Chat(ChatError::SessionArchived));
        }
        self.repository().validate_annotation_sources(session_code, &annotations).map_err(ChatSendError::Chat)?;
        // Session ownership comes from persisted server state, not the caller's requested agent.
        let agent = agent.with_tool_execution_mode(session.execution_mode);
        let assistant_code = session.source_code.as_deref().and_then(|source| source.strip_prefix("__assistant:"));
        let knowledge_context=self.knowledge_quality_context(&session,user_code.as_deref()).map_err(ChatSendError::Chat)?;
        let knowledge_bound=knowledge_context.is_some();
        let agent = if let Some(assistant_code) = assistant_code {
            let service = agent.with_collaboration_owner(crate::domain::agent::AgentCollaborationOwner {
                kind: "ASSISTANT".into(), code: assistant_code.into(), origin_session_code: Some(session.session_code.clone()), task_code: session.task_code.clone(),
            });
            service.with_platform_scope()
        } else if knowledge_bound {agent.with_platform_scope()} else { agent };
        let (attachments, invocation_attachments) = self
            .prepare_session_attachments(&session, attachments)
            .map_err(ChatSendError::Chat)?;
        let prompt = if command.resume.is_empty() {
            if !annotations.is_empty() { content.clone() } else { agent_prompt_for_content(&content) }
        } else {
            resume_prompt(&command.resume)
        };
        let prompt=knowledge_context.map_or_else(||prompt.clone(),|context|format!("{context}\n\n用户请求：\n{prompt}"));
        let transcript_user_content =
            resume_user_message(&command.resume).unwrap_or_else(|| content.clone());
        let should_generate_title = should_generate_manual_session_title(&session);
        let workspace_code = session.workspace_code.clone();
        let channel = if command
            .task_code
            .as_ref()
            .or(session.task_code.as_ref())
            .is_some()
        {
            ChatMessageChannel::TaskAgent
        } else {
            ChatMessageChannel::MainAgent
        };
        let effective_agent_code = if session.last_run_code.is_some() || assistant_code.is_some() || knowledge_bound {
            session.agent_code.clone()
        } else {
            normalized_optional(command.agent_code.clone()).or(session.agent_code.clone())
        }
        .unwrap_or_else(|| MAIN_AGENT_CODE.to_string());
        let requested_skills = self.authorize_requested_skills(&effective_agent_code, &command.requested_skills)
            .map_err(ChatSendError::Chat)?;
        let goal = super::composer_input::goal_objective(&content).map_err(ChatSendError::Chat)?;
        if goal.is_some() { self.ensure_goal_agent(&effective_agent_code).map_err(ChatSendError::Chat)?; }
        let runtime_type = agent.runtime_type_for_admission(&workspace_code, &effective_agent_code).map_err(ChatSendError::Agent)?;
        let reservation = self.reserve_run(
            session_code,
            command.run_code.clone(),
            runtime_type,
            context_compaction,
        )?;
        let run_code = reservation.run_code;
        let runtime_adapter_key = reservation.runtime_adapter_key;
        let active_run_reservation = reservation.guard;
        let _session_run_guard = acquire_session_run(self.coordinator_namespace, session_code)?;
        // The session was initially read before run admission. A location
        // change may have completed in that gap; never execute with that stale scope.
        let admitted = self.repository().get_session(session_code).map_err(ChatSendError::Chat)?;
        if admitted.status != session.status || admitted.project_code != session.project_code
            || admitted.directory_code != session.directory_code
            || admitted.working_directory_code != session.working_directory_code
            || admitted.app_code != session.app_code
        {
            return Err(ChatSendError::Chat(ChatError::SessionLocationChanged));
        }
        self.ensure_session_directory_available(session_code).map_err(ChatSendError::Chat)?;
        let mapped_runtime_session_id = self
            .repository()
            .runtime_session_id(session_code, runtime_adapter_key, &effective_agent_code)
            .map_err(ChatSendError::Chat)?;
        if let Some(objective) = goal {
            self.apply_goal_command(session_code, objective, user_code.clone()).map_err(ChatSendError::Chat)?;
        }
        let user_message = if transcript_user_content.is_empty() && attachments.is_empty() && annotations.is_empty() {
            None
        } else {
            Some(self.create_message_with_skills(
                session_code,
                (!command.resume.is_empty()).then(|| run_code.clone()),
                ChatMessageRole::User,
                channel,
                agent.runtime_input_actor().unwrap_or("User"),
                &transcript_user_content,
                attachments.clone(),
                None,
                requested_skills.clone(),
                annotations.clone(),
            )?)
        };
        if should_generate_title
            && !content.is_empty()
            && !context_compaction
            && let Some(llm_runtime) = llm_runtime
        {
            let chat_service = self.clone();
            let session_code = session_code.to_string();
            let workspace_code = workspace_code.clone();
            let user_message_content = content.clone();
            let title_run_code = run_code.clone();
            let title_locale = locale.clone();
            crate::app::telemetry::spawn_traced(async move {
                chat_service
                    .try_generate_manual_session_title(
                        &session_code,
                        &workspace_code,
                        &user_message_content,
                        &title_run_code,
                        title_locale,
                        llm_runtime,
                    )
                    .await;
            });
        }
        let project_scope = self
            .invocation_execution_scope(&session, command.workspace_code.as_deref(), !command.resume.is_empty())
            .map_err(ChatSendError::Chat)?;
        let app_code_for_invocation = match project_scope.as_ref() {
            Some(scope) => scope.app_code.clone(),
            None => command.app_code.or(session.app_code.clone()),
        };
        let task_code_for_invocation = command.task_code.or(session.task_code.clone());
        let supports_user_input_requests = command
            .client_capabilities
            .as_ref()
            .is_some_and(|capabilities| capabilities.user_input_requests);
        let system_model_id_for_invocation = command.system_model_id;
        let limits_for_invocation = command.limits;
        let tools_for_invocation = command.tools;
        let invocation = AgentInvocationCommand {
            run_code: Some(run_code),
            prompt: Some(prompt),
            workspace_code: Some(workspace_code.clone()),
            conversation_code: Some(session.session_code.clone()),
            agent_code: Some(effective_agent_code.clone()),
            system_agent_session_id: mapped_runtime_session_id
                .or(session.system_agent_session_id.clone()),
            app_code: app_code_for_invocation.clone(),
            execution_scope_override: project_scope,
            task_code: task_code_for_invocation.clone(),
            model: Some(resolved_model.clone()),
            system_model_id: system_model_id_for_invocation.clone(),
            limits: limits_for_invocation,
            tools: tools_for_invocation.clone(),
            attachments: invocation_attachments,
            page_context: command.page_context,
            supports_user_input_requests,
            transcript_input_kind: None,
            transcript_input_metadata: Some(json!({"annotations": annotations, "requestedSkills": requested_skills, "resume": command.resume, "a2uiClientCapabilities":command.client_capabilities.and_then(|c|c.a2ui),"a2uiMetadata":command.a2ui_metadata,"contextCompaction": context_compaction})),
        };
        if let Some(signal) = admission_signal {
            let _ = signal.send(());
        }
        let mut run = if let Some(sink) = sink {
            agent
                .invoke_stream_reserved(
                    invocation,
                    user_code.clone(),
                    locale.clone(),
                    sink,
                    active_run_reservation,
                )
                .await
        } else {
            agent
                .invoke_reserved(
                    invocation,
                    user_code.clone(),
                    locale.clone(),
                    active_run_reservation,
                )
                .await
        }
        .map_err(ChatSendError::Agent)?;
        let awaiting_user_input = run.status == "AWAITING_USER_INPUT";
        let (assistant_content, user_input_request) = if awaiting_user_input {
            paused_run_presentation(&run)?
        } else {
            (run.final_output.clone(), None)
        };
        let mut assistant_message = self.create_message(
            session_code,
            Some(run.run_code.clone()),
            ChatMessageRole::Assistant,
            channel,
            "Assistant",
            &assistant_content,
            Vec::new(),
            run.token_usage,
        )?;
        if !run.artifact_records.is_empty() {
            self.repository()
                .append_artifacts(
                    session_code,
                    &run.run_code,
                    &assistant_message.message_code,
                    &run.artifact_records,
                )
                .map_err(ChatSendError::Chat)?;
            assistant_message.artifacts =
                crate::domain::chat::ChatArtifacts::from_records_with_actions(
                    &run.artifact_records,
                    &artifact_actions,
                );
        }
        let mut result_messages = user_message.into_iter().collect::<Vec<_>>();
        result_messages.push(assistant_message.clone());
        let mut user_input_request = if let Some(raw) = user_input_request {
            Some(self.persist_user_input_request(session_code, &run, &assistant_message, raw)?)
        } else {
            None
        };
        let updated_at = current_timestamp();
        if let Some(runtime_session_id) = run.system_agent_session_id.as_deref() {
            self.repository()
                .upsert_runtime_session(
                    session_code,
                    run.runtime_adapter_key.as_str(),
                    &effective_agent_code,
                    runtime_session_id,
                    Some(run.run_code.as_str()),
                    &updated_at,
                )
                .map_err(ChatSendError::Chat)?;
        }
        self.repository()
            .update_session_run(
                session_code,
                &run.run_code,
                run.status,
                Some(effective_agent_code.as_str()),
                run.system_agent_session_id.as_deref(),
                &updated_at,
            )
            .map_err(ChatSendError::Chat)?;
        if !awaiting_user_input && !context_compaction && let Some(short_term_memory) = agent.short_term_memory() {
            let goal_result = self
                .run_goal_continuations(
                    session_code,
                    &session,
                    &effective_agent_code,
                    app_code_for_invocation,
                    task_code_for_invocation,
                    system_model_id_for_invocation,
                    resolved_model,
                    tools_for_invocation,
                    user_code,
                    locale,
                    agent,
                    short_term_memory,
                    run.clone(),
                    run.system_agent_session_id.clone(),
                    supports_user_input_requests,
                )
                .await?;
            goal_result.merge_into(&mut run, &mut result_messages, &mut user_input_request);
        }
        let updated_session = self
            .repository()
            .get_session(session_code)
            .map_err(ChatSendError::Chat)?;
        Ok(ChatSendResult {
            session: updated_session,
            run,
            messages: result_messages,
            user_input_request,
        })
    }

    pub(super) fn persist_user_input_request(
        &self,
        session_code: &str,
        run: &AgentInvocationResult,
        assistant_message: &ChatMessage,
        raw: RawUserInputRequest,
    ) -> Result<ChatUserInputRequest, ChatSendError> {
        let questions = raw
            .questions
            .into_iter()
            .map(|question| ChatUserInputQuestion {
                question_code: question.id,
                question: question.question,
                options: question
                    .options
                    .into_iter()
                    .map(|option| ChatUserInputOption {
                        option_code: option.id,
                        label: option.label,
                        description: option.description,
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        if questions.is_empty() {
            return Err(ChatSendError::Agent(AgentInvocationError::RuntimeFailed(
                "chat_ask_user returned no questions".into(),
            )));
        }
        let request = ChatUserInputRequest {
            request_code: raw
                .framework_interaction_id
                .ok_or_else(|| {
                    ChatSendError::Agent(AgentInvocationError::RuntimeFailed(
                        "user question is missing its Framework interaction identity".into(),
                    ))
                })?
                .to_string(),
            session_code: session_code.to_string(),
            run_code: run.run_code.clone(),
            assistant_message_code: Some(assistant_message.message_code.clone()),
            intro: raw.intro,
            questions,
            answers: None,
            model_code: Some(run.model_code.clone()),
            reasoning_effort: run.reasoning_effort,
            status: ChatUserInputRequestStatus::Pending,
            requested_at: current_timestamp(),
            answered_at: None,
        };
        self.repository()
            .create_user_input_request(&request)
            .map_err(ChatSendError::Chat)?;
        if let Some(assistants)=self.assistants.get() {
            assistants.record_soul_question(session_code,&request.request_code).map_err(|_|ChatSendError::Chat(ChatError::StorageFailure))?;
        }
        Ok(request)
    }

    fn prepare_session_attachments(
        &self,
        session: &ChatSession,
        attachments: Vec<ChatMessageAttachment>,
    ) -> Result<(Vec<ChatMessageAttachment>, Vec<ChatMessageAttachment>), ChatError> {
        if attachments
            .iter()
            .all(|attachment| attachment.resource_code.is_none())
        {
            return Ok((attachments.clone(), attachments));
        }
        let resources = self.resources.as_ref().ok_or(ChatError::StorageFailure)?;
        let mut persisted = Vec::with_capacity(attachments.len());
        let mut invocation = Vec::with_capacity(attachments.len());
        for mut attachment in attachments {
            let Some(attachment_code) = attachment.resource_code.clone() else {
                invocation.push(attachment.clone());
                persisted.push(attachment);
                continue;
            };
            let (record, bytes) = resources
                .read_chat_session_attachment(
                    &session.workspace_code,
                    &session.session_code,
                    &attachment_code,
                )
                .map_err(chat_error_from_resource)?;
            apply_attachment_record(&mut attachment, &record);
            let mut agent_attachment = attachment.clone();
            hydrate_attachment_for_agent(&mut agent_attachment, &record, &bytes);
            persisted.push(attachment);
            invocation.push(agent_attachment);
        }
        Ok((persisted, invocation))
    }

    pub(super) fn create_message(
        &self,
        session_code: &str,
        run_code: Option<String>,
        role: ChatMessageRole,
        channel: ChatMessageChannel,
        actor: &str,
        content: &str,
        attachments: Vec<ChatMessageAttachment>,
        token_usage: Option<ModelTokenUsage>,
    ) -> Result<ChatMessage, ChatSendError> {
        self.create_message_with_skills(session_code, run_code, role, channel, actor, content, attachments, token_usage, Vec::new(), Vec::new())
    }

    pub(super) fn create_message_with_skills(
        &self, session_code: &str, run_code: Option<String>, role: ChatMessageRole,
        channel: ChatMessageChannel, actor: &str, content: &str,
        attachments: Vec<ChatMessageAttachment>, token_usage: Option<ModelTokenUsage>,
        requested_skills: Vec<String>,
        annotations: Vec<eliteforge_agent_core::MessageAnnotation>,
    ) -> Result<ChatMessage, ChatSendError> {
        let id = self
            .id_provider
            .next_id()
            .map_err(|_| ChatSendError::Chat(ChatError::StorageFailure))?
            .value();
        let message = ChatMessage {
            annotations,
            id,
            message_code: format!("MSG-{id}"),
            session_code: session_code.to_string(),
            run_code,
            role,
            channel,
            actor: actor.into(),
            content: content.into(),
            requested_skills,
            attachments,
            input_tokens: token_usage.map(|usage| usage.input_tokens),
            output_tokens: token_usage.map(|usage| usage.output_tokens),
            total_tokens: token_usage.map(|usage| usage.total_tokens),
            created_at: current_timestamp(),
            artifacts: None,
        };
        let mut repository = self.repository();
        repository
            .create_message(&message)
            .map_err(ChatSendError::Chat)?;
        Ok(message)
    }

    pub(super) fn repository(&self) -> std::sync::MutexGuard<'_, Box<dyn ChatRepository>> {
        self.repository
            .lock()
            .expect("chat repository mutex poisoned")
    }
}
