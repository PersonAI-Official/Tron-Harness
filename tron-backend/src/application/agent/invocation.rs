mod agui_warning;
mod attachments;
#[path = "invocation/continuation.rs"]
mod continuation;
mod helpers;
mod run_context_builder;
mod short_term_memory;

pub(crate) use attachments::user_prompt_message;

use std::sync::Arc;

use crate::{
    application::{
        app::AppService, mcp::McpService, prompt::PromptService,
        short_term_memory::ShortTermMemoryService, skill::SkillRegistryService,
        variable::VariableService, workspace::WorkspaceService,
    },
    domain::agent::{
        AgentModelConfig, AgentModelProtocol, AgentNativeRunStatus, AgentRunHistoryRecord,
        AgentRunStatus, AutoModelRouteContext, ModelRequest, ModelResponse, ToolRegistry,
    },
    shared::{event_bus::EventBusHandle, time::current_timestamp},
};

use super::{
    ActiveRunGuard, ActiveRunRegistry, AgentInvocationCommand, AgentInvocationError,
    AgentInvocationResult, AgentManagementService, AgentOrchestrationRun, AgentOrchestrator,
    AgentRunExecutionMetadata, AgentRunExecutor, AgentRunHistoryService, AgentRuntimeRegistry,
    ModelClient, ModelStreamSink,
};
use helpers::safe_preview;
pub(crate) use run_context_builder::{
    AgentRunContextBuilder, PreparedAgentRun, authorized_domain_records, visible_domain_context,
};
#[derive(Clone)]
pub struct AgentInvocationService {
    tool_execution_mode: crate::domain::agent::ToolExecutionMode,
    platform_scope: bool,
    collaboration_owner: Option<crate::domain::agent::AgentCollaborationOwner>,
    runtime_input: Option<(String, serde_json::Value)>,
    assistant_work_context: Option<crate::application::assistant::AssistantService>,
    workspaces: WorkspaceService,
    apps: AppService,
    variables: VariableService,
    skills: SkillRegistryService,
    management: AgentManagementService,
    history: AgentRunHistoryService,
    mcp: McpService,
    event_bus: EventBusHandle,
    model_client: Arc<dyn ModelClient>,
    tool_registry: Arc<ToolRegistry>,
    runtime_http_client: reqwest::Client,
    directory_inspector:
        Option<Arc<dyn crate::application::project::ApplicationDirectoryInspector>>,
    short_term_memory: Option<ShortTermMemoryService>,
    prompt_service: Option<PromptService>,
    auto_model_route: Option<AutoModelRouteContext>,
    auto_model_router: Option<crate::application::model_router::AgentModelRouter>,
    active_runs: ActiveRunRegistry,
    framework_host: Option<super::FrameworkHost>,
}

impl AgentInvocationService {
    pub(crate) fn runtime_type_for_admission(
        &self,
        workspace: &str,
        code: &str,
    ) -> Result<crate::domain::agent::AgentRuntimeType, AgentInvocationError> {
        let runtime = if self.platform_scope {
            self.management.resolve_platform_agent_runtime(Some(code))
        } else {
            self.management
                .resolve_workspace_agent_runtime(workspace, Some(code))
        };
        runtime
            .map(|runtime| runtime.runtime_type)
            .map_err(|error| {
                AgentInvocationError::RuntimeFailed(format!(
                    "agent runtime resolution failed: {error:?}"
                ))
            })
    }

    pub fn new(
        workspaces: WorkspaceService,
        apps: AppService,
        variables: VariableService,
        skills: SkillRegistryService,
        management: AgentManagementService,
        history: AgentRunHistoryService,
        mcp: McpService,
        event_bus: EventBusHandle,
        model_client: Arc<dyn ModelClient>,
        tool_registry: Arc<ToolRegistry>,
        runtime_http_client: reqwest::Client,
    ) -> Self {
        Self {
            tool_execution_mode: Default::default(),
            platform_scope: false,
            collaboration_owner: None,
            runtime_input: None,
            assistant_work_context: None,
            workspaces,
            apps,
            variables,
            skills,
            management,
            history,
            mcp,
            event_bus,
            model_client,
            tool_registry,
            runtime_http_client,
            directory_inspector: None,
            short_term_memory: None,
            prompt_service: None,
            auto_model_route: None,
            auto_model_router: None,
            active_runs: ActiveRunRegistry::new(),
            framework_host: None,
        }
    }

    /// Enable platform execution on a host-owned service clone. HTTP request payloads cannot set this.
    pub fn with_platform_scope(mut self) -> Self {
        self.platform_scope = true;
        self
    }

    pub(crate) fn with_tool_execution_mode(
        mut self,
        mode: crate::domain::agent::ToolExecutionMode,
    ) -> Self {
        self.tool_execution_mode = mode;
        self
    }

    pub fn with_collaboration_owner(
        mut self,
        owner: crate::domain::agent::AgentCollaborationOwner,
    ) -> Self {
        self.collaboration_owner = Some(owner);
        self
    }

    /// Host-only input identity. Public chat payloads cannot impersonate a scheduler or a PersonAI.
    pub(crate) fn with_runtime_input(
        mut self,
        kind: impl Into<String>,
        metadata: serde_json::Value,
    ) -> Self {
        self.runtime_input = Some((kind.into(), metadata));
        self
    }

    pub(crate) fn runtime_input_actor(&self) -> Option<&str> {
        self.runtime_input
            .as_ref()
            .and_then(|(_, metadata)| metadata.get("source").and_then(serde_json::Value::as_str))
    }

    pub(crate) fn with_assistant_work_context(
        mut self,
        assistants: crate::application::assistant::AssistantService,
    ) -> Self {
        self.assistant_work_context = Some(assistants);
        self
    }

    pub fn with_framework_host(mut self, host: super::FrameworkHost) -> Self {
        self.framework_host = Some(host);
        self
    }

    pub fn with_directory_inspector(
        mut self,
        inspector: Option<Arc<dyn crate::application::project::ApplicationDirectoryInspector>>,
    ) -> Self {
        self.directory_inspector = inspector;
        self
    }

    pub fn with_short_term_memory(mut self, service: ShortTermMemoryService) -> Self {
        self.short_term_memory = Some(service);
        self
    }

    pub fn with_prompt_service(mut self, service: PromptService) -> Self {
        self.prompt_service = Some(service);
        self
    }

    pub fn with_auto_model_route(
        mut self,
        route: Option<AutoModelRouteContext>,
        router: Option<crate::application::model_router::AgentModelRouter>,
    ) -> Self {
        self.auto_model_route = route;
        self.auto_model_router = router;
        self
    }

    pub fn with_active_runs(mut self, active_runs: ActiveRunRegistry) -> Self {
        self.active_runs = active_runs;
        self
    }

    pub fn short_term_memory(&self) -> Option<ShortTermMemoryService> {
        self.short_term_memory.clone()
    }

    pub(crate) fn collaboration_harness(
        &self,
    ) -> Result<super::runtime::ReactAgent, crate::domain::agent::AgentError> {
        super::ReactAgentBuilder::new(
            self.model_client.clone(),
            self.tool_registry.clone(),
            self.event_bus.clone(),
        )
        .with_framework_host(self.framework_host.clone())
        .with_directory_inspector(self.directory_inspector.clone())
        .with_short_term_memory(self.short_term_memory.clone())
        .with_auto_model_router(self.auto_model_router.clone())
        .build()
    }

    pub(crate) fn run_context_builder(&self) -> AgentRunContextBuilder {
        let builder = AgentRunContextBuilder::new(
            self.workspaces.clone(),
            self.apps.clone(),
            self.variables.clone(),
            self.skills.clone(),
            self.management.clone(),
            self.mcp.clone(),
            self.event_bus.clone(),
            self.tool_registry.clone(),
            self.short_term_memory.clone(),
            self.prompt_service.clone(),
            self.auto_model_route.clone(),
            self.directory_inspector.clone(),
        )
        .with_framework_host(self.framework_host.clone())
        .with_execution_identity(self.platform_scope, self.collaboration_owner.clone())
        .with_tool_execution_mode(self.tool_execution_mode)
        .with_assistant_work_context(self.assistant_work_context.clone());
        match &self.auto_model_router {
            Some(router) => builder.with_model_router(router.clone()),
            None => builder,
        }
    }

    pub async fn generate_stateless_model(
        &self,
        model: &AgentModelConfig,
        request: &ModelRequest,
    ) -> Result<ModelResponse, AgentInvocationError> {
        self.model_client
            .generate(model, request)
            .await
            .map_err(AgentInvocationError::ModelFailed)
    }

    pub async fn invoke(
        &self,
        command: AgentInvocationCommand,
        user_code: Option<String>,
        locale: Option<String>,
    ) -> Result<AgentInvocationResult, AgentInvocationError> {
        self.invoke_with_stream(command, user_code, locale, None, None)
            .await
    }

    pub async fn invoke_stream(
        &self,
        command: AgentInvocationCommand,
        user_code: Option<String>,
        locale: Option<String>,
        sink: ModelStreamSink,
    ) -> Result<AgentInvocationResult, AgentInvocationError> {
        self.invoke_with_stream(command, user_code, locale, Some(sink), None)
            .await
    }

    pub(crate) async fn invoke_reserved(
        &self,
        command: AgentInvocationCommand,
        user_code: Option<String>,
        locale: Option<String>,
        reservation: ActiveRunGuard,
    ) -> Result<AgentInvocationResult, AgentInvocationError> {
        self.invoke_with_stream(command, user_code, locale, None, Some(reservation))
            .await
    }

    pub(crate) async fn invoke_stream_reserved(
        &self,
        command: AgentInvocationCommand,
        user_code: Option<String>,
        locale: Option<String>,
        sink: ModelStreamSink,
        reservation: ActiveRunGuard,
    ) -> Result<AgentInvocationResult, AgentInvocationError> {
        self.invoke_with_stream(command, user_code, locale, Some(sink), Some(reservation))
            .await
    }

    async fn invoke_with_stream(
        &self,
        mut command: AgentInvocationCommand,
        user_code: Option<String>,
        locale: Option<String>,
        sink: Option<ModelStreamSink>,
        reservation: Option<ActiveRunGuard>,
    ) -> Result<AgentInvocationResult, AgentInvocationError> {
        if let Some((kind, metadata)) = &self.runtime_input {
            command.transcript_input_kind = Some(kind.clone());
            let mut merged = command
                .transcript_input_metadata
                .take()
                .unwrap_or_else(|| serde_json::json!({}));
            if let (Some(target), Some(source)) = (merged.as_object_mut(), metadata.as_object()) {
                target.extend(source.clone());
            }
            command.transcript_input_metadata = Some(merged);
        }
        let runtime_registry = AgentRuntimeRegistry::new(
            self.model_client.clone(),
            self.tool_registry.clone(),
            self.event_bus.clone(),
            self.runtime_http_client.clone(),
        )
        .with_directory_inspector(self.directory_inspector.clone())
        .with_short_term_memory(self.short_term_memory.clone())
        .with_framework_host(self.framework_host.clone())
        .with_active_runs(self.active_runs.clone())
        .with_auto_model_router(self.auto_model_router.clone());
        let executor = AgentRunExecutor::new(runtime_registry);
        let orchestrator = AgentOrchestrator::new(self.run_context_builder(), executor);
        let AgentOrchestrationRun {
            execution_mode: _execution_mode,
            plan_steps: _plan_steps,
            collaboration: _collaboration,
            metadata,
            execution,
        } = orchestrator
            .execute(command, user_code, locale, sink, reservation)
            .await?;
        let AgentRunExecutionMetadata {
            run_code,
            conversation_code,
            workspace_code,
            user_code,
            app_code,
            task_code,
            model_code,
            reasoning_effort,
            started_at,
        } = metadata;

        match execution {
            Ok(orchestrated) => {
                let result = orchestrated.run;
                let native_status = result.native.as_ref().map(|native| native.status);
                let awaiting_user_input = match native_status {
                    Some(status) => status == AgentNativeRunStatus::AwaitingInput,
                    None => result
                        .observations
                        .iter()
                        .any(|observation| observation.tool_name == super::CHAT_ASK_USER_TOOL),
                };
                let limit_error = match native_status {
                    Some(AgentNativeRunStatus::StepLimit) => {
                        Some(AgentInvocationError::MaxStepsExceeded)
                    }
                    Some(AgentNativeRunStatus::ToolCallLimit) => {
                        Some(AgentInvocationError::MaxToolCallsExceeded)
                    }
                    _ => None,
                };
                let status = match native_status {
                    Some(AgentNativeRunStatus::StepLimit) => "STEP_LIMIT",
                    Some(AgentNativeRunStatus::ToolCallLimit) => "TOOL_CALL_LIMIT",
                    _ if awaiting_user_input => "AWAITING_USER_INPUT",
                    _ => "COMPLETED",
                };
                let finished_at = current_timestamp();
                let token_usage = result.token_usage;
                self.history
                    .create_run(AgentRunHistoryRecord {
                        id: 0,
                        run_code: result.run_code.clone(),
                        conversation_code: result.conversation_code.clone(),
                        workspace_code: result.workspace_code.clone(),
                        user_code,
                        app_code,
                        task_code,
                        model_code: model_code.clone(),
                        reasoning_effort,
                        status: if limit_error.is_some() {
                            AgentRunStatus::Failed
                        } else {
                            AgentRunStatus::Completed
                        },
                        outcome: match native_status {
                            Some(AgentNativeRunStatus::StepLimit) => "step_limit".into(),
                            Some(AgentNativeRunStatus::ToolCallLimit) => "tool_call_limit".into(),
                            _ if awaiting_user_input => "awaiting_user_input".into(),
                            _ => "completed".into(),
                        },
                        step_count: result.step_count,
                        observation_count: result.observations.len() as u32,
                        input_tokens: token_usage.map(|usage| usage.input_tokens),
                        output_tokens: token_usage.map(|usage| usage.output_tokens),
                        total_tokens: token_usage.map(|usage| usage.total_tokens),
                        output_preview: Some(safe_preview("output", &result.final_output)),
                        error_code: limit_error.as_ref().map(|error| error.code().into()),
                        error_message: limit_error.as_ref().map(AgentInvocationError::message),
                        started_at,
                        finished_at,
                        created_at: current_timestamp(),
                    })
                    .map_err(|_| AgentInvocationError::HistoryStorageFailed)?;
                Ok(AgentInvocationResult {
                    run_code: result.run_code,
                    conversation_code: result.conversation_code,
                    workspace_code: result.workspace_code,
                    model_code,
                    runtime_adapter_key: orchestrated.runtime_adapter_key,
                    reasoning_effort,
                    status,
                    native: result.native,
                    sdk_execution: result.sdk_execution,
                    final_output: result.final_output,
                    final_message_ids: result.final_message_ids,
                    step_count: result.step_count,
                    token_usage,
                    latest_request_estimate: result.latest_request_estimate,
                    latest_usage_audit: result.latest_usage_audit,
                    system_agent_session_id: result.system_agent_session_id,
                    observations: result.observations,
                    artifact_records: result.artifact_records,
                })
            }
            Err(invocation_error) => {
                let finished_at = current_timestamp();
                self.history
                    .create_run(AgentRunHistoryRecord {
                        id: 0,
                        run_code,
                        conversation_code,
                        workspace_code,
                        user_code,
                        app_code,
                        task_code,
                        model_code,
                        reasoning_effort,
                        status: AgentRunStatus::Failed,
                        outcome: "failed".into(),
                        step_count: 0,
                        observation_count: 0,
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                        output_preview: None,
                        error_code: Some(invocation_error.code().into()),
                        error_message: Some(safe_preview(
                            "diagnostic",
                            &invocation_error.message(),
                        )),
                        started_at,
                        finished_at,
                        created_at: current_timestamp(),
                    })
                    .map_err(|_| AgentInvocationError::HistoryStorageFailed)?;
                Err(invocation_error)
            }
        }
    }
}

pub fn generate_agent_run_code() -> String {
    eliteforge_agent_core::RunId::new_v4().to_string()
}

pub(crate) fn build_system_runtime_model_config(
    model_name: &str,
    context_window_tokens: Option<u32>,
) -> AgentModelConfig {
    AgentModelConfig {
        model_code: "SYSTEM_AGENT_MODEL".into(),
        protocol: AgentModelProtocol::ChatCompletions,
        base_url: "http://system-agent.internal".into(),
        api_path: None,
        api_key: "sk-system-agent".into(),
        request_headers: Default::default(),
        model_name: model_name.to_string(),
        thinking_enabled: true,
        supports_developer_role: false,
        reasoning_effort: None,
        responses_stateful_enabled: false,
        temperature: None,
        max_tokens: None,
        context_window_tokens,
        top_p: None,
        max_retries: Some(0),
    }
}
