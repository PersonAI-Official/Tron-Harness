use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{
    domain::automation::AutomationError, response::ApiResponse, shared::auth_holder::AuthHolder,
};

pub fn automation_error_response(error: AutomationError) -> Response {
    let locale = request_locale();
    let (status, code, message_key) = match error {
        AutomationError::NotFound => (
            StatusCode::NOT_FOUND,
            "AUTOMATION_NOT_FOUND",
            "automations.not_found",
        ),
        AutomationError::WorkspaceRequired => (
            StatusCode::BAD_REQUEST,
            "AUTOMATION_WORKSPACE_REQUIRED",
            "automations.workspace_required",
        ),
        AutomationError::InvalidWorkspace => (
            StatusCode::BAD_REQUEST,
            "AUTOMATION_WORKSPACE_INVALID",
            "automations.workspace_invalid",
        ),
        AutomationError::NameRequired => (
            StatusCode::BAD_REQUEST,
            "AUTOMATION_NAME_REQUIRED",
            "automations.name_required",
        ),
        AutomationError::PromptRequired => (
            StatusCode::BAD_REQUEST,
            "AUTOMATION_PROMPT_REQUIRED",
            "automations.prompt_required",
        ),
        AutomationError::InvalidCron => (
            StatusCode::BAD_REQUEST,
            "AUTOMATION_CRON_INVALID",
            "automations.cron_invalid",
        ),
        AutomationError::InvalidStatus => (
            StatusCode::BAD_REQUEST,
            "AUTOMATION_STATUS_INVALID",
            "automations.status_invalid",
        ),
        AutomationError::ArchivedMutationDenied => (
            StatusCode::BAD_REQUEST,
            "AUTOMATION_ARCHIVED_INVALID",
            "automations.archived_invalid",
        ),
        AutomationError::SessionRunActive => (
            StatusCode::CONFLICT,
            "CHAT_SESSION_RUN_ACTIVE",
            "error.chat.session_run_active",
        ),
        AutomationError::StorageFailure => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "AUTOMATION_STORAGE_FAILED",
            "automations.storage_failed",
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

fn request_locale() -> String {
    AuthHolder::get()
        .map(|context| context.locale.clone())
        .unwrap_or_else(|| tron_i18n::DEFAULT_LOCALE.as_str().to_string())
}
