use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{
    application::agent::{AgentAguiStreamError, AgentInvocationError},
    domain::agent::{AgentManagementError, AgentRunHistoryError},
    response::ApiResponse,
    shared::locale::SupportedLocale,
};

/// Locale already resolved for the current request by the profile middleware.
fn request_locale() -> String {
    crate::shared::auth_holder::AuthHolder::get()
        .map(|context| context.locale.clone())
        .unwrap_or_else(|| tron_i18n::DEFAULT_LOCALE.as_str().to_string())
}

pub fn agent_error_response(error: AgentInvocationError) -> Response {
    let locale = request_locale();
    // Runtime diagnostics are logged by the shared mapper; the response uses this locale.
    let (status, code, dynamic_message) = match error {
        AgentInvocationError::PromptRequired => {
            (StatusCode::BAD_REQUEST, "AGENT_PROMPT_REQUIRED", None)
        }
        AgentInvocationError::WorkspaceRequired => {
            (StatusCode::BAD_REQUEST, "AGENT_WORKSPACE_REQUIRED", None)
        }
        AgentInvocationError::WorkspaceInvalid => {
            (StatusCode::BAD_REQUEST, "AGENT_WORKSPACE_INVALID", None)
        }
        AgentInvocationError::ModelRequired => {
            (StatusCode::BAD_REQUEST, "AGENT_MODEL_REQUIRED", None)
        }
        AgentInvocationError::HistoryStorageFailed => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "AGENT_HISTORY_STORAGE_FAILED",
            None,
        ),
        AgentInvocationError::SessionBusy(_) => (StatusCode::CONFLICT, "RUN_ALREADY_ACTIVE", None),
        AgentInvocationError::RunStopped => (StatusCode::CONFLICT, "AGENT_RUN_STOPPED", None),
        AgentInvocationError::RuntimeFailed(message)
        | AgentInvocationError::RuntimeNotStarted(message) => (
            StatusCode::BAD_REQUEST,
            "AGENT_RUNTIME_FAILED",
            Some(message),
        ),
        error @ AgentInvocationError::RuntimeVariableResolutionFailed(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            error.code(),
            Some(error.message()),
        ),
        AgentInvocationError::RuntimeSecretDeliveryUntrusted => (
            StatusCode::BAD_REQUEST,
            "VARIABLE_RUNTIME_SECRET_DELIVERY_UNTRUSTED",
            None,
        ),
        AgentInvocationError::ModelFailed(message) => {
            (StatusCode::BAD_GATEWAY, "AGENT_MODEL_FAILED", Some(message))
        }
        AgentInvocationError::ToolFailed(message) => {
            (StatusCode::BAD_GATEWAY, "AGENT_TOOL_FAILED", Some(message))
        }
        AgentInvocationError::EmptyFinalOutput(message) => (
            StatusCode::BAD_GATEWAY,
            "AGENT_EMPTY_FINAL_OUTPUT",
            Some(message),
        ),
        AgentInvocationError::MaxStepsExceeded => {
            (StatusCode::BAD_REQUEST, "AGENT_MAX_STEPS_EXCEEDED", None)
        }
        AgentInvocationError::MaxToolCallsExceeded => (
            StatusCode::BAD_REQUEST,
            "AGENT_MAX_TOOL_CALLS_EXCEEDED",
            None,
        ),
    };
    let message = super::super::chats::error_messages::message(
        code,
        dynamic_message.as_deref().unwrap_or_default(),
        SupportedLocale::from_raw(&locale),
    );
    (status, Json(ApiResponse::error(code, message))).into_response()
}

pub fn agent_history_error_response(error: AgentRunHistoryError) -> Response {
    let locale = request_locale();
    let (status, code, message_key) = match error {
        AgentRunHistoryError::RunNotFound => (
            StatusCode::NOT_FOUND,
            "AGENT_RUN_NOT_FOUND",
            "agents.agent.run_not_found",
        ),
        AgentRunHistoryError::StorageFailure => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "AGENT_HISTORY_STORAGE_FAILED",
            "agents.agent.history_storage_failed",
        ),
    };
    (
        status,
        Json(ApiResponse::error(
            code,
            tron_i18n::t!(message_key, locale = &locale),
        )),
    )
        .into_response()
}

pub fn agent_agui_error_response(error: AgentAguiStreamError, locale: &str) -> Response {
    let (status, code, message_key) = match error {
        AgentAguiStreamError::StreamKeyRequired => (
            StatusCode::BAD_REQUEST,
            "AGENT_AGUI_STREAM_KEY_REQUIRED",
            "agents.agui.stream_key_required",
        ),
        AgentAguiStreamError::ReplayReadFailed(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "AGENT_AGUI_REPLAY_READ_FAILED",
            "agents.agui.replay_read_failed",
        ),
        AgentAguiStreamError::ReplayWriteFailed(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "AGENT_AGUI_REPLAY_WRITE_FAILED",
            "agents.agui.replay_write_failed",
        ),
        AgentAguiStreamError::ReplayIncomplete(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "AGENT_AGUI_REPLAY_INCOMPLETE",
            "agents.agui.replay_incomplete",
        ),
    };
    (
        status,
        Json(ApiResponse::error(
            code,
            tron_i18n::t!(message_key, locale = locale),
        )),
    )
        .into_response()
}

pub fn agent_management_error_response(error: AgentManagementError) -> Response {
    let locale = request_locale();
    let (status, code, message_key) = match &error {
        AgentManagementError::BuiltinAgentReadOnly => (
            StatusCode::FORBIDDEN,
            "101028017",
            "tron.agent.builtin_read_only",
        ),
        AgentManagementError::PlatformAgentReadOnly => (
            StatusCode::FORBIDDEN,
            "101028018",
            "tron.agent.platform_read_only",
        ),
        AgentManagementError::InternalAgentGroupInvalid => (
            StatusCode::BAD_REQUEST,
            "101028016",
            "tron.agent.group_invalid",
        ),
        AgentManagementError::ToolPermissionInvalid => (
            StatusCode::BAD_REQUEST,
            "101028015",
            "tron.agent.tool_permission_invalid",
        ),
        AgentManagementError::InternalAgentDefinitionInvalid => (
            StatusCode::BAD_REQUEST,
            "101028001",
            "tron.agent.definition_invalid",
        ),
        AgentManagementError::InternalAgentPolicyInvalid => (
            StatusCode::BAD_REQUEST,
            "101028001",
            "tron.agent.policy_invalid",
        ),
        AgentManagementError::InternalAgentCodeInvalid => (
            StatusCode::BAD_REQUEST,
            "101028009",
            "tron.agent.code_invalid",
        ),
        AgentManagementError::InternalAgentCodeConflict => (
            StatusCode::CONFLICT,
            "101028010",
            "tron.agent.code_conflict",
        ),
        AgentManagementError::InvalidWorkspace => (
            StatusCode::BAD_REQUEST,
            "AGENT_WORKSPACE_INVALID",
            "tron.agent.workspace_invalid",
        ),
        AgentManagementError::InternalAgentNotFound => (
            StatusCode::NOT_FOUND,
            "AGENT_INTERNAL_NOT_FOUND",
            "tron.agent.internal_not_found",
        ),
        AgentManagementError::SystemAgentNotFound => (
            StatusCode::NOT_FOUND,
            "AGENT_SYSTEM_NOT_FOUND",
            "agents.agent.management_system_agent_not_found",
        ),
        AgentManagementError::SystemAgentUnavailable => (
            StatusCode::BAD_REQUEST,
            "AGENT_SYSTEM_UNAVAILABLE",
            "agents.agent.management_system_agent_unavailable",
        ),
        AgentManagementError::InternalToolAssignmentStale(_) => (
            StatusCode::BAD_REQUEST,
            "AGENT_INTERNAL_TOOL_ASSIGNMENT_STALE",
            "tron.agent.tool_unavailable",
        ),
        AgentManagementError::InternalMcpServerAssignmentStale(_) => (
            StatusCode::BAD_REQUEST,
            "AGENT_INTERNAL_MCP_SERVER_ASSIGNMENT_STALE",
            "tron.agent.mcp_unavailable",
        ),
        AgentManagementError::InternalToolUnknown(_) => (
            StatusCode::BAD_REQUEST,
            "AGENT_INTERNAL_TOOL_UNKNOWN",
            "tron.agent.tool_unknown",
        ),
        AgentManagementError::StorageFailure => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "AGENT_MANAGEMENT_STORAGE_FAILED",
            "agents.agent.management_storage_failed",
        ),
    };
    let message = match &error {
        AgentManagementError::InternalToolUnknown(code)
        | AgentManagementError::InternalToolAssignmentStale(code)
        | AgentManagementError::InternalMcpServerAssignmentStale(code) => {
            format!("{}: {code}", tron_i18n::t!(message_key, locale = &locale))
        }
        _ => tron_i18n::t!(message_key, locale = &locale).into_owned(),
    };
    (status, Json(ApiResponse::error(code, message))).into_response()
}
