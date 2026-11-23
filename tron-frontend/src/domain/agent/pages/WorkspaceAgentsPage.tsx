import { INTERNAL_AGENT_DIALOG_CLASS, INTERNAL_AGENT_SECTIONS, InternalAgentProfileFields, InternalAgentRuntimeFields } from "../components/InternalAgentFormFields";
import { AgentPolicyFields } from "../components/AgentPolicyFields";
import { AgentActivationSwitch } from "../components/AgentActivationSwitch";
import { reconcileResourceDraft } from "../model/ResourceAssignmentDraft";
import { Fragment, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  fetchToolDomains,
  fetchWorkspaceInternalAgents,
  getNativeToolConfig,
  deleteWorkspaceInternalAgent,
  fetchWorkspaceSystemAgents,
  listTools,
  patchWorkspaceInternalAgentMcpPolicy,
  patchWorkspaceInternalAgentPolicy,
  patchWorkspaceInternalAgentDefinition,
  resetWorkspaceInternalAgentPolicy,
  updateWorkspaceInternalAgentMcpServerAssignments,
  updateWorkspaceInternalAgentToolAssignments,
} from "@/domain/agent/api";
import { AgentPolicySwitch, AgentStatusBadge } from "@/domain/agent/components";
import {
  translateInheritedFrom,
  translateInternalAgentGroupName,
  translateInternalAgentName,
  translateSystemAgentDescription,
  translateSystemAgentSource,
} from "@/domain/agent/constants/agentPresentation";
import {
  listToolDomainsMock,
  listToolsMock,
  listWorkspaceInternalAgentsMock,
  deleteWorkspaceInternalAgentMock,
  patchWorkspaceInternalAgentMcpPolicyMock,
  patchWorkspaceInternalAgentPolicyMock,
  patchWorkspaceInternalAgentDefinitionMock,
  resetWorkspaceInternalAgentPolicyMock,
  updateWorkspaceInternalAgentMcpServerAssignmentsMock,
  updateWorkspaceInternalAgentToolAssignmentsMock,
  listWorkspaceSystemAgentsMock,
  setWorkspaceSystemAgentEnabledMock,
} from "@/domain/agent/mock";
import {
  disableWorkspaceSystemAgent,
  enableWorkspaceSystemAgent,
} from "@/domain/agent/api";
import type {
  NativeToolSchemeRecord,
  PlatformToolRecord,
  ToolCatalogDomainRecord,
  WorkspaceInternalAgentMcpPolicyPayload,
  WorkspaceInternalAgentPolicyPayload,
  WorkspaceInternalAgentRecord,
  WorkspaceSystemAgentRecord,
} from "@/domain/agent/model";
import {
  fetchWorkspaceMcpServers,
  fetchPlatformMcpServers,
  listPlatformMcpServersMock,
  listWorkspaceMcpServersMock,
} from "@/domain/mcp";
import type { WorkspaceMcpServerRecord } from "@/domain/mcp/model";
import { getServiceDataSource } from "@/shared/api/serviceConfig";
import { DataTable } from "@/components/data-table";
import { Field, SelectInput, TextInput } from "@/components/forms";
import { CreateInternalAgentDialog } from "../components/CreateInternalAgentDialog";
import { InternalAgentSkillField } from "../components/InternalAgentSkillField";
import {
  setAgentSkills,
  CAPABILITY_RESOURCES_CHANGED,
} from "@/domain/skill/api";
import { InternalAgentKnowledgeField } from "../components/InternalAgentKnowledgeField";
import { AgentWorkspaceField } from "../components/AgentWorkspaceField";
import { persistAgentWorkspaceGrants } from "../hooks/agentWorkspaceGrants";
import {
  InternalAgentAssignmentDialog,
  type InternalAgentAssignmentKind,
} from "../components/InternalAgentAssignmentDialog";
import {
  PLATFORM_INTERNAL_AGENT_SCOPE,
  notifyInternalAgentRegistryChanged,
} from "../model";
import { Button } from "@/components/ui/button";
import { InfoTooltip } from "@/components/info-tooltip";
import { SearchField } from "@/components/search-field";
import { TablePagination } from "@/components/table-pagination";
import {
  PageHeader,
  SurfaceCard,
  useTablePagination,
  useWorkspaceSpaces,
} from "@/shared";
import { useToast } from "@/components/ui/toast";
import { SelectItem } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

import {
  AgentAvatar,
} from "../components/AgentAvatarPicker";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useQueryClient } from "@tanstack/react-query";
import { setAgentKnowledgeLibraries } from "@/domain/knowledge-base/api/knowledgeBaseApi";
import { InternalAgentBrowseItem } from "../components/InternalAgentBrowseItem";
import { useInternalAgentGroups } from "../hooks/useInternalAgentGroups";
import {
  groupInternalAgents,
  internalAgentGroupName,
} from "../model/internalAgentGroups";

interface InternalAgentDraft {
  description: string;
  enabled: boolean;
  groupName: string;
  avatarId: string;
  candidateModelCodes: string[];

  name: string;
  rolePrompt: string;
  maxSteps: number;
  maxToolCalls: number;
  maxParallelToolCalls: number;
  mcpAccessMode: WorkspaceInternalAgentRecord["mcpAccessMode"];
  selectedServerCodes: string[];
  selectedToolCodes: string[];
  toolAssignmentMode: WorkspaceInternalAgentRecord["toolAssignmentMode"];
  selectedCategoryCodes?: string[];
  nativeToolSchemeCode?: string | null;
}

type InternalSaveState = "idle" | "pending" | "saving" | "saved" | "error";

function normalizeToolCodes(toolCodes: string[]) {
  return [...new Set(toolCodes)].sort((left, right) =>
    left.localeCompare(right)
  );
}

function normalizeServerCodes(serverCodes: string[]) {
  return [...new Set(serverCodes)].sort((left, right) =>
    left.localeCompare(right)
  );
}

function hasSameToolCodes(left: string[], right: string[]) {
  if (left.length !== right.length) {
    return false;
  }

  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) {
      return false;
    }
  }

  return true;
}

function hasSameServerCodes(left: string[], right: string[]) {
  if (left.length !== right.length) {
    return false;
  }

  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) {
      return false;
    }
  }

  return true;
}

function draftFromRecord(
  record: WorkspaceInternalAgentRecord
): InternalAgentDraft {
  return {
    enabled: record.enabled !== false,
    avatarId: record.avatarId || "robot",
    candidateModelCodes: normalizeToolCodes(record.candidateModelCodes ?? []),
    groupName: record.groupName || "",
    name: record.name,
    description: record.description,
    rolePrompt: record.rolePrompt,

    maxSteps: record.maxSteps,
    maxToolCalls: record.maxToolCalls,
    maxParallelToolCalls: record.maxParallelToolCalls,
    mcpAccessMode: record.mcpAccessMode,
    selectedServerCodes: normalizeServerCodes(record.selectedServerCodes),
    selectedToolCodes: normalizeToolCodes(record.selectedToolCodes),
    toolAssignmentMode: record.toolAssignmentMode,
    selectedCategoryCodes: record.selectedCategoryCodes ?? [],
    nativeToolSchemeCode: record.nativeToolSchemeCode ?? "",
  };
}

function mergeSavedDraft(
  record: WorkspaceInternalAgentRecord,
  submitted: InternalAgentDraft,
  latest: InternalAgentDraft
) {
  let merged = draftFromRecord(record);
  for (const key of Object.keys(merged) as (keyof InternalAgentDraft)[]) {
    const currentValue = latest[key];
    const submittedValue = submitted[key];
    const changedSinceSubmission =
      Array.isArray(currentValue) && Array.isArray(submittedValue)
        ? !hasSameToolCodes(currentValue, submittedValue)
        : currentValue !== submittedValue;
    if (changedSinceSubmission) merged = { ...merged, [key]: currentValue };
  }
  return merged;
}

function hasDraftChanges(
  record: WorkspaceInternalAgentRecord,
  draft: InternalAgentDraft
) {
  return (
    (record.enabled !== false) !== draft.enabled ||
    (record.groupName || "") !== draft.groupName ||
    (record.avatarId || "robot") !== draft.avatarId ||
    !hasSameToolCodes(normalizeToolCodes(record.candidateModelCodes ?? []), normalizeToolCodes(draft.candidateModelCodes)) ||
    record.name !== draft.name ||
    record.description !== draft.description ||

    record.rolePrompt !== draft.rolePrompt ||
    record.toolAssignmentMode !== draft.toolAssignmentMode ||
    JSON.stringify(record.selectedCategoryCodes ?? []) !== JSON.stringify(draft.selectedCategoryCodes ?? []) ||
    record.maxSteps !== draft.maxSteps ||
    record.maxToolCalls !== draft.maxToolCalls ||
    record.maxParallelToolCalls !== draft.maxParallelToolCalls ||
    record.mcpAccessMode !== draft.mcpAccessMode ||
    (record.nativeToolSchemeCode ?? "") !== (draft.nativeToolSchemeCode ?? "") ||
    !hasSameServerCodes(
      normalizeServerCodes(record.selectedServerCodes),
      normalizeServerCodes(draft.selectedServerCodes)
    ) ||
    !hasSameToolCodes(
      normalizeToolCodes(record.selectedToolCodes),
      normalizeToolCodes(draft.selectedToolCodes)
    )
  );
}

function buildDraft(
  baseRecord: WorkspaceInternalAgentRecord,
  currentDraft: InternalAgentDraft,
  patch: Partial<InternalAgentDraft>
) {
  return {
    ...currentDraft,
    ...patch,
    selectedServerCodes: patch.selectedServerCodes
      ? normalizeServerCodes(patch.selectedServerCodes)
      : currentDraft.selectedServerCodes ??
        normalizeServerCodes(baseRecord.selectedServerCodes),
    selectedToolCodes: patch.selectedToolCodes
      ? normalizeToolCodes(patch.selectedToolCodes)
      : currentDraft.selectedToolCodes ??
        normalizeToolCodes(baseRecord.selectedToolCodes),
  };
}

export type WorkspaceAgentCategory = "internal" | "system";

interface WorkspaceAgentsPageProps {
  category: WorkspaceAgentCategory;
  allowCreate?: boolean;
  scope?: "platform" | "workspace";
}

export function WorkspaceAgentsPage({
  category,
  scope = "workspace",
  allowCreate = true,
}: WorkspaceAgentsPageProps) {
  const { t } = useTranslation(["pages"]);
  const toast = useToast();
  const dataSource = getServiceDataSource("agent");
  const mcpDataSource = getServiceDataSource("mcp");
  const { selectedPrimarySpace, selectedSecondarySpace } = useWorkspaceSpaces();
  const activeWorkspaceCode =
    scope === "platform"
      ? PLATFORM_INTERNAL_AGENT_SCOPE
      : selectedSecondarySpace?.workspaceCode ??
        selectedPrimarySpace?.workspaceCode ??
        "";
  const activeWorkspaceLabel =
    selectedSecondarySpace?.label ??
    selectedPrimarySpace?.name ??
    t("pages:workspaceAgents.workspace.unselected", {
      defaultValue: "No workspace selected",
    });

  const queryClient = useQueryClient();
  const [editingAgentCode, setEditingAgentCode] = useState<string | null>(null);
  const [editingAgentTab, setEditingAgentTab] = useState("profile");
  const [restoreBaselines, setRestoreBaselines] = useState<
    Record<string, WorkspaceInternalAgentRecord>
  >({});
  const [skillDrafts, setSkillDrafts] = useState<Record<string, string[]>>({});
  const [knowledgeDrafts, setKnowledgeDrafts] = useState<
    Record<string, string[]>
  >({});
  const [workspaceDrafts, setWorkspaceDrafts] = useState<
    Record<string, string[]>
  >({});
  const [assignmentDialog, setAssignmentDialog] = useState<{
    agentCode: string;
    workspaceCode: string;
    kind: InternalAgentAssignmentKind;
  } | null>(null);
  const [internalAgents, setInternalAgents] = useState<
    WorkspaceInternalAgentRecord[]
  >([]);
  const [toolDomains, setToolDomains] = useState<ToolCatalogDomainRecord[]>([]);
  const [tools, setTools] = useState<PlatformToolRecord[]>([]);
  const [nativeSchemes, setNativeSchemes] = useState<NativeToolSchemeRecord[]>([]);
  const [workspaceMcpServers, setWorkspaceMcpServers] = useState<
    WorkspaceMcpServerRecord[]
  >([]);
  const [systemAgents, setSystemAgents] = useState<
    WorkspaceSystemAgentRecord[]
  >([]);
  const [internalDrafts, setInternalDrafts] = useState<
    Record<string, InternalAgentDraft>
  >({});
  const [internalSaveStates, setInternalSaveStates] = useState<
    Record<string, InternalSaveState>
  >({});
  const [internalSaveErrors, setInternalSaveErrors] = useState<
    Record<string, string>
  >({});
  const [internalSearch, setInternalSearch] = useState("");
  const [groupFilter, setGroupFilter] = useState("all");
  const {
    groups: catalogGroups,
    create: createGroup,
    remove: removeGroup,
  } = useInternalAgentGroups(
    category === "internal" ? activeWorkspaceCode : ""
  );
  const [groupManagerOpen, setGroupManagerOpen] = useState(false);
  const [groupToDelete, setGroupToDelete] = useState<string | null>(null);
  const [agentToDelete, setAgentToDelete] =
    useState<WorkspaceInternalAgentRecord | null>(null);
  const [systemSearch, setSystemSearch] = useState("");
  const [debouncedSystemSearch, setDebouncedSystemSearch] = useState("");
  const [loading, setLoading] = useState(false);
  const [busyKeys, setBusyKeys] = useState<Record<string, boolean>>({});
  const internalDraftsRef = useRef<Record<string, InternalAgentDraft>>({});

  const pageT = (
    key: string,
    defaultValue: string,
    options?: Record<string, unknown>
  ) => t(`pages:workspaceAgents.${key}`, { defaultValue, ...options });
  const internalPageT = (
    key: string,
    defaultValue: string,
    options?: Record<string, unknown>
  ) => t(`pages:workspaceAgents.internal.${key}`, { defaultValue, ...options });

  const systemTableT = (
    key: string,
    defaultValue: string,
    options?: Record<string, unknown>
  ) => t(`pages:workspaceAgents.table.${key}`, { defaultValue, ...options });
  const internalAgentsPage = category === "internal";
  const pageTitle =
    scope === "platform"
      ? internalPageT("inheritance.title", "Global internal agents")
      : internalAgentsPage
      ? internalPageT("title", "Internal Agents")
      : systemTableT("title", "Inherited System Agents");

  function isBusy(code: string) {
    return Boolean(busyKeys[code]);
  }

  function setBusy(code: string, busy: boolean) {
    setBusyKeys((current) => {
      if (busy) {
        if (current[code]) {
          return current;
        }
        return {
          ...current,
          [code]: true,
        };
      }

      if (!current[code]) {
        return current;
      }

      const next = { ...current };
      delete next[code];
      return next;
    });
  }

  function setInternalSaveState(
    agentCode: string,
    nextState: InternalSaveState
  ) {
    setInternalSaveStates((current) => {
      if (current[agentCode] === nextState) {
        return current;
      }
      return {
        ...current,
        [agentCode]: nextState,
      };
    });
  }

  function updateDraftMap(next: WorkspaceInternalAgentRecord[]) {
    const nextDrafts = next.reduce<Record<string, InternalAgentDraft>>(
      (accum, item) => {
        accum[item.agentCode] = draftFromRecord(item);
        return accum;
      },
      {}
    );
    internalDraftsRef.current = nextDrafts;
    setInternalDrafts(nextDrafts);
  }

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDebouncedSystemSearch(systemSearch.trim().toLowerCase());
    }, 200);

    return () => window.clearTimeout(timer);
  }, [systemSearch]);

  useEffect(() => {
    setEditingAgentCode(null);
    setSkillDrafts({});
    setKnowledgeDrafts({});
    setWorkspaceDrafts({});
    setRestoreBaselines({});
    void refreshWorkspaceData();
  }, [activeWorkspaceCode, category, dataSource, mcpDataSource, scope]);

  useEffect(() => {
    const changed = () => {
      void refreshWorkspaceData(true);
      void queryClient.invalidateQueries({ queryKey: ["agent-skills"] });
    };
    window.addEventListener(CAPABILITY_RESOURCES_CHANGED, changed);
    return () =>
      window.removeEventListener(CAPABILITY_RESOURCES_CHANGED, changed);
  }, [activeWorkspaceCode, scope, category, internalAgents]);

  const filteredSystemAgents = systemAgents.filter((record) => {
    const normalizedSearch = debouncedSystemSearch;
    if (!normalizedSearch) {
      return true;
    }

    return [
      record.name,
      record.agentCode,
      record.description,
      record.inheritedFrom,
      record.status,
      translateSystemAgentDescription(t, record),
      translateSystemAgentSource(t, record.source),
      translateInheritedFrom(t, record.inheritedFrom),
    ]
      .join(" ")
      .toLowerCase()
      .includes(normalizedSearch);
  });

  const systemPagination = useTablePagination(filteredSystemAgents);

  function resolveWorkspacePolicyLabel(record: WorkspaceSystemAgentRecord) {
    if (!record.available) {
      return systemTableT("policyUnavailable", "Unavailable");
    }
    if (!record.platformEnabled) {
      return systemTableT("policyPlatformDisabled", "Platform disabled");
    }
    if (record.workspaceOverrideDisabled) {
      return systemTableT("policyLocalDisable", "Local disable");
    }
    return systemTableT("policyInherited", "Platform baseline");
  }

  function canToggleWorkspaceAgent(record: WorkspaceSystemAgentRecord) {
    if (!record.available) {
      return false;
    }
    return record.workspaceOverrideDisabled || record.platformEnabled;
  }

  function hasToolChanges(
    record: WorkspaceInternalAgentRecord,
    draft: InternalAgentDraft
  ) {
    return !hasSameToolCodes(
      normalizeToolCodes(record.selectedToolCodes),
      normalizeToolCodes(draft.selectedToolCodes)
    );
  }

  function hasMcpServerChanges(
    record: WorkspaceInternalAgentRecord,
    draft: InternalAgentDraft
  ) {
    return !hasSameServerCodes(
      normalizeServerCodes(record.selectedServerCodes),
      normalizeServerCodes(draft.selectedServerCodes)
    );
  }

  function setInternalDraft(
    agentCode: string,
    patch: Partial<InternalAgentDraft>
  ) {
    const baseRecord = internalAgents.find(
      (item) => item.agentCode === agentCode
    );
    if (!baseRecord) {
      return;
    }

    // React may defer state updaters. Keep consecutive input events on the latest
    // draft and compute save status without relying on updater execution timing.
    const nextDraft = buildDraft(
      baseRecord,
      internalDraftsRef.current[agentCode] ?? draftFromRecord(baseRecord),
      patch
    );
    internalDraftsRef.current = {
      ...internalDraftsRef.current,
      [agentCode]: nextDraft,
    };
    setInternalDrafts((current) => ({ ...current, [agentCode]: nextDraft }));

    setInternalSaveState(
      agentCode,
      hasDraftChanges(baseRecord, nextDraft) ? "pending" : "idle"
    );
  }

  function openAssignmentDialog(
    agentCode: string,
    kind: InternalAgentAssignmentKind
  ) {
    setAssignmentDialog({
      agentCode,
      kind,
      workspaceCode: activeWorkspaceCode,
    });
  }

  async function refreshWorkspaceData(preserveDrafts = false) {
    if (!activeWorkspaceCode) {
      setInternalAgents([]);
      setToolDomains([]);
      setTools([]);
      setNativeSchemes([]);
      setWorkspaceMcpServers([]);
      setSystemAgents([]);
      internalDraftsRef.current = {};
      setInternalDrafts({});
      setInternalSaveStates({});
      setInternalSaveErrors({});
      setBusyKeys({});
      return;
    }

    setLoading(true);
    try {
      if (internalAgentsPage) {
        const [
          nextInternalAgents,
          nextToolDomains,
          nextTools,
          nextNativeSchemes,
          nextWorkspaceMcpServers,
        ] = await Promise.all([
          dataSource === "api"
            ? fetchWorkspaceInternalAgents(activeWorkspaceCode, undefined, true)
            : Promise.resolve(
                listWorkspaceInternalAgentsMock(activeWorkspaceCode)
              ),
          dataSource === "api"
            ? fetchToolDomains()
            : Promise.resolve(listToolDomainsMock()),
          dataSource === "api" ? listTools() : Promise.resolve(listToolsMock()),
          dataSource === "api"
            ? getNativeToolConfig()
            : Promise.resolve({ schemes: [], defaultSchemeCode: "default", agentOverrides: {} }),
          scope === "platform"
            ? (mcpDataSource === "api"
                ? fetchPlatformMcpServers()
                : Promise.resolve(listPlatformMcpServersMock())
              ).then((servers) =>
                servers.map(
                  (server): WorkspaceMcpServerRecord => ({
                    ...server,
                    platformEnabled: server.enabled,
                    effectiveEnabled: server.enabled,
                    source: "platform",
                    workspaceOverrideDisabled: false,
                  })
                )
              )
            : mcpDataSource === "api"
            ? fetchWorkspaceMcpServers(activeWorkspaceCode)
            : Promise.resolve(listWorkspaceMcpServersMock(activeWorkspaceCode)),
        ]);

        setInternalAgents(nextInternalAgents);
        setToolDomains(nextToolDomains);
        setTools(nextTools);
        setNativeSchemes(nextNativeSchemes.schemes ?? []);
        setWorkspaceMcpServers(nextWorkspaceMcpServers);
        if (preserveDrafts) {
          const nextDrafts = Object.fromEntries(
            nextInternalAgents.map((agent) => [
              agent.agentCode,
              {
                ...(internalDraftsRef.current[agent.agentCode] ??
                  draftFromRecord(agent)),
                selectedToolCodes: reconcileResourceDraft(
                  internalDraftsRef.current[agent.agentCode]
                    ?.selectedToolCodes ?? agent.selectedToolCodes,
                  internalAgents.find(
                    (previous) => previous.agentCode === agent.agentCode
                  )?.selectedToolCodes ?? agent.selectedToolCodes,
                  agent.selectedToolCodes,
                  nextTools
                    .filter((tool) => tool.assignable !== false)
                    .map((tool) => tool.toolCode)
                ),
                selectedServerCodes: reconcileResourceDraft(
                  internalDraftsRef.current[agent.agentCode]
                    ?.selectedServerCodes ?? agent.selectedServerCodes,
                  internalAgents.find(
                    (previous) => previous.agentCode === agent.agentCode
                  )?.selectedServerCodes ?? agent.selectedServerCodes,
                  agent.selectedServerCodes,
                  nextWorkspaceMcpServers
                    .filter(
                      (server) =>
                        server.effectiveEnabled && server.assignable !== false
                    )
                    .map((server) => server.serverCode)
                ),
              },
            ])
          );
          internalDraftsRef.current = nextDrafts;
          setInternalDrafts(nextDrafts);
        } else updateDraftMap(nextInternalAgents);
        setInternalSaveStates({});
        setInternalSaveErrors({});
      } else {
        const nextSystemAgents =
          dataSource === "api"
            ? await fetchWorkspaceSystemAgents(activeWorkspaceCode, undefined, true)
            : await Promise.resolve(
                listWorkspaceSystemAgentsMock(activeWorkspaceCode)
              );
        setSystemAgents(nextSystemAgents);
      }
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : pageT("errors.load", "Workspace agent policy could not be loaded")
      );
    } finally {
      setLoading(false);
    }
  }

  async function refreshCreatedAgent() {
    const records =
      dataSource === "api"
        ? await fetchWorkspaceInternalAgents(activeWorkspaceCode, undefined, true)
        : listWorkspaceInternalAgentsMock(activeWorkspaceCode);
    setInternalAgents(records);
    setInternalDrafts((current) =>
      Object.fromEntries(
        records.map((record) => [
          record.agentCode,
          current[record.agentCode] ?? draftFromRecord(record),
        ])
      )
    );
  }

  async function handleDeleteGroup(name: string) {
    try {
      await removeGroup(name);
      if (groupFilter === `group:${name}`) setGroupFilter("all");
      await refreshCreatedAgent();
      notifyInternalAgentRegistryChanged();
      toast.success(internalPageT("groups.deleted", "Group deleted"));
      setGroupToDelete(null);
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : internalPageT("groups.deleteFailed", "Could not delete group")
      );
    }
  }

  async function handleDeleteAgent(agent: WorkspaceInternalAgentRecord) {
    const key = `internal-delete:${agent.agentCode}`;
    if (isBusy(key)) return;
    setBusy(key, true);
    try {
      if (dataSource === "api")
        await deleteWorkspaceInternalAgent(
          activeWorkspaceCode,
          agent.agentCode
        );
      else
        deleteWorkspaceInternalAgentMock(activeWorkspaceCode, agent.agentCode);
      setAgentToDelete(null);
      internalDraftsRef.current = Object.fromEntries(
        Object.entries(internalDraftsRef.current).filter(
          ([code]) => code !== agent.agentCode
        )
      );
      setInternalDrafts((current) => {
        const next = { ...current };
        delete next[agent.agentCode];
        return next;
      });
      setInternalAgents((current) =>
        current.filter((record) => record.agentCode !== agent.agentCode)
      );
      await refreshCreatedAgent();
      await Promise.all(
        ["knowledge-base-agents", "knowledge-library-agent-grants"].map(
          (keyName) => queryClient.invalidateQueries({ queryKey: [keyName] })
        )
      );
      notifyInternalAgentRegistryChanged();
      toast.success(internalPageT("groups.agentDeleted", "Agent deleted"));
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : internalPageT("groups.deleteAgentFailed", "Could not delete agent")
      );
    } finally {
      setBusy(key, false);
    }
  }

  async function refreshInternalAgent(
    agentCode: string,
    submittedDraft?: InternalAgentDraft
  ) {
    if (!activeWorkspaceCode) return;

    const nextInternalAgents =
      dataSource === "api"
        ? await fetchWorkspaceInternalAgents(activeWorkspaceCode, undefined, true)
        : listWorkspaceInternalAgentsMock(activeWorkspaceCode);
    const savedAgent = nextInternalAgents.find(
      (agent) => agent.agentCode === agentCode
    );
    if (!savedAgent)
      throw new Error(
        internalPageT("errors.save", "Internal agent policy update failed")
      );

    // Other rows may have pending edits while this row's save is in flight.
    // Apply the authoritative response only to the row whose save completed.
    setInternalAgents((current) =>
      current.map((agent) =>
        agent.agentCode === agentCode ? savedAgent : agent
      )
    );
    // Keep edits made while saving; only unchanged fields adopt the server response.
    const latestDraft = internalDraftsRef.current[agentCode];
    const savedDraft =
      submittedDraft && latestDraft
        ? mergeSavedDraft(savedAgent, submittedDraft, latestDraft)
        : draftFromRecord(savedAgent);
    internalDraftsRef.current = {
      ...internalDraftsRef.current,
      [agentCode]: savedDraft,
    };
    setInternalDrafts((current) => ({ ...current, [agentCode]: savedDraft }));
    return hasDraftChanges(savedAgent, savedDraft);
  }

  async function handleSaveInternalAgent(agentCode: string) {
    const row =
      restoreBaselines[agentCode] ??
      internalAgents.find((item) => item.agentCode === agentCode);
    const draft = internalDraftsRef.current[agentCode];
    if (
      !row ||
      !draft ||
      isBusy(`internal-save:${agentCode}`) ||
      isBusy(`internal-reset:${agentCode}`)
    ) {
      return;
    }
    if (
      !hasDraftChanges(row, draft) &&
      !skillDrafts[agentCode] &&
      !knowledgeDrafts[agentCode] &&
      !workspaceDrafts[agentCode] &&
      !restoreBaselines[agentCode]
    ) {
      return;
    }

    for (const key of ["maxSteps", "maxToolCalls", "maxParallelToolCalls"] as const) {
      if (!Number.isSafeInteger(draft[key]) || draft[key] < (key === "maxParallelToolCalls" ? 1 : 0) || draft[key] > 4294967295) {
        toast.error(t(`workspaceAgents.internal.labels.${key}`, { defaultValue: key }));
        return;
      }
    }

    const payload: WorkspaceInternalAgentPolicyPayload = {};
    if (JSON.stringify(row.selectedCategoryCodes ?? []) !== JSON.stringify(draft.selectedCategoryCodes ?? [])) payload.selectedCategoryCodes = draft.selectedCategoryCodes ?? [];
    if (row.toolAssignmentMode !== draft.toolAssignmentMode) {
      payload.toolAssignmentMode = draft.toolAssignmentMode;
    }
    if (row.maxSteps !== draft.maxSteps) {
      payload.maxSteps = draft.maxSteps;
    }
    if (row.maxToolCalls !== draft.maxToolCalls) {
      payload.maxToolCalls = draft.maxToolCalls;
    }
    if (row.maxParallelToolCalls !== draft.maxParallelToolCalls) {
      payload.maxParallelToolCalls = draft.maxParallelToolCalls;
    }
    if ((row.nativeToolSchemeCode ?? "") !== (draft.nativeToolSchemeCode ?? "")) {
      payload.nativeToolSchemeCode = draft.nativeToolSchemeCode ?? "";
    }
    const mcpPayload: WorkspaceInternalAgentMcpPolicyPayload = {};
    if (row.mcpAccessMode !== draft.mcpAccessMode) {
      mcpPayload.mcpAccessMode = draft.mcpAccessMode;
    }

    const shouldSaveTools =
      draft.toolAssignmentMode === "selected_tools" &&
      (row.toolAssignmentMode !== "selected_tools" ||
        hasToolChanges(row, draft));
    const shouldSaveServers =
      draft.mcpAccessMode === "selected_servers"
        ? hasMcpServerChanges(row, draft)
        : false;

    setBusy(`internal-save:${agentCode}`, true);
    setInternalSaveState(agentCode, "saving");
    try {
      if (!row.builtIn || scope === "platform") {
        if (
          draft.groupName &&
          (row.groupName || "") !== draft.groupName &&
          !catalogGroups.includes(draft.groupName)
        )
          await createGroup(draft.groupName);
        if (restoreBaselines[agentCode]) {
          if (dataSource === "api")
            await resetWorkspaceInternalAgentPolicy(
              activeWorkspaceCode,
              agentCode
            );
          else
            resetWorkspaceInternalAgentPolicyMock(
              activeWorkspaceCode,
              agentCode
            );
        }
        if (
          (row.enabled !== false) !== draft.enabled ||
          (row.groupName || "") !== draft.groupName ||
          row.name !== draft.name ||
          row.description !== draft.description ||
          row.rolePrompt !== draft.rolePrompt ||

          (row.avatarId || "robot") !== draft.avatarId ||
          !hasSameToolCodes(normalizeToolCodes(row.candidateModelCodes ?? []), normalizeToolCodes(draft.candidateModelCodes))
        ) {
          const definition = {
            ...((row.enabled !== false) !== draft.enabled ? { enabled: draft.enabled } : {}),
            ...(!hasSameToolCodes(normalizeToolCodes(row.candidateModelCodes ?? []), normalizeToolCodes(draft.candidateModelCodes)) ? { candidateModelCodes: draft.candidateModelCodes } : {}),
            ...((row.groupName || "") !== draft.groupName
              ? { groupName: draft.groupName }
              : {}),
            ...((row.avatarId || "robot") !== draft.avatarId
              ? { avatarId: draft.avatarId }
              : {}),

            ...(row.name !== draft.name ? { name: draft.name } : {}),
            ...(row.description !== draft.description ? { description: draft.description } : {}),
            ...(row.rolePrompt !== draft.rolePrompt
              ? { rolePrompt: draft.rolePrompt }
              : {}),
          };
          if (dataSource === "api") {
            await patchWorkspaceInternalAgentDefinition(
              activeWorkspaceCode,
              agentCode,
              definition
            );
          } else {
            patchWorkspaceInternalAgentDefinitionMock(
              activeWorkspaceCode,
              agentCode,
              definition
            );
          }
        }
        // Persist the visible selection before changing its policy mode. The all-tools
        // response is an effective list, so it may differ from stored assignments.
        if (shouldSaveTools) {
          if (dataSource === "api") {
            await updateWorkspaceInternalAgentToolAssignments(
              activeWorkspaceCode,
              agentCode,
              draft.selectedToolCodes
            );
          } else {
            updateWorkspaceInternalAgentToolAssignmentsMock(
              activeWorkspaceCode,
              agentCode,
              draft.selectedToolCodes
            );
          }
        }
        if (Object.keys(payload).length > 0) {
          if (dataSource === "api") {
            await patchWorkspaceInternalAgentPolicy(
              activeWorkspaceCode,
              agentCode,
              payload
            );
          } else {
            patchWorkspaceInternalAgentPolicyMock(
              activeWorkspaceCode,
              agentCode,
              payload
            );
          }
        }
        if (shouldSaveServers) {
          if (dataSource === "api") {
            await updateWorkspaceInternalAgentMcpServerAssignments(
              activeWorkspaceCode,
              agentCode,
              draft.selectedServerCodes
            );
          } else {
            updateWorkspaceInternalAgentMcpServerAssignmentsMock(
              activeWorkspaceCode,
              agentCode,
              draft.selectedServerCodes
            );
          }
        }
        if (Object.keys(mcpPayload).length > 0) {
          if (dataSource === "api") {
            await patchWorkspaceInternalAgentMcpPolicy(
              activeWorkspaceCode,
              agentCode,
              mcpPayload
            );
          } else {
            patchWorkspaceInternalAgentMcpPolicyMock(
              activeWorkspaceCode,
              agentCode,
              mcpPayload
            );
          }
        }
        if (skillDrafts[agentCode]) {
          await setAgentSkills(agentCode, skillDrafts[agentCode]);
          await queryClient.invalidateQueries({ queryKey: ["agent-skills"] });
        }
        if (knowledgeDrafts[agentCode]) {
          await setAgentKnowledgeLibraries(
            agentCode,
            knowledgeDrafts[agentCode]
          );
          await Promise.all(
            [
              "knowledge-agent-libraries",
              "knowledge-base",
              "knowledge-bases",
              "platform-knowledge-bases",
              "knowledge-library-agent-grants",
            ].map((key) => queryClient.invalidateQueries({ queryKey: [key] }))
          );
        }
      }
      if (
        scope === "platform" &&
        agentCode !== "main-agent" &&
        workspaceDrafts[agentCode]
      ) {
        await persistAgentWorkspaceGrants(
          agentCode,
          workspaceDrafts[agentCode]
        );
        await queryClient.invalidateQueries({
          queryKey: ["agent-workspace-grants", agentCode],
        });
      }
      const hasPendingChanges = await refreshInternalAgent(agentCode, draft);
      setSkillDrafts((current) => {
        const next = { ...current };
        delete next[agentCode];
        return next;
      });
      setKnowledgeDrafts((current) => {
        const next = { ...current };
        delete next[agentCode];
        return next;
      });
      setWorkspaceDrafts((current) => {
        const next = { ...current };
        delete next[agentCode];
        return next;
      });
      notifyInternalAgentRegistryChanged();
      setInternalSaveState(agentCode, hasPendingChanges ? "pending" : "saved");
      if (!hasPendingChanges) {
        setEditingAgentCode(null);
        toast.success(internalPageT("status.saved", "Saved"));
      }
    } catch (error) {
      const message =
        error instanceof Error
          ? error.message
          : internalPageT("errors.save", "Internal agent policy update failed");
      setInternalSaveErrors((current) => ({
        ...current,
        [agentCode]: message,
      }));
      setInternalSaveState(agentCode, "error");
      toast.error(message);
    } finally {
      setBusy(`internal-save:${agentCode}`, false);
    }
  }

  async function handleRestoreInheritance(agentCode: string) {
    setBusy(`internal-reset:${agentCode}`, true);
    try {
      const platformRecords =
        dataSource === "api"
          ? await fetchWorkspaceInternalAgents(PLATFORM_INTERNAL_AGENT_SCOPE, undefined, true)
          : listWorkspaceInternalAgentsMock(PLATFORM_INTERNAL_AGENT_SCOPE);
      const baseline = platformRecords.find(
        (record) => record.agentCode === agentCode
      );
      if (!baseline)
        throw new Error(
          internalPageT("errors.reset", "Internal policy reset failed")
        );
      const draft = draftFromRecord(baseline);
      setRestoreBaselines((current) => ({ ...current, [agentCode]: baseline }));
      internalDraftsRef.current = {
        ...internalDraftsRef.current,
        [agentCode]: draft,
      };
      setInternalDrafts((current) => ({ ...current, [agentCode]: draft }));
      setInternalSaveState(agentCode, "pending");
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : internalPageT("errors.reset", "Internal policy reset failed")
      );
    } finally {
      setBusy(`internal-reset:${agentCode}`, false);
    }
  }

  async function handleToggleSystemAgent(row: WorkspaceSystemAgentRecord) {
    if (
      !activeWorkspaceCode ||
      isBusy(`system:${row.agentCode}`) ||
      !canToggleWorkspaceAgent(row)
    ) {
      return;
    }

    setBusy(`system:${row.agentCode}`, true);
    try {
      const updated =
        dataSource === "api"
          ? row.enabled
            ? await disableWorkspaceSystemAgent(
                activeWorkspaceCode,
                row.agentCode
              )
            : await enableWorkspaceSystemAgent(
                activeWorkspaceCode,
                row.agentCode
              )
          : setWorkspaceSystemAgentEnabledMock(
              activeWorkspaceCode,
              row.agentCode,
              !row.enabled
            );
      await refreshWorkspaceData();
      if (row.enabled) {
        toast.success(
          pageT("messages.disabled", "{{name}} disabled in {{workspace}}", {
            name: updated.name,
            workspace: activeWorkspaceLabel,
          })
        );
      } else if (updated.enabled) {
        toast.success(
          pageT("messages.enabled", "{{name}} now follows platform policy", {
            name: updated.name,
          })
        );
      } else {
        toast.success(
          pageT(
            "messages.stillDisabled",
            "{{name}} follows platform policy and remains disabled",
            {
              name: updated.name,
            }
          )
        );
      }
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : pageT("errors.toggle", "Workspace agent policy update failed")
      );
    } finally {
      setBusy(`system:${row.agentCode}`, false);
    }
  }

  if (!activeWorkspaceCode) {
    return (
      <SurfaceCard>
        <h1 className="text-2xl font-semibold text-[var(--color-on-surface)]">
          {pageTitle}
        </h1>
        <p className="mt-3 text-sm text-[var(--color-on-surface-variant)]">
          {internalAgentsPage
            ? pageT(
                "emptyInternalWorkspace",
                "Select a workspace to manage internal agents."
              )
            : pageT(
                "emptyWorkspace",
                "Select a workspace to manage inherited system-agent policy."
              )}
        </p>
      </SurfaceCard>
    );
  }

  return (
    <div
      className={
        internalAgentsPage && scope === "platform" ? "space-y-4" : "space-y-5"
      }
    >
      {internalAgentsPage && scope === "platform" ? (
        <div className="flex flex-wrap items-center justify-between gap-3">
          <h2 className="text-base font-semibold">
            {t("pages:platformData.agentDefinitions")}
          </h2>
        </div>
      ) : (
        <PageHeader title={pageTitle} />
      )}

      {internalAgentsPage ? (
        <div className="space-y-5">
          <div className="flex flex-wrap items-center gap-3">
            <SearchField
              ariaLabel={internalPageT(
                "groups.search",
                "Search agents or groups"
              )}
              placeholder={internalPageT(
                "groups.search",
                "Search agents or groups"
              )}
              className="min-w-0 grow basis-full sm:basis-0 sm:max-w-[360px]"
              value={internalSearch}
              onChange={setInternalSearch}
            />
            <SelectInput
              aria-label={internalPageT("groups.label", "Group")}
              className="w-full sm:w-48"
              value={groupFilter}
              onValueChange={(value) => setGroupFilter(value || "all")}
            >
              <SelectItem value="all">
                {internalPageT("groups.all", "All groups")}
              </SelectItem>
              <SelectItem value="ungrouped">
                {internalPageT("groups.ungrouped", "Ungrouped")}
              </SelectItem>
              {[
                ...new Set([
                  ...catalogGroups,
                  ...internalAgents
                    .map((agent) =>
                      internalAgentGroupName(agent, (item) =>
                        translateInternalAgentName(t, item)
                      )
                    )
                    .filter(Boolean),
                ]),
              ]
                .sort()
                .map((name) => (
                  <SelectItem key={name} value={`group:${name}`}>
                    {translateInternalAgentGroupName(t, name)}
                  </SelectItem>
                ))}
            </SelectInput>
            {allowCreate ? (
              <div className="flex items-center gap-3 sm:ml-auto">
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => {
                    setGroupToDelete(null);
                    setGroupManagerOpen(true);
                  }}
                >
                  {internalPageT("groups.manage", "Manage groups")}
                </Button>
                  <CreateInternalAgentDialog
                    key={activeWorkspaceCode}
                    workspaceCode={activeWorkspaceCode}
                    allowWorkspaceAssignment={scope === "platform"}
                    onCreated={refreshCreatedAgent}
                  />
              </div>
            ) : null}
          </div>
          <SurfaceCard className="min-h-40 p-4">
          {groupInternalAgents(
            internalAgents,
            internalSearch,
            groupFilter,
            (agent) => translateInternalAgentName(t, agent)
          ).length === 0 ? (
            <p className="px-1 py-2 text-sm text-[var(--color-on-surface-variant)]">
              {internalPageT(
                "groups.empty",
                "No agents match the current filters."
              )}
            </p>
          ) : null}
          <div className="grid grid-cols-[repeat(auto-fill,minmax(112px,1fr))] gap-3 sm:grid-cols-[repeat(auto-fill,minmax(128px,1fr))]">
          {groupInternalAgents(
            internalAgents,
            internalSearch,
            groupFilter,
            (agent) => translateInternalAgentName(t, agent)
          ).map(({ name: groupName, agents }) => (
            <Fragment
              key={`${activeWorkspaceCode}:${groupName}`}
            >
                {agents.map((agent) => {
                  const draft =
                    internalDrafts[agent.agentCode] ?? draftFromRecord(agent);
                  const isSaving = isBusy(`internal-save:${agent.agentCode}`);
                  const editingDisabled =
                    (Boolean(agent.builtIn) && scope !== "platform") ||
                    isSaving ||
                    isBusy(`internal-reset:${agent.agentCode}`) ||
                    loading;
                  const resourceEditingDisabled =
                    (Boolean(agent.builtIn) && scope !== "platform") ||
                    isSaving ||
                    isBusy(`internal-reset:${agent.agentCode}`) ||
                    loading;
                  const internalBusy =
                    isSaving ||
                    isBusy(`internal-reset:${agent.agentCode}`) ||
                    loading;
                  const saveState =
                    internalSaveStates[agent.agentCode] ?? "idle";

                  let statusLabel = "";
                  let statusClassName =
                    "text-[var(--color-on-surface-variant)]";
                  if (saveState === "pending") {
                    statusLabel = internalPageT(
                      "status.pending",
                      "Unsaved changes"
                    );
                  } else if (saveState === "saving") {
                    statusLabel = internalPageT("status.saving", "Saving...");
                  } else if (saveState === "saved") {
                    statusLabel = internalPageT("status.saved", "Saved");
                  } else if (saveState === "error") {
                    statusLabel = internalPageT(
                      "status.error",
                      "Save failed. Please retry."
                    );
                    if (internalSaveErrors[agent.agentCode])
                      statusLabel += ` ${internalSaveErrors[agent.agentCode]}`;
                    statusClassName = "text-[var(--color-error)]";
                  }

                  return (
                    <Fragment key={agent.agentCode}>
                      <InternalAgentBrowseItem
                        agent={agent}
                        name={translateInternalAgentName(t, agent)}
                        groupLabel={groupName
                          ? translateInternalAgentGroupName(t, groupName)
                          : internalPageT("groups.ungrouped", "Ungrouped")}
                        disabled={
                          loading ||
                          isBusy(`internal-delete:${agent.agentCode}`)
                        }
                        canDelete={
                          !agent.builtIn &&
                          (scope === "platform" || !agent.platformDefined)
                        }
                        onDelete={() => setAgentToDelete(agent)}
                        onOpen={() => {
                          const nextDraft = draftFromRecord(agent);
                          internalDraftsRef.current = {
                            ...internalDraftsRef.current,
                            [agent.agentCode]: nextDraft,
                          };
                          setInternalDrafts((current) => ({
                            ...current,
                            [agent.agentCode]: nextDraft,
                          }));
                          setInternalSaveState(agent.agentCode, "idle");
                          setSkillDrafts({});
                          setKnowledgeDrafts({});
                          setWorkspaceDrafts({});
                          setRestoreBaselines({});
                          setEditingAgentTab("profile");
                          setEditingAgentCode(agent.agentCode);
                        }}
                      />
                      {editingAgentCode === agent.agentCode ? (
                        <Dialog
                          disablePointerDismissal
                          open
                          onOpenChange={(open, details) => {
                            if (
                              details.reason === "outside-press" ||
                              internalBusy
                            ) {
                              details.cancel();
                              return;
                            }
                            if (!open) {
                              setEditingAgentCode(null);
                              setAssignmentDialog(null);
                            }
                          }}
                        >
                          <DialogContent
                            className={INTERNAL_AGENT_DIALOG_CLASS}
                            showCloseButton={!internalBusy}
                            closeLabel={internalPageT(
                              "definition.close",
                              "Close"
                            )}
                          >
                            <DialogHeader>
                              <div className="flex items-center justify-between gap-4 pr-8"><DialogTitle>
                                {internalPageT("editAgent", "Edit {{name}}", {
                                  name: translateInternalAgentName(t, agent),
                                })}
                              </DialogTitle>
                              {scope === "platform" && <AgentActivationSwitch checked={draft.enabled} disabled={internalBusy} onChange={(enabled) => setInternalDraft(agent.agentCode, { enabled })} />}
                              </div>
                            </DialogHeader>
                            <div className="flex min-h-0 flex-1 flex-col px-1 pb-1">
                              {scope !== "platform" || statusLabel ? (
                                <div className="text-right text-xs">
                                  {scope !== "platform" ? (
                                    <p className="text-[var(--color-on-surface-variant)]">
                                      {!agent.platformDefined
                                        ? internalPageT(
                                            "inheritance.local",
                                            "Workspace definition"
                                          )
                                        : agent.source === "workspace_override"
                                        ? internalPageT(
                                            "inheritance.override",
                                            "Workspace override"
                                          )
                                        : internalPageT(
                                            "inheritance.inherited",
                                            "Inherited from platform"
                                          )}
                                    </p>
                                  ) : null}
                                  {scope !== "platform" &&
                                  !agent.builtIn &&
                                  agent.platformDefined &&
                                  agent.source === "workspace_override" ? (
                                    <Button
                                      variant="outline"
                                      size="sm"
                                      disabled={internalBusy}
                                      onClick={() =>
                                        void handleRestoreInheritance(
                                          agent.agentCode
                                        )
                                      }
                                    >
                                      {internalPageT(
                                        "inheritance.restore",
                                        "Restore inheritance"
                                      )}
                                    </Button>
                                  ) : null}
                                  <p
                                    className={`min-h-4 ${statusClassName}`}
                                    role="status"
                                    aria-live="polite"
                                    aria-atomic="true"
                                  >
                                    {statusLabel}
                                  </p>
                                </div>
                              ) : null}

                              <Tabs
                                className="min-h-0 flex-1"
                                value={editingAgentTab}
                                onValueChange={(value) => {
                                  if (typeof value === "string") setEditingAgentTab(value);
                                }}
                              >
                                <TabsList variant="line" className="w-full shrink-0 justify-start border-b border-[var(--color-outline-variant)]">
                                  {INTERNAL_AGENT_SECTIONS.map(([value, label]) => (
                                    <TabsTrigger key={value} value={value} className="flex-none px-3">
                                      {internalPageT(`definition.tabs.${value}`, label)}
                                    </TabsTrigger>
                                  ))}
                                  {scope === "platform" ? (
                                    <TabsTrigger value="workspace" className="flex-none px-3">
                                      {internalPageT("definition.tabs.workspace", "Workspace")}
                                    </TabsTrigger>
                                  ) : null}
                                </TabsList>
                                <TabsContent value="profile" keepMounted className="tron-scrollbar min-h-0 flex-1 space-y-3 overflow-y-auto px-1 py-3">
                                  <InternalAgentProfileFields code={agent.agentCode} draft={draft} disabled={editingDisabled} workspaceCode={activeWorkspaceCode} onChange={(change) => setInternalDraft(agent.agentCode, change)}
                                    avatar={agent.builtIn && scope !== "platform" ? <AgentAvatar avatarId={draft.avatarId} className="size-24 rounded-full" /> : undefined} />
                                </TabsContent>
                                <TabsContent value="runtime" keepMounted className="tron-scrollbar min-h-0 flex-1 space-y-3 overflow-y-auto px-1 py-3">
                                  <InternalAgentRuntimeFields workspaceCode={activeWorkspaceCode} candidateModelCodes={draft.candidateModelCodes} disabled={editingDisabled} onChange={(change) => setInternalDraft(agent.agentCode, change)} />
                                </TabsContent>
                                <TabsContent value="resources" keepMounted className="tron-scrollbar min-h-0 flex-1 space-y-3 overflow-y-auto px-1 py-3">
                                  <AgentPolicyFields tools={tools} nativeSchemes={nativeSchemes} panel="resources" draft={draft} disabled={resourceEditingDisabled} onChange={(change) => setInternalDraft(agent.agentCode, change)} onAssignment={(kind) => openAssignmentDialog(agent.agentCode, kind)}>
                                    <InternalAgentSkillField agentCode={agent.agentCode} disabled={resourceEditingDisabled} selectedNames={skillDrafts[agent.agentCode]} onSelectionChange={(names) => setSkillDrafts((current) => ({ ...current, [agent.agentCode]: names }))} />
                                    <InternalAgentKnowledgeField key={`${activeWorkspaceCode}:${agent.agentCode}:knowledge`} agentCode={agent.agentCode} agentName={translateInternalAgentName(t, { ...agent, name: draft.name })} disabled={editingDisabled} selectedIds={knowledgeDrafts[agent.agentCode]}
                                      onSelectionChange={!agent.builtIn || scope === "platform" ? (ids) => setKnowledgeDrafts((current) => ({ ...current, [agent.agentCode]: ids })) : undefined} />
                                  </AgentPolicyFields>
                                </TabsContent>
                                <TabsContent value="advanced" keepMounted className="tron-scrollbar min-h-0 flex-1 space-y-3 overflow-y-auto px-1 py-3">
                                  <AgentPolicyFields panel="advanced" nativeSchemes={nativeSchemes} draft={draft} disabled={editingDisabled} onChange={(change) => setInternalDraft(agent.agentCode, change)} />
                                </TabsContent>

                              {assignmentDialog?.workspaceCode ===
                                activeWorkspaceCode &&
                              assignmentDialog.agentCode === agent.agentCode ? (
                                <InternalAgentAssignmentDialog
                                  key={`${activeWorkspaceCode}:${agent.agentCode}:${assignmentDialog.kind}`}
                                  kind={assignmentDialog.kind}
                                  agentName={translateInternalAgentName(t, { ...agent, name: draft.name })}
                                  busy={editingDisabled}
                                  initialSelectedCodes={
                                    assignmentDialog.kind === "mcp"
                                      ? draft.selectedServerCodes
                                      : draft.toolAssignmentMode ===
                                        "selected_tools"
                                      ? draft.selectedToolCodes
                                      : []
                                  }
                                  servers={workspaceMcpServers}
                                  tools={tools}
                                  toolDomains={toolDomains}
                                  onClose={() => setAssignmentDialog(null)}
                                  onConfirm={(codes) => {
                                    setInternalDraft(
                                      agent.agentCode,
                                      assignmentDialog.kind === "mcp"
                                        ? {
                                            mcpAccessMode: "selected_servers",
                                            selectedServerCodes: codes,
                                          }
                                        : {
                                            toolAssignmentMode:
                                              "selected_tools",
                                            selectedToolCodes: codes,
                                          }
                                    );
                                    setAssignmentDialog(null);
                                  }}
                                />
                              ) : null}
                                {scope === "platform" ? (
                                  <TabsContent value="workspace" keepMounted className="tron-scrollbar min-h-0 flex-1 space-y-3 overflow-y-auto px-1 py-3">
                                    <AgentWorkspaceField
                                      code={agent.agentCode}
                                      name={draft.name}
                                      disabled={internalBusy}
                                      selectedCodes={workspaceDrafts[agent.agentCode]}
                                      onSelectionChange={(codes) =>
                                        setWorkspaceDrafts((current) => ({
                                          ...current,
                                          [agent.agentCode]: codes,
                                        }))
                                      }
                                    />
                                  </TabsContent>
                                ) : null}
                              </Tabs>
                            </div>
                            <DialogFooter>
                              <Button
                                variant="outline"
                                disabled={internalBusy}
                                onClick={() => {
                                  setEditingAgentCode(null);
                                  setAssignmentDialog(null);
                                }}
                              >
                                {internalPageT("definition.cancel", "Cancel")}
                              </Button>
                              {!agent.builtIn || scope === "platform" ? (
                                <Button
                                  disabled={
                                    internalBusy ||
                                    !draft.name.trim() ||
                                    (!hasDraftChanges(agent, draft) &&
                                      !skillDrafts[agent.agentCode] &&
                                      !knowledgeDrafts[agent.agentCode] &&
                                      !workspaceDrafts[agent.agentCode] &&
                                      !restoreBaselines[agent.agentCode])
                                  }
                                  onClick={() =>
                                    void handleSaveInternalAgent(
                                      agent.agentCode
                                    )
                                  }
                                >
                                  {isSaving
                                    ? internalPageT(
                                        "actions.saving",
                                        "Saving..."
                                      )
                                    : internalPageT("actions.save", "Save")}
                                </Button>
                              ) : null}
                            </DialogFooter>
                          </DialogContent>
                        </Dialog>
                      ) : null}
                    </Fragment>
                  );
                })}
            </Fragment>
          ))}
          </div>
          </SurfaceCard>
        </div>
      ) : null}

      {!internalAgentsPage ? (
        <DataTable<WorkspaceSystemAgentRecord>
          rows={systemPagination.visibleItems}
          rowKey={(row) => `${activeWorkspaceCode}-${row.agentCode}`}
          pagination={
            <TablePagination
              itemLabel={pageT("pagination.itemLabel", "workspace agents")}
              {...systemPagination.paginationProps}
            />
          }
          toolbar={
            <SearchField
              ariaLabel={systemTableT("searchAria", "Search workspace agents")}
              className="min-w-0 grow basis-full sm:basis-0 sm:max-w-[360px]"
              placeholder={systemTableT(
                "searchPlaceholder",
                "Search workspace agents..."
              )}
              value={systemSearch}
              onChange={setSystemSearch}
            />
          }
          emptyState={systemTableT(
            "empty",
            "No workspace agents match the current filters."
          )}
          columns={[
            {
              id: "agent",
              header: systemTableT("columns.agent", "Agent"),
              width: "22%",
              render: (row) => (
                <div>
                  <p className="font-medium text-[var(--color-on-surface)]">
                    {row.name}
                  </p>
                  <p className="mt-1 font-mono text-[12px] text-[var(--color-on-surface-variant)]">
                    {row.agentCode}
                  </p>
                </div>
              ),
            },
            {
              id: "description",
              header: systemTableT("columns.description", "Description"),
              width: "24%",
              render: (row) => (
                <p className="text-[13px] text-[var(--color-on-surface-variant)]">
                  {translateSystemAgentDescription(t, row)}
                </p>
              ),
            },
            {
              id: "source",
              header: systemTableT("columns.source", "Source"),
              width: "12%",
              render: (row) => translateSystemAgentSource(t, row.source),
            },
            {
              id: "inheritedFrom",
              header: systemTableT("columns.inheritedFrom", "Inherited From"),
              width: "14%",
              render: (row) => translateInheritedFrom(t, row.inheritedFrom),
            },
            {
              id: "status",
              header: systemTableT("columns.status", "Status"),
              width: "12%",
              render: (row) => <AgentStatusBadge status={row.status} />,
            },
            {
              id: "policy",
              header: systemTableT("columns.policy", "Policy"),
              width: "18%",
              render: (row) => resolveWorkspacePolicyLabel(row),
            },
            {
              id: "actions",
              header: systemTableT("columns.actions", "Actions"),
              align: "right",
              width: "16%",
              render: (row) => (
                <div className="flex justify-end">
                  <AgentPolicySwitch
                    checked={row.enabled}
                    disabled={
                      !canToggleWorkspaceAgent(row) ||
                      isBusy(`system:${row.agentCode}`) ||
                      loading
                    }
                    label={
                      !row.available
                        ? systemTableT(
                            "unavailableLabel",
                            "{{name}} unavailable",
                            { name: row.name }
                          )
                        : !row.platformEnabled && !row.workspaceOverrideDisabled
                        ? systemTableT(
                            "platformDisabledLabel",
                            "{{name}} disabled by platform policy",
                            { name: row.name }
                          )
                        : row.workspaceOverrideDisabled
                        ? systemTableT("enableLabel", "Enable {{name}}", {
                            name: row.name,
                          })
                        : systemTableT("disableLabel", "Disable {{name}}", {
                            name: row.name,
                          })
                    }
                    loading={isBusy(`system:${row.agentCode}`)}
                    onToggle={() => void handleToggleSystemAgent(row)}
                  />
                </div>
              ),
            },
          ]}
        />
      ) : null}
      <Dialog
        open={groupManagerOpen}
        onOpenChange={(open) => {
          if (!open && !groupToDelete) setGroupManagerOpen(false);
        }}
      >
        <DialogContent
          className="sm:max-w-lg"
          showCloseButton={!groupToDelete}
          closeLabel={internalPageT("definition.close", "Close")}
        >
          {groupToDelete ? (
            <>
              <DialogHeader>
                <DialogTitle>
                  {internalPageT("groups.confirmDeleteTitle", "Delete group?")}
                </DialogTitle>
                <DialogDescription>
                  {internalPageT(
                    "groups.confirmDeleteDescription",
                    "Agents in this group will become ungrouped."
                  )}
                </DialogDescription>
              </DialogHeader>
              <p className="text-sm font-medium">
                {translateInternalAgentGroupName(t, groupToDelete)}
              </p>
              <DialogFooter>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => setGroupToDelete(null)}
                >
                  {internalPageT("definition.cancel", "Cancel")}
                </Button>
                <Button
                  type="button"
                  variant="destructive"
                  onClick={() => void handleDeleteGroup(groupToDelete)}
                >
                  {internalPageT("groups.delete", "Delete group")}
                </Button>
              </DialogFooter>
            </>
          ) : (
            <>
              <DialogHeader>
                <DialogTitle>
                  {internalPageT("groups.manage", "Manage groups")}
                </DialogTitle>
                <DialogDescription>
                  {internalPageT(
                    "groups.manageDescription",
                    "Remove groups that are no longer needed."
                  )}
                </DialogDescription>
              </DialogHeader>
              <div className="max-h-[55dvh] space-y-2 overflow-y-auto">
                {catalogGroups.length ? (
                  catalogGroups.map((name) => (
                    <div
                      key={name}
                      className="flex items-center justify-between gap-3 rounded-lg border border-[var(--color-outline-variant)] px-3 py-2"
                    >
                      <span className="min-w-0 truncate text-sm">
                        {translateInternalAgentGroupName(t, name)}
                      </span>
                      <Button
                        type="button"
                        size="sm"
                        variant="destructive"
                        onClick={() => setGroupToDelete(name)}
                      >
                        {internalPageT("groups.delete", "Delete group")}
                      </Button>
                    </div>
                  ))
                ) : (
                  <p className="py-4 text-sm text-[var(--color-on-surface-variant)]">
                    {internalPageT("groups.noGroups", "No groups to manage")}
                  </p>
                )}
              </div>
              <DialogFooter>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => setGroupManagerOpen(false)}
                >
                  {internalPageT("definition.cancel", "Close")}
                </Button>
              </DialogFooter>
            </>
          )}
        </DialogContent>
      </Dialog>
      <Dialog
        open={Boolean(agentToDelete)}
        onOpenChange={(open) => {
          if (!open) setAgentToDelete(null);
        }}
      >
        <DialogContent
          className="sm:max-w-md"
          closeLabel={internalPageT("definition.close", "Close")}
        >
          <DialogHeader>
            <DialogTitle>
              {internalPageT(
                "groups.confirmDeleteAgentTitle",
                "Delete internal agent?"
              )}
            </DialogTitle>
            <DialogDescription>
              {internalPageT(
                "groups.confirmDeleteAgentDescription",
                "Its settings and workspace assignments will be removed."
              )}
            </DialogDescription>
          </DialogHeader>
          <p className="text-sm font-medium">
            {agentToDelete ? translateInternalAgentName(t, agentToDelete) : ""}
          </p>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              disabled={Boolean(
                agentToDelete &&
                  isBusy(`internal-delete:${agentToDelete.agentCode}`)
              )}
              onClick={() => setAgentToDelete(null)}
            >
              {internalPageT("definition.cancel", "Cancel")}
            </Button>
            <Button
              type="button"
              variant="destructive"
              disabled={Boolean(
                agentToDelete &&
                  isBusy(`internal-delete:${agentToDelete.agentCode}`)
              )}
              onClick={() => {
                if (agentToDelete) void handleDeleteAgent(agentToDelete);
              }}
            >
              {internalPageT("groups.deleteAgent", "Delete agent")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
