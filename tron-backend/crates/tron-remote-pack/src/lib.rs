//! Tron product pack for the shared remote RPC layer.
//!
//! The pack intentionally talks to the existing loopback HTTP API. This keeps
//! business handlers, profile middleware and streaming semantics exactly as
//! they already are; the remote protocol only changes how the phone reaches
//! the listener.

use async_trait::async_trait;
use eliteforge_agent_remote_rpc::{
    EventEmitter, Message, RemotePack, RemotePrincipal, RpcError, RpcMethod,
    RpcRegistry, SendMessageResult, SessionCreateParams, SessionHistoryParams, SessionHistoryResult,
    SessionListParams, SessionListResult, SessionRpc, SessionSendMessageParams,
    SessionStopRunParams, SessionSummary, SessionUnwatchEventsParams, SessionWatchEventsParams,
    StopRunResult, Subscription,
};
use base64::Engine;
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::broadcast;

const TRON_CODE: &str = "tron";
const TRON_NAME: &str = "Tron";

#[derive(Clone)]
pub struct TronRemotePack {
    http_base: String,
    client: reqwest::Client,
    /// Long-lived SSE responses must not inherit the unary request deadline.
    stream_client: reqwest::Client,
    agui: Option<Arc<dyn AguiEventSource>>,
}

/// Product-side bridge from the remote pack to Tron's AG-UI presentation
/// service. Tron implements this; the pack does not depend on Tron crates.
#[async_trait]
pub trait AguiEventSource: Send + Sync {
    async fn replay(&self, session_id: &str, cursor: Option<u64>) -> Result<Vec<Value>, String>;
    fn subscribe(&self, session_id: &str) -> Result<AguiLive, String>;
}

pub struct AguiLive {
    pub receiver: broadcast::Receiver<Value>,
}

impl TronRemotePack {
    pub fn new(http_base: impl Into<String>, agui: Option<Arc<dyn AguiEventSource>>) -> Self {
        Self {
            http_base: http_base.into().trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
            stream_client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .read_timeout(std::time::Duration::from_secs(900))
                .build()
                .unwrap_or_default(),
            agui,
        }
    }

    async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, RpcError> {
        self.get_with_assistant(path, query, None).await
    }

    /// PersonAI sessions are user-scoped, not workspace-scoped: Tron's chat
    /// middleware only resolves them when the request carries `X-Assistant-Code`.
    async fn get_with_assistant(
        &self,
        path: &str,
        query: &[(&str, &str)],
        assistant_code: Option<&str>,
    ) -> Result<Value, RpcError> {
        let mut request = self
            .client
            .get(format!("{}{}", self.http_base, path))
            .query(query)
            .header("Accept-Language", "zh-CN");
        if let Some(code) = assistant_code {
            request = request.header("X-Assistant-Code", code);
        }
        let response = request
            .send()
            .await
            .map_err(|e| RpcError::internal(format!("tron request failed: {e}")))?;
        let envelope: Value = response
            .json()
            .await
            .map_err(|e| RpcError::internal(format!("tron response invalid: {e}")))?;
        unwrap_envelope(envelope)
    }

    /// Chat and AG-UI endpoints resolve ownership from the workspace header
    /// (or query) and PersonAI sessions additionally from `X-Assistant-Code`.
    async fn get_with_scope(
        &self,
        path: &str,
        query: &[(&str, &str)],
        workspace: Option<&str>,
        assistant_code: Option<&str>,
    ) -> Result<Value, RpcError> {
        let mut request = self
            .client
            .get(format!("{}{}", self.http_base, path))
            .query(query)
            .header("Accept-Language", "zh-CN");
        if let Some(code) = workspace.map(str::trim).filter(|value| !value.is_empty()) {
            request = request.header("X-Workspace-Code", code);
        }
        if let Some(code) = assistant_code.map(str::trim).filter(|value| !value.is_empty()) {
            request = request.header("X-Assistant-Code", code);
        }
        let response = request
            .send()
            .await
            .map_err(|e| RpcError::internal(format!("tron request failed: {e}")))?;
        let envelope: Value = response
            .json()
            .await
            .map_err(|e| RpcError::internal(format!("tron response invalid: {e}")))?;
        unwrap_envelope(envelope)
    }

    async fn post(&self, path: &str, body: Value) -> Result<Value, RpcError> {
        self.post_with_assistant(path, body, None).await
    }

    async fn post_with_assistant(
        &self,
        path: &str,
        body: Value,
        assistant_code: Option<&str>,
    ) -> Result<Value, RpcError> {
        self.post_with_scope(path, body, None, assistant_code).await
    }

    /// Chat endpoints resolve the active workspace from `X-Workspace-Code`
    /// (or the query string) before the handler reads the JSON body; PersonAI
    /// sessions are resolved through `X-Assistant-Code` instead.
    async fn post_with_workspace(
        &self,
        path: &str,
        body: Value,
        workspace: &str,
    ) -> Result<Value, RpcError> {
        self.post_with_scope(path, body, Some(workspace), None).await
    }

    async fn post_with_scope(
        &self,
        path: &str,
        body: Value,
        workspace: Option<&str>,
        assistant_code: Option<&str>,
    ) -> Result<Value, RpcError> {
        let mut request = self
            .client
            .post(format!("{}{}", self.http_base, path))
            .header("Accept-Language", "zh-CN")
            .json(&body);
        if let Some(code) = workspace.map(str::trim).filter(|value| !value.is_empty()) {
            request = request.header("X-Workspace-Code", code);
        }
        if let Some(code) = assistant_code.map(str::trim).filter(|value| !value.is_empty()) {
            request = request.header("X-Assistant-Code", code);
        }
        let response = request
            .send()
            .await
            .map_err(|e| RpcError::internal(format!("tron request failed: {e}")))?;
        let envelope: Value = response
            .json()
            .await
            .map_err(|e| RpcError::internal(format!("tron response invalid: {e}")))?;
        unwrap_envelope(envelope)
    }

    async fn put_with_scope(
        &self,
        path: &str,
        body: Value,
        workspace: Option<&str>,
        assistant_code: Option<&str>,
    ) -> Result<Value, RpcError> {
        let mut request = self
            .client
            .put(format!("{}{}", self.http_base, path))
            .header("Accept-Language", "zh-CN")
            .json(&body);
        if let Some(code) = workspace.map(str::trim).filter(|value| !value.is_empty()) {
            request = request.header("X-Workspace-Code", code);
        }
        if let Some(code) = assistant_code.map(str::trim).filter(|value| !value.is_empty()) {
            request = request.header("X-Assistant-Code", code);
        }
        let response = request
            .send()
            .await
            .map_err(|e| RpcError::internal(format!("tron request failed: {e}")))?;
        let envelope: Value = response
            .json()
            .await
            .map_err(|e| RpcError::internal(format!("tron response invalid: {e}")))?;
        unwrap_envelope(envelope)
    }

    async fn post_multipart_with_scope(
        &self,
        path: &str,
        form: reqwest::multipart::Form,
        workspace: Option<&str>,
        assistant_code: Option<&str>,
    ) -> Result<Value, RpcError> {
        let mut request = self
            .client
            .post(format!("{}{}", self.http_base, path))
            .header("Accept-Language", "zh-CN")
            .multipart(form);
        if let Some(code) = workspace.map(str::trim).filter(|value| !value.is_empty()) {
            request = request.header("X-Workspace-Code", code);
        }
        if let Some(code) = assistant_code.map(str::trim).filter(|value| !value.is_empty()) {
            request = request.header("X-Assistant-Code", code);
        }
        let response = request
            .send()
            .await
            .map_err(|e| RpcError::internal(format!("tron request failed: {e}")))?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            return Err(RpcError::internal(format!(
                "tron attachment upload returned {status}: {detail}"
            )));
        }
        let envelope: Value = response
            .json()
            .await
            .map_err(|e| RpcError::internal(format!("tron response invalid: {e}")))?;
        unwrap_envelope(envelope)
    }

    /// Collects a server-sent-event response into the individual JSON events
    /// the phone already understands. Remote RPC only streams through
    /// `session.sendMessage`; interaction resumptions complete as one payload.
    async fn post_sse_collect(
        &self,
        path: &str,
        body: Value,
        workspace: Option<&str>,
        assistant_code: Option<&str>,
    ) -> Result<Vec<Value>, RpcError> {
        let mut request = self
            .stream_client
            .post(format!("{}{}", self.http_base, path))
            .header("Accept-Language", "zh-CN")
            .json(&body);
        if let Some(code) = workspace.map(str::trim).filter(|value| !value.is_empty()) {
            request = request.header("X-Workspace-Code", code);
        }
        if let Some(code) = assistant_code.map(str::trim).filter(|value| !value.is_empty()) {
            request = request.header("X-Assistant-Code", code);
        }
        let response = request
            .send()
            .await
            .map_err(|e| RpcError::internal(format!("tron stream failed: {e}")))?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            return Err(RpcError::internal(format!(
                "tron stream returned {status}: {detail}"
            )));
        }
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut collected = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| RpcError::internal(format!("stream read failed: {e}")))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(index) = buffer.find("\n\n") {
                let frame = buffer[..index].to_owned();
                buffer = buffer[index + 2..].to_owned();
                let data = frame
                    .lines()
                    .filter_map(|line| line.strip_prefix("data:"))
                    .map(str::trim_start)
                    .collect::<Vec<_>>()
                    .join("\n");
                if data.is_empty() {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<Value>(&data) {
                    collected.push(value);
                }
            }
        }
        Ok(collected)
    }

    /// PersonAI context travels in the RPC metadata because the session methods
    /// only carry a session id.
    fn metadata_assistant_code(metadata: &Option<Value>) -> Option<String> {
        metadata
            .as_ref()?
            .get("assistantCode")?
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    }

    /// The workspace a session lives in travels in the RPC metadata because the
    /// generic session contract only carries a session id. Falling back to the
    /// node's default workspace would make sessions in other workspaces
    /// unreadable ("session not found") on remote clients.
    fn metadata_workspace_code(metadata: &Option<Value>) -> Option<String> {
        metadata
            .as_ref()?
            .get("workspaceCode")?
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    }

    /// Chat sidebar sections are product navigation widgets, so the client
    /// renders whatever the node defines instead of hard-coded tabs.
    async fn navigation_menus(&self) -> Result<Value, RpcError> {
        let domains = self.get("/api/v1/navigation/domains", &[]).await?;
        let list = domains.as_array().cloned().unwrap_or_default();
        let domain = list
            .iter()
            .find(|item| item.get("isDefault").and_then(Value::as_bool).unwrap_or(false))
            .or_else(|| list.first())
            .and_then(|item| item.get("domainCode").and_then(Value::as_str))
            .unwrap_or("ADVANCED");
        let tree = self
            .get(
                &format!("/api/v1/navigation/domains/{domain}/menu-tree"),
                &[],
            )
            .await?;
        let mut menus = Vec::new();
        if let Some(items) = tree.get("menus").and_then(Value::as_array) {
            Self::collect_chat_widgets(items, &mut menus);
        }
        Ok(Value::Array(menus))
    }

    fn collect_chat_widgets(items: &[Value], out: &mut Vec<Value>) {
        for item in items {
            let menu_type = item.get("menuType").and_then(Value::as_str).unwrap_or("");
            let page_code = item.get("pageCode").and_then(Value::as_str).unwrap_or("");
            if menu_type == "Widget" && page_code.starts_with("chat.") {
                out.push(json!({
                    "menuCode": item.get("menuCode").cloned().unwrap_or(Value::Null),
                    "label": item.get("label").cloned().unwrap_or(Value::Null),
                    "pageCode": page_code,
                    "icon": item.get("icon").cloned().unwrap_or(Value::Null),
                }));
            }
            if let Some(children) = item.get("children").and_then(Value::as_array) {
                Self::collect_chat_widgets(children, out);
            }
        }
    }

    async fn post_query(&self, path: &str, query: &[(&str, &str)], body: Value) -> Result<Value, RpcError> {
        let response = self
            .client
            .post(format!("{}{}", self.http_base, path))
            .query(query)
            .header("Accept-Language", "zh-CN")
            .json(&body)
            .send()
            .await
            .map_err(|e| RpcError::internal(format!("tron request failed: {e}")))?;
        let envelope: Value = response
            .json()
            .await
            .map_err(|e| RpcError::internal(format!("tron response invalid: {e}")))?;
        unwrap_envelope(envelope)
    }

    async fn default_workspace(&self) -> Result<String, RpcError> {
        let data = self.get("/api/v1/admin/workspaces/options", &[]).await?;
        let list = data
            .as_array()
            .cloned()
            .or_else(|| data.get("options").and_then(Value::as_array).cloned())
            .unwrap_or_default();
        for item in &list {
            if let Some(code) = item
                .get("workspaceCode")
                .or_else(|| item.get("code"))
                .and_then(Value::as_str)
            {
                return Ok(code.to_owned());
            }
        }
        Err(RpcError::internal("no workspace is available"))
    }

    async fn workspace_for(&self, params_workspace: Option<&str>) -> Result<String, RpcError> {
        match params_workspace.map(str::trim).filter(|value| !value.is_empty()) {
            Some(value) => Ok(value.to_owned()),
            None => self.default_workspace().await,
        }
    }

    fn session_from_value(item: &Value, workspace: &str) -> Option<SessionSummary> {
        let id = item
            .get("sessionCode")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)?
            .to_owned();
        let title = item
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Chat")
            .to_owned();
        let status = item
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("ACTIVE")
            .to_owned();
        let updated_at = item
            .get("updatedAt")
            .cloned()
            .unwrap_or_else(|| json!(0));
        Some(SessionSummary {
            id,
            product_code: TRON_CODE.into(),
            title,
            updated_at,
            status,
            workspace: Some(workspace.into()),
            metadata: Some(json!({
                "sourceType": item.get("sourceType").cloned().unwrap_or(Value::Null),
                "assistantCode": item.get("agentCode").cloned().unwrap_or(Value::Null),
                "projectCode": item.get("projectCode").cloned().unwrap_or(Value::Null),
                "directoryCode": item.get("directoryCode").cloned().unwrap_or(Value::Null),
                "workingDirectoryCode": item.get("workingDirectoryCode").cloned().unwrap_or(Value::Null),
            })),
        })
    }

    fn message_from_value(item: &Value) -> Option<Message> {
        let id = item.get("messageCode").cloned()?;
        let role = item
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("ASSISTANT")
            .to_lowercase();
        Some(Message {
            id,
            role,
            content: item
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            created_at: item.get("createdAt").cloned().unwrap_or(Value::Null),
            run_id: item
                .get("runCode")
                .and_then(Value::as_str)
                .map(str::to_owned),
            metadata: Some(item.clone()),
        })
    }
}

fn session_key(session: &Value) -> (&str, &str) {
    (
        session["updatedAt"].as_str().unwrap_or_default().trim_end_matches('Z'),
        session["sessionCode"].as_str().unwrap_or_default(),
    )
}

fn page_session_groups(
    mut groups: Value,
    cursor: Option<&str>,
    limit: usize,
) -> Result<(Value, Option<String>), RpcError> {
    let cursor: Option<(String, String)> = cursor
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| RpcError::invalid_params("invalid session cursor"))?;
    let groups_array = groups.as_array_mut()
        .ok_or_else(|| RpcError::internal("invalid session groups"))?;
    let mut keys = groups_array.iter()
        .filter_map(|group| group["sessions"].as_array())
        .flatten()
        .map(session_key)
        .filter(|key| cursor.as_ref().is_none_or(|(time, code)| *key < (time.as_str(), code.as_str())))
        .collect::<Vec<_>>();
    keys.sort_unstable_by(|a, b| b.cmp(a));
    let next = (keys.len() > limit).then(|| json!(keys[limit - 1]).to_string());
    let selected = keys.into_iter().take(limit)
        .map(|(_, code)| code.to_owned()).collect::<std::collections::HashSet<_>>();
    for group in groups_array.iter_mut() {
        if let Some(sessions) = group["sessions"].as_array_mut() {
            sessions.retain(|session| selected.contains(session_key(session).1));
        }
        if let Some(codes) = group["pendingUserInputSessionCodes"].as_array_mut() {
            codes.retain(|code| code.as_str().is_some_and(|code| selected.contains(code)));
        }
    }
    groups_array.retain(|group| group["sessions"].as_array().is_some_and(|items| !items.is_empty()));
    Ok((groups, next))
}

#[async_trait]
impl SessionRpc for TronRemotePack {
    async fn list(&self, _ctx: &RemotePrincipal, params: SessionListParams) -> Result<SessionListResult, RpcError> {
        let workspace = self
            .workspace_for(params.filter.as_ref().and_then(|v| v.get("workspace")).and_then(Value::as_str))
            .await?;
        let data = self
            .get("/api/v1/chats/session-groups", &[("workspaceCode", &workspace), ("sourceType", "MANUAL")])
            .await?;
        let limit = params.limit.unwrap_or(50).clamp(1, 100) as usize;
        let (data, next_cursor) = page_session_groups(data, params.cursor.as_deref(), limit)?;
        let groups = data.as_array().cloned().unwrap_or_default();
        let mut sessions = Vec::new();
        for group in groups {
            if let Some(items) = group.get("sessions").and_then(Value::as_array) {
                for item in items {
                    if let Some(session) = Self::session_from_value(item, &workspace) {
                        sessions.push(session);
                    }
                }
            }
        }
        Ok(SessionListResult {
            sessions,
            next_cursor,
        })
    }

    async fn history(&self, _ctx: &RemotePrincipal, params: SessionHistoryParams) -> Result<SessionHistoryResult, RpcError> {
        let workspace = self
            .workspace_for(Self::metadata_workspace_code(&params.metadata).as_deref())
            .await?;
        let assistant = Self::metadata_assistant_code(&params.metadata);
        let data = self
            .get_with_assistant(
                &format!("/api/v1/chats/sessions/{}/messages", params.session_id),
                &[("workspaceCode", &workspace)],
                assistant.as_deref(),
            )
            .await?;
        let items = data
            .as_array()
            .cloned()
            .or_else(|| data.get("messages").and_then(Value::as_array).cloned())
            .unwrap_or_default();
        let messages = items.iter().filter_map(Self::message_from_value).collect();
        Ok(SessionHistoryResult {
            messages,
            next_cursor: None,
        })
    }

    async fn create(&self, _ctx: &RemotePrincipal, params: SessionCreateParams) -> Result<SessionSummary, RpcError> {
        let workspace = self.workspace_for(params.workspace.as_deref()).await?;
        let mut body = json!({ "workspaceCode": workspace });
        if let Some(title) = params.title {
            body["title"] = json!(title);
        }
        // A session started from a project/directory group must keep that
        // context, otherwise it lands in "随聊" with no workspace of its own.
        if let Some(metadata) = params.metadata.as_ref() {
            for key in ["projectCode", "directoryCode", "workingDirectoryCode"] {
                if let Some(value) = metadata
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    body[key] = json!(value);
                }
            }
        }
        let data = self
            .post_with_workspace("/api/v1/chats/sessions", body, &workspace)
            .await?;
        Self::session_from_value(&data, &workspace)
            .ok_or_else(|| RpcError::internal("tron did not return a session"))
    }

    async fn send_message(
        &self,
        _ctx: &RemotePrincipal,
        params: SessionSendMessageParams,
        events: EventEmitter,
    ) -> Result<SendMessageResult, RpcError> {
        let workspace = self
            .workspace_for(Self::metadata_workspace_code(&params.metadata).as_deref())
            .await?;
        let assistant = Self::metadata_assistant_code(&params.metadata);
        let mut body = json!({
            "content": params.content,
            "workspaceCode": workspace,
            "clientCapabilities": params.client_capabilities.unwrap_or_else(|| json!({"userInputRequests": false})),
        });
        if let Some(attachments) = params.attachments {
            body["attachments"] = attachments;
        }
        // The remote client sends the same explicit model selection the desktop
        // client sends. Without it Tron skips its auto-model router and hands the
        // agent a placeholder route with no context window, so the run fails
        // before the first model call.
        if let Some(selection) = params.model_selection {
            body["modelSelection"] = selection;
        }
        if let Some(effort) = params
            .reasoning_effort
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
        {
            body["reasoningEffort"] = json!(effort);
        }
        if let Some(metadata) = params.metadata {
            body["a2uiMetadata"] = metadata;
        }
        let mut request = self
            .stream_client
            .post(format!(
                "{}/api/v1/chats/sessions/{}/messages/stream",
                self.http_base, params.session_id
            ))
            .query(&[("workspaceCode", workspace.as_str())])
            .header("Accept-Language", "zh-CN")
            .json(&body);
        if let Some(code) = assistant.as_deref() {
            request = request.header("X-Assistant-Code", code);
        }
        let response = request
            .send()
            .await
            .map_err(|e| RpcError::internal(format!("tron stream failed: {e}")))?;
        if !response.status().is_success() {
            return Err(RpcError::internal(format!(
                "tron stream returned {}",
                response.status()
            )));
        }
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut run_id = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| RpcError::internal(format!("stream read failed: {e}")))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(index) = buffer.find("\n\n") {
                let frame = buffer[..index].to_owned();
                buffer = buffer[index + 2..].to_owned();
                let data = frame
                    .lines()
                    .filter_map(|line| line.strip_prefix("data:"))
                    .map(str::trim_start)
                    .collect::<Vec<_>>()
                    .join("\n");
                if data.is_empty() {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<Value>(&data) {
                    if run_id.is_empty() {
                        run_id = value
                            .get("runId")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                    }
                    events.emit(value)?;
                }
            }
            if events.is_cancelled() {
                break;
            }
        }
        if run_id.is_empty() {
            run_id = format!("run-{}", eliteforge_agent_remote_protocol::now_ms());
        }
        Ok(SendMessageResult { run_id })
    }

    async fn stop_run(&self, _ctx: &RemotePrincipal, params: SessionStopRunParams) -> Result<StopRunResult, RpcError> {
        let body = json!({
            "sessionCode": params.session_id,
            "expectedRunCode": params.run_id.unwrap_or_default(),
        });
        let assistant = Self::metadata_assistant_code(&params.metadata);
        let workspace = self
            .workspace_for(Self::metadata_workspace_code(&params.metadata).as_deref())
            .await?;
        let _ = self
            .post_with_scope(
                "/api/v1/chats/sessions/stop-run",
                body,
                Some(workspace.as_str()),
                assistant.as_deref(),
            )
            .await?;
        Ok(StopRunResult { cancelled: true })
    }

    async fn watch_events(
        &self,
        _ctx: &RemotePrincipal,
        params: SessionWatchEventsParams,
        events: EventEmitter,
    ) -> Result<Subscription, RpcError> {
        let source = self
            .agui
            .as_ref()
            .ok_or_else(|| RpcError::unsupported("AG-UI event source is not connected"))?
            .clone();
        for event in source
            .replay(&params.session_id, params.cursor)
            .await
            .map_err(RpcError::internal)?
        {
            events.emit(event)?;
        }
        let mut live = source
            .subscribe(&params.session_id)
            .map_err(RpcError::internal)?;
        let cancelled = events.cancelled();
        loop {
            tokio::select! {
                _ = cancelled.cancelled() => break,
                received = live.receiver.recv() => match received {
                    Ok(event) => { events.emit(event)?; }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        for event in source.replay(&params.session_id, params.cursor).await.map_err(RpcError::internal)? {
                            events.emit(event)?;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
        Ok(Subscription {
            subscription_id: format!("sub-{}", eliteforge_agent_remote_protocol::now_ms()),
        })
    }

    async fn unwatch_events(&self, _ctx: &RemotePrincipal, _params: SessionUnwatchEventsParams) -> Result<(), RpcError> {
        Ok(())
    }
}

struct TronMethod {
    pack: TronRemotePack,
    method: String,
}

#[async_trait]
impl RpcMethod for TronMethod {
    async fn call(&self, _ctx: &RemotePrincipal, params: Value) -> Result<Value, RpcError> {
        let workspace = params
            .get("workspaceCode")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let assistant_code = params
            .get("assistantCode")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        match self.method.as_str() {
            "tron.workspaces.list" => self.pack.get("/api/v1/admin/workspaces/options", &[]).await,
            // The same option list the desktop composer renders: each option
            // carries the server-owned `modelSelection`, so the phone never
            // invents a model code or quality tier.
            "tron.models.selectOptions" => {
                self.pack
                    .get(
                        "/api/v1/model-providers/select-options",
                        &[("workspaceCode", workspace.as_str())],
                    )
                    .await
            }
            "tron.assistants.list" => {
                self.pack
                    .get("/api/v1/assistants", &[("workspaceCode", workspace.as_str())])
                    .await
            }
            "tron.assistants.mainSession" => {
                self.pack
                    .post_query(
                        "/api/v1/assistants/main/session",
                        &[("workspaceCode", workspace.as_str())],
                        json!({}),
                    )
                    .await
            }
            "tron.assistants.createSession" => {
                self.pack
                    .post_query(
                        &format!("/api/v1/assistants/{assistant_code}/sessions"),
                        &[("workspaceCode", workspace.as_str())],
                        json!({}),
                    )
                    .await
            }
            "tron.projects.list" => self.pack.post("/api/v1/projects/list", params).await,
            "tron.directories.list" => {
                self.pack
                    .get("/api/v1/directories", &[("workspaceCode", workspace.as_str())])
                    .await
            }
            "tron.session.groups" => {
                let groups = self.pack
                    .get(
                        "/api/v1/chats/session-groups",
                        &[("workspaceCode", workspace.as_str()), ("sourceType", "MANUAL")],
                    )
                    .await?;
                let Some(limit) = params.get("limit").and_then(Value::as_u64) else {
                    return Ok(groups);
                };
                let (groups, next_cursor) = page_session_groups(
                    groups, params.get("cursor").and_then(Value::as_str), limit.clamp(1, 100) as usize,
                )?;
                Ok(json!({"groups": groups, "nextCursor": next_cursor}))
            }
            "tron.chat.protocolEvents" => {
                let session_code = params
                    .get("sessionCode")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let data = self
                    .pack
                    .get_with_scope(
                        &format!(
                            "/api/v1/agents/agui/streams/{session_code}/protocol-events"
                        ),
                        &[],
                        Some(workspace.as_str()),
                        Some(assistant_code.as_str()),
                    )
                    .await?;
                // The phone only renders the process; text deltas already come
                // from the chat transcript. Keeping the payload small avoids
                // hitting the tunnel's per-result size cap on long sessions.
                Ok(Value::Array(
                    data.as_array()
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|entry| entry.get("data").cloned())
                        .filter(|event| {
                            matches!(
                                event.get("type").and_then(Value::as_str).unwrap_or_default(),
                                "RUN_STARTED"
                                    | "RUN_FINISHED"
                                    | "RUN_ERROR"
                                    | "TOOL_CALL_START"
                                    | "TOOL_CALL_ARGS"
                                    | "TOOL_CALL_END"
                                    | "TOOL_CALL_RESULT"
                                    | "REASONING_MESSAGE_CONTENT"
                            )
                        })
                        .map(|mut event| {
                            // The final answer already lives in the transcript;
                            // replaying it again would balloon the payload.
                            if event.get("type").and_then(Value::as_str)
                                == Some("RUN_FINISHED")
                            {
                                if let Some(object) = event.as_object_mut() {
                                    object.remove("result");
                                }
                            }
                            event
                        })
                        .collect(),
                ))
            }
            "tron.chat.setWorkingDirectory" => {
                let session_code = params
                    .get("sessionCode")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let directory_code = params
                    .get("directoryCode")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                self.pack
                    .put_with_scope(
                        &format!(
                            "/api/v1/chats/sessions/{session_code}/working-directory"
                        ),
                        json!({ "directoryCode": directory_code }),
                        Some(workspace.as_str()),
                        Some(assistant_code.as_str()),
                    )
                    .await
            }
            "tron.chat.steer" => {
                let session_code = params
                    .get("sessionCode")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                let run_id = params
                    .get("runId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                let content = params
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                if session_code.is_empty() || run_id.is_empty() || content.is_empty() {
                    return Err(RpcError::invalid_params(
                        "sessionCode, runId and content are required",
                    ));
                }
                self.pack
                    .post_with_scope(
                        "/api/v1/chats/sessions/steer-message",
                        json!({
                            "sessionCode": session_code,
                            "expectedRunCode": run_id,
                            "content": content,
                            "attachments": [],
                            "annotations": [],
                            "requestedSkills": [],
                        }),
                        Some(workspace.as_str()),
                        Some(assistant_code.as_str()),
                    )
                    .await
            }
            "tron.chat.answerUserInput" => {
                let session_code = params
                    .get("sessionCode")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                let request_code = params
                    .get("requestCode")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                if session_code.is_empty() || request_code.is_empty() {
                    return Err(RpcError::invalid_params(
                        "sessionCode and requestCode are required",
                    ));
                }
                let mut body = json!({
                    "answers": params.get("answers").cloned().unwrap_or_else(|| json!([])),
                    "resume": params.get("resume").cloned().unwrap_or_else(|| json!([])),
                    "clientCapabilities": params
                        .get("clientCapabilities")
                        .cloned()
                        .unwrap_or_else(|| json!({"userInputRequests": true})),
                });
                if let Some(metadata) = params.get("a2uiMetadata") {
                    body["a2uiMetadata"] = metadata.clone();
                }
                let events = self
                    .pack
                    .post_sse_collect(
                        &format!(
                            "/api/v1/chats/sessions/{session_code}/user-input-requests/{request_code}/answers/stream"
                        ),
                        body,
                        Some(workspace.as_str()),
                        Some(assistant_code.as_str()),
                    )
                    .await?;
                Ok(Value::Array(events))
            }
            "tron.chat.uploadAttachment" => {
                let session_code = params
                    .get("sessionCode")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                let file_name = params
                    .get("fileName")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                let content_type = params
                    .get("contentType")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("application/octet-stream")
                    .to_owned();
                let data = params
                    .get("dataBase64")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if session_code.is_empty() || file_name.is_empty() || data.is_empty() {
                    return Err(RpcError::invalid_params(
                        "sessionCode, fileName and dataBase64 are required",
                    ));
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|_| RpcError::invalid_params("dataBase64 is invalid"))?;
                let kind = params
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("file")
                    .to_owned();
                let summary = params
                    .get("summary")
                    .and_then(Value::as_str)
                    .unwrap_or(file_name.as_str())
                    .to_owned();
                let part = reqwest::multipart::Part::bytes(bytes)
                    .file_name(file_name)
                    .mime_str(&content_type)
                    .map_err(|_| RpcError::invalid_params("contentType is invalid"))?;
                let form = reqwest::multipart::Form::new()
                    .text("kind", kind)
                    .text("summary", summary)
                    .text("metadata", "{}")
                    .part("file", part);
                self.pack
                    .post_multipart_with_scope(
                        &format!("/api/v1/chats/sessions/{session_code}/attachments"),
                        form,
                        Some(workspace.as_str()),
                        Some(assistant_code.as_str()),
                    )
                    .await
            }
            "tron.chat.userInputRequests" => {
                let session_code = params
                    .get("sessionCode")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                if session_code.is_empty() {
                    return Err(RpcError::invalid_params("sessionCode is required"));
                }
                self.pack
                    .get_with_scope(
                        &format!("/api/v1/chats/sessions/{session_code}/user-input-requests"),
                        &[],
                        Some(workspace.as_str()),
                        Some(assistant_code.as_str()),
                    )
                    .await
            }
            "tron.chat.a2uiState" => {
                let session_code = params
                    .get("sessionCode")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                if session_code.is_empty() {
                    return Err(RpcError::invalid_params("sessionCode is required"));
                }
                self.pack
                    .get_with_scope(
                        &format!("/api/v1/chats/sessions/{session_code}/a2ui-state"),
                        &[],
                        Some(workspace.as_str()),
                        Some(assistant_code.as_str()),
                    )
                    .await
            }
            "tron.chat.a2uiAction" => {
                let session_code = params
                    .get("sessionCode")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                let action_id = params
                    .get("actionId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                let surface_instance_id = params
                    .get("surfaceInstanceId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                let expected_revision = params
                    .get("expectedRevision")
                    .and_then(Value::as_u64)
                    .unwrap_or_default();
                if session_code.is_empty() || action_id.is_empty() || surface_instance_id.is_empty()
                {
                    return Err(RpcError::invalid_params(
                        "sessionCode, actionId and surfaceInstanceId are required",
                    ));
                }
                let events = self
                    .pack
                    .post_sse_collect(
                        &format!("/api/v1/chats/sessions/{session_code}/a2ui/actions/stream"),
                        json!({
                            "actionId": action_id,
                            "surfaceInstanceId": surface_instance_id,
                            "expectedRevision": expected_revision,
                            "message": params.get("message").cloned().unwrap_or(Value::Null),
                            "metadata": params.get("metadata").cloned().unwrap_or_else(|| json!({})),
                        }),
                        Some(workspace.as_str()),
                        Some(assistant_code.as_str()),
                    )
                    .await?;
                Ok(Value::Array(events))
            }
            "tron.navigation.menus" => self.pack.navigation_menus().await,
            "tron.skins.list" => self.pack.get("/api/v1/system/skins", &[]).await,
            "tron.skins.active" => self.pack.get("/api/v1/system/skins/active", &[]).await,
            other => Err(RpcError::unsupported(format!("{other} is not implemented"))),
        }
    }
}

impl RemotePack for TronRemotePack {
    fn session_rpc(&self) -> Option<Arc<dyn SessionRpc>> {
        Some(Arc::new(self.clone()))
    }

    fn register(&self, registry: &mut RpcRegistry) {
        for method in [
            "tron.workspaces.list",
            "tron.models.selectOptions",
            "tron.assistants.list",
            "tron.assistants.mainSession",
            "tron.assistants.createSession",
            "tron.projects.list",
            "tron.directories.list",
            "tron.session.groups",
            "tron.chat.protocolEvents",
            "tron.chat.setWorkingDirectory",
            "tron.chat.steer",
            "tron.chat.answerUserInput",
            "tron.chat.uploadAttachment",
            "tron.chat.userInputRequests",
            "tron.chat.a2uiState",
            "tron.chat.a2uiAction",
            "tron.navigation.menus",
            "tron.skins.list",
            "tron.skins.active",
        ] {
            registry.register(
                method,
                Arc::new(TronMethod {
                    pack: self.clone(),
                    method: method.to_owned(),
                }),
            );
        }
    }
}

fn unwrap_envelope(value: Value) -> Result<Value, RpcError> {
    let code = value
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("SUCCESS");
    if code != "SUCCESS" && code != "0" {
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("tron request failed");
        return Err(RpcError::internal(message.to_owned()));
    }
    Ok(value.get("data").cloned().unwrap_or(Value::Null))
}

pub fn product_identity() -> eliteforge_agent_remote_protocol::ProductIdentity {
    eliteforge_agent_remote_protocol::ProductIdentity {
        product_code: TRON_CODE.into(),
        product_name: TRON_NAME.into(),
        node_id: String::new(),
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        machine_name: String::new(),
    }
}

