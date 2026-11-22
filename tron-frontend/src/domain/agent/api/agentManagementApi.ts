import { requestApi } from "@/shared/api/httpClient";

import type {
  NativeToolConfigRecord,
  NativeToolSchemeRecord,
  PlatformToolRecord,
  ToolCatalogDomainRecord,
  SystemAgentModelRecord,
  PlatformSystemAgentRecord,
  PlatformSystemAgentRescanResult,
  WorkspaceSystemAgentRecord,
  WorkspaceInternalAgentMcpPolicyPayload,
  WorkspaceInternalAgentPolicyPayload,
  WorkspaceInternalAgentRecord,
  ListToolsQuery,
  CreateInternalAgentPayload,
  UpdateInternalAgentDefinitionPayload,
  SystemAgentProfilePayload,
} from "@/domain/agent/model";

import { agentApiEndpoints } from "./agentApi";
import { PLATFORM_INTERNAL_AGENT_SCOPE } from "../model";

function internalAgentEndpoint(endpoint: string, workspaceCode: string) {
  return workspaceCode === PLATFORM_INTERNAL_AGENT_SCOPE
    ? endpoint.replace("/workspaces/{workspaceCode}", "/registry")
    : endpoint;
}

export interface ToolListPayload {
  domainCode?: string;
  search?: string;
}

type AgentStatusApi =
  | "ENABLED"
  | "DISABLED"
  | "UNAVAILABLE"
  | PlatformSystemAgentRecord["status"];
type AgentSourceApi = "SYSTEM" | PlatformSystemAgentRecord["source"];
type ToolOperationKindApi =
  | PlatformToolRecord["operationKind"]
  | "QUERY"
  | "COMMAND"
  | "Unknown"
  | "UNKNOWN";
type ToolAssignmentModeApi =
  | WorkspaceInternalAgentRecord["toolAssignmentMode"]
  | Uppercase<WorkspaceInternalAgentRecord["toolAssignmentMode"]>
  | "AllReadOnlyTools" | "SelectedCategories";
type WorkspaceInternalAgentSourceApi =
  | WorkspaceInternalAgentRecord["source"]
  | "PLATFORM_DEFAULT"
  | "WORKSPACE_OVERRIDE";
type McpAccessModeApi =
  | WorkspaceInternalAgentRecord["mcpAccessMode"]
  | Uppercase<WorkspaceInternalAgentRecord["mcpAccessMode"]>;

interface ManagedSystemAgentRecordApi {
  agentCode: string;
  baseAgentCode?: string | null;
  modelId?: string | null;
  fastMode?: boolean;
  reasoningEffort?: PlatformSystemAgentRecord["reasoningEffort"];
  avatarId?: string | null;
  name: string;
  description: string;
  source: AgentSourceApi;
  status: AgentStatusApi;
  enabled: boolean;
  available: boolean;
  models?: SystemAgentModelRecord[] | null;
  lastScannedAt: string | null;
  inheritedFrom?: string | null;
  platformEnabled?: boolean;
  workspaceOverrideDisabled?: boolean;
}

interface ToolCatalogDomainRecordApi {
  domainCode: string;
  name: string;
  description: string;
  toolCount: number;
  sampleTools?: string[] | null;
  hasMutationTools?: boolean;
}

interface ToolRecordApi {
  category?: { code: string; names: Record<string, string> } | null;
  source?: { providerId: string | null; pluginId: string | null; packId: string | null; adapterId: string | null } | null;
  providerPackId?: string | null;
  assignable?: boolean;
  assistantAssignable?: boolean;
  assistantReplacementToolCode?: string | null;
  disabledReason?: string | null;
  toolCode: string;
  toolName: string;
  domainCode: string;
  name: string;
  description: string;
  operationKind: ToolOperationKindApi;
}

export interface WorkspaceInternalAgentRecordApi {
  enabled?: boolean;
  agentCode: string;
  name: string;
  description: string;
  rolePrompt: string;

  avatarId?: string;
  candidateModelCodes?: string[];
  groupName?: string | null;
  builtIn?: boolean;
  platformDefined: boolean;
  source?: WorkspaceInternalAgentSourceApi;
  mcpAccessMode?: McpAccessModeApi;
  toolAssignmentMode: ToolAssignmentModeApi;
  selectedCategoryCodes?: string[];
  maxSteps: number;
  maxToolCalls: number;
  maxParallelToolCalls?: number;
  selectedServerCodes?: string[];
  selectedToolCodes: string[];
  nativeToolSchemeCode?: string | null;
}

function normalizeAgentModels(models: ManagedSystemAgentRecordApi["models"]) {
  if (!Array.isArray(models)) return [];
  return models
    .map((model) => ({
      supportedReasoningEfforts: model.supportedReasoningEfforts,
      displayName: model.displayName,
      modelId: model.modelId,
    }))
    .filter((model) => model.modelId?.trim().length > 0)
    .map((model) => ({
      supportedReasoningEfforts: model.supportedReasoningEfforts,
      displayName: model.displayName?.trim() || model.modelId.trim(),
      modelId: model.modelId.trim(),
    }));
}

interface PlatformSystemAgentRescanResultApi {
  scannedCount: number;
  availableCount: number;
  unavailableCount: number;
  records: ManagedSystemAgentRecordApi[];
}

function normalizeToolOperationKind(
  kind: ToolOperationKindApi
): PlatformToolRecord["operationKind"] {
  switch (kind) {
    case "COMMAND":
      return "command";
    case "command":
      return kind;
    case "QUERY":
      return "query";
    case "query":
      return kind;
    default:
      return "unknown";
  }
}

function normalizeToolAssignmentMode(
  mode: ToolAssignmentModeApi
): WorkspaceInternalAgentRecord["toolAssignmentMode"] {
  switch (mode) {
    case "SelectedCategories":
    case "SELECTED_CATEGORIES":
    case "selected_categories":
      return "selected_categories";
    case "ALL_PLATFORM_TOOLS":
    case "all_platform_tools":
      return "all_platform_tools";
    case "AllReadOnlyTools":
    case "ALL_READ_ONLY_TOOLS":
    case "all_read_only_tools":
      return "all_read_only_tools";
    case "SELECTED_TOOLS":
    case "selected_tools":
      return "selected_tools";
    default:
      return "all_platform_tools";
  }
}



function normalizeWorkspaceInternalAgentSource(
  source: WorkspaceInternalAgentSourceApi | undefined
): WorkspaceInternalAgentRecord["source"] {
  switch (source) {
    case "WORKSPACE_OVERRIDE":
      return "workspace_override";
    case "PLATFORM_DEFAULT":
      return "platform_default";
    default:
      return "platform_default";
  }
}

function normalizeMcpAccessMode(
  mode: McpAccessModeApi | undefined
): WorkspaceInternalAgentRecord["mcpAccessMode"] {
  switch (mode) {
    case "ALL_ENABLED_SERVERS":
    case "all_enabled_servers":
      return "all_enabled_servers";
    case "SELECTED_SERVERS":
    case "selected_servers":
      return "selected_servers";
    default:
      return "disabled";
  }
}

function normalizeToolCatalogDomainRecord(
  record: ToolCatalogDomainRecordApi
): ToolCatalogDomainRecord {
  return {
    description: record.description,
    domainCode: record.domainCode,
    hasMutationTools: Boolean(record.hasMutationTools),
    name: record.name,
    sampleTools: Array.isArray(record.sampleTools)
      ? record.sampleTools.filter((toolCode) => toolCode.trim().length > 0)
      : [],
    toolCount: Number.isFinite(record.toolCount) ? record.toolCount : 0,
  };
}

function normalizeToolRecord(record: ToolRecordApi): PlatformToolRecord {
  return {
    category: record.category,
    source: record.source,
    providerPackId: record.providerPackId,
    assignable: record.assignable,
    assistantAssignable: record.assistantAssignable,
    assistantReplacementToolCode: record.assistantReplacementToolCode,
    disabledReason: record.disabledReason,
    description: record.description,
    domainCode: record.domainCode,
    name: record.name,
    operationKind: normalizeToolOperationKind(record.operationKind),
    toolCode: record.toolCode,
    toolName: record.toolName,
  };
}

export function normalizeWorkspaceInternalAgentRecord(
  record: WorkspaceInternalAgentRecordApi
): WorkspaceInternalAgentRecord {
  return {
    enabled: record.enabled ?? true,
    agentCode: record.agentCode,
    description: record.description,
    rolePrompt: record.rolePrompt ?? "",

    avatarId: record.avatarId || "robot",
    candidateModelCodes: record.candidateModelCodes ?? [],
    groupName: record.groupName || null,
    builtIn: record.builtIn ?? false,
    platformDefined:
      record.platformDefined ?? record.agentCode === "main-agent",
    mcpAccessMode: normalizeMcpAccessMode(record.mcpAccessMode),
    name: record.name,
    maxSteps:
      typeof record.maxSteps === "number" && Number.isFinite(record.maxSteps)
        ? record.maxSteps
        : 0,
    maxToolCalls:
      typeof record.maxToolCalls === "number" &&
      Number.isFinite(record.maxToolCalls)
        ? record.maxToolCalls
        : 0,
    maxParallelToolCalls:
      typeof record.maxParallelToolCalls === "number" &&
      Number.isFinite(record.maxParallelToolCalls)
        ? record.maxParallelToolCalls
        : 10,
    source: normalizeWorkspaceInternalAgentSource(record.source),
    selectedServerCodes: Array.isArray(record.selectedServerCodes)
      ? [...new Set(record.selectedServerCodes)]
      : [],
    nativeToolSchemeCode: record.nativeToolSchemeCode ?? null,
    selectedToolCodes: Array.isArray(record.selectedToolCodes)
      ? [...new Set(record.selectedToolCodes)]
      : [],
    toolAssignmentMode: normalizeToolAssignmentMode(record.toolAssignmentMode),
    selectedCategoryCodes: record.selectedCategoryCodes ?? [],
  };
}

function normalizeAgentStatus(
  status: AgentStatusApi
): PlatformSystemAgentRecord["status"] {
  switch (status) {
    case "ENABLED":
      return "Enabled";
    case "DISABLED":
      return "Disabled";
    case "UNAVAILABLE":
      return "Unavailable";
    default:
      return status;
  }
}

function normalizeAgentSource(source: AgentSourceApi) {
  if (source === "SYSTEM") {
    return "System Scan";
  }

  return source;
}

function normalizePlatformSystemAgentRecord(
  record: ManagedSystemAgentRecordApi
): PlatformSystemAgentRecord {
  return {
    agentCode: record.agentCode,
    baseAgentCode: record.baseAgentCode,
    modelId: record.modelId,
    fastMode: record.fastMode ?? false,
    reasoningEffort: record.reasoningEffort ?? null,
    avatarId: record.avatarId ?? null,
    available: record.available,
    description: record.description,
    enabled: record.enabled,
    lastScannedAt: record.lastScannedAt,
    models: normalizeAgentModels(record.models),
    name: record.name,
    source: normalizeAgentSource(record.source),
    status: normalizeAgentStatus(record.status),
  };
}

function normalizeWorkspaceSystemAgentRecord(
  record: ManagedSystemAgentRecordApi
): WorkspaceSystemAgentRecord {
  return {
    agentCode: record.agentCode,
    baseAgentCode: record.baseAgentCode,
    modelId: record.modelId,
    fastMode: record.fastMode ?? false,
    reasoningEffort: record.reasoningEffort ?? null,
    avatarId: record.avatarId ?? null,
    available: record.available,
    description: record.description,
    enabled: record.enabled,
    inheritedFrom: (record.inheritedFrom ??
      "Platform") as WorkspaceSystemAgentRecord["inheritedFrom"],
    lastScannedAt: record.lastScannedAt,
    models: normalizeAgentModels(record.models),
    name: record.name,
    platformEnabled: record.platformEnabled ?? record.enabled,
    source: normalizeAgentSource(record.source),
    status: normalizeAgentStatus(record.status),
    workspaceOverrideDisabled: record.workspaceOverrideDisabled ?? false,
  };
}

export function fetchPlatformSystemAgents(signal?: AbortSignal) {
  return requestApi<ManagedSystemAgentRecordApi[]>(
    agentApiEndpoints.listPlatformSystemAgents,
    { signal }
  ).then((records) => records.map(normalizePlatformSystemAgentRecord));
}

export function createPlatformSystemAgent(payload: SystemAgentProfilePayload) {
  return requestApi<{ agentCode: string }>(agentApiEndpoints.createPlatformSystemAgent, { body: payload });
}
export function updatePlatformSystemAgent(agentCode: string, payload: Pick<SystemAgentProfilePayload, "name" | "description" | "modelId" | "reasoningEffort" | "avatarId" | "fastMode">) {
  return requestApi<{ agentCode: string }>(agentApiEndpoints.updatePlatformSystemAgent, { pathParams: { agentCode }, body: payload });
}
export function deletePlatformSystemAgent(agentCode: string) {
  return requestApi<{ deleted: boolean }>(agentApiEndpoints.deletePlatformSystemAgent, { pathParams: { agentCode } });
}

export function rescanPlatformSystemAgents() {
  return requestApi<PlatformSystemAgentRescanResultApi>(
    agentApiEndpoints.rescanPlatformSystemAgents,
    {
      body: {},
    }
  ).then(
    (result): PlatformSystemAgentRescanResult => ({
      ...result,
      records: result.records.map(normalizePlatformSystemAgentRecord),
    })
  );
}

export function enablePlatformSystemAgent(agentCode: string) {
  return requestApi<ManagedSystemAgentRecordApi>(
    agentApiEndpoints.enablePlatformSystemAgent,
    {
      pathParams: { agentCode },
    }
  ).then(normalizePlatformSystemAgentRecord);
}

export function disablePlatformSystemAgent(agentCode: string) {
  return requestApi<ManagedSystemAgentRecordApi>(
    agentApiEndpoints.disablePlatformSystemAgent,
    {
      pathParams: { agentCode },
    }
  ).then(normalizePlatformSystemAgentRecord);
}

export function fetchWorkspaceSystemAgents(
  workspaceCode: string,
  signal?: AbortSignal,
  includeDisabled = false
) {
  return requestApi<ManagedSystemAgentRecordApi[]>(
    agentApiEndpoints.listWorkspaceSystemAgents,
    {
      pathParams: { workspaceCode },
      signal,
    }
  ).then((records) => records.map(normalizeWorkspaceSystemAgentRecord).filter((agent) => includeDisabled || (agent.enabled && agent.available)));
}

export function enableWorkspaceSystemAgent(
  workspaceCode: string,
  agentCode: string
) {
  return requestApi<ManagedSystemAgentRecordApi>(
    agentApiEndpoints.enableWorkspaceSystemAgent,
    {
      pathParams: { workspaceCode, agentCode },
    }
  ).then(normalizeWorkspaceSystemAgentRecord);
}

export function disableWorkspaceSystemAgent(
  workspaceCode: string,
  agentCode: string
) {
  return requestApi<ManagedSystemAgentRecordApi>(
    agentApiEndpoints.disableWorkspaceSystemAgent,
    {
      pathParams: { workspaceCode, agentCode },
    }
  ).then(normalizeWorkspaceSystemAgentRecord);
}

export function fetchToolDomains(signal?: AbortSignal) {
  return requestApi<ToolCatalogDomainRecordApi[]>(
    agentApiEndpoints.getToolDomains,
    { signal }
  ).then((records) => records.map(normalizeToolCatalogDomainRecord));
}

export interface ToolCatalogIssueRecord {
  toolCode: string | null;
  toolName: string | null;
  providerId: string | null;
  packId: string | null;
  message: string;
}

export function fetchToolCatalogIssues(signal?: AbortSignal, refresh = false) {
  return requestApi<ToolCatalogIssueRecord[]>(agentApiEndpoints.getToolCatalogIssues, {
    signal,
    queryParams: { refresh: refresh || undefined },
  });
}

export function fetchToolCategories(signal?: AbortSignal) {
  return requestApi<{ code: string; name: string; toolCount: number }[]>(
    agentApiEndpoints.getToolCategories,
    { signal },
  );
}

export function listTools(payload: ListToolsQuery = {}, signal?: AbortSignal) {
  return requestApi<ToolRecordApi[]>(agentApiEndpoints.getTools, {
    queryParams: {
      domainCode: payload.domainCode,
      search: payload.search,
    },
    signal,
  }).then((records) => records.map(normalizeToolRecord));
}

export function fetchWorkspaceInternalAgents(
  workspaceCode: string,
  signal?: AbortSignal,
  includeDisabled = false
) {
  return requestApi<WorkspaceInternalAgentRecordApi[]>(
    internalAgentEndpoint(
      agentApiEndpoints.listWorkspaceInternalAgents,
      workspaceCode
    ),
    {
      pathParams: { workspaceCode },
      signal,
    }
  ).then((records) => records.map(normalizeWorkspaceInternalAgentRecord).filter((agent) => includeDisabled || agent.enabled !== false));
}

export function createWorkspaceInternalAgent(
  workspaceCode: string,
  payload: CreateInternalAgentPayload
) {
  return requestApi<{
    agentCode: string;
    name: string;
    description: string;
    rolePrompt: string;

    avatarId: string;
    groupName: string | null;
  }>(
    internalAgentEndpoint(
      "POST /api/v1/agents/workspaces/{workspaceCode}/internal",
      workspaceCode
    ),
    { pathParams: { workspaceCode }, body: payload }
  );
}

export function deleteWorkspaceInternalAgent(
  workspaceCode: string,
  agentCode: string
) {
  return requestApi<Record<string, never>>(
    internalAgentEndpoint(
      agentApiEndpoints.deleteWorkspaceInternalAgent,
      workspaceCode
    ),
    { pathParams: { workspaceCode, agentCode } }
  );
}

export function patchWorkspaceInternalAgentDefinition(
  workspaceCode: string,
  agentCode: string,
  payload: UpdateInternalAgentDefinitionPayload
) {
  return requestApi<{
    agentCode: string;
    name: string;
    description: string;
    rolePrompt: string;

    avatarId: string;
    groupName: string | null;
  }>(
    internalAgentEndpoint(
      "PATCH /api/v1/agents/workspaces/{workspaceCode}/internal/{agentCode}/definition",
      workspaceCode
    ),
    { pathParams: { workspaceCode, agentCode }, body: payload }
  );
}

export function fetchWorkspaceInternalAgent(
  workspaceCode: string,
  agentCode: string,
  signal?: AbortSignal
) {
  return requestApi<WorkspaceInternalAgentRecordApi>(
    internalAgentEndpoint(
      agentApiEndpoints.getWorkspaceInternalAgent,
      workspaceCode
    ),
    {
      pathParams: { workspaceCode, agentCode },
      signal,
    }
  ).then(normalizeWorkspaceInternalAgentRecord);
}

export function patchWorkspaceInternalAgentPolicy(
  workspaceCode: string,
  agentCode: string,
  payload: WorkspaceInternalAgentPolicyPayload
) {
  return requestApi<WorkspaceInternalAgentRecordApi>(
    internalAgentEndpoint(
      agentApiEndpoints.patchWorkspaceInternalAgentPolicy,
      workspaceCode
    ),
    {
      pathParams: { workspaceCode, agentCode },
      body: payload,
    }
  ).then(normalizeWorkspaceInternalAgentRecord);
}

export function updateWorkspaceInternalAgentToolAssignments(
  workspaceCode: string,
  agentCode: string,
  toolCodes: string[]
) {
  return requestApi<WorkspaceInternalAgentRecordApi>(
    internalAgentEndpoint(
      agentApiEndpoints.updateWorkspaceInternalAgentToolAssignments,
      workspaceCode
    ),
    {
      pathParams: { workspaceCode, agentCode },
      body: { toolCodes },
    }
  ).then(normalizeWorkspaceInternalAgentRecord);
}

export function patchWorkspaceInternalAgentMcpPolicy(
  workspaceCode: string,
  agentCode: string,
  payload: WorkspaceInternalAgentMcpPolicyPayload
) {
  return requestApi<WorkspaceInternalAgentRecordApi>(
    internalAgentEndpoint(
      agentApiEndpoints.patchWorkspaceInternalAgentMcpPolicy,
      workspaceCode
    ),
    {
      pathParams: { workspaceCode, agentCode },
      body: payload,
    }
  ).then(normalizeWorkspaceInternalAgentRecord);
}

export function updateWorkspaceInternalAgentMcpServerAssignments(
  workspaceCode: string,
  agentCode: string,
  serverCodes: string[]
) {
  return requestApi<WorkspaceInternalAgentRecordApi>(
    internalAgentEndpoint(
      agentApiEndpoints.updateWorkspaceInternalAgentMcpServerAssignments,
      workspaceCode
    ),
    {
      pathParams: { workspaceCode, agentCode },
      body: { serverCodes },
    }
  ).then(normalizeWorkspaceInternalAgentRecord);
}

export function resetWorkspaceInternalAgentPolicy(
  workspaceCode: string,
  agentCode: string
) {
  return requestApi<WorkspaceInternalAgentRecordApi>(
    agentApiEndpoints.resetWorkspaceInternalAgentPolicy,
    {
      pathParams: { workspaceCode, agentCode },
    }
  ).then(normalizeWorkspaceInternalAgentRecord);
}

function internalGroupEndpoint(workspaceCode: string) {
  return workspaceCode === PLATFORM_INTERNAL_AGENT_SCOPE
    ? "GET /api/v1/agents/registry/internal-groups"
    : `GET /api/v1/agents/workspaces/${encodeURIComponent(
        workspaceCode
      )}/internal-groups`;
}

export function fetchInternalAgentGroups(workspaceCode: string) {
  return requestApi<string[]>(internalGroupEndpoint(workspaceCode));
}

export function createInternalAgentGroup(workspaceCode: string, name: string) {
  return requestApi<{ name: string }>(
    internalGroupEndpoint(workspaceCode).replace("GET ", "POST "),
    { body: { name } }
  );
}

export function deleteInternalAgentGroup(workspaceCode: string, name: string) {
  return requestApi<Record<string, never>>(
    internalGroupEndpoint(workspaceCode).replace("GET ", "DELETE "),
    { body: { name } }
  );
}

export function getNativeToolConfig(signal?: AbortSignal) {
  return requestApi<NativeToolConfigRecord>(agentApiEndpoints.getNativeToolConfig, {
    signal,
  });
}

export function replaceNativeToolConfig(payload: {
  schemes: NativeToolSchemeRecord[];
  defaultSchemeCode: string;
}) {
  return requestApi<NativeToolConfigRecord>(agentApiEndpoints.replaceNativeToolConfig, {
    body: payload,
  });
}

export function patchAgentNativeScheme(
  workspaceCode: string,
  agentCode: string,
  schemeCode: string | null
) {
  return requestApi<string | null>(
    internalAgentEndpoint(agentApiEndpoints.patchAgentNativeScheme, workspaceCode),
    {
      pathParams: { workspaceCode, agentCode },
      body: { schemeCode },
    }
  );
}
