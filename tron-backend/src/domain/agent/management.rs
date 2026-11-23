use serde_json::Value;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagedAgentType {
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolAssignmentMode {
    AllPlatformTools,
    #[serde(rename = "SelectedCategories", alias = "SELECTED_CATEGORIES")]
    SelectedCategories,
    SelectedTools,
    #[serde(rename = "AllReadOnlyTools", alias = "ALL_READ_ONLY_TOOLS")]
    AllReadOnlyTools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McpAccessMode {
    Disabled,
    AllEnabledServers,
    SelectedServers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InternalAgentPolicySource {
    PlatformDefault,
    WorkspaceOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInternalAgentPolicy {
    pub workspace_code: String,
    pub agent_code: String,
    pub tool_assignment_mode: ToolAssignmentMode,
    #[serde(default)]
    pub selected_category_codes: Vec<String>,
    pub mcp_access_mode: McpAccessMode,
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub max_parallel_tool_calls: u32,
    pub overridden_fields: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInternalAgentDefinition {
    pub workspace_code: String,
    pub agent_code: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub role_prompt: Option<String>,
    #[serde(default)]
    pub avatar_id: Option<String>,
    #[serde(default)]
    pub group_name: Option<String>,
    #[serde(default)]
    pub candidate_model_codes: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInternalAgentConfig {
    #[serde(default = "default_agent_enabled")]
    pub enabled: bool,
    pub agent_code: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub role_prompt: String,
    #[serde(default = "default_avatar_id")]
    pub avatar_id: String,
    #[serde(default)]
    pub group_name: Option<String>,
    #[serde(default)]
    pub candidate_model_codes: Vec<String>,
    pub tool_assignment_mode: ToolAssignmentMode,
    #[serde(default)]
    pub selected_category_codes: Vec<String>,
    pub mcp_access_mode: McpAccessMode,
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub max_parallel_tool_calls: u32,
    pub selected_tool_codes: Vec<String>,
    pub selected_server_codes: Vec<String>,
    /// Named native-exposure scheme; absent inherits the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_tool_scheme_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInternalAgentToolAssignment {
    pub workspace_code: String,
    pub agent_code: String,
    pub tool_code: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInternalAgentMcpServerAssignment {
    pub workspace_code: String,
    pub agent_code: String,
    pub server_code: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeToolScheme {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub tool_ids: Vec<String>,
    #[serde(default)]
    pub built_in: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeToolConfig {
    pub schemes: Vec<NativeToolScheme>,
    pub default_scheme_code: String,
    #[serde(default)]
    pub agent_overrides: std::collections::BTreeMap<String, String>,
}

impl Default for NativeToolConfig {
    fn default() -> Self {
        Self {
            schemes: vec![NativeToolScheme {
                code: "default".into(),
                name: "Default".into(),
                tool_ids: vec![
                    "command_exec".into(),
                    "command_exec_read".into(),
                    "command_exec_stop".into(),
                    "tool_search".into(),
                    "tool_schema".into(),
                    "tool_execute".into(),
                    "chat_ask_user".into(),
                    "skill_load".into(),
                ],
                built_in: true,
            }],
            default_scheme_code: "default".into(),
            agent_overrides: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternalAgentToolPolicy {
    #[serde(default = "default_agent_enabled")]
    pub enabled: bool,
    pub workspace_code: String,
    pub agent_code: String,
    pub name: String,
    pub description: String,
    pub role_prompt: String,
    #[serde(default = "default_avatar_id")]
    pub avatar_id: String,
    #[serde(default)]
    pub group_name: Option<String>,
    #[serde(default)]
    pub candidate_model_codes: Vec<String>,
    pub platform_defined: bool,
    #[serde(default)]
    pub built_in: bool,
    pub source: InternalAgentPolicySource,
    pub tool_assignment_mode: ToolAssignmentMode,
    #[serde(default)]
    pub selected_category_codes: Vec<String>,
    pub mcp_access_mode: McpAccessMode,
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub max_parallel_tool_calls: u32,
    pub selected_tool_codes: Vec<String>,
    pub selected_server_codes: Vec<String>,
    pub assigned_tool_count: usize,
    pub effective_tool_count: usize,
    /// Selected native-exposure scheme; None inherits the global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_tool_scheme_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentRuntimeType {
    React,
    CodexExec,
    CodexAppServer,
    OpenCodeRun,
    DshHeadless,
    OpenCodeServe,
    OpenCodeAcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagedAgentSource {
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SystemAgentStatus {
    Enabled,
    Disabled,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemAgentRegistryEntry {
    #[serde(default)]
    pub fast_mode: bool,
    pub agent_code: String,
    pub base_agent_code: Option<String>,
    pub model_id: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<crate::domain::reasoning::ReasoningEffort>,
    #[serde(default)]
    pub avatar_id: Option<String>,
    pub name: String,
    pub description: String,
    pub runtime_type: AgentRuntimeType,
    pub command_name: String,
    pub source: ManagedAgentSource,
    pub enabled: bool,
    pub available: bool,
    pub last_scanned_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSystemAgentRecord {
    #[serde(default)]
    pub fast_mode: bool,
    pub agent_code: String,
    pub base_agent_code: Option<String>,
    pub model_id: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<crate::domain::reasoning::ReasoningEffort>,
    #[serde(default)]
    pub avatar_id: Option<String>,
    pub name: String,
    pub description: String,
    pub agent_type: ManagedAgentType,
    pub runtime_type: AgentRuntimeType,
    pub source: ManagedAgentSource,
    pub inherited_from: Option<String>,
    pub platform_enabled: Option<bool>,
    pub workspace_override_disabled: Option<bool>,
    pub status: SystemAgentStatus,
    pub enabled: bool,
    pub available: bool,
    #[serde(default)]
    pub models: Vec<SystemAgentModelRecord>,
    pub last_scanned_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SystemAgentModelRecord {
    #[serde(default)]
    pub supported_reasoning_efforts: Vec<crate::domain::reasoning::ReasoningEffort>,
    pub model_id: String,
    pub display_name: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSystemAgentPolicy {
    pub workspace_code: String,
    pub agent_code: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemAgentSyncResult {
    pub scanned_count: usize,
    pub available_count: usize,
    pub unavailable_count: usize,
    pub records: Vec<ManagedSystemAgentRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAdapterCapabilityRecord {
    pub adapter_key: String,
    pub name: String,
    pub stage: String,
    pub available: bool,
    pub enabled: bool,
    pub supports_resume: bool,
    pub supports_cancel: bool,
    pub supports_steer: bool,
    pub supports_streaming: bool,
    pub supports_ephemeral: bool,
    pub supports_workdir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentManagementError {
    InvalidWorkspace,
    InternalAgentDefinitionInvalid,
    InternalAgentGroupInvalid,
    PlatformAgentReadOnly,
    BuiltinAgentReadOnly,
    InternalAgentCodeInvalid,
    InternalAgentCodeConflict,
    InternalAgentPolicyInvalid,
    InternalAgentNotFound,
    SystemAgentNotFound,
    InternalToolAssignmentStale(String),
    InternalMcpServerAssignmentStale(String),
    InternalToolUnknown(String),
    SystemAgentUnavailable,
    ToolPermissionInvalid,
    StorageFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolDomainRecord {
    pub domain_code: String,
    pub name: String,
    pub description: String,
    pub tool_count: usize,
    /// Exact invocation names (`call_name`), never catalog IDs.
    pub sample_tools: Vec<String>,
    pub has_mutation_tools: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolOperationKind {
    Query,
    Command,
    #[serde(rename = "Unknown", alias = "UNKNOWN")]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolAvailabilityState {
    Available,
    Disabled,
}

#[derive(Clone, Debug)]
pub struct ToolCatalogIssue {
    /// Discovery warnings (for example a rejected duplicate) do not disable a valid entry.
    pub blocks_tool: bool,
    pub tool_code: Option<String>,
    pub tool_name: Option<String>,
    pub provider_id: Option<String>,
    pub pack_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCatalogRecord {
    #[serde(default)]
    pub display: Option<eliteforge_agent_core::ToolDisplayMetadata>,
    pub tool_code: String,
    pub tool_name: String,
    pub domain_code: String,
    #[serde(default)]
    pub category: Option<eliteforge_agent_core::ToolCategory>,
    #[serde(default)]
    pub source: eliteforge_agent_core::ToolSource,
    pub name: String,
    pub description: String,
    pub operation_kind: ToolOperationKind,
    pub input_schema: Value,
    pub module_owner: String,
    pub availability_state: ToolAvailabilityState,
    pub sample_result_summary: String,
}

fn default_avatar_id() -> String {
    "robot".into()
}

fn default_agent_enabled() -> bool {
    true
}
