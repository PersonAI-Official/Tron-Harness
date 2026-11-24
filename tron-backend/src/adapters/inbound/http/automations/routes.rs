use axum::{
    Router,
    routing::{delete, get, post},
};

use crate::app::AppState;

use super::handler::{
    archive_automation, create_automation, delete_automation, delete_execution_record,
    execute_automation, get_automation, list_automations, list_execution_records, options,
    patch_automation, patch_status, preview_schedule, restore_automation,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/automations/options", get(options))
        .route(
            "/api/v1/automations/schedule-preview",
            get(preview_schedule),
        )
        .route(
            "/api/v1/automations",
            get(list_automations).post(create_automation),
        )
        .route(
            "/api/v1/automations/{automation_code}",
            get(get_automation)
                .patch(patch_automation)
                .delete(delete_automation),
        )
        .route(
            "/api/v1/automations/{automation_code}/status",
            post(patch_status).patch(patch_status),
        )
        .route(
            "/api/v1/automations/{automation_code}/archive",
            post(archive_automation),
        )
        .route(
            "/api/v1/automations/{automation_code}/restore",
            post(restore_automation),
        )
        .route(
            "/api/v1/automations/{automation_code}/execute",
            post(execute_automation),
        )
        .route(
            "/api/v1/automations/{automation_code}/executions",
            get(list_execution_records),
        )
        .route(
            "/api/v1/automations/{automation_code}/executions/{execution_code}",
            delete(delete_execution_record),
        )
}
