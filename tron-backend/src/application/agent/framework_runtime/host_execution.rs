//! Native preparation and execution use one SDK owner through product completion.
use super::*;
use tron_i18n::t;
impl FrameworkHost {
    pub(super) fn attach_skill_access(
        &self,
        context: &AgentRunContext,
        request: &mut fw::RunRequest,
    ) -> Result<(), fw::AgentError> {
        if let Some(skills) = self.0.skill_registry.get() {
            let agent = context.agent_code.as_deref().unwrap_or("main-agent");
            let mut names: std::collections::BTreeSet<String> = skills
                .effective_agent_skill_names(agent)
                .map_err(|e| fw::AgentError::InvalidConfiguration(e.to_string()))?;
            // The negotiated Framework A2UI skill belongs to its capability
            // pack, not the user's independently assigned skill registry.
            if request
                .extensions
                .get::<eliteforge_agent_official_plugin_a2ui::A2uiRunOptions>()
                .is_some()
            {
                names.insert(eliteforge_agent_official_plugin_a2ui::A2UI_SKILL_NAME.into());
            }
            request.extensions.insert(fw::SkillAccess { names });
            request
                .extensions
                .insert(crate::application::skill::SkillAgentIdentity(agent.into()));
        }
        Ok(())
    }
    pub(super) async fn run_inner(
        &self,
        harness: ReactAgent,
        mut context: AgentRunContext,
        sink: Option<ModelStreamSink>,
        steering: Option<ActiveRunControl>,
        registration: host_control::NativeRegistration,
    ) -> Result<AgentRunResult, AgentError> {
        self.prepare_working_directory(&mut context)
            .map_err(AgentError::InvalidConfiguration)?;
        crate::application::agent::execution_context::sync_execution_context(&mut context);
        // Artifact delivery is run-scoped: the collector lives in memory, the
        // host inserts it as a trusted run extension, and only the settled run
        // snapshot reaches persistence.
        let artifact_config = tron_harness_capability::ArtifactDeliveryConfig::default();
        let artifact_collector = Arc::new(tron_harness_capability::RunArtifactCollector::new(
            artifact_config.max_files_per_run,
        ));
        let artifact_root = context
            .execution_scope
            .local_directory
            .as_deref()
            .map(PathBuf::from);
        collaboration::authorize_tools(self, &mut context);
        let onboarding =
            super::super::onboarding::active(&context, harness.framework_assistants().as_ref());
        if onboarding {
            super::super::onboarding::narrow(&mut context);
        }
        let admission = self.0.config_lock.lock().await;
        let _runtime_lease = self.0.runtime_access.read().await;
        self.ensure_open()
            .map_err(AgentError::InvalidConfiguration)?;
        registration.ensure_active()?;
        crate::adapters::outbound::persistence::sqlite::SqliteChatRepository::ensure_run_session(
            self, &context,
        )
        .await
        .map_err(|e| AgentError::InvalidConfiguration(e.to_string()))?;
        if let Some(session_code) = context.conversation_code.as_deref() {
            self.recover_interrupted_checkpoint(session_code).await?;
        }
        let mut lifecycle = host_run::ProductLifecycle::start(&harness, &context)?;
        let agent = if let Some(code) = context.agent_code.as_deref() {
            self.internal_agent_profile(&context.workspace_code, code, onboarding)
                .await
                .map_err(map_error)?
        } else {
            self.agent().await.map_err(map_error)?.clone()
        };
        let session = context
            .conversation_code
            .clone()
            .unwrap_or_else(|| context.run_code.clone());
        let memory_input = context
            .short_term_memory
            .as_ref()
            .and_then(|memory| memory.current_input_message.as_ref());
        let (history, current) = if let Some(input) = memory_input {
            let history = if context.messages.last().is_some_and(|last| last == input) {
                &context.messages[..context.messages.len() - 1]
            } else {
                &context.messages[..]
            };
            (history, input)
        } else {
            let current = context
                .messages
                .iter()
                .rposition(|message| message.role == "user")
                .ok_or_else(|| {
                    AgentError::InvalidConfiguration("run requires a current user message".into())
                })?;
            if current + 1 != context.messages.len() {
                return Err(AgentError::InvalidConfiguration(
                    "current input must follow the prepared conversation history".into(),
                ));
            }
            (&context.messages[..current], &context.messages[current])
        };
        let prefix_len = context.short_term_memory.as_ref().map_or_else(
            || {
                history
                    .iter()
                    .take_while(|message| super::super::messages::is_prompt_prefix(message))
                    .count()
            },
            |memory| memory.prompt_prefix.len().min(history.len()),
        );
        let history = super::super::messages::encode_history(&history[prefix_len..])
            .map_err(AgentError::InvalidConfiguration)?;
        let input = super::super::messages::encode(current)
            .map_err(AgentError::InvalidConfiguration)?
            .content;
        let answers = host_interactions::answers_from_parts(&current.parts)?;
        let a2ui_options = current
            .parts
            .iter()
            .find_map(|part| {
                if let crate::domain::agent::AgentMessagePart::Context {
                    kind,
                    metadata: Some(metadata),
                    ..
                } = part
                {
                    if kind == "tron_interaction_response" {
                        return metadata.get("a2uiClientCapabilities").and_then(|v| {
                            serde_json::from_value::<
                                eliteforge_agent_official_plugin_a2ui::A2uiClientCapabilities,
                            >(v.clone())
                            .ok()
                        });
                    }
                }
                None
            })
            .filter(|c| {
                c.version == eliteforge_agent_official_plugin_a2ui::VERSION
                    && c.catalog_ids.iter().any(|id| {
                        [
                            eliteforge_agent_official_plugin_a2ui::AGENT_CATALOG_ID,
                            eliteforge_agent_official_plugin_a2ui::MINIMAL_CATALOG_ID,
                        ]
                        .contains(&id.as_str())
                    })
            });
        let a2ui_options = a2ui_options.filter(|_| {
            self.0
                .tools_enabled
                .load(std::sync::atomic::Ordering::Acquire)
                && self
                    .capability_state()
                    .0
                    .get(eliteforge_agent_official_plugin_a2ui::A2UI_PACK_ID)
                    .copied()
                    .unwrap_or(true)
        });
        let key = fw::ConversationKey::new(&session);
        let snapshot = agent.conversation(&key).await.map_err(map_error)?;
        if snapshot.revision == 0 && !history.is_empty() {
            agent
                .append_conversation(&key, 0, history)
                .await
                .map_err(map_error)?;
        }
        let compact = current
            .plain_text_content()
            .trim()
            .eq_ignore_ascii_case("/compact")
            || context
                .short_term_memory
                .as_ref()
                .and_then(|memory| memory.input_metadata.as_ref())
                .and_then(|metadata| metadata.get("contextCompaction"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
        let mut request = fw::RunRequest::new(&session, input);
        if compact {
            request.extensions.insert(fw::ContextCompactionRequest);
        }
        let memory = context.short_term_memory.as_ref();
        request.annotations = memory
            .and_then(|input| input.input_metadata.as_ref())
            .and_then(|metadata| metadata.get("annotations"))
            .map(|value| serde_json::from_value(value.clone()))
            .transpose()
            .map_err(|_| AgentError::InvalidConfiguration("invalid annotations".into()))?
            .unwrap_or_default();
        request.requested_skills = memory
            .and_then(|input| input.input_metadata.as_ref())
            .and_then(|metadata| metadata.get("requestedSkills"))
            .map(|value| serde_json::from_value(value.clone()))
            .transpose()
            .map_err(|_| AgentError::InvalidConfiguration("invalid requested skills".into()))?
            .unwrap_or_default();
        let runtime_input = memory.filter(|input| input.input_kind != "USER_PROMPT");
        let source = runtime_input
            .and_then(|input| input.input_metadata.as_ref())
            .and_then(|metadata| metadata.get("source"))
            .and_then(serde_json::Value::as_str)
            .or_else(|| runtime_input.map(|input| input.input_kind.as_str()))
            .unwrap_or("UserMessage");
        request.message_provenance = Some(fw::MessageProvenance {
            sender: fw::MessageSender {
                kind: if runtime_input.is_some() {
                    "SYSTEM"
                } else {
                    "HUMAN"
                }
                .into(),
                id: if runtime_input.is_some() {
                    Some(source.into())
                } else {
                    context.user_code.clone()
                },
                display_name: Some(match source {
                    "Heartbeat" => t!(
                        "agent_runtime.message.sender.system_heartbeat",
                        locale = context.locale.as_str()
                    )
                    .into_owned(),
                    "TaskReminder" => t!(
                        "agent_runtime.message.sender.task_reminder",
                        locale = context.locale.as_str()
                    )
                    .into_owned(),
                    "ThreadFeedback" => t!(
                        "agent_runtime.message.sender.thread_feedback",
                        locale = context.locale.as_str()
                    )
                    .into_owned(),
                    "UserMessage" => t!(
                        "agent_runtime.message.sender.user",
                        locale = context.locale.as_str()
                    )
                    .into_owned(),
                    _ => t!(
                        "agent_runtime.message.sender.memory_maintenance",
                        locale = context.locale.as_str()
                    )
                    .into_owned(),
                }),
                avatar_url: None,
            },
            source: Some(source.into()),
            correlation: runtime_input.and_then(|input| input.input_metadata.clone()),
        });
        let agent_code = context
            .agent_code
            .clone()
            .unwrap_or_else(|| "main-agent".into());
        request.assistant_provenance = Some(fw::MessageProvenance {
            sender: fw::MessageSender {
                kind: if context.uses_platform_agent_definition()
                    && context
                        .collaboration_owner
                        .as_ref()
                        .is_some_and(|owner| owner.kind == "ASSISTANT")
                {
                    "ASSISTANT"
                } else {
                    "AGENT"
                }
                .into(),
                id: Some(agent_code.clone()),
                display_name: Some(agent_code),
                avatar_url: None,
            },
            source: Some("AgentResponse".into()),
            correlation: None,
        });
        // Only Ask permissions are auto-approved; No remains enforced by both policies.
        if context.tool_execution_mode == crate::domain::agent::ToolExecutionMode::AutoApprove {
            request
                .extensions
                .insert(tron_harness_capability::TronToolAutoApproval);
        }
        if context.tool_execution_mode == crate::domain::agent::ToolExecutionMode::Plan {
            request
                .extensions
                .insert(tron_harness_capability::TronPlanMode);
        }
        if let Some(prompts) = self.0.prompt_service.get() {
            let mut keys = vec![
                "agent.goal-judge",
                "memory.context-summary",
                "skills.guidance",
                "a2ui.guidance",
                "agent.directory-guidance",
            ];
            if context.is_owning_assistant() {
                keys.extend_from_slice(
                    super::super::assistant_context::AssistantGuidance::prompt_keys(false),
                );
                keys.extend_from_slice(
                    super::super::assistant_context::AssistantGuidance::prompt_keys(true),
                );
            }
            let resolved = prompts
                .runtime_prompt_texts(&keys, &context.locale, &std::collections::HashMap::new())
                .map_err(|error| {
                    AgentError::InvalidConfiguration(format!(
                        "managed developer prompts could not be resolved: {error}"
                    ))
                })?;
            request
                .extensions
                .insert(fw::ManagedDeveloperPrompts(resolved));
        }
        if let Some(capabilities) = a2ui_options {
            request
                .extensions
                .insert(eliteforge_agent_official_plugin_a2ui::A2uiRunOptions {
                    capabilities,
                    owner_agent_id: context
                        .agent_code
                        .clone()
                        .unwrap_or_else(|| "main-agent".into()),
                });
        }
        super::file_coordination::attach(&context, &session, &mut request);
        request.run_id = Some(fw::RunId::parse_str(&context.run_code).map_err(|e| {
            AgentError::InvalidConfiguration(format!("invalid native run identity: {e}"))
        })?);
        let framework = self.framework().await.map_err(map_error)?;
        request.goal_id = framework
            .goals()
            .map_err(map_error)?
            .active(&session)
            .await
            .map_err(|e| map_error(e.into()))?
            .map(|g| g.id);
        self.shared_database()
            .await
            .map_err(|e| map_error(e.into()))?
            .transaction(async {
                host_interactions::apply_answers(&agent, &session, &answers, &mut request)
                    .await
                    .map_err(|e| fw::StateError::Backend(e.to_string()))
            })
            .await
            .map_err(|e| map_error(e.into()))?;
        request.limits.max_steps = super::runtime_count_limit(context.limits.max_steps);
        request.limits.max_tool_calls = super::runtime_count_limit(context.limits.max_tool_calls);
        request.limits.tool_batch_budget_policy = fw::ToolBatchBudgetPolicy::ExecuteAvailable;
        request.limits.max_parallel_tools = context.limits.max_parallel_tool_calls.max(1) as usize;
        request.limits.max_context_messages = context.messages.len().max(256).saturating_add(
            request
                .limits
                .max_steps
                .saturating_add(request.limits.max_tool_calls.saturating_mul(2)),
        );
        request.limits.max_context_tokens =
            context.model.context_window_tokens.unwrap_or(128_000) as usize;
        request.limits.max_model_output_tokens = context
            .model
            .max_tokens
            .filter(|value| *value > 0)
            .unwrap_or_else(|| self.invocation_config().default_output_tokens);
        // Context capacity is not a cumulative usage quota. Tron does not
        // configure a run-wide token ceiling.
        request.limits.max_total_tokens = fw::RunLimits::UNLIMITED_TOKENS;
        let invocation = self.invocation_config();
        request.limits.run_timeout = Duration::from_secs(invocation.agent_run_timeout_seconds);
        request.limits.model_timeout = request.limits.run_timeout;
        request.limits.max_model_retries = if context.auto_model_route.is_some() {
            2
        } else {
            context.model.max_retries.unwrap_or(0).min(3) as usize
        };
        request.limits.max_model_calls_per_step =
            Some((request.limits.max_model_retries + 1).max(3));
        self.attach_tool_policy(&mut request, &context.locale);
        request.limits.max_overflow_recoveries = harness.framework_overflow_limit();
        request.generation.temperature = context.model.temperature;
        request.generation.top_p = context.model.top_p;
        request.generation.max_output_tokens = context.model.max_tokens.filter(|value| *value > 0);
        let mut run_state = HostRunState::new(context.clone());
        run_state.onboarding = onboarding;
        let state = Arc::new(Mutex::new(run_state));
        request
            .extensions
            .insert(fw::ToolResultProcessorService(Arc::new(ToolAudit)));
        if let Some(user) = context.user_code.as_ref() {
            request.extensions.insert(
                eliteforge_agent_official_plugin_tools_local::SudoPasswordProviderService(
                    Arc::new(super::host_sudo::HostSudoInput {
                        host: Arc::downgrade(&self.0),
                        session: session.clone(),
                        user: user.clone(),
                    }),
                ),
            );
        }
        request.extensions.insert(
            eliteforge_agent_official_plugin_tools_local::CommandProgressSinkService(Arc::new(
                super::super::tools::CommandProgress {
                    harness: harness.clone(),
                    context: context.clone(),
                },
            )),
        );
        if onboarding {
            request.extensions.insert(fw::SkillAccess {
                names: [eliteforge_agent_official_plugin_a2ui::A2UI_SKILL_NAME.into()]
                    .into_iter()
                    .collect(),
            });
            let service = framework
                .services()
                .map_err(map_error)?
                .get::<eliteforge_agent_official_plugin_a2ui::A2uiService>()
                .ok_or_else(|| {
                    AgentError::InvalidConfiguration("A2UI unavailable for onboarding".into())
                })?;
            request.extensions.insert((*service).clone());
        } else {
            self.attach_skill_access(&context, &mut request)
                .map_err(map_error)?;
        }
        // Framework-pack bindings (bash/fs/search/terminal, collaboration,
        // skills, a2ui, remote plugins) are minted inside the packs, so the
        // host publishes this run's authorized taxonomy here instead of
        // labelling each pack binding individually.
        request
            .extensions
            .insert(super::super::tools::tool_group_labels(&context));
        // Resolve the Agent's selected native-exposure scheme (or the global
        // default). The allowlist does not add authority; policy and the
        // effective catalog still decide which IDs are present.
        let native_ids = match self.0.agent_management.get() {
            Some(management) => management
                .native_tool_ids_for_agent(
                    context.workspace_code.as_str(),
                    context.agent_code.as_deref().unwrap_or("main-agent"),
                )
                .map(super::super::tools::native_ids_from_scheme)
                .unwrap_or_else(|_| super::super::tools::default_native_tool_ids()),
            None => super::super::tools::default_native_tool_ids(),
        };
        request
            .extensions
            .insert(super::super::tools::native_tool_whitelist(native_ids));
        self.attach_memory_backend(&context, &mut request.extensions);
        request.extensions.insert(HostExecution {
            harness: harness.clone(),
            state: state.clone(),
            summaries: self.summary_store().await.map_err(map_error)?,
            sink,
        });
        tron_harness_capability::TronRunScope {
            platform_scope: context.is_platform_assistant(),
            user_code: context.user_code.clone().unwrap_or_else(|| "system".into()),
            workspace_code: context.workspace_code.clone(),
            session_code: session.clone(),
            run_code: context.run_code.clone(),
            app_code: context.execution_scope.app_code.clone(),
            task_code: context.execution_scope.task_code.clone(),
            agent_code: context.agent_code.clone().or_else(|| {
                context
                    .short_term_memory
                    .as_ref()
                    .and_then(|memory| memory.agent_code.clone())
            }),
        }
        .attach(&mut request)
        .map_err(map_error)?;
        if self
            .0
            .tools_enabled
            .load(std::sync::atomic::Ordering::Acquire)
            && let Some(workspace_root) = artifact_root.clone()
        {
            request
                .extensions
                .insert(tron_harness_capability::ArtifactsRunOptions {
                    workspace_root,
                    collector: artifact_collector.clone(),
                });
        }
        if let Some(resources) = &self.0.resources {
            request.extensions.insert(
                eliteforge_agent_official_plugin_tools_local::CommandOutputCaptureService(
                    Arc::new(CommandCapture(resources.clone())),
                ),
            );
            request
                .extensions
                .insert(FileReferenceReaderService(Arc::new(AttachmentReader(
                    resources.clone(),
                ))));
        }
        if let Some(control) = steering {
            request.extensions.insert(
                eliteforge_agent_official_plugin_tools_local::CommandCompletions::new(&session),
            );
            request
                .extensions
                .insert(fw::ContextCompactionObserverBinding(Arc::new(
                    control.clone(),
                )));
            request
                .extensions
                .insert(fw::LiveInputSourceService(Arc::new(HostSteering {
                    control,
                })));
        }
        if !onboarding {
            collaboration::attach(self, &context, &mut request);
        }
        harness
            .framework_model_scope(&context)?
            .attach(&mut request);
        self.ensure_open()
            .map_err(AgentError::InvalidConfiguration)?;
        let trace_guard = self
            .telemetry()
            .await
            .map_err(|_| AgentError::InvalidConfiguration("telemetry unavailable".into()))?
            .begin(&context, &mut request)
            .map_err(|_| AgentError::InvalidConfiguration("trace registration failed".into()))?;
        let parent = eliteforge_agent_framework::current_continuation_parent();
        let result = if let Some(parent) = parent {
            let started = agent
                .start_child_with_output_owned::<eliteforge_agent_official_plugin_agui::AgUiStream>(
                    request, &parent,
                )
                .await;
            drop(admission);
            match started {
                Ok((handle, stream)) => {
                    self.wait_native_child(
                        agent.clone(),
                        handle,
                        stream,
                        &context,
                        registration.clone(),
                        trace_guard.trace_id(),
                    )
                    .await
                }
                Err(error) => Err(error),
            }
        } else {
            let started = agent
                .start_with_output::<eliteforge_agent_official_plugin_agui::AgUiStream>(request);
            drop(admission);
            match started {
                Ok((handle, stream)) => {
                    self.wait_native(
                        agent.clone(),
                        handle,
                        stream,
                        &context,
                        registration.clone(),
                        trace_guard.trace_id(),
                    )
                    .await
                }
                Err(error) => Err(error),
            }
        };
        trace_guard.finish(&result);
        drop(trace_guard);
        if onboarding
            && result
                .as_ref()
                .is_ok_and(|r| r.status == fw::RunStatus::AwaitingInput)
        {
            if let Some(assistants) = harness.framework_assistants() {
                for interaction in agent
                    .pending_interactions(
                        context
                            .conversation_code
                            .as_deref()
                            .unwrap_or(&context.run_code),
                    )
                    .await
                    .map_err(map_error)?
                {
                    if !matches!(interaction.request.kind, fw::InteractionKind::Input) {
                        continue;
                    }
                    assistants
                        .record_soul_question(
                            &interaction.session_id,
                            &interaction.request.id.to_string(),
                        )
                        .map_err(|e| AgentError::InvalidConfiguration(e.to_string()))?;
                }
            }
        }
        let final_message_ids = if result
            .as_ref()
            .is_ok_and(|result| result.status == fw::RunStatus::Completed)
        {
            agent.latest_checkpoint(&session).await.ok().flatten()
                .filter(|checkpoint| checkpoint.run_id.to_string() == registration.run_id())
                .map(|checkpoint| {
                    let Some(step) = checkpoint.history.steps.iter().rev().find(|step| step.status == fw::HistoryStatus::Completed) else { return Vec::new(); };
                    let mut messages: Vec<_> = checkpoint.history.messages.iter().filter(|message|
                        message.step_id.as_deref() == Some(step.id.as_str()) && message.output_index.is_some()
                            && message.content.iter().any(|block| matches!(block, fw::HistoryContentBlock::Text { text } if !text.trim().is_empty()))
                    ).collect();
                    messages.sort_by_key(|message| message.output_index);
                    let explicit = messages.iter().any(|message| message.phase.as_deref() == Some("final_answer"));
                    messages.into_iter().filter(|message| message.phase.as_deref() == Some("final_answer")
                        || (!explicit && message.phase.is_none())).map(|message| message.id.clone()).collect()
                }).unwrap_or_default()
        } else {
            Vec::new()
        };
        let state = state.lock().await;
        let failure = match &result {
            Err(error) => Some(match error.primary() {
                fw::AgentError::SessionBusy(_) | fw::AgentError::Cancelled => map_error_ref(error),
                fw::AgentError::State(fw::StateError::Busy)
                    if runtime_input
                        .is_some_and(|input| input.input_kind == "ASSISTANT_MEMORY_BATCH")
                        && !matches!(error, fw::AgentError::WithCleanup { .. }) =>
                {
                    AgentError::SessionBusy(session.clone())
                }
                _ if matches!(error, fw::AgentError::WithCleanup { .. }) => map_error_ref(error),
                _ => state.error.clone().unwrap_or_else(|| map_error_ref(error)),
            }),
            _ => None,
        };
        if let Some(error) = failure {
            lifecycle.finish(&state.context, &Err(fw::AgentError::Run(error.to_string())))?;
            return Err(error);
        }
        lifecycle.finish(&state.context, &result)?;
        let result = result.map_err(map_error)?;
        let output = result.output.clone();
        Ok(AgentRunResult {
            run_code: context.run_code,
            conversation_code: context.conversation_code,
            workspace_code: context.workspace_code,
            final_output: output,
            final_message_ids,
            step_count: result.steps as u32,
            token_usage: state.usage,
            effective_model: Some(state.context.model.clone()),
            latest_request_estimate: state.estimate.clone(),
            latest_usage_audit: state.audit.clone(),
            system_agent_session_id: state.context.system_agent_session_id.clone(),
            observations: state.context.observations.clone(),
            native: Some(host_control::native_outcome(
                registration.run_id(),
                &result,
            )?),
            sdk_execution: Some(host_snapshot::product_snapshot(
                registration.snapshot(&result).map_err(map_error)?,
            )?),
            artifact_records: artifact_collector
                .snapshot()
                .into_iter()
                .map(|record| crate::domain::chat::ArtifactRecord {
                    id: record.id,
                    display: record.display,
                    description: record.description,
                    media_type: record.media_type,
                    byte_size: record.byte_size.unwrap_or(record.fingerprint.size),
                    source: crate::domain::chat::ArtifactLocation {
                        kind: record.source.kind,
                        root: record.source.root,
                        relative_path: record.source.relative_path,
                    },
                    fingerprint: crate::domain::chat::ArtifactFingerprint {
                        size: record.fingerprint.size,
                        modified_at_ns: record.fingerprint.modified_at_ns,
                    },
                })
                .collect(),
        })
    }
}
