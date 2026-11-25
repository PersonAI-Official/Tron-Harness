use super::*;
use crate::{application::workspace::WorkspaceSessionPurger, domain::workspace::WorkspaceError};

impl ChatService {
    /// Permanently delete every conversation linked to the workspaces. The
    /// preflight pass retires admission for all sessions first, so one running
    /// conversation aborts before any history or attachment is removed.
    pub fn purge_workspace_sessions(&self, workspace_codes: &[String]) -> Result<usize, ChatError> {
        let session_codes = self
            .repository()
            .session_codes_by_workspaces(workspace_codes)?;
        self.purge_sessions(session_codes)
    }

    pub(crate) fn purge_task_sessions(
        &self,
        task_code: &str,
        work_session_code: Option<&str>,
    ) -> Result<usize, ChatError> {
        let sessions = self.repository().list_sessions()?;
        let codes = sessions
            .into_iter()
            .filter(|session| {
                session.task_code.as_deref() == Some(task_code)
                    || (session.source_type == ChatSessionSourceType::Task
                        && session.source_code.as_deref() == Some(task_code))
                    || work_session_code == Some(session.session_code.as_str())
            })
            .map(|session| session.session_code)
            .collect();
        self.purge_sessions(codes)
    }

    pub(crate) fn purge_automation_sessions(
        &self,
        automation_code: &str,
        mut execution_sessions: Vec<String>,
    ) -> Result<usize, ChatError> {
        let sessions = self
            .repository()
            .list_sessions_scoped(None, Some(ChatSessionSourceType::Automation))?;
        execution_sessions.extend(
            sessions
                .into_iter()
                .filter(|session| session.source_code.as_deref() == Some(automation_code))
                .map(|session| session.session_code),
        );
        execution_sessions.sort_unstable();
        execution_sessions.dedup();
        self.purge_sessions(execution_sessions)
    }

    fn purge_sessions(&self, session_codes: Vec<String>) -> Result<usize, ChatError> {
        let mut removable = Vec::with_capacity(session_codes.len());
        for session_code in session_codes {
            match self.prepare_session_deletion(&session_code) {
                Ok(()) => removable.push(session_code),
                Err(ChatError::SessionNotFound) => {}
                Err(error) => {
                    self.reactivate_prepared_sessions(&removable);
                    return Err(error);
                }
            }
        }
        for (index, session_code) in removable.iter().enumerate() {
            match self.delete_session(session_code) {
                Ok(_) | Err(ChatError::SessionNotFound) => {}
                Err(error) => {
                    self.reactivate_prepared_sessions(&removable[index..]);
                    return Err(error);
                }
            }
        }
        Ok(removable.len())
    }

    fn reactivate_prepared_sessions(&self, codes: &[String]) {
        for code in codes {
            if self.repository().get_session(code).is_ok_and(|session| {
                session.status == ChatSessionStatus::Active
            }) {
                let _ = self.active_runs.reactivate_session(code);
            }
        }
    }
}

impl WorkspaceSessionPurger for ChatService {
    fn purge_workspace_sessions(
        &self,
        workspace_codes: &[String],
    ) -> Result<usize, WorkspaceError> {
        ChatService::purge_workspace_sessions(self, workspace_codes).map_err(|error| match error {
            ChatError::SessionRunActive => WorkspaceError::WorkspaceSessionsBusy,
            _ => WorkspaceError::SessionCleanupFailed,
        })
    }
}
