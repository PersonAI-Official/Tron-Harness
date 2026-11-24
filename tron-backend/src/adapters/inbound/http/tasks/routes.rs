use axum::{
    Router,
    routing::{get, post},
};

use crate::app::AppState;

use super::handler::{
    archive_task, create_task, create_task_flow, delete_task, delete_task_flow, execute_task,
    get_task, get_task_flow, get_workspace_task_flow, kanban, list_task_flows, list_tasks, options,
    list_task_executions, patch_status, patch_task, restore_task, switch_workspace_task_flow,
    update_task_flow,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/tasks/options", get(options))
        .route(
            "/api/v1/admin/task-flows",
            get(list_task_flows).post(create_task_flow),
        )
        .route(
            "/api/v1/admin/task-flows/{flow_code}",
            get(get_task_flow)
                .patch(update_task_flow)
                .delete(delete_task_flow),
        )
        .route(
            "/api/v1/task-flows/workspaces/{workspace_code}",
            get(get_workspace_task_flow).put(switch_workspace_task_flow),
        )
        .route("/api/v1/tasks/kanban", get(kanban))
        .route("/api/v1/tasks", get(list_tasks).post(create_task))
        .route(
            "/api/v1/tasks/{task_code}",
            get(get_task).patch(patch_task).delete(delete_task),
        )
        .route("/api/v1/tasks/{task_code}/execute", post(execute_task))
        .route(
            "/api/v1/tasks/{task_code}/executions",
            get(list_task_executions),
        )
        .route(
            "/api/v1/tasks/{task_code}/status",
            post(patch_status).patch(patch_status),
        )
        .route("/api/v1/tasks/{task_code}/archive", post(archive_task))
        .route("/api/v1/tasks/{task_code}/restore", post(restore_task))
}
