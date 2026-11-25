use crate::{
    application::{
        agent::{
            AgentAguiStreamService, AgentInvocationError, AgentInvocationService,
            AgentManagementService, generate_agent_run_code,
        },
        automation::{AutomationExecutionRunner, AutomationService},
        chat::{
            ChatSendError, ChatService, CreateChatSessionCommand, SendChatMessageCommand,
            model_resolution::prepare_send_model,
        },
        model::ModelService,
    },
    domain::{
        automation::{Automation, AutomationExecutionRecord, AutomationExecutionStatus},
        chat::ChatSessionSourceType,
        model_provider::ModelSelection,
    },
};

const AUTOMATION_AGENT_CODE: &str = "main-agent";
const AUTOMATION_LOCALE: &str = "en-US";

pub(super) struct MainAgentAutomationRunner {
    framework: crate::application::agent::FrameworkHost,
    automations: AutomationService,
    chats: ChatService,
    presentation: AgentAguiStreamService,
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
        presentation: AgentAguiStreamService,
        agent_management: AgentManagementService,
        models: ModelService,
        model_providers: crate::application::model_provider::ModelProviderService,
        agent: AgentInvocationService,
    ) -> Self {
        Self {
            framework,
            automations,
            chats,
            presentation,
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
        let presentation = self.presentation.clone();
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
                presentation,
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
    presentation: AgentAguiStreamService,
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
    let agent_code = automation.agent_code.trim();
    let effective_agent_code = if agent_code.is_empty() {
        AUTOMATION_AGENT_CODE
    } else {
        agent_code
    };
    let mut resolved_runtime = agent_management
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
    if let Err(error) = automations.link_execution_session(
        &automation.automation_code,
        &execution.execution_code,
        &session.session_code,
    ) {
        if let Err(cleanup_error) = chats.delete_session(&session.session_code)
            && !matches!(
                cleanup_error,
                crate::domain::chat::ChatError::SessionNotFound
            )
        {
            tracing::warn!(%cleanup_error, session_code = %session.session_code, "Automation session cleanup failed");
        }
        return Err(format!("Automation execution session link failed: {error:?}").into());
    }
    let requested_model = automation
        .model_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let model_selection = if uses_main_agent {
        requested_model
            .as_deref()
            .is_none_or(|code| code == "PERSON_AI:ROUTE:ChatAuto")
            .then(ModelSelection::auto)
    } else {
        // A configured automation uses its explicit system model.
        resolved_runtime.system_model_id = requested_model.clone();
        None
    };
    let run_code = generate_agent_run_code();
    let request = SendChatMessageCommand {
        annotations: Vec::new(),
        requested_skills: Vec::new(),
        a2ui_action_id: None,
        a2ui_metadata: None,
        run_code: Some(run_code.clone()),
        content: Some(automation.prompt_text.clone()),
        workspace_code: Some(automation.workspace_code.clone()),
        app_code: None,
        task_code: None,
        agent_code: Some(resolved_runtime.agent_code.clone()),
        model_selection,
        model_code: requested_model,
        system_model_id: resolved_runtime.system_model_id.clone(),
        reasoning_effort: None,
        page_context: None,
        limits: None,
        tools: Vec::new(),
        attachments: Vec::new(),
        client_capabilities: None,
        resume: Vec::new(),
    };
    let prepared = prepare_send_model(
        &models,
        &model_providers,
        &agent_management,
        &session,
        &request,
        &resolved_runtime,
    )
    .map_err(|error| {
        tracing::warn!(?error, "Automation model resolution failed");
        tron_i18n::t!(
            "automation.execution.model_resolution_failed",
            locale = AUTOMATION_LOCALE
        )
        .into_owned()
    })?;
    let agent = agent.with_auto_model_route(prepared.auto_route, prepared.auto_router);
    let model = crate::adapters::outbound::model::with_person_ai_conversation(
        prepared.config,
        &session.session_code,
        Some(&run_code),
    );
    let result = chats
        .send_background_message(
            &session.session_code,
            request,
            model,
            None,
            AUTOMATION_LOCALE.to_owned(),
            agent,
            &presentation,
            None,
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
