use super::{
    internal_definitions::InternalAgentApiError,
    request::{UpdateInternalAgentMcpPolicyRequest, UpdateInternalAgentPolicyRequest},
};
use crate::{
    adapters::inbound::http::auth::RequestAuthContext,
    app::AppState,
    application::agent::{
        CreateWorkspaceInternalAgentCommand, UpdateWorkspaceInternalAgentDefinitionCommand,
    },
    domain::agent::{
        AgentManagementError, InternalAgentPolicySource, InternalAgentToolPolicy, McpAccessMode,
        ToolAssignmentMode,
    },
    response::ApiResponse,
};
use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
    response::Response,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreatePlatformInternalAgentRequest {
    agent_code: String,
    name: String,
    #[serde(default)]
    description: String,
    role_prompt: String,
    avatar_id: Option<String>,
    group_name: Option<String>,
    #[serde(default)]
    candidate_model_codes: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdatePlatformInternalAgentDefinitionRequest {
    enabled: Option<bool>,
    name: Option<String>,
    description: Option<String>,
    role_prompt: Option<String>,
    avatar_id: Option<String>,
    group_name: Option<String>,
    #[serde(default)]
    candidate_model_codes: Option<Vec<String>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplacePlatformInternalAgentToolsRequest {
    tool_codes: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplacePlatformInternalAgentServersRequest {
    server_codes: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInternalAgentView {
    enabled: bool,
    workspace_code: String,
    agent_code: String,
    name: String,
    description: String,
    role_prompt: String,
    avatar_id: String,
    group_name: Option<String>,
    #[serde(default)]
    candidate_model_codes: Vec<String>,
    platform_defined: bool,
    built_in: bool,
    source: InternalAgentPolicySource,
    tool_assignment_mode: ToolAssignmentMode,
    #[serde(default)]
    selected_category_codes: Vec<String>,
    mcp_access_mode: McpAccessMode,
    max_steps: u32,
    max_tool_calls: u32,
    max_parallel_tool_calls: u32,
    selected_tool_codes: Vec<String>,
    selected_server_codes: Vec<String>,
    assigned_tool_count: usize,
    effective_tool_count: usize,
}
impl From<InternalAgentToolPolicy> for PlatformInternalAgentView {
    fn from(record: InternalAgentToolPolicy) -> Self {
        Self {
            enabled: record.enabled,
            workspace_code: record.workspace_code,
            agent_code: record.agent_code,
            name: record.name,
            description: record.description,
            role_prompt: record.role_prompt,
            avatar_id: record.avatar_id,
            group_name: record.group_name,
            candidate_model_codes: record.candidate_model_codes,
            platform_defined: record.platform_defined,
            built_in: record.built_in,
            source: record.source,
            tool_assignment_mode: record.tool_assignment_mode,
            selected_category_codes: record.selected_category_codes.clone(),
            mcp_access_mode: record.mcp_access_mode,
            max_steps: record.max_steps,
            max_tool_calls: record.max_tool_calls,
            max_parallel_tool_calls: record.max_parallel_tool_calls,
            selected_tool_codes: record.selected_tool_codes,
            selected_server_codes: record.selected_server_codes,
            assigned_tool_count: record.assigned_tool_count,
            effective_tool_count: record.effective_tool_count,
        }
    }
}
macro_rules! endpoint_response {
    ($($name:ident),+ $(,)?) => { $(
        #[derive(Serialize)]
        #[serde(transparent)]
        pub struct $name(PlatformInternalAgentView);
        impl From<InternalAgentToolPolicy> for $name {
            fn from(record: InternalAgentToolPolicy) -> Self { Self(record.into()) }
        }
    )+ };
}
endpoint_response!(
    ListPlatformInternalAgentResponse,
    GetPlatformInternalAgentResponse,
    CreatePlatformInternalAgentResponse,
    UpdatePlatformInternalAgentDefinitionResponse,
    UpdatePlatformInternalAgentPolicyResponse,
    UpdatePlatformInternalAgentMcpPolicyResponse,
    ReplacePlatformInternalAgentToolsResponse,
    ReplacePlatformInternalAgentServersResponse
);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdatePlatformInternalAgentPolicyRequest {
    tool_assignment_mode: Option<String>,
    selected_category_codes: Option<Vec<String>>,
    max_steps: Option<u32>,
    max_tool_calls: Option<u32>,
    max_parallel_tool_calls: Option<u32>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdatePlatformInternalAgentMcpPolicyRequest {
    mcp_access_mode: Option<String>,
}

pub async fn list_platform_internal_agents(
    State(state): State<AppState>,
    auth: RequestAuthContext,
) -> Result<Json<ApiResponse<Vec<ListPlatformInternalAgentResponse>>>, InternalAgentApiError> {
    state
        .agent_management
        .list_platform_internal_agents()
        .map(|records| {
            Json(ApiResponse::success(
                records.into_iter().map(Into::into).collect(),
            ))
        })
        .map_err(|error| InternalAgentApiError(error, auth.locale))
}

pub async fn create_platform_internal_agent(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Json(request): Json<CreatePlatformInternalAgentRequest>,
) -> Result<Json<ApiResponse<CreatePlatformInternalAgentResponse>>, InternalAgentApiError> {
    let record = state
        .agent_management
        .create_platform_internal_agent(CreateWorkspaceInternalAgentCommand {
            agent_code: request.agent_code,
            name: request.name,
            description: request.description,
            role_prompt: request.role_prompt,
            avatar_id: request.avatar_id,
            group_name: request.group_name,
            candidate_model_codes: request.candidate_model_codes,
        })
        .map_err(|error| InternalAgentApiError(error, auth.locale))?;
    Ok(Json(ApiResponse::success(record.into())))
}

pub async fn delete_platform_internal_agent(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(code): Path<String>,
) -> Result<Json<ApiResponse<crate::response::EmptyData>>, InternalAgentApiError> {
    state
        .agent_management
        .delete_platform_internal_agent(&code)
        .map(|()| Json(ApiResponse::success(crate::response::EmptyData {})))
        .map_err(|error| InternalAgentApiError(error, auth.locale))
}

pub async fn update_platform_internal_agent_definition(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(code): Path<String>,
    Json(request): Json<UpdatePlatformInternalAgentDefinitionRequest>,
) -> Result<Json<ApiResponse<UpdatePlatformInternalAgentDefinitionResponse>>, InternalAgentApiError>
{
    let record = state
        .agent_management
        .update_platform_internal_agent_definition(
            &code,
            UpdateWorkspaceInternalAgentDefinitionCommand {
                enabled: request.enabled,
                name: request.name,
                description: request.description,
                role_prompt: request.role_prompt,
                avatar_id: request.avatar_id,
                group_name: request.group_name,
                candidate_model_codes: request.candidate_model_codes,
            },
        )
        .map_err(|error| InternalAgentApiError(error, auth.locale))?;
    Ok(Json(ApiResponse::success(record.into())))
}

pub async fn update_platform_internal_agent_policy(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(code): Path<String>,
    Json(request): Json<UpdatePlatformInternalAgentPolicyRequest>,
) -> Response {
    let command =
        match super::handler::to_internal_agent_policy_command(UpdateInternalAgentPolicyRequest {
            tool_assignment_mode: request.tool_assignment_mode,
            selected_category_codes: request.selected_category_codes,
            max_steps: request.max_steps,
            max_tool_calls: request.max_tool_calls,
            max_parallel_tool_calls: request.max_parallel_tool_calls,
            native_tool_scheme_code: None,
        }) {
            Ok(command) => command,
            Err(_) => {
                return InternalAgentApiError(
                    AgentManagementError::InternalAgentPolicyInvalid,
                    auth.locale,
                )
                .into_response();
            }
        };
    match state
        .agent_management
        .update_platform_internal_agent_policy(&code, command)
    {
        Ok(record) => Json(ApiResponse::success(
            UpdatePlatformInternalAgentPolicyResponse::from(record),
        ))
        .into_response(),
        Err(error) => InternalAgentApiError(error, auth.locale).into_response(),
    }
}

pub async fn update_platform_internal_agent_mcp_policy(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(code): Path<String>,
    Json(request): Json<UpdatePlatformInternalAgentMcpPolicyRequest>,
) -> Response {
    let command = match super::handler::to_internal_agent_mcp_policy_command(
        UpdateInternalAgentMcpPolicyRequest {
            mcp_access_mode: request.mcp_access_mode,
        },
    ) {
        Ok(command) => command,
        Err(_) => {
            return InternalAgentApiError(
                AgentManagementError::InternalAgentPolicyInvalid,
                auth.locale,
            )
            .into_response();
        }
    };
    match state
        .agent_management
        .update_platform_internal_agent_mcp_policy(&code, command)
    {
        Ok(record) => Json(ApiResponse::success(
            UpdatePlatformInternalAgentMcpPolicyResponse::from(record),
        ))
        .into_response(),
        Err(error) => InternalAgentApiError(error, auth.locale).into_response(),
    }
}

pub async fn replace_platform_internal_agent_tool_assignments(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(code): Path<String>,
    Json(request): Json<ReplacePlatformInternalAgentToolsRequest>,
) -> Result<Json<ApiResponse<ReplacePlatformInternalAgentToolsResponse>>, InternalAgentApiError> {
    state
        .agent_management
        .replace_platform_internal_agent_tool_assignments(&code, request.tool_codes)
        .map(|record| Json(ApiResponse::success(record.into())))
        .map_err(|error| InternalAgentApiError(error, auth.locale))
}

pub async fn replace_platform_internal_agent_mcp_server_assignments(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(code): Path<String>,
    Json(request): Json<ReplacePlatformInternalAgentServersRequest>,
) -> Result<Json<ApiResponse<ReplacePlatformInternalAgentServersResponse>>, InternalAgentApiError> {
    let servers = state
        .mcp
        .list_platform_servers(crate::domain::mcp::PlatformMcpServerListQuery {
            search: None,
            status: None,
        })
        .map_err(|_| {
            InternalAgentApiError(AgentManagementError::StorageFailure, auth.locale.clone())
        })?;
    for code in &request.server_codes {
        if !servers
            .iter()
            .any(|server| server.server_code == code.trim() && server.enabled)
        {
            return Err(InternalAgentApiError(
                AgentManagementError::InternalMcpServerAssignmentStale(code.clone()),
                auth.locale,
            ));
        }
    }
    state
        .agent_management
        .replace_platform_internal_agent_mcp_server_assignments(&code, request.server_codes)
        .map(|record| Json(ApiResponse::success(record.into())))
        .map_err(|error| InternalAgentApiError(error, auth.locale))
}

pub async fn get_platform_internal_agent(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(code): Path<String>,
) -> Result<Json<ApiResponse<GetPlatformInternalAgentResponse>>, InternalAgentApiError> {
    let record = state
        .agent_management
        .all_platform_internal_agents()
        .map_err(|error| InternalAgentApiError(error, auth.locale.clone()))?
        .into_iter()
        .find(|agent| agent.agent_code.eq_ignore_ascii_case(&code))
        .ok_or_else(|| {
            InternalAgentApiError(AgentManagementError::InternalAgentNotFound, auth.locale)
        })?;
    Ok(Json(ApiResponse::success(record.into())))
}
