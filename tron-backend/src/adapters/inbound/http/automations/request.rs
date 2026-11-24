use serde::Deserialize;

use crate::{
    application::automation::{
        CreateAutomationCommand, UpdateAutomationCommand, UpdateAutomationStatusCommand,
    },
    domain::automation::{AutomationConcurrencyMode, AutomationStatus},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAutomationRequest {
    pub workspace_code: String,
    pub name: String,
    pub description: Option<String>,
    pub agent_code: Option<String>,
    pub model_code: Option<String>,
    pub concurrency_mode: Option<AutomationConcurrencyMode>,
    pub prompt: String,
    pub cron_expression: String,
    pub status: Option<AutomationStatus>,
}

impl From<CreateAutomationRequest> for CreateAutomationCommand {
    fn from(request: CreateAutomationRequest) -> Self {
        Self {
            workspace_code: request.workspace_code,
            name: request.name,
            description: request.description,
            agent_code: request.agent_code,
            model_code: request.model_code,
            concurrency_mode: request.concurrency_mode,
            prompt: request.prompt,
            cron_expression: request.cron_expression,
            status: request.status,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAutomationRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub agent_code: Option<String>,
    pub model_code: Option<Option<String>>,
    pub concurrency_mode: Option<AutomationConcurrencyMode>,
    pub prompt: Option<String>,
    pub cron_expression: Option<String>,
}

impl From<UpdateAutomationRequest> for UpdateAutomationCommand {
    fn from(request: UpdateAutomationRequest) -> Self {
        Self {
            name: request.name,
            description: request.description,
            agent_code: request.agent_code,
            model_code: request.model_code,
            concurrency_mode: request.concurrency_mode,
            prompt: request.prompt,
            cron_expression: request.cron_expression,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAutomationStatusRequest {
    pub status: AutomationStatus,
}

impl From<UpdateAutomationStatusRequest> for UpdateAutomationStatusCommand {
    fn from(request: UpdateAutomationStatusRequest) -> Self {
        Self {
            status: request.status,
        }
    }
}
