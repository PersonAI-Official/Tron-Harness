use std::sync::Arc;

use serde::Deserialize;

use crate::{
    application::automation::{
        AutomationService, CreateAutomationCommand, UpdateAutomationCommand,
        UpdateAutomationStatusCommand,
    },
    domain::{
        agent::{AgentRunContext, Tool, ToolError, ToolExecutionResult, ToolRequest},
        automation::{AutomationListQuery, AutomationStatus},
    },
};

use super::common::{
    ensure_workspace_match, enum_tool_error, json_observation, normalize_optional_token,
    optional_trimmed, parse_tool_input, resolve_workspace_code, service_tool_error,
};

pub(super) fn tools(automations: AutomationService) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(AutomationPageTool {
            automations: automations.clone(),
        }),
        Arc::new(AutomationGetTool {
            automations: automations.clone(),
        }),
        Arc::new(AutomationCreateTool {
            automations: automations.clone(),
        }),
        Arc::new(AutomationUpdateTool {
            automations: automations.clone(),
        }),
        Arc::new(AutomationDeleteTool {
            automations: automations.clone(),
        }),
        Arc::new(AutomationRestoreTool {
            automations: automations.clone(),
        }),
        Arc::new(AutomationExecuteTool { automations }),
    ]
}

#[derive(Clone)]
struct AutomationPageTool {
    automations: AutomationService,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutomationPageInput {
    workspace_code: Option<String>,
    current: Option<usize>,
    size: Option<usize>,
    search: Option<String>,
    status: Option<String>,
    include_archived: Option<bool>,
}

impl Tool for AutomationPageTool {
    fn name(&self) -> &str {
        "automation_page"
    }

    fn exec(
        &self,
        request: ToolRequest,
        context: &AgentRunContext,
    ) -> Result<ToolExecutionResult, ToolError> {
        let input: AutomationPageInput = parse_tool_input(request.input)?;
        let workspace_code = resolve_workspace_code(&input.workspace_code, context)?;
        let output = self
            .automations
            .list_automations(AutomationListQuery {
                workspace_code: Some(workspace_code),
                current: input.current,
                size: input.size,
                search: input.search,
                status: parse_automation_status(input.status.as_deref())?,
                include_archived: Some(input.include_archived.unwrap_or(false)),
            })
            .map_err(service_tool_error)?;
        json_observation(self.name(), &output)
    }
}

#[derive(Clone)]
struct AutomationGetTool {
    automations: AutomationService,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutomationCodeInput {
    workspace_code: Option<String>,
    automation_code: String,
}

impl Tool for AutomationGetTool {
    fn name(&self) -> &str {
        "automation_get"
    }

    fn exec(
        &self,
        request: ToolRequest,
        context: &AgentRunContext,
    ) -> Result<ToolExecutionResult, ToolError> {
        let input: AutomationCodeInput = parse_tool_input(request.input)?;
        let workspace_code = resolve_workspace_code(&input.workspace_code, context)?;
        let output = self
            .automations
            .get_automation(input.automation_code.trim())
            .map_err(service_tool_error)?;
        ensure_workspace_match(&workspace_code, &output.workspace_code, "automation")?;
        json_observation(self.name(), &output)
    }
}

#[derive(Clone)]
struct AutomationCreateTool {
    automations: AutomationService,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutomationCreateInput {
    workspace_code: Option<String>,
    name: String,
    description: Option<String>,
    agent_code: Option<String>,
    model_code: Option<String>,
    prompt: String,
    cron_expression: String,
    status: Option<String>,
}

impl Tool for AutomationCreateTool {
    fn name(&self) -> &str {
        "automation_create"
    }

    fn exec(
        &self,
        request: ToolRequest,
        context: &AgentRunContext,
    ) -> Result<ToolExecutionResult, ToolError> {
        let input: AutomationCreateInput = parse_tool_input(request.input)?;
        let workspace_code = resolve_workspace_code(&input.workspace_code, context)?;
        let output = self
            .automations
            .create_automation(CreateAutomationCommand {
                workspace_code,
                name: input.name,
                description: input.description,
                agent_code: input.agent_code,
                model_code: input.model_code,
                concurrency_mode: None,
                prompt: input.prompt,
                cron_expression: input.cron_expression,
                status: parse_automation_status(input.status.as_deref())?,
            })
            .map_err(service_tool_error)?;
        json_observation(self.name(), &output)
    }
}

#[derive(Clone)]
struct AutomationUpdateTool {
    automations: AutomationService,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutomationUpdateInput {
    workspace_code: Option<String>,
    automation_code: String,
    name: Option<String>,
    description: Option<String>,
    agent_code: Option<String>,
    model_code: Option<String>,
    prompt: Option<String>,
    cron_expression: Option<String>,
    status: Option<String>,
}

impl Tool for AutomationUpdateTool {
    fn name(&self) -> &str {
        "automation_update"
    }

    fn exec(
        &self,
        request: ToolRequest,
        context: &AgentRunContext,
    ) -> Result<ToolExecutionResult, ToolError> {
        let input: AutomationUpdateInput = parse_tool_input(request.input)?;
        let workspace_code = resolve_workspace_code(&input.workspace_code, context)?;
        let automation_code = input.automation_code.trim().to_string();
        let current = self
            .automations
            .get_automation(&automation_code)
            .map_err(service_tool_error)?;
        ensure_workspace_match(&workspace_code, &current.workspace_code, "automation")?;
        let mut output = self
            .automations
            .update_automation(
                &automation_code,
                UpdateAutomationCommand {
                    name: input.name,
                    description: input
                        .description
                        .map(|description| optional_trimmed(Some(description))),
                    agent_code: input.agent_code,
                    model_code: input.model_code.map(Some),
                    concurrency_mode: None,
                    prompt: input.prompt,
                    cron_expression: input.cron_expression,
                },
            )
            .map_err(service_tool_error)?;
        if let Some(status) = parse_automation_status(input.status.as_deref())? {
            output = self
                .automations
                .update_status(&automation_code, UpdateAutomationStatusCommand { status })
                .map_err(service_tool_error)?;
        }
        json_observation(self.name(), &output)
    }
}

#[derive(Clone)]
struct AutomationDeleteTool {
    automations: AutomationService,
}

impl Tool for AutomationDeleteTool {
    fn name(&self) -> &str {
        "automation_delete"
    }

    fn exec(
        &self,
        request: ToolRequest,
        context: &AgentRunContext,
    ) -> Result<ToolExecutionResult, ToolError> {
        let input: AutomationCodeInput = parse_tool_input(request.input)?;
        let workspace_code = resolve_workspace_code(&input.workspace_code, context)?;
        let current = self
            .automations
            .get_automation(input.automation_code.trim())
            .map_err(service_tool_error)?;
        ensure_workspace_match(&workspace_code, &current.workspace_code, "automation")?;
        let output = self
            .automations
            .archive_automation(input.automation_code.trim())
            .map_err(service_tool_error)?;
        json_observation(self.name(), &output)
    }
}

#[derive(Clone)]
struct AutomationRestoreTool {
    automations: AutomationService,
}

impl Tool for AutomationRestoreTool {
    fn name(&self) -> &str {
        "automation_restore"
    }

    fn exec(
        &self,
        request: ToolRequest,
        context: &AgentRunContext,
    ) -> Result<ToolExecutionResult, ToolError> {
        let input: AutomationCodeInput = parse_tool_input(request.input)?;
        let workspace_code = resolve_workspace_code(&input.workspace_code, context)?;
        let current = self
            .automations
            .get_automation(input.automation_code.trim())
            .map_err(service_tool_error)?;
        ensure_workspace_match(&workspace_code, &current.workspace_code, "automation")?;
        let output = self
            .automations
            .restore_automation(input.automation_code.trim())
            .map_err(service_tool_error)?;
        json_observation(self.name(), &output)
    }
}

#[derive(Clone)]
struct AutomationExecuteTool {
    automations: AutomationService,
}

impl Tool for AutomationExecuteTool {
    fn name(&self) -> &str {
        "automation_execute"
    }

    fn exec(
        &self,
        request: ToolRequest,
        context: &AgentRunContext,
    ) -> Result<ToolExecutionResult, ToolError> {
        let input: AutomationCodeInput = parse_tool_input(request.input)?;
        let workspace_code = resolve_workspace_code(&input.workspace_code, context)?;
        let current = self
            .automations
            .get_automation(input.automation_code.trim())
            .map_err(service_tool_error)?;
        ensure_workspace_match(&workspace_code, &current.workspace_code, "automation")?;
        let output = self
            .automations
            .execute_now(input.automation_code.trim())
            .map_err(service_tool_error)?;
        json_observation(self.name(), &output)
    }
}

fn parse_automation_status(value: Option<&str>) -> Result<Option<AutomationStatus>, ToolError> {
    match normalize_optional_token(value).as_deref() {
        None => Ok(None),
        Some("active") => Ok(Some(AutomationStatus::Active)),
        Some("paused") => Ok(Some(AutomationStatus::Paused)),
        Some("archived") => Ok(Some(AutomationStatus::Archived)),
        Some(other) => Err(enum_tool_error("status", other)),
    }
}
