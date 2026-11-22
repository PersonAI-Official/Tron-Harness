mod agent_directory;
mod internal_agents;
mod internal_definition_error;
mod internal_definitions;
mod internal_groups;
mod native_tools;
pub(crate) mod platform_internal_agents;
mod platform_runtime;
mod system_agent_profiles;
mod system_agents;
mod tool_metadata;
mod tool_permissions;
mod workspace_grants;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::{
    BusinessToolCatalog,
    runtime_capability::{build_runtime_capabilities, find_runtime_capability},
    system_agent_model_probe::{ProcessSystemCommandProbe, SystemCommandProbe},
};

use self::system_agents::supported_system_agents_from_config;

use crate::{
    application::workspace::WorkspaceService,
    config::SystemAgentConfig,
    domain::{
        agent::{
            AgentManagementError, AgentRuntimeType, InternalAgentPolicySource,
            InternalAgentToolPolicy, ManagedAgentSource, ManagedAgentType,
            ManagedSystemAgentRecord, McpAccessMode, PlatformInternalAgentConfig,
            RuntimeAdapterCapabilityRecord, SystemAgentModelRecord, SystemAgentRegistryEntry,
            SystemAgentStatus, SystemAgentSyncResult, ToolAssignmentMode, ToolCatalogRecord,
            ToolDomainRecord, WorkspaceInternalAgentDefinition,
            WorkspaceInternalAgentMcpServerAssignment, WorkspaceInternalAgentPolicy,
            WorkspaceInternalAgentToolAssignment, WorkspaceSystemAgentPolicy,
        },
        workspace::{WorkspaceDetail, WorkspaceError, WorkspaceStatus},
    },
    shared::time::current_timestamp,
};

#[derive(Debug, Clone, Default)]
pub struct UpdateWorkspaceInternalAgentPolicyCommand {
    pub tool_assignment_mode: Option<ToolAssignmentMode>,
    /// Some(empty) clears the agent override; None keeps the current value.
    pub native_tool_scheme_code: Option<String>,
    pub selected_category_codes: Option<Vec<String>>,
    pub max_steps: Option<u32>,
    pub max_tool_calls: Option<u32>,
    pub max_parallel_tool_calls: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct CreateWorkspaceInternalAgentCommand {
    pub agent_code: String,
    pub name: String,
    pub description: String,
    pub role_prompt: String,
    pub avatar_id: Option<String>,
    /// Missing keeps the current group on update; an empty string clears it.
    pub group_name: Option<String>,
    pub candidate_model_codes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateWorkspaceInternalAgentDefinitionCommand {
    pub enabled: Option<bool>,
    pub candidate_model_codes: Option<Vec<String>>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub role_prompt: Option<String>,
    pub avatar_id: Option<String>,
    /// Missing keeps the current group on update; an empty string clears it.
    pub group_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateWorkspaceInternalAgentMcpPolicyCommand {
    pub mcp_access_mode: Option<McpAccessMode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAgentRuntime {
    pub agent_code: String,
    pub system_model_id: Option<String>,
    pub system_reasoning_effort: Option<crate::domain::reasoning::ReasoningEffort>,
    pub system_fast_mode: bool,
    pub runtime_type: AgentRuntimeType,
    pub command_name: Option<String>,
    pub name: String,
}

const MAIN_AGENT_CODE: &str = "main-agent";

pub trait AgentManagementRepository: Send {
    fn copy_initial_agent_skills(
        &mut self,
        _from: &str,
        _to: &str,
    ) -> Result<(), AgentManagementError> {
        Ok(())
    }

    fn list_assistant_codes(&mut self) -> Result<Vec<String>, AgentManagementError> {
        Ok(vec![])
    }

    fn replace_workspace_resource_assignments(
        &mut self,
        _policy: &WorkspaceInternalAgentPolicy,
        _mcp: bool,
        _codes: &[String],
    ) -> Result<(), AgentManagementError> {
        Err(AgentManagementError::StorageFailure)
    }

    fn list_internal_agent_groups(
        &mut self,
        scope: &str,
    ) -> Result<Vec<String>, AgentManagementError>;
    fn save_internal_agent_group(
        &mut self,
        scope: &str,
        name: &str,
    ) -> Result<(), AgentManagementError>;
    fn delete_internal_agent_group(
        &mut self,
        scope: &str,
        name: &str,
    ) -> Result<bool, AgentManagementError>;

    fn native_tool_config(&mut self) -> Result<Option<String>, AgentManagementError> {
        Ok(None)
    }
    fn replace_native_tool_config(
        &mut self,
        _config_json: &str,
    ) -> Result<(), AgentManagementError> {
        Ok(())
    }
    fn list_tool_permissions(
        &mut self,
    ) -> Result<Vec<tron_harness_capability::ToolPermissionSetting>, AgentManagementError> {
        Ok(vec![])
    }
    fn replace_tool_permissions(
        &mut self,
        _: &[tron_harness_capability::ToolPermissionSetting],
    ) -> Result<(), AgentManagementError> {
        Err(AgentManagementError::StorageFailure)
    }
    fn list_discovered_tools(&mut self) -> Result<Vec<ToolCatalogRecord>, AgentManagementError> {
        Ok(vec![])
    }
    fn remember_discovered_tool(
        &mut self,
        _: &ToolCatalogRecord,
    ) -> Result<(), AgentManagementError> {
        Ok(())
    }
    fn remember_discovered_tools(
        &mut self,
        tools: &[ToolCatalogRecord],
    ) -> Result<(), AgentManagementError> {
        for tool in tools {
            self.remember_discovered_tool(tool)?;
        }
        Ok(())
    }

    fn agent_workspace_grants(
        &mut self,
        _agent: &str,
    ) -> Result<Option<Vec<String>>, AgentManagementError> {
        Ok(None)
    }
    fn assigned_agent_codes(
        &mut self,
        workspace: &str,
        agents: &[String],
    ) -> Result<Vec<String>, AgentManagementError>;
    fn replace_agent_workspace_grants(
        &mut self,
        _agent: &str,
        _workspaces: &[String],
    ) -> Result<(), AgentManagementError> {
        Err(AgentManagementError::StorageFailure)
    }

    fn insert_platform_internal_agent_config(
        &mut self,
        config: &PlatformInternalAgentConfig,
    ) -> Result<(), AgentManagementError>;
    fn insert_workspace_internal_agent_definition(
        &mut self,
        definition: &WorkspaceInternalAgentDefinition,
    ) -> Result<(), AgentManagementError>;
    fn list_platform_internal_agent_configs(
        &mut self,
    ) -> Result<Vec<PlatformInternalAgentConfig>, AgentManagementError>;
    fn upsert_platform_internal_agent_config(
        &mut self,
        config: &PlatformInternalAgentConfig,
    ) -> Result<(), AgentManagementError>;
    fn delete_platform_internal_agent_config(
        &mut self,
        agent_code: &str,
    ) -> Result<bool, AgentManagementError>;
    fn delete_workspace_internal_agent(
        &mut self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<bool, AgentManagementError>;
    fn delete_workspace_internal_agent_definition(
        &mut self,
        workspace: &str,
        agent: &str,
    ) -> Result<(), AgentManagementError>;
    fn list_workspace_internal_agent_definitions(
        &mut self,
        workspace_code: &str,
    ) -> Result<Vec<WorkspaceInternalAgentDefinition>, AgentManagementError>;
    fn list_workspace_internal_agent_definitions_batch(
        &mut self,
        workspaces: &[String],
    ) -> Result<Vec<WorkspaceInternalAgentDefinition>, AgentManagementError> {
        let mut definitions = Vec::new();
        for workspace in workspaces {
            definitions.extend(self.list_workspace_internal_agent_definitions(workspace)?);
        }
        Ok(definitions)
    }
    fn upsert_workspace_internal_agent_definition(
        &mut self,
        definition: &WorkspaceInternalAgentDefinition,
    ) -> Result<(), AgentManagementError>;
    fn list_system_agents(&mut self)
    -> Result<Vec<SystemAgentRegistryEntry>, AgentManagementError>;
    fn upsert_system_agent(
        &mut self,
        agent: &SystemAgentRegistryEntry,
    ) -> Result<(), AgentManagementError>;
    fn insert_system_agent(
        &mut self,
        agent: &SystemAgentRegistryEntry,
    ) -> Result<(), AgentManagementError> {
        if self
            .list_system_agents()?
            .iter()
            .any(|record| record.agent_code == agent.agent_code)
        {
            return Err(AgentManagementError::InternalAgentCodeConflict);
        }
        self.upsert_system_agent(agent)
    }
    fn delete_system_agent(&mut self, agent_code: &str) -> Result<bool, AgentManagementError>;
    fn list_workspace_system_policies(
        &mut self,
        workspace_code: Option<&str>,
    ) -> Result<Vec<WorkspaceSystemAgentPolicy>, AgentManagementError>;
    fn upsert_workspace_system_policy(
        &mut self,
        policy: &WorkspaceSystemAgentPolicy,
    ) -> Result<(), AgentManagementError>;
    fn delete_workspace_system_policy(
        &mut self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<bool, AgentManagementError>;
    fn list_workspace_internal_agent_policies(
        &mut self,
        workspace_code: Option<&str>,
    ) -> Result<Vec<WorkspaceInternalAgentPolicy>, AgentManagementError>;
    fn upsert_workspace_internal_agent_policy(
        &mut self,
        policy: &WorkspaceInternalAgentPolicy,
    ) -> Result<(), AgentManagementError>;
    fn delete_workspace_internal_agent_policy(
        &mut self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<bool, AgentManagementError>;
    fn list_workspace_internal_agent_tool_assignments(
        &mut self,
        workspace_code: Option<&str>,
        agent_code: Option<&str>,
    ) -> Result<Vec<WorkspaceInternalAgentToolAssignment>, AgentManagementError>;
    fn upsert_workspace_internal_agent_tool_assignment(
        &mut self,
        assignment: &WorkspaceInternalAgentToolAssignment,
    ) -> Result<(), AgentManagementError>;
    fn delete_workspace_internal_agent_tool_assignments(
        &mut self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<(), AgentManagementError>;
    fn list_workspace_internal_agent_mcp_server_assignments(
        &mut self,
        workspace_code: Option<&str>,
        agent_code: Option<&str>,
    ) -> Result<Vec<WorkspaceInternalAgentMcpServerAssignment>, AgentManagementError>;
    fn upsert_workspace_internal_agent_mcp_server_assignment(
        &mut self,
        assignment: &WorkspaceInternalAgentMcpServerAssignment,
    ) -> Result<(), AgentManagementError>;
    fn delete_workspace_internal_agent_mcp_server_assignments(
        &mut self,
        workspace_code: &str,
        agent_code: &str,
    ) -> Result<(), AgentManagementError>;
}

#[derive(Debug, Clone)]
struct SupportedSystemAgentDefinition {
    agent_code: String,
    name: String,
    description: String,
    runtime_type: AgentRuntimeType,
    command_name: String,
}

#[derive(Clone)]
pub struct AgentManagementService {
    repository: Arc<Mutex<Box<dyn AgentManagementRepository>>>,
    tool_metadata: Arc<Mutex<tool_metadata::ToolMetadataCache>>,
    mcp: Arc<std::sync::OnceLock<crate::application::mcp::McpService>>,
    capabilities: Arc<std::sync::OnceLock<crate::application::capability::CapabilityResources>>,
    workspaces: WorkspaceService,
    supported_agents: Vec<SupportedSystemAgentDefinition>,
    probe: Arc<dyn SystemCommandProbe>,
    model_catalog: Arc<Mutex<HashMap<String, Vec<SystemAgentModelRecord>>>>,
}

impl AgentManagementService {
    pub fn new(
        repository: impl AgentManagementRepository + 'static,
        workspaces: WorkspaceService,
        config: &SystemAgentConfig,
    ) -> Self {
        Self::with_probe(
            repository,
            workspaces,
            supported_system_agents_from_config(config),
            Arc::new(ProcessSystemCommandProbe),
        )
    }

    fn with_probe(
        repository: impl AgentManagementRepository + 'static,
        workspaces: WorkspaceService,
        supported_agents: Vec<SupportedSystemAgentDefinition>,
        probe: Arc<dyn SystemCommandProbe>,
    ) -> Self {
        Self {
            repository: Arc::new(Mutex::new(Box::new(repository))),
            tool_metadata: Default::default(),
            workspaces,
            supported_agents,
            probe,
            model_catalog: Arc::new(Mutex::new(HashMap::new())),
            mcp: Default::default(),
            capabilities: Default::default(),
        }
    }

    pub fn bootstrap_registry(&self) -> Result<SystemAgentSyncResult, AgentManagementError> {
        self.rescan_system_agents()
    }

    pub fn list_tool_domains(&self, locale: &str) -> Vec<ToolDomainRecord> {
        BusinessToolCatalog::domains_for_records(&self.list_tools(None, None, locale), locale)
    }

    pub fn list_tools(
        &self,
        domain_code: Option<&str>,
        search: Option<&str>,
        locale: &str,
    ) -> Vec<ToolCatalogRecord> {
        let mut records = BusinessToolCatalog::list_tools(domain_code, search, locale);
        if let Ok(dynamic) = self.cached_discovered_tools() {
            let fixed_indexes: std::collections::HashMap<_, _> = records
                .iter()
                .enumerate()
                .map(|(i, record)| (record.tool_code.clone(), i))
                .collect();
            let a2ui_ids: std::collections::HashSet<_> =
                eliteforge_agent_official_plugin_a2ui::tool_descriptors()
                    .into_iter()
                    .map(|descriptor| descriptor.id)
                    .collect();
            for mut tool in dynamic {
                super::tool_catalog::localize_plugin_display(&mut tool, locale);
                // Recover domains for records written before providers declared them.
                if tool.domain_code == "native" {
                    if [
                        eliteforge_agent_core::TOOL_SEARCH_ID,
                        eliteforge_agent_core::TOOL_EXECUTE_ID,
                    ]
                    .contains(&tool.tool_code.as_str())
                    {
                        tool.domain_code = "tool".into();
                    } else if tool.source.provider_id.as_deref() == Some("skills.default") {
                        tool.domain_code = "skill".into();
                    } else if tool.source.provider_id.as_deref() == Some("tools.agents") {
                        tool.domain_code = if tool.tool_code.starts_with("thread.") {
                            "collaboration_thread"
                        } else {
                            "subagent"
                        }
                        .into();
                    } else if tool.tool_code == format!("tron.{}", super::CHAT_ASK_USER_TOOL) {
                        tool.domain_code = "user_interaction".into();
                    }
                }
                if [
                    eliteforge_agent_core::TOOL_SEARCH_ID,
                    eliteforge_agent_core::TOOL_EXECUTE_ID,
                ]
                .contains(&tool.tool_code.as_str())
                {
                    tool.category = Some(super::tool_catalog::builtin_category("execution"));
                }
                // Older discoveries can lack metadata. Use the installed provider's
                // own static catalog to recover it without guessing from names.
                if a2ui_ids.contains(&tool.tool_code) {
                    if tool.category.is_none() {
                        tool.category =
                            Some(eliteforge_agent_official_plugin_a2ui::tool_category());
                    }
                    if tool.source.provider_id.is_none() {
                        tool.source.provider_id = Some("A2uiService".into());
                    }
                    if tool.source.plugin_id.is_none() {
                        tool.source.plugin_id = Some("agent.a2ui".into());
                    }
                    if tool.source.pack_id.is_none() {
                        tool.source.pack_id =
                            Some(eliteforge_agent_official_plugin_a2ui::A2UI_PACK_ID.into());
                    }
                }
                if tool.tool_code == format!("tron.{}", super::CHAT_ASK_USER_TOOL)
                    && tool.category.is_none()
                {
                    tool.category = Some(super::tool_catalog::builtin_category("interaction"));
                }
                if let Some(index) = fixed_indexes.get(&tool.tool_code) {
                    let fixed = &mut records[*index];
                    if tool.display.is_some() {
                        fixed.display = tool.display.clone();
                    }
                    if tool.category.is_some() {
                        fixed.category = tool.category;
                    }
                    if fixed
                        .category
                        .as_ref()
                        .is_some_and(|category| category.code == "tron.platform")
                    {
                        // The host provider is declared by the product catalog;
                        // registration adds ownership without replacing that origin.
                        if tool.source.plugin_id.is_some() {
                            fixed.source.plugin_id = tool.source.plugin_id;
                        }
                        if tool.source.pack_id.is_some() {
                            fixed.source.pack_id = tool.source.pack_id;
                        }
                        if tool.source.adapter_id.is_some() {
                            fixed.source.adapter_id = tool.source.adapter_id;
                        }
                    } else if tool.source != Default::default() {
                        fixed.source = tool.source;
                    }
                } else if !super::tool_catalog::is_fixed_code(&tool.tool_code)
                    && domain_code.is_none_or(|domain| tool.domain_code == domain)
                    && search.is_none_or(|query| {
                        format!("{} {} {}", tool.tool_code, tool.name, tool.description)
                            .to_lowercase()
                            .contains(&query.to_lowercase())
                    })
                {
                    records.push(tool);
                }
            }
        }
        for tool in &mut records {
            super::tool_catalog::localize_plugin_display(tool, locale);
        }
        if let Some(capabilities) = self.capabilities.get() {
            for tool in &mut records {
                if !capabilities.available("tool", &tool.tool_code) {
                    tool.availability_state = crate::domain::agent::ToolAvailabilityState::Disabled;
                }
            }
        }
        records
    }

    pub fn bind_mcp(&self, mcp: crate::application::mcp::McpService) {
        let _ = self.mcp.set(mcp);
    }
    pub(super) fn mcp_assignable(&self, code: &str) -> bool {
        self.capability_available("mcp", code)
            && self
                .mcp
                .get()
                .is_none_or(|m| m.get_platform_server(code).is_ok_and(|s| s.enabled))
    }
    pub fn bind_capabilities(
        &self,
        resources: crate::application::capability::CapabilityResources,
    ) {
        let _ = self.capabilities.set(resources);
    }
    pub fn capability_resource(
        &self,
        kind: &str,
        code: &str,
    ) -> Option<crate::application::capability::CapabilityResource> {
        self.capabilities.get()?.resource(kind, code).ok().flatten()
    }
    pub fn capability_available(&self, kind: &str, code: &str) -> bool {
        self.capabilities
            .get()
            .is_none_or(|c| c.available(kind, code))
    }
    pub fn persist_internal_agent_defaults(&self) -> Result<(), AgentManagementError> {
        for config in self.platform_internal_agent_configs()? {
            self.repository()
                .upsert_platform_internal_agent_config(&config)?;
        }
        Ok(())
    }

    pub fn tool(&self, tool_code: &str, locale: &str) -> Option<ToolCatalogRecord> {
        self.list_tools(None, None, locale)
            .into_iter()
            .find(|t| t.tool_code == tool_code)
    }

    /// Resolve one invocation against a fresh catalog snapshot. Rebuilding the
    /// complete catalog for every assigned code makes all-tools agents quadratic.
    pub(super) fn tools_by_codes(&self, codes: &[String], locale: &str) -> Vec<ToolCatalogRecord> {
        let mut catalog = std::collections::HashMap::new();
        for tool in self.list_tools(None, None, locale) {
            // `tool()` selects the first matching catalog entry. Keep that
            // precedence if a discovered resource repeats a built-in code.
            catalog.entry(tool.tool_code.clone()).or_insert(tool);
        }
        codes
            .iter()
            .filter_map(|code| catalog.get(code).cloned())
            .collect()
    }

    fn active_workspace_detail(
        &self,
        workspace_code: &str,
    ) -> Result<WorkspaceDetail, AgentManagementError> {
        let workspace = self
            .workspaces
            .get_workspace(workspace_code)
            .map_err(map_workspace_error)?;
        if workspace.status != WorkspaceStatus::Active {
            return Err(AgentManagementError::InvalidWorkspace);
        }
        Ok(workspace)
    }

    fn repository(&self) -> std::sync::MutexGuard<'_, Box<dyn AgentManagementRepository>> {
        self.repository
            .lock()
            .expect("agent management repository mutex poisoned")
    }

    fn system_agent_models(&self, agent_code: &str) -> Vec<SystemAgentModelRecord> {
        self.model_catalog
            .lock()
            .expect("agent management model catalog mutex poisoned")
            .get(agent_code)
            .cloned()
            .unwrap_or_default()
    }

    fn set_system_agent_models(&self, agent_code: &str, models: Vec<SystemAgentModelRecord>) {
        self.model_catalog
            .lock()
            .expect("agent management model catalog mutex poisoned")
            .insert(agent_code.to_string(), models);
    }
}

fn map_workspace_error(error: WorkspaceError) -> AgentManagementError {
    match error {
        WorkspaceError::NotFound | WorkspaceError::InvalidWorkspaceCode => {
            AgentManagementError::InvalidWorkspace
        }
        _ => AgentManagementError::StorageFailure,
    }
}
