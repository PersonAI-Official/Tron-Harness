use axum::{
    Router,
    routing::{get, patch, post, put},
};

use super::internal_definitions::{create_internal_agent, update_internal_agent_definition};
use super::platform_internal_agents as platform;
use crate::app::AppState;

use super::handler::{
    activate_agui_stream, create_agent_run, deactivate_agui_stream, disable_platform_system_agent,
    disable_workspace_system_agent, enable_platform_system_agent, enable_workspace_system_agent,
    get_agent_run, get_native_tool_config, get_runtime_capability, get_tool,
    get_workspace_internal_agent, list_agent_runs, list_agui_stream_events,
    list_agui_stream_protocol_events, list_platform_system_agents, list_runtime_capabilities,
    list_tool_categories, list_tool_domains, list_tools, list_workspace_internal_agents,
    list_workspace_system_agents, replace_native_tool_config,
    replace_workspace_internal_agent_mcp_server_assignments,
    replace_workspace_internal_agent_tool_assignments, rescan_platform_system_agents,
    reset_workspace_internal_agent_policy, update_agent_native_scheme,
    update_workspace_internal_agent_mcp_policy, update_workspace_internal_agent_policy,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/agents/registry/internal-groups", get(super::internal_groups::list_platform).post(super::internal_groups::create_platform).delete(super::internal_groups::delete_platform))
        .route("/api/v1/agents/workspaces/{workspace_code}/internal-groups", get(super::internal_groups::list_workspace).post(super::internal_groups::create_workspace).delete(super::internal_groups::delete_workspace))
        .route("/api/v1/agents/registry/{agent_code}/workspace-grants", get(super::workspace_grants::list).put(super::workspace_grants::replace))
        .route("/api/v1/agents/registry/internal", get(platform::list_platform_internal_agents).post(platform::create_platform_internal_agent))
        .route("/api/v1/agents/registry/internal/{agent_code}", get(platform::get_platform_internal_agent).delete(platform::delete_platform_internal_agent))
        .route("/api/v1/agents/registry/internal/{agent_code}/definition", patch(platform::update_platform_internal_agent_definition))
        .route("/api/v1/agents/registry/internal/{agent_code}/policy", patch(platform::update_platform_internal_agent_policy))
        .route("/api/v1/agents/registry/internal/{agent_code}/mcp-policy", patch(platform::update_platform_internal_agent_mcp_policy))
        .route("/api/v1/agents/registry/internal/{agent_code}/tool-assignments", put(platform::replace_platform_internal_agent_tool_assignments))
        .route("/api/v1/agents/registry/internal/{agent_code}/mcp-server-assignments", put(platform::replace_platform_internal_agent_mcp_server_assignments))
        .route(
            "/api/v1/agents/runs",
            get(list_agent_runs).post(create_agent_run),
        )
        .route("/api/v1/agents/runs/{run_code}", get(get_agent_run))
        .route(
            "/api/v1/agents/registry/system",
            get(list_platform_system_agents).post(super::system_agent_profiles::create),
        )
        .route("/api/v1/agents/registry/system/{agent_code}", put(super::system_agent_profiles::update).delete(super::system_agent_profiles::delete))
        .route(
            "/api/v1/agents/registry/system/rescan",
            post(rescan_platform_system_agents),
        )
        .route(
            "/api/v1/agents/registry/system/{agent_code}/enable",
            post(enable_platform_system_agent),
        )
        .route(
            "/api/v1/agents/registry/system/{agent_code}/disable",
            post(disable_platform_system_agent),
        )
        .route("/api/v1/agents/runtimes", get(list_runtime_capabilities))
        .route(
            "/api/v1/agents/runtimes/{adapter_key}/capabilities",
            get(get_runtime_capability),
        )
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/system",
            get(list_workspace_system_agents),
        )
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/system/{agent_code}/enable",
            post(enable_workspace_system_agent),
        )
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/system/{agent_code}/disable",
            post(disable_workspace_system_agent),
        )
        .route("/api/v1/tools/permissions", get(super::tool_permissions::list).put(super::tool_permissions::replace))
        .route("/api/v1/tools/catalog-issues", get(super::tool_catalog_issues::list))
        .route("/api/v1/tools/domains", get(list_tool_domains))
        .route("/api/v1/tools/categories", get(list_tool_categories))
        .route("/api/v1/tools", get(list_tools))
        .route(
            "/api/v1/tools/native-schemes",
            get(get_native_tool_config).put(replace_native_tool_config),
        )
        .route("/api/v1/tools/{tool_code}", get(get_tool))
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/internal",
            get(list_workspace_internal_agents).post(create_internal_agent),
        )
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/internal/{agent_code}",
            get(get_workspace_internal_agent).delete(super::internal_definitions::delete_internal_agent),
        )
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/internal/{agent_code}/definition",
            patch(update_internal_agent_definition),
        )
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/internal/{agent_code}/policy",
            patch(update_workspace_internal_agent_policy),
        )
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/internal/{agent_code}/tool-assignments",
            put(replace_workspace_internal_agent_tool_assignments),
        )
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/internal/{agent_code}/mcp-policy",
            patch(update_workspace_internal_agent_mcp_policy),
        )
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/internal/{agent_code}/mcp-server-assignments",
            put(replace_workspace_internal_agent_mcp_server_assignments),
        )
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/internal/{agent_code}/native-scheme",
            patch(update_agent_native_scheme),
        )
        .route(
            "/api/v1/agents/workspaces/{workspace_code}/internal/{agent_code}/reset-policy",
            post(reset_workspace_internal_agent_policy),
        )
        .route(
            "/api/v1/agents/agui/streams/{stream_key}",
            post(activate_agui_stream).delete(deactivate_agui_stream),
        )
        .route(
            "/api/v1/agents/agui/streams/{stream_key}/events",
            get(list_agui_stream_events),
        )
        .route(
            "/api/v1/agents/agui/streams/{stream_key}/protocol-events",
            get(list_agui_stream_protocol_events),
        )
}
