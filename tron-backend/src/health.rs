use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    app::{AppReadiness, AppReadinessStatus},
    config::AppConfig,
    response::ApiResponse,
};

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthData {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReadinessComponents {
    storage: &'static str,
    graph: &'static str,
    rag: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReadinessData {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    components: ReadinessComponents,
}

#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses((status = 200, description = "Service is healthy", body = ApiResponse<HealthData>)),
    tag = "System"
)]
pub async fn health(State(_config): State<AppConfig>) -> Response {
    let mut response = Json(ApiResponse::success(HealthData {
        status: "ok",
        service: "tron-backend",
        version: env!("CARGO_PKG_VERSION"),
    }))
    .into_response();
    let optimizing =
        crate::adapters::outbound::persistence::sqlite::framework_storage::is_optimizing();
    response.headers_mut().insert(
        "x-tron-storage-optimizing",
        axum::http::HeaderValue::from_static(if optimizing { "1" } else { "0" }),
    );
    response
}

#[utoipa::path(
    get,
    path = "/api/v1/readiness",
    responses(
        (status = 200, description = "All dependencies are ready", body = ApiResponse<ReadinessData>),
        (status = 503, description = "A dependency is unavailable", body = ApiResponse<ReadinessData>)
    ),
    tag = "System"
)]
pub async fn readiness(State(readiness): State<AppReadiness>) -> Response {
    let readiness = readiness_payload(readiness.check());
    if readiness.status == "ready" {
        Json(ApiResponse::success(readiness)).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse {
                code: "DEPENDENCY_NOT_READY",
                message: readiness_failure_message(&readiness.components).into(),
                data: readiness,
            }),
        )
            .into_response()
    }
}

fn readiness_payload(status: AppReadinessStatus) -> ReadinessData {
    ReadinessData {
        status: if status.storage_ready && status.graph_ready && status.rag_ready {
            "ready"
        } else {
            "not_ready"
        },
        service: "tron-backend",
        version: env!("CARGO_PKG_VERSION"),
        components: ReadinessComponents {
            storage: component_status(status.storage_ready),
            graph: component_status(status.graph_ready),
            rag: component_status(status.rag_ready),
        },
    }
}

fn component_status(ready: bool) -> &'static str {
    if ready { "up" } else { "down" }
}

fn readiness_failure_message(components: &ReadinessComponents) -> &'static str {
    match (components.storage, components.graph, components.rag) {
        ("down", "down", _) => "storage and graph dependencies not ready",
        ("down", _, _) => "storage dependency not ready",
        (_, "down", _) => "graph dependency not ready",
        (_, _, "down") => "RAG dependency not ready",
        _ => "dependency not ready",
    }
}
