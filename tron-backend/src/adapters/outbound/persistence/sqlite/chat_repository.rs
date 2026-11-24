//! Tron owns only workspace/business links. Session lifecycle and presentation
//! history are stored through Framework services on the injected connection.
use super::framework_storage::{blocking, chat_error};
use crate::{
    application::{
        agent::FrameworkHost,
        chat::{ChatContainer, ChatContainerSnapshot, ChatRepository},
    },
    domain::chat::*,
};
use eliteforge_agent_core as fw;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

mod container;

pub struct SqliteChatRepository {
    host: FrameworkHost,
}

/// One round trip builds the whole session projection: business link, native
/// session record, latest non-summary run, and the external runtime reference.
/// Child runs share ownership, but must not replace the owning chat's run.
/// The correlated subqueries use the ef_runs_session index and
/// ef_session_context primary key.
const SESSION_PROJECTION_SQL: &str = "SELECT l.session_id, l.business_json, \
    json_extract(s.record,'$.title'), json_extract(s.record,'$.status'), \
    json_extract(s.record,'$.created_at_ms'), json_extract(s.record,'$.updated_at_ms'), \
    (SELECT r.record FROM ef_runs r WHERE r.session_id=l.session_id \
        AND json_extract(r.record,'$.kind')<>'SUMMARY' \
        ORDER BY json_extract(r.record,'$.started_at_ms') DESC, r.id DESC LIMIT 1), \
    (SELECT c.value FROM ef_session_context c WHERE c.session_id=l.session_id AND c.key='tron.external-reference') \
    FROM workspace_session_links l JOIN ef_sessions s ON s.id=l.session_id";

/// Columns of SESSION_PROJECTION_SQL before the ChatSession mapping.
struct SessionProjection {
    session_id: String,
    link_json: String,
    title: String,
    status: String,
    created_at_ms: i64,
    updated_at_ms: i64,
    last_run_json: Option<String>,
    external_reference: Option<String>,
}
fn session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionProjection> {
    Ok(SessionProjection {
        session_id: row.get(0)?,
        link_json: row.get(1)?,
        title: row.get(2)?,
        status: row.get(3)?,
        created_at_ms: row.get(4)?,
        updated_at_ms: row.get(5)?,
        last_run_json: row.get(6)?,
        external_reference: row.get(7)?,
    })
}
fn chat_session_from_row(row: SessionProjection) -> Result<ChatSession, ChatError> {
    let link: Link = serde_json::from_str(&row.link_json).map_err(|_| ChatError::StorageFailure)?;
    let status = match row.status.as_str() {
        "Active" | "ACTIVE" => ChatSessionStatus::Active,
        "Archived" | "ARCHIVED" => ChatSessionStatus::Archived,
        _ => return Err(ChatError::StorageFailure),
    };
    let last_run = row
        .last_run_json
        .as_deref()
        .map(serde_json::from_str::<fw::ManagedRun>)
        .transpose()
        .map_err(|_| ChatError::StorageFailure)?;
    let last = last_run.as_ref();
    let runtime = row
        .external_reference
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value["sessionId"].as_str().map(str::to_owned));
    Ok(ChatSession {
        execution_mode: link.execution_mode,
        id: link.id,
        session_code: row.session_id,
        workspace_code: link.workspace_code,
        project_code: link.project_code,
        directory_code: link.directory_code,
        working_directory_code: link.working_directory_code,
        user_code: link.user_code,
        app_code: link.app_code,
        task_code: link.task_code,
        source_type: link.source_type,
        source_code: link.source_code,
        agent_code: link.agent_code,
        system_agent_session_id: runtime,
        title: row.title,
        status,
        last_run_code: last.map(|r| r.id.to_string()),
        last_run_status: last.map(|r| {
            match r.status {
                fw::ManagedRunStatus::Completed => "COMPLETED",
                fw::ManagedRunStatus::AwaitingInput => "AWAITING_USER_INPUT",
                fw::ManagedRunStatus::Running => "RUNNING",
                fw::ManagedRunStatus::Cancelled => "STOPPED",
                _ => "FAILED",
            }
            .into()
        }),
        created_at: crate::shared::time::timestamp_from_millis(row.created_at_ms),
        updated_at: crate::shared::time::timestamp_from_millis(row.updated_at_ms),
    })
}

#[derive(Clone, Serialize, Deserialize)]
struct Link {
    #[serde(default)]
    execution_mode: crate::domain::agent::ToolExecutionMode,
    id: i64,
    workspace_code: String,
    project_code: Option<String>,
    #[serde(default)]
    directory_code: Option<String>,
    #[serde(default)]
    working_directory_code: Option<String>,
    user_code: Option<String>,
    app_code: Option<String>,
    task_code: Option<String>,
    source_type: ChatSessionSourceType,
    source_code: Option<String>,
    agent_code: Option<String>,
}
impl Link {
    fn from_session(s: &ChatSession) -> Self {
        Self {
            execution_mode: s.execution_mode,
            id: s.id,
            workspace_code: s.workspace_code.clone(),
            project_code: s.project_code.clone(),
            directory_code: s.directory_code.clone(),
            working_directory_code: s.working_directory_code.clone(),
            user_code: s.user_code.clone(),
            app_code: s.app_code.clone(),
            task_code: s.task_code.clone(),
            source_type: s.source_type,
            source_code: s.source_code.clone(),
            agent_code: s.agent_code.clone(),
        }
    }
}
impl SqliteChatRepository {
    pub fn connect(database_url: &str) -> Result<Self, ChatError> {
        let host = if database_url == ":memory:" {
            FrameworkHost::temporary(None)
        } else {
            Ok(FrameworkHost::persistent(database_url, None))
        }
        .map_err(|_| ChatError::StorageFailure)?;
        Self::with_host(host)
    }
    pub fn with_host(host: FrameworkHost) -> Result<Self, ChatError> {
        blocking(async {
            host.storage_framework()
                .await
                .map_err(|_| ChatError::StorageFailure)?;
            host.shared_database()
                .await
                .map_err(chat_error)?
                .call(|c| super::schema::initialize_business(c))
                .await
                .map_err(|_| ChatError::StorageFailure)?;
            Ok(Self { host })
        })
    }
    fn delete_session_internal(
        &mut self,
        id: &str,
        heartbeat_retention: bool,
    ) -> Result<(), ChatError> {
        blocking(self.delete_session_records(id, heartbeat_retention))
    }
    async fn delete_session_records(
        &self,
        id: &str,
        heartbeat_retention: bool,
    ) -> Result<(), ChatError> {
        let started = std::time::Instant::now();
        let link = self.link(id).await?;
        let user_code = link.user_code.clone();
        let db = self.host.shared_database().await.map_err(chat_error)?;
        let sessions = self
            .framework()
            .await?
            .sessions()
            .map_err(|_| ChatError::StorageFailure)?;
        let code = id.to_owned();
        // Staged before the business rows disappear so a failed rename still
        // rolls the whole deletion back; the slow content removal runs after
        // commit, off the request path and outside the write transaction.
        let mut staged = None;
        let result = db.transaction(async {
                let record=sessions.get(id).await?;
                if heartbeat_retention && record.status!=fw::SessionStatus::Archived {return Err(fw::StateError::Invalid("session must be archived".into()));}
                if self.framework().await.map_err(|e|fw::StateError::Backend(e.to_string()))?.goals().map_err(|e|fw::StateError::Backend(e.to_string()))?.active(id).await?.is_some_and(|goal| goal.status == fw::GoalStatus::Active) {
                    return Err(fw::StateError::Busy);
                }
                let guard_code = code.clone();
                let blocked = db.call(move |c| c.query_row(
                    "SELECT EXISTS(SELECT 1 FROM assistant_sessions s WHERE s.session_code=?1 AND s.purpose='Heartbeat' AND (
                        EXISTS(SELECT 1 FROM assistant_events e WHERE (e.session_code=s.session_code OR e.event_id=s.correlation_key) AND (e.source!='Heartbeat' OR e.status IN ('Queued','Running','AwaitingInput')))
                        OR EXISTS(SELECT 1 FROM tasks t WHERE t.work_session_code=s.session_code)
                        OR EXISTS(SELECT 1 FROM collaboration_threads t WHERE t.session_id=s.session_code)
                        OR EXISTS(SELECT 1 FROM assistant_work_changes w WHERE json_extract(w.record_json,'$.sessionCode')=s.session_code AND json_extract(w.record_json,'$.resolvedAt') IS NULL)))",
                    [&guard_code], |r| r.get::<_, bool>(0),
                )).await.map_err(|e|fw::StateError::Backend(e.to_string()))?;
                if blocked { return Err(fw::StateError::Busy); }
                if heartbeat_retention {
                    let guard_code = code.clone();
                    let expired = db.call(move |c| c.query_row(
                        "SELECT EXISTS(SELECT 1 FROM assistant_sessions s WHERE s.session_code=?1 AND s.purpose='Heartbeat' AND EXISTS(SELECT 1 FROM assistant_sessions n WHERE n.assistant_code=s.assistant_code AND n.purpose='Heartbeat' AND n.rowid>s.rowid LIMIT 1 OFFSET 19))",
                        [&guard_code], |r| r.get::<_, bool>(0),
                    )).await.map_err(|e|fw::StateError::Backend(e.to_string()))?;
                    if !expired { return Err(fw::StateError::Busy); }
                }
                sessions.delete(id).await?;
                db.call(move|c| {
                    // Historical traces, logs and statistics have their own retention
                    // policy and are independent of the chat session lifecycle.
                    c.execute("DELETE FROM assistant_events WHERE source='Heartbeat' AND EXISTS(SELECT 1 FROM assistant_sessions s WHERE s.session_code=?1 AND s.purpose='Heartbeat' AND (assistant_events.session_code=s.session_code OR assistant_events.event_id=s.correlation_key))",[&code])?;
                    c.execute("DELETE FROM assistant_sessions WHERE session_code=?1 AND purpose='Heartbeat'",[&code])?;
                    c.execute("DELETE FROM resource_chat_session_attachments WHERE session_code=?1",[&code])?;
                    for (table, sql) in [
                        ("chat_session_auto_model_routes", "DELETE FROM chat_session_auto_model_routes WHERE session_code=?1"),
                        ("provider_model_run_snapshots", "DELETE FROM provider_model_run_snapshots WHERE session_code=?1"),
                        ("automation_execution_records", "UPDATE automation_execution_records SET session_code=NULL WHERE session_code=?1"),
                        ("task_execution_records", "UPDATE task_execution_records SET session_code=NULL WHERE session_code=?1"),
                    ] {
                        if c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",[table],|r|r.get::<_,bool>(0))? { c.execute(sql,[&code])?; }
                    }
                    c.execute("DELETE FROM workspace_session_links WHERE session_id=?1",[&code])?;
                    Ok::<_,rusqlite::Error>(())
                }).await.map_err(|e|fw::StateError::Backend(e.to_string()))?;
                staged = Some(
                    self.host
                        .stage_session_working_directories(vec![(id.to_owned(), user_code.clone())])
                        .await
                        .map_err(fw::StateError::Backend)?,
                );
                Ok(())
            }).await.map_err(chat_error);
        if result.is_ok()
            && let Some(staged) = staged.take()
        {
            staged.commit_detached();
        }
        tracing::debug!(
            session_code = id,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "chat session rows deleted"
        );
        result
    }
    pub(crate) async fn ensure_run_session(
        host: &FrameworkHost,
        context: &crate::domain::agent::AgentRunContext,
    ) -> Result<(), ChatError> {
        host.shared_database()
            .await
            .map_err(chat_error)?
            .call(|c| super::schema::initialize_business(c))
            .await
            .map_err(|_| ChatError::StorageFailure)?;
        let repository = Self { host: host.clone() };
        let id = context
            .conversation_code
            .as_deref()
            .unwrap_or(&context.run_code);
        match repository.session(id).await {
            Ok(session) => {
                if session.workspace_code != context.workspace_code {
                    return Err(ChatError::SessionNotFound);
                }
                if session.status != ChatSessionStatus::Active {
                    return Err(ChatError::SessionArchived);
                }
                return Ok(());
            }
            // A missing conversation is materialized lazily: explicit
            // conversation codes and run-scoped temp sessions share the same
            // creation path, bound to the already validated run workspace.
            Err(ChatError::SessionNotFound) => {}
            Err(error) => return Err(error),
        }
        let mut record = fw::SessionRecord::new("临时运行");
        record.id = id.into();
        let link = Link {
            execution_mode: context.tool_execution_mode,
            id: 0,
            workspace_code: context.workspace_code.clone(),
            project_code: None,
            directory_code: None,
            working_directory_code: None,
            user_code: context.user_code.clone(),
            app_code: context.execution_scope.app_code.clone(),
            task_code: context.execution_scope.task_code.clone(),
            source_type: ChatSessionSourceType::Manual,
            source_code: None,
            agent_code: context
                .short_term_memory
                .as_ref()
                .and_then(|m| m.agent_code.clone()),
        };
        let value = serde_json::to_string(&link).map_err(|_| ChatError::StorageFailure)?;
        let id = id.to_owned();
        let db = host.shared_database().await.map_err(chat_error)?;
        let sessions = repository
            .framework()
            .await?
            .sessions()
            .map_err(|_| ChatError::StorageFailure)?;
        db.transaction(async {
            sessions.create(record).await?;
            db.call(move |c| {
                c.execute(
                    "INSERT INTO workspace_session_links VALUES (?1,?2,?3)",
                    params![id, link.workspace_code, value],
                )
            })
            .await
            .map_err(|e| fw::StateError::Backend(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(chat_error)
    }
    async fn framework(&self) -> Result<eliteforge_agent_framework::Framework, ChatError> {
        self.host
            .storage_framework()
            .await
            .map_err(|_| ChatError::StorageFailure)
    }
    async fn link(&self, id: &str) -> Result<Link, ChatError> {
        let id = id.to_owned();
        let json: Option<String> = self
            .host
            .shared_database()
            .await
            .map_err(chat_error)?
            .call(move |c| {
                c.query_row(
                    "SELECT business_json FROM workspace_session_links WHERE session_id=?1",
                    [id],
                    |r| r.get(0),
                )
                .optional()
            })
            .await
            .map_err(|_| ChatError::StorageFailure)?;
        serde_json::from_str(&json.ok_or(ChatError::SessionNotFound)?)
            .map_err(|_| ChatError::StorageFailure)
    }
    async fn session(&self, id: &str) -> Result<ChatSession, ChatError> {
        // Building the framework lazily creates the ef_* tables this
        // projection reads; the first call initializes shared storage.
        self.framework().await?;
        let id = id.to_owned();
        let raw = self
            .host
            .shared_database()
            .await
            .map_err(chat_error)?
            .call_read(move |c| -> Result<Vec<SessionProjection>, ChatError> {
                let mut statement = c
                    .prepare(&format!("{SESSION_PROJECTION_SQL} WHERE l.session_id=?1"))
                    .map_err(|_| ChatError::StorageFailure)?;
                statement
                    .query_map(params![id], session_row)
                    .map_err(|_| ChatError::StorageFailure)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| ChatError::StorageFailure)
            })
            .await
            .map_err(|_| ChatError::StorageFailure)?;
        chat_session_from_row(raw.into_iter().next().ok_or(ChatError::SessionNotFound)?)
    }
    async fn events(&self, session: &str, kind: &str) -> Result<Vec<fw::SessionEvent>, ChatError> {
        self.link(session).await?;
        let store = self
            .framework()
            .await?
            .events()
            .map_err(|_| ChatError::StorageFailure)?;
        let mut result = Vec::new();
        let mut after = 0;
        loop {
            let page = store
                .list_by_kind(session, kind, after, 1000)
                .await
                .map_err(chat_error)?;
            if page.is_empty() {
                break;
            }
            after = page.last().unwrap().sequence;
            result.extend(page);
        }
        let deleted = self
            .framework()
            .await?
            .history()
            .map_err(|_| ChatError::StorageFailure)?
            .deleted_runs(session)
            .await
            .map_err(chat_error)?
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        result.retain(|event| event.run_id.is_none_or(|run| !deleted.contains(&run)));
        Ok(result)
    }
    async fn put_context(&self, id: &str, key: &str, value: Value) -> Result<(), ChatError> {
        let s = self
            .framework()
            .await?
            .sessions()
            .map_err(|_| ChatError::StorageFailure)?;
        let expected = s
            .read_context(id, key)
            .await
            .map_err(chat_error)?
            .map(|(revision, _)| revision);
        s.write_context(id, key, expected, value)
            .await
            .map_err(chat_error)?;
        Ok(())
    }
}
impl ChatRepository for SqliteChatRepository {
    fn validate_annotation_sources(
        &mut self,
        session: &str,
        annotations: &[fw::MessageAnnotation],
    ) -> Result<(), ChatError> {
        if annotations.is_empty() {
            return Ok(());
        }
        blocking(async {
            self.framework()
                .await?
                .history()
                .map_err(|_| ChatError::StorageFailure)?
                .validate_annotation_sources(session, annotations)
                .await
                .map_err(|error| match error {
                    fw::StateError::Invalid(_) => ChatError::AnnotationsInvalid,
                    other => chat_error(other),
                })
        })
    }

    fn session_titles(
        &mut self,
        codes: &[String],
    ) -> Result<Vec<crate::application::chat::ChatSessionTitle>, ChatError> {
        if codes.is_empty() {
            return Ok(Vec::new());
        }
        let codes = serde_json::to_string(codes).map_err(|_| ChatError::StorageFailure)?;
        blocking(async {
            self.host.shared_database().await.map_err(chat_error)?.call_read(move |c| {
                let mut statement = c.prepare(&format!("SELECT l.session_id,l.workspace_code,json_extract(s.record,'$.title') FROM workspace_session_links l JOIN ef_sessions s ON s.id=l.session_id WHERE l.session_id IN (SELECT value FROM json_each(?1))"))?;
                statement.query_map([codes],|row| Ok(crate::application::chat::ChatSessionTitle {
                    session_code:row.get(0)?,workspace_code:row.get(1)?,title:row.get(2)?,
                }))?.collect::<rusqlite::Result<Vec<_>>>()
            }).await.map_err(|_| ChatError::StorageFailure)
        })
    }
    fn sessions_by_codes(&mut self, codes: &[String]) -> Result<Vec<ChatSession>, ChatError> {
        if codes.is_empty() {
            return Ok(Vec::new());
        }
        let codes = serde_json::to_string(codes).map_err(|_| ChatError::StorageFailure)?;
        blocking(async {
            let rows = self.host.shared_database().await.map_err(chat_error)?.call_read(move |c| {
                let mut q = c.prepare(&format!("{SESSION_PROJECTION_SQL} WHERE l.session_id IN (SELECT value FROM json_each(?1))"))?;
                q.query_map([codes], session_row)?.collect::<rusqlite::Result<Vec<_>>>()
            }).await.map_err(|_| ChatError::StorageFailure)?;
            rows.into_iter().map(chat_session_from_row).collect()
        })
    }
    fn pending_input_counts(
        &mut self,
        codes: &[String],
    ) -> Result<std::collections::BTreeMap<String, u32>, ChatError> {
        if codes.is_empty() {
            return Ok(Default::default());
        }
        let codes = serde_json::to_string(codes).map_err(|_| ChatError::StorageFailure)?;
        blocking(async {
            self.host.shared_database().await.map_err(chat_error)?.call_read(move |c| {
                let mut q = c.prepare("WITH requests AS (SELECT e.session_id,json_extract(e.record,'$.payload.requestCode') AS request_code FROM ef_events e WHERE e.session_id IN (SELECT value FROM json_each(?1)) AND e.kind='tron.interaction-view' GROUP BY e.session_id,request_code) SELECT requests.session_id,SUM(CASE WHEN i.answered=0 THEN 1 ELSE 0 END),SUM(i.id IS NULL) FROM requests LEFT JOIN ef_interactions i ON i.session_id=requests.session_id AND i.id=requests.request_code GROUP BY requests.session_id")?;
                q.query_map([codes], |r| Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,u32>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()
            }).await.map_err(|_| ChatError::StorageFailure)?.into_iter().map(|(session,pending,missing)| {
                if missing != 0 { Err(ChatError::StorageFailure) } else { Ok((session,pending)) }
            }).collect()
        })
    }
    fn create_session(&mut self, session: &ChatSession) -> Result<(), ChatError> {
        let mut record = fw::SessionRecord::new(&session.title);
        record.id = session.session_code.clone();
        let link = Link::from_session(session);
        let id = record.id.clone();
        let value = serde_json::to_string(&link).map_err(|_| ChatError::StorageFailure)?;
        blocking(async {
            let db = self.host.shared_database().await.map_err(chat_error)?;
            let sessions = self
                .framework()
                .await?
                .sessions()
                .map_err(|_| ChatError::StorageFailure)?;
            db.transaction(async {
                sessions.create(record).await?;
                db.call(move |c| -> rusqlite::Result<()> {
                    if let Some(directory) = link.directory_code.as_deref() {
                        let exists: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM directories WHERE directory_code=?1 AND workspace_code=?2 AND archived=0)", params![directory, link.workspace_code], |row| row.get(0))?;
                        if !exists { return Err(rusqlite::Error::InvalidQuery); }
                    }
                    if let Some(directory) = link.working_directory_code.as_deref() {
                        let exists: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM directories WHERE directory_code=?1 AND workspace_code=?2 AND archived=0)", params![directory, link.workspace_code], |row| row.get(0))?;
                        if !exists { return Err(rusqlite::Error::InvalidQuery); }
                        if let Some(project) = link.project_code.as_deref() {
                            let exists: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM projects WHERE project_code=?1 AND workspace_code=?2 AND status='ACTIVE')", params![project, link.workspace_code], |row| row.get(0))?;
                            if !exists { return Err(rusqlite::Error::InvalidQuery); }
                            c.execute("INSERT INTO topic_memos(project_code,notes,revision) VALUES (?1,'',0) ON CONFLICT(project_code) DO NOTHING", [project])?;
                            let reference = format!("TOPIC-REF-{id}");
                            let inserted = c.execute("INSERT INTO topic_references(reference_code,project_code,kind,target_code,connection_code,label,url) SELECT ?1,?2,'Directory',directory_code,'',display_name,NULL FROM directories WHERE directory_code=?3 ON CONFLICT(project_code,kind,target_code,connection_code) DO NOTHING", params![reference, project, directory])?;
                            if inserted != 0 { c.execute("UPDATE topic_memos SET revision=revision+1 WHERE project_code=?1", [project])?; }
                        }
                    }
                    c.execute("INSERT INTO workspace_session_links(session_id,workspace_code,business_json) VALUES(?1,?2,?3)", params![id,link.workspace_code,value])?;
                    Ok(())
                }).await.map_err(|e|fw::StateError::Backend(e.to_string()))?; Ok(())
            }).await.map_err(chat_error)
        })
    }
    fn delete_session(&mut self, id: &str) -> Result<(), ChatError> {
        self.delete_session_internal(id, false)
    }
    fn delete_expired_heartbeat_session(&mut self, id: &str) -> Result<(), ChatError> {
        self.delete_session_internal(id, true)
    }
    fn list_sessions(&mut self) -> Result<Vec<ChatSession>, ChatError> {
        blocking(async {
            self.framework().await?;
            let db = self.host.shared_database().await.map_err(chat_error)?;
            let raw = db
                .call_read(|c| -> Result<Vec<SessionProjection>, ChatError> {
                    let mut statement = c
                        .prepare(&format!("{SESSION_PROJECTION_SQL} ORDER BY l.session_id"))
                        .map_err(|_| ChatError::StorageFailure)?;
                    statement
                        .query_map([], session_row)
                        .map_err(|_| ChatError::StorageFailure)?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| ChatError::StorageFailure)
                })
                .await
                .map_err(|_| ChatError::StorageFailure)?;
            let mut result = raw
                .into_iter()
                .map(chat_session_from_row)
                .collect::<Result<Vec<_>, _>>()?;
            result.sort_by(|a, b| {
                b.updated_at
                    .cmp(&a.updated_at)
                    .then_with(|| b.session_code.cmp(&a.session_code))
            });
            Ok(result)
        })
    }
    fn session_codes_by_workspaces(
        &mut self,
        workspace_codes: &[String],
    ) -> Result<Vec<String>, ChatError> {
        if workspace_codes.is_empty() {
            return Ok(Vec::new());
        }
        let codes =
            serde_json::to_string(workspace_codes).map_err(|_| ChatError::StorageFailure)?;
        blocking(async {
            let db = self.host.shared_database().await.map_err(chat_error)?;
            db.call_read(move |c| -> Result<Vec<String>, ChatError> {
                let mut statement = c
                    .prepare(
                        "SELECT session_id FROM workspace_session_links
                         WHERE workspace_code IN (SELECT value FROM json_each(?1))
                         ORDER BY session_id",
                    )
                    .map_err(|_| ChatError::StorageFailure)?;
                statement
                    .query_map([&codes], |row| row.get::<_, String>(0))
                    .map_err(|_| ChatError::StorageFailure)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| ChatError::StorageFailure)
            })
            .await
            .map_err(|_| ChatError::StorageFailure)
        })
    }

    /// Management listing keeps filtering, counting and paging in SQL so the
    /// settings page scales across every workspace at once.
    fn list_managed_sessions(
        &mut self,
        workspace_code: Option<&str>,
        status: Option<ChatSessionStatus>,
        source_type: Option<ChatSessionSourceType>,
        search: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<ChatSession>, u64), ChatError> {
        blocking(async {
            self.framework().await?;
            let db = self.host.shared_database().await.map_err(chat_error)?;
            let workspace = workspace_code.map(str::to_owned);
            let search = search.map(str::to_owned);
            let status = status.map(|status| match status {
                ChatSessionStatus::Active => "ACTIVE".to_string(),
                ChatSessionStatus::Archived => "ARCHIVED".to_string(),
            });
            let source = source_type.map(|source| match source {
                ChatSessionSourceType::Manual => "MANUAL".to_string(),
                ChatSessionSourceType::Task => "TASK".to_string(),
                ChatSessionSourceType::Automation => "AUTOMATION".to_string(),
                ChatSessionSourceType::Terminal => "TERMINAL".to_string(),
            });
            let filter = "WHERE (?1 IS NULL OR l.workspace_code=?1) \
                AND (?2 IS NULL OR upper(json_extract(s.record,'$.status'))=?2) \
                AND (?3 IS NULL OR upper(json_extract(l.business_json,'$.source_type'))=?3) \
                AND (?4 IS NULL OR json_extract(s.record,'$.title') LIKE '%'||?4||'%' COLLATE NOCASE) \
                AND substr(coalesce(json_extract(l.business_json,'$.source_code'),''),1,12) <> '__assistant:' \
                AND substr(coalesce(json_extract(l.business_json,'$.source_code'),''),1,10) <> '__knowledge:'";
            let rows = db
                .call_read(move |c| -> Result<(Vec<SessionProjection>, u64), ChatError> {
                    let total: u64 = c
                        .query_row(
                            &format!(
                                "SELECT count(*) FROM workspace_session_links l \
                                 JOIN ef_sessions s ON s.id=l.session_id {filter}"
                            ),
                            params![workspace, status, source, search],
                            |r| r.get(0),
                        )
                        .map_err(|_| ChatError::StorageFailure)?;
                    let mut statement = c
                        .prepare(&format!("{SESSION_PROJECTION_SQL} {filter} \
                            ORDER BY json_extract(s.record,'$.updated_at_ms') DESC, l.session_id DESC \
                            LIMIT ?5 OFFSET ?6"))
                        .map_err(|_| ChatError::StorageFailure)?;
                    let rows = statement
                        .query_map(
                            params![
                                workspace,
                                status,
                                source,
                                search,
                                i64::from(limit),
                                i64::from(offset)
                            ],
                            session_row,
                        )
                        .map_err(|_| ChatError::StorageFailure)?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| ChatError::StorageFailure)?;
                    Ok((rows, total))
                })
                .await
                .map_err(|_| ChatError::StorageFailure)?;
            let items = rows
                .0
                .into_iter()
                .map(chat_session_from_row)
                .collect::<Result<Vec<_>, _>>()?;
            Ok((items, rows.1))
        })
    }
    fn get_session(&mut self, id: &str) -> Result<ChatSession, ChatError> {
        blocking(self.session(id))
    }
    fn find_source_session(
        &mut self,
        user: &str,
        source: &str,
    ) -> Result<Option<ChatSession>, ChatError> {
        let user = user.to_owned();
        let source = source.to_owned();
        blocking(async {
            self.framework().await?;
            let database = self.host.shared_database().await.map_err(chat_error)?;
            let row=database.call_read(move|connection|->Result<Option<SessionProjection>,ChatError>{
                connection.query_row(&format!("{SESSION_PROJECTION_SQL} WHERE json_extract(l.business_json,'$.user_code')=?1 AND json_extract(l.business_json,'$.source_code')=?2 AND json_extract(s.record,'$.status') IN ('Active','ACTIVE') LIMIT 1"),params![user,source],session_row).optional().map_err(|_|ChatError::StorageFailure)
            }).await.map_err(|_|ChatError::StorageFailure)?;
            row.map(chat_session_from_row).transpose()
        })
    }
    fn update_session_status(
        &mut self,
        id: &str,
        status: ChatSessionStatus,
        _at: &str,
    ) -> Result<(), ChatError> {
        blocking(async {
            let link = self.link(id).await?;
            let f = self.framework().await?;
            let s = f.sessions().map_err(|_| ChatError::StorageFailure)?;
            let db = self.host.shared_database().await.map_err(chat_error)?;
            let mut topic_name_conflict = false;
            db.transaction(async {
                if status == ChatSessionStatus::Archived {
                    if f.goals()
                        .map_err(|e| fw::StateError::Backend(e.to_string()))?
                        .active(id)
                        .await?
                        .is_some_and(|goal| goal.status == fw::GoalStatus::Active)
                    {
                        return Err(fw::StateError::Busy);
                    }
                    s.archive(id).await?;
                } else {
                    if let Some(project_code) = link.project_code {
                        let workspace_code = link.workspace_code.clone();
                        let changed = db.call(move |c| -> rusqlite::Result<Option<usize>> {
                            let name: Option<String> = c.query_row("SELECT name FROM projects WHERE project_code=?1 AND workspace_code=?2", params![project_code, workspace_code], |row| row.get(0)).optional()?;
                            let Some(name) = name else { return Ok(Some(0)); };
                            if super::project_repository::active_name_exists(c, &workspace_code, &name, &project_code)? {
                                return Ok(None);
                            }
                            c.execute("UPDATE projects SET status='ACTIVE',archived_at=NULL WHERE project_code=?1 AND workspace_code=?2", params![project_code, workspace_code]).map(Some)
                        }).await.map_err(|error| fw::StateError::Backend(error.to_string()))?;
                        match changed {
                            None => { topic_name_conflict = true; return Err(fw::StateError::Conflict); }
                            Some(0) => return Err(fw::StateError::NotFound),
                            Some(_) => {}
                        }
                    }
                    if let Some(directory_code) = link.directory_code {
                        let workspace_code = link.workspace_code;
                        let changed = db.call(move |c| c.execute("UPDATE directories SET archived=0 WHERE directory_code=?1 AND workspace_code=?2", params![directory_code, workspace_code]))
                            .await.map_err(|error| fw::StateError::Backend(error.to_string()))?;
                        if changed == 0 { return Err(fw::StateError::NotFound); }
                    }
                    s.restore(id).await?;
                }
                Ok(())
            })
            .await
            .map_err(|error| if topic_name_conflict { ChatError::ProjectNameConflict } else { chat_error(error) })?;
            Ok(())
        })
    }
    fn update_session_settings(
        &mut self,
        id: &str,
        title: Option<&str>,
        execution_mode: Option<crate::domain::agent::ToolExecutionMode>,
        _at: &str,
    ) -> Result<(), ChatError> {
        blocking(async {
            let db = self.host.shared_database().await.map_err(chat_error)?;
            let sessions = self
                .framework()
                .await?
                .sessions()
                .map_err(|_| ChatError::StorageFailure)?;
            db.transaction(async {
                let mut link = self.link(id).await
                    .map_err(|error| fw::StateError::Backend(error.to_string()))?;
                if let Some(mode) = execution_mode {
                    link.execution_mode = mode;
                    let value = serde_json::to_string(&link)
                        .map_err(|error| fw::StateError::Backend(error.to_string()))?;
                    let code = id.to_owned();
                    db.call(move |connection| {
                        connection.execute(
                            "UPDATE workspace_session_links SET business_json=?2 WHERE session_id=?1",
                            params![code, value],
                        )
                    }).await.map_err(|error| fw::StateError::Backend(error.to_string()))?;
                }
                let mut record = sessions.get(id).await?;
                let expected = record.revision;
                if let Some(title) = title {
                    record.title = title.into();
                }
                sessions.update(record, expected).await?;
                Ok(())
            }).await.map_err(chat_error)
        })
    }
    fn update_session_location(
        &mut self,
        id: &str,
        project_code: Option<&str>,
        directory_code: Option<&str>,
        working_directory_code: Option<&str>,
        reference_code: &str,
    ) -> Result<(), ChatError> {
        let id = id.to_owned();
        let project = project_code.map(str::to_owned);
        let placement_directory = directory_code.map(str::to_owned);
        let working = working_directory_code.map(str::to_owned);
        let reference = reference_code.to_owned();
        blocking(async {
            let db = self.host.shared_database().await.map_err(chat_error)?;
            db.call(move |connection| {
                let result = (|| -> Result<(), ChatError> {
                    let tx = super::framework_storage::business_transaction(connection, rusqlite::TransactionBehavior::Immediate)
                        .map_err(|_| ChatError::StorageFailure)?;
                    let (link_json, status): (String, String) = tx.query_row(
                        "SELECT l.business_json,json_extract(s.record,'$.status') FROM workspace_session_links l JOIN ef_sessions s ON s.id=l.session_id WHERE l.session_id=?1",
                        [&id], |row| Ok((row.get(0)?, row.get(1)?)),
                    ).optional().map_err(|_| ChatError::StorageFailure)?.ok_or(ChatError::SessionNotFound)?;
                    if !matches!(status.as_str(), "Active" | "ACTIVE") { return Err(ChatError::SessionArchived); }
                    let mut link: Link = serde_json::from_str(&link_json).map_err(|_| ChatError::StorageFailure)?;
                    if let Some(code) = project.as_deref() {
                        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM projects WHERE project_code=?1 AND workspace_code=?2 AND status='ACTIVE')", params![code, link.workspace_code], |row| row.get(0)).map_err(|_| ChatError::StorageFailure)?;
                        if !exists { return Err(ChatError::ProjectInvalid); }
                    }
                    for code in [placement_directory.as_deref(), working.as_deref()].into_iter().flatten() {
                        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM directories WHERE directory_code=?1 AND workspace_code=?2 AND archived=0)", params![code, link.workspace_code], |row| row.get(0)).map_err(|_| ChatError::StorageFailure)?;
                        if !exists { return Err(ChatError::DirectoryInvalid); }
                    }
                    if let (Some(project), Some(directory)) = (project.as_deref(), working.as_deref()) {
                        tx.execute("INSERT INTO topic_memos(project_code,notes,revision) VALUES (?1,'',0) ON CONFLICT(project_code) DO NOTHING", [project]).map_err(|_| ChatError::StorageFailure)?;
                        let inserted = tx.execute("INSERT INTO topic_references(reference_code,project_code,kind,target_code,connection_code,label,url) SELECT ?1,?2,'Directory',directory_code,'',display_name,NULL FROM directories WHERE directory_code=?3 ON CONFLICT(project_code,kind,target_code,connection_code) DO NOTHING", params![reference, project, directory]).map_err(|_| ChatError::StorageFailure)?;
                        if inserted != 0 { tx.execute("UPDATE topic_memos SET revision=revision+1 WHERE project_code=?1", [project]).map_err(|_| ChatError::StorageFailure)?; }
                    }
                    if link.project_code != project { link.app_code = None; }
                    link.project_code = project;
                    link.directory_code = placement_directory;
                    link.working_directory_code = working;
                    let value = serde_json::to_string(&link).map_err(|_| ChatError::StorageFailure)?;
                    tx.execute("UPDATE workspace_session_links SET business_json=?2 WHERE session_id=?1", params![id, value]).map_err(|_| ChatError::StorageFailure)?;
                    tx.commit().map_err(|_| ChatError::StorageFailure)
                })();
                Ok::<_, rusqlite::Error>(result)
            }).await.map_err(|_| ChatError::StorageFailure)?
        })
    }
    fn container_snapshot(
        &mut self,
        container: &ChatContainer,
    ) -> Result<ChatContainerSnapshot, ChatError> {
        blocking(self.read_container_snapshot(container))
    }
    fn update_managed_session_location(
        &mut self,
        id: &str,
        target_workspace: &str,
        project_code: Option<&str>,
        working_directory_code: Option<&str>,
        reference_code: &str,
    ) -> Result<(), ChatError> {
        let id = id.to_owned();
        let target_workspace = target_workspace.to_owned();
        let project = project_code.map(str::to_owned);
        let working = working_directory_code.map(str::to_owned);
        let reference = reference_code.to_owned();
        blocking(async {
            let db = self.host.shared_database().await.map_err(chat_error)?;
            db.call(move |connection| {
                let result = (|| -> Result<(), ChatError> {
                    let tx = super::framework_storage::business_transaction(connection, rusqlite::TransactionBehavior::Immediate)
                        .map_err(|_| ChatError::StorageFailure)?;
                    let (link_json, status): (String, String) = tx.query_row(
                        "SELECT l.business_json,json_extract(s.record,'$.status') FROM workspace_session_links l JOIN ef_sessions s ON s.id=l.session_id WHERE l.session_id=?1",
                        [&id], |row| Ok((row.get(0)?, row.get(1)?)),
                    ).optional().map_err(|_| ChatError::StorageFailure)?.ok_or(ChatError::SessionNotFound)?;
                    if !matches!(status.as_str(), "Active" | "ACTIVE") { return Err(ChatError::SessionArchived); }
                    let mut link: Link = serde_json::from_str(&link_json).map_err(|_| ChatError::StorageFailure)?;
                    // The target workspace must exist and stay active.
                    let workspace_active: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE workspace_code=?1 AND status='active')",
                        [&target_workspace], |row| row.get(0),
                    ).map_err(|_| ChatError::StorageFailure)?;
                    if !workspace_active { return Err(ChatError::WorkspaceForbidden); }
                    if let Some(code) = project.as_deref() {
                        let exists: bool = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM projects WHERE project_code=?1 AND workspace_code=?2 AND status='ACTIVE')",
                            params![code, target_workspace], |row| row.get(0),
                        ).map_err(|_| ChatError::StorageFailure)?;
                        if !exists { return Err(ChatError::ProjectInvalid); }
                    }
                    if let Some(code) = working.as_deref() {
                        let exists: bool = tx.query_row(
                            "SELECT EXISTS(SELECT 1 FROM directories WHERE directory_code=?1 AND workspace_code=?2 AND archived=0)",
                            params![code, target_workspace], |row| row.get(0),
                        ).map_err(|_| ChatError::StorageFailure)?;
                        if !exists { return Err(ChatError::DirectoryInvalid); }
                    }
                    if let (Some(project), Some(directory)) = (project.as_deref(), working.as_deref()) {
                        tx.execute("INSERT INTO topic_memos(project_code,notes,revision) VALUES (?1,'',0) ON CONFLICT(project_code) DO NOTHING", [project]).map_err(|_| ChatError::StorageFailure)?;
                        let inserted = tx.execute(
                            "INSERT INTO topic_references(reference_code,project_code,kind,target_code,connection_code,label,url) SELECT ?1,?2,'Directory',directory_code,'',display_name,NULL FROM directories WHERE directory_code=?3 ON CONFLICT(project_code,kind,target_code,connection_code) DO NOTHING",
                            params![reference, project, directory],
                        ).map_err(|_| ChatError::StorageFailure)?;
                        if inserted != 0 { tx.execute("UPDATE topic_memos SET revision=revision+1 WHERE project_code=?1", [project]).map_err(|_| ChatError::StorageFailure)?; }
                    }
                    if link.project_code != project || link.workspace_code != target_workspace {
                        link.app_code = None;
                    }
                    link.workspace_code = target_workspace.clone();
                    link.project_code = project;
                    link.directory_code = None;
                    link.working_directory_code = working;
                    let value = serde_json::to_string(&link).map_err(|_| ChatError::StorageFailure)?;
                    // Rewrite the link row so the workspace index follows the move.
                    tx.execute(
                        "UPDATE workspace_session_links SET workspace_code=?2, business_json=?3 WHERE session_id=?1",
                        params![id, target_workspace, value],
                    ).map_err(|_| ChatError::StorageFailure)?;
                    tx.commit().map_err(|_| ChatError::StorageFailure)
                })();
                Ok::<_, rusqlite::Error>(result)
            }).await.map_err(|_| ChatError::StorageFailure)?
        })
    }
    fn mutate_container(
        &mut self,
        container: &ChatContainer,
        expected: &ChatContainerSnapshot,
        delete: bool,
    ) -> Result<(), ChatError> {
        blocking(self.apply_container_mutation(container, expected, delete))
    }
    fn update_session_run(
        &mut self,
        id: &str,
        _run: &str,
        _status: &str,
        agent: Option<&str>,
        runtime: Option<&str>,
        _at: &str,
    ) -> Result<(), ChatError> {
        blocking(async {
            let db = self.host.shared_database().await.map_err(chat_error)?;
            let sessions = self
                .framework()
                .await?
                .sessions()
                .map_err(|_| ChatError::StorageFailure)?;
            db.transaction(async {
                let mut link = self
                    .link(id)
                    .await
                    .map_err(|e| fw::StateError::Backend(e.to_string()))?;
                if let Some(agent) = agent {
                    link.agent_code = Some(agent.into());
                }
                let value = serde_json::to_string(&link)
                    .map_err(|e| fw::StateError::Backend(e.to_string()))?;
                let code = id.to_owned();
                db.call(move |c| {
                    c.execute(
                        "UPDATE workspace_session_links SET business_json=?2 WHERE session_id=?1",
                        params![code, value],
                    )
                })
                .await
                .map_err(|e| fw::StateError::Backend(e.to_string()))?;
                if let Some(runtime) = runtime {
                    let expected = sessions
                        .read_context(id, "tron.external-reference")
                        .await?
                        .map(|(revision, _)| revision);
                    sessions
                        .write_context(
                            id,
                            "tron.external-reference",
                            expected,
                            json!({"sessionId":runtime}),
                        )
                        .await?;
                }
                Ok(())
            })
            .await
            .map_err(chat_error)
        })
    }

    fn runtime_session_id(
        &mut self,
        id: &str,
        adapter: &str,
        agent: &str,
    ) -> Result<Option<String>, ChatError> {
        blocking(async {
            self.link(id).await?;
            let key = format!("tron.external/{adapter}/{agent}");
            Ok(self
                .framework()
                .await?
                .sessions()
                .map_err(|_| ChatError::StorageFailure)?
                .read_context(id, &key)
                .await
                .map_err(chat_error)?
                .and_then(|(_, v)| v.as_str().map(str::to_owned)))
        })
    }
    fn upsert_runtime_session(
        &mut self,
        id: &str,
        adapter: &str,
        agent: &str,
        runtime: &str,
        _run: Option<&str>,
        _at: &str,
    ) -> Result<(), ChatError> {
        blocking(self.put_context(
            id,
            &format!("tron.external/{adapter}/{agent}"),
            json!(runtime),
        ))
    }
    fn create_message(&mut self, message: &ChatMessage) -> Result<(), ChatError> {
        blocking(async {
            self.link(&message.session_code).await?;
            let mut event = fw::SessionEvent::new(
                &message.session_code,
                "tron.message",
                serde_json::to_value(message).map_err(|_| ChatError::StorageFailure)?,
            );
            event.run_id = message
                .run_code
                .as_deref()
                .and_then(|id| fw::RunId::parse_str(id).ok());
            self.framework()
                .await?
                .sessions()
                .map_err(|_| ChatError::StorageFailure)?
                .store
                .append_event(event)
                .await
                .map_err(chat_error)?;
            Ok(())
        })
    }
    fn append_artifacts(
        &mut self,
        session_code: &str,
        run_code: &str,
        message_code: &str,
        records: &[crate::domain::chat::ArtifactRecord],
    ) -> Result<(), ChatError> {
        blocking(async {
            self.link(session_code).await?;
            let mut event = fw::SessionEvent::new(
                session_code,
                "tron.artifacts",
                json!({
                    "runCode": run_code,
                    "messageCode": message_code,
                    "records": records,
                }),
            );
            event.run_id = fw::RunId::parse_str(run_code).ok();
            self.framework()
                .await?
                .sessions()
                .map_err(|_| ChatError::StorageFailure)?
                .store
                .append_event(event)
                .await
                .map_err(chat_error)?;
            Ok(())
        })
    }

    fn artifacts_by_session(
        &mut self,
        session_code: &str,
    ) -> Result<
        std::collections::BTreeMap<String, Vec<crate::domain::chat::ArtifactRecord>>,
        ChatError,
    > {
        blocking(async {
            let mut by_run = std::collections::BTreeMap::new();
            for event in self.events(session_code, "tron.artifacts").await? {
                let Some(run_code) = event
                    .payload
                    .get("runCode")
                    .and_then(|value| value.as_str())
                else {
                    continue;
                };
                let Some(records) = event.payload.get("records") else {
                    continue;
                };
                let parsed: Vec<crate::domain::chat::ArtifactRecord> =
                    serde_json::from_value(records.clone())
                        .map_err(|_| ChatError::StorageFailure)?;
                by_run
                    .entry(run_code.to_owned())
                    .or_insert_with(Vec::new)
                    .extend(parsed);
            }
            Ok(by_run)
        })
    }

    fn list_messages(&mut self, id: &str) -> Result<Vec<ChatMessage>, ChatError> {
        blocking(async {
            self.events(id, "tron.message")
                .await?
                .into_iter()
                .map(|e| serde_json::from_value(e.payload).map_err(|_| ChatError::StorageFailure))
                .collect()
        })
    }
    fn list_user_input_requests(
        &mut self,
        id: &str,
    ) -> Result<Vec<ChatUserInputRequest>, ChatError> {
        blocking(async {
            let mut requests = std::collections::BTreeMap::new();
            for e in self.events(id, "tron.interaction-view").await? {
                let request: ChatUserInputRequest =
                    serde_json::from_value(e.payload).map_err(|_| ChatError::StorageFailure)?;
                requests.insert(request.request_code.clone(), request);
            }
            let services = self
                .framework()
                .await?
                .services()
                .map_err(|_| ChatError::StorageFailure)?;
            let interactions = services
                .get::<fw::InteractionStoreService>()
                .ok_or(ChatError::StorageFailure)?;
            // One batched lookup for every pending request in the session.
            let ids = requests
                .keys()
                .map(|code| fw::RunId::parse_str(code))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| ChatError::StorageFailure)?;
            let records = interactions
                .0
                .get_many(id, &ids)
                .await
                .map_err(chat_error)?;
            let mut views = Vec::new();
            for (mut request, record) in requests.into_values().zip(records) {
                request.status = if record.response.is_some() {
                    ChatUserInputRequestStatus::Answered
                } else {
                    ChatUserInputRequestStatus::Pending
                };
                request.answered_at = record
                    .answered_at_unix_ms
                    .map(|v| crate::shared::time::timestamp_from_millis(v as i64));
                request.answers = record
                    .response
                    .and_then(|r| r.value.get("answers").and_then(Value::as_object).cloned())
                    .map(|answers| {
                        answers
                            .into_iter()
                            .map(|(question_code, value)| ChatUserInputAnswer {
                                question_code,
                                answer_kind: if value.get("option_id").is_some() {
                                    ChatUserInputAnswerKind::Option
                                } else if value.get("text").is_some() {
                                    ChatUserInputAnswerKind::Other
                                } else {
                                    ChatUserInputAnswerKind::Skipped
                                },
                                option_code: value["option_id"].as_str().map(str::to_owned),
                                other_text: value["text"].as_str().map(str::to_owned),
                            })
                            .collect()
                    });
                views.push(request);
            }
            Ok(views)
        })
    }
    fn create_user_input_request(
        &mut self,
        request: &ChatUserInputRequest,
    ) -> Result<(), ChatError> {
        blocking(async {
            self.framework()
                .await?
                .sessions()
                .map_err(|_| ChatError::StorageFailure)?
                .store
                .append_event(fw::SessionEvent::new(
                    &request.session_code,
                    "tron.interaction-view",
                    serde_json::to_value(request).map_err(|_| ChatError::StorageFailure)?,
                ))
                .await
                .map_err(chat_error)?;
            Ok(())
        })
    }
}
