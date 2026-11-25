use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{domain::task::TaskError, response::ApiResponse, shared::auth_holder::AuthHolder};

pub fn task_error_response(error: TaskError) -> Response {
    let locale = request_locale();
    let (status, code) = match &error {
        TaskError::NotFound => (StatusCode::NOT_FOUND, "TASK_NOT_FOUND"),
        TaskError::WorkspaceRequired => (StatusCode::BAD_REQUEST, "TASK_WORKSPACE_REQUIRED"),
        TaskError::InvalidWorkspace => (StatusCode::BAD_REQUEST, "TASK_WORKSPACE_INVALID"),
        TaskError::InvalidParent => (StatusCode::BAD_REQUEST, "TASK_PARENT_INVALID"),
        TaskError::AssigneeNotInWorkspace => (StatusCode::BAD_REQUEST, "TASK_ASSIGNEE_INVALID"),
        TaskError::InvalidApp => (StatusCode::BAD_REQUEST, "TASK_APP_INVALID"),
        TaskError::ActiveSubtaskExists => (StatusCode::BAD_REQUEST, "TASK_ACTIVE_SUBTASK_EXISTS"),
        TaskError::SubtaskExists => (StatusCode::BAD_REQUEST, "TASK_SUBTASK_EXISTS"),
        TaskError::ArchivedMutationDenied => (StatusCode::BAD_REQUEST, "TASK_ARCHIVED_INVALID"),
        TaskError::StatusNotInFlow => (StatusCode::BAD_REQUEST, "TASK_STATUS_FLOW_INVALID"),
        TaskError::FlowNotFound => (StatusCode::NOT_FOUND, "TASK_FLOW_NOT_FOUND"),
        TaskError::FlowCodeConflict => (StatusCode::CONFLICT, "TASK_FLOW_CODE_CONFLICT"),
        TaskError::FlowInUse => (StatusCode::CONFLICT, "TASK_FLOW_IN_USE"),
        TaskError::WorkspaceFlowHasOpenTasks => (
            StatusCode::CONFLICT,
            "TASK_FLOW_SWITCH_REQUIRES_CLOSED_TASKS",
        ),
        TaskError::InvalidProgress | TaskError::InvalidRequest => {
            (StatusCode::BAD_REQUEST, "TASK_REQUEST_INVALID")
        }
        TaskError::SessionRunActive => (StatusCode::CONFLICT, "CHAT_SESSION_RUN_ACTIVE"),
        TaskError::StorageFailure => (StatusCode::INTERNAL_SERVER_ERROR, "TASK_STORAGE_FAILED"),
    };
    (
        status,
        Json(ApiResponse::error(
            code,
            tron_i18n::t!(task_message_key(&error), locale = &locale),
        )),
    )
        .into_response()
}

fn task_message_key(error: &TaskError) -> &'static str {
    match error {
        TaskError::NotFound => "tron.task.not_found",
        TaskError::WorkspaceRequired => "tron.task.workspace_required",
        TaskError::InvalidWorkspace => "tron.task.workspace_invalid",
        TaskError::InvalidParent => "tron.task.parent_invalid",
        TaskError::AssigneeNotInWorkspace => "tron.task.assignee_invalid",
        TaskError::InvalidApp => "tron.task.app_invalid",
        TaskError::ActiveSubtaskExists => "tron.task.active_subtask_exists",
        TaskError::SubtaskExists => "tron.task.subtask_exists",
        TaskError::ArchivedMutationDenied => "tron.task.archived_invalid",
        TaskError::StatusNotInFlow => "tron.task.status_flow_invalid",
        TaskError::FlowNotFound => "tron.task.flow_not_found",
        TaskError::FlowCodeConflict => "tron.task.flow_code_conflict",
        TaskError::FlowInUse => "tron.task.flow_in_use",
        TaskError::WorkspaceFlowHasOpenTasks => "tron.task.flow_switch_requires_closed_tasks",
        TaskError::InvalidProgress | TaskError::InvalidRequest => "tron.task.request_invalid",
        TaskError::SessionRunActive => "error.chat.session_run_active",
        TaskError::StorageFailure => "tron.task.storage_failed",
    }
}

pub fn task_prompt_render_error_response() -> Response {
    let locale = request_locale();
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse::error(
            "TASK_PROMPT_RENDER_FAILED",
            tron_i18n::t!("tron.task.prompt_render_failed", locale = &locale),
        )),
    )
        .into_response()
}

fn request_locale() -> String {
    AuthHolder::get()
        .map(|context| context.locale.clone())
        .unwrap_or_else(|| tron_i18n::DEFAULT_LOCALE.as_str().to_string())
}
