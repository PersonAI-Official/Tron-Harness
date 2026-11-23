//! Trusted task identity and lifecycle for the Framework file coordinator.
use super::*;
use eliteforge_agent_official_plugin_tools_local::coordination::{
    FileCoordinationBackend, FileCoordinationIdentity, FileInvalidation, FileSubject,
    default_file_coordination,
};

pub(super) fn attach(context: &AgentRunContext, session: &str, request: &mut fw::RunRequest) {
    request
        .extensions
        .insert(FileCoordinationIdentity(FileSubject {
            security_domain: format!(
                "{}:{}",
                context.workspace_code,
                context.user_code.as_deref().unwrap_or("system"),
            ),
            session_id: session.to_owned(),
            task_instance_id: format!(
                "{}:{}:{}",
                session,
                context.agent_code.as_deref().unwrap_or("main-agent"),
                context
                    .execution_scope
                    .task_code
                    .as_deref()
                    .unwrap_or("chat"),
            ),
        }));
}

impl FrameworkHost {
    pub(crate) async fn invalidate_file_session(&self, session: &str) -> Result<(), String> {
        // Do not initialize another Framework during deletion or use the
        // storage-only Framework: it does not own the selected file backend.
        let backend = if let Some(framework) = self.0.framework.get() {
            framework
                .services()
                .map_err(|error| error.to_string())?
                .extension::<dyn FileCoordinationBackend>()
                .unwrap_or_else(default_file_coordination)
        } else {
            default_file_coordination()
        };
        backend
            .invalidate(FileInvalidation {
                session_id: Some(session.to_owned()),
                ..Default::default()
            })
            .await
            .map_err(|error| error.to_string())
    }
}
