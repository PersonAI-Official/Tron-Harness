use super::internal_agents::normalize_string_codes;
use super::*;

impl AgentManagementService {
    /// Definition identity is independent from the workspace containing work.
    pub fn is_assistant_definition(&self, agent_code: &str) -> Result<bool, AgentManagementError> {
        Ok(self
            .repository()
            .list_assistant_codes()?
            .iter()
            .any(|code| code.eq_ignore_ascii_case(agent_code)))
    }

    /// Fresh installations start with a main Agent using the runtime's ReAct loop.
    pub fn initialize_default_main_agent(&self) -> Result<(), AgentManagementError> {
        let mut repository = self.repository();
        if repository
            .list_platform_internal_agent_configs()?
            .iter()
            .any(|config| config.agent_code == "main-agent")
        {
            return Ok(());
        }
        let config = default_config(internal_definitions::internal_agents().remove(0));
        repository.insert_platform_internal_agent_config(&config)
    }

    /// Initialize an independent PersonAI with a full copy of generic Agent policy.
    pub fn initialize_assistant_definition(
        &self,
        code: &str,
        name: &str,
        role_prompt: Option<String>,
    ) -> Result<(), AgentManagementError> {
        let configs = self.platform_internal_agent_configs()?;
        if configs.iter().any(|c| c.agent_code == code) {
            return self
                .repository()
                .copy_initial_agent_skills("main-agent", code);
        }
        let mut config = configs
            .into_iter()
            .find(|c| c.agent_code == "main-agent")
            .ok_or(AgentManagementError::InternalAgentNotFound)?;
        config.agent_code = code.into();
        config.max_steps = 0;
        config.max_tool_calls = 0;
        config.avatar_id = match code {
            "main-assistant" => "neo".into(),
            "memory-assistant" => "black-hole".into(),
            _ => config.avatar_id,
        };
        config.name = name.into();
        config.description = match code {
            "main-assistant" => "统筹用户事务，协调智能体执行任务并跟进结果。",
            "memory-assistant" => "整理稳定事实与已验证经验，维护长期记忆和可复用技能。",
            _ => "",
        }
        .into();
        config.group_name = None;
        restrict_desktop_assignment(&mut config);
        if let Some(prompt) = role_prompt {
            config.role_prompt = prompt
        }
        self.repository()
            .insert_platform_internal_agent_config(&config)?;
        self.repository()
            .copy_initial_agent_skills("main-agent", code)
    }

    pub(super) fn platform_internal_agent_configs(
        &self,
    ) -> Result<Vec<PlatformInternalAgentConfig>, AgentManagementError> {
        let mut configs = internal_definitions::internal_agents()
            .into_iter()
            .map(default_config)
            .collect::<Vec<_>>();
        for mut config in self.repository().list_platform_internal_agent_configs()? {
            if let Some(existing) = configs
                .iter_mut()
                .find(|existing| existing.agent_code == config.agent_code)
            {
                if config.agent_code.eq_ignore_ascii_case("main-agent")
                    && config.role_prompt.trim().is_empty()
                {
                    config.role_prompt = internal_definitions::default_main_agent_role_prompt();
                }
                *existing = config;
            } else {
                configs.push(config);
            }
        }
        Ok(configs)
    }

    pub fn list_platform_internal_agents(
        &self,
    ) -> Result<Vec<InternalAgentToolPolicy>, AgentManagementError> {
        let assistants = self.repository().list_assistant_codes()?;
        Ok(self
            .all_platform_internal_agents()?
            .into_iter()
            .filter(|p| !assistants.contains(&p.agent_code))
            .collect())
    }
    /// Identity-only listing must not build tool policies or resolve capabilities.
    pub fn platform_internal_agent_codes(&self) -> Result<Vec<String>, AgentManagementError> {
        Ok(self
            .platform_internal_agent_configs()?
            .into_iter()
            .map(|c| c.agent_code)
            .collect())
    }
    pub fn all_platform_internal_agents(
        &self,
    ) -> Result<Vec<InternalAgentToolPolicy>, AgentManagementError> {
        let tools = self.list_tools(None, None, "en-US");
        Ok(self
            .platform_internal_agent_configs()?
            .into_iter()
            .map(|config| platform_view(config, &tools))
            .collect())
    }

    pub fn prepare_platform_internal_agent(
        &self,
        command: CreateWorkspaceInternalAgentCommand,
    ) -> Result<PlatformInternalAgentConfig, AgentManagementError> {
        internal_definitions::validate_definition(&command.name, &command.role_prompt)?;
        let mut config = default_config(internal_definitions::internal_agents().remove(0));
        config.agent_code = self.validate_new_agent_code(&command.agent_code)?;
        config.group_name = self
            .validate_internal_agent_group("", command.group_name)?
            .filter(|name| !name.is_empty());
        config.name = command.name.trim().to_owned();
        config.description = internal_definitions::validate_description(&command.description)?;
        config.role_prompt = command.role_prompt;
        config.avatar_id = internal_definitions::validate_avatar_id(command.avatar_id.as_deref())?;
        config.candidate_model_codes =
            internal_definitions::normalize_candidate_model_codes(command.candidate_model_codes)?;
        Ok(config)
    }
    pub fn create_platform_internal_agent(
        &self,
        command: CreateWorkspaceInternalAgentCommand,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        let config = self.prepare_platform_internal_agent(command)?;
        self.repository()
            .insert_platform_internal_agent_config(&config)?;
        Ok(platform_view(config, &self.list_tools(None, None, "en-US")))
    }

    pub fn delete_platform_internal_agent(&self, code: &str) -> Result<(), AgentManagementError> {
        let code = code.trim();
        if self
            .repository()
            .list_assistant_codes()?
            .iter()
            .any(|d| d.eq_ignore_ascii_case(code))
        {
            return Err(AgentManagementError::PlatformAgentReadOnly);
        }
        if internal_definitions::is_builtin_agent(code) {
            return Err(AgentManagementError::BuiltinAgentReadOnly);
        }
        let Some(config) = self
            .repository()
            .list_platform_internal_agent_configs()?
            .into_iter()
            .find(|config| config.agent_code.eq_ignore_ascii_case(code))
        else {
            return Err(AgentManagementError::InternalAgentNotFound);
        };
        if !self
            .repository()
            .delete_platform_internal_agent_config(&config.agent_code)?
        {
            return Err(AgentManagementError::InternalAgentNotFound);
        }
        Ok(())
    }

    fn change_platform_internal_agent(
        &self,
        code: &str,
        change: impl FnOnce(&mut PlatformInternalAgentConfig) -> Result<(), AgentManagementError>,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        let mut repository = self.repository();
        let mut config = repository
            .list_platform_internal_agent_configs()?
            .into_iter()
            .find(|config| config.agent_code.eq_ignore_ascii_case(code))
            .or_else(|| {
                internal_definitions::internal_agents()
                    .into_iter()
                    .find(|agent| agent.agent_code.eq_ignore_ascii_case(code))
                    .map(default_config)
            })
            .ok_or(AgentManagementError::InternalAgentNotFound)?;
        change(&mut config)?;
        restrict_desktop_assignment(&mut config);
        repository.upsert_platform_internal_agent_config(&config)?;
        drop(repository);
        Ok(platform_view(config, &self.list_tools(None, None, "en-US")))
    }

    pub fn update_platform_internal_agent_definition(
        &self,
        code: &str,
        command: UpdateWorkspaceInternalAgentDefinitionCommand,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        if let Some(id) = &command.avatar_id {
            internal_definitions::validate_avatar_id(Some(id))?;
        }
        let group_name = if self.is_assistant_definition(code)? {
            Some(String::new())
        } else {
            self.validate_internal_agent_group("", command.group_name)?
        };
        self.change_platform_internal_agent(code, |config| {
            if let Some(enabled) = command.enabled {
                config.enabled = enabled;
            }
            if let Some(name) = group_name {
                config.group_name = (!name.is_empty()).then_some(name);
            }
            if let Some(codes) = command.candidate_model_codes {
                config.candidate_model_codes =
                    internal_definitions::normalize_candidate_model_codes(codes)?;
            }
            if let Some(id) = command.avatar_id {
                config.avatar_id = id;
            }
            if let Some(name) = command.name {
                config.name = name.trim().to_owned();
            }
            if let Some(description) = command.description {
                config.description = internal_definitions::validate_description(&description)?;
            }
            if let Some(prompt) = command.role_prompt {
                config.role_prompt = prompt;
            }
            internal_definitions::validate_definition(&config.name, &config.role_prompt)
        })
    }

    pub fn update_platform_internal_agent_policy(
        &self,
        code: &str,
        command: UpdateWorkspaceInternalAgentPolicyCommand,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        self.validate_selected_categories(&command.selected_category_codes)?;
        if !crate::domain::computer_use::allows(code)
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
        if command.max_parallel_tool_calls == Some(0) {
            return Err(AgentManagementError::InternalAgentPolicyInvalid);
        }
        self.change_platform_internal_agent(code, |config| {
            if let Some(value) = command.selected_category_codes {
                config.selected_category_codes = normalize_string_codes(value);
            }
            if let Some(value) = command.tool_assignment_mode {
                config.tool_assignment_mode = value;
            }
            if let Some(value) = command.max_steps {
                config.max_steps = value;
            }
            if let Some(value) = command.max_tool_calls {
                config.max_tool_calls = value;
            }
            if let Some(value) = command.max_parallel_tool_calls {
                config.max_parallel_tool_calls = value;
            }
            Ok(())
        })
    }

    pub fn update_platform_internal_agent_mcp_policy(
        &self,
        code: &str,
        command: UpdateWorkspaceInternalAgentMcpPolicyCommand,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        self.change_platform_internal_agent(code, |config| {
            if let Some(mode) = command.mcp_access_mode {
                config.mcp_access_mode = mode;
            }
            Ok(())
        })
    }

    pub fn replace_platform_internal_agent_tool_assignments(
        &self,
        code: &str,
        tools: Vec<String>,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        let _capability_guard = self.capabilities.get().map(|c| c.assignment_guard());
        // Dynamic catalog lookups also lock the repository. Validate before the
        // config mutation acquires its guard, and reject the whole selection on error.
        let tools = normalize_string_codes(tools);
        for tool_code in &tools {
            let tool = self
                .tool(tool_code, "en-US")
                .ok_or_else(|| AgentManagementError::InternalToolUnknown(tool_code.clone()))?;
            if !crate::domain::computer_use::tool_allowed(code, &tool) {
                return Err(AgentManagementError::InternalToolAssignmentStale(
                    tool_code.clone(),
                ));
            }
            if tool.availability_state != crate::domain::agent::ToolAvailabilityState::Available {
                return Err(AgentManagementError::InternalToolAssignmentStale(
                    tool_code.clone(),
                ));
            }
        }
        self.change_platform_internal_agent(code, |config| {
            config.selected_tool_codes = tools;
            Ok(())
        })
    }

    pub fn replace_platform_internal_agent_mcp_server_assignments(
        &self,
        code: &str,
        servers: Vec<String>,
    ) -> Result<InternalAgentToolPolicy, AgentManagementError> {
        let _capability_guard = self.capabilities.get().map(|c| c.assignment_guard());
        for server in &servers {
            if !self.mcp_assignable(server) {
                return Err(AgentManagementError::InternalMcpServerAssignmentStale(
                    server.clone(),
                ));
            }
        }
        self.change_platform_internal_agent(code, |config| {
            config.selected_server_codes = normalize_string_codes(servers);
            Ok(())
        })
    }
}

fn platform_view(
    config: PlatformInternalAgentConfig,
    catalog: &[ToolCatalogRecord],
) -> InternalAgentToolPolicy {
    let tools = catalog
        .iter()
        .filter(|tool| {
            crate::domain::computer_use::tool_allowed(&config.agent_code, tool)
                && config.tool_assignment_mode.selects_policy(
                    tool,
                    &config.selected_tool_codes,
                    &config.selected_category_codes,
                )
        })
        .map(|tool| tool.tool_code.clone())
        .collect::<Vec<_>>();
    let count = tools.len();
    InternalAgentToolPolicy {
        enabled: config.enabled,
        workspace_code: String::new(),
        agent_code: config.agent_code.clone(),
        name: config.name,
        description: config.description,
        role_prompt: config.role_prompt,
        avatar_id: config.avatar_id,
        candidate_model_codes: config.candidate_model_codes,
        group_name: config.group_name,
        platform_defined: true,
        built_in: internal_definitions::is_builtin_agent(&config.agent_code),
        source: InternalAgentPolicySource::PlatformDefault,
        tool_assignment_mode: config.tool_assignment_mode,
        selected_category_codes: config.selected_category_codes.clone(),
        mcp_access_mode: config.mcp_access_mode,
        max_steps: config.max_steps,
        max_tool_calls: config.max_tool_calls,
        max_parallel_tool_calls: config.max_parallel_tool_calls,
        selected_tool_codes: tools,
        selected_server_codes: config.selected_server_codes,
        assigned_tool_count: count,
        effective_tool_count: count,
        native_tool_scheme_code: None,
    }
}

fn restrict_desktop_assignment(config: &mut PlatformInternalAgentConfig) {
    if !crate::domain::computer_use::allows(&config.agent_code) {
        config
            .selected_tool_codes
            .retain(|code| !crate::domain::computer_use::is_tool(code, code, None, None));
        config
            .selected_category_codes
            .retain(|code| code != crate::domain::computer_use::DOMAIN);
        config
            .selected_server_codes
            .retain(|code| code != "tron.computer-use.cua-driver");
    }
}

fn default_config(
    definition: internal_definitions::InternalAgentDefinition,
) -> PlatformInternalAgentConfig {
    PlatformInternalAgentConfig {
        enabled: definition.enabled,
        agent_code: definition.agent_code,
        name: definition.name,
        description: definition.description,
        role_prompt: definition.role_prompt,
        avatar_id: definition.avatar_id,
        candidate_model_codes: definition.candidate_model_codes,
        group_name: definition.group_name,
        tool_assignment_mode: definition.tool_assignment_mode,
        selected_category_codes: definition.selected_category_codes.clone(),
        mcp_access_mode: definition.mcp_access_mode,
        max_steps: definition.max_steps,
        max_tool_calls: definition.max_tool_calls,
        max_parallel_tool_calls: definition.max_parallel_tool_calls,
        selected_tool_codes: definition.selected_tool_codes,
        selected_server_codes: definition.selected_server_codes,
    }
}
