import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { TabsContent } from "@/components/ui/tabs";
import { fetchToolDomains, getNativeToolConfig, listTools, patchWorkspaceInternalAgentPolicy, patchWorkspaceInternalAgentMcpPolicy, updateWorkspaceInternalAgentToolAssignments, updateWorkspaceInternalAgentMcpServerAssignments } from "../api";
import { notifyInternalAgentRegistryChanged, PLATFORM_INTERNAL_AGENT_SCOPE, type WorkspaceInternalAgentRecord, type WorkspaceInternalAgentPolicyPayload, type PlatformToolRecord, type ToolCatalogDomainRecord } from "../model";
import { getServiceDataSource } from "@/shared/api/serviceConfig";
import * as agentMocks from "../mock";
import { fetchPlatformMcpServers, listPlatformMcpServersMock } from "@/domain/mcp";
import type { WorkspaceMcpServerRecord } from "@/domain/mcp/model";
import { setAgentSkills } from "@/domain/skill/api";
import { setAgentKnowledgeLibraries } from "@/domain/knowledge-base/api/knowledgeBaseApi";
import { InternalAgentSkillField } from "./InternalAgentSkillField";
import { InternalAgentKnowledgeField } from "./InternalAgentKnowledgeField";
import { InternalAgentAssignmentDialog, type InternalAgentAssignmentKind } from "./InternalAgentAssignmentDialog";
import { AgentPolicyFields, newAgentPolicyDraft, type AgentPolicyDraft } from "./AgentPolicyFields";
import { assistantToolsForRule, hasAssistantToolAlias } from "./assistantToolPolicy";

export interface AgentResourcesEditorHandle { validate(): void; save(createdCode?: string): Promise<void> }
const policyKeys = ["toolAssignmentMode", "maxSteps", "maxToolCalls", "maxParallelToolCalls"] as const;
const copy = (value: AgentPolicyDraft): AgentPolicyDraft => ({ ...value, selectedCategoryCodes: [...(value.selectedCategoryCodes ?? [])], selectedToolCodes: [...value.selectedToolCodes], selectedServerCodes: [...value.selectedServerCodes] });
const sameCodes = (a: string[], b: string[]) => a.length === b.length && a.every((code) => b.includes(code));

/** The same draft and controls serve creation and editing. Persistence starts only on parent Save. */
export const AgentResourcesEditor = forwardRef<AgentResourcesEditorHandle, {
  agentCode?: string; agentName: string; workspaceCode?: string; disabled?: boolean;
  audience?: "agent" | "assistant";
  loadRecord?: (code: string, signal: AbortSignal) => Promise<WorkspaceInternalAgentRecord>;
}>(function AgentResourcesEditor({ agentCode, agentName, workspaceCode = "$platform", disabled = false, loadRecord, audience = "agent" }, ref) {
  const { t } = useTranslation("pages");
  const client = useQueryClient();
  const [baseline, setBaseline] = useState<AgentPolicyDraft | null>(() => agentCode ? null : newAgentPolicyDraft());
  const [draft, setDraft] = useState<AgentPolicyDraft | null>(() => agentCode ? null : newAgentPolicyDraft());
  const [tools, setTools] = useState<PlatformToolRecord[]>([]);
  const [domains, setDomains] = useState<ToolCatalogDomainRecord[]>([]);
  const [servers, setServers] = useState<WorkspaceMcpServerRecord[]>([]);
  const [nativeSchemes, setNativeSchemes] = useState<import("../model").NativeToolSchemeRecord[]>([]);
  const [skillDraft, setSkillDraft] = useState<string[] | undefined>();
  const [knowledgeDraft, setKnowledgeDraft] = useState<string[] | undefined>();
  const [assignment, setAssignment] = useState<InternalAgentAssignmentKind | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [loading, setLoading] = useState(!!agentCode);
  const [catalogLoading, setCatalogLoading] = useState(true);
  const [recordError, setRecordError] = useState("");
  const [catalogError, setCatalogError] = useState("");
  const error = recordError || catalogError;
  const loadedCode = useRef<string | undefined>(undefined);
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  useEffect(() => {
    if (!agentCode || !loadRecord || loadedCode.current === agentCode) return;
    const controller = new AbortController();
    setLoading(true); setBaseline(null); setDraft(null);
    loadRecord(agentCode, controller.signal).then((record) => {
      if (controller.signal.aborted) return;
      setBaseline(copy(record)); setDraft(copy(record)); loadedCode.current = agentCode; setRecordError("");
    }).catch((reason: unknown) => { if (!controller.signal.aborted) setRecordError(String(reason)); })
      .finally(() => { if (!controller.signal.aborted) setLoading(false); });
    return () => controller.abort();
  }, [agentCode, loadRecord, attempt]);
  useEffect(() => {
    const controller = new AbortController();
    setCatalogLoading(true);
    Promise.all(getServiceDataSource("agent") === "api" ? [fetchToolDomains(controller.signal), listTools({}, controller.signal), fetchPlatformMcpServers({}, controller.signal), getNativeToolConfig(controller.signal)] as const : [Promise.resolve(agentMocks.listToolDomainsMock()), Promise.resolve(agentMocks.listToolsMock()), Promise.resolve(listPlatformMcpServersMock()), Promise.resolve({ schemes: [], defaultSchemeCode: "default", agentOverrides: {} })] as const)
      .then(([nextDomains, nextTools, nextServers, nextNativeConfig]) => {
        if (controller.signal.aborted) return;
        setDomains(nextDomains); setTools(nextTools);
        setServers(nextServers.map((server) => ({ ...server, platformEnabled: server.enabled, effectiveEnabled: server.enabled, source: "platform", workspaceOverrideDisabled: false })));
        setCatalogError("");
      }).catch((reason: unknown) => { if (!controller.signal.aborted) setCatalogError(String(reason)); })
      .finally(() => { if (!controller.signal.aborted) setCatalogLoading(false); });
    return () => controller.abort();
  }, [attempt]);
  const dirty = !!baseline && !!draft && (policyKeys.some((key) => baseline[key] !== draft[key]) || (baseline.nativeToolSchemeCode ?? "") !== (draft.nativeToolSchemeCode ?? "") || baseline.mcpAccessMode !== draft.mcpAccessMode || !sameCodes(baseline.selectedCategoryCodes ?? [], draft.selectedCategoryCodes ?? []) || !sameCodes(baseline.selectedToolCodes, draft.selectedToolCodes) || !sameCodes(baseline.selectedServerCodes, draft.selectedServerCodes) || skillDraft !== undefined || knowledgeDraft !== undefined);
  function validate() {
    if (!draft) { if (dirty) throw new Error(error || t("agentSkills.loading")); return; }
    for (const key of ["maxSteps", "maxToolCalls", "maxParallelToolCalls"] as const) {
      if (!Number.isSafeInteger(draft[key]) || draft[key] < (key === "maxParallelToolCalls" ? 1 : 0) || draft[key] > 4294967295) throw new Error(t(`workspaceAgents.internal.labels.${key}`));
    }
  }
  useImperativeHandle(ref, () => ({ validate, save: async (createdCode) => {
    if (!dirty) return;
    validate();
    const code = createdCode ?? agentCode;
    if (!code || !baseline || !draft || loading || savingRef.current) throw new Error(error || t("agentSkills.loading"));
    savingRef.current = true; setSaving(true);
    const api = getServiceDataSource("agent") === "api";
    let saved = baseline;
    try {
      if (draft.toolAssignmentMode === "selected_tools" && (saved.toolAssignmentMode !== "selected_tools" || !sameCodes(saved.selectedToolCodes, draft.selectedToolCodes))) {
        saved = copy(await (api ? updateWorkspaceInternalAgentToolAssignments(workspaceCode, code, draft.selectedToolCodes) : agentMocks.updateWorkspaceInternalAgentToolAssignmentsMock(workspaceCode, code, draft.selectedToolCodes))); setBaseline(saved);
      }
      const policy: WorkspaceInternalAgentPolicyPayload = {};
      if (!sameCodes(saved.selectedCategoryCodes ?? [], draft.selectedCategoryCodes ?? [])) policy.selectedCategoryCodes = draft.selectedCategoryCodes ?? [];
      for (const key of policyKeys) if (saved[key] !== draft[key]) Object.assign(policy, { [key]: draft[key] });
      if ((saved.nativeToolSchemeCode ?? "") !== (draft.nativeToolSchemeCode ?? "")) policy.nativeToolSchemeCode = draft.nativeToolSchemeCode ?? "";
      if (Object.keys(policy).length) { saved = copy(await (api ? patchWorkspaceInternalAgentPolicy(workspaceCode, code, policy) : agentMocks.patchWorkspaceInternalAgentPolicyMock(workspaceCode, code, policy))); setBaseline(saved); }
      if (draft.mcpAccessMode === "selected_servers" && (saved.mcpAccessMode !== "selected_servers" || !sameCodes(saved.selectedServerCodes, draft.selectedServerCodes))) {
        saved = copy(await (api ? updateWorkspaceInternalAgentMcpServerAssignments(workspaceCode, code, draft.selectedServerCodes) : agentMocks.updateWorkspaceInternalAgentMcpServerAssignmentsMock(workspaceCode, code, draft.selectedServerCodes))); setBaseline(saved);
      }
      if (saved.mcpAccessMode !== draft.mcpAccessMode) { saved = copy(await (api ? patchWorkspaceInternalAgentMcpPolicy(workspaceCode, code, { mcpAccessMode: draft.mcpAccessMode }) : agentMocks.patchWorkspaceInternalAgentMcpPolicyMock(workspaceCode, code, { mcpAccessMode: draft.mcpAccessMode }))); setBaseline(saved); }
      if (skillDraft !== undefined) { await setAgentSkills(code, skillDraft); setSkillDraft(undefined); await client.invalidateQueries({ queryKey: ["agent-skills", code] }); }
      if (knowledgeDraft !== undefined) { await setAgentKnowledgeLibraries(code, knowledgeDraft); setKnowledgeDraft(undefined); await client.invalidateQueries({ queryKey: ["knowledge-agent-libraries"] }); }
      setBaseline(copy(draft)); notifyInternalAgentRegistryChanged();
    } finally { savingRef.current = false; setSaving(false); }
  } }), [dirty, draft, baseline, agentCode, workspaceCode, loading, error, skillDraft, knowledgeDraft, client, t]);
  const blocked = disabled || saving || loading;
  const patch = (change: Partial<AgentPolicyDraft>) => setDraft((current) => current ? { ...current, ...change } : current);
  const assignmentTools = audience === "assistant" ? tools.map((tool) => tool.assistantAssignable === false
    ? { ...tool, assignable: false, disabledReason: t(`workspaceAgents.internal.assistantToolPolicy.${hasAssistantToolAlias(tool) ? "replacedByAssistant" : "workspaceOnly"}`) }
    : tool) : tools;
  const initialTools = !draft || draft.toolAssignmentMode === "selected_tools" || audience !== "assistant" ? draft?.selectedToolCodes ?? []
    : assistantToolsForRule(tools, draft.toolAssignmentMode, draft.selectedCategoryCodes ?? []).map((tool) => tool.toolCode);
  return <>
    <TabsContent value="resources" keepMounted className="tron-scrollbar min-h-0 flex-1 space-y-3 overflow-y-auto px-1 py-3">
      {error ? <div role="alert">{error}<Button type="button" variant="outline" disabled={disabled} onClick={() => setAttempt((value) => value + 1)}>{t("agentSkills.retry")}</Button></div> : null}
      {!draft ? <p role="status">{t("agentSkills.loading")}</p> : <AgentPolicyFields tools={tools} nativeSchemes={nativeSchemes} showNativeScheme={workspaceCode !== PLATFORM_INTERNAL_AGENT_SCOPE} audience={audience} panel="resources" draft={draft} disabled={blocked || catalogLoading || !!error} onChange={patch} onAssignment={setAssignment}>
        <InternalAgentSkillField agentCode={agentCode ?? ""} disabled={blocked} selectedNames={skillDraft} onSelectionChange={setSkillDraft} />
        <InternalAgentKnowledgeField agentCode={agentCode ?? ""} agentName={agentName} disabled={blocked} selectedIds={knowledgeDraft} onSelectionChange={setKnowledgeDraft} />
      </AgentPolicyFields>}
    </TabsContent>
    <TabsContent value="advanced" keepMounted className="tron-scrollbar min-h-0 flex-1 space-y-3 overflow-y-auto px-1 py-3">
      {!draft ? <p role="status">{t("agentSkills.loading")}</p> : <AgentPolicyFields panel="advanced" nativeSchemes={nativeSchemes} showNativeScheme={workspaceCode !== PLATFORM_INTERNAL_AGENT_SCOPE} draft={draft} disabled={blocked} onChange={patch} />}
    </TabsContent>
    {assignment && draft ? <InternalAgentAssignmentDialog kind={assignment} agentName={agentName} busy={blocked} initialSelectedCodes={assignment === "mcp" ? draft.selectedServerCodes : draft.toolAssignmentMode === "selected_tools" || audience === "assistant" ? initialTools : []} tools={assignmentTools} toolDomains={domains} servers={servers} allowRemovingUnavailable={audience === "assistant"} switchingFromRule={audience === "assistant" && assignment === "tools" && draft.toolAssignmentMode !== "selected_tools"} onClose={() => setAssignment(null)} onConfirm={(codes) => {
      patch(assignment === "mcp" ? { mcpAccessMode: "selected_servers", selectedServerCodes: codes } : { toolAssignmentMode: "selected_tools", selectedToolCodes: codes }); setAssignment(null);
    }} /> : null}
  </>;
});
