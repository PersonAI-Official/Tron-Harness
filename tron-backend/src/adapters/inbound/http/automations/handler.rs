use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{
    application::automation::AutomationService,
    domain::automation::{AutomationExecutionListQuery, AutomationListQuery},
    response::ApiResponse,
};

use super::{
    error_mapper::automation_error_response,
    request::{CreateAutomationRequest, UpdateAutomationRequest, UpdateAutomationStatusRequest},
};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulePreviewQuery {
    cron_expression: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SchedulePreviewView {
    execution_times: Vec<i64>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteAutomationView {
    automation_code: String,
}

pub async fn preview_schedule(Query(query): Query<SchedulePreviewQuery>) -> Response {
    match crate::application::automation::preview_schedule(&query.cron_expression) {
        Ok(execution_times) => Json(ApiResponse::success(SchedulePreviewView {
            execution_times,
        }))
        .into_response(),
        Err(error) => automation_error_response(error),
    }
}

pub async fn options(State(automations): State<AutomationService>) -> Response {
    Json(ApiResponse::success(automations.options())).into_response()
}

pub async fn list_automations(
    State(automations): State<AutomationService>,
    Query(query): Query<AutomationListQuery>,
) -> Response {
    match automations.list_automations(query) {
        Ok(page) => Json(ApiResponse::success(page)).into_response(),
        Err(error) => automation_error_response(error),
    }
}

pub async fn create_automation(
    State(automations): State<AutomationService>,
    Json(request): Json<CreateAutomationRequest>,
) -> Response {
    match automations.create_automation(request.into()) {
        Ok(detail) => (StatusCode::CREATED, Json(ApiResponse::success(detail))).into_response(),
        Err(error) => automation_error_response(error),
    }
}

pub async fn get_automation(
    State(automations): State<AutomationService>,
    Path(automation_code): Path<String>,
) -> Response {
    match automations.get_automation(&automation_code) {
        Ok(detail) => Json(ApiResponse::success(detail)).into_response(),
        Err(error) => automation_error_response(error),
    }
}

pub async fn patch_automation(
    State(automations): State<AutomationService>,
    Path(automation_code): Path<String>,
    Json(request): Json<UpdateAutomationRequest>,
) -> Response {
    match automations.update_automation(&automation_code, request.into()) {
        Ok(detail) => Json(ApiResponse::success(detail)).into_response(),
        Err(error) => automation_error_response(error),
    }
}

pub async fn delete_automation(
    State(chats): State<crate::application::chat::ChatService>,
    State(automations): State<AutomationService>,
    Path(automation_code): Path<String>,
) -> Response {
    match automations.delete_automation(&automation_code, &chats) {
        Ok(()) => Json(ApiResponse::success(DeleteAutomationView {
            automation_code,
        }))
        .into_response(),
        Err(error) => automation_error_response(error),
    }
}

pub async fn patch_status(
    State(automations): State<AutomationService>,
    Path(automation_code): Path<String>,
    Json(request): Json<UpdateAutomationStatusRequest>,
) -> Response {
    match automations.update_status(&automation_code, request.into()) {
        Ok(detail) => Json(ApiResponse::success(detail)).into_response(),
        Err(error) => automation_error_response(error),
    }
}

pub async fn archive_automation(
    State(automations): State<AutomationService>,
    Path(automation_code): Path<String>,
) -> Response {
    match automations.archive_automation(&automation_code) {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(error) => automation_error_response(error),
    }
}

pub async fn restore_automation(
    State(automations): State<AutomationService>,
    Path(automation_code): Path<String>,
) -> Response {
    match automations.restore_automation(&automation_code) {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(error) => automation_error_response(error),
    }
}

pub async fn execute_automation(
    State(automations): State<AutomationService>,
    Path(automation_code): Path<String>,
) -> Response {
    match automations.execute_now(&automation_code) {
        Ok(detail) => Json(ApiResponse::success(detail)).into_response(),
        Err(error) => automation_error_response(error),
    }
}

pub async fn list_execution_records(
    State(automations): State<AutomationService>,
    Path(automation_code): Path<String>,
    Query(query): Query<AutomationExecutionListQuery>,
) -> Response {
    match automations.list_execution_records(&automation_code, query) {
        Ok(page) => Json(ApiResponse::success(page)).into_response(),
        Err(error) => automation_error_response(error),
    }
}

pub async fn delete_execution_record(
    State(automations): State<AutomationService>,
    Path((automation_code, execution_code)): Path<(String, String)>,
) -> Response {
    match automations.delete_execution_record(&automation_code, &execution_code) {
        Ok(record) => Json(ApiResponse::success(record)).into_response(),
        Err(error) => automation_error_response(error),
    }
}
