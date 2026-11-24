use crate::shared::{auth_holder::AuthHolder, locale::SupportedLocale};
use tron_i18n::t;

// HTTP and SSE share stable error codes; diagnostic details stay in server logs
// and never enter the user-facing message. Display text lives in locales/chat.yml
// under `error.chat.*`.
pub(super) fn message(code: &str, detail: &str, locale: SupportedLocale) -> String {
    if let Some(message) = context_budget_message(detail, locale) {
        return message;
    }
    let translated = match code {
        "CHAT_ATTACHMENT_TOO_LARGE" => t!("error.chat.attachment_too_large", locale = locale),
        "CHAT_ATTACHMENT_INVALID" => t!("error.chat.attachment_invalid", locale = locale),
        "CHAT_ATTACHMENT_NOT_FOUND" => t!("error.chat.attachment_not_found", locale = locale),
        "CHAT_MESSAGE_REQUIRED" | "AGENT_PROMPT_REQUIRED" => {
            t!("error.chat.message_required", locale = locale)
        }
        "CHAT_SESSION_NOT_FOUND" | "CHAT_RUN_CONTROL_SESSION_NOT_FOUND" => {
            t!("error.chat.session_not_found", locale = locale)
        }
        "CHAT_SESSION_MUST_BE_ARCHIVED" => {
            t!("error.chat.session_must_be_archived", locale = locale)
        }
        "CHAT_SESSION_ARCHIVED" => t!("error.chat.session_archived", locale = locale),
        "CHAT_SESSION_RUN_ACTIVE" => t!("error.chat.session_run_active", locale = locale),
        "CHAT_CONTAINER_DELETE_REJECTED" => {
            t!("error.chat.container_delete_rejected", locale = locale)
        }
        "CHAT_SESSION_LOCATION_CHANGED" => {
            t!("error.chat.session_location_changed", locale = locale)
        }
        "CHAT_SESSION_TITLE_REQUIRED" => {
            t!("error.chat.session_title_required", locale = locale)
        }
        "CHAT_WORKSPACE_REQUIRED" | "AGENT_WORKSPACE_REQUIRED" => {
            t!("error.chat.workspace_required", locale = locale)
        }
        "CHAT_WORKSPACE_FORBIDDEN" => t!("error.chat.workspace_forbidden", locale = locale),
        "AGENT_WORKSPACE_INVALID" => t!("error.chat.workspace_invalid", locale = locale),
        "CHAT_PROJECT_REQUIRED" => t!("error.chat.project_required", locale = locale),
        "101045005" => t!("error.chat.project_name_conflict", locale = locale),
        "CHAT_PROJECT_INVALID" => t!("error.chat.project_invalid", locale = locale),
        "CHAT_DIRECTORY_FIXED" => t!("error.chat.directory_fixed", locale = locale),
        "CHAT_DIRECTORY_INVALID" => t!("error.chat.directory_invalid", locale = locale),
        "CHAT_USER_INPUT_ALREADY_ANSWERED" => {
            t!("error.chat.user_input_already_answered", locale = locale)
        }
        "CHAT_USER_INPUT_ANSWER_INVALID" => {
            t!("error.chat.user_input_answer_invalid", locale = locale)
        }
        "CHAT_USER_INPUT_PENDING" => t!("error.chat.user_input_pending", locale = locale),
        "CHAT_USER_INPUT_REQUEST_NOT_FOUND" => {
            t!("error.chat.user_input_request_not_found", locale = locale)
        }
        "CHAT_STORAGE_FAILED" | "AGENT_HISTORY_STORAGE_FAILED" => {
            t!("error.chat.storage_failed", locale = locale)
        }
        "CHAT_RUN_ALREADY_ACTIVE" | "RUN_ALREADY_ACTIVE" => {
            t!("error.chat.run_already_active", locale = locale)
        }
        "CHAT_SESSION_CODE_REQUIRED" | "CHAT_RUN_CONTROL_SESSION_CODE_REQUIRED" => {
            t!("error.chat.session_code_required", locale = locale)
        }
        "CHAT_EXPECTED_RUN_CODE_REQUIRED" | "CHAT_RUN_CONTROL_EXPECTED_RUN_CODE_REQUIRED" => {
            t!("error.chat.expected_run_code_required", locale = locale)
        }
        "CHAT_STEER_NO_ACTIVE_RUN" | "CHAT_RUN_CONTROL_NO_ACTIVE_RUN" => {
            t!("error.chat.no_active_run", locale = locale)
        }
        "CHAT_STEER_RUN_MISMATCH" | "CHAT_RUN_CONTROL_RUN_MISMATCH" => {
            t!("error.chat.run_mismatch", locale = locale)
        }
        "CHAT_STEER_RUNTIME_UNSUPPORTED" => {
            t!("error.chat.runtime_unsupported", locale = locale)
        }
        "CHAT_STEER_RUN_STARTING" => t!("error.chat.run_starting", locale = locale),
        "CHAT_STEER_RUN_FINISHING" | "CHAT_RUN_CONTROL_RUN_FINISHING" => {
            t!("error.chat.run_finishing", locale = locale)
        }
        "CHAT_STEER_REGISTRY_UNAVAILABLE" | "CHAT_RUN_CONTROL_REGISTRY_UNAVAILABLE" => {
            t!("error.chat.registry_unavailable", locale = locale)
        }
        "CHAT_STOP_RUN_REQUEST_INVALID"
        | "CHAT_ACTIVE_RUN_QUERY_INVALID"
        | "CHAT_STEER_REQUEST_INVALID" => t!("error.chat.request_invalid", locale = locale),
        "AGENT_MODEL_REQUIRED" => t!("error.chat.model_required", locale = locale),
        "AGENT_RUN_STOPPED" | "CANCELLED" => t!("error.chat.run_stopped", locale = locale),
        "AGENT_RUNTIME_FAILED" => t!("error.chat.runtime_failed", locale = locale),
        "VARIABLE_RUNTIME_SECRET_DELIVERY_UNTRUSTED" => {
            t!("error.chat.secret_delivery_untrusted", locale = locale)
        }
        "VARIABLE_RUNTIME_DECRYPT_FAILED" => {
            t!("error.chat.variable_decrypt_failed", locale = locale)
        }
        "VARIABLE_RUNTIME_ENCRYPTION_NOT_CONFIGURED" => {
            t!("error.chat.encryption_not_configured", locale = locale)
        }
        "VARIABLE_RUNTIME_STORAGE_FAILED" | "VARIABLE_RUNTIME_RESOLUTION_FAILED" => {
            t!("error.chat.variable_load_failed", locale = locale)
        }
        "AGENT_MODEL_FAILED" => t!("error.chat.model_failed", locale = locale),
        "AGENT_TOOL_FAILED" => t!("error.chat.tool_failed", locale = locale),
        "AGENT_EMPTY_FINAL_OUTPUT" => t!("error.chat.empty_final_output", locale = locale),
        "AGENT_MAX_STEPS_EXCEEDED" | "STEP_LIMIT" => {
            t!("error.chat.max_steps_exceeded", locale = locale)
        }
        "AGENT_MAX_TOOL_CALLS_EXCEEDED" | "TOOL_CALL_LIMIT" => {
            t!("error.chat.max_tool_calls_exceeded", locale = locale)
        }
        "CHAT_STREAM_COMPLETION_UNAVAILABLE" | "CHAT_STREAM_COMPLETION_INVALID" | "101031004" => {
            t!("error.chat.stream_completion_unavailable", locale = locale)
        }
        "CHAT_STREAM_SERIALIZATION_FAILED" => {
            t!("error.chat.stream_serialization_failed", locale = locale)
        }
        "CHAT_CONTEXT_DETAIL_NOT_FOUND" => {
            t!("error.chat.context_detail_not_found", locale = locale)
        }
        "COMPACTION_MODEL_NOT_CONFIGURED" => {
            t!(
                "error.chat.compaction_model_not_configured",
                locale = locale
            )
        }
        "SHORT_TERM_MEMORY_REQUEST_INVALID" => {
            t!(
                "error.chat.short_term_memory_request_invalid",
                locale = locale
            )
        }
        "SHORT_TERM_MEMORY_COMPACTION_CONFLICT" => {
            t!(
                "error.chat.short_term_memory_compaction_conflict",
                locale = locale
            )
        }
        "SHORT_TERM_MEMORY_RUN_CONFLICT" => {
            t!("error.chat.short_term_memory_run_conflict", locale = locale)
        }
        "SHORT_TERM_MEMORY_COMPACTION_INPUT_TOO_LARGE" => {
            t!(
                "error.chat.short_term_memory_compaction_input_too_large",
                locale = locale
            )
        }
        "SHORT_TERM_MEMORY_COMPACTION_MODEL_FAILED" => {
            t!(
                "error.chat.short_term_memory_compaction_model_failed",
                locale = locale
            )
        }
        "SHORT_TERM_MEMORY_CONTEXT_BUDGET_EXCEEDED" => {
            t!(
                "error.chat.short_term_memory_context_budget_exceeded",
                locale = locale
            )
        }
        "SHORT_TERM_MEMORY_TODO_INVALID" => {
            t!("error.chat.short_term_memory_todo_invalid", locale = locale)
        }
        "SHORT_TERM_MEMORY_GOAL_NOT_FOUND" => {
            t!(
                "error.chat.short_term_memory_goal_not_found",
                locale = locale
            )
        }
        "SHORT_TERM_MEMORY_GOAL_CONFLICT" => {
            t!(
                "error.chat.short_term_memory_goal_conflict",
                locale = locale
            )
        }
        "SHORT_TERM_MEMORY_GOAL_STATE_CONFLICT" => {
            t!(
                "error.chat.short_term_memory_goal_state_conflict",
                locale = locale
            )
        }
        "SHORT_TERM_MEMORY_GOAL_INVALID" => {
            t!("error.chat.short_term_memory_goal_invalid", locale = locale)
        }
        "SHORT_TERM_MEMORY_STORAGE_FAILED" => {
            t!(
                "error.chat.short_term_memory_storage_failed",
                locale = locale
            )
        }
        "CHAT_SESSION_DELETE_FAILED" => t!("error.chat.session_delete_failed", locale = locale),
        // Artifact codes keep their historical generic fallback (the concrete
        // diagnostic stays in `detail`); unknown codes do the same.
        _ => {
            tracing::warn!(code, detail, "unmapped chat error");
            t!("error.chat.operation_failed", locale = locale)
        }
    };
    if !detail.is_empty() {
        tracing::debug!(code, detail, "chat error diagnostic");
    }
    translated.into_owned()
}

pub(super) fn context_budget_message(detail: &str, locale: SupportedLocale) -> Option<String> {
    if let Some((_, suffix)) = detail.split_once("context budget exceeded: tron_context_block:") {
        let id = suffix.split_whitespace().next().unwrap_or_default();
        let id = match id {
            "framework.instructions" => "seed_prompt_prefix",
            "framework.history" => "current_session_history",
            "current_session_checkpoint" => "conversation_history_summary",
            _ => id,
        };
        let name = crate::application::short_term_memory::host_block_text_keys(id)
            .map(|keys| tron_i18n::translate(locale.as_str(), keys.display_name).into_owned())
            .unwrap_or_else(|| id.to_owned());
        return Some(
            t!(
                "error.chat.context_block_budget_exceeded",
                locale = locale,
                block = name
            ),
        );
    }
    detail
        .contains("context budget exceeded:")
        .then(|| t!("error.chat.context_budget_exceeded", locale = locale).into_owned())
}

/// Locale of the current request scope.
///
/// The HTTP layer wraps every protected request in an [`AuthHolder`] scope, so
/// this stays request-local even when sessions with different interface
/// languages run concurrently. Callers that already hold the request locale
/// pass it to [`message`] explicitly instead.
pub(super) fn request_locale() -> SupportedLocale {
    AuthHolder::get()
        .map(|auth| SupportedLocale::from_raw(&auth.locale))
        .unwrap_or_default()
}
