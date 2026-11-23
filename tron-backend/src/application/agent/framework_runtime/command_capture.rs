//! Command output uses the same private storage and deletion lifecycle as chat attachments.
use super::HostExecution;
use crate::application::resource::{ChatSessionAttachmentCreateCommand, ResourceService};
use async_trait::async_trait;
use eliteforge_agent_core as fw;
use eliteforge_agent_official_plugin_tools_local as local;
use serde_json::{Value, json};
use std::io::{Seek, Write};

pub(super) struct CommandCapture(pub ResourceService);

struct SessionOutputFile {
    temporary: tempfile::NamedTempFile,
    resources: ResourceService,
    command: ChatSessionAttachmentCreateCommand,
    locale: String,
}

impl Write for SessionOutputFile {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.temporary.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.temporary.flush()
    }
}

impl local::CommandOutputWriter for SessionOutputFile {
    fn publish(&mut self) -> Result<Value, String> {
        self.temporary
            .as_file_mut()
            .rewind()
            .map_err(|error| error.to_string())?;
        let record = self
            .resources
            .store_chat_session_attachment(self.command.clone(), self.temporary.as_file_mut())
            .map_err(|error| {
                tron_i18n::t!(
                    "agent_runtime.tool.command_capture.publish_failed",
                    locale = self.locale.as_str(),
                    detail = error
                )
            })?;
        Ok(json!({"kind":"session_attachment", "attachmentCode":record.attachment_code}))
    }
}

#[async_trait]
impl local::CommandOutputCaptureProvider for CommandCapture {
    async fn create(
        &self,
        stream: &str,
        run: &fw::RunContext,
    ) -> Result<local::CommandOutputFile, fw::ToolError> {
        let host = HostExecution::from_extensions(&run.extensions)
            .map_err(|error| fw::ToolError::Failed(error.to_string()))?;
        let context = host.state.lock().await.context.clone();
        let locale = context.locale.as_str();
        let session = context
            .conversation_code
            .as_ref()
            .filter(|code| !code.trim().is_empty())
            .ok_or_else(|| {
                fw::ToolError::Failed(
                    tron_i18n::t!(
                        "agent_runtime.tool.file_access.session_required",
                        locale = locale
                    )
                    .into_owned(),
                )
            })?;
        let resources = self.0.for_agent_in_workspace(
            context.agent_code.as_deref(),
            context.execution_workspace_code(),
        );
        let max_bytes = resources.max_chat_session_attachment_bytes();
        let temporary = tempfile::NamedTempFile::new().map_err(|error| {
            fw::ToolError::Failed(tron_i18n::t!(
                "agent_runtime.tool.command_capture.create_failed",
                locale = locale,
                detail = error
            ))
        })?;
        let command = ChatSessionAttachmentCreateCommand {
            workspace_code: context.workspace_code.clone(),
            session_code: session.clone(),
            attachment_code: format!("CMD-OUT-{:032X}", rand::random::<u128>()),
            file_name: format!("command-{stream}.txt"),
            content_type: "text/plain".into(),
        };
        Ok(local::CommandOutputFile {
            writer: Box::new(SessionOutputFile {
                temporary,
                resources,
                command,
                locale: locale.into(),
            }),
            max_bytes,
        })
    }
}
