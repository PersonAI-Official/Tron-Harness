//! Transport and workspace authorization only; Framework owns history storage.
use crate::{
    adapters::inbound::http::auth::RequestAuthContext, app::AppState, response::ApiResponse,
    shared::locale::SupportedLocale,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use eliteforge_agent_core as fw;
use serde::{Deserialize, Serialize};
use tron_i18n::t;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadHistoryRequest {
    pub size: usize,
    pub cursor: Option<fw::HistoryCursor>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadHistoryResponse {
    pub compaction_details: std::collections::BTreeMap<String, Vec<CompactionBlockResponse>>,
    pub can_delete_turns: bool,
    pub session_id: String,
    pub version: String,
    pub next_cursor: Option<fw::HistoryCursor>,
    pub runs: Vec<fw::HistoryRun>,
    pub records: fw::HistoryRecords,
    pub pending_interactions: Vec<fw::HistoryInteraction>,
    pub active_runs: Vec<fw::HistoryRun>,
    /// Public delivery projections keyed by run code. Additive: older clients
    /// ignore the field, and private locations never appear here.
    pub artifacts_by_run: std::collections::BTreeMap<String, crate::domain::chat::ChatArtifacts>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionBlockResponse {
    id: String,
    before_tokens: usize,
    after_tokens: usize,
    kind: &'static str,
}
impl From<fw::SessionHistory> for ReadHistoryResponse {
    fn from(mut h: fw::SessionHistory) -> Self {
        for message in &mut h.records.messages {
            message.content = message
                .content
                .drain(..)
                .filter_map(public_history_block)
                .collect();
        }
        Self {
            compaction_details: Default::default(),
            can_delete_turns: h.can_delete_turns,
            session_id: h.session_id,
            version: h.version,
            next_cursor: h.next_cursor,
            runs: h.runs,
            records: h.records,
            pending_interactions: h.pending_interactions,
            active_runs: h.active_runs,
            artifacts_by_run: Default::default(),
        }
    }
}

// The lossless conversation codec is for model resumption. Only its public
// projection belongs in the transcript; genuine user JSON stays untouched.
pub(crate) fn public_history_block(
    block: fw::HistoryContentBlock,
) -> Option<fw::HistoryContentBlock> {
    let fw::HistoryContentBlock::Json { value } = &block else {
        return Some(block);
    };
    match value
        .pointer("/tronMessagePart/type")
        .and_then(serde_json::Value::as_str)
    {
        Some("reasoningSummary") => value
            .pointer("/tronMessagePart/text")
            .and_then(serde_json::Value::as_str)
            .map(|text| fw::HistoryContentBlock::Reasoning {
                text: text.to_owned(),
            }),
        Some("context" | "reasoning" | "compactionCheckpoint" | "toolCall" | "toolResult") => None,
        _ => Some(block),
    }
}
pub async fn read_history(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(session): Path<String>,
    request: Result<Json<ReadHistoryRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let locale = SupportedLocale::from_raw(&auth.locale);
    let Json(query) = match request {
        Ok(q) => q,
        Err(_) => return history_error(fw::StateError::Invalid("request".into()), locale),
    };
    let chat = match state.chats.get_session(&session) {
        Ok(chat) => chat,
        Err(error) => return super::error_mapper::chat_error_response(error),
    };
    let show_live_messages = query.cursor.is_none()
        && matches!(
            chat.source_type,
            crate::domain::chat::ChatSessionSourceType::Task
                | crate::domain::chat::ChatSessionSourceType::Automation
        );
    let result = async {
        let framework = state
            .agent_framework
            .storage_framework()
            .await
            .map_err(|_| fw::StateError::Backend("framework unavailable".into()))?;
        let history = framework
            .history()
            .map_err(|_| fw::StateError::Backend("history unavailable".into()))?
            .read_session(
                &session,
                fw::HistoryQuery {
                    limit: query.size,
                    cursor: query.cursor,
                },
            )
            .await?;
        let details = crate::adapters::outbound::persistence::sqlite::compaction_details::read_compaction_details(
            &state.agent_framework, &session, &history.records.steps,
        ).await?;
        let mut response = ReadHistoryResponse::from(history);
        if show_live_messages && !response.active_runs.is_empty()
            && let Ok(events) = state.agent_presentation.protocol_events(&session)
        {
            append_live_assistant_messages(&mut response, events);
        }
        response.artifacts_by_run = state
            .chats
            .artifacts_by_session(&session)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(run_code, records)| {
                crate::domain::chat::ChatArtifacts::from_records(&records)
                    .map(|artifacts| (run_code, artifacts))
            })
            .collect();
        response.compaction_details = details.into_iter().map(|(id, blocks)| (id,
            blocks.into_iter().map(|block| CompactionBlockResponse {
                id: block.id, before_tokens: block.before_tokens,
                after_tokens: block.after_tokens, kind: block.kind,
            }).collect()
        )).collect();
        Ok::<_, fw::StateError>(response)
    }
    .await;
    match result {
        Ok(history) => Json(ApiResponse::success(history)).into_response(),
        Err(error) => history_error(error, locale),
    }
}
// Agent UI polls structured history while an external run is active. Project the
// still-streaming messages into that view until Framework commits their records.
fn append_live_assistant_messages(
    history: &mut ReadHistoryResponse,
    events: Vec<crate::domain::agent::AgentAguiProtocolEvent>,
) {
    use std::collections::{HashMap, HashSet};

    let active_runs: HashMap<_, _> = history
        .active_runs
        .iter()
        .map(|run| (run.id.to_string(), run))
        .collect();
    let persisted: HashSet<_> = history
        .records
        .messages
        .iter()
        .map(|message| message.id.as_str())
        .collect();
    let mut pending = HashMap::new();
    for event in events {
        let data = event.data;
        let Some(id) = data.get("messageId").and_then(serde_json::Value::as_str) else {
            continue;
        };
        match event.sse_event.as_str() {
            "TEXT_MESSAGE_START"
                if data.get("role").and_then(serde_json::Value::as_str) == Some("assistant")
                    && !persisted.contains(id) =>
            {
                let native_run = data
                    .pointer("/metadata/framework/runId")
                    .and_then(serde_json::Value::as_str);
                let Some(run) = native_run.and_then(|id| active_runs.get(id)) else {
                    continue;
                };
                pending.entry(id.to_owned()).or_insert_with(|| {
                    let metadata = data.pointer("/metadata/framework/message");
                    fw::HistoryMessage {
                        id: id.to_owned(),
                        run_id: run.id,
                        created_at_ms: data
                            .get("timestamp")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(run.started_at_ms),
                        role: fw::Role::Assistant,
                        content: vec![fw::HistoryContentBlock::Text {
                            text: String::new(),
                        }],
                        status: fw::HistoryStatus::Running,
                        annotations: Vec::new(),
                        requested_skills: Vec::new(),
                        provenance: None,
                        step_id: metadata
                            .and_then(|message| message.get("stepId"))
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned),
                        phase: metadata
                            .and_then(|message| message.get("phase"))
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned),
                        output_index: metadata
                            .and_then(|message| message.get("outputIndex"))
                            .and_then(serde_json::Value::as_u64)
                            .and_then(|index| usize::try_from(index).ok()),
                        structured: None,
                    }
                });
            }
            "TEXT_MESSAGE_CONTENT" => {
                if let Some(message) = pending.get_mut(id)
                    && let Some(fw::HistoryContentBlock::Text { text }) =
                        message.content.first_mut()
                    && let Some(delta) = data.get("delta").and_then(serde_json::Value::as_str)
                {
                    text.push_str(delta);
                }
            }
            _ => {}
        }
    }
    let mut live_messages: Vec<_> = pending
        .into_values()
        .filter(|message| {
            matches!(message.content.first(),
            Some(fw::HistoryContentBlock::Text { text }) if !text.is_empty())
        })
        .collect();
    live_messages.sort_by(|left, right| {
        left.created_at_ms
            .cmp(&right.created_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    history.records.messages.extend(live_messages);
}

fn history_error(error: fw::StateError, locale: SupportedLocale) -> Response {
    let descriptor = crate::application::chat::history_error::descriptor(&error);
    let (status, message) = match descriptor.message_key {
        "tron.chat.history_query_invalid" => (
            StatusCode::BAD_REQUEST,
            t!("tron.chat.history_query_invalid", locale = locale),
        ),
        "tron.chat.history_version_changed" => (
            StatusCode::CONFLICT,
            t!("tron.chat.history_version_changed", locale = locale),
        ),
        "tron.chat.history_not_found" => (
            StatusCode::NOT_FOUND,
            t!("tron.chat.history_not_found", locale = locale),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            t!("tron.chat.history_read_failed", locale = locale),
        ),
    };
    (status, Json(ApiResponse::error(descriptor.code, message))).into_response()
}
