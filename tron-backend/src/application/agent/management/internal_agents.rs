use std::collections::HashMap;

use super::internal_definitions::InternalAgentDefinition;
use super::*;

impl AgentManagementService {
    pub fn delete_internal_agent(
        &self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<(), AgentManagementError> {
        self.active_workspace_detail(workspace_code)?;
        let agent_code = agent_code.trim();
        if super::internal_definitions::is_builtin_agent(agent_code) {
            return Err(AgentManagementError::BuiltinAgentReadOnly);
        }
        if self
            .repository()
            .list_platform_internal_agent_configs()?
            .iter()
            .any(|config| config.agent_code.eq_ignore_ascii_case(agent_code))
        {
            return Err(AgentManagementError::PlatformAgentReadOnly);
        }
        let definition = self
            .repository()
            .list_workspace_internal_agent_definitions(workspace_code)?
            .into_iter()
            .find(|definition| definition.agent_code.eq_ignore_ascii_case(agent_code))
            .ok_or(AgentManagementError::InternalAgentNotFound)?;
        if !self
            .repository()
            .delete_workspace_internal_agent(workspace_code, &definition.agent_code)?
        {
            return Err(AgentManagementError::InternalAgentNotFound);
        }
        self.clear_agent_native_scheme(workspace_code, &definition.agent_code)?;
        Ok(())
    }

    pub fn list_workspace_internal_agents(
        &self,
        workspace_code: &str,
    ) -> Result<Vec<InternalAgentToolPolicy>, AgentManagementError> {
        self.active_workspace_detail(workspace_code)?;
        let overrides = self
            .repository()
            .list_workspace_internal_agent_policies(Some(workspace_code))?
            .into_iter()
            .map(|policy| (policy.agent_code.clone(), policy))
            .collect::<HashMap<_, _>>();
        let mcp_assignments = self
            .repository()
            .list_workspace_internal_agent_mcp_server_assignments(Some(workspace_code), None)?;

        let mut selected_server_codes_by_agent = HashMap::<String, Vec<String>>::new();

        for assignment in mcp_assignments {
            selected_server_codes_by_agent
                .entry(assignment.agent_code)
                .or_default()
                .push(assignment.server_code);
        }
        let tools = self.list_tools(None, None, "en-US");
        let local_definitions = self
            .repository()
            .list_workspace_internal_agent_definitions(workspace_code)?;
        let mut selected_tools_by_agent = HashMap::<String, Vec<String>>::new();
        for assignment in self
            .repository()
            .list_workspace_internal_agent_tool_assignments(Some(workspace_code), None)?
        {
            selected_tools_by_agent
                .entry(assignment.agent_code)
                .or_default()
                .push(assignment.tool_code);
        }
        let native_overrides = self.native_tool_config()?.agent_overrides;
        let mut policies = Vec::new();
        let assistants = self.repository().list_assistant_codes()?;
        for definition in self.internal_agent_definitions(workspace_code)? {
            if assistants.contains(&definition.agent_code) {
                continue;
            }
            let agent_code = definition.agent_code.clone();
            let override_policy = overrides.get(&definition.agent_code);
            let default_policy = to_internal_policy_record(workspace_code, &definition);
            let policy = merge_policy(default_policy, override_policy);
            let mode = policy.tool_assignment_mode;
            let selected = if policy
                .overridden_fields
                .iter()
                .any(|f| f == "selectedToolCodes")
            {
                selected_tools_by_agent
                    .remove(&agent_code)
                    .unwrap_or_default()
            } else {
                definition.selected_tool_codes.clone()
            };
            let selected_tool_codes = normalize_string_codes(
                tools
                    .iter()
                    .filter(|tool| {
                        crate::domain::computer_use::tool_allowed(&agent_code, tool)
                            && mode.includes_policy(
                                tool,
                                &selected,
                                &policy.selected_category_codes,
                            )
                    })
                    .map(|tool| tool.tool_code.clone())
                    .collect(),
            );
            let effective_tool_count = selected_tool_codes.len();
            let assigned_tool_count = effective_tool_count;
            policies.push(InternalAgentToolPolicy {
                enabled: definition.enabled,
                workspace_code: workspace_code.to_string(),
                agent_code: agent_code.clone(),
                name: definition.name,
                description: definition.description,
                role_prompt: definition.role_prompt,
                avatar_id: definition.avatar_id,
                candidate_model_codes: definition.candidate_model_codes,
                group_name: definition.group_name,
                platform_defined: definition.platform_defined,
                built_in: definition.built_in,
                source: (override_policy.is_some()
                    || !definition.platform_defined
                    || local_definitions
                        .iter()
                        .any(|record| record.agent_code == agent_code))
                .then_some(InternalAgentPolicySource::WorkspaceOverride)
                .unwrap_or(InternalAgentPolicySource::PlatformDefault),
                tool_assignment_mode: mode,
                selected_category_codes: policy.selected_category_codes.clone(),
                mcp_access_mode: policy.mcp_access_mode,
                max_steps: policy.max_steps,
                max_tool_calls: policy.max_tool_calls,
                max_parallel_tool_calls: policy.max_parallel_tool_calls,
                selected_tool_codes,
                selected_server_codes: normalize_string_codes(
                    if policy
                        .overridden_fields
                        .iter()
                        .any(|field| field == "selectedServerCodes")
                    {
                        selected_server_codes_by_agent
                            .remove(&agent_code)
                            .unwrap_or_default()
                    } else {
                        definition.selected_server_codes
                    },
                ),
                assigned_tool_count,
                effective_tool_count,
                native_tool_scheme_code: native_overrides
                    .get(&super::native_tools::override_key(
                        workspace_code,
                        &agent_code,
                    ))
                    .cloned(),
            })
        }
        Ok(policies)
    }

    pub fn workspace_internal_agent_policy(
        &self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        self.list_workspace_internal_agents(workspace_code)?
            .into_iter()
            .find(|policy| policy.agent_code.eq_ignore_ascii_case(agent_code))
            .ok_or(AgentManagementError::InternalAgentNotFound)
    }

    pub fn update_internal_agent_policy(
        &self,
        workspace_code: &str,
        agent_code: &str,
        command: UpdateWorkspaceInternalAgentPolicyCommand,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        if command.max_parallel_tool_calls == Some(0) {
            return Err(AgentManagementError::InternalAgentPolicyInvalid);
        }
        if let Some(code) = command.native_tool_scheme_code.as_deref() {
            self.validate_native_scheme(code)?;
        }
        self.validate_selected_categories(&command.selected_category_codes)?;
        if !crate::domain::computer_use::allows(agent_code)
            && command
                .selected_category_codes
                .as_ref()
                .is_some_and(|categories| {
                    categories
                        .iter()
                        .any(|code| code == crate::domain::computer_use::DOMAIN)
                })
        {
            return Err(AgentManagementError::InternalAgentPolicyInvalid);
        }
        self.active_workspace_detail(workspace_code)?;
        let definition = self
            .resolve_internal_agent_default(workspace_code, agent_code)?
            .ok_or(AgentManagementError::InternalAgentNotFound)?;
        let agent_code = definition.agent_code.as_str();
        let current = self.workspace_internal_agent_policy(workspace_code, agent_code)?;
        let mut overridden_fields = self.saved_override_fields(workspace_code, agent_code)?;
        for (field, changed) in [
            ("toolAssignmentMode", command.tool_assignment_mode.is_some()),
            (
                "selectedCategoryCodes",
                command.selected_category_codes.is_some(),
            ),
            ("maxSteps", command.max_steps.is_some()),
            ("maxToolCalls", command.max_tool_calls.is_some()),
            (
                "maxParallelToolCalls",
                command.max_parallel_tool_calls.is_some(),
            ),
        ] {
            if changed && !overridden_fields.iter().any(|existing| existing == field) {
                overridden_fields.push(field.into());
            }
        }
        let now = current_timestamp();
        let updated = WorkspaceInternalAgentPolicy {
            workspace_code: workspace_code.to_string(),
            agent_code: definition.agent_code.clone(),
            selected_category_codes: command
                .selected_category_codes
                .map(normalize_string_codes)
                .unwrap_or(current.selected_category_codes.clone()),
            tool_assignment_mode: command
                .tool_assignment_mode
                .unwrap_or(current.tool_assignment_mode),
            mcp_access_mode: current.mcp_access_mode,
            max_steps: command.max_steps.unwrap_or(current.max_steps),
            max_tool_calls: command.max_tool_calls.unwrap_or(current.max_tool_calls),
            max_parallel_tool_calls: command
                .max_parallel_tool_calls
                .unwrap_or(current.max_parallel_tool_calls),
            overridden_fields,
            created_at: current_timestamp(),
            updated_at: now,
        };
        let previous_created_at = self
            .repository()
            .list_workspace_internal_agent_policies(Some(workspace_code))?
            .into_iter()
            .find(|policy| policy.agent_code.eq_ignore_ascii_case(agent_code))
            .map(|policy| policy.created_at)
            .unwrap_or_else(|| updated.created_at.clone());
        let updated_policy = WorkspaceInternalAgentPolicy {
            created_at: previous_created_at,
            ..updated
        };
        self.repository()
            .upsert_workspace_internal_agent_policy(&updated_policy)?;
        if let Some(code) = command.native_tool_scheme_code.as_deref() {
            self.set_agent_native_scheme(
                workspace_code,
                agent_code,
                if code.trim().is_empty() {
                    None
                } else {
                    Some(code)
                },
            )?;
        }
        self.workspace_internal_agent_policy(workspace_code, agent_code)
    }

    pub fn update_internal_agent_mcp_policy(
        &self,
        workspace_code: &str,
        agent_code: &str,
        command: UpdateWorkspaceInternalAgentMcpPolicyCommand,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        self.active_workspace_detail(workspace_code)?;
        let definition = self
            .resolve_internal_agent_default(workspace_code, agent_code)?
            .ok_or(AgentManagementError::InternalAgentNotFound)?;
        let agent_code = definition.agent_code.as_str();
        let current = self.workspace_internal_agent_policy(workspace_code, agent_code)?;
        let mut overridden_fields = self.saved_override_fields(workspace_code, agent_code)?;
        if command.mcp_access_mode.is_some()
            && !overridden_fields
                .iter()
                .any(|field| field == "mcpAccessMode")
        {
            overridden_fields.push("mcpAccessMode".into());
        }
        let previous_created_at = self
            .repository()
            .list_workspace_internal_agent_policies(Some(workspace_code))?
            .into_iter()
            .find(|policy| policy.agent_code.eq_ignore_ascii_case(agent_code))
            .map(|policy| policy.created_at)
            .unwrap_or_else(current_timestamp);
        let updated = WorkspaceInternalAgentPolicy {
            workspace_code: workspace_code.to_string(),
            agent_code: definition.agent_code.clone(),
            tool_assignment_mode: current.tool_assignment_mode,
            selected_category_codes: current.selected_category_codes.clone(),
            mcp_access_mode: command.mcp_access_mode.unwrap_or(current.mcp_access_mode),
            max_steps: current.max_steps,
            max_tool_calls: current.max_tool_calls,
            max_parallel_tool_calls: current.max_parallel_tool_calls,
            overridden_fields,
            created_at: previous_created_at,
            updated_at: current_timestamp(),
        };
        self.repository()
            .upsert_workspace_internal_agent_policy(&updated)?;
        self.workspace_internal_agent_policy(workspace_code, agent_code)
    }

    pub fn replace_internal_agent_tool_assignments(
        &self,
        workspace_code: &str,
        agent_code: &str,
        mut tool_codes: Vec<String>,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        let _capability_guard = self.capabilities.get().map(|c| c.assignment_guard());
        self.active_workspace_detail(workspace_code)?;
        let definition = self
            .resolve_internal_agent_default(workspace_code, agent_code)?
            .ok_or(AgentManagementError::InternalAgentNotFound)?;
        let agent_code = definition.agent_code.as_str();
        let mut normalized = Vec::new();
        for tool_code in tool_codes.drain(..) {
            let trimmed = tool_code.trim().to_string();
            if !trimmed.is_empty() && !normalized.iter().any(|existing| existing == &trimmed) {
                let Some(tool_definition) = self.tool(&trimmed, "en-US") else {
                    return Err(AgentManagementError::InternalToolUnknown(trimmed));
                };
                if !crate::domain::computer_use::tool_allowed(agent_code, &tool_definition) {
                    return Err(AgentManagementError::InternalToolAssignmentStale(trimmed));
                }
                if tool_definition.availability_state
                    != crate::domain::agent::ToolAvailabilityState::Available
                {
                    return Err(AgentManagementError::InternalToolAssignmentStale(trimmed));
                }
                normalized.push(trimmed);
            }
        }
        let policy =
            self.assignment_override_record(workspace_code, agent_code, "selectedToolCodes")?;
        self.repository()
            .replace_workspace_resource_assignments(&policy, false, &normalized)?;
        self.workspace_internal_agent_policy(workspace_code, agent_code)
    }

    pub fn reset_internal_agent_policy(
        &self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        self.active_workspace_detail(workspace_code)?;
        let definition = self
            .resolve_internal_agent_default(workspace_code, agent_code)?
            .ok_or(AgentManagementError::InternalAgentNotFound)?;
        let agent_code = definition.agent_code.as_str();
        if definition.platform_defined && !definition.built_in {
            self.repository()
                .delete_workspace_internal_agent_definition(workspace_code, agent_code)?;
        }
        let _ = self
            .repository()
            .delete_workspace_internal_agent_policy(workspace_code, agent_code)?;
        self.repository()
            .delete_workspace_internal_agent_tool_assignments(workspace_code, agent_code)?;
        self.repository()
            .delete_workspace_internal_agent_mcp_server_assignments(workspace_code, agent_code)?;
        self.clear_agent_native_scheme(workspace_code, agent_code)?;
        self.workspace_internal_agent_policy(workspace_code, agent_code)
    }

    pub fn replace_internal_agent_mcp_server_assignments(
        &self,
        workspace_code: &str,
        agent_code: &str,
        mut server_codes: Vec<String>,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        let _capability_guard = self.capabilities.get().map(|c| c.assignment_guard());
        self.active_workspace_detail(workspace_code)?;
        let definition = self
            .resolve_internal_agent_default(workspace_code, agent_code)?
            .ok_or(AgentManagementError::InternalAgentNotFound)?;
        let agent_code = definition.agent_code.as_str();
        let normalized = normalize_string_codes(server_codes.drain(..).collect());
        for code in &normalized {
            if !self.mcp_assignable(code) {
                return Err(AgentManagementError::InternalMcpServerAssignmentStale(
                    code.clone(),
                ));
            }
        }
        let policy =
            self.assignment_override_record(workspace_code, agent_code, "selectedServerCodes")?;
        self.repository()
            .replace_workspace_resource_assignments(&policy, true, &normalized)?;
        self.workspace_internal_agent_policy(workspace_code, agent_code)
    }

    pub fn resolve_internal_agent_effective_tool_definitions(
        &self,
        workspace_code: &str,
        agent_code: &str,
        locale: &str,
    ) -> Result<Vec<ToolCatalogRecord>, AgentManagementError> {
        let tool_codes = self
            .workspace_internal_agent_policy(workspace_code, agent_code)?
            .selected_tool_codes;
        Ok(self
            .tools_by_codes(&tool_codes, locale)
            .into_iter()
            .filter(|tool| {
                tool.availability_state == crate::domain::agent::ToolAvailabilityState::Available
            })
            .collect())
    }

    pub(super) fn resolve_internal_agent_default(
        &self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<Option<InternalAgentDefinition>, AgentManagementError> {
        Ok(self
            .internal_agent_definitions(workspace_code)?
            .into_iter()
            .find(|definition| definition.agent_code.eq_ignore_ascii_case(agent_code)))
    }
    fn saved_override_fields(
        &self,
        workspace: &str,
        agent: &str,
    ) -> Result<Vec<String>, AgentManagementError> {
        Ok(self
            .repository()
            .list_workspace_internal_agent_policies(Some(workspace))?
            .into_iter()
            .find(|record| record.agent_code == agent)
            .map(|record| record.overridden_fields)
            .unwrap_or_default())
    }

    fn assignment_override_record(
        &self,
        workspace: &str,
        agent: &str,
        field: &str,
    ) -> Result<WorkspaceInternalAgentPolicy, AgentManagementError> {
        let definition = self
            .resolve_internal_agent_default(workspace, agent)?
            .ok_or(AgentManagementError::InternalAgentNotFound)?;
        let mut record = self
            .repository()
            .list_workspace_internal_agent_policies(Some(workspace))?
            .into_iter()
            .find(|record| record.agent_code == agent)
            .unwrap_or_else(|| to_internal_policy_record(workspace, &definition));
        if !record
            .overridden_fields
            .iter()
            .any(|existing| existing == field)
        {
            record.overridden_fields.push(field.into());
        }
        Ok(record)
    }
}

fn merge_policy(
    mut base: WorkspaceInternalAgentPolicy,
    saved: Option<&WorkspaceInternalAgentPolicy>,
) -> WorkspaceInternalAgentPolicy {
    if let Some(saved) = saved {
        for field in &saved.overridden_fields {
            match field.as_str() {
                "toolAssignmentMode" => base.tool_assignment_mode = saved.tool_assignment_mode,
                "selectedCategoryCodes" => {
                    base.selected_category_codes = saved.selected_category_codes.clone()
                }
                "mcpAccessMode" => base.mcp_access_mode = saved.mcp_access_mode,
                "maxSteps" => base.max_steps = saved.max_steps,
                "maxToolCalls" => base.max_tool_calls = saved.max_tool_calls,
                "maxParallelToolCalls" => {
                    base.max_parallel_tool_calls = saved.max_parallel_tool_calls
                }
                _ => {}
            }
        }
        base.overridden_fields = saved.overridden_fields.clone();
    }
    base
}

fn to_internal_policy_record(
    workspace_code: &str,
    definition: &InternalAgentDefinition,
) -> WorkspaceInternalAgentPolicy {
    WorkspaceInternalAgentPolicy {
        workspace_code: workspace_code.to_string(),
        agent_code: definition.agent_code.clone(),
        tool_assignment_mode: definition.tool_assignment_mode,
        selected_category_codes: definition.selected_category_codes.clone(),
        mcp_access_mode: definition.mcp_access_mode,
        max_steps: definition.max_steps,
        max_tool_calls: definition.max_tool_calls,
        max_parallel_tool_calls: definition.max_parallel_tool_calls,
        overridden_fields: vec![],
        created_at: current_timestamp(),
        updated_at: current_timestamp(),
    }
}

pub(super) fn normalize_string_codes(values: Vec<String>) -> Vec<String> {
    let mut normalized = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized.sort_unstable();
    normalized.dedup();
    normalized
}
