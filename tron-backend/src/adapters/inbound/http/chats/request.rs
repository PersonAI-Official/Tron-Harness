use serde::Deserialize;

use crate::{
    application::chat::ChatResumeEntry,
    domain::chat::{ChatMessageAttachment, ChatUserInputAnswer},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GetActiveChatRunQuery {
    pub workspace_code: Option<String>,
    pub session_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StopChatRunRequest {
    pub session_code: Option<String>,
    pub expected_run_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SteerChatMessageRequest {
    pub page_context: Option<crate::application::agent::AgentPageContext>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnswerChatUserInputRequest {
    pub client_capabilities: Option<crate::domain::chat::ChatClientCapabilities>,
    pub a2ui_metadata: Option<serde_json::Value>,
    pub answers: Option<Vec<ChatUserInputAnswer>>,
    #[serde(default)]
    pub resume: Vec<ChatResumeEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendChatMessageRequest {
    #[serde(default)]
    pub annotations: Vec<eliteforge_agent_core::MessageAnnotation>,
    pub a2ui_metadata: Option<serde_json::Value>,
    pub content: Option<String>,
    pub workspace_code: Option<String>,
    pub app_code: Option<String>,
    pub task_code: Option<String>,
    pub agent_code: Option<String>,
    pub model_selection: Option<crate::domain::model_provider::ModelSelection>,
    pub model_code: Option<String>,
    pub system_model_id: Option<String>,
    pub reasoning_effort: Option<crate::domain::reasoning::ReasoningEffort>,
    pub page_context: Option<crate::application::agent::AgentPageContext>,
    pub limits: Option<crate::domain::agent::AgentRunLimits>,
    #[serde(default)]
    pub tools: Vec<crate::domain::agent::EffectiveTool>,
    #[serde(default)]
    pub attachments: Vec<ChatMessageAttachment>,
    pub client_capabilities: Option<crate::domain::chat::ChatClientCapabilities>,
    #[serde(default)]
    pub resume: Vec<ChatResumeEntry>,
    #[serde(default)]
    pub requested_skills: Vec<String>,
}

impl From<SendChatMessageRequest> for crate::application::chat::SendChatMessageCommand {
    fn from(request: SendChatMessageRequest) -> Self {
        Self {
            run_code: None,
            a2ui_action_id: None,
            annotations: request.annotations,
            a2ui_metadata: request.a2ui_metadata,
            content: request.content,
            workspace_code: request.workspace_code,
            app_code: request.app_code,
            task_code: request.task_code,
            agent_code: request.agent_code,
            model_selection: request.model_selection,
            model_code: request.model_code,
            system_model_id: request.system_model_id,
            reasoning_effort: request.reasoning_effort,
            page_context: request.page_context,
            limits: request.limits,
            tools: request.tools,
            attachments: request.attachments,
            client_capabilities: request.client_capabilities,
            resume: request.resume,
            requested_skills: request.requested_skills,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StreamChatMessageRequest {
    #[serde(default)]
    pub annotations: Vec<eliteforge_agent_core::MessageAnnotation>,
    pub a2ui_metadata: Option<serde_json::Value>,
    pub content: Option<String>,
    pub run_code: Option<String>,
    #[serde(default)]
    pub task_execution: bool,
    pub workspace_code: Option<String>,
    pub app_code: Option<String>,
    pub task_code: Option<String>,
    pub agent_code: Option<String>,
    pub model_selection: Option<crate::domain::model_provider::ModelSelection>,
    pub model_code: Option<String>,
    pub system_model_id: Option<String>,
    pub reasoning_effort: Option<crate::domain::reasoning::ReasoningEffort>,
    pub page_context: Option<crate::application::agent::AgentPageContext>,
    pub limits: Option<crate::domain::agent::AgentRunLimits>,
    #[serde(default)]
    pub tools: Vec<crate::domain::agent::EffectiveTool>,
    #[serde(default)]
    pub attachments: Vec<ChatMessageAttachment>,
    pub client_capabilities: Option<crate::domain::chat::ChatClientCapabilities>,
    #[serde(default)]
    pub resume: Vec<ChatResumeEntry>,
    #[serde(default)]
    pub requested_skills: Vec<String>,
}

impl From<StreamChatMessageRequest> for crate::application::chat::SendChatMessageCommand {
    fn from(request: StreamChatMessageRequest) -> Self {
        Self {
            run_code: request.run_code,
            a2ui_action_id: None,
            annotations: request.annotations,
            a2ui_metadata: request.a2ui_metadata,
            content: request.content,
            workspace_code: request.workspace_code,
            app_code: request.app_code,
            task_code: request.task_code,
            agent_code: request.agent_code,
            model_selection: request.model_selection,
            model_code: request.model_code,
            system_model_id: request.system_model_id,
            reasoning_effort: request.reasoning_effort,
            page_context: request.page_context,
            limits: request.limits,
            tools: request.tools,
            attachments: request.attachments,
            client_capabilities: request.client_capabilities,
            resume: request.resume,
            requested_skills: request.requested_skills,
        }
    }
}
