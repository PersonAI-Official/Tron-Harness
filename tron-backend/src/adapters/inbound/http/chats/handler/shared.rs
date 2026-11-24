use axum::response::sse::Event;
use serde_json::{Value, json};
use tron_i18n::t;

use crate::{
    app::AppState,
    application::{
        agent::{AgentInvocationError, ResolvedAgentRuntime, build_system_runtime_model_config},
        chat::{ChatSendError, ChatSendResult, SendChatMessageCommand},
    },
    domain::{
        agent::{AgentModelConfig, AgentRuntimeType, ManagedSystemAgentRecord},
        chat::{ChatError, ChatSession},
        model_provider::{
            ModelProviderCode, ModelSelection, ModelSelectionKind, ProviderRouteCode,
        },
    },
    shared::locale::SupportedLocale,
};

pub(crate) fn protocol_event_belongs_to_run(data: &Value, run_code: &str) -> bool {
    data.get("runId").and_then(Value::as_str) == Some(run_code)
}

#[derive(Debug)]
pub(crate) struct ChatSsePayload {
    pub(crate) event_name: String,
    pub(crate) data: Value,
    /// Request locale, kept so the serialization fallback can be localized even
    /// after the middleware scope has handed the body stream to the server.
    pub(crate) locale: SupportedLocale,
}

impl ChatSsePayload {
    pub(crate) fn agui(
        event_name: impl Into<String>,
        mut data: Value,
        locale: SupportedLocale,
    ) -> Self {
        let event_name = event_name.into();
        if event_name == "RUN_ERROR" {
            // Framework details can be raw transport diagnostics; localize them
            // at the request boundary and keep the original detail in server logs.
            let code = data.get("code").and_then(Value::as_str).unwrap_or_default();
            let detail = data
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default();
            data["message"] = json!(super::super::error_messages::message(
                code, detail, locale
            ));
        }
        Self {
            event_name,
            data,
            locale,
        }
    }

    pub(crate) fn event(&self) -> Event {
        Event::default()
            .event(self.event_name.clone())
            .json_data(&self.data)
            .unwrap_or_else(|_| {
                Event::default()
                    .event("RUN_ERROR")
                    .data(json!({"type":"RUN_ERROR","code":"CHAT_STREAM_SERIALIZATION_FAILED","message":t!("error.chat.stream_serialization_failed", locale = self.locale)}).to_string())
            })
    }
}

pub(crate) enum ChatStreamCompletion {
    Success(ChatSendResult),
    Error { code: String, message: String },
}

impl ChatStreamCompletion {
    pub(crate) fn payload(
        &self,
        session_code: &str,
        run_code: &str,
        locale: SupportedLocale,
    ) -> ChatSsePayload {
        match self {
            Self::Success(result) => {
                if matches!(result.run.status, "STEP_LIMIT" | "TOOL_CALL_LIMIT") {
                    return ChatSsePayload::agui(
                        "RUN_ERROR",
                        json!({
                            "type": "RUN_ERROR", "threadId": session_code, "runId": result.run.run_code,
                            "code": result.run.status, "message": super::super::error_messages::message(result.run.status, "", locale),
                            "result": result, "timestamp": chrono::Utc::now().timestamp_millis(),
                        }),
                        locale,
                    );
                }
                let outcome = result
                    .user_input_request
                    .as_ref()
                    .map(|request| {
                        json!({
                            "type": "interrupt",
                            "interrupts": [{
                                "id": request.request_code,
                                "reason": "input_required",
                                "message": request.intro,
                                "metadata": {
                                    "requestCode": request.request_code,
                                    "runCode": request.run_code,
                                    "questions": request.questions,
                                },
                            }],
                        })
                    })
                    .or_else(|| native_interrupt_outcome(result.run.native.as_ref()))
                    .unwrap_or_else(|| json!({ "type": "success" }));
                ChatSsePayload::agui(
                    "RUN_FINISHED",
                    json!({
                        "type": "RUN_FINISHED",
                        "threadId": session_code,
                        "runId": result.run.run_code,
                        "outcome": outcome,
                        "result": result,
                        "timestamp": chrono::Utc::now().timestamp_millis(),
                    }),
                    locale,
                )
            }
            Self::Error { code, message } => ChatSsePayload::agui(
                "RUN_ERROR",
                json!({
                    "type": "RUN_ERROR",
                    "threadId": session_code,
                    "runId": run_code,
                    "code": code,
                    "message": message,
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                }),
                locale,
            ),
        }
    }
}

pub(crate) fn chat_send_error_code(error: &ChatSendError) -> &'static str {
    match error {
        ChatSendError::Chat(ChatError::AttachmentInvalid(_)) => "CHAT_ATTACHMENT_INVALID",
        ChatSendError::Chat(ChatError::AttachmentNotFound) => "CHAT_ATTACHMENT_NOT_FOUND",
        ChatSendError::Chat(ChatError::ArtifactNotFound) => "CHAT_ARTIFACT_NOT_FOUND",
        ChatSendError::Chat(ChatError::ArtifactChanged) => "CHAT_ARTIFACT_CHANGED",
        ChatSendError::Chat(ChatError::ArtifactUnavailable) => "CHAT_ARTIFACT_UNAVAILABLE",
        ChatSendError::Chat(
            error @ (ChatError::AnnotationsInvalid
            | ChatError::RequestedSkillsInvalid
            | ChatError::RequestedSkillForbidden
            | ChatError::GoalCommandInvalid
            | ChatError::GoalCommandUnavailable
            | ChatError::GoalCommandConflict),
        ) => {
            crate::application::chat::composer_input::descriptor(error)
                .unwrap()
                .code
        }
        ChatSendError::Chat(ChatError::MessageRequired) => "CHAT_MESSAGE_REQUIRED",
        ChatSendError::Chat(ChatError::SessionNotFound) => "CHAT_SESSION_NOT_FOUND",
        ChatSendError::Chat(ChatError::SessionMustBeArchived) => "CHAT_SESSION_MUST_BE_ARCHIVED",
        ChatSendError::Chat(ChatError::SessionArchived) => "CHAT_SESSION_ARCHIVED",
        ChatSendError::Chat(ChatError::SessionRunActive) => "CHAT_SESSION_RUN_ACTIVE",
        ChatSendError::Chat(ChatError::ContainerDeleteRejected) => "CHAT_CONTAINER_DELETE_REJECTED",
        ChatSendError::Chat(ChatError::SessionLocationChanged) => "CHAT_SESSION_LOCATION_CHANGED",
        ChatSendError::Chat(ChatError::SessionTitleRequired) => "CHAT_SESSION_TITLE_REQUIRED",
        ChatSendError::Chat(ChatError::WorkspaceRequired) => "CHAT_WORKSPACE_REQUIRED",
        ChatSendError::Chat(ChatError::WorkspaceForbidden) => "CHAT_WORKSPACE_FORBIDDEN",
        ChatSendError::Chat(ChatError::ProjectRequired) => "CHAT_PROJECT_REQUIRED",
        ChatSendError::Chat(ChatError::ProjectNameConflict) => {
            crate::application::chat::ArchiveError::TopicNameConflict
                .descriptor()
                .code
        }
        ChatSendError::Chat(ChatError::ProjectInvalid) => "CHAT_PROJECT_INVALID",
        ChatSendError::Chat(ChatError::DirectoryFixed) => "CHAT_DIRECTORY_FIXED",
        ChatSendError::Chat(ChatError::DirectoryInvalid) => "CHAT_DIRECTORY_INVALID",
        ChatSendError::Chat(ChatError::UserInputAlreadyAnswered) => {
            "CHAT_USER_INPUT_ALREADY_ANSWERED"
        }
        ChatSendError::Chat(ChatError::UserInputAnswerInvalid(_)) => {
            "CHAT_USER_INPUT_ANSWER_INVALID"
        }
        ChatSendError::Chat(ChatError::UserInputPending) => "CHAT_USER_INPUT_PENDING",
        ChatSendError::Chat(ChatError::UserInputRequestNotFound) => {
            "CHAT_USER_INPUT_REQUEST_NOT_FOUND"
        }
        ChatSendError::Chat(ChatError::StorageFailure) => "CHAT_STORAGE_FAILED",
        ChatSendError::Agent(error) => error.code(),
        ChatSendError::RunAlreadyActive { .. } => "CHAT_RUN_ALREADY_ACTIVE",
    }
}

pub(crate) fn chat_send_error_message(error: ChatSendError, locale: SupportedLocale) -> String {
    if let ChatSendError::Chat(ref chat_error) = error {
        if crate::application::chat::composer_input::descriptor(chat_error).is_some() {
            return super::super::error_mapper::composer_error_message_for(chat_error, locale);
        }
    }
    let code = chat_send_error_code(&error);
    let detail = match &error {
        ChatSendError::Agent(error) => error.message(),
        ChatSendError::Chat(error) => format!("{error:?}"),
        ChatSendError::RunAlreadyActive { .. } => String::new(),
    };
    super::super::error_messages::message(code, &detail, locale)
}

pub(crate) fn normalized_required(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalized_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn effective_agent_code(
    session: &ChatSession,
    request: &SendChatMessageCommand,
) -> String {
    let selected = if session.last_run_code.is_some()
        || session.source_code.as_deref().is_some_and(|source| {
            source.starts_with("__assistant:") || source.starts_with("__knowledge:")
        }) {
        session.agent_code.clone()
    } else {
        normalized_optional(request.agent_code.clone()).or(session.agent_code.clone())
    };
    selected.unwrap_or_else(|| "main-agent".to_string())
}

pub(crate) fn resolve_chat_agent_runtime(
    state: &AppState,
    session: &ChatSession,
    request: &SendChatMessageCommand,
) -> Result<ResolvedAgentRuntime, ChatSendError> {
    let effective_agent_code = effective_agent_code(session, request);
    let runtime = if session.source_code.as_deref().is_some_and(|source| {
        source.starts_with("__assistant:") || source.starts_with("__knowledge:")
    }) {
        state
            .agent_management
            .resolve_platform_agent_runtime(Some(effective_agent_code.as_str()))
    } else {
        state.agent_management.resolve_workspace_agent_runtime(
            &session.workspace_code,
            Some(effective_agent_code.as_str()),
        )
    };
    runtime.map_err(|error| match error {
        crate::domain::agent::AgentManagementError::InvalidWorkspace => {
            ChatSendError::Chat(ChatError::WorkspaceForbidden)
        }
        crate::domain::agent::AgentManagementError::SystemAgentNotFound => {
            ChatSendError::Agent(AgentInvocationError::RuntimeFailed(format!(
                "agent `{effective_agent_code}` was not found"
            )))
        }
        crate::domain::agent::AgentManagementError::SystemAgentUnavailable => {
            ChatSendError::Agent(AgentInvocationError::RuntimeFailed(format!(
                "agent `{effective_agent_code}` is unavailable or disabled for this workspace"
            )))
        }
        crate::domain::agent::AgentManagementError::InternalAgentNotFound
        | crate::domain::agent::AgentManagementError::InternalAgentCodeInvalid
        | crate::domain::agent::AgentManagementError::InternalAgentCodeConflict
        | crate::domain::agent::AgentManagementError::BuiltinAgentReadOnly
        | crate::domain::agent::AgentManagementError::PlatformAgentReadOnly
        | crate::domain::agent::AgentManagementError::InternalAgentGroupInvalid
        | crate::domain::agent::AgentManagementError::InternalAgentDefinitionInvalid
        | crate::domain::agent::AgentManagementError::InternalAgentPolicyInvalid
        | crate::domain::agent::AgentManagementError::ToolPermissionInvalid
        | crate::domain::agent::AgentManagementError::InternalMcpServerAssignmentStale(_)
        | crate::domain::agent::AgentManagementError::InternalToolAssignmentStale(_)
        | crate::domain::agent::AgentManagementError::InternalToolUnknown(_) => {
            ChatSendError::Agent(AgentInvocationError::RuntimeFailed(format!(
                "agent `{effective_agent_code}` is not available for this workspace"
            )))
        }
        crate::domain::agent::AgentManagementError::StorageFailure => ChatSendError::Agent(
            AgentInvocationError::RuntimeFailed("agent runtime resolution failed".into()),
        ),
    })
}

pub(crate) fn prepare_send_model(
    state: &AppState,
    session: &ChatSession,
    request: &SendChatMessageCommand,
    runtime: &ResolvedAgentRuntime,
) -> Result<PreparedChatModel, ChatSendError> {
    if let Some(selection) = request.model_selection.as_ref() {
        validate_chat_model_selection(selection)?;
    }
    if matches!(runtime.runtime_type, AgentRuntimeType::React) {
        let is_auto = request.model_selection.as_ref().is_some_and(|selection| {
            selection.kind == crate::domain::model_provider::ModelSelectionKind::Auto
                || (selection.kind
                    == crate::domain::model_provider::ModelSelectionKind::ProviderRoute
                    && selection.route_code
                        == Some(crate::domain::model_provider::ProviderRouteCode::ChatAuto))
        });
        if is_auto
            || (session.workspace_code.is_empty()
                && request.model_selection.is_none()
                && request
                    .model_code
                    .as_deref()
                    .is_none_or(|code| code.trim().is_empty()))
        {
            let router = crate::application::model_router::AgentModelRouter::new(
                state.models.clone(),
                state.model_providers.clone(),
            )
            .with_agent_management(state.agent_management.clone());
            let (model, auto_route) = router
                .resolve_initial(
                    &session.workspace_code,
                    &session.session_code,
                    &runtime.agent_code,
                    request.reasoning_effort,
                )
                .map_err(|message| {
                    ChatSendError::Agent(AgentInvocationError::RuntimeFailed(message))
                })?;
            return Ok(PreparedChatModel {
                config: model,
                auto_route: Some(auto_route),
                auto_router: Some(router),
            });
        }
        let fixed_model_code = request.model_selection.as_ref().and_then(|selection| {
            (selection.kind == crate::domain::model_provider::ModelSelectionKind::Fixed
                && selection.provider_code.is_none())
            .then(|| {
                selection
                    .model_code
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })
            .flatten()
        });
        match state.model_providers.resolve_chat_selection(
            &session.workspace_code,
            request.model_selection.as_ref(),
            request.model_code.as_deref(),
            request.reasoning_effort,
        ) {
            Ok(Some(config)) => {
                if config.model_code == "PERSON_AI:ROUTE:ChatAuto" {
                    let router = crate::application::model_router::AgentModelRouter::new(
                        state.models.clone(),
                        state.model_providers.clone(),
                    )
                    .with_agent_management(state.agent_management.clone());
                    let (model, auto_route) = router
                        .resolve_initial(
                            &session.workspace_code,
                            &session.session_code,
                            &runtime.agent_code,
                            request.reasoning_effort,
                        )
                        .map_err(|message| {
                            ChatSendError::Agent(AgentInvocationError::RuntimeFailed(message))
                        })?;
                    return Ok(PreparedChatModel {
                        config: model,
                        auto_route: Some(auto_route),
                        auto_router: Some(router),
                    });
                }
                return Ok(PreparedChatModel::fixed(config));
            }
            Ok(None) => {}
            Err(error) => {
                return Err(ChatSendError::Agent(AgentInvocationError::RuntimeFailed(
                    error.to_string(),
                )));
            }
        }
        let config = state
            .models
            .text_agent_model_config_with_reasoning_effort(
                &session.workspace_code,
                fixed_model_code.or(request.model_code.as_deref()),
                request.reasoning_effort,
            )
            .map_err(|error| match error {
                crate::domain::model::ModelError::ReasoningEffortUnsupported => {
                    ChatSendError::Agent(AgentInvocationError::RuntimeFailed(error.to_string()))
                }
                _ => ChatSendError::Chat(ChatError::StorageFailure),
            })?
            .ok_or(ChatSendError::Agent(AgentInvocationError::ModelRequired))?;
        return Ok(PreparedChatModel::fixed(config));
    }

    let model_name = runtime
        .system_model_id
        .as_deref()
        .or(request.system_model_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or(ChatSendError::Agent(AgentInvocationError::ModelRequired))?;

    let system_agent = state
        .agent_management
        .list_workspace_system_agents(&session.workspace_code)
        .map_err(|_| {
            ChatSendError::Agent(AgentInvocationError::RuntimeFailed(
                "workspace system agent policy load failed".into(),
            ))
        })?
        .into_iter()
        .find(|record| record.agent_code.eq_ignore_ascii_case(&runtime.agent_code))
        .ok_or_else(|| {
            ChatSendError::Agent(AgentInvocationError::RuntimeFailed(format!(
                "agent `{}` was not found",
                runtime.agent_code
            )))
        })?;

    let matched_model = validate_system_agent_model(&system_agent, &model_name)?;
    Ok(PreparedChatModel::fixed(build_system_runtime_model_config(
        &model_name,
        matched_model.and_then(|item| item.context_window_tokens),
    )))
}

pub(crate) struct PreparedChatModel {
    pub config: AgentModelConfig,
    pub auto_route: Option<crate::domain::agent::AutoModelRouteContext>,
    pub auto_router: Option<crate::application::model_router::AgentModelRouter>,
}

impl PreparedChatModel {
    fn fixed(config: AgentModelConfig) -> Self {
        Self {
            config,
            auto_route: None,
            auto_router: None,
        }
    }
}

fn validate_chat_model_selection(selection: &ModelSelection) -> Result<(), ChatSendError> {
    let invalid = |message: &str| {
        ChatSendError::Agent(AgentInvocationError::RuntimeFailed(message.to_owned()))
    };
    match selection.kind {
        ModelSelectionKind::Auto => {
            if selection.provider_code.is_some()
                || selection.model_code.is_some()
                || selection.quality_type.is_some()
                || selection.route_code.is_some()
            {
                return Err(invalid(
                    "Auto modelSelection cannot include a model or provider",
                ));
            }
            Ok(())
        }
        ModelSelectionKind::Fixed => {
            if selection.route_code.is_some() {
                return Err(invalid("Fixed modelSelection cannot include routeCode"));
            }
            match selection.provider_code {
                Some(ModelProviderCode::PersonAi) if selection.quality_type.is_none() => {
                    return Err(invalid(
                        "Person-AI Fixed modelSelection requires qualityType",
                    ));
                }
                None if selection.quality_type.is_some() => {
                    return Err(invalid(
                        "platform Fixed modelSelection cannot include qualityType",
                    ));
                }
                _ => {}
            }
            if selection
                .model_code
                .as_deref()
                .is_none_or(|model_code| model_code.trim().is_empty())
            {
                return Err(invalid("Fixed modelSelection requires modelCode"));
            }
            Ok(())
        }
        ModelSelectionKind::ProviderRoute => {
            if selection.provider_code != Some(ModelProviderCode::PersonAi)
                || selection.route_code != Some(ProviderRouteCode::ChatAuto)
                || selection.model_code.is_some()
                || selection.quality_type.is_some()
            {
                return Err(invalid(
                    "chat ProviderRoute modelSelection must be PersonAi ChatAuto without modelCode",
                ));
            }
            Ok(())
        }
    }
}

fn validate_system_agent_model(
    system_agent: &ManagedSystemAgentRecord,
    model_name: &str,
) -> Result<Option<crate::domain::agent::SystemAgentModelRecord>, ChatSendError> {
    if !system_agent.enabled || !system_agent.available {
        return Err(ChatSendError::Agent(AgentInvocationError::RuntimeFailed(
            format!(
                "agent `{}` is unavailable or disabled for this workspace",
                system_agent.agent_code
            ),
        )));
    }

    if system_agent.models.is_empty() {
        return Ok(None);
    }

    let model_matched = system_agent
        .models
        .iter()
        .find(|item| item.model_id.eq_ignore_ascii_case(model_name));
    if let Some(model) = model_matched {
        return Ok(Some(model.clone()));
    }

    Err(ChatSendError::Agent(AgentInvocationError::RuntimeFailed(
        format!(
            "model `{model_name}` is not available for agent `{}`",
            system_agent.agent_code
        ),
    )))
}

// This decorates the independently completed product response when replay failed;
// normal streaming preserves the SDK's native terminal verbatim.
fn native_interrupt_outcome(
    native: Option<&crate::domain::agent::AgentNativeRunOutcome>,
) -> Option<Value> {
    use crate::domain::agent::{AgentNativeInteractionKind, AgentNativeRunStatus};
    let native = native.filter(|native| native.status == AgentNativeRunStatus::AwaitingInput)?;
    let interrupts=native.interactions.iter().map(|interaction| {
        let message=serde_json::from_value::<eliteforge_agent_core::Content>(interaction.prompt.clone()).ok().map(|content|content.text());
        let (reason,tool_call_id)=match &interaction.kind {
            AgentNativeInteractionKind::Input=>("input_required",None),
            AgentNativeInteractionKind::Approval{invocation_id}=>("tool_call",Some(invocation_id)),
        };
        let mut value=json!({"id":interaction.id,"reason":reason,"responseSchema":interaction.response_schema,"metadata":{"nativeInteraction":interaction}});
        if let Some(message)=message {value["message"]=json!(message);}
        if let Some(id)=tool_call_id {value["toolCallId"]=json!(id);}
        value
    }).collect::<Vec<_>>();
    Some(json!({"type":"interrupt","interrupts":interrupts}))
}
