use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use serde_json::Value;

use crate::{
    adapters::inbound::http::{auth::RequestAuthContext, chats::handler::start_task_chat_run},
    app::AppState,
    application::{
        agent::generate_agent_run_code,
        chat::{CreateChatSessionCommand, SendChatMessageCommand},
        task::TaskService,
    },
    domain::{
        chat::{ChatClientCapabilities, ChatSessionSourceType},
        model_provider::ModelSelection,
        prompt::PromptRuntimeProvider,
        task::{TaskError, TaskExecutionResult, TaskFlowDraft, TaskListQuery},
    },
    response::ApiResponse,
};

use super::{
    error_mapper::{task_error_response, task_prompt_render_error_response},
    request::{
        CreateTaskRequest, TaskExecutionListQuery, TaskFlowRequest, TaskFlowWorkspaceQuery,
        UpdateTaskRequest, UpdateTaskStatusRequest, WorkspaceTaskFlowRequest,
    },
};

pub async fn options(
    State(tasks): State<TaskService>,
    Query(query): Query<TaskFlowWorkspaceQuery>,
) -> Response {
    match tasks.options(&query.workspace_code) {
        Ok(options) => Json(ApiResponse::success(options)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn list_task_flows(State(tasks): State<TaskService>) -> Response {
    match tasks.list_task_flows() {
        Ok(flows) => Json(ApiResponse::success(flows)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn create_task_flow(
    State(tasks): State<TaskService>,
    Json(request): Json<TaskFlowRequest>,
) -> Response {
    match tasks.create_task_flow(request.into()) {
        Ok(flow) => (StatusCode::CREATED, Json(ApiResponse::success(flow))).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn get_task_flow(
    State(tasks): State<TaskService>,
    Path(flow_code): Path<String>,
) -> Response {
    match tasks.get_task_flow(&flow_code) {
        Ok(flow) => Json(ApiResponse::success(flow)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn update_task_flow(
    State(tasks): State<TaskService>,
    Path(flow_code): Path<String>,
    Json(request): Json<TaskFlowRequest>,
) -> Response {
    let mut draft: TaskFlowDraft = request.into();
    draft.flow_code = flow_code.clone();
    match tasks.update_task_flow(&flow_code, draft) {
        Ok(flow) => Json(ApiResponse::success(flow)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn delete_task_flow(
    State(tasks): State<TaskService>,
    Path(flow_code): Path<String>,
) -> Response {
    match tasks.delete_task_flow(&flow_code) {
        Ok(()) => Json(ApiResponse::success(
            serde_json::json!({ "flowCode": flow_code }),
        ))
        .into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn get_workspace_task_flow(
    State(tasks): State<TaskService>,
    Path(workspace_code): Path<String>,
) -> Response {
    match tasks.workspace_task_flow(&workspace_code) {
        Ok(flow) => Json(ApiResponse::success(flow)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn switch_workspace_task_flow(
    State(tasks): State<TaskService>,
    Path(workspace_code): Path<String>,
    Json(request): Json<WorkspaceTaskFlowRequest>,
) -> Response {
    match tasks.assign_workspace_task_flow(&workspace_code, &request.flow_code) {
        Ok(flow) => Json(ApiResponse::success(flow)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn kanban(
    State(tasks): State<TaskService>,
    Query(query): Query<TaskListQuery>,
) -> Response {
    match tasks.kanban(query) {
        Ok(columns) => Json(ApiResponse::success(columns)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<TaskListQuery>,
) -> Response {
    if let Some(owner) = query.owner_assistant_code.as_deref() {
        if state.assistants.get(owner).is_err() {
            return task_error_response(TaskError::InvalidRequest);
        }
    }
    match state.tasks.list_tasks(query) {
        Ok(page) => Json(ApiResponse::success(page)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn create_task(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> Response {
    if let Some(owner) = request.owner_assistant_code.as_deref() {
        if state.assistants.get(owner).is_err() {
            return task_error_response(TaskError::InvalidRequest);
        }
    }
    match state.tasks.create_task(request.into()) {
        Ok(detail) => (StatusCode::CREATED, Json(ApiResponse::success(detail))).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn get_task(State(tasks): State<TaskService>, Path(task_code): Path<String>) -> Response {
    match tasks.get_task(&task_code) {
        Ok(detail) => Json(ApiResponse::success(detail)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn patch_task(
    State(tasks): State<TaskService>,
    Path(task_code): Path<String>,
    Json(request): Json<UpdateTaskRequest>,
) -> Response {
    match tasks.update_task(&task_code, request.into()) {
        Ok(detail) => Json(ApiResponse::success(detail)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn patch_status(
    State(tasks): State<TaskService>,
    Path(task_code): Path<String>,
    Json(request): Json<UpdateTaskStatusRequest>,
) -> Response {
    match tasks.update_status(&task_code, request.into()) {
        Ok(detail) => Json(ApiResponse::success(detail)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn execute_task(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(task_code): Path<String>,
) -> Response {
    let task = match state.tasks.get_task(&task_code) {
        Ok(task) => task,
        Err(error) => return task_error_response(error),
    };
    // PersonAI-owned work must be dispatched by the PersonAI event runner so its owner,
    // work session and feedback route survive the request lifetime.
    if task.owner_assistant_code.is_some() {
        return task_error_response(TaskError::InvalidRequest);
    }
    if task.archived_at.is_some() {
        return task_error_response(TaskError::ArchivedMutationDenied);
    }
    let execution_status = match task_execution_status(&state.tasks, &task) {
        Ok(status) => status,
        Err(error) => return task_error_response(error),
    };
    let prompt = match state.prompts.runtime_prompt_text(
        "task.execution",
        &auth.locale,
        &task_execution_runtime_values(&task, &execution_status),
    ) {
        Ok(Some(prompt)) => prompt,
        Ok(None) | Err(_) => return task_prompt_render_error_response(),
    };

    let session = match state.chats.create_session(
        CreateChatSessionCommand {
            execution_mode: Default::default(),
            workspace_code: task.workspace_code.clone(),
            project_code: None,
            directory_code: None,
            working_directory_code: None,
            title: Some(tron_i18n::t!(
                "tasks.session_title",
                locale = &auth.locale,
                title = task.title
            )),
            app_code: None,
            task_code: Some(task.task_code.clone()),
            source_type: Some(ChatSessionSourceType::Task),
            source_code: Some(task.task_code.clone()),
            agent_code: Some(task.agent_code.clone()),
        },
        Some(auth.user().user_code.clone()),
    ) {
        Ok(session) => session,
        Err(error) => {
            return crate::adapters::inbound::http::chats::error_mapper::chat_error_response(error);
        }
    };

    let locale = auth.locale.clone();
    let run_code = generate_agent_run_code();
    let session_code = session.session_code.clone();
    let model_selection = task
        .model_code
        .as_deref()
        .is_none_or(|code| code.trim().is_empty() || code == "PERSON_AI:ROUTE:ChatAuto")
        .then(ModelSelection::auto);
    let request = SendChatMessageCommand {
        annotations: Vec::new(),
        requested_skills: Vec::new(),
        a2ui_action_id: None,
        a2ui_metadata: None,
        run_code: Some(run_code.clone()),
        content: Some(prompt),
        workspace_code: task.workspace_code.clone(),
        app_code: None,
        task_code: Some(task.task_code.clone()),
        agent_code: Some(task.agent_code.clone()),
        model_selection,
        model_code: task.model_code.clone(),
        system_model_id: task.model_code.clone(),
        reasoning_effort: None,
        page_context: None,
        limits: None,
        tools: Vec::new(),
        attachments: Vec::new(),
        client_capabilities: Some(ChatClientCapabilities {
            user_input_requests: true,
            ..Default::default()
        }),
        resume: Vec::new(),
    };
    let started = match state.tasks.confirm_task_session_owner(&task.task_code) {
        Ok(()) => start_task_chat_run(&state, auth, session_code.clone(), request).await,
        Err(error) => Err(task_error_response(error)),
    };
    if let Err(response) = started {
        if let Err(cleanup_error) = state.chats.delete_session(&session_code)
            && !matches!(
                cleanup_error,
                crate::domain::chat::ChatError::SessionNotFound
            )
        {
            tracing::warn!(%cleanup_error, %session_code, "Task session cleanup failed");
        }
        return response;
    }

    Json(ApiResponse::success(TaskExecutionResult {
        task_code: task.task_code,
        session_code,
        run_code,
        agent_code: session
            .agent_code
            .unwrap_or_else(|| "main-agent".to_string()),
        model_code: task.model_code,
        summary: tron_i18n::t!("tasks.execution_submitted", locale = &locale).into_owned(),
    }))
    .into_response()
}

pub async fn list_task_executions(
    State(tasks): State<TaskService>,
    Path(task_code): Path<String>,
    Query(query): Query<TaskExecutionListQuery>,
) -> Response {
    match tasks.list_task_executions(
        &task_code,
        query.current.unwrap_or(1),
        query.size.unwrap_or(15),
    ) {
        Ok(page) => Json(ApiResponse::success(page)).into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn delete_task(
    State(chats): State<crate::application::chat::ChatService>,
    State(tasks): State<TaskService>,
    Path(task_code): Path<String>,
) -> Response {
    match tasks.delete_task(&task_code, &chats) {
        Ok(()) => Json(ApiResponse::success(
            serde_json::json!({ "taskCode": task_code }),
        ))
        .into_response(),
        Err(error) => task_error_response(error),
    }
}

pub async fn archive_task(
    State(tasks): State<TaskService>,
    Path(task_code): Path<String>,
) -> Response {
    match tasks.archive_task(&task_code) {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(error) => task_error_response(error),
    }
}

fn task_execution_runtime_values(
    task: &crate::domain::task::TaskDetail,
    execution_status: &str,
) -> HashMap<PromptRuntimeProvider, Value> {
    let description = task
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("No description provided.");
    let apps = if task.app_codes.is_empty() {
        "None".to_string()
    } else {
        task.app_codes.join(", ")
    };
    let source_url = task
        .source_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("None");
    let task_context = format!(
        "Task code: {}\nTitle: {}\nType: {:?}\nStatus: {:?}\nPriority: {:?}\nProgress: {}%\nSource type: {:?}\nSource URL: {}\nApps: {}\n\nDescription:\n{}",
        task.task_code,
        task.title,
        task.task_type,
        execution_status,
        task.priority,
        task.progress,
        task.source_type,
        source_url,
        apps,
        description,
    );
    HashMap::from([(
        PromptRuntimeProvider::TaskContext,
        Value::String(task_context),
    )])
}

fn task_execution_status(
    tasks: &TaskService,
    task: &crate::domain::task::TaskDetail,
) -> Result<String, TaskError> {
    if task.status != crate::domain::task::NOT_STARTED_STATUS_CODE {
        return Ok(task.status.clone());
    }
    let flow = tasks.get_task_flow(&task.flow_code)?;
    Ok(flow
        .statuses
        .iter()
        .find(|status| status.status_code == crate::domain::task::IN_PROGRESS_STATUS_CODE)
        .or_else(|| {
            flow.statuses.iter().find(|status| {
                status.status_code != crate::domain::task::NOT_STARTED_STATUS_CODE
                    && status.status_code != crate::domain::task::CLOSED_STATUS_CODE
            })
        })
        .map(|status| status.status_code.clone())
        .unwrap_or_else(|| task.status.clone()))
}

pub async fn restore_task(
    State(tasks): State<TaskService>,
    Path(task_code): Path<String>,
) -> Response {
    match tasks.restore_task(&task_code) {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(error) => task_error_response(error),
    }
}
