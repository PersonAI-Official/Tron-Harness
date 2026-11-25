//! Authenticated transport over Framework-owned A2UI state and action receipts.
use crate::{
    adapters::inbound::http::auth::RequestAuthContext, app::AppState,
    application::chat::SendChatMessageCommand, response::ApiResponse,
    shared::locale::SupportedLocale,
};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use eliteforge_agent_core as fw;
use eliteforge_agent_official_plugin_a2ui as a2ui;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{convert::Infallible, time::Duration};
use tron_i18n::t;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionRequest {
    action_id: String,
    surface_instance_id: String,
    expected_revision: u64,
    message: Value,
    metadata: Value,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateResponse {
    session_id: String,
    revision: u64,
    surfaces: Vec<a2ui::SurfaceRecord>,
    pending_interactions: Vec<a2ui::InteractionSurface>,
}
async fn service(state: &AppState) -> Result<std::sync::Arc<a2ui::A2uiService>, fw::AgentError> {
    state
        .agent_framework
        .runtime_framework()
        .await
        .map_err(|_| fw::AgentError::Run("Framework unavailable".into()))?
        .services()?
        .get::<a2ui::A2uiService>()
        .ok_or_else(|| fw::AgentError::Run("A2UI unavailable".into()))
}
pub async fn state(
    State(app): State<AppState>,
    auth: RequestAuthContext,
    Path(session): Path<String>,
) -> Response {
    if let Err(error) = app.chats.get_session(&session) {
        return super::error_mapper::chat_error_response(error);
    }
    if !app
        .agent_framework
        .capability_state()
        .0
        .get(a2ui::A2UI_PACK_ID)
        .copied()
        .unwrap_or(true)
    {
        return Json(ApiResponse::success(StateResponse {
            session_id: session,
            revision: 0,
            surfaces: vec![],
            pending_interactions: vec![],
        }))
        .into_response();
    }
    let result = async { service(&app).await?.state(&session).await }.await;
    match result {
        Ok(s) => Json(ApiResponse::success(StateResponse {
            session_id: s.session_id,
            revision: s.revision,
            surfaces: s.surfaces,
            pending_interactions: s.pending_interactions,
        }))
        .into_response(),
        Err(e) => error(e, SupportedLocale::from_raw(&auth.locale)),
    }
}
pub async fn action(
    State(app): State<AppState>,
    auth: RequestAuthContext,
    Path(session): Path<String>,
    request: Result<Json<ActionRequest>, JsonRejection>,
) -> Response {
    let locale = SupportedLocale::from_raw(&auth.locale);
    if let Err(e) = app.chats.ensure_session_writable(&session) {
        return super::error_mapper::chat_error_response(e);
    }
    let (packs, extensions) = app.agent_framework.capability_state();
    if !packs.get(a2ui::A2UI_PACK_ID).copied().unwrap_or(true)
        || !extensions.get("tool").copied().unwrap_or(true)
    {
        return error(fw::AgentError::State(fw::StateError::NotFound), locale);
    }
    let Json(request) = match request {
        Ok(v) => v,
        Err(_) => {
            return error(
                fw::AgentError::InvalidConfiguration("Invalid request".into()),
                locale,
            );
        }
    };
    let service = match service(&app).await {
        Ok(s) => s,
        Err(e) => return error(e, locale),
    };
    let input = a2ui::ActionInput {
        action_id: request.action_id,
        surface_instance_id: request.surface_instance_id,
        expected_revision: request.expected_revision,
        message: request.message,
        metadata: request.metadata,
    };
    // Existing receipts remain observable while the session is busy.
    let existing = service
        .action_receipt(&session, &input.action_id)
        .await
        .is_ok();
    if !existing {
        if let Err(response) = super::handler::ensure_no_active_run(&app, &session) {
            return response;
        }
    }
    let receipt = match service.admit_action(&session, input).await {
        Ok(r) => r,
        Err(e) => return error(e, locale),
    };
    let claimed = match service
        .claim_action(&session, &receipt.input.action_id)
        .await
    {
        Ok(v) => v,
        Err(e) => return error(e, locale),
    };
    if !claimed {
        // Reconnect to the original product completion. Never dispatch a second run.
        let id = receipt.input.action_id;
        let run = receipt.run_id;
        let host = app.agent_framework.clone();
        let stream = async_stream::stream! {
            let started=tokio::time::Instant::now();
            let deadline=tokio::time::Instant::now()+Duration::from_secs(1800);
            loop {
                match service.action_receipt(&session,&id).await {
                    Ok(r) if r.completion.is_some()=>{let data=r.completion.unwrap_or(Value::Null);yield Ok::<Event,Infallible>(Event::default().event(data["type"].as_str().unwrap_or("RUN_FINISHED")).json_data(data).unwrap_or_default());break;}
                    Err(_)=>break,
                    _=>{}
                }
                let uncertain = if started.elapsed() > Duration::from_secs(5) {
                    match (host.storage_framework().await, fw::RunId::parse_str(&run)) {
                        (Ok(framework),Ok(id)) => match framework.runs() {
                            Ok(runs) => !matches!(runs.get(id).await, Ok(record) if record.status == fw::ManagedRunStatus::Running && record.lease_until_ms > fw::unix_time_ms()),
                            Err(_) => true,
                        },
                        _ => true,
                    }
                } else {false};
                if uncertain || tokio::time::Instant::now()>deadline {yield Ok(Event::default().event("RUN_ERROR").json_data(json!({"type":"RUN_ERROR","threadId":session,"runId":run,"productCompletion":true,"code":"101031005","message":t!("error.chat.a2ui_run_uncertain", locale = locale)})).unwrap_or_default());break;}
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        return Sse::new(stream)
            .keep_alive(KeepAlive::default())
            .into_response();
    }
    let session_record = match app.chats.get_session(&session) {
        Ok(s) => s,
        Err(e) => return super::error_mapper::chat_error_response(e),
    };
    let model_selection = match service.state(&session).await {
        Ok(state) => state
            .surfaces
            .iter()
            .find(|s| s.surface_instance_id == receipt.input.surface_instance_id)
            .and_then(|s| {
                app.model_providers
                    .get_run_snapshot(&s.run_id)
                    .ok()
                    .flatten()
            })
            .map(|snapshot| snapshot.model_selection),
        Err(_) => None,
    };
    let command = SendChatMessageCommand {
        annotations: Vec::new(),
        requested_skills: Vec::new(),
        run_code: Some(receipt.run_id),
        a2ui_action_id: Some(receipt.input.action_id),
        content: Some(format!(
            "UI action {}: {}",
            receipt.input.message["action"]["name"]
                .as_str()
                .unwrap_or("submit"),
            receipt.input.message["action"]["context"]
        )),
        workspace_code: Some(session_record.workspace_code),
        agent_code: Some(receipt.owner_agent_id),
        app_code: None,
        task_code: None,
        model_selection,
        model_code: None,
        system_model_id: None,
        reasoning_effort: None,
        page_context: None,
        limits: None,
        tools: vec![],
        attachments: vec![],
        resume: vec![],
        client_capabilities: Some(crate::domain::chat::ChatClientCapabilities {
            user_input_requests: true,
            a2ui: Some(crate::domain::chat::ChatA2uiCapabilities {
                version: a2ui::VERSION.into(),
                catalog_ids: vec![
                    a2ui::AGENT_CATALOG_ID.into(),
                    a2ui::MINIMAL_CATALOG_ID.into(),
                ],
            }),
            artifacts: None,
        }),
        a2ui_metadata: Some(
            json!({"action":receipt.input.message,"transport":receipt.input.metadata,"surfaceInstanceId":receipt.input.surface_instance_id}),
        ),
    };
    let action_id = command.a2ui_action_id.clone().unwrap();
    let run_id = command.run_code.clone().unwrap();
    let response =
        super::handler::stream_message_command(app, auth, session.clone(), command).await;
    // Preparation can fail before a run task exists. Persist that terminal outcome
    // as well, otherwise a reconnect would wait forever on a claimed receipt.
    if !response.status().is_success() {
        let _ = service
            .complete_action(
                &session,
                &action_id,
                json!({
                    "type":"RUN_ERROR","threadId":session,"runId":run_id,
                    "productCompletion":true,"code":"101031006",
                    "message":t!("error.chat.a2ui_action_not_started", locale = locale)
                }),
            )
            .await;
    }
    response
}
fn error(e: fw::AgentError, locale: SupportedLocale) -> Response {
    let descriptor = crate::application::chat::a2ui_error::descriptor(&e);
    let (status, message) = match descriptor.message_key {
        "tron.chat.a2ui_conflict" => (
            StatusCode::CONFLICT,
            t!("tron.chat.a2ui_conflict", locale = locale),
        ),
        "tron.chat.a2ui_not_found" => (
            StatusCode::NOT_FOUND,
            t!("tron.chat.a2ui_not_found", locale = locale),
        ),
        "tron.chat.a2ui_invalid" => (
            StatusCode::BAD_REQUEST,
            t!("tron.chat.a2ui_invalid", locale = locale),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            t!("tron.chat.a2ui_unavailable", locale = locale),
        ),
    };
    (
        status,
        Json(ApiResponse::<crate::response::EmptyData>::error(
            descriptor.code,
            message,
        )),
    )
        .into_response()
}
