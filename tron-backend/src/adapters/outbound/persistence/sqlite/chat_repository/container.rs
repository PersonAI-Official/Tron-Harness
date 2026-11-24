//! Container mutations share the Framework transaction and reject a stale scope.
use super::*;

impl SqliteChatRepository {
    pub(super) async fn read_container_snapshot(
        &self,
        container: &ChatContainer,
    ) -> Result<ChatContainerSnapshot, ChatError> {
        self.framework().await?;
        let db = self.host.shared_database().await.map_err(chat_error)?;
        let container = container.clone();
        db.call(move |c| -> rusqlite::Result<ChatContainerSnapshot> {
            let (workspace, project, directory_codes) = match &container {
                ChatContainer::Project { workspace_code, project_code } => {
                    c.query_row("SELECT project_code FROM projects WHERE project_code=?1 AND workspace_code=?2", params![project_code, workspace_code], |row| row.get::<_, String>(0))?;
                    (workspace_code, Some(project_code), Vec::new())
                }
                ChatContainer::Directory { workspace_code, directory_code } => {
                    let (host, root): (String, String) = c.query_row("SELECT execution_host,canonical_path FROM directories WHERE directory_code=?1 AND workspace_code=?2", params![directory_code, workspace_code], |row| Ok((row.get(0)?, row.get(1)?)))?;
                    let mut query = c.prepare("SELECT directory_code,canonical_path FROM directories WHERE workspace_code=?1 AND execution_host=?2 ORDER BY directory_code")?;
                    let rows = query.query_map(params![workspace_code, host], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
                    let codes = rows.into_iter().filter(|(_, path)| std::path::Path::new(path).starts_with(&root)).map(|(code, _)| code).collect::<Vec<_>>();
                    (workspace_code, None, codes)
                }
            };
            let mut query = c.prepare("SELECT l.session_id,l.business_json FROM workspace_session_links l JOIN ef_sessions s ON s.id=l.session_id WHERE l.workspace_code=?1 ORDER BY l.session_id")?;
            let rows = query.query_map([workspace], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
            let mut session_codes = Vec::new();
            for (code, json) in rows {
                let link: Link = serde_json::from_str(&json).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                let included = if let Some(project) = project {
                    link.project_code.as_ref() == Some(project)
                } else {
                    link.directory_code.as_ref().is_some_and(|code| directory_codes.contains(code))
                };
                if included { session_codes.push(code); }
            }
            let mut query = c.prepare("SELECT session_code,attachment_code FROM resource_chat_session_attachments WHERE workspace_code=?1 ORDER BY session_code,attachment_code")?;
            let attachments = query.query_map([workspace], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
            let attachment_codes = attachments.into_iter().filter(|(session, _)| session_codes.binary_search(session).is_ok()).map(|(_, code)| code).collect();
            Ok(ChatContainerSnapshot { directory_codes, session_codes, attachment_codes })
        }).await.map_err(|_| ChatError::StorageFailure)
    }

    pub(super) async fn apply_container_mutation(
        &self,
        container: &ChatContainer,
        expected: &ChatContainerSnapshot,
        delete: bool,
    ) -> Result<(), ChatError> {
        let framework = self.framework().await?;
        let sessions = framework
            .sessions()
            .map_err(|_| ChatError::StorageFailure)?;
        let db = self.host.shared_database().await.map_err(chat_error)?;
        // Stage only session-owned directories. User project directories are preserved.
        let mut staged = None;
        let result = db.transaction(async {
            let actual = self.read_container_snapshot(container).await.map_err(|e| fw::StateError::Backend(e.to_string()))?;
            if &actual != expected { return Err(fw::StateError::Conflict); }
            let selected = serde_json::to_string(&actual.session_codes).map_err(|e| fw::StateError::Backend(e.to_string()))?;
            let selected_for_read = selected.clone();
            let records = db.call(move |c| -> Result<Vec<(fw::SessionRecord, Link, bool, bool)>, fw::StateError> {
                let mut query = c.prepare("SELECT s.record,l.business_json,EXISTS(SELECT 1 FROM ef_goals g WHERE g.session_id=s.id AND g.status='ACTIVE'),EXISTS(SELECT 1 FROM assistant_sessions d WHERE d.session_code=s.id AND d.purpose='Heartbeat' AND (EXISTS(SELECT 1 FROM assistant_events e WHERE (e.session_code=d.session_code OR e.event_id=d.correlation_key) AND (e.source!='Heartbeat' OR e.status IN ('Queued','Running','AwaitingInput'))) OR EXISTS(SELECT 1 FROM tasks t WHERE t.work_session_code=d.session_code) OR EXISTS(SELECT 1 FROM collaboration_threads t WHERE t.session_id=d.session_code) OR EXISTS(SELECT 1 FROM assistant_work_changes w WHERE json_extract(w.record_json,'$.sessionCode')=d.session_code AND json_extract(w.record_json,'$.resolvedAt') IS NULL))) FROM ef_sessions s JOIN workspace_session_links l ON l.session_id=s.id WHERE s.id IN (SELECT value FROM json_each(?1)) ORDER BY s.id").map_err(|e| fw::StateError::Backend(e.to_string()))?;
                query.query_map([selected_for_read], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,bool>(2)?,row.get::<_,bool>(3)?))).map_err(|e| fw::StateError::Backend(e.to_string()))?
                    .map(|row| {
                        let (session,link,active,blocked) = row.map_err(|e| fw::StateError::Backend(e.to_string()))?;
                        Ok((serde_json::from_str(&session).map_err(|e| fw::StateError::Backend(e.to_string()))?,serde_json::from_str(&link).map_err(|e| fw::StateError::Backend(e.to_string()))?, active, blocked))
                    }).collect()
            }).await.map_err(|e| fw::StateError::Backend(e.to_string()))?;
            if records.iter().any(|(_,_,active,blocked)| *active || (delete && *blocked)) { return Err(fw::StateError::Busy); }
            if records.len() != actual.session_codes.len() { return Err(fw::StateError::Conflict); }
            if delete {
                let owners = records.iter().map(|(session,link,_,_)| (session.id.clone(),link.user_code.clone())).collect();
                staged = Some(self.host.stage_session_working_directories(owners).await.map_err(|error| {
                    tracing::error!(%error, "session directory cleanup staging failed");
                    fw::StateError::Backend("session_directory_cleanup_failed".into())
                })?);
            }
            for (record, _, _, _) in records {
                if delete {
                    // Framework retains its own busy, tombstone and lifecycle fences.
                    sessions.delete(&record.id).await?;
                } else if record.status != fw::SessionStatus::Archived {
                    sessions.archive(&record.id).await?;
                }
            }
            if delete {
                db.call(move |c| -> rusqlite::Result<()> {
                    c.execute("DELETE FROM assistant_events WHERE source='Heartbeat' AND EXISTS(SELECT 1 FROM assistant_sessions s WHERE s.session_code IN (SELECT value FROM json_each(?1)) AND s.purpose='Heartbeat' AND (assistant_events.session_code=s.session_code OR assistant_events.event_id=s.correlation_key))", [&selected])?;
                    c.execute("DELETE FROM assistant_sessions WHERE session_code IN (SELECT value FROM json_each(?1)) AND purpose='Heartbeat'", [&selected])?;
                    c.execute("DELETE FROM resource_chat_session_attachments WHERE session_code IN (SELECT value FROM json_each(?1))", [&selected])?;
                    for (table, sql) in [
                        ("chat_session_auto_model_routes", "DELETE FROM chat_session_auto_model_routes WHERE session_code IN (SELECT value FROM json_each(?1))"),
                        ("provider_model_run_snapshots", "DELETE FROM provider_model_run_snapshots WHERE session_code IN (SELECT value FROM json_each(?1))"),
                        ("automation_execution_records", "UPDATE automation_execution_records SET session_code=NULL WHERE session_code IN (SELECT value FROM json_each(?1))"),
                        ("task_execution_records", "UPDATE task_execution_records SET session_code=NULL WHERE session_code IN (SELECT value FROM json_each(?1))"),
                    ] {
                        if c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",[table],|r|r.get::<_,bool>(0))? { c.execute(sql,[&selected])?; }
                    }
                    c.execute("DELETE FROM workspace_session_links WHERE session_id IN (SELECT value FROM json_each(?1))", [&selected])?;
                    Ok(())
                }).await.map_err(|e| fw::StateError::Backend(e.to_string()))?;
            }
            let container = container.clone();
            db.call(move |c| -> rusqlite::Result<()> {
                match container {
                    ChatContainer::Project { workspace_code, project_code } => {
                        if delete {
                            c.execute("DELETE FROM projects WHERE project_code=?1 AND workspace_code=?2", params![project_code, workspace_code])?;
                        } else {
                            c.execute("UPDATE projects SET status='ARCHIVED',archived_at=CAST(strftime('%s','now') AS INTEGER)*1000,updated_at=CAST(strftime('%s','now') AS INTEGER)*1000 WHERE project_code=?1 AND workspace_code=?2", params![project_code, workspace_code])?;
                        }
                    }
                    ChatContainer::Directory { workspace_code, .. } => {
                        for code in actual.directory_codes {
                            if delete {
                                c.execute("UPDATE topic_memos SET revision=revision+1 WHERE project_code IN (SELECT project_code FROM topic_references WHERE kind='Directory' AND target_code=?1)", [&code])?;
                                c.execute("DELETE FROM topic_references WHERE kind='Directory' AND target_code=?1", [&code])?;
                                // Other topics may use this directory only for execution.
                                // Keep those chats; remove their now-invalid directory reference.
                                c.execute("UPDATE workspace_session_links SET business_json=json_set(business_json,'$.working_directory_code',NULL) WHERE workspace_code=?1 AND json_extract(business_json,'$.working_directory_code')=?2", params![workspace_code, code])?;
                                c.execute("DELETE FROM directories WHERE directory_code=?1 AND workspace_code=?2", params![code, workspace_code])?;
                            } else {
                                c.execute("UPDATE directories SET archived=1 WHERE directory_code=?1 AND workspace_code=?2", params![code, workspace_code])?;
                            }
                        }
                    }
                }
                Ok(())
            }).await.map_err(|e| fw::StateError::Backend(e.to_string()))?;
            Ok(())
        }).await.map_err(|error| match error {
            fw::StateError::Conflict => ChatError::SessionLocationChanged,
            fw::StateError::Backend(ref message) if message == "session_directory_cleanup_failed" => ChatError::StorageFailure,
            other => chat_error(other),
        });
        if result.is_ok()
            && let Some(staged) = staged.take()
        {
            staged.commit().map_err(|_| ChatError::StorageFailure)?;
        }
        result
    }
}
