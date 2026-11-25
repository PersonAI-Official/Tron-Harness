use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    application::agent::AgentInvocationResult,
    application::agent::AgentPageContext,
    domain::{
        agent::{AgentRunLimits, EffectiveTool},
        chat::{
            ChatClientCapabilities, ChatMessage, ChatMessageAttachment, ChatSession,
            ChatSessionSourceType,
        },
        model_provider::ModelSelection,
        reasoning::ReasoningEffort,
    },
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionQuery {
    pub workspace_code: Option<String>,
    pub source_type: Option<ChatSessionSourceType>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatSessionCommand {
    #[serde(default)]
    pub execution_mode: crate::domain::agent::ToolExecutionMode,
    pub workspace_code: Option<String>,
    pub project_code: Option<String>,
    pub directory_code: Option<String>,
    pub working_directory_code: Option<String>,
    pub title: Option<String>,
    pub app_code: Option<String>,
    pub task_code: Option<String>,
    pub source_type: Option<ChatSessionSourceType>,
    pub source_code: Option<String>,
    pub agent_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChatSessionCommand {
    pub title: Option<String>,
    pub execution_mode: Option<crate::domain::agent::ToolExecutionMode>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResumeEntry {
    pub interrupt_id: String,
    pub status: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendChatMessageCommand {
    #[serde(default)]
    pub annotations: Vec<eliteforge_agent_core::MessageAnnotation>,
    #[serde(skip)]
    pub run_code: Option<String>,
    #[serde(skip)]
    pub a2ui_action_id: Option<String>,
    #[serde(default)]
    pub a2ui_metadata: Option<Value>,
    pub content: Option<String>,
    pub workspace_code: Option<String>,
    pub app_code: Option<String>,
    pub task_code: Option<String>,
    pub agent_code: Option<String>,
    /// First-party clients send the full selection. `modelCode` remains for
    /// older clients and is normalized as a Fixed selection by the handler.
    pub model_selection: Option<ModelSelection>,
    pub model_code: Option<String>,
    pub system_model_id: Option<String>,
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default)]
    pub(crate) page_context: Option<AgentPageContext>,
    pub limits: Option<AgentRunLimits>,
    #[serde(default)]
    pub tools: Vec<EffectiveTool>,
    #[serde(default)]
    pub attachments: Vec<ChatMessageAttachment>,
    #[serde(default)]
    pub client_capabilities: Option<ChatClientCapabilities>,
    #[serde(default)]
    pub resume: Vec<ChatResumeEntry>,
    #[serde(default)]
    pub requested_skills: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteerChatMessageCommand {
    #[serde(default)]
    pub page_context: Option<AgentPageContext>,
    #[serde(default)]
    pub annotations: Vec<eliteforge_agent_core::MessageAnnotation>,
    pub session_code: Option<String>,
    pub expected_run_code: Option<String>,
    pub content: Option<String>,
    #[serde(default)]
    pub attachments: Vec<ChatMessageAttachment>,
    #[serde(default)]
    pub requested_skills: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ChatSessionAttachmentUploadCommand {
    pub kind: String,
    pub summary: Option<String>,
    pub metadata: Value,
    pub file_name: String,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendResult {
    pub session: ChatSession,
    pub run: AgentInvocationResult,
    pub messages: Vec<ChatMessage>,
    pub user_input_request: Option<crate::domain::chat::ChatUserInputRequest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSteerResult {
    pub session_code: String,
    pub run_code: String,
    pub message: ChatMessage,
}
