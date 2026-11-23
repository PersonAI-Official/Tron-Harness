//! Application-owned named Agent resolution over official collaboration tools.
use super::*;
use crate::application::agent::{AgentInvocationCommand, AgentRunContextBuilder};
use crate::domain::agent::EffectiveTool;
use eliteforge_agent_core::{AgentCollaborationRuntime, AgentCollaborationService};
use eliteforge_agent_official_plugin_tools_agents as official;
use std::sync::Weak;

#[path = "host_collaboration_external.rs"]
mod external;

#[path = "host_collaboration_children.rs"]
mod children;
#[path = "host_collaboration_threads.rs"]
mod threads;
pub(super) use children::Children;
pub(super) use threads::Threads;
pub use threads::{AssistantThreadCatalogItem, ThreadFeedback, ThreadFeedbackSink};

#[derive(Clone, Copy)]
enum ExecutionLimits {
    /// Synchronous delegation: the caller waits inline for the result.
    AgentDefinition,
    /// Detached delegation: the child outlives the caller's run.
    AgentDefinitionDetached,
    Request,
}

#[derive(Clone, Copy)]
enum SessionPreparation {
    New,
    Existing,
}

pub(in crate::application::agent::framework_runtime) struct DirectoryAgent {
    pub agent_code: String,
    pub name: String,
    pub description: String,
    pub avatar_id: String,
    pub model_id: Option<String>,
}

#[derive(Clone)]
pub(in crate::application::agent::framework_runtime) struct Runtime {
    host: Weak<Host>,
    pub builder: AgentRunContextBuilder,
    pub context: AgentRunContext,
}
fn failure(error: impl std::fmt::Display) -> fw::AgentError {
    fw::AgentError::InvalidConfiguration(error.to_string())
}

fn enabled(host: &FrameworkHost) -> bool {
    let (packs, extensions) = host.capability_state();
    packs
        .get(eliteforge_agent_capability_tools::AGENT_COLLABORATION_PACK)
        .copied()
        .unwrap_or(true)
        && extensions.get("tool").copied().unwrap_or(true)
}

pub(super) fn authorize_tools(host: &FrameworkHost, context: &mut AgentRunContext) {
    if host
        .0
        .collaboration_builder
        .get()
        .and_then(Weak::upgrade)
        .is_none()
        || !enabled(host)
    {
        return;
    }
    if context.effective_tools.is_empty() {
        context.effective_tools = context.tools.clone();
    }
    for descriptor in [
        official::AgentGetTool::descriptor(),
        official::AgentSpawnTool::descriptor(),
        official::ChildWaitTool::descriptor(),
        official::ChildSendTool::descriptor(),
        official::ChildAnswerTool::descriptor(),
        official::ChildCancelTool::descriptor(),
        official::ChildReadTool::descriptor(),
        official::ChildListTool::descriptor(),
        official::ThreadCreateTool::descriptor(),
        official::ThreadSendTool::descriptor(),
        official::ThreadAnswerTool::descriptor(),
        official::ThreadWaitTool::descriptor(),
        official::ThreadReadTool::descriptor(),
        official::ThreadListTool::descriptor(),
        official::ThreadCloseTool::descriptor(),
    ] {
        let tool = EffectiveTool {
            tool_name: descriptor.call_name,
            domain_code: None,
            description: descriptor.description,
            input_schema: descriptor.input_schema,
            skill_name: None,
            required_variable_keys: vec![],
        };
        for tools in [&mut context.tools, &mut context.effective_tools] {
            if !tools.iter().any(|entry| entry.tool_name == tool.tool_name) {
                tools.push(tool.clone());
            }
        }
    }
}
pub(super) fn attach(
    host: &FrameworkHost,
    context: &AgentRunContext,
    request: &mut fw::RunRequest,
) {
    if !enabled(host) {
        return;
    }
    if let Some(builder) = host.0.collaboration_builder.get().and_then(Weak::upgrade) {
        let runtime = Runtime {
            host: Arc::downgrade(&host.0),
            builder: (*builder).clone(),
            context: context.clone(),
        };
        request.extensions.insert(runtime.clone());
        request
            .extensions
            .insert(AgentCollaborationService(Arc::new(runtime)));
    }
}
impl Runtime {
    fn answer_provenance(
        &self,
        parent: &fw::RunContext,
        interaction: fw::RunId,
    ) -> fw::MessageProvenance {
        fw::MessageProvenance {
            sender: fw::MessageSender {
                kind: if self
                    .context
                    .collaboration_owner
                    .as_ref()
                    .is_some_and(|owner| {
                        owner.kind == "ASSISTANT"
                            && self.context.agent_code.as_deref() == Some(owner.code.as_str())
                    }) {
                    "ASSISTANT"
                } else {
                    "AGENT"
                }
                .into(),
                id: self.context.agent_code.clone(),
                display_name: self.context.agent_code.clone(),
                avatar_url: None,
            },
            source: Some("CALLER_INTERACTION".into()),
            correlation: Some(
                serde_json::json!({"callerSessionId":parent.session_id,"interactionId":interaction}),
            ),
        }
    }
    fn host(&self) -> Result<FrameworkHost, fw::AgentError> {
        let host = FrameworkHost(
            self.host
                .upgrade()
                .ok_or_else(|| failure("Agent host is closed"))?,
        );
        host.ensure_open().map_err(failure)?;
        Ok(host)
    }
    pub fn directory(&self) -> Result<Vec<DirectoryAgent>, fw::AgentError> {
        let management = self.builder.agent_management();
        let internal = management
            .internal_agent_directory(&self.context.workspace_code)
            .map_err(|error| failure(format!("Agent directory load failed: {error:?}")))?;
        let mut directory = internal
            .into_iter()
            .map(|agent| DirectoryAgent {
                agent_code: agent.agent_code,
                name: agent.name,
                description: agent.responsibility,
                avatar_id: agent.avatar_id,
                model_id: None,
            })
            .collect::<Vec<_>>();
        let system = if self.context.workspace_code.is_empty() {
            management.list_platform_system_agents()
        } else {
            management.list_workspace_system_agents(&self.context.workspace_code)
        }
        .map_err(|error| failure(format!("System Agent directory load failed: {error:?}")))?;
        directory.extend(
            system
                .into_iter()
                .filter(|agent| agent.enabled && agent.available && agent.model_id.is_some())
                .map(|agent| DirectoryAgent {
                    agent_code: agent.agent_code,
                    name: agent.name,
                    description: agent.description,
                    avatar_id: agent.avatar_id.unwrap_or_else(|| "robot".into()),
                    model_id: agent.model_id,
                }),
        );
        Ok(directory)
    }
    async fn read_agent_details(
        &self,
        code: &str,
        parent: &fw::RunContext,
    ) -> Result<serde_json::Value, fw::AgentError> {
        parent.check_cancelled()?;
        self.host()?;
        let caller = HostExecution::from_extensions(&parent.extensions)?;
        {
            let state = caller.state.lock().await;
            let caller_context = &state.context;
            if caller_context.workspace_code != self.context.workspace_code
                || caller_context.user_code != self.context.user_code
                || caller_context
                    .collaboration_owner
                    .as_ref()
                    .map(|owner| (&owner.kind, &owner.code))
                    != self
                        .context
                        .collaboration_owner
                        .as_ref()
                        .map(|owner| (&owner.kind, &owner.code))
            {
                return Err(failure("Agent details caller scope changed"));
            }
        }
        let entry = self
            .directory()?
            .into_iter()
            .find(|entry| entry.agent_code == code)
            .ok_or_else(|| failure(format!("Agent `{code}` is unavailable in this workspace")))?;
        let management = self.builder.agent_management();
        if entry.model_id.is_some() {
            let runtime = if self.context.workspace_code.is_empty() {
                management.resolve_platform_agent_runtime(Some(code))
            } else {
                management.resolve_workspace_agent_runtime(&self.context.workspace_code, Some(code))
            }
            .map_err(|error| failure(format!("Agent configuration lookup failed: {error:?}")))?;
            return Ok(serde_json::json!({
                "agentCode": entry.agent_code, "name": entry.name, "description": entry.description,
                "runtimeType": runtime.runtime_type, "modelId": runtime.system_model_id,
                "reasoningEffort": runtime.system_reasoning_effort, "fastMode": runtime.system_fast_mode,
            }));
        }
        let policy = if self.context.workspace_code.is_empty() {
            management.platform_internal_agent_policy(code)
        } else {
            management.workspace_internal_agent_policy(&self.context.workspace_code, code)
        }
        .map_err(|error| failure(format!("Agent configuration lookup failed: {error:?}")))?;
        if !policy.enabled {
            return Err(failure(format!("Agent `{code}` is disabled")));
        }
        // Query returns data for this one target only; never model credentials.
        serde_json::to_value(policy).map_err(failure)
    }

    fn authorize(&self, code: &str) -> Result<String, fw::AgentError> {
        self.directory()?
            .into_iter()
            .find(|entry| entry.agent_code == code)
            .map(|entry| entry.agent_code)
            .ok_or_else(|| failure(format!("Agent `{code}` is unavailable in this workspace")))
    }
    async fn prepare_new(
        &self,
        request: fw::RunRequest,
        session: String,
        parent: &fw::RunContext,
        limits: ExecutionLimits,
    ) -> Result<(Agent, fw::RunRequest, AgentRunContext), fw::AgentError> {
        self.prepare_for_caller(request, session, None, parent, limits)
            .await
    }

    async fn prepare_existing(
        &self,
        request: fw::RunRequest,
        session: String,
        bound: &AgentRunContext,
        parent: &fw::RunContext,
        limits: ExecutionLimits,
    ) -> Result<(Agent, fw::RunRequest, AgentRunContext), fw::AgentError> {
        self.prepare_for_caller(request, session, Some(bound), parent, limits)
            .await
    }

    async fn prepare_for_caller(
        &self,
        request: fw::RunRequest,
        session: String,
        bound: Option<&AgentRunContext>,
        parent: &fw::RunContext,
        limits: ExecutionLimits,
    ) -> Result<(Agent, fw::RunRequest, AgentRunContext), fw::AgentError> {
        self.host()?;
        let caller = HostExecution::from_extensions(&parent.extensions)?;
        let caller_context = caller.state.lock().await.context.clone();
        if caller_context.workspace_code != self.context.workspace_code
            || caller_context.user_code != self.context.user_code
            || caller_context
                .collaboration_owner
                .as_ref()
                .map(|owner| (&owner.kind, &owner.code))
                != self
                    .context
                    .collaboration_owner
                    .as_ref()
                    .map(|owner| (&owner.kind, &owner.code))
        {
            return Err(failure("collaboration caller scope changed"));
        }
        self.prepare_owned(
            request,
            session,
            bound.unwrap_or(&caller_context),
            &caller_context,
            caller.harness.clone(),
            Some(parent),
            limits,
            None,
            if bound.is_some() {
                SessionPreparation::Existing
            } else {
                SessionPreparation::New
            },
        )
        .await
    }
    async fn prepare_owned(
        &self,
        mut request: fw::RunRequest,
        session: String,
        base: &AgentRunContext,
        caller_context: &AgentRunContext,
        harness: ReactAgent,
        parent: Option<&fw::RunContext>,
        limits: ExecutionLimits,
        sender: Option<fw::MessageSender>,
        session_preparation: SessionPreparation,
    ) -> Result<(Agent, fw::RunRequest, AgentRunContext), fw::AgentError> {
        let host = self.host()?;
        let code = request
            .agent_code
            .take()
            .or_else(|| base.agent_code.clone())
            .ok_or_else(|| failure("Agent code is required"))?;
        let code = self.authorize(&code)?;
        let text = request.input.text();
        let preparation_prompt = if text.trim().is_empty() && request.resume.is_some() {
            "Continue after the interaction response.".to_owned()
        } else {
            text
        };
        let system_model = self
            .directory()?
            .into_iter()
            .find(|entry| entry.agent_code == code)
            .and_then(|entry| entry.model_id);
        let (model, route, harness) = if let Some(model) = &system_model {
            (
                crate::application::agent::build_system_runtime_model_config(model, None),
                None,
                harness,
            )
        } else {
            let router = self
                .builder
                .model_router()
                .ok_or_else(|| failure("Agent Auto model router is unavailable"))?;
            let (model, route) = router
                .resolve_initial(&base.workspace_code, &session, &code, None)
                .map_err(failure)?;
            (
                model,
                Some(route),
                harness.framework_with_model_router(router),
            )
        };
        let mut command: AgentInvocationCommand = serde_json::from_value(serde_json::json!({
            "prompt": preparation_prompt, "workspaceCode": base.workspace_code, "conversationCode": session,
            "agentCode": code, "model": model, "systemModelId": system_model,
        }))
        .map_err(failure)?;
        // Collaboration questions target the invoking agent. The caller may
        // separately ask its human, then answer this native interaction.
        command.supports_user_input_requests = false;
        command.run_code = request
            .resume
            .as_ref()
            .map(|resume| resume.run_id.to_string());
        let mut execution_scope = base.execution_scope.clone();
        if let Some(directory) = request.working_directory.take() {
            if matches!(session_preparation, SessionPreparation::Existing) {
                return Err(failure(
                    "an existing thread keeps its bound working directory",
                ));
            }
            execution_scope = host
                .delegated_execution_scope(base, &directory)
                .map_err(failure)?;
        }
        command.execution_scope_override = Some(execution_scope);
        // Collaboration uses native sessions; no product chat or checkpoint is fabricated.
        let collaboration_owner = threads::owner(base);
        let mut prepared = self
            .builder
            .clone()
            .with_execution_identity(
                base.workspace_code.is_empty(),
                Some(collaboration_owner.clone()),
            )
            .with_tool_execution_mode(caller_context.tool_execution_mode)
            .prepare(command, base.user_code.clone(), Some(base.locale.clone()))
            .await
            .map_err(|error| failure(error.message()))?;
        let context = &mut prepared.context;
        context.collaboration_owner = Some(collaboration_owner);
        context.conversation_code = Some(session.clone());
        context.stream_key = Some(session.clone());
        context.model = crate::adapters::outbound::model::with_person_ai_conversation(
            context.model.clone(),
            &session,
            Some(&context.run_code),
        );
        context.auto_model_route = route;
        let current_input =
            super::super::messages::decode(&fw::Message::user(request.input.clone()))
                .map_err(failure)?;
        if let Some(memory) = &mut context.short_term_memory {
            memory.current_input_message =
                (!request.input.text().is_empty()).then_some(current_input);
        } else {
            let messages = std::sync::Arc::make_mut(&mut context.messages);
            messages.pop();
            messages.push(current_input);
        }
        if sender.is_none()
            && matches!(
                limits,
                ExecutionLimits::AgentDefinition | ExecutionLimits::AgentDefinitionDetached
            )
        {
            let key = match limits {
                ExecutionLimits::AgentDefinitionDetached => "agent.delegated-background",
                _ => "agent.delegated-sync",
            };
            let text = if let Some(prompts) = host.0.prompt_service.get() {
                prompts
                    .runtime_prompt_text(key, &context.locale, &std::collections::HashMap::new())
                    .map_err(|error| failure(format!("managed developer prompt `{key}`: {error}")))?
                    .ok_or_else(|| {
                        failure(format!("managed developer prompt `{key}` is disabled"))
                    })?
            } else {
                crate::domain::prompt::builtin_prompt_text(key, &context.locale)
                    .ok_or_else(|| failure(format!("bundled developer prompt `{key}` is missing")))?
                    .to_owned()
            };
            let guidance = crate::domain::agent::AgentMessage::system(text)
                .with_instruction_origin(fw::InstructionOrigin::Plugin);
            std::sync::Arc::make_mut(&mut context.messages).push(guidance.clone());
            if let Some(memory) = &mut context.short_term_memory {
                memory.prompt_prefix.push(guidance);
                memory.prompt_prefix_sources.push(None);
            }
        }
        authorize_tools(&host, context);
        let caller_guidance = if let Some(prompts) = host.0.prompt_service.get() {
            prompts
                .runtime_prompt_text(
                    "agent.collaboration-caller",
                    &context.locale,
                    &std::collections::HashMap::new(),
                )
                .map_err(|error| failure(format!("managed caller prompt: {error}")))?
                .ok_or_else(|| failure("managed caller prompt is disabled"))?
        } else {
            crate::domain::prompt::builtin_prompt_text(
                "agent.collaboration-caller",
                &context.locale,
            )
            .ok_or_else(|| failure("bundled caller prompt is missing"))?
            .to_owned()
        };
        let caller_guidance = crate::domain::agent::AgentMessage::system(caller_guidance)
            .with_instruction_origin(fw::InstructionOrigin::Plugin);
        std::sync::Arc::make_mut(&mut context.messages).push(caller_guidance.clone());
        if let Some(memory) = &mut context.short_term_memory {
            memory.prompt_prefix.push(caller_guidance);
            memory.prompt_prefix_sources.push(None);
        }
        let caller_question = official::AskCallerTool::descriptor();
        for tools in [&mut context.tools, &mut context.effective_tools] {
            tools.retain(|tool| {
                !matches!(
                    tool.tool_name.as_str(),
                    "chat_ask_user" | "ask_user" | "a2ui_request_input"
                )
            });
            if !tools
                .iter()
                .any(|tool| tool.tool_name == caller_question.call_name)
            {
                tools.push(EffectiveTool {
                    tool_name: caller_question.call_name.clone(),
                    domain_code: Some("subagent".into()),
                    description: caller_question.description.clone(),
                    input_schema: caller_question.input_schema.clone(),
                    skill_name: None,
                    required_variable_keys: vec![],
                });
            }
        }
        match session_preparation {
            SessionPreparation::New => {
                host.create_collaboration_session(
                    context,
                    parent
                        .map(|parent| parent.session_id.as_str())
                        .unwrap_or(session.as_str()),
                )
                .await?
            }
            SessionPreparation::Existing => host.validate_collaboration_session(context).await?,
        }
        let agent = host
            .collaboration_agent(
                &context.workspace_code,
                &code,
                self.builder.agent_management(),
            )
            .await?;
        request.session_id = session.clone();
        request.run_id = Some(fw::RunId::parse_str(&context.run_code).map_err(failure)?);
        request.model = None;
        request.extensions = fw::Extensions::default();
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
        request.extensions.insert(fw::CallerInteractionScope);
        // Capture the active caller tool span before SDK-owned tasks are started.
        let trace_parent = opentelemetry::Context::current();
        request
            .extensions
            .insert(eliteforge_agent_official_plugin_otel::TraceParent(
                trace_parent,
            ));
        // Ownership is derived from the trusted caller, never tool arguments.
        let mut host_context = parent
            .and_then(fw::RunContext::host_context)
            .unwrap_or_else(|| serde_json::json!({}));
        if let Some(metadata) = host_context.as_object_mut() {
            metadata.insert(
                "collaborationOwner".into(),
                serde_json::to_value(&context.collaboration_owner).map_err(failure)?,
            );
        } else {
            return Err(failure("host collaboration context must be an object"));
        }
        request.host_context = Some(host_context);
        let answered_by = request
            .message_provenance
            .clone()
            .filter(|_| request.resume.is_some());
        request.message_provenance = answered_by.or_else(|| Some(fw::MessageProvenance {
            sender: sender.unwrap_or_else(|| fw::MessageSender {
                kind: if caller_context
                    .collaboration_owner
                    .as_ref()
                    .is_some_and(|owner| {
                        owner.kind == "ASSISTANT"
                            && caller_context.agent_code.as_deref() == Some(owner.code.as_str())
                    }) {
                    "ASSISTANT"
                } else {
                    "AGENT"
                }
                .into(),
                id: caller_context.agent_code.clone(),
                display_name: caller_context.agent_code.clone(),
                avatar_url: None,
            }),
            source: Some("THREAD".into()),
            correlation: Some(
                serde_json::json!({"originSessionCode": parent.map(|parent| &parent.session_id), "initiatorAgentCode": caller_context.agent_code}),
            ),
        }));
        request.assistant_provenance = Some(fw::MessageProvenance {
            sender: fw::MessageSender {
                kind: "AGENT".into(),
                id: Some(code.clone()),
                display_name: Some(code.clone()),
                avatar_url: None,
            },
            source: Some("THREAD".into()),
            correlation: None,
        });
        match limits {
            ExecutionLimits::AgentDefinition | ExecutionLimits::AgentDefinitionDetached => {
                request.limits.max_steps = super::runtime_count_limit(context.limits.max_steps);
                request.limits.max_tool_calls =
                    super::runtime_count_limit(context.limits.max_tool_calls);
            }
            ExecutionLimits::Request => {
                request.limits.max_steps = request
                    .limits
                    .max_steps
                    .min(super::runtime_count_limit(context.limits.max_steps))
                    .min(parent.map_or(usize::MAX, |parent| parent.limits.max_steps));
                request.limits.max_tool_calls = request
                    .limits
                    .max_tool_calls
                    .min(super::runtime_count_limit(context.limits.max_tool_calls));
            }
        }
        request.limits.max_parallel_tools = context.limits.max_parallel_tool_calls.max(1) as usize;
        request.limits.max_total_tokens = request
            .limits
            .max_total_tokens
            .min(parent.map_or(u64::MAX, |parent| parent.token_ceiling));
        let invocation = host.invocation_config();
        request.limits.run_timeout = Duration::from_secs(invocation.agent_run_timeout_seconds);
        request.limits.model_timeout = request.limits.run_timeout;
        request.limits.max_model_retries = if context.auto_model_route.is_some() {
            2
        } else {
            context.model.max_retries.unwrap_or(0).min(3) as usize
        };
        request.limits.max_model_calls_per_step =
            Some((request.limits.max_model_retries + 1).max(3));
        host.attach_tool_policy(&mut request, &context.locale);
        request.limits.max_overflow_recoveries = harness.framework_overflow_limit();
        request.limits.tool_batch_budget_policy = fw::ToolBatchBudgetPolicy::ExecuteAvailable;
        request.limits.max_context_tokens =
            context.model.context_window_tokens.unwrap_or(128_000) as usize;
        request.limits.max_model_output_tokens = context
            .model
            .max_tokens
            .unwrap_or(host.invocation_config().default_output_tokens);
        request.generation.temperature = context.model.temperature;
        request.generation.top_p = context.model.top_p;
        request.generation.max_output_tokens = context.model.max_tokens;
        request
            .extensions
            .insert(fw::ToolResultProcessorService(Arc::new(ToolAudit)));
        host.attach_skill_access(context, &mut request)?;
        // Collaboration/subagent runs resolve their own authorized tool set
        // (`authorize_tools` above), so the same host-owned taxonomy map is
        // published for their packs; nested node runs inherit it.
        request
            .extensions
            .insert(super::super::tools::tool_group_labels(context));
        // Child runs resolve the same Agent scheme; their own authorization
        // scope still decides which bindings exist.
        let native_ids = match host.0.agent_management.get() {
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
        host.attach_memory_backend(context, &mut request.extensions);
        super::file_coordination::attach(&context, &session, &mut request);
        request.extensions.insert(HostExecution {
            harness: harness.clone(),
            state: Arc::new(Mutex::new(HostRunState::new(context.clone()))),
            summaries: host.summary_store().await?,
            sink: None,
        });
        // Human credentials stay in the invoking chat, including nested children.
        if let Some(provider) = parent.and_then(|run| run.extensions.get::<eliteforge_agent_official_plugin_tools_local::SudoPasswordProviderService>()) {
            request.extensions.insert((*provider).clone());
        }
        request.extensions.insert(
            eliteforge_agent_official_plugin_tools_local::CommandProgressSinkService(Arc::new(
                super::super::tools::CommandProgress {
                    harness: harness.clone(),
                    context: context.clone(),
                },
            )),
        );
        tron_harness_capability::TronRunScope {
            platform_scope: context.is_platform_assistant(),
            user_code: context.user_code.clone().unwrap_or_else(|| "system".into()),
            workspace_code: context.workspace_code.clone(),
            session_code: session,
            run_code: context.run_code.clone(),
            app_code: context.execution_scope.app_code.clone(),
            task_code: context.execution_scope.task_code.clone(),
            agent_code: context.agent_code.clone(),
        }
        .attach(&mut request)?;
        if let Some(resources) = &host.0.resources {
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
        attach(&host, context, &mut request);
        if system_model.is_some() {
            host.prepare_collaboration_external(
                prepared.runtime_selection.clone(),
                context,
                &mut request,
            )
            .await?;
        } else {
            harness
                .framework_model_scope(context)
                .map_err(failure)?
                .attach(&mut request);
        }
        let context = context.clone();
        // The native run owns MCP leases even if the caller drops its child waiter.
        request.extensions.insert(prepared.leases);
        Ok((agent, request, context))
    }
}
