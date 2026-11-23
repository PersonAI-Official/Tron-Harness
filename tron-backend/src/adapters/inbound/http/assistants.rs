mod soul;
use crate::adapters::inbound::http::auth::RequestAuthContext;
use crate::{
    app::AppState,
    application::{
        agent::{
            CreateWorkspaceInternalAgentCommand, UpdateWorkspaceInternalAgentDefinitionCommand,
        },
        assistant::*,
    },
    domain::assistant::*,
    response::{ApiResponse, EmptyData},
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post, put},
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(soul::routes())
        .route("/api/v1/assistants", get(list).post(create))
        .route("/api/v1/assistants/main/session", post(main_session))
        .route(
            "/api/v1/assistants/{code}",
            get(detail).patch(update).delete(delete),
        )
        .route("/api/v1/assistants/{code}/main", put(designate_main))
        .route(
            "/api/v1/assistants/{code}/sessions",
            get(sessions).post(resolve_session),
        )
        .route("/api/v1/assistants/{code}/events", get(events))
        .route(
            "/api/v1/assistants/{code}/tasks/{task_code}/cancel",
            post(cancel_task),
        )
        .route("/api/v1/assistants/{code}/threads", get(threads))
        .route(
            "/api/v1/assistants/{code}/threads/{session}/messages",
            post(thread_message),
        )
        .route(
            "/api/v1/assistants/{code}/threads/{session}/answer",
            post(thread_answer),
        )
        .route(
            "/api/v1/assistants/{code}/threads/{session}/history/query",
            post(thread_history),
        )
        .route(
            "/api/v1/notifications",
            get(notifications).delete(delete_archived_notifications),
        )
        .route("/api/v1/notifications/batch", patch(update_notifications))
        .route(
            "/api/v1/notifications/{id}",
            patch(update_notification).delete(delete_notification),
        )
}
#[derive(Debug)]
pub struct AssistantApiError(pub AssistantError);
impl From<AssistantError> for AssistantApiError {
    fn from(e: AssistantError) -> Self {
        Self(e)
    }
}
impl IntoResponse for AssistantApiError {
    fn into_response(self) -> Response {
        assistant_error_response(error_descriptor(&self.0))
    }
}

fn assistant_error_response(descriptor: ApplicationErrorDescriptor) -> Response {
    let locale = crate::shared::auth_holder::AuthHolder::get()
        .map(|context| context.locale.clone())
        .unwrap_or_else(|| tron_i18n::DEFAULT_LOCALE.as_str().to_string());
    let (status, message_key) = match descriptor.message_key {
        "tron.assistant.not_found" => (StatusCode::NOT_FOUND, "tron.assistant.not_found"),
        "tron.assistant.request_invalid" => {
            (StatusCode::BAD_REQUEST, "tron.assistant.request_invalid")
        }
        "tron.assistant.state_changed" => (StatusCode::CONFLICT, "tron.assistant.state_changed"),
        "tron.assistant.task_completed" => (StatusCode::CONFLICT, "tron.assistant.task_completed"),
        "tron.assistant.task_has_active_subtasks" => (
            StatusCode::CONFLICT,
            "tron.assistant.task_has_active_subtasks",
        ),
        "tron.assistant.storage_failed" => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "tron.assistant.storage_failed",
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "tron.assistant.runtime_failed",
        ),
    };
    (
        status,
        Json(ApiResponse::<EmptyData>::protocol_error(
            descriptor.code,
            tron_i18n::t!(message_key, locale = &locale),
        )),
    )
        .into_response()
}
macro_rules! assistant_response {
    ($name:ident) => {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            assistant_code: String,
            agent_code: String,
            name: String,
            description: String,
            role_prompt: String,
            avatar_id: String,
            candidate_model_codes: Vec<String>,
            kind: AssistantKind,
            enabled: bool,
            heartbeat_minutes: u32,
            memory_auto_approve: bool,
            is_main: bool,
            created_at: i64,
            updated_at: i64,
            tool_assignment_mode: crate::domain::agent::ToolAssignmentMode,
            #[serde(default)]
            selected_category_codes: Vec<String>,
            mcp_access_mode: crate::domain::agent::McpAccessMode,
            max_steps: u32,
            max_tool_calls: u32,
            max_parallel_tool_calls: u32,
            selected_tool_codes: Vec<String>,
            selected_server_codes: Vec<String>,
            native_tool_scheme_code: Option<String>,
        }
        impl $name {
            fn from_record(
                service: &AssistantService,
                d: Assistant,
            ) -> Result<Self, AssistantError> {
                let p = service.definition(&d.assistant_code)?;
                Ok(Self {
                    assistant_code: d.assistant_code,
                    agent_code: p.agent_code,
                    name: p.name,
                    description: p.description,
                    role_prompt: p.role_prompt,
                    avatar_id: p.avatar_id,
                    candidate_model_codes: p.candidate_model_codes,
                    kind: d.kind,
                    enabled: d.enabled,
                    heartbeat_minutes: d.heartbeat_minutes,
                    memory_auto_approve: d.memory_auto_approve,
                    is_main: d.is_main,
                    created_at: d.created_at,
                    updated_at: d.updated_at,
                    tool_assignment_mode: p.tool_assignment_mode,
                    selected_category_codes: p.selected_category_codes.clone(),
                    mcp_access_mode: p.mcp_access_mode,
                    max_steps: p.max_steps,
                    max_tool_calls: p.max_tool_calls,
                    max_parallel_tool_calls: p.max_parallel_tool_calls,
                    selected_tool_codes: p.selected_tool_codes,
                    selected_server_codes: p.selected_server_codes,
                    native_tool_scheme_code: p.native_tool_scheme_code,
                })
            }
        }
    };
}
assistant_response!(ListAssistantResponse);
assistant_response!(GetAssistantResponse);
assistant_response!(CreateAssistantResponse);
assistant_response!(UpdateAssistantResponse);
assistant_response!(DesignateMainResponse);
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAssistantRequest {
    pub assistant_code: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub role_prompt: String,
    pub avatar_id: Option<String>,
    #[serde(default)]
    pub candidate_model_codes: Vec<String>,
    #[serde(default = "custom_kind")]
    pub kind: AssistantKind,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "cadence")]
    pub heartbeat_minutes: u32,
    #[serde(default)]
    pub memory_auto_approve: bool,
}
fn custom_kind() -> AssistantKind {
    AssistantKind::Custom
}
fn yes() -> bool {
    true
}
fn cadence() -> u32 {
    30
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAssistantRequest {
    pub description: Option<String>,
    pub role_prompt: Option<String>,
    pub avatar_id: Option<String>,
    pub candidate_model_codes: Option<Vec<String>>,
    pub enabled: Option<bool>,
    pub heartbeat_minutes: Option<u32>,
    pub memory_auto_approve: Option<bool>,
}
async fn list(
    State(s): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ListAssistantResponse>>>, AssistantApiError> {
    let records = s
        .assistants
        .list()?
        .into_iter()
        .map(|d| ListAssistantResponse::from_record(&s.assistants, d))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ApiResponse::protocol_success(records)))
}
async fn detail(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<ApiResponse<GetAssistantResponse>>, AssistantApiError> {
    Ok(Json(ApiResponse::protocol_success(
        GetAssistantResponse::from_record(&s.assistants, s.assistants.get(&code)?)?,
    )))
}
async fn create(
    State(s): State<AppState>,
    Json(r): Json<CreateAssistantRequest>,
) -> Result<Json<ApiResponse<CreateAssistantResponse>>, AssistantApiError> {
    let d = s.assistants.create(CreateAssistantCommand {
        definition: CreateWorkspaceInternalAgentCommand {
            agent_code: r.assistant_code,
            name: r.name,
            description: r.description,
            role_prompt: r.role_prompt,
            avatar_id: r.avatar_id,
            group_name: None,
            candidate_model_codes: r.candidate_model_codes,
        },
        kind: r.kind,
        enabled: r.enabled,
        heartbeat_minutes: r.heartbeat_minutes,
        memory_auto_approve: r.memory_auto_approve,
    })?;
    Ok(Json(ApiResponse::protocol_success(
        CreateAssistantResponse::from_record(&s.assistants, d)?,
    )))
}
async fn update(
    State(s): State<AppState>,
    Path(code): Path<String>,
    Json(r): Json<UpdateAssistantRequest>,
) -> Result<Json<ApiResponse<UpdateAssistantResponse>>, AssistantApiError> {
    let d = s.assistants.update(
        &code,
        UpdateAssistantCommand {
            definition: UpdateWorkspaceInternalAgentDefinitionCommand {
                enabled: None,
                name: None,
                description: r.description,
                role_prompt: r.role_prompt,
                avatar_id: r.avatar_id,
                group_name: None,
                candidate_model_codes: r.candidate_model_codes,
            },
            enabled: r.enabled,
            heartbeat_minutes: r.heartbeat_minutes,
            memory_auto_approve: r.memory_auto_approve,
        },
    )?;
    Ok(Json(ApiResponse::protocol_success(
        UpdateAssistantResponse::from_record(&s.assistants, d)?,
    )))
}
async fn delete(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<ApiResponse<EmptyData>>, AssistantApiError> {
    s.assistants.delete(&code)?;
    Ok(Json(ApiResponse::protocol_success(EmptyData {})))
}
async fn designate_main(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<ApiResponse<DesignateMainResponse>>, AssistantApiError> {
    s.assistants.designate_main(&code)?;
    Ok(Json(ApiResponse::protocol_success(
        DesignateMainResponse::from_record(&s.assistants, s.assistants.get(&code)?)?,
    )))
}
macro_rules! session_response {
    ($name:ident) => {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            id: String,
            session_code: String,
            workspace_code: Option<String>,
            project_code: Option<String>,
            user_code: Option<String>,
            app_code: Option<String>,
            task_code: Option<String>,
            source_type: String,
            source_code: Option<String>,
            agent_code: Option<String>,
            system_agent_session_id: Option<String>,
            title: String,
            status: String,
            last_run_code: Option<String>,
            last_run_status: Option<String>,
            created_at: i64,
            updated_at: i64,
            owner_assistant_code: String,
            purpose: AssistantSessionPurpose,
        }
        impl $name {
            fn from_record(chat: crate::domain::chat::ChatSession, d: AssistantSession) -> Self {
                Self {
                    id: chat.id.to_string(),
                    session_code: chat.session_code,
                    workspace_code: d.workspace_code,
                    project_code: chat.project_code,
                    user_code: chat.user_code,
                    app_code: chat.app_code,
                    task_code: chat.task_code,
                    source_type: "Assistant".into(),
                    source_code: Some(d.assistant_code.clone()),
                    agent_code: chat.agent_code,
                    system_agent_session_id: chat.system_agent_session_id,
                    title: chat.title,
                    status: format!("{:?}", chat.status),
                    last_run_code: chat.last_run_code,
                    last_run_status: chat.last_run_status,
                    created_at: d.created_at,
                    updated_at: d.updated_at,
                    owner_assistant_code: d.assistant_code,
                    purpose: d.purpose,
                }
            }
        }
    };
}
session_response!(ResolveAssistantSessionResponse);
session_response!(MainAssistantSessionResponse);
session_response!(ListAssistantSessionResponse);
async fn main_session(
    State(s): State<AppState>,
    auth: RequestAuthContext,
) -> Result<Json<ApiResponse<MainAssistantSessionResponse>>, AssistantApiError> {
    let code = s.assistants.main()?.assistant_code;
    let mut soul = s.assistants.soul(&code)?;
    if soul.state != SoulState::Ready && soul.session_code.is_none() {
        soul = s
            .assistants
            .begin_awakening(&code, &auth.user().user_code, &auth.locale)?
            .0;
    }
    let d = if soul.user_code.as_deref() == Some(&auth.user().user_code)
        && soul.session_code.is_some()
    {
        s.assistants
            .session(soul.session_code.as_deref().unwrap())?
            .ok_or(AssistantError::NotFound)?
    } else {
        s.assistants.resolve_session(
            &code,
            AssistantSessionPurpose::Interactive,
            &format!("interactive:{}", auth.user().user_code),
            None,
        )?
    };
    let chat = s
        .chats
        .ensure_assistant_session(&d, Some(auth.user().user_code.clone()))
        .map_err(|_| AssistantError::Storage)?;
    Ok(Json(ApiResponse::protocol_success(
        MainAssistantSessionResponse::from_record(chat, d),
    )))
}
async fn resolve_session(
    State(s): State<AppState>,
    auth: RequestAuthContext,
    Path(code): Path<String>,
) -> Result<Json<ApiResponse<ResolveAssistantSessionResponse>>, AssistantApiError> {
    let mut soul = s.assistants.soul(&code)?;
    if soul.state != SoulState::Ready && soul.session_code.is_none() {
        soul = s
            .assistants
            .begin_awakening(&code, &auth.user().user_code, &auth.locale)?
            .0;
    }
    let d = if soul.user_code.as_deref() == Some(&auth.user().user_code)
        && soul.session_code.is_some()
    {
        s.assistants
            .session(soul.session_code.as_deref().unwrap())?
            .ok_or(AssistantError::NotFound)?
    } else {
        s.assistants.resolve_session(
            &code,
            AssistantSessionPurpose::Interactive,
            &format!("interactive:{}", auth.user().user_code),
            None,
        )?
    };
    let chat = s
        .chats
        .ensure_assistant_session(&d, Some(auth.user().user_code.clone()))
        .map_err(|_| AssistantError::Storage)?;
    Ok(Json(ApiResponse::protocol_success(
        ResolveAssistantSessionResponse::from_record(chat, d),
    )))
}
async fn sessions(
    State(s): State<AppState>,
    auth: RequestAuthContext,
    Path(code): Path<String>,
) -> Result<Json<ApiResponse<Vec<ListAssistantSessionResponse>>>, AssistantApiError> {
    let sessions = s.assistants.sessions(&code)?;
    let codes = sessions
        .iter()
        .map(|session| session.session_code.clone())
        .collect::<Vec<_>>();
    let mut chats = s
        .chats
        .sessions_by_codes(&codes)
        .map_err(|_| AssistantError::Storage)?
        .into_iter()
        .filter(|chat| chat.user_code.as_deref() == Some(&auth.user().user_code))
        .map(|chat| (chat.session_code.clone(), chat))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut heartbeat_count = 0;
    let result = sessions
        .into_iter()
        .filter_map(|session| {
            chats.remove(&session.session_code).and_then(|chat| {
                if session.purpose == AssistantSessionPurpose::Heartbeat {
                    heartbeat_count += 1;
                    if heartbeat_count > HEARTBEAT_HISTORY_LIMIT {
                        return None;
                    }
                }
                Some(ListAssistantSessionResponse::from_record(chat, session))
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(ApiResponse::protocol_success(result)))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAssistantEventResponse {
    event_id: String,
    source: AssistantEventSource,
    status: AssistantEventStatus,
    session_code: Option<String>,
    task_code: Option<String>,
    attempts: u32,
    result: Option<String>,
    created_at: i64,
}
async fn events(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<ApiResponse<Vec<ListAssistantEventResponse>>>, AssistantApiError> {
    Ok(Json(ApiResponse::protocol_success(
        s.assistants
            .activity_events(&code)?
            .into_iter()
            .map(|e| ListAssistantEventResponse {
                event_id: e.event_id,
                source: e.source,
                status: e.status,
                session_code: e.session_code,
                task_code: e.task_code,
                attempts: e.attempts,
                result: e.result,
                created_at: e.created_at,
            })
            .collect(),
    )))
}

async fn cancel_task(
    State(s): State<AppState>,
    Path((code, task_code)): Path<(String, String)>,
) -> Response {
    match s.tasks.cancel_assistant_task(&code, &task_code) {
        Ok(()) => Json(ApiResponse::protocol_success(EmptyData {})).into_response(),
        Err(error) => assistant_error_response(error.descriptor()),
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNotificationResponse {
    id: String,
    assistant_code: Option<String>,
    title: String,
    body: String,
    category: String,
    target: serde_json::Value,
    read_at: Option<i64>,
    resolved_at: Option<i64>,
    archived_at: Option<i64>,
    created_at: i64,
}
async fn notifications(
    State(s): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ListNotificationResponse>>>, AssistantApiError> {
    Ok(Json(ApiResponse::protocol_success(
        s.assistants
            .notifications()?
            .into_iter()
            .map(|n| ListNotificationResponse {
                id: n.id,
                assistant_code: n.assistant_code,
                title: n.title,
                body: n.body,
                category: n.category,
                target: n.target,
                read_at: n.read_at,
                resolved_at: n.resolved_at,
                archived_at: n.archived_at,
                created_at: n.created_at,
            })
            .collect(),
    )))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateNotificationRequest {
    pub read: bool,
    #[serde(default)]
    pub archived: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateNotificationsRequest {
    ids: Vec<String>,
    action: UpdateNotificationsAction,
}
#[derive(Deserialize)]
enum UpdateNotificationsAction {
    MarkRead,
    Archive,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateNotificationsResponse {
    updated_count: usize,
}
async fn update_notifications(
    State(s): State<AppState>,
    _auth: RequestAuthContext,
    Json(r): Json<UpdateNotificationsRequest>,
) -> Result<Json<ApiResponse<UpdateNotificationsResponse>>, AssistantApiError> {
    // AppState/repository are scoped to the selected profile, as for the list API.
    let updated_count = s.assistants.update_notifications(
        r.ids,
        matches!(r.action, UpdateNotificationsAction::Archive),
    )?;
    Ok(Json(ApiResponse::protocol_success(
        UpdateNotificationsResponse { updated_count },
    )))
}
async fn update_notification(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(r): Json<UpdateNotificationRequest>,
) -> Result<Json<ApiResponse<EmptyData>>, AssistantApiError> {
    s.assistants.update_notification(&id, r.read, r.archived)?;
    Ok(Json(ApiResponse::protocol_success(EmptyData {})))
}
async fn delete_notification(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<EmptyData>>, AssistantApiError> {
    s.assistants.delete_archived_notification(&id)?;
    Ok(Json(ApiResponse::protocol_success(EmptyData {})))
}
async fn delete_archived_notifications(
    State(s): State<AppState>,
) -> Result<Json<ApiResponse<usize>>, AssistantApiError> {
    Ok(Json(ApiResponse::protocol_success(
        s.assistants.delete_archived_notifications()?,
    )))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAssistantThreadResponse {
    session_code: String,
    title: Option<String>,
    status: String,
    latest_run_code: Option<String>,
    closed: bool,
    agent_code: String,
    origin_session_code: String,
    task_code: Option<String>,
}
async fn threads(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<ApiResponse<Vec<ListAssistantThreadResponse>>>, AssistantApiError> {
    s.assistants.get(&code)?;
    let records = s
        .agent_framework
        .assistant_thread_catalog(&code)
        .await
        .map_err(|e| AssistantError::Runtime(e.to_string()))?;
    Ok(Json(ApiResponse::protocol_success(
        records
            .into_iter()
            .map(|t| ListAssistantThreadResponse {
                session_code: t.summary.session_id,
                title: t.summary.title,
                status: format!("{:?}", t.summary.status),
                latest_run_code: t.summary.latest_run_id.map(|r| r.to_string()),
                closed: t.closed,
                agent_code: t.agent_code,
                origin_session_code: t.origin_session_code,
                task_code: t.task_code,
            })
            .collect(),
    )))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadAssistantThreadHistoryRequest {
    pub size: usize,
    pub cursor: Option<eliteforge_agent_core::HistoryCursor>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadAssistantThreadHistoryResponse {
    session_id: String,
    version: String,
    next_cursor: Option<eliteforge_agent_core::HistoryCursor>,
    runs: Vec<eliteforge_agent_core::HistoryRun>,
    records: eliteforge_agent_core::HistoryRecords,
    pending_interactions: Vec<eliteforge_agent_core::HistoryInteraction>,
    active_runs: Vec<eliteforge_agent_core::HistoryRun>,
}
async fn thread_history(
    State(s): State<AppState>,
    Path((code, thread)): Path<(String, String)>,
    Json(r): Json<ReadAssistantThreadHistoryRequest>,
) -> Result<Json<ApiResponse<ReadAssistantThreadHistoryResponse>>, AssistantApiError> {
    verify_thread(&s, &code, &thread).await?;
    if r.size == 0 || r.size > 100 {
        return Err(AssistantError::Invalid.into());
    }
    let mut h = s
        .agent_framework
        .assistant_thread_history(
            &code,
            &thread,
            eliteforge_agent_core::HistoryQuery {
                limit: r.size,
                cursor: r.cursor,
            },
        )
        .await
        .map_err(|e| AssistantError::Runtime(e.to_string()))?;
    for message in &mut h.records.messages {
        message.content = message
            .content
            .drain(..)
            .filter_map(crate::adapters::inbound::http::chats::history::public_history_block)
            .collect();
    }
    Ok(Json(ApiResponse::protocol_success(
        ReadAssistantThreadHistoryResponse {
            session_id: h.session_id,
            version: h.version,
            next_cursor: h.next_cursor,
            runs: h.runs,
            records: h.records,
            pending_interactions: h.pending_interactions,
            active_runs: h.active_runs,
        },
    )))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendAssistantThreadMessageRequest {
    pub content: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendAssistantThreadMessageResponse {
    session_code: String,
    status: String,
    latest_run_code: Option<String>,
}
async fn thread_message(
    State(s): State<AppState>,
    auth: RequestAuthContext,
    Path((code, thread)): Path<(String, String)>,
    Json(r): Json<SendAssistantThreadMessageRequest>,
) -> Result<Json<ApiResponse<SendAssistantThreadMessageResponse>>, AssistantApiError> {
    verify_thread(&s, &code, &thread).await?;
    if r.content.trim().is_empty() || r.content.len() > 1024 * 1024 {
        return Err(AssistantError::Invalid.into());
    }
    let summary = s
        .agent_framework
        .assistant_thread_send(
            &code,
            &thread,
            &r.content,
            &auth.user().user_code,
            &auth.locale,
            &s.assistant_invocation_service(),
        )
        .await
        .map_err(|e| AssistantError::Runtime(e.to_string()))?;
    Ok(Json(ApiResponse::protocol_success(
        SendAssistantThreadMessageResponse {
            session_code: summary.session_id,
            status: format!("{:?}", summary.status),
            latest_run_code: summary.latest_run_id.map(|r| r.to_string()),
        },
    )))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnswerAssistantThreadRequest {
    #[serde(default)]
    pub answers: Vec<AssistantThreadAnswer>,
    pub interaction_id: Option<String>,
    pub value: Option<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssistantThreadAnswer {
    pub interaction_id: String,
    pub value: serde_json::Value,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerAssistantThreadResponse {
    session_code: String,
    status: String,
    latest_run_code: Option<String>,
}
async fn thread_answer(
    State(s): State<AppState>,
    auth: RequestAuthContext,
    Path((code, thread)): Path<(String, String)>,
    Json(r): Json<AnswerAssistantThreadRequest>,
) -> Result<Json<ApiResponse<AnswerAssistantThreadResponse>>, AssistantApiError> {
    verify_thread(&s, &code, &thread).await?;
    let answers = match (r.interaction_id, r.value) {
        (Some(interaction_id), Some(value)) if r.answers.is_empty() => {
            vec![AssistantThreadAnswer {
                interaction_id,
                value,
            }]
        }
        (None, None) if !r.answers.is_empty() => r.answers,
        _ => return Err(AssistantError::Invalid.into()),
    };
    let answers = answers
        .into_iter()
        .map(|answer| {
            eliteforge_agent_core::RunId::parse_str(&answer.interaction_id)
                .map(|id| (id, answer.value))
                .map_err(|_| AssistantError::Invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let summary = s
        .agent_framework
        .assistant_thread_answer(
            &code,
            &thread,
            answers,
            &auth.user().user_code,
            &auth.locale,
            &s.assistant_invocation_service(),
        )
        .await
        .map_err(|e| AssistantError::Runtime(e.to_string()))?;
    Ok(Json(ApiResponse::protocol_success(
        AnswerAssistantThreadResponse {
            session_code: summary.session_id,
            status: format!("{:?}", summary.status),
            latest_run_code: summary.latest_run_id.map(|r| r.to_string()),
        },
    )))
}

async fn verify_thread(
    state: &AppState,
    assistant: &str,
    thread: &str,
) -> Result<(), AssistantError> {
    state.assistants.get(assistant)?;
    if !state
        .agent_framework
        .assistant_thread_catalog(assistant)
        .await
        .map_err(|e| AssistantError::Runtime(e.to_string()))?
        .iter()
        .any(|t| t.summary.session_id == thread)
    {
        return Err(AssistantError::NotFound);
    }
    Ok(())
}
