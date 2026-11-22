use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListToolsQuery {
    pub domain_code: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateInternalAgentPolicyRequest {
    pub tool_assignment_mode: Option<String>,
    /// Empty string clears the per-Agent override; absent keeps the current one.
    pub native_tool_scheme_code: Option<String>,
    pub selected_category_codes: Option<Vec<String>>,
    pub max_steps: Option<u32>,
    pub max_tool_calls: Option<u32>,
    pub max_parallel_tool_calls: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceInternalAgentToolAssignmentsRequest {
    pub tool_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateInternalAgentMcpPolicyRequest {
    pub mcp_access_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceInternalAgentMcpServerAssignmentsRequest {
    pub server_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceNativeToolConfigRequest {
    pub schemes: Vec<crate::domain::agent::NativeToolScheme>,
    pub default_scheme_code: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAgentNativeSchemeRequest {
    pub scheme_code: Option<String>,
}
