export const agentApiEndpoints = {
  getToolDomains: "GET /api/v1/tools/domains",
  getToolCatalogIssues: "GET /api/v1/tools/catalog-issues",
  getToolCategories: "GET /api/v1/tools/categories",
  getTools: "GET /api/v1/tools",
  getNativeToolConfig: "GET /api/v1/tools/native-schemes",
  replaceNativeToolConfig: "PUT /api/v1/tools/native-schemes",
  patchAgentNativeScheme:
    "PATCH /api/v1/agents/workspaces/{workspaceCode}/internal/{agentCode}/native-scheme",
  listMainAgentMessages: "GET /api/v1/agent/main/messages",
  listPlatformSystemAgents: "GET /api/v1/agents/registry/system",
  createPlatformSystemAgent: "POST /api/v1/agents/registry/system",
  updatePlatformSystemAgent: "PUT /api/v1/agents/registry/system/{agentCode}",
  deletePlatformSystemAgent: "DELETE /api/v1/agents/registry/system/{agentCode}",
  rescanPlatformSystemAgents: "POST /api/v1/agents/registry/system/rescan",
  enablePlatformSystemAgent: "POST /api/v1/agents/registry/system/{agentCode}/enable",
  disablePlatformSystemAgent: "POST /api/v1/agents/registry/system/{agentCode}/disable",
  listWorkspaceSystemAgents: "GET /api/v1/agents/workspaces/{workspaceCode}/system",
  enableWorkspaceSystemAgent: "POST /api/v1/agents/workspaces/{workspaceCode}/system/{agentCode}/enable",
  disableWorkspaceSystemAgent: "POST /api/v1/agents/workspaces/{workspaceCode}/system/{agentCode}/disable",
  listWorkspaceInternalAgents: "GET /api/v1/agents/workspaces/{workspaceCode}/internal",
  deleteWorkspaceInternalAgent: "DELETE /api/v1/agents/workspaces/{workspaceCode}/internal/{agentCode}",
  getWorkspaceInternalAgent: "GET /api/v1/agents/workspaces/{workspaceCode}/internal/{agentCode}",
  patchWorkspaceInternalAgentPolicy:
    "PATCH /api/v1/agents/workspaces/{workspaceCode}/internal/{agentCode}/policy",
  updateWorkspaceInternalAgentToolAssignments:
    "PUT /api/v1/agents/workspaces/{workspaceCode}/internal/{agentCode}/tool-assignments",
  patchWorkspaceInternalAgentMcpPolicy:
    "PATCH /api/v1/agents/workspaces/{workspaceCode}/internal/{agentCode}/mcp-policy",
  updateWorkspaceInternalAgentMcpServerAssignments:
    "PUT /api/v1/agents/workspaces/{workspaceCode}/internal/{agentCode}/mcp-server-assignments",
  resetWorkspaceInternalAgentPolicy:
    "POST /api/v1/agents/workspaces/{workspaceCode}/internal/{agentCode}/reset-policy",
} as const;
