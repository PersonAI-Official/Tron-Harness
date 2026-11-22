use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::{
    adapters::inbound::http::auth::RequestAuthContext,
    app::AppState,
    application::agent::{
        AgentInvocationCommand, AgentInvocationService, AgentManagementService,
        UpdateWorkspaceInternalAgentMcpPolicyCommand, UpdateWorkspaceInternalAgentPolicyCommand,
    },
    domain::mcp::WorkspaceMcpServerListQuery,
    domain::{
        agent::{AgentRunHistoryQuery, AgentRunHistoryRecord, McpAccessMode, ToolAssignmentMode},
        model_provider::ProviderModelRunSnapshotView,
    },
    response::ApiResponse,
};

use super::error_mapper::{
    agent_agui_error_response, agent_error_response, agent_history_error_response,
    agent_management_error_response,
};
use super::request::{
    ListToolsQuery, ReplaceInternalAgentMcpServerAssignmentsRequest,
    ReplaceInternalAgentToolAssignmentsRequest, UpdateInternalAgentMcpPolicyRequest,
    UpdateInternalAgentPolicyRequest,
};

pub async fn create_agent_run(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Json(request): Json<AgentInvocationCommand>,
) -> Response {
    let service = AgentInvocationService::new(
        state.workspaces.clone(),
        state.apps.clone(),
        state.variables.clone(),
        state.skills.clone(),
        state.agent_management.clone(),
        state.agent_runs.clone(),
        state.mcp.clone(),
        state.event_bus.clone(),
        state.agent_model_client.clone(),
        state.agent_tool_registry.clone(),
        state.runtime_http_client.clone(),
    )
    .with_framework_host(state.agent_framework.clone())
    .with_directory_inspector(Some(std::sync::Arc::new(
        crate::adapters::outbound::application_directory::GitApplicationDirectoryInspector,
    )))
    .with_active_runs(state.active_runs.clone())
    .with_prompt_service(state.prompts.clone());
    match service
        .invoke(
            request,
            Some(auth.user().user_code.clone()),
            Some(auth.locale.clone()),
        )
        .await
    {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(error) => agent_error_response(error),
    }
}

pub async fn list_agent_runs(
    State(state): State<AppState>,
    _auth: RequestAuthContext,
    headers: axum::http::HeaderMap,
    Query(query): Query<AgentRunHistoryQuery>,
) -> Response {
    let excluded = if headers.contains_key("x-assistant-code") {
        std::collections::BTreeSet::new()
    } else {
        match state.assistants.list().and_then(|assistants| {
            assistants
                .into_iter()
                .map(|assistant| state.assistants.sessions(&assistant.assistant_code))
                .collect::<Result<Vec<_>, _>>()
        }) {
            Ok(sessions) => sessions
                .into_iter()
                .flatten()
                .map(|session| session.session_code)
                .collect(),
            Err(_) => {
                return agent_history_error_response(
                    crate::domain::agent::AgentRunHistoryError::StorageFailure,
                );
            }
        }
    };
    match state.agent_runs.list_runs_excluding(query, &excluded) {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(error) => agent_history_error_response(error),
    }
}

pub async fn get_agent_run(
    State(state): State<AppState>,
    _auth: RequestAuthContext,
    Path(run_code): Path<String>,
) -> Response {
    match state.agent_runs.get_run(&run_code) {
        Ok(result) => Json(ApiResponse::success(AgentRunDetailResponse {
            model_snapshot: state
                .model_providers
                .get_run_snapshot(&run_code)
                .ok()
                .flatten(),
            run: result,
        }))
        .into_response(),
        Err(error) => agent_history_error_response(error),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentRunDetailResponse {
    #[serde(flatten)]
    run: AgentRunHistoryRecord,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_snapshot: Option<ProviderModelRunSnapshotView>,
}

pub async fn activate_agui_stream(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(stream_key): Path<String>,
) -> Response {
    let service = state.agent_presentation.clone();
    match service.activate(&stream_key) {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(error) => agent_agui_error_response(error, &auth.locale),
    }
}

pub async fn list_tool_domains(
    State(state): State<AppState>,
    State(agent_management): State<AgentManagementService>,
    auth: RequestAuthContext,
) -> Response {
    if let Err(error) = state.agent_framework.ensure_capability_catalog().await {
        tracing::warn!(%error, "Serving known tool domains after catalog discovery failed");
    }
    Json(ApiResponse::success(
        agent_management.list_tool_domains(&auth.locale),
    ))
    .into_response()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolCategoryView {
    code: String,
    name: String,
    tool_count: usize,
}

pub async fn list_tool_categories(
    State(state): State<AppState>,
    State(agent_management): State<AgentManagementService>,
    auth: RequestAuthContext,
) -> Response {
    if let Err(error) = state.agent_framework.ensure_capability_catalog().await {
        tracing::warn!(%error, "Serving known categories after catalog discovery failed");
    }
    let mut categories = std::collections::BTreeMap::<String, ToolCategoryView>::new();
    for tool in agent_management.list_tools(None, None, &auth.locale) {
        let Some(category) = tool.category else {
            continue;
        };
        let name = category
            .names
            .get(&auth.locale)
            .or_else(|| category.names.get("en-US"))
            .cloned()
            .unwrap_or_else(|| category.code.clone());
        let entry = categories
            .entry(category.code.clone())
            .or_insert(ToolCategoryView {
                code: category.code,
                name,
                tool_count: 0,
            });
        entry.tool_count += 1;
    }
    Json(ApiResponse::protocol_success(
        categories.into_values().collect::<Vec<_>>(),
    ))
    .into_response()
}

pub async fn list_tools(
    State(state): State<AppState>,
    State(agent_management): State<AgentManagementService>,
    auth: RequestAuthContext,
    Query(query): Query<ListToolsQuery>,
) -> Response {
    if let Err(error) = state.agent_framework.ensure_capability_catalog().await {
        tracing::warn!(%error, "Serving known tools after catalog discovery failed");
    }
    let tools = agent_management.list_tools(
        query.domain_code.as_deref(),
        query.search.as_deref(),
        &auth.locale,
    );
    let catalog_issues = state.agent_framework.tool_catalog_issues();
    Json(ApiResponse::success(
        tools
            .into_iter()
            .map(|mut tool| {
                let owner = agent_management.capability_resource("tool", &tool.tool_code);
                if tool.source.pack_id.is_none() {
                    tool.source.pack_id = owner.as_ref().map(|item| item.pack_id.clone());
                }
                let catalog_issue = catalog_issues.iter().find(|issue| {
                    issue.blocks_tool
                        && issue
                            .pack_id
                            .as_ref()
                            .is_none_or(|pack| tool.source.pack_id.as_ref() == Some(pack))
                        && issue.provider_id.as_ref().is_none_or(|provider| {
                            tool.source.provider_id.as_ref() == Some(provider)
                                || tool.source.plugin_id.as_ref() == Some(provider)
                        })
                        && issue
                            .tool_code
                            .as_ref()
                            .is_none_or(|code| code == &tool.tool_code)
                        && issue
                            .tool_name
                            .as_ref()
                            .is_none_or(|name| name == &tool.tool_name)
                });
                if catalog_issue.is_some() {
                    tool.availability_state = crate::domain::agent::ToolAvailabilityState::Disabled;
                }
                let assignable = tool.availability_state
                    == crate::domain::agent::ToolAvailabilityState::Available;
                let assistant_assignable =
                    !crate::application::agent::assistant_workspace_tool(&tool);
                let assistant_replacement_tool_code =
                    crate::application::agent::assistant_tool_replacement(&tool.tool_code)
                        .map(str::to_owned);
                let disabled_reason =
                    catalog_issue
                        .map(|issue| issue.message.clone())
                        .or_else(|| {
                            owner.as_ref().filter(|o| !o.available).map(|o| {
                                tron_i18n::t!(
                                    "agents.tool.pack_disabled",
                                    locale = &auth.locale,
                                    pack = o.pack_id
                                )
                            })
                        });
                ToolAssignmentVo {
                    provider_pack_id: owner.as_ref().map(|o| o.pack_id.clone()),
                    assignable,
                    assistant_assignable,
                    assistant_replacement_tool_code,
                    disabled_reason,
                    tool_code: tool.tool_code,
                    tool_name: tool.tool_name,
                    domain_code: tool.domain_code,
                    category: tool.category.map(|category| ToolCategoryMetadataVo {
                        code: category.code,
                        names: category.names,
                    }),
                    source: ToolSourceVo {
                        provider_id: tool.source.provider_id,
                        plugin_id: tool.source.plugin_id,
                        pack_id: tool.source.pack_id,
                        adapter_id: tool.source.adapter_id,
                    },
                    name: tool.name,
                    description: tool.description,
                    operation_kind: tool.operation_kind,
                    input_schema: tool.input_schema,
                    module_owner: tool.module_owner,
                    availability_state: tool.availability_state,
                    sample_result_summary: tool.sample_result_summary,
                }
            })
            .collect::<Vec<_>>(),
    ))
    .into_response()
}

pub async fn get_tool(
    State(agent_management): State<AgentManagementService>,
    auth: RequestAuthContext,
    Path(tool_code): Path<String>,
) -> Response {
    match agent_management.tool(&tool_code, &auth.locale) {
        Some(tool) => Json(ApiResponse::success(tool)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error(
                "AGENT_TOOL_NOT_FOUND",
                tron_i18n::t!("agents.tool.not_found", locale = &auth.locale),
            )),
        )
            .into_response(),
    }
}

pub async fn list_workspace_internal_agents(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
    Path(workspace_code): Path<String>,
) -> Response {
    match agent_management.list_workspace_internal_agents(&workspace_code) {
        Ok(records) => Json(ApiResponse::success(records)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn get_workspace_internal_agent(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
    Path((workspace_code, agent_code)): Path<(String, String)>,
) -> Response {
    match agent_management.workspace_internal_agent_policy(&workspace_code, &agent_code) {
        Ok(record) => Json(ApiResponse::success(record)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn update_workspace_internal_agent_policy(
    State(agent_management): State<AgentManagementService>,
    auth: RequestAuthContext,
    Path((workspace_code, agent_code)): Path<(String, String)>,
    Json(request): Json<UpdateInternalAgentPolicyRequest>,
) -> Response {
    let command = match to_internal_agent_policy_command(request) {
        Ok(command) => command,
        Err((code, message_key)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(
                    code,
                    tron_i18n::t!(message_key, locale = &auth.locale),
                )),
            )
                .into_response();
        }
    };
    match agent_management.update_internal_agent_policy(&workspace_code, &agent_code, command) {
        Ok(record) => Json(ApiResponse::success(record)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn replace_workspace_internal_agent_tool_assignments(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
    Path((workspace_code, agent_code)): Path<(String, String)>,
    Json(request): Json<ReplaceInternalAgentToolAssignmentsRequest>,
) -> Response {
    let tool_codes = normalize_tool_codes(request.tool_codes);
    match agent_management.replace_internal_agent_tool_assignments(
        &workspace_code,
        &agent_code,
        tool_codes,
    ) {
        Ok(record) => Json(ApiResponse::success(record)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn reset_workspace_internal_agent_policy(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
    Path((workspace_code, agent_code)): Path<(String, String)>,
) -> Response {
    match agent_management.reset_internal_agent_policy(&workspace_code, &agent_code) {
        Ok(record) => Json(ApiResponse::success(record)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn update_workspace_internal_agent_mcp_policy(
    State(agent_management): State<AgentManagementService>,
    auth: RequestAuthContext,
    Path((workspace_code, agent_code)): Path<(String, String)>,
    Json(request): Json<UpdateInternalAgentMcpPolicyRequest>,
) -> Response {
    let command = match to_internal_agent_mcp_policy_command(request) {
        Ok(command) => command,
        Err((code, message_key)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::error(
                    code,
                    tron_i18n::t!(message_key, locale = &auth.locale),
                )),
            )
                .into_response();
        }
    };
    match agent_management.update_internal_agent_mcp_policy(&workspace_code, &agent_code, command) {
        Ok(record) => Json(ApiResponse::success(record)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn replace_workspace_internal_agent_mcp_server_assignments(
    State(state): State<AppState>,
    _auth: RequestAuthContext,
    Path((workspace_code, agent_code)): Path<(String, String)>,
    Json(request): Json<ReplaceInternalAgentMcpServerAssignmentsRequest>,
) -> Response {
    let server_codes = normalize_tool_codes(request.server_codes);
    if let Err(error) =
        validate_workspace_mcp_server_assignments(&state, &workspace_code, &server_codes)
    {
        return agent_management_error_response(error);
    }
    match state
        .agent_management
        .replace_internal_agent_mcp_server_assignments(&workspace_code, &agent_code, server_codes)
    {
        Ok(record) => Json(ApiResponse::success(record)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn list_platform_system_agents(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
) -> Response {
    match agent_management.list_platform_system_agents() {
        Ok(records) => Json(ApiResponse::success(records)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn list_runtime_capabilities(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
) -> Response {
    match agent_management.list_runtime_capabilities() {
        Ok(records) => Json(ApiResponse::success(records)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn get_runtime_capability(
    State(agent_management): State<AgentManagementService>,
    auth: RequestAuthContext,
    Path(adapter_key): Path<String>,
) -> Response {
    match agent_management.runtime_capability(&adapter_key) {
        Ok(Some(record)) => Json(ApiResponse::success(record)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error(
                "AGENT_RUNTIME_NOT_FOUND",
                tron_i18n::t!("agents.runtime.not_found", locale = &auth.locale),
            )),
        )
            .into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn rescan_platform_system_agents(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
) -> Response {
    let result = tokio::task::spawn_blocking(move || agent_management.rescan_system_agents())
        .await
        .unwrap_or(Err(
            crate::domain::agent::AgentManagementError::StorageFailure,
        ));
    match result {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn enable_platform_system_agent(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
    Path(agent_code): Path<String>,
) -> Response {
    match agent_management.enable_platform_system_agent(&agent_code) {
        Ok(record) => Json(ApiResponse::success(record)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn disable_platform_system_agent(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
    Path(agent_code): Path<String>,
) -> Response {
    match agent_management.disable_platform_system_agent(&agent_code) {
        Ok(record) => Json(ApiResponse::success(record)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn list_workspace_system_agents(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
    Path(workspace_code): Path<String>,
) -> Response {
    match agent_management.list_workspace_system_agents(&workspace_code) {
        Ok(records) => Json(ApiResponse::success(records)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn enable_workspace_system_agent(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
    Path((workspace_code, agent_code)): Path<(String, String)>,
) -> Response {
    match agent_management.enable_workspace_system_agent(&workspace_code, &agent_code) {
        Ok(record) => Json(ApiResponse::success(record)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn disable_workspace_system_agent(
    State(agent_management): State<AgentManagementService>,
    _auth: RequestAuthContext,
    Path((workspace_code, agent_code)): Path<(String, String)>,
) -> Response {
    match agent_management.disable_workspace_system_agent(&workspace_code, &agent_code) {
        Ok(record) => Json(ApiResponse::success(record)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn deactivate_agui_stream(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(stream_key): Path<String>,
) -> Response {
    let service = state.agent_presentation.clone();
    match service.deactivate(&stream_key) {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(error) => agent_agui_error_response(error, &auth.locale),
    }
}

pub async fn get_native_tool_config(
    State(agent_management): State<AgentManagementService>,
) -> Response {
    match agent_management.native_tool_config() {
        Ok(config) => Json(ApiResponse::success(config)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn replace_native_tool_config(
    State(agent_management): State<AgentManagementService>,
    Json(request): Json<super::request::ReplaceNativeToolConfigRequest>,
) -> Response {
    match agent_management.replace_native_tool_config(request.schemes, request.default_scheme_code)
    {
        Ok(config) => Json(ApiResponse::success(config)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub async fn update_agent_native_scheme(
    State(agent_management): State<AgentManagementService>,
    Path((workspace_code, agent_code)): Path<(String, String)>,
    Json(request): Json<super::request::UpdateAgentNativeSchemeRequest>,
) -> Response {
    match agent_management.set_agent_native_scheme(
        &workspace_code,
        &agent_code,
        request.scheme_code.as_deref(),
    ) {
        Ok(scheme_code) => Json(ApiResponse::success(scheme_code)).into_response(),
        Err(error) => agent_management_error_response(error),
    }
}

pub(super) fn to_internal_agent_policy_command(
    request: UpdateInternalAgentPolicyRequest,
) -> Result<UpdateWorkspaceInternalAgentPolicyCommand, (&'static str, &'static str)> {
    let tool_assignment_mode = match request.tool_assignment_mode {
        Some(value) => Some(parse_tool_assignment_mode(&value)?),
        None => None,
    };
    Ok(UpdateWorkspaceInternalAgentPolicyCommand {
        tool_assignment_mode,
        native_tool_scheme_code: request.native_tool_scheme_code,
        selected_category_codes: request.selected_category_codes,
        max_steps: request.max_steps,
        max_tool_calls: request.max_tool_calls,
        max_parallel_tool_calls: request.max_parallel_tool_calls,
    })
}

pub(super) fn to_internal_agent_mcp_policy_command(
    request: UpdateInternalAgentMcpPolicyRequest,
) -> Result<UpdateWorkspaceInternalAgentMcpPolicyCommand, (&'static str, &'static str)> {
    let mcp_access_mode = match request.mcp_access_mode {
        Some(value) => Some(parse_mcp_access_mode(&value)?),
        None => None,
    };

    Ok(UpdateWorkspaceInternalAgentMcpPolicyCommand { mcp_access_mode })
}

fn parse_tool_assignment_mode(
    raw: &str,
) -> Result<ToolAssignmentMode, (&'static str, &'static str)> {
    match normalize_mode_token(raw).as_str() {
        "all_platform_tools" => Ok(ToolAssignmentMode::AllPlatformTools),
        "selected_categories" | "selectedcategories" => Ok(ToolAssignmentMode::SelectedCategories),
        "selected_tools" => Ok(ToolAssignmentMode::SelectedTools),
        "all_read_only_tools" | "allreadonlytools" => Ok(ToolAssignmentMode::AllReadOnlyTools),
        _ => Err((
            "AGENT_TOOL_ASSIGNMENT_MODE_INVALID",
            "agents.policy.tool_assignment_mode_invalid",
        )),
    }
}

fn parse_mcp_access_mode(raw: &str) -> Result<McpAccessMode, (&'static str, &'static str)> {
    match normalize_mode_token(raw).as_str() {
        "disabled" => Ok(McpAccessMode::Disabled),
        "all_enabled_servers" => Ok(McpAccessMode::AllEnabledServers),
        "selected_servers" => Ok(McpAccessMode::SelectedServers),
        _ => Err((
            "AGENT_MCP_ACCESS_MODE_INVALID",
            "agents.policy.mcp_access_mode_invalid",
        )),
    }
}

fn normalize_tool_codes(values: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for value in values {
        let normalized = value.trim().to_string();
        if normalized.is_empty() {
            continue;
        }
        if !deduped
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(&normalized))
        {
            deduped.push(normalized);
        }
    }
    deduped
}

fn validate_workspace_mcp_server_assignments(
    state: &AppState,
    workspace_code: &str,
    server_codes: &[String],
) -> Result<(), crate::domain::agent::AgentManagementError> {
    let effective_servers = state
        .mcp
        .list_workspace_effective_servers(
            workspace_code,
            WorkspaceMcpServerListQuery { search: None },
        )
        .map_err(map_mcp_assignment_error)?;
    let effective_codes = effective_servers
        .into_iter()
        .filter(|server| server.effective_enabled)
        .map(|server| server.server_code)
        .collect::<std::collections::HashSet<_>>();
    for server_code in server_codes {
        if !effective_codes.contains(server_code) {
            return Err(
                crate::domain::agent::AgentManagementError::InternalMcpServerAssignmentStale(
                    server_code.clone(),
                ),
            );
        }
    }
    Ok(())
}

fn map_mcp_assignment_error(
    error: crate::domain::mcp::McpError,
) -> crate::domain::agent::AgentManagementError {
    match error {
        crate::domain::mcp::McpError::InvalidWorkspace
        | crate::domain::mcp::McpError::WorkspaceRequired => {
            crate::domain::agent::AgentManagementError::InvalidWorkspace
        }
        _ => crate::domain::agent::AgentManagementError::StorageFailure,
    }
}

fn normalize_mode_token(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace('-', "_")
}

pub async fn list_agui_stream_events(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(stream_key): Path<String>,
) -> Response {
    let service = state.agent_presentation.clone();
    match service.events(&stream_key) {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(error) => agent_agui_error_response(error, &auth.locale),
    }
}

pub async fn list_agui_stream_protocol_events(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(stream_key): Path<String>,
) -> Response {
    let service = state.agent_presentation.clone();
    match service.protocol_events(&stream_key) {
        Ok(result) => Json(ApiResponse::success(result)).into_response(),
        Err(error) => agent_agui_error_response(error, &auth.locale),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolAssignmentVo {
    tool_code: String,
    tool_name: String,
    domain_code: String,
    category: Option<ToolCategoryMetadataVo>,
    source: ToolSourceVo,
    name: String,
    description: String,
    operation_kind: crate::domain::agent::ToolOperationKind,
    input_schema: serde_json::Value,
    module_owner: String,
    availability_state: crate::domain::agent::ToolAvailabilityState,
    sample_result_summary: String,
    provider_pack_id: Option<String>,
    assignable: bool,
    assistant_assignable: bool,
    assistant_replacement_tool_code: Option<String>,
    disabled_reason: Option<String>,
}

#[derive(Serialize)]
struct ToolCategoryMetadataVo {
    code: String,
    names: std::collections::BTreeMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolSourceVo {
    provider_id: Option<String>,
    plugin_id: Option<String>,
    pack_id: Option<String>,
    adapter_id: Option<String>,
}
