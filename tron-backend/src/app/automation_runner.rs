use crate::{
    application::{
        agent::{
            AgentInvocationError, AgentInvocationService, AgentManagementService,
            build_system_runtime_model_config,
        },
        automation::{AutomationExecutionRunner, AutomationService},
        chat::{ChatSendError, ChatService, CreateChatSessionCommand, SendChatMessageCommand},
        model::ModelService,
    },
    domain::{
        automation::{Automation, AutomationExecutionRecord, AutomationExecutionStatus},
        chat::ChatSessionSourceType,
    },
};

const AUTOMATION_AGENT_CODE: &str = "main-agent";
const AUTOMATION_LOCALE: &str = "en-US";

pub(super) struct MainAgentAutomationRunner {
    framework: crate::application::agent::FrameworkHost,
    automations: AutomationService,
    chats: ChatService,
    agent_management: AgentManagementService,
    models: ModelService,
    model_providers: crate::application::model_provider::ModelProviderService,
    agent: AgentInvocationService,
}

impl MainAgentAutomationRunner {
    pub(super) fn new(
        framework: crate::application::agent::FrameworkHost,
        automations: AutomationService,
        chats: ChatService,
        agent_management: AgentManagementService,
        models: ModelService,
        model_providers: crate::application::model_provider::ModelProviderService,
        agent: AgentInvocationService,
    ) -> Self {
        Self {
            framework,
            automations,
            chats,
            agent_management,
            models,
            model_providers,
            agent,
        }
    }
}

impl AutomationExecutionRunner for MainAgentAutomationRunner {
    fn dispatch(&self, automation: Automation, execution: AutomationExecutionRecord) {
        let automations = self.automations.clone();
        let chats = self.chats.clone();
        let agent_management = self.agent_management.clone();
        let models = self.models.clone();
        let model_providers = self.model_providers.clone();
        let agent = self.agent.clone();
        let framework = self.framework.clone();
        crate::app::telemetry::spawn_traced(async move {
            use opentelemetry::{
                Context, KeyValue,
                trace::{FutureExt, Status, TraceContextExt},
            };
            use tracing::Instrument;
            // Each scheduled occurrence owns a fresh root; manual runs retain their HTTP parent.
            let parent = if execution.trigger_source
                == crate::domain::automation::AutomationTriggerSource::Scheduler
            {
                Context::new()
            } else {
                Context::current()
            };
            let telemetry = match framework.telemetry().await {
                Ok(telemetry) => telemetry,
                Err(error) => {
                    tracing::error!(?error, "Automation tracing unavailable");
                    let _ = automations.complete_execution(
                        &automation.automation_code,
                        &execution.execution_code,
                        AutomationExecutionStatus::Failed,
                        Some("Telemetry unavailable".into()),
                        None,
                    );
                    return;
                }
            };
            let guard = telemetry.begin_operation(
                "tron.automation.execute",
                &parent,
                vec![
                    KeyValue::new("tron.workspace.code", automation.workspace_code.clone()),
                    KeyValue::new("tron.automation.code", automation.automation_code.clone()),
                    KeyValue::new(
                        "tron.automation.execution.code",
                        execution.execution_code.clone(),
                    ),
                ],
            );
            let context = guard.0.clone();
            let ids = context.span().span_context().clone();
            let log_span = tracing::info_span!("automation.execute", trace_id=%ids.trace_id(), span_id=%ids.span_id());
            tracing::info!(trace_id=%ids.trace_id(), span_id=%ids.span_id(), automation_code=%automation.automation_code,
                execution_code=%execution.execution_code, "Automation execution started");
            let result = run_automation_with_main_agent(
                automation.clone(),
                execution.clone(),
                automations.clone(),
                chats,
                agent_management,
                models,
                model_providers,
                agent,
            )
            .instrument(log_span)
            .with_context(context.clone())
            .await;
            if matches!(&result, Err(AutomationRunError::Failed(_))) {
                context
                    .span()
                    .set_status(Status::error("Automation execution failed"));
            }
            let (status, summary, session_code) = match result {
                Ok(result) => (
                    AutomationExecutionStatus::Success,
                    Some(result.summary),
                    Some(result.session_code),
                ),
                Err(AutomationRunError::Failed(summary)) => {
                    (AutomationExecutionStatus::Failed, Some(summary), None)
                }
                Err(AutomationRunError::Canceled) => {
                    (AutomationExecutionStatus::Canceled, None, None)
                }
            };
            let _ = automations.complete_execution(
                &automation.automation_code,
                &execution.execution_code,
                status,
                summary,
                session_code,
            );
            if status == AutomationExecutionStatus::Failed {
                tracing::error!(trace_id=%ids.trace_id(), span_id=%ids.span_id(), "Automation execution failed");
            } else if status == AutomationExecutionStatus::Canceled {
                tracing::info!(trace_id=%ids.trace_id(), span_id=%ids.span_id(), "Automation execution canceled");
            } else {
                tracing::info!(trace_id=%ids.trace_id(), span_id=%ids.span_id(), "Automation execution finished");
            }
            guard.complete(if status == AutomationExecutionStatus::Failed {
                "error"
            } else if status == AutomationExecutionStatus::Canceled {
                "cancelled"
            } else {
                "completed"
            });
            drop(guard);
        });
    }
}

struct AutomationRunResult {
    session_code: String,
    summary: String,
}

enum AutomationRunError {
    Failed(String),
    Canceled,
}

impl From<String> for AutomationRunError {
    fn from(summary: String) -> Self {
        Self::Failed(summary)
    }
}

async fn run_automation_with_main_agent(
    automation: Automation,
    execution: AutomationExecutionRecord,
    automations: AutomationService,
    chats: ChatService,
    agent_management: AgentManagementService,
    models: ModelService,
    model_providers: crate::application::model_provider::ModelProviderService,
    agent: AgentInvocationService,
) -> Result<AutomationRunResult, AutomationRunError> {
    let Some(previous) = automations
        .prepare_serial_execution(&automation.automation_code, &execution.execution_code)
        .map_err(|error| {
            AutomationRunError::Failed(format!("Automation execution setup failed: {error:?}"))
        })?
    else {
        return Err(AutomationRunError::Canceled);
    };
    let previous_sessions = previous
        .into_iter()
        .filter_map(|record| record.session_code)
        .collect::<Vec<_>>();
    chats
        .stop_sessions_unchecked(&previous_sessions)
        .await
        .map_err(|error| {
            AutomationRunError::Failed(format!(
                "Could not stop the previous automation run: {error:?}"
            ))
        })?;
    let mut agent = agent;
    let agent_code = automation.agent_code.trim();
    let effective_agent_code = if agent_code.is_empty() {
        AUTOMATION_AGENT_CODE
    } else {
        agent_code
    };
    let resolved_runtime = agent_management
        .resolve_workspace_agent_runtime(&automation.workspace_code, Some(effective_agent_code))
        .map_err(|error| {
            format!(
                "Automation agent `{effective_agent_code}` is unavailable for workspace {}: {error:?}",
                automation.workspace_code
            )
        })?;
    let uses_main_agent = matches!(
        resolved_runtime.runtime_type,
        crate::domain::agent::AgentRuntimeType::React
    );
    let session = chats
        .create_session(
            CreateChatSessionCommand {
                execution_mode: Default::default(),
                workspace_code: Some(automation.workspace_code.clone()),
                project_code: None,
                directory_code: None,
                working_directory_code: None,
                title: Some(format!("Automation: {}", automation.name)),
                app_code: None,
                task_code: None,
                source_type: Some(ChatSessionSourceType::Automation),
                source_code: Some(automation.automation_code.clone()),
                agent_code: Some(resolved_runtime.agent_code.clone()),
            },
            None,
        )
        .map_err(|error| format!("Automation chat session creation failed: {error}"))?;
    automations
        .link_execution_session(
            &automation.automation_code,
            &execution.execution_code,
            &session.session_code,
        )
        .map_err(|error| format!("Automation execution session link failed: {error:?}"))?;
    let (model, model_code, system_model_id) = if uses_main_agent {
        let requested_model = automation
            .model_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let model =
            if requested_model.is_none() || requested_model == Some("PERSON_AI:ROUTE:ChatAuto") {
                let router = crate::application::model_router::AgentModelRouter::new(
                    models.clone(),
                    model_providers.clone(),
                )
                .with_agent_management(agent_management.clone());
                let (model, route) = router
                    .resolve_initial(
                        &automation.workspace_code,
                        &session.session_code,
                        &resolved_runtime.agent_code,
                        None,
                    )
                    .map_err(|error| format!("Automation model routing failed: {error}"))?;
                agent = agent.with_auto_model_route(Some(route), Some(router));
                model
            } else {
                models
                    .text_agent_model_config(&automation.workspace_code, requested_model)
                    .map_err(|error| format!("Text model resolution failed: {error}"))?
                    .ok_or_else(|| {
                        "No matching text model is configured for automation execution.".to_string()
                    })?
            };
        let model_code = model.model_code.clone();
        (model, Some(model_code), None)
    } else {
        let system_model_id = automation
            .model_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                format!(
                    "Automation agent `{}` requires a system model selection.",
                    resolved_runtime.agent_code
                )
            })?
            .to_string();
        let system_agents = agent_management
            .list_workspace_system_agents(&automation.workspace_code)
            .map_err(|error| {
                format!(
                    "Automation agent `{}` model validation failed for workspace {}: {error:?}",
                    resolved_runtime.agent_code, automation.workspace_code
                )
            })?;
        let system_agent = system_agents
            .into_iter()
            .find(|agent| {
                agent
                    .agent_code
                    .eq_ignore_ascii_case(&resolved_runtime.agent_code)
            })
            .ok_or_else(|| {
                format!(
                    "Automation agent `{}` is not registered for workspace {}.",
                    resolved_runtime.agent_code, automation.workspace_code
                )
            })?;
        if !system_agent.enabled || !system_agent.available {
            return Err(AutomationRunError::Failed(format!(
                "Automation agent `{}` is disabled or unavailable for workspace {}.",
                resolved_runtime.agent_code, automation.workspace_code
            )));
        }
        let context_window_tokens = if system_agent.models.is_empty() {
            None
        } else {
            Some(
                system_agent
                    .models
                    .iter()
                    .find(|model| model.model_id == system_model_id)
                    .ok_or_else(|| {
                        format!(
                            "Automation model `{system_model_id}` is not available for agent `{}`.",
                            resolved_runtime.agent_code
                        )
                    })?
                    .context_window_tokens,
            )
            .flatten()
        };
        (
            build_system_runtime_model_config(&system_model_id, context_window_tokens),
            None,
            Some(system_model_id),
        )
    };
    let result = chats
        .send_message(
            &session.session_code,
            SendChatMessageCommand {
                annotations: Vec::new(),
                requested_skills: Vec::new(),
                a2ui_action_id: None,
                a2ui_metadata: None,
                run_code: None,
                content: Some(automation.prompt_text.clone()),
                workspace_code: Some(automation.workspace_code.clone()),
                app_code: None,
                task_code: None,
                agent_code: Some(resolved_runtime.agent_code.clone()),
                model_selection: None,
                model_code,
                system_model_id,
                reasoning_effort: None,
                page_context: None,
                limits: None,
                tools: Vec::new(),
                attachments: Vec::new(),
                client_capabilities: None,
                resume: Vec::new(),
            },
            model,
            None,
            Some(AUTOMATION_LOCALE.to_string()),
            None,
            agent,
        )
        .await
        .map_err(|error| match error {
            ChatSendError::Agent(AgentInvocationError::RunStopped) => AutomationRunError::Canceled,
            error => {
                AutomationRunError::Failed(format!("Main agent automation run failed: {error:?}"))
            }
        })?;
    let session_code = session.session_code.clone();
    let final_reply = result.run.final_output.trim();
    Ok(AutomationRunResult {
        session_code: session_code.clone(),
        summary: if final_reply.is_empty() {
            format!(
                "Agent {} completed automation execution {} in session {} with run {}.",
                resolved_runtime.agent_code,
                execution.execution_code,
                session_code,
                result.run.run_code
            )
        } else {
            final_reply.to_string()
        },
    })
}
