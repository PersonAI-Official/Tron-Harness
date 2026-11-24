import { ChatAttachmentButton } from "../components/ChatAttachmentButton";
import { contextBlockName, type ContextBlockUpdateMode } from "@/shared/context/contextBlockPresentation";
import { ContextBlockHelp } from "@/shared/context/ContextBlockHelp";
import { useContextBlockCatalog, type ContextBlockCatalog } from "@/shared/context/useContextBlockCatalog";
import {
  FileArchive,
  Bot,
  Copy,
  CheckCircle2,
  CircleAlert,
  CircleDot,
  LoaderCircle,
  Pause,
  Check,
  ChevronDown,
} from "lucide-react";
import { ShinyText } from "@/shared/motion";
import { createTronMockChatAdapter } from "@/domain/chat/mock/adapter";
import { ChatSubagentsPanel } from "../components/ChatSubagentsPanel";
import { useChatSubagents } from "../hooks/useChatSubagents";
import { useTabPanelScopeId } from "@/shared/layout/content/TabPanelRegistry";
import { useTabVisibility } from "@/components/editable-tabs";
import { AgentTerminalSessionContext } from "../terminal/AgentTerminalToolCard";
import { SessionApplicationsPanel } from "@/domain/chat/components/SessionApplicationsPanel";
import { useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent } from "react";
import { useLocation, useNavigate, useSearchParams } from "react-router";

import { Badge } from "@/components/ui/badge";
import { InfoTooltip } from "@/components/info-tooltip";
import { useWorkspaceSpaces, useTabPanelConfig, useOpenRightSidebar, useTheme, type TargetPageFrameInspectionResult, useRightPanelTabs } from "@/shared";

import { useToast } from "@/components/ui/toast";
import { MarkdownLink } from "@/components/markdown-link";
import { DesktopBridge } from "@/shared/desktop/DesktopBridge";
import { cn } from "@/shared/lib/utils";
import { CHAT_SESSIONS_REMOVED_EVENT } from "@/shared/lib/shellEvents";
import {
  createChatSession,
  useChatAguiEvents,
  useChatAguiProtocolEvents,
  cancelChatSessionGoal,
  clearChatSessionGoal,
  pauseChatSessionGoal,
  resumeChatSessionGoal,
  setChatSessionGoal,
  updateChatSessionGoal,
  useChatLatestCompactionState,
  useChatMessageListBySession,
  useChatSessionLatestGoalState,
  useChatSessionTodosState,
  useChatSessionGroupList,
  useChatGeneratedTitle,
  useChatWorkRailLabels,
  useChatHistoryLabels,
} from "@/domain/chat/hooks";
import { useAuthSession } from "@/domain/auth";
import { useTranslation } from "react-i18next";
import { useTabStore } from "@/store/tabStore";
import { INTERNAL_AGENT_REGISTRY_CHANGED_EVENT } from "@/domain/agent/model";
import { fetchWorkspaceInternalAgents, fetchWorkspaceSystemAgents } from "@/domain/agent/api";
import { listWorkspaceInternalAgentsMock, listWorkspaceSystemAgentsMock } from "@/domain/agent/mock";
import type { SystemAgentModelRecord, WorkspaceInternalAgentRecord, WorkspaceSystemAgentRecord } from "@/domain/agent/model";
import { fetchModelSelectOptions } from "@/domain/model/api";
import { autoModelRoutes } from "@/domain/model/constants/autoModelRoutes";
import { applyCurrentLocaleHeader } from "@/i18n";
import type {
  ModelSelectOption,
  ModelSelection,
  ReasoningEffort,
} from "@/domain/model/model";
import { reasoningEffortOptions } from "@/domain/model/constants/modelPageOptions";
import { getApiAccessToken, requestApi } from "@/shared/api/httpClient";
import { chatApiEndpoints } from "@/domain/chat/api";
import { getServiceDataSource } from "@/shared/api/serviceConfig";
import { useAguiWorkReplayPreferences } from "@/domain/settings/model/aguiWorkReplayPreferences";
import { AgentChatWindow, type AgentChatSubmitter } from "@/domain/chat/components/AgentChatWindow";
import { ChatWorkingDirectoryControl } from "@/domain/chat/components/ChatWorkingDirectoryControl";
import { ChatDraftResources } from "../model/chatDraftResources";
import { ChatExecutionModeSelector } from "../components/ChatExecutionModeSelector";
import { ChatPlanReviewCard } from "../components/ChatPlanReviewCard";
import { ChatModelEffortSelector } from "../components/ChatModelEffortSelector";
import { useChatExecutionMode } from "../hooks/useChatExecutionMode";
import type { TaskExecutionResult } from "@/domain/task/model";
import { useCreateChatTab } from "@/shared/layout/chat/hooks/useCreateChatTab";
import { ContextDetailPreview, contextDetailRawValue, type ContextDetailBlock } from "@/domain/chat/components/ContextDetailPreview";
import { useSavedContextAssembly } from "../hooks/useSavedContextAssembly";
import { useContextModelLabel } from "../hooks/useContextModelLabel";
import { useChatContextRefreshInterval } from "../hooks/useChatContextRefreshInterval";
import { contextAssemblySnapshot, type ContextAssemblySnapshot } from "../model/contextAssemblyReport";
import { contextBudgetBorrowing } from "../model/contextBudgetBorrowing";
import { createInternalChatAdapter } from "@tron/chat-internal-adapter";
import {
  type ChatAttachmentLabels,
  type ChatUserInputLabels,
  type ChatRuntimeRunState,
  type ChatSession as RuntimeChatSession,
  type ToolExecutionMode,
} from "@tron/chat-core";
import { contextBlockPlacements, truncateLabel, type ContextBlockPlacement } from "@personai/agent-ui-chat-core";
import { inferChatAgentCodeFromModel } from "../model/chatAgentCode";
import {
  canClearGoalStatus,
  canStartNewGoalFromStatus,
  isEditableGoalStatus,
} from "../model/goalStatusState";
import type {
  ChatAguiEvent,
  ChatAguiStreamEvent,
  ChatMessage,
  ChatSessionCompaction,
  ChatSessionGoalView,
  ChatSessionGoalStatus,
  ChatSessionGoalCommand,
  ChatSession,
  ChatSessionGroup,
  ChatSessionTodo,
  ChatSessionTodoPriority,
  ChatSessionTodoStatus,
  ChatMessageAttachment,
  ChatContextBudgetBreakdown,
  ChatTokenEstimateSnapshot,
  ChatTokenUsage,
  ChatUsageAuditSnapshot,
  SendChatMessageInput,
  SetChatSessionGoalInput,
  UpdateChatSessionGoalInput,
} from "@/domain/chat/model";
import { type EditableTab } from "@/components/editable-tabs";
import { useInspection } from "../hooks/useInspection";
import {
  downloadChatSessionAttachment,
  uploadChatSessionAttachment,
} from "../api/chatAttachmentApi";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";


function useStableCallback<TArgs extends unknown[], TResult>(
  callback: (...args: TArgs) => TResult,
): (...args: TArgs) => TResult {
  const callbackRef = useRef(callback);
  callbackRef.current = callback;
  return useCallback((...args: TArgs) => callbackRef.current(...args), []);
}

const chatSessionModelSelectionStorageKey = "tron.chat.session.model.selection";

function sessionModelSelectionKey(sessionCode: string, agentCode: string) {
  return `${sessionCode}::${agentCode.trim().toLowerCase() || "main-agent"}`;
}

function readChatSessionModelSelections() {
  if (typeof window === "undefined") return {} as Record<string, string>;
  try {
    const raw = window.localStorage.getItem(chatSessionModelSelectionStorageKey);
    if (!raw) return {} as Record<string, string>;
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return {} as Record<string, string>;
    const selections: Record<string, string> = {};
    Object.entries(parsed).forEach(([sessionCode, optionCode]) => {
      if (typeof optionCode !== "string" || !optionCode.trim()) return;
      selections[sessionCode.includes("::") ? sessionCode : `${sessionCode}::main-agent`] = optionCode;
    });
    return selections;
  } catch {
    return {} as Record<string, string>;
  }
}

function writeChatSessionModelSelections(selections: Record<string, string>) {
  if (typeof window === "undefined") return;
  try {
    if (!Object.keys(selections).length) {
      window.localStorage.removeItem(chatSessionModelSelectionStorageKey);
      return;
    }
    window.localStorage.setItem(chatSessionModelSelectionStorageKey, JSON.stringify(selections));
  } catch {
    // Ignore local persistence failures and keep in-memory behavior.
  }
}

function shouldAutoFocusChatComposer() {
  if (typeof window === "undefined") return false;
  if (typeof window.matchMedia !== "function") {
    return window.innerWidth >= 1024;
  }
  return window.matchMedia("(min-width: 1024px)").matches
    || window.matchMedia("(hover: hover) and (pointer: fine)").matches;
}




type ChatSessionSourceTab = "chat" | "automation" | "task";

const defaultChatSessionSourceTab: ChatSessionSourceTab = "chat";
const chatSessionSourceTabStorageKey = "tron.chat.activeSourceTab";

function normalizedSessionSource(value: string | null | undefined) {
  return value?.trim().toLowerCase() ?? "";
}

function parseChatSessionSourceTab(value: string | null | undefined): ChatSessionSourceTab | undefined {
  const source = normalizedSessionSource(value);
  if (source === "chat" || source === "automation" || source === "task") return source;
  return undefined;
}

function writeStoredChatSessionSourceTab(sourceTab: ChatSessionSourceTab) {
  if (typeof window === "undefined") return;
  window.sessionStorage.setItem(chatSessionSourceTabStorageKey, sourceTab);
}

function readInspectionString(dom: Record<string, unknown>, key: string) {
  const value = dom[key];
  return typeof value === "string" ? value : "";
}

function inspectionSubmitMode(result: TargetPageFrameInspectionResult) {
  return readInspectionString(result.dom, "submitMode") === "compose" ? "compose" : "send";
}

function inspectionMessageContent(result: TargetPageFrameInspectionResult) {
  return result.remark.trim() || readInspectionString(result.dom, "remark").trim();
}

function sourceTabFromValues(
  sourceType: string | null | undefined,
  sourceCode: string | null | undefined,
  taskCode: string | null | undefined,
  groupTitle?: string,
): ChatSessionSourceTab {
  const source = normalizedSessionSource(sourceType);
  if (source === "task") return "task";
  if (source === "automation") return "automation";
  if (taskCode?.trim()) return "task";

  const code = normalizedSessionSource(sourceCode);
  const title = normalizedSessionSource(groupTitle);
  if (code.startsWith("automation") || title === "automation" || title.startsWith("automation:")) return "automation";
  return "chat";
}

function sourceTabForGroup(group: ChatSessionGroup): ChatSessionSourceTab {
  const firstSession = group.sessions[0];
  return sourceTabFromValues(
    group.sourceType ?? firstSession?.sourceType,
    group.sourceCode ?? firstSession?.sourceCode,
    group.taskCode ?? firstSession?.taskCode,
    group.taskTitle,
  );
}

function sourceTabForSessionCode(groups: ChatSessionGroup[], sessionCode: string): ChatSessionSourceTab | undefined {
  const group = groups.find((item) => item.sessions.some((session) => session.sessionCode === sessionCode));
  return group ? sourceTabForGroup(group) : undefined;
}

async function copyPlainText(value: string) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(value);
    return;
  }

  const textarea = document.createElement("textarea");
  textarea.value = value;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand("copy");
  document.body.removeChild(textarea);
  if (!copied) throw new Error("Clipboard copy failed");
}

function firstSessionForSourceTab(groups: ChatSessionGroup[], sourceTab: ChatSessionSourceTab) {
  return groups
    .filter((group) => sourceTabForGroup(group) === sourceTab)
    .flatMap((group) => group.sessions)[0];
}

function findSession(groups: ChatSessionGroup[], sessionCode: string) {
  return groups
    .flatMap((group) => group.sessions)
    .find((session) => session.sessionCode === sessionCode);
}

function orderSessionGroups(groups: ChatSessionGroup[]) {
  return [...groups].sort((left, right) => {
    if ((left.pendingUserInputCount ?? 0) !== (right.pendingUserInputCount ?? 0)) {
      return (right.pendingUserInputCount ?? 0) - (left.pendingUserInputCount ?? 0);
    }
    if (!left.taskCode && right.taskCode) return -1;
    if (left.taskCode && !right.taskCode) return 1;
    return left.taskTitle.localeCompare(right.taskTitle);
  });
}

function sessionGroupKey(group: ChatSessionGroup) {
  return [
    group.sourceType ?? "Manual",
    group.sourceCode ?? "",
    group.taskCode ?? "",
    group.taskTitle,
  ].join(":");
}

function sessionGroupKeyForSession(session: ChatSession) {
  // 提取不含查询参数的 sessionCode
  const cleanSessionCode = session.sessionCode.split("?")[0];
  return [
    session.sourceType ?? "Manual",
    session.sourceCode ?? "",
    session.taskCode ?? "",
    session.taskCode ? session.taskCode : "Chat",
    cleanSessionCode,
  ].join(":");
}

function insertSession(groups: ChatSessionGroup[], session: ChatSession): ChatSessionGroup[] {
  const groupKey = sessionGroupKeyForSession(session);
  let inserted = false;
  const nextGroups = groups.map((group) => {
    const currentKey = sessionGroupKey(group);
    if (currentKey !== groupKey) return group;
    inserted = true;
    return {
      ...group,
      sessions: [session, ...group.sessions.filter((item) => item.sessionCode !== session.sessionCode)],
    };
  });

  if (inserted) return orderSessionGroups(nextGroups);

  return orderSessionGroups([
    {
      taskCode: session.taskCode,
      sourceCode: session.sourceCode,
      sourceType: session.sourceType,
      taskTitle: session.taskCode ?? "Chat",
      sessions: [session],
    },
    ...nextGroups,
  ]);
}

function replaceSession(groups: ChatSessionGroup[], session: ChatSession): ChatSessionGroup[] {
  return orderSessionGroups(groups.map((group) => ({
    ...group,
    sessions: group.sessions.map((item) => (
      item.sessionCode === session.sessionCode ? session : item
    )),
  })));
}



type TranscriptAguiEvent = ChatAguiEvent | ChatAguiStreamEvent;

function aguiEventMetadata(event: TranscriptAguiEvent) {
  if (!("metadata" in event) || typeof event.metadata !== "object" || event.metadata === null) {
    return null;
  }
  return event.metadata as Record<string, string>;
}

function latestEventValue(events: TranscriptAguiEvent[], key: string) {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const metadata = aguiEventMetadata(events[index]);
    if (metadata?.[key]) return metadata[key];
  }
  return null;
}

function contextValue(value: string | null | undefined) {
  const trimmed = value?.trim();
  return trimmed && trimmed !== "unavailable" ? trimmed : null;
}


function tokenField(value: number | null | undefined) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function tokenFieldFromMetadata(value: string | null | undefined) {
  if (!value) return null;
  const normalized = value.replaceAll(",", "").trim();
  if (!normalized) return null;
  const parsed = Number.parseInt(normalized, 10);
  return Number.isFinite(parsed) ? parsed : null;
}

function normalizeUsageEstimate(parsed: Partial<ChatTokenEstimateSnapshot> | null | undefined): ChatTokenEstimateSnapshot | null {
  if (typeof parsed?.estimatedInputTokens !== "number") return null;
  if (typeof parsed.messageTokenEstimate !== "number") return null;
  if (typeof parsed.toolSchemaTokenEstimate !== "number") return null;
  return {
    checkpointCode: parsed.checkpointCode ?? null,
    contextWindowLimit: tokenField(parsed.contextWindowLimit),
    effectiveInputBudget: tokenField(parsed.effectiveInputBudget),
    estimator: parsed.estimator ?? "unicode_estimate",
    estimatedInputTokens: parsed.estimatedInputTokens,
    messageTokenEstimate: parsed.messageTokenEstimate,
    modelCode: parsed.modelCode ?? "",
    modelName: parsed.modelName ?? "",
    modelProtocol: parsed.modelProtocol ?? "",
    outputTokenReserve: tokenField(parsed.outputTokenReserve),
    overBudget: Boolean(parsed.overBudget),
    compactionTriggerReached: Boolean(parsed.compactionTriggerReached),
    rebased: Boolean(parsed.rebased),
    toolSchemaTokenEstimate: parsed.toolSchemaTokenEstimate,
    usageConfidence: parsed.usageConfidence ?? "estimated",
    usageSource: parsed.usageSource ?? "local_estimate",
  };
}

export function normalizeContextBudgetBreakdown(parsed: Partial<ChatContextBudgetBreakdown> | null | undefined): ChatContextBudgetBreakdown | null {
  if (typeof parsed?.estimatedInputTokens !== "number") return null;
  return {
    checkpointBudgetTokens: tokenField(parsed.checkpointBudgetTokens),
    checkpointTokens: tokenField(parsed.checkpointTokens) ?? 0,
    compressedHistoryMessageCount: tokenField(parsed.compressedHistoryMessageCount),
    compressedHistoryTokens: tokenField(parsed.compressedHistoryTokens) ?? 0,
    compactionTriggerRatioBps: tokenField(parsed.compactionTriggerRatioBps),
    compactionTriggerReached: Boolean(parsed.compactionTriggerReached),
    compactionTriggerTokens: tokenField(parsed.compactionTriggerTokens),
    contextWindowLimit: tokenField(parsed.contextWindowLimit),
    currentSessionHistoryBudgetTokens: tokenField(parsed.currentSessionHistoryBudgetTokens),
    effectiveInputBudget: tokenField(parsed.effectiveInputBudget),
    effectiveRetainedHistoryTokens: tokenField(parsed.effectiveRetainedHistoryTokens),
    estimatedInputTokens: parsed.estimatedInputTokens,
    historySourceMessageCount: tokenField(parsed.historySourceMessageCount),
    inputBudgetExceeded: Boolean(parsed.inputBudgetExceeded),
    outputTokenReserve: tokenField(parsed.outputTokenReserve),
    applicationContextTokens: tokenField(parsed.applicationContextTokens) ?? 0,
    applicationContextMessageCount: tokenField(parsed.applicationContextMessageCount) ?? 0,
    promptPrefixMessageCount: tokenField(parsed.promptPrefixMessageCount),
    promptPrefixBudgetTokens: tokenField(parsed.promptPrefixBudgetTokens),
    promptPrefixMaxRatioBps: tokenField(parsed.promptPrefixMaxRatioBps),
    promptPrefixMaxTokens: tokenField(parsed.promptPrefixMaxTokens),
    promptPrefixTokens: tokenField(parsed.promptPrefixTokens) ?? 0,
    rawTailMessageCount: tokenField(parsed.rawTailMessageCount),
    rawTailStartSeq: tokenField(parsed.rawTailStartSeq),
    rawTailTranscriptTokens: tokenField(parsed.rawTailTranscriptTokens) ?? 0,
    recentTranscriptTokens: tokenField(parsed.recentTranscriptTokens) ?? 0,
    retainedHistoryRatioBps: tokenField(parsed.retainedHistoryRatioBps),
    toolSchemaBudgetTokens: tokenField(parsed.toolSchemaBudgetTokens),
    toolSchemaTokens: tokenField(parsed.toolSchemaTokens) ?? 0,
    workingStateBudgetTokens: tokenField(parsed.workingStateBudgetTokens),
    workingStateTokens: tokenField(parsed.workingStateTokens) ?? 0,
  };
}

function normalizeTokenUsage(parsed: Partial<ChatTokenUsage> | null | undefined): ChatTokenUsage | null {
  if (typeof parsed?.inputTokens !== "number") return null;
  if (typeof parsed.outputTokens !== "number") return null;
  if (typeof parsed.totalTokens !== "number") return null;
  return {
    cachedInputTokens: tokenField(parsed.cachedInputTokens),
    inputTokens: parsed.inputTokens,
    outputTokens: parsed.outputTokens,
    reasoningTokens: tokenField(parsed.reasoningTokens),
    totalTokens: parsed.totalTokens,
  };
}

function parsedUsageEstimate(value: string | null | undefined): ChatTokenEstimateSnapshot | null {
  if (!value) return null;
  try {
    return normalizeUsageEstimate(JSON.parse(value) as Partial<ChatTokenEstimateSnapshot>);
  } catch {
    return null;
  }
}

function parsedContextBudgetBreakdown(value: string | null | undefined): ChatContextBudgetBreakdown | null {
  if (!value) return null;
  try {
    return normalizeContextBudgetBreakdown(JSON.parse(value) as Partial<ChatContextBudgetBreakdown>);
  } catch {
    return null;
  }
}

function parsedUsageAudit(value: string | null | undefined): ChatUsageAuditSnapshot | null {
  if (!value) return null;
  try {
    const parsed = JSON.parse(value) as Partial<ChatUsageAuditSnapshot>;
    const requestEstimate = normalizeUsageEstimate(parsed.requestEstimate);
    if (!requestEstimate) return null;
    return {
      cumulativeRunUsage: normalizeTokenUsage(parsed.cumulativeRunUsage),
      inputTokenDelta: typeof parsed.inputTokenDelta === "number" ? parsed.inputTokenDelta : null,
      inputTokenRatio: typeof parsed.inputTokenRatio === "number" ? parsed.inputTokenRatio : null,
      providerUsage: normalizeTokenUsage(parsed.providerUsage),
      requestEstimate,
      responseId: parsed.responseId ?? null,
      usageConfidence: parsed.usageConfidence ?? "unavailable",
      usageSource: parsed.usageSource ?? "unavailable",
    };
  } catch {
    return null;
  }
}

function latestUsageEstimateFromEvents(events: TranscriptAguiEvent[]) {
  return parsedUsageEstimate(latestEventValue(events, "usage_estimate_json"))
    ?? parsedUsageEstimate(latestEventValue(events, "usageEstimate"));
}

function latestUsageAuditFromEvents(events: TranscriptAguiEvent[]) {
  return parsedUsageAudit(latestEventValue(events, "usage_audit_json"));
}

function sessionProviderUsageFromEvents(events: TranscriptAguiEvent[]): ChatTokenUsage | null {
  const seen = new Set<string>();
  const totals = { inputTokens: 0, outputTokens: 0, totalTokens: 0 };
  let hasUsage = false;
  let cachedInputTokens = 0;
  let hasCachedInput = false;
  let reasoningTokens = 0;
  let hasReasoning = false;

  events.forEach((event) => {
    const metadata = aguiEventMetadata(event);
    const audit = parsedUsageAudit(metadata?.usage_audit_json);
    const usage = audit?.providerUsage;
    if (!usage) return;

    const eventId = "id" in event && typeof event.id === "string" ? event.id : "";
    const key = eventId || [
      aguiEventRunCode(event),
      aguiEventTime(event),
      audit.responseId ?? "",
      usage.inputTokens,
      usage.outputTokens,
      usage.totalTokens,
    ].join("\u0000");
    if (seen.has(key)) return;
    seen.add(key);

    hasUsage = true;
    totals.inputTokens += usage.inputTokens;
    totals.outputTokens += usage.outputTokens;
    totals.totalTokens += usage.totalTokens;
    if (usage.cachedInputTokens != null) {
      hasCachedInput = true;
      cachedInputTokens += usage.cachedInputTokens;
    }
    if (usage.reasoningTokens != null) {
      hasReasoning = true;
      reasoningTokens += usage.reasoningTokens;
    }
  });

  if (!hasUsage) return null;
  return {
    ...totals,
    cachedInputTokens: hasCachedInput ? cachedInputTokens : null,
    reasoningTokens: hasReasoning ? reasoningTokens : null,
  };
}

const contextEventPayloadCache = new WeakMap<TranscriptAguiEvent, Record<string, unknown> | null>();

function shortTermMemoryEventPayload(event: TranscriptAguiEvent): Record<string, unknown> | null {
  if (contextEventPayloadCache.has(event)) return contextEventPayloadCache.get(event) ?? null;
  if ("value" in event && typeof event.value === "object" && event.value !== null) {
    return event.value as Record<string, unknown>;
  }
  const encoded = aguiEventMetadata(event)?.context_event_json;
  if (!encoded) return null;
  try {
    const parsed = JSON.parse(encoded) as unknown;
    const payload = typeof parsed === "object" && parsed !== null
      ? parsed as Record<string, unknown>
      : null;
    contextEventPayloadCache.set(event, payload);
    return payload;
  } catch {
    return null;
  }
}

export function latestContextBudgetBreakdownFromEvents(events: TranscriptAguiEvent[]) {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const payload = shortTermMemoryEventPayload(events[index]);
    const breakdown = payload?.budgetBreakdown;
    if (typeof breakdown === "object" && breakdown !== null) {
      const normalized = normalizeContextBudgetBreakdown(
        breakdown as Partial<ChatContextBudgetBreakdown>,
      );
      if (normalized) return normalized;
    }
    const normalized = parsedContextBudgetBreakdown(
      aguiEventMetadata(events[index])?.budgetBreakdown,
    );
    if (normalized) return normalized;
  }
  return null;
}

function latestContextAssemblyFromEvents(events: TranscriptAguiEvent[]) {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index];
    const payload = shortTermMemoryEventPayload(event);
    const snapshot = contextAssemblySnapshot(payload);
    if (snapshot) return {
      snapshot,
      breakdown: normalizeContextBudgetBreakdown(payload?.budgetBreakdown as Partial<ChatContextBudgetBreakdown> | undefined),
    };
    // A following model-requested event also carries budgetBreakdown; it must
    // not hide the full report emitted immediately before that request.
    if (payload?.budgetBreakdown && "name" in event
      && event.name === "tron.short_term_memory.context.assembled") break;
  }
  return { snapshot: null, breakdown: latestContextBudgetBreakdownFromEvents(events) };
}

function chatTokenUsage(messages: ChatMessage[]) {
  const assistantMessages = messages.filter((message) => message.role === "ASSISTANT");
  let hasUsage = false;
  let input = 0;
  let output = 0;
  let total = 0;

  assistantMessages.forEach((message) => {
    const inputTokens = tokenField(message.inputTokens);
    const outputTokens = tokenField(message.outputTokens);
    const totalTokens = tokenField(message.totalTokens);
    if (inputTokens == null && outputTokens == null && totalTokens == null) return;

    hasUsage = true;
    input += inputTokens ?? 0;
    output += outputTokens ?? 0;
    total += totalTokens ?? (inputTokens ?? 0) + (outputTokens ?? 0);
  });

  return {
    input: hasUsage ? input : null,
    output: hasUsage ? output : null,
    total: hasUsage ? total : null,
  };
}


function formatCompactTokenCount(value: number | null | undefined) {
  if (value == null) return "-";
  const absolute = Math.abs(value);
  if (absolute >= 1_000_000) {
    return `${new Intl.NumberFormat("en-US", { maximumFractionDigits: 1 }).format(value / 1_000_000)}m`;
  }
  if (absolute >= 1_000) {
    return `${new Intl.NumberFormat("en-US", { maximumFractionDigits: absolute < 10_000 ? 1 : 0 }).format(value / 1_000)}k`;
  }
  return new Intl.NumberFormat("en-US").format(value);
}

function formatTokenLimit(usedTokens: number | null | undefined, limitTokens: number | null | undefined) {
  const used = formatCompactTokenCount(usedTokens);
  const limit = limitTokens == null ? "-" : formatCompactTokenCount(limitTokens);
  return `${used} / ${limit}`;
}

function contextPercent(usedTokens: number | null | undefined, limitTokens: number | null | undefined) {
  if (usedTokens == null || limitTokens == null || limitTokens <= 0) return 0;
  return Math.min(100, Math.max(0, (usedTokens / limitTokens) * 100));
}

function formatPercent(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value)) return "-";
  return `${new Intl.NumberFormat("en-US", { maximumFractionDigits: value < 10 ? 1 : 0 }).format(value)}%`;
}


function contextStatusTone(estimate: ChatTokenEstimateSnapshot | null, breakdown: ChatContextBudgetBreakdown | null): BadgeToneName {
  if (estimate?.overBudget || breakdown?.inputBudgetExceeded) return "error";
  if (estimate?.compactionTriggerReached || breakdown?.compactionTriggerReached) return "warning";
  return "success";
}

function contextStatusLabel(estimate: ChatTokenEstimateSnapshot | null, breakdown: ChatContextBudgetBreakdown | null, translate: (key: string) => string) {
  if (estimate?.overBudget || breakdown?.inputBudgetExceeded) return translate("chat.shortTermMemory.contextPanel.overBudget");
  if (estimate?.compactionTriggerReached || breakdown?.compactionTriggerReached) return translate("chat.shortTermMemory.contextPanel.nearCompaction");
  return null;
}

function contextWindowLimitFromSources(
  model: ChatModelOption | null | undefined,
  selectedModel: ChatModelOption | null | undefined,
  events: TranscriptAguiEvent[],
) {
  const metadataWindow = tokenFieldFromMetadata(
    contextValue(latestEventValue(events, "contextWindowTokens"))
    ?? contextValue(latestEventValue(events, "context_window_tokens"))
    ?? contextValue(latestEventValue(events, "contextWindowLimit"))
    ?? contextValue(latestEventValue(events, "context_window_limit")),
  );
  return model?.contextWindowTokens ?? selectedModel?.contextWindowTokens ?? metadataWindow ?? null;
}

function aguiEventType(event: TranscriptAguiEvent) {
  if (typeof event.type === "string") return event.type;
  if ("eventName" in event && typeof event.eventName === "string") return event.eventName;
  return "";
}

function aguiEventTime(event: TranscriptAguiEvent) {
  if ("occurredAt" in event && typeof event.occurredAt === "string") return event.occurredAt;
  if ("timestamp" in event && typeof event.timestamp === "number") return new Date(event.timestamp).toISOString();
  return new Date().toISOString();
}

function aguiEventRunCode(event: TranscriptAguiEvent) {
  if ("runId" in event && typeof event.runId === "string") return event.runId;
  if ("runCode" in event && typeof event.runCode === "string") return event.runCode;
  return "stream";
}


function aguiEventSummary(event: TranscriptAguiEvent) {
  if ("delta" in event && typeof event.delta === "string" && event.delta.trim()) return event.delta;
  if ("content" in event && typeof event.content === "string" && event.content.trim()) return event.content;
  if ("message" in event && typeof event.message === "string" && event.message.trim()) return event.message;
  if ("toolCallName" in event && typeof event.toolCallName === "string") return event.toolCallName;
  if ("metadata" in event && event.metadata && typeof event.metadata === "object") {
    const metadata = event.metadata as Record<string, string>;
    return metadata.summary ?? metadata.skillName ?? ("outcome" in event && typeof event.outcome === "string" ? event.outcome : "event received");
  }
  return "event received";
}

function aguiEventMillis(event: TranscriptAguiEvent) {
  if ("timestamp" in event && typeof event.timestamp === "number") return event.timestamp;
  const parsed = Date.parse(aguiEventTime(event));
  return Number.isNaN(parsed) ? null : parsed;
}

export type ShortTermMemoryRuntimeNoticeKind =
  | "compaction_requested"
  | "compaction_started"
  | "compaction_completed"
  | "compaction_failed"
  | "tool_schema_over_budget";

export interface ShortTermMemoryRuntimeNotice {
  kind: ShortTermMemoryRuntimeNoticeKind;
  timestamp: number;
  estimatedTokens?: number | null;
  limitTokens?: number | null;
  toolCount?: number | null;
}

function shortTermMemoryEventName(event: TranscriptAguiEvent) {
  if ("name" in event && typeof event.name === "string") return event.name;
  if ("eventName" in event && typeof event.eventName === "string") return event.eventName;
  return "";
}

function numberFromUnknown(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function compactionNoticeKind(event: TranscriptAguiEvent): ShortTermMemoryRuntimeNoticeKind | null {
  switch (shortTermMemoryEventName(event)) {
    case "tron.short_term_memory.context.compaction_requested":
    case "agent.context.compaction_requested":
      return "compaction_requested";
    case "tron.short_term_memory.compaction.started":
    case "agent.compaction.started":
      return "compaction_started";
    case "tron.short_term_memory.compaction.completed":
    case "agent.compaction.completed":
      return "compaction_completed";
    case "tron.short_term_memory.compaction.failed":
    case "agent.compaction.failed":
      return "compaction_failed";
    default:
      return null;
  }
}

function isRunFinishedEvent(event: TranscriptAguiEvent) {
  const type = aguiEventType(event).toUpperCase();
  return type === "RUN_FINISHED" || type === "AGENT.RUN.COMPLETED";
}

function isRunErrorEvent(event: TranscriptAguiEvent) {
  const type = aguiEventType(event).toUpperCase();
  return type === "RUN_ERROR" || type === "AGENT.RUN.FAILED";
}

function toolSchemaBudgetFields(message: string) {
  const matched = message.match(
    /estimatedTokens=(\d+),\s*limitTokens=(\d+),\s*toolCount=(\d+)/i,
  );
  return {
    estimatedTokens: matched ? Number.parseInt(matched[1], 10) : null,
    limitTokens: matched ? Number.parseInt(matched[2], 10) : null,
    toolCount: matched ? Number.parseInt(matched[3], 10) : null,
  };
}

export function latestShortTermMemoryRuntimeNotice(
  events: TranscriptAguiEvent[],
): ShortTermMemoryRuntimeNotice | null {
  let compactionNotice: ShortTermMemoryRuntimeNotice | null = null;
  let toolSchemaNotice: ShortTermMemoryRuntimeNotice | null = null;
  let latestSuccessfulRunAt = Number.NEGATIVE_INFINITY;

  for (let index = 0; index < events.length; index += 1) {
    const event = events[index];
    const timestamp = aguiEventMillis(event) ?? index;
    const kind = compactionNoticeKind(event);
    if (kind) {
      const payload = shortTermMemoryEventPayload(event);
      compactionNotice = {
        kind,
        timestamp,
        estimatedTokens: numberFromUnknown(payload?.compactionInputTokenEstimate)
          ?? numberFromUnknown(payload?.estimatedTokens),
        limitTokens: numberFromUnknown(payload?.maxInputTokens),
      };
    }

    if (isRunFinishedEvent(event)) {
      latestSuccessfulRunAt = Math.max(latestSuccessfulRunAt, timestamp);
      continue;
    }
    if (!isRunErrorEvent(event)) continue;
    const metadata = aguiEventMetadata(event);
    const code = (
      "code" in event && typeof event.code === "string"
        ? event.code
        : metadata?.failureCode ?? ""
    ).toLowerCase();
    const message = aguiEventSummary(event);
    if (code !== "tool_schema_over_budget" && !message.includes("TOOL_SCHEMA_OVER_BUDGET")) {
      continue;
    }
    const fields = toolSchemaBudgetFields(message);
    toolSchemaNotice = {
      kind: "tool_schema_over_budget",
      timestamp,
      ...fields,
    };
  }

  if (toolSchemaNotice && toolSchemaNotice.timestamp < latestSuccessfulRunAt) {
    toolSchemaNotice = null;
  }
  if (!toolSchemaNotice) return compactionNotice;
  if (!compactionNotice) return toolSchemaNotice;
  return toolSchemaNotice.timestamp >= compactionNotice.timestamp
    ? toolSchemaNotice
    : compactionNotice;
}
















function aguiEventStableKey(event: TranscriptAguiEvent) {
  const metadata = "metadata" in event ? event.metadata as Record<string, unknown> | undefined : undefined;
  const tron = metadata?.tron as { eventId?: unknown } | undefined;
  if (typeof tron?.eventId === "string") return `protocol:${tron.eventId}`;
  if ("id" in event && typeof event.id === "string") return `audit:${event.id}`;
  const messageId = "messageId" in event && typeof event.messageId === "string" ? event.messageId : "";
  const toolCallId = "toolCallId" in event && typeof event.toolCallId === "string" ? event.toolCallId : "";
  return [
    aguiEventRunCode(event),
    aguiEventType(event),
    "name" in event ? event.name : "",
    aguiEventTime(event),
    messageId,
    toolCallId,
    aguiEventSummary(event),
    "value" in event ? JSON.stringify(event.value) : "",
  ].join("\u0000");
}

function mergeTranscriptAguiEvents<T extends TranscriptAguiEvent>(...sources: T[][]) {
  const seen = new Set<string>();
  const merged: T[] = [];
  sources.flat().forEach((event) => {
    const key = aguiEventStableKey(event);
    if (seen.has(key)) return;
    seen.add(key);
    merged.push(event);
  });
  return merged.sort((left, right) => (aguiEventMillis(left) ?? 0) - (aguiEventMillis(right) ?? 0));
}


































function availableChatModels(models: ModelSelectOption[]) {
  // Keep unavailable fixed models in this list. A persisted session selection
  // must remain visible (and disabled) instead of silently falling back.
  // Keep the PersonAI ChatAuto option too: modelsForAgent reads its reasoning
  // efforts for the synthetic automatic route and filters it out of the list.
  return models.filter((model) => model.modelType === "TEXT");
}

interface ChatModelOption {
  accessPlatform: string | null;
  cacheHitCoefficient: number | null;
  contextWindowTokens: number | null;
  defaultReasoningEffort: ReasoningEffort | null;
  displayName: string;
  enabled: boolean;
  inputCoefficient: number | null;
  isDefault: boolean;
  modelCode: string | null;
  modelName: string;
  modelSelection: ModelSelection | null;
  optionCode: string;
  providerCode: string | null;
  remoteStatus: string | null;
  qualityType: "High" | "Middle" | "Low" | null;
  source: "internal-agent" | "system-agent";
  supportedReasoningEfforts: ReasoningEffort[];
  upstreamVendorName: string | null;
  outputCoefficient: number | null;
  vendor: string | null;
}

interface QueryStatus {
  loading?: boolean;
  error?: Error | null;
  retry?: () => void;
}

interface ChatRightPanelSnapshot {
  compactionBusy: boolean;
  contextEvents: TranscriptAguiEvent[];
  goal?: ChatSessionGoalView | null;
  goalBusy: boolean;
  goalStatus: QueryStatus;
  latestCompaction?: ChatSessionCompaction | null;
  latestCompactionStatus: QueryStatus;
  latestRequestEstimate?: ChatTokenEstimateSnapshot | null;
  latestUsageAudit?: ChatUsageAuditSnapshot | null;
  messages: ChatMessage[];
  model?: ChatModelOption | null;
  selectedModel?: ChatModelOption | null;
  session: ChatSession;
  todos?: ChatSessionTodo[];
  todosStatus: QueryStatus;
  workspaceGoalRunning: boolean;
}

interface ChatAgentOption {
  agentCode: string;
  description: string;
  models: SystemAgentModelRecord[];
  name: string;
  kind: "internal" | "system";
}

const mainChatAgentOption: ChatAgentOption = {
  agentCode: "main-agent",
  description: "Platform main in-process agent runtime.",
  models: [],
  name: "Main Agent",
  kind: "internal",
};

function availableChatAgents(internalRecords: WorkspaceInternalAgentRecord[], systemRecords: WorkspaceSystemAgentRecord[]) {
  const internalAgents: ChatAgentOption[] = internalRecords.filter((record) => record.enabled !== false).map((record) => ({
    agentCode: record.agentCode,
    description: record.description,
    models: [] as SystemAgentModelRecord[],
    name: record.name,
    kind: "internal" as const,
  }));
  const systemAgents = systemRecords
    .filter((record) => record.available && record.enabled)
    .map((record) => ({
      agentCode: record.agentCode,
      description: record.description,
      models: record.models,
      name: record.name,
      kind: "system" as const,
    }));
  return [...internalAgents, ...systemAgents];
}

function isMainChatAgent(agentCode: string) {
  return agentCode.trim().toLowerCase() === mainChatAgentOption.agentCode;
}

function normalizeModelSelection(model: ModelSelectOption): ModelSelection {
  return model.modelSelection ?? {
    kind: "Fixed",
    providerCode: null,
    modelCode: model.modelCode,
    qualityType: null,
    routeCode: null,
  };
}

function modelsForAgent(models: ModelSelectOption[], selectedAgent: ChatAgentOption | null, automaticModelLabel: string): ChatModelOption[] {
  if (!selectedAgent || selectedAgent.kind === "internal") {
    const autoSource = models.find((model) => model.modelSelection?.kind === "ProviderRoute"
      && model.modelSelection.routeCode === "ChatAuto") ?? null;
    const fixedModels = models
      .filter((model) => model !== autoSource)
      .map((model) => ({
      accessPlatform: model.accessPlatform ?? null,
      cacheHitCoefficient: model.cacheHitCoefficient ?? null,
      contextWindowTokens: model.contextWindowTokens ?? null,
      defaultReasoningEffort: model.defaultReasoningEffort ?? null,
      displayName: model.displayName,
      enabled: model.enabled,
      inputCoefficient: model.inputCoefficient ?? null,
      isDefault: Boolean(model.isDefault),
      modelCode: model.modelCode,
      modelName: model.modelName,
      modelSelection: normalizeModelSelection(model),
      optionCode: model.optionCode ?? `main:${model.modelCode}`,
      providerCode: model.providerCode ?? null,
      remoteStatus: model.remoteStatus ?? null,
      qualityType: model.qualityType ?? model.modelSelection?.qualityType ?? null,
      source: "internal-agent" as const,
      supportedReasoningEfforts: model.supportedReasoningEfforts ?? [],
      upstreamVendorName: model.upstreamVendorName ?? null,
      outputCoefficient: model.outputCoefficient ?? null,
      vendor: model.vendor ?? null,
    }));
    return [autoChatModelOption(
      selectedAgent?.agentCode ?? mainChatAgentOption.agentCode,
      automaticModelLabel,
      autoSource,
    ), ...fixedModels];
  }

  return selectedAgent.models.map((model) => ({
    accessPlatform: null,
    cacheHitCoefficient: null,
    contextWindowTokens: model.contextWindowTokens ?? null,
    defaultReasoningEffort: null,
    displayName: model.displayName || model.modelId,
    enabled: true,
    inputCoefficient: null,
    isDefault: false,
    modelCode: null,
    modelName: model.modelId,
    modelSelection: null,
    optionCode: `system:${selectedAgent.agentCode}:${model.modelId}`,
    providerCode: null,
    remoteStatus: null,
    qualityType: null,
    source: "system-agent" as const,
    supportedReasoningEfforts: model.supportedReasoningEfforts ?? [],
    upstreamVendorName: null,
    outputCoefficient: null,
    vendor: null,
  }));
}

function autoChatModelOption(
  agentCode: string,
  displayName: string,
  source: ModelSelectOption | null,
): ChatModelOption {
  return {
    accessPlatform: null,
    cacheHitCoefficient: null,
    contextWindowTokens: null,
    defaultReasoningEffort: source?.defaultReasoningEffort ?? null,
    displayName,
    enabled: true,
    inputCoefficient: null,
    isDefault: true,
    modelCode: null,
    modelName: "",
    modelSelection: {
      kind: "Auto",
      providerCode: null,
      modelCode: null,
      qualityType: null,
      routeCode: null,
    },
    optionCode: `auto:${agentCode}`,
    providerCode: null,
    remoteStatus: null,
    qualityType: null,
    source: "internal-agent",
    supportedReasoningEfforts: source?.supportedReasoningEfforts ?? [],
    upstreamVendorName: null,
    outputCoefficient: null,
    vendor: null,
  };
}

export function defaultChatModelOption<
  TOption extends {
    enabled: boolean;
    isDefault: boolean;
    modelSelection: ModelSelection | null;
    source: ChatModelOption["source"];
  },
>(options: TOption[]) {
  if (!options.length) return null;
  if (options[0].source === "system-agent") return options[0];
  const enabled = options.filter((option) => option.enabled);
  return enabled.find((option) => option.modelSelection?.kind === "Auto")
    ?? enabled.find((option) => option.modelSelection?.kind === "ProviderRoute"
    && option.modelSelection.routeCode === "ChatAuto")
    ?? enabled.find((option) => option.isDefault)
    ?? enabled[0]
    ?? options[0];
}

function iconButtonClass(active = false) {
  return cn(
    "inline-flex size-8 flex-none items-center justify-center rounded-md border border-transparent text-[var(--color-on-surface-variant)] transition hover:bg-[var(--color-hover-surface)] hover:text-[var(--color-on-surface)] disabled:pointer-events-none disabled:opacity-50",
    active && "bg-[var(--color-secondary-container)] text-[var(--color-on-secondary-container)]",
  );
}

function smallActionButtonClass() {
  return "inline-flex min-h-8 items-center rounded-md border border-[var(--color-outline-variant)] px-2.5 text-xs font-medium text-[var(--color-on-surface-variant)] transition hover:bg-[var(--color-hover-surface)] hover:text-[var(--color-on-surface)] disabled:pointer-events-none disabled:opacity-50";
}

function QueryStatusLine({
  errorText,
  loadingText,
  retryText,
  status,
}: {
  errorText: string;
  loadingText: string;
  retryText: string;
  status?: QueryStatus;
}) {
  if (status?.loading) {
    return (
      <p className="inline-flex items-center gap-1.5 text-xs text-[var(--color-on-surface-variant)]">
        <LoaderCircle className="animate-spin" size={13} />
        <ShinyText>{loadingText}</ShinyText>
      </p>
    );
  }
  if (status?.error) {
    return (
      <div className="space-y-2 text-xs text-[var(--color-error)]">
        <div className="flex items-start gap-1.5">
          <CircleAlert className="mt-0.5 shrink-0" size={13} />
          <span>{errorText}</span>
        </div>
        {status.retry ? (
          <button className={smallActionButtonClass()} type="button" onClick={status.retry}>

            <span className="ml-1.5">{retryText}</span>
          </button>
        ) : null}
      </div>
    );
  }
  return null;
}



type BadgeToneName = "secondary" | "info" | "success" | "warning" | "error";

function goalStatusTone(status: ChatSessionGoalStatus): BadgeToneName {
  if (status === "BLOCKED") return "warning";
  if (status === "COMPLETED") return "success";
  if (status === "CANCELLED" || status === "CLEARED") return "secondary";
  return "secondary";
}

function todoStatusTone(status: ChatSessionTodoStatus): BadgeToneName {
  if (status === "completed") return "success";
  if (status === "cancelled") return "secondary";
  if (status === "in_progress") return "info";
  return "secondary";
}

function todoPriorityTone(priority: ChatSessionTodoPriority): BadgeToneName {
  if (priority === "high") return "warning";
  return "secondary";
}

function todoStatusDotClass(status: ChatSessionTodoStatus) {
  if (status === "in_progress") return "bg-[var(--color-focus-ring)]";
  if (status === "completed") return "bg-[var(--color-success)]";
  if (status === "cancelled") return "bg-[var(--color-outline-variant)]";
  return "bg-[var(--color-outline)]";
}

function isFoldedTodo(todo: ChatSessionTodo) {
  return todo.status === "completed" || todo.status === "cancelled";
}

function parseGoalMaxRunsDraft(value: string, fallback: number) {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? Math.max(1, parsed) : fallback;
}

function WorkspaceTabState({
  goal,
  running,
}: {
  goal?: ChatSessionGoalView | null;
  running: boolean;
}) {
  const { t } = useTranslation("pages");
  const status = goal?.goal.status;
  if (!status) return null;
  const label = t(`chat.shortTermMemory.goalStatuses.${status}`);

  if (status === "ACTIVE" && running) {
    return (
      <span className="inline-flex size-4 items-center justify-center text-[var(--color-focus-ring)]" title={label} aria-label={label}>
        <LoaderCircle className="animate-spin" size={14} />
      </span>
    );
  }

  if (status === "ACTIVE") {
    return (
      <span className="inline-flex size-4 items-center justify-center text-[var(--color-focus-ring)]" title={label} aria-label={label}>
        <CircleDot size={14} />
      </span>
    );
  }

  if (status === "BLOCKED") {
    return (
      <span className="inline-flex size-4 items-center justify-center text-[var(--color-warning)]" title={label} aria-label={label}>
        <CircleAlert size={14} />
      </span>
    );
  }

  if (status === "PAUSED") {
    return (
      <span className="inline-flex size-4 items-center justify-center text-[var(--color-on-surface-variant)]" title={label} aria-label={label}>
        <Pause size={14} />
      </span>
    );
  }

  if (status === "COMPLETED") {
    return (
      <span className="inline-flex size-4 items-center justify-center text-[var(--color-success)]" title={label} aria-label={label}>
        <CheckCircle2 size={14} />
      </span>
    );
  }

  return null;
}

function GoalDetailBlock({
  children,
  title,
  tone = "neutral",
}: {
  children: React.ReactNode;
  title: string;
  tone?: "neutral" | "warning";
}) {
  return (
    <div
      className={cn(
        "space-y-1 border-l-2 border-[var(--color-outline-variant)] pl-3",
        tone === "warning" && "border-[var(--color-warning)]",
      )}
    >
      <div className="text-[11px] font-medium text-[var(--color-on-surface-variant)]">{title}</div>
      <div className="max-h-24 overflow-y-auto whitespace-pre-wrap break-words pr-2 text-xs leading-5 text-[var(--color-on-surface)]">
        {children}
      </div>
    </div>
  );
}

function goalVerdictLabel(value: string | null | undefined, translate: (key: string) => string) {
  const normalized = value?.trim().toUpperCase();
  if (!normalized) return translate("chat.shortTermMemory.goalVerdicts.UNKNOWN");
  if (normalized === "CONTINUE") return translate("chat.shortTermMemory.goalVerdicts.CONTINUE");
  if (normalized === "COMPLETE" || normalized === "COMPLETED") return translate("chat.shortTermMemory.goalVerdicts.COMPLETED");
  if (normalized === "BLOCKED") return translate("chat.shortTermMemory.goalVerdicts.BLOCKED");
  if (normalized === "NEEDS_USER_INPUT") return translate("chat.shortTermMemory.goalVerdicts.NEEDS_USER_INPUT");
  if (normalized === "FAILED" || normalized === "ERROR") return translate("chat.shortTermMemory.goalVerdicts.FAILED");
  return value?.trim() ?? normalized;
}

interface ContextMemoryBlockItem {
  placement?: ContextBlockPlacement | "protocol";
  code: string;
  colorIndex: number;
  detailCode?: ContextDetailBlock;
  detailValue?: unknown;
  snapshotSequence?: string;
  order: number;
  sources?: { id: string; name: string }[];
  limitDescription?: string;
  configuredTokens?: number | null;
  lentTokens?: number;
  borrowedTokens?: number;
  unallocatedTokens?: number;
  limitTokens: number | null;
  name: string;
  description?: string;
  updateMode?: ContextBlockUpdateMode;
  stat: string;
  tokens: number;
}



export function buildReportedContextMemoryBlocks(
  snapshot: ContextAssemblySnapshot,
  translate: (key: string, options?: Record<string, unknown>) => string,
  catalog?: ContextBlockCatalog,
): ContextMemoryBlockItem[] {
  const prefix = "chat.shortTermMemory.contextPanel.";
  const borrowing = contextBudgetBorrowing(snapshot.blocks);
  const placements = contextBlockPlacements(snapshot.blocks, "framework.history");
  const label = (id: string) => catalog?.get(id)?.displayName || contextBlockName(id, translate);
  const detailAliases: Record<string, ContextDetailBlock> = {
    "framework.instructions": "systemPrompt", seed_prompt_prefix: "systemPrompt",
    application_context: "applicationContext", "framework.history": "history",
  };
  const blocks: ContextMemoryBlockItem[] = [...snapshot.blocks].sort((left,right) => left.position-right.position).map((block, index) => ({
    code: block.id,
    placement: placements.get(block.id),
    colorIndex: index,
    detailCode: block.detailValue !== undefined || snapshot.snapshotSequence ? detailAliases[block.id] ?? block.id : undefined,
    detailValue: block.detailValue,
    snapshotSequence: snapshot.snapshotSequence,
    order: index + 1,
    sources: block.sources.length > 1 || block.sources.some((id) => id !== block.id)
      ? block.sources.map((id) => ({ id, name: id === "framework.instructions" ? translate(prefix + "baseInstructionsName") : label(id) })) : undefined,
    configuredTokens: block.configuredTokens,
    lentTokens: borrowing.lentByBlock.get(block.id) ?? 0,
    borrowedTokens: block.id === "framework.history" ? borrowing.borrowedTokens : 0,
    unallocatedTokens: block.id === "framework.history" ? borrowing.unallocatedTokens : 0,
    limitTokens: block.configuredTokens,
    name: label(block.id),
    description: catalog?.get(block.id)?.description,
    updateMode: catalog?.get(block.id)?.updateMode,
    tokens: block.finalTokens,
    stat: translate(prefix + "messageCount", { count: block.messageCount }),
    limitDescription: block.reason || (block.originalTokens !== null && block.originalTokens > block.finalTokens
      ? translate(prefix + "reductionStat", { original: formatCompactTokenCount(block.originalTokens), final: formatCompactTokenCount(block.finalTokens) }) : undefined),
  }));
  const protocolLimit = snapshot.budgetBreakdown?.toolSchemaBudgetTokens;
  if (snapshot.protocolTokens !== null) blocks.push({
    code: "protocol",
    placement: "protocol",
    order: blocks.length + 1,
    colorIndex: blocks.length,
    detailCode: snapshot.toolsDetail !== undefined || snapshot.snapshotSequence ? "toolSchema" : undefined,
    detailValue: snapshot.toolsDetail,
    snapshotSequence: snapshot.snapshotSequence,
    name: label("protocol") === "protocol" ? translate(prefix + "protocolName") : label("protocol"),
    description: catalog?.get("protocol")?.description,
    updateMode: catalog?.get("protocol")?.updateMode,
    stat: "",
    limitTokens: typeof protocolLimit === "number" && Number.isSafeInteger(protocolLimit) && protocolLimit >= 0
      ? protocolLimit : null,
    tokens: snapshot.protocolTokens,
    limitDescription: translate(prefix + "protocolNote"),
  });
  return blocks;
}

// 主题图表色循环取色：同一份装配报告里相邻的记忆块必定不同色，明细行沿用同一取色结果。
const contextBlockChartColors = [
  "bg-[var(--color-chart-1)]",
  "bg-[var(--color-chart-2)]",
  "bg-[var(--color-chart-3)]",
  "bg-[var(--color-chart-4)]",
  "bg-[var(--color-chart-5)]",
];

function contextBlockColorClass(colorIndex: number) {
  return contextBlockChartColors[Math.abs(Math.trunc(colorIndex)) % contextBlockChartColors.length];
}

function ContextOverviewPanel({
  breakdown,
  compactionBusy,

  snapshot,
  modelLabel,
  onCreateCompaction,
  session,
}: {
  breakdown: ChatContextBudgetBreakdown | null;
  compactionBusy?: boolean;
  snapshot: ContextAssemblySnapshot | null;
  modelLabel: string;
  onCreateCompaction?: () => void;
  session?: ChatSession;
}) {
  const { t } = useTranslation("pages");
  const usedTokens = snapshot?.estimatedInputTokens ?? null;
  const inputBudget = snapshot?.inputCapacity ?? null;
  const windowLimit = breakdown?.contextWindowLimit ?? null;
  const percent = usedTokens !== null && inputBudget !== null && inputBudget > 0
    ? contextPercent(usedTokens, inputBudget) : null;
  const statusLabel = contextStatusLabel(null, breakdown, (key) => t(key));
  const statusTone = contextStatusTone(null, breakdown);

  return (
    <div className="min-w-0 space-y-3">
      <div className="flex min-w-0 items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-1">
            <h3 className="text-xs font-semibold uppercase text-[var(--color-on-surface)]">
              {t("chat.shortTermMemory.contextPanel.shortTermMemory")}
            </h3>
            <InfoTooltip ariaLabel={t("chat.shortTermMemory.contextPanel.budgetFormulaAria")} align="start">
              <p className="max-w-[18rem] whitespace-normal leading-5">{t("chat.shortTermMemory.contextPanel.savedReportHelp")}</p>
            </InfoTooltip>
          </div>
          <p className="truncate text-[11px] text-[var(--color-on-surface-variant)]" title={modelLabel} data-testid="context-model-label">
            {modelLabel}
            {windowLimit ? ` · ${formatCompactTokenCount(windowLimit)} ${t("chat.shortTermMemory.contextPanel.windowSuffix")}` : ""}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {snapshot && statusLabel ? <Badge className="h-5 rounded px-1.5 text-[11px]" variant={statusTone}>
            {statusLabel}
          </Badge> : null}
          <Button
            aria-label={t(compactionBusy ? "chat.shortTermMemory.compactingMemory" : "chat.shortTermMemory.compactMemory")}
            aria-busy={compactionBusy || undefined}
            disabled={!session || !onCreateCompaction || compactionBusy}
            size="icon-sm"
            title={t("chat.shortTermMemory.compactMemory")}
            type="button"
            variant="ghost"
            onClick={onCreateCompaction}
          >
            {compactionBusy ? <LoaderCircle aria-hidden="true" className="animate-spin" /> : <FileArchive aria-hidden="true" />}
          </Button>
        </div>
      </div>
      <div className="flex min-w-0 items-baseline justify-between gap-3">
        <div className="min-w-0 truncate text-2xl font-bold text-[var(--color-on-surface)]">
          {formatCompactTokenCount(usedTokens)}
          <span className="px-1">/</span>
          {formatCompactTokenCount(inputBudget)}
        </div>
        <div className="shrink-0 text-sm font-medium text-[var(--color-on-surface-variant)]">
          {formatPercent(percent)}
        </div>
      </div>
      {percent !== null ? <div className="h-2.5 overflow-hidden rounded-full bg-[var(--color-surface-container-high)]">
        <div
          className={cn(
            "h-full rounded-full transition-[width]",
            statusTone === "error" ? "bg-[var(--color-error)]" : statusTone === "warning" ? "bg-[var(--color-warning)]" : "bg-[var(--color-success)]",
          )}
          style={{ width: `${percent}%` }}
        />
      </div> : null}
    </div>
  );
}

function ContextRuntimeNoticePanel({ notice }: { notice: ShortTermMemoryRuntimeNotice }) {
  const { t } = useTranslation("pages");
  const pending = notice.kind === "compaction_requested" || notice.kind === "compaction_started";
  const success = notice.kind === "compaction_completed";
  const title = t(`chat.shortTermMemory.contextPanel.runtimeNotices.${notice.kind}.title`);
  const detail = notice.kind === "tool_schema_over_budget"
    && notice.estimatedTokens != null
    && notice.limitTokens != null
    ? t("chat.shortTermMemory.contextPanel.runtimeNotices.tool_schema_over_budget.detailWithBudget", {
      actual: formatCompactTokenCount(notice.estimatedTokens),
      limit: formatCompactTokenCount(notice.limitTokens),
      count: notice.toolCount ?? "-",
    })
    : t(`chat.shortTermMemory.contextPanel.runtimeNotices.${notice.kind}.detail`);

  return (
    <section
      aria-live="polite"
      className={cn(
        "flex items-start gap-2.5 rounded-md border px-3 py-2.5 text-xs",
        pending && "border-[var(--color-outline-variant)] bg-[var(--color-secondary-container)] text-[var(--color-on-secondary-container)]",
        success && "border-[var(--color-success)]/30 bg-[var(--color-success-container)] text-[var(--color-on-success-container)]",
        !pending && !success && "border-[var(--color-error)]/30 bg-[var(--color-error-container)] text-[var(--color-on-error-container)]",
      )}
      data-notice-kind={notice.kind}
      data-testid="context-runtime-notice"
      role={!pending && !success ? "alert" : "status"}
    >
      {pending ? (
        <LoaderCircle className="mt-0.5 shrink-0 animate-spin" size={14} />
      ) : success ? (
        <CheckCircle2 className="mt-0.5 shrink-0" size={14} />
      ) : (
        <CircleAlert className="mt-0.5 shrink-0" size={14} />
      )}
      <div className="min-w-0 space-y-0.5">
        <div className="font-medium">{title}</div>
        <div className="break-words opacity-80">{detail}</div>
      </div>
    </section>
  );
}

function ContextStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-md bg-[var(--color-surface-container)] px-2 py-1.5">
      <div className="truncate text-sm font-semibold text-[var(--color-on-surface)]">{value}</div>
      <div className="truncate text-[10px] text-[var(--color-on-surface-variant)]">{label}</div>
    </div>
  );
}

function ContextStructurePanel({
  sessionCode,
  blocks,
  assistantCode,
}: {
  sessionCode: string;
  blocks: ContextMemoryBlockItem[];
  assistantCode?: string;
}) {
  return (
    <div>
      {blocks.map((block) => (
        <ContextMemoryBlockRow key={block.code} block={block} assistantCode={assistantCode} sessionCode={sessionCode} />
      ))}
    </div>
  );
}

function ContextMemoryBlockRow({ block, assistantCode, sessionCode }: { block: ContextMemoryBlockItem; assistantCode?: string; sessionCode: string }) {
  const { t } = useTranslation("pages");
  const navigate = useNavigate();
  const [dialogOpen, setDialogOpen] = useState(false);
  const [detail, setDetail] = useState<unknown>(null);
  const [detailViewMode, setDetailViewMode] = useState<"preview" | "raw">("preview");
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState(false);
  const fillClass = contextBlockColorClass(block.colorIndex);
  const percent = contextPercent(block.tokens, block.limitTokens);
  const hasCapacity = block.limitTokens != null && block.limitTokens > 0;
  const lentTokens = block.lentTokens ?? 0;
  const borrowedTokens = block.borrowedTokens ?? 0;
  const unallocatedTokens = block.unallocatedTokens ?? 0;
  const budgetTitle = lentTokens > 0
    ? t("chat.shortTermMemory.contextPanel.lentBudget", { used: block.tokens, configured: block.configuredTokens, lent: lentTokens })
    : borrowedTokens > 0
      ? t("chat.shortTermMemory.contextPanel.borrowedBudget", { used: block.tokens, configured: block.configuredTokens, borrowed: borrowedTokens, unallocated: unallocatedTokens })
      : block.limitTokens == null ? String(block.tokens) : `${block.tokens} / ${block.limitTokens}`;
  useEffect(() => {
    if (!dialogOpen || !block.detailCode) return;
    if (block.detailValue !== undefined) {
      setDetail(block.detailValue);
      setLoading(false);
      setLoadError(false);
      return;
    }
    const controller = new AbortController();
    setLoading(true);
    setLoadError(false);
    setDetail(null);
    let refreshing = true;
    void requestApi<unknown>(chatApiEndpoints.getChatContextDetail, {
      queryParams: { sessionCode, blockCode: block.detailCode, ...(block.snapshotSequence ? { snapshotSequence: block.snapshotSequence } : {}) },
      headers: assistantCode ? { "X-Assistant-Code": assistantCode } : undefined,
      signal: controller.signal,
    }).then((value) => { if (!controller.signal.aborted) setDetail(value); }).catch(() => {
      if (!controller.signal.aborted) setLoadError(true);
    }).finally(() => {
      refreshing = false;
      if (!controller.signal.aborted) setLoading(false);
    });
    const timer = block.detailCode === "applicationContext" && !block.snapshotSequence ? window.setInterval(() => {
      if (refreshing) return;
      refreshing = true;
      void requestApi<unknown>(chatApiEndpoints.getChatContextDetail, {
        queryParams: { sessionCode, blockCode: block.detailCode }, headers: assistantCode ? { "X-Assistant-Code": assistantCode } : undefined, signal: controller.signal,
      }).then((value) => { if (!controller.signal.aborted) { setDetail(value); setLoadError(false); } }).catch(() => {
        if (!controller.signal.aborted) setLoadError(true);
      }).finally(() => { refreshing = false; });
    }, 3000) : undefined;
    return () => { controller.abort(); if (timer !== undefined) window.clearInterval(timer); };
  }, [block.detailCode, block.detailValue, block.snapshotSequence, dialogOpen, assistantCode, sessionCode]);
  let parsedDetail = detail;
  if (typeof detail === "string") {
    try {
      parsedDetail = JSON.parse(detail) as unknown;
    } catch {
      parsedDetail = detail;
    }
  }
  const rawValue = block.detailCode ? contextDetailRawValue(block.detailCode, parsedDetail) : parsedDetail;
  const rawDetail = detail == null
    ? ""
    : typeof rawValue === "string" ? rawValue : JSON.stringify(rawValue, null, 2) ?? String(rawValue);
  const contents = (
    <>
      <div className="flex min-w-0 items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-1">
          <button type="button" disabled={!block.detailCode} onClick={() => setDialogOpen(true)} title={block.code === "protocol" ? undefined : block.limitDescription} className="group flex min-w-0 flex-wrap items-baseline gap-1 rounded text-left outline-none enabled:cursor-pointer focus-visible:ring-2 focus-visible:ring-[var(--color-focus-ring)]">
            {block.order ? <span className="shrink-0 font-mono text-[10px] text-[var(--color-on-surface-variant)]">{block.order}.</span> : null}
            <span className="min-w-0 break-words text-xs font-semibold text-[var(--color-on-surface)] transition-colors group-hover:text-[var(--color-primary)]" title={block.code === "protocol" ? undefined : block.code}>{block.name}</span>
            {block.stat ? <span className="shrink-0 text-[10px] text-[var(--color-on-surface-variant)]">· {block.stat}</span> : null}
          </button>
          <ContextBlockHelp name={block.name} description={block.description} updateMode={block.updateMode}>
            {block.sources?.length ? <div>
                <p className="mb-1">{t("chat.shortTermMemory.contextPanel.mergedSourcesNote")}</p>
                <ol className="space-y-1">
                  {block.sources.map((source, index) => <li className="break-words" key={`${source.id}:${index}`}>
                    <span className="mr-1 font-mono">{block.order}.{index + 1}</span>{source.name}
                  </li>)}
                </ol>
              </div> : null}
          </ContextBlockHelp>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {borrowedTokens > 0 ? <Badge size="sm" variant="info" title={budgetTitle}>{t("chat.shortTermMemory.contextPanel.borrowing")}</Badge> : null}
          <span className="font-mono text-[11px]" data-testid={`context-budget-${block.code}`} title={budgetTitle}>
            {lentTokens > 0 ? <>
              ({formatCompactTokenCount(block.tokens)} + <span className="text-[var(--color-primary)]">{formatCompactTokenCount(lentTokens)}</span>) / {formatCompactTokenCount(block.limitTokens)}
            </> : block.limitTokens == null ? formatCompactTokenCount(block.tokens) : formatTokenLimit(block.tokens, block.limitTokens)}
          </span>
        </div>
      </div>
      {hasCapacity ? <div className="relative mt-1.5 h-1.5 overflow-hidden rounded-full bg-[var(--color-surface-container-high)]" role="progressbar" aria-label={block.name} aria-valuetext={budgetTitle} aria-valuenow={Math.min(block.tokens + lentTokens, block.limitTokens!)} aria-valuemin={0} aria-valuemax={block.limitTokens!} title={budgetTitle}>
        <div className={cn("absolute inset-y-0 left-0", fillClass)} style={{ width: `${percent}%` }} />
        {lentTokens > 0 ? <div className="absolute inset-y-0 bg-[var(--color-primary)] opacity-50" data-testid={`context-lent-${block.code}`} style={{ left: `${percent}%`, width: `${contextPercent(lentTokens, block.limitTokens)}%` }} /> : null}
      </div> : null}
    </>
  );
  return (
    <div className="py-2 first:pt-0 last:pb-0">
      {contents}
      <Dialog open={dialogOpen} onOpenChange={(open) => {
        setDialogOpen(open);
        if (open) setDetailViewMode("preview");
      }}>
        <DialogContent closeLabel={t("chat.applications.closePreview")} className="grid max-h-[80vh] sm:max-w-2xl grid-rows-[auto_minmax(0,1fr)] gap-3 overflow-hidden">
          <div className="flex min-w-0 items-center justify-between gap-3 pr-8">
            <h2 className="truncate text-sm font-semibold text-[var(--color-on-surface)]">{block.name}</h2>
            {detail != null ? (
              <div className="flex shrink-0 items-center gap-1">
                <CopyValueButton
                  ariaLabel={t("chat.shortTermMemory.contextPanel.copyRawJson")}
                  title={t("chat.shortTermMemory.contextPanel.copyRawJson")}
                  value={rawDetail}
                />
                <div className="flex items-center rounded-md bg-[var(--color-surface-container-low)] p-0.5">
                <button
                  aria-pressed={detailViewMode === "preview"}
                  className={cn("rounded px-2 py-1 text-xs transition-colors", detailViewMode === "preview" ? "bg-[var(--color-elevated-surface)] text-[var(--color-on-surface)] shadow-sm" : "text-[var(--color-on-surface-variant)] hover:text-[var(--color-on-surface)]")}
                  type="button"
                  onClick={() => setDetailViewMode("preview")}
                >
                  {t("chat.shortTermMemory.contextPanel.detailViewTree")}
                </button>
                <button
                  aria-pressed={detailViewMode === "raw"}
                  className={cn("rounded px-2 py-1 text-xs transition-colors", detailViewMode === "raw" ? "bg-[var(--color-elevated-surface)] text-[var(--color-on-surface)] shadow-sm" : "text-[var(--color-on-surface-variant)] hover:text-[var(--color-on-surface)]")}
                  type="button"
                  onClick={() => setDetailViewMode("raw")}
                >
                  {t("chat.shortTermMemory.contextPanel.detailViewRaw")}
                </button>
                </div>
              </div>
            ) : null}
          </div>
          <div className="min-h-0 overflow-auto rounded-md bg-[var(--color-surface-container-lowest)] p-3">
            {block.detailCode === "systemPrompt" && block.limitDescription ? <p className="mb-3 text-xs text-[var(--color-on-surface-variant)]">{block.limitDescription}</p> : null}
            {block.detailCode === "applicationContext" ? <p className="mb-3 text-xs text-[var(--color-on-surface-variant)]">{t("chat.applications.snapshotNote")}</p> : null}
            {loading ? <p className="text-sm text-[var(--color-on-surface-variant)]">{t("chat.shortTermMemory.contextPanel.detailLoading")}</p>
              : loadError ? <p className="text-sm text-[var(--color-error)]">{t("chat.shortTermMemory.contextPanel.detailFailed")}</p>
                : detailViewMode === "preview" && block.detailCode
                  ? <ContextDetailPreview block={block.detailCode} value={parsedDetail} onOpenPrompt={(promptCode) => {
                      const promptPath = `/settings/prompts/detail?${new URLSearchParams({ promptCode }).toString()}`;
                      const tabId = useTabStore.getState().addTab({ title: t("chat.shortTermMemory.contextPanel.detailPromptManagement"), path: promptPath, closable: true, activeNav: `prompt-detail:${promptCode}` });
                      useTabStore.getState().updateTab(tabId, { path: promptPath });
                      setDialogOpen(false);
                      navigate(promptPath);
                    }} />
                  : <pre className="whitespace-pre-wrap break-all font-mono text-xs leading-5 text-[var(--color-on-surface)]">{rawDetail}</pre>}
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function ProviderUsagePanel({
  events,
  fallbackSessionUsage,
  usageAudit,
}: {
  events: TranscriptAguiEvent[];
  fallbackSessionUsage: { input: number | null; output: number | null; total: number | null };
  usageAudit: ChatUsageAuditSnapshot | null;
}) {
  const { t } = useTranslation("pages");
  const providerUsage = usageAudit?.providerUsage ?? null;
  const sessionUsage = sessionProviderUsageFromEvents(events);
  const sessionInput = sessionUsage?.inputTokens ?? fallbackSessionUsage.input;
  const sessionOutput = sessionUsage?.outputTokens ?? fallbackSessionUsage.output;

  return (
    <section className="space-y-3 rounded-md border border-[var(--color-outline-variant)] bg-[var(--color-surface-container-lowest)] p-3">
      <div>
        <h3 className="text-xs font-semibold uppercase text-[var(--color-on-surface-variant)]">
          {t("chat.shortTermMemory.contextSections.providerUsage")}
        </h3>
      </div>
      <div>
        <h4 className="mb-1.5 text-[11px] font-medium text-[var(--color-on-surface-variant)]">
          {t("chat.shortTermMemory.contextPanel.currentSessionUsage")}
        </h4>
        <div className="grid grid-cols-4 gap-1.5">
          <ContextStat label={t("chat.shortTermMemory.contextPanel.input")} value={formatCompactTokenCount(sessionInput)} />
          <ContextStat label={t("chat.shortTermMemory.contextPanel.cached")} value={formatCompactTokenCount(sessionUsage?.cachedInputTokens ?? 0)} />
          <ContextStat label={t("chat.shortTermMemory.contextPanel.reasoning")} value={formatCompactTokenCount(sessionUsage?.reasoningTokens)} />
          <ContextStat label={t("chat.shortTermMemory.contextPanel.output")} value={formatCompactTokenCount(sessionOutput)} />
        </div>
      </div>
      <div>
        <h4 className="mb-1.5 text-[11px] font-medium text-[var(--color-on-surface-variant)]">
          {t("chat.shortTermMemory.contextPanel.latestRequestUsage")}
        </h4>
        <div className="grid grid-cols-4 gap-1.5">
          <ContextStat label={t("chat.shortTermMemory.contextPanel.input")} value={formatCompactTokenCount(providerUsage?.inputTokens)} />
          <ContextStat label={t("chat.shortTermMemory.contextPanel.cached")} value={formatCompactTokenCount(providerUsage?.cachedInputTokens)} />
          <ContextStat label={t("chat.shortTermMemory.contextPanel.reasoning")} value={formatCompactTokenCount(providerUsage?.reasoningTokens)} />
          <ContextStat label={t("chat.shortTermMemory.contextPanel.output")} value={formatCompactTokenCount(providerUsage?.outputTokens)} />
        </div>
      </div>
    </section>
  );
}

function RightWorkspaceSidebar({
  goal,
  goalBusy,
  goalStatus,
  onCancelGoal,
  onClearGoal,
  onCreateGoal,
  onPauseGoal,
  onResumeGoal,
  onUpdateGoal,
  sessionCode,
  todos,
  todosStatus,
}: {
  goal?: ChatSessionGoalView | null;
  goalBusy?: boolean;
  goalStatus?: QueryStatus;
  onCancelGoal?: (input: ChatSessionGoalCommand) => Promise<boolean> | boolean;
  onClearGoal?: (input: ChatSessionGoalCommand) => Promise<boolean> | boolean;
  onCreateGoal?: (input: SetChatSessionGoalInput) => Promise<boolean> | boolean;
  onPauseGoal?: (input: ChatSessionGoalCommand) => Promise<boolean> | boolean;
  onResumeGoal?: (input: ChatSessionGoalCommand) => Promise<boolean> | boolean;
  onUpdateGoal?: (input: UpdateChatSessionGoalInput) => Promise<boolean> | boolean;
  sessionCode?: string;
  todos?: ChatSessionTodo[];
  todosStatus?: QueryStatus;
}) {
  const { t } = useTranslation("pages");
  const [creatingGoal, setCreatingGoal] = useState(false);
  const [editingGoal, setEditingGoal] = useState(false);
  const [goalDraft, setGoalDraft] = useState("");
  const [goalMaxRunsDraft, setGoalMaxRunsDraft] = useState("");
  const syncedGoalDraftSourceRef = useRef<string | null>(null);
  const goalDraftSource = useMemo(() => JSON.stringify({
    goalCode: goal?.goal.goalCode ?? "",
    maxRuns: goal?.goal.maxRuns ?? 0,
    objective: goal?.goal.objectiveMarkdown ?? "",
    sessionCode: sessionCode ?? "",
  }), [goal?.goal.goalCode, goal?.goal.maxRuns, goal?.goal.objectiveMarkdown, sessionCode]);
  const activeTodos = (todos ?? []).filter((todo) => !isFoldedTodo(todo));
  const foldedTodoCount = (todos ?? []).length - activeTodos.length;
  const hasGoalLoadState = Boolean(goalStatus?.loading || goalStatus?.error);
  const hasTodoLoadState = Boolean(todosStatus?.loading || todosStatus?.error);
  const goalStatusLabel = goal ? t(`chat.shortTermMemory.goalStatuses.${goal.goal.status}`) : "";
  const canCreateGoal = Boolean(sessionCode && onCreateGoal && (!goal || canStartNewGoalFromStatus(goal.goal.status)));
  const canEditGoal = Boolean(goal && onUpdateGoal && isEditableGoalStatus(goal.goal.status));
  const showCreateGoalForm = canCreateGoal && (creatingGoal || !goal);

  useEffect(() => {
    if (creatingGoal || editingGoal) return;
    if (syncedGoalDraftSourceRef.current === goalDraftSource) return;

    syncedGoalDraftSourceRef.current = goalDraftSource;
    setGoalDraft(goal?.goal.objectiveMarkdown ?? "");
    setGoalMaxRunsDraft(goal ? String(goal.goal.maxRuns) : "20");
  }, [creatingGoal, editingGoal, goal, goalDraftSource]);

  function resetGoalDraft() {
    setGoalDraft(goal?.goal.objectiveMarkdown ?? "");
    setGoalMaxRunsDraft(goal ? String(goal.goal.maxRuns) : "20");
  }

  function resetCreateGoalDraft() {
    setGoalDraft("");
    setGoalMaxRunsDraft("20");
  }

  function startGoalCreate() {
    resetCreateGoalDraft();
    setEditingGoal(false);
    setCreatingGoal(true);
  }

  function startGoalEdit() {
    resetGoalDraft();
    setCreatingGoal(false);
    setEditingGoal(true);
  }

  function cancelGoalEdit() {
    resetGoalDraft();
    setEditingGoal(false);
  }

  function cancelGoalCreate() {
    resetGoalDraft();
    setCreatingGoal(false);
  }

  async function handleCreateGoal() {
    if (!sessionCode || !onCreateGoal) return;
    const objective = goalDraft.trim();
    if (!objective) return;
    const saved = await onCreateGoal({
      sessionCode,
      objective,
      maxRuns: parseGoalMaxRunsDraft(goalMaxRunsDraft, 20),
    });
    if (saved !== false) {
      resetCreateGoalDraft();
      setCreatingGoal(false);
    }
  }

  async function handleSaveGoal() {
    if (!goal || !onUpdateGoal) return;
    const objective = goalDraft.trim();
    if (!objective) return;
    const saved = await onUpdateGoal({
      sessionCode: goal.goal.sessionCode,
      goalCode: goal.goal.goalCode,
      objective,
      maxRuns: parseGoalMaxRunsDraft(goalMaxRunsDraft, goal.goal.maxRuns),
    });
    if (saved !== false) setEditingGoal(false);
  }

  function goalCommandInput(): ChatSessionGoalCommand | null {
    if (!goal) return null;
    return {
      sessionCode: goal.goal.sessionCode,
      goalCode: goal.goal.goalCode,
    };
  }

  function runGoalCommand(action?: (input: ChatSessionGoalCommand) => Promise<boolean> | boolean) {
    const input = goalCommandInput();
    if (!input || !action) return;
    void action(input);
  }

  function renderGoalForm(mode: "create" | "edit") {
    const savingLabel = mode === "create"
      ? t("chat.shortTermMemory.setGoal")
      : t("chat.shortTermMemory.saveGoal");
    const onSave = mode === "create" ? handleCreateGoal : handleSaveGoal;
    const onCancel = mode === "create" ? cancelGoalCreate : cancelGoalEdit;
    const showCancel = mode === "edit" || Boolean(goal);

    return (
      <div className="space-y-3">
        <textarea
          className="min-h-28 w-full resize-y rounded-md border border-[var(--color-outline-variant)] bg-[var(--color-surface-container-lowest)] px-2 py-1.5 text-sm text-[var(--color-on-surface)] outline-none transition focus:border-[var(--color-primary)]"
          disabled={goalBusy}
          placeholder={t("chat.shortTermMemory.goalPlaceholder")}
          value={goalDraft}
          onChange={(event) => setGoalDraft(event.target.value)}
        />
        <label className="block space-y-1">
          <span className="text-[11px] font-medium text-[var(--color-on-surface-variant)]">
            {t("chat.shortTermMemory.goalMaxRuns")}
          </span>
          <input
            className="block h-8 w-24 rounded-md border border-[var(--color-outline-variant)] bg-[var(--color-surface-container-lowest)] px-2 text-sm text-[var(--color-on-surface)] outline-none transition focus:border-[var(--color-primary)]"
            disabled={goalBusy}
            type="text"
            inputMode="numeric"
            value={goalMaxRunsDraft}
            onChange={(event) => { if (/^\d*$/.test(event.target.value)) setGoalMaxRunsDraft(event.target.value); }}
          />
        </label>
        <div className="flex flex-wrap items-center gap-1.5">
          <button
            className={smallActionButtonClass()}
            disabled={goalBusy || !goalDraft.trim() || !goalMaxRunsDraft || Number(goalMaxRunsDraft) < 1}
            type="button"
            onClick={() => void onSave()}
          >

            <span className="ml-1.5">{goalBusy ? t("chat.shortTermMemory.savingGoal") : savingLabel}</span>
          </button>
          {showCancel ? (
            <button
              className={smallActionButtonClass()}
              disabled={goalBusy}
              type="button"
              onClick={onCancel}
            >

              <span className="ml-1.5">{t("chat.shortTermMemory.cancelEditGoal")}</span>
            </button>
          ) : null}
        </div>
      </div>
    );
  }

  function renderGoalActionButtons() {
    if (!goal || editingGoal || showCreateGoalForm) return null;
    const status = goal.goal.status;
    return (
      <div className="flex flex-wrap items-center gap-1.5">
        {status === "ACTIVE" && onPauseGoal ? (
          <button
            className={smallActionButtonClass()}
            disabled={goalBusy}
            type="button"
            onClick={() => runGoalCommand(onPauseGoal)}
          >

            <span className="ml-1.5">{t("chat.shortTermMemory.pauseGoal")}</span>
          </button>
        ) : null}
        {(status === "PAUSED" || status === "BLOCKED") && onResumeGoal ? (
          <button
            className={smallActionButtonClass()}
            disabled={goalBusy}
            type="button"
            onClick={() => runGoalCommand(onResumeGoal)}
          >

            <span className="ml-1.5">{t("chat.shortTermMemory.resumeGoal")}</span>
          </button>
        ) : null}
        {isEditableGoalStatus(status) && onCancelGoal ? (
          <button
            className={smallActionButtonClass()}
            disabled={goalBusy}
            type="button"
            onClick={() => runGoalCommand(onCancelGoal)}
          >

            <span className="ml-1.5">{t("chat.shortTermMemory.cancelGoal")}</span>
          </button>
        ) : null}
        {canClearGoalStatus(status) && onClearGoal ? (
          <button
            className={smallActionButtonClass()}
            disabled={goalBusy}
            type="button"
            onClick={() => runGoalCommand(onClearGoal)}
          >

            <span className="ml-1.5">{t("chat.shortTermMemory.clearGoal")}</span>
          </button>
        ) : null}
        {canCreateGoal ? (
          <button
            className={smallActionButtonClass()}
            disabled={goalBusy}
            type="button"
            onClick={startGoalCreate}
          >

            <span className="ml-1.5">{t("chat.shortTermMemory.newGoal")}</span>
          </button>
        ) : null}
      </div>
    );
  }

  return (
    <div className="space-y-5 text-sm">
      <ContextBlock
        title={t("chat.shortTermMemory.goal")}
        actions={goal ? (
          <div className="flex items-center gap-1.5">
            <Badge className="h-5 rounded px-1.5 text-[11px]" variant={goalStatusTone(goal.goal.status)}>
              {goalStatusLabel}
            </Badge>
            {canEditGoal && !editingGoal ? (
              <button
                className={smallActionButtonClass()}
                disabled={goalBusy}
                type="button"
                onClick={startGoalEdit}
              >

                <span className="ml-1.5">{t("chat.shortTermMemory.editGoal")}</span>
              </button>
            ) : null}
          </div>
        ) : null}
      >
        {hasGoalLoadState ? (
          <QueryStatusLine
            errorText={t("chat.shortTermMemory.loadFailed")}
            loadingText={t("chat.shortTermMemory.loadingGoal")}
            retryText={t("chat.shortTermMemory.retry")}
            status={goalStatus}
          />
        ) : showCreateGoalForm ? (
          renderGoalForm("create")
        ) : goal ? (
          editingGoal ? (
            renderGoalForm("edit")
          ) : (
            <div className="space-y-3">
              <div className="max-h-32 overflow-y-auto break-words pr-2 text-sm leading-5 text-[var(--color-on-surface)]">
                {goal.goal.objectiveMarkdown}
              </div>
              <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-[var(--color-on-surface-variant)]">
                <span>{goalVerdictLabel(goal.goal.lastVerdict, t)}</span>
                <span>{t("chat.shortTermMemory.goalRunCount", { used: goal.goal.runsUsed, max: goal.goal.maxRuns })}</span>
                {goal.goal.lastRunCode ? (
                  <span className="font-mono">{truncateLabel(goal.goal.lastRunCode, 18)}</span>
                ) : null}
              </div>
              {goal.goal.status === "BLOCKED" && goal.goal.blockedReason ? (
                <GoalDetailBlock title={t("chat.shortTermMemory.blockedReason")} tone="warning">
                  {goal.goal.blockedReason}
                </GoalDetailBlock>
              ) : null}
              {goal.goal.status === "BLOCKED" && goal.goal.requiredUserInput ? (
                <GoalDetailBlock title={t("chat.shortTermMemory.requiredUserInput")} tone="warning">
                  {goal.goal.requiredUserInput}
                </GoalDetailBlock>
              ) : null}
              {goal.goal.status !== "BLOCKED" && goal.goal.lastReason ? (
                <GoalDetailBlock title={t("chat.shortTermMemory.judgeReason")}>
                  {goal.goal.lastReason}
                </GoalDetailBlock>
              ) : null}
              {renderGoalActionButtons()}
            </div>
          )
        ) : (
          <p className="text-xs text-[var(--color-on-surface-variant)]">
            {t("chat.shortTermMemory.noGoal")}
          </p>
        )}
      </ContextBlock>

      <ContextBlock
        title={t("chat.shortTermMemory.todo")}
        actions={activeTodos.length ? (
          <Badge className="h-5 rounded px-1.5 text-[11px]" variant="secondary">
            {t("chat.shortTermMemory.activeTodoCount", { count: activeTodos.length })}
          </Badge>
        ) : null}
      >
        {hasTodoLoadState ? (
          <QueryStatusLine
            errorText={t("chat.shortTermMemory.loadFailed")}
            loadingText={t("chat.shortTermMemory.loadingTodos")}
            retryText={t("chat.shortTermMemory.retry")}
            status={todosStatus}
          />
        ) : activeTodos.length ? (
          <div className="space-y-1">
            {activeTodos.map((todo) => (
              <div
                key={todo.todoCode}
                className="grid min-w-0 grid-cols-[0.5rem_minmax(0,1fr)_auto_auto] items-center gap-[12px] py-1 text-xs"
                title={todo.content}
              >
                <span className={cn("size-2 rounded-full", todoStatusDotClass(todo.status))} aria-hidden="true" />
                <span className="min-w-0 truncate text-sm text-[var(--color-on-surface)]">{todo.content}</span>
                <Badge className="h-5 rounded px-1.5 text-[11px]" variant={todoStatusTone(todo.status)}>
                  {t(`chat.shortTermMemory.todoStatuses.${todo.status}`)}
                </Badge>
                <Badge className="h-5 rounded px-1.5 text-[11px]" variant={todoPriorityTone(todo.priority)}>
                  {t(`chat.shortTermMemory.todoPriorities.${todo.priority}`)}
                </Badge>
              </div>
            ))}
            {foldedTodoCount ? (
              <div className="border-t border-[var(--color-outline-variant)] pt-2 text-xs text-[var(--color-on-surface-variant)]">
                {t("chat.shortTermMemory.foldedTodoCount", { count: foldedTodoCount })}
              </div>
            ) : null}
          </div>
        ) : (
          <div className="space-y-2 text-xs text-[var(--color-on-surface-variant)]">
            <p>{t("chat.shortTermMemory.emptyTodos")}</p>
            {foldedTodoCount ? <p>{t("chat.shortTermMemory.foldedTodoCount", { count: foldedTodoCount })}</p> : null}
          </div>
        )}
      </ContextBlock>
    </div>
  );
}

export function RightContextSidebar({
  assistantCode,
  refreshInterval,
  events,
  latestCompactionStatus,
  latestUsageAudit,
  messages,
  onCreateCompaction,
  compactionBusy,
  session,
}: {
  assistantCode?: string;
  refreshInterval?: number;
  events: TranscriptAguiEvent[];
  goal?: ChatSessionGoalView | null;
  latestCompaction?: ChatSessionCompaction | null;
  latestCompactionStatus?: QueryStatus;
  latestRequestEstimate?: ChatTokenEstimateSnapshot | null;
  latestUsageAudit?: ChatUsageAuditSnapshot | null;
  messages: ChatMessage[];
  model?: ChatModelOption | null;
  onCreateCompaction?: () => void;
  compactionBusy?: boolean;
  selectedModel?: ChatModelOption | null;
  session?: ChatSession;
  todos?: ChatSessionTodo[];
}) {
  const { t } = useTranslation("pages");
  const blockCatalog = useContextBlockCatalog();
  const usage = chatTokenUsage(messages);
  const usageAudit = latestUsageAudit ?? latestUsageAuditFromEvents(events);
  const liveAssembly = useMemo(() => latestContextAssemblyFromEvents(events), [events]);
  const assembly = useSavedContextAssembly(session?.sessionCode ?? "", liveAssembly.snapshot?.snapshotSequence ? liveAssembly.snapshot : null, assistantCode, refreshInterval);
  const modelLabel = useContextModelLabel(assembly.snapshot?.modelCode ?? null, session?.workspaceCode ?? "");
  const budgetBreakdown = assembly.snapshot ? normalizeContextBudgetBreakdown({
    ...assembly.snapshot.budgetBreakdown,
    estimatedInputTokens: assembly.snapshot.estimatedInputTokens ?? undefined,
    effectiveInputBudget: assembly.snapshot.inputCapacity ?? undefined,
  }) : null;
  const runtimeNotice = latestShortTermMemoryRuntimeNotice(events);
  const contextBlocks = useMemo(() => assembly.snapshot ? buildReportedContextMemoryBlocks(assembly.snapshot, (key, options) => t(key, options), blockCatalog) : [], [assembly.snapshot,t,blockCatalog]);
  return (
    <section className="space-y-3 text-sm">
      <ProviderUsagePanel events={events} fallbackSessionUsage={usage} usageAudit={usageAudit} />
      <div className="space-y-2 rounded-md border border-[var(--color-outline-variant)] bg-[var(--color-surface-container-lowest)] p-3" data-testid="context-short-term-memory-card">
        <ContextOverviewPanel
          breakdown={budgetBreakdown}
          compactionBusy={compactionBusy || latestCompactionStatus?.loading}
          snapshot={assembly.snapshot}
          modelLabel={modelLabel}
          onCreateCompaction={onCreateCompaction}
          session={session}
        />
        <div className="pt-2">
          {runtimeNotice ? <ContextRuntimeNoticePanel notice={runtimeNotice} /> : null}
          <details>
            <summary className="cursor-pointer text-xs font-medium text-[var(--color-on-surface-variant)]">{t("chat.shortTermMemory.contextPanel.structureDetails")}</summary>
          {assembly.loading ? <p role="status">{t("chat.shortTermMemory.contextPanel.loadingReport")}</p> : assembly.error ? <div role="alert"><p>{assembly.error.message}</p><button type="button" onClick={() => void assembly.retry()}>{t("chat.shortTermMemory.retry")}</button></div> : assembly.snapshot ? <ContextStructurePanel blocks={contextBlocks} assistantCode={assistantCode} sessionCode={session?.sessionCode ?? ""} /> : <p>{t("chat.shortTermMemory.contextPanel.noReport")}</p>}
          </details>
        </div>
      </div>
      {latestCompactionStatus?.error ? (
        <QueryStatusLine
          errorText={t("chat.shortTermMemory.loadFailed")}
          loadingText={t("chat.shortTermMemory.loadingCheckpoint")}
          retryText={t("chat.shortTermMemory.retry")}
          status={latestCompactionStatus}
        />
      ) : null}
    </section>
  );
}

function ChatSessionWorkspacePanel({
  active,
  snapshot,
  onCancelGoal,
  onClearGoal,
  onCreateGoal,
  onPauseGoal,
  onResumeGoal,
  onUpdateGoal,
}: {
  active: boolean;
  snapshot: ChatRightPanelSnapshot;
  onCancelGoal: (input: ChatSessionGoalCommand) => Promise<boolean> | boolean;
  onClearGoal: (input: ChatSessionGoalCommand) => Promise<boolean> | boolean;
  onCreateGoal: (input: SetChatSessionGoalInput) => Promise<boolean> | boolean;
  onPauseGoal: (input: ChatSessionGoalCommand) => Promise<boolean> | boolean;
  onResumeGoal: (input: ChatSessionGoalCommand) => Promise<boolean> | boolean;
  onUpdateGoal: (input: UpdateChatSessionGoalInput) => Promise<boolean> | boolean;
}) {
  const sessionCode = snapshot.session.sessionCode;
  return <div className="absolute inset-0 min-h-0 overflow-y-auto overscroll-y-contain" data-session-code={sessionCode}><div className="m-4"><SessionApplicationsPanel key={sessionCode} session={snapshot.session} active={active} /><RightWorkspaceSidebar goal={snapshot.goal} goalBusy={snapshot.goalBusy} goalStatus={snapshot.goalStatus} onCancelGoal={onCancelGoal} onClearGoal={onClearGoal} onCreateGoal={onCreateGoal} onPauseGoal={onPauseGoal} onResumeGoal={onResumeGoal} onUpdateGoal={onUpdateGoal} sessionCode={sessionCode} todos={snapshot.todos} todosStatus={snapshot.todosStatus} /></div></div>;
}

function ChatSessionContextPanel({ snapshot, onCreateCompaction }: { snapshot: ChatRightPanelSnapshot; onCreateCompaction: () => Promise<void> | void }) {
  return <div className="absolute inset-0 min-h-0 overflow-y-auto overscroll-y-contain"><div className="m-4"><RightContextSidebar events={snapshot.contextEvents} goal={snapshot.goal} latestCompaction={snapshot.latestCompaction} latestCompactionStatus={snapshot.latestCompactionStatus} latestRequestEstimate={snapshot.latestRequestEstimate} latestUsageAudit={snapshot.latestUsageAudit} messages={snapshot.messages} model={snapshot.model} compactionBusy={snapshot.compactionBusy} onCreateCompaction={onCreateCompaction} selectedModel={snapshot.selectedModel} session={snapshot.session} todos={snapshot.todos} /></div></div>;
}

function ContextBlock({
  actions,
  children,
  className,
  title,
}: {
  actions?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  title: React.ReactNode;
}) {
  return (
    <section className={cn("space-y-2 border-b border-[var(--color-outline-variant)] pb-4 last:border-b-0", className)}>
      <div className="flex min-w-0 items-center justify-between gap-[12px]">
        <h3 className="truncate text-xs font-semibold uppercase text-[var(--color-on-surface-variant)]">{title}</h3>
        {actions ? <div className="shrink-0">{actions}</div> : null}
      </div>
      {children}
    </section>
  );
}


function CopyValueButton({
  ariaLabel,
  title,
  value,
}: {
  ariaLabel: string;
  title: string;
  value: string;
}) {
  const [copied, setCopied] = useState(false);
  const resetTimerRef = useRef<number | null>(null);

  useEffect(() => () => {
    if (resetTimerRef.current !== null) window.clearTimeout(resetTimerRef.current);
  }, []);

  async function handleCopy() {
    if (!value.trim()) return;
    try {
      await copyPlainText(value);
      setCopied(true);
      if (resetTimerRef.current !== null) window.clearTimeout(resetTimerRef.current);
      resetTimerRef.current = window.setTimeout(() => setCopied(false), 1400);
    } catch {
      setCopied(false);
    }
  }

  return (
    <button
      aria-label={ariaLabel}
      className="inline-flex size-6 items-center justify-center rounded-md text-[var(--color-on-surface-variant)] transition hover:bg-[var(--color-hover-surface)] hover:text-[var(--color-on-surface)] disabled:pointer-events-none disabled:opacity-40"
      disabled={!value.trim()}
      title={title}
      type="button"
      onClick={() => void handleCopy()}
    >
      {copied ? <CheckCircle2 size={14} /> : <Copy size={14} />}
    </button>
  );
}

function ChatAgentSelector({
  locked,
  loading,
  agents,
  onSelect,
  selectedAgentCode,
}: {
  locked?: boolean;
  loading: boolean;
  agents: ChatAgentOption[];
  onSelect: (agentCode: string) => void;
  selectedAgentCode: string;
}) {
  const [open, setOpen] = useState(false);
  const { t } = useTranslation("pages");
  const selectedAgent = agents.find((agent) => agent.agentCode === selectedAgentCode) ?? null;
  const disabled = loading || agents.length === 0 || Boolean(locked);

  return (
    <div className="relative min-w-0">
      <DropdownMenu open={open} onOpenChange={setOpen}>
        <DropdownMenuTrigger
          aria-label={t("chat.management.agentSelector.selectAria")}
          className="inline-flex h-8 w-full min-w-0 items-center gap-[12px] rounded-md border-0 bg-transparent px-2 text-xs text-[var(--color-on-surface-variant)] shadow-none transition hover:bg-[var(--color-hover-surface)] hover:text-[var(--color-on-surface)] disabled:pointer-events-none disabled:opacity-50"
          disabled={disabled}
          title={locked ? t("chat.management.agentSelector.locked") : undefined}
        >
          <Bot aria-hidden="true" className="size-3.5 shrink-0" />
          <span className="truncate">{loading ? t("chat.management.agentSelector.loading") : selectedAgent?.name ?? t("chat.management.agentSelector.mainAgent")}</span>
          <ChevronDown aria-hidden="true" size={14} />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="max-h-72 min-w-[180px] max-w-[260px] overflow-y-auto p-1.5" side="top" sideOffset={8}>
          {agents.map((agent) => (
            <DropdownMenuItem
              className={cn(
                "w-full flex-col items-start px-3 py-2 pr-8",
                agent.agentCode === selectedAgentCode && "bg-[var(--color-secondary-container)] text-[var(--color-on-secondary-container)]",
              )}
              key={agent.agentCode}
              onClick={() => {
                onSelect(agent.agentCode);
              }}
            >
              {agent.agentCode === selectedAgentCode ? <Check aria-hidden="true" className="absolute right-2 top-2 size-4" /> : null}
              <span className="block min-w-0 truncate font-medium">{agent.name}</span>
              <span className="mt-0.5 block min-w-0 truncate text-[11px] text-[var(--color-on-surface-variant)]">
                {agent.agentCode}
              </span>
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}

export function ChatManagementPage() {
  const scopeTabId = useTabPanelScopeId();
  const tabVisible = useTabVisibility();
  const location = useLocation();
  const [taskExecutionLaunch, setTaskExecutionLaunch] = useState<TaskExecutionResult | null>(() => {
    const state = location.state as { taskExecution?: TaskExecutionResult } | null;
    return state?.taskExecution ?? null;
  });
  useEffect(() => {
    const state = location.state as { taskExecution?: TaskExecutionResult } | null;
    const launch = state?.taskExecution;
    if (launch?.prompt && launch.sessionCode && launch.runCode) {
      setTaskExecutionLaunch((current) => current?.runCode === launch.runCode ? current : launch);
    }
  }, [location.state]);
  const [initialChatMessage] = useState(() => useTabStore.getState().tabs.find((tab) => tab.id === scopeTabId)?.initialChatMessage);
  const initialMessageSubmitted = useRef(false);
  const taskExecutionSubmittedRef = useRef("");
  const taskExecutionDispatchRef = useRef("");
  const contextPanelOpen = useTabStore((state) => state.tabs.find((tab) => tab.id === scopeTabId)?.rightPanelOpen ?? false);
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const routerSearchText = searchParams.toString();
  const [currentSearchText, setCurrentSearchText] = useState(routerSearchText);
  const currentSearchParams = useMemo(() => new URLSearchParams(currentSearchText), [currentSearchText]);
  const requestedSessionCode = currentSearchParams.get("sessionCode")?.trim() ?? "";
  const requestedDraftCode = currentSearchParams.get("draftCode")?.trim() ?? "";
  const draftProjectCode = currentSearchParams.get("projectCode")?.trim() || undefined;
  const draftDirectoryCode = currentSearchParams.get("directoryCode")?.trim() || undefined;
  const isDraft = Boolean(requestedDraftCode && !requestedSessionCode);
  const [draftDirectories, setDraftDirectories] = useState<Record<string, string | null>>({});
  const draftDirectorySelection = draftDirectories[requestedDraftCode];
  const draftComposerKey = useRef(requestedDraftCode);
  const [draftResources] = useState(() => new ChatDraftResources());
  const requestedSourceTab = parseChatSessionSourceTab(currentSearchParams.get("sourceTab"));
  const toast = useToast();
  const { agentReplyEffect, agentReplySpeed } = useTheme();
  const openRightSidebar = useOpenRightSidebar();
  const { i18n, t } = useTranslation("pages");
  const userInputLabels = useMemo<ChatUserInputLabels>(() => ({
    ariaLabel: t("chat.userInput.ariaLabel"),
    progress: (current, total) => t("chat.userInput.progress", { current, total }),
    back: t("chat.userInput.back"),
    next: t("chat.userInput.next"),
    continue: t("chat.userInput.continue"),
    approve: t("chat.userInput.approve"),
    approveAll: t("chat.userInput.approveAll"),
    rejectAll: t("chat.userInput.rejectAll"),
    approvalTitle: t("chat.userInput.approvalTitle"),
    approvalQuestion: (action: string) => t("chat.userInput.approvalQuestion", { action }),
    approvalFallbackQuestion: t("chat.userInput.approvalFallbackQuestion"),
    approvalTool: t("chat.userInput.approvalTool"),
    approvalExecution: t("chat.userInput.approvalExecution"),
    approvalReason: t("chat.userInput.approvalReason"),
    approvalDirectory: t("chat.userInput.approvalDirectory"),
    approvalParameters: t("chat.userInput.approvalParameters"),
    approvalMissingDetails: t("chat.userInput.approvalMissingDetails"),
    approvalScope: t("chat.userInput.approvalScope"),
    approvalBatch: t("chat.userInput.approvalBatch"),
    approvalBatchHint: t("chat.userInput.approvalBatchHint"),
    approvalWaiting: t("chat.userInput.approvalWaiting"),
    approvalRetry: t("chat.userInput.approvalRetry"),
    approvalChange: t("chat.userInput.approvalChange"),
    responseSubmitted: t("chat.userInput.responseSubmitted"),
    approved: t("chat.userInput.approved"),
    rejected: t("chat.userInput.rejected"),
    reject: t("chat.userInput.reject"),
    continuing: t("chat.userInput.continuing"),
    otherOptionLabel: t("chat.userInput.otherOptionLabel"),
    otherOptionDescription: t("chat.userInput.otherOptionDescription"),
    skipOptionLabel: t("chat.userInput.skipOptionLabel"),
    skipOptionDescription: t("chat.userInput.skipOptionDescription"),
    otherAnswerPlaceholder: t("chat.userInput.otherAnswerPlaceholder"),
    otherAnswerAriaLabel: (index) => t("chat.userInput.otherAnswerAriaLabel", { index }),
  }), [i18n.language, t]);
  const workRailLabels = useChatWorkRailLabels();
  const historyLabels = useChatHistoryLabels();
  const { session: authSession } = useAuthSession();
  const { preferences: aguiWorkReplayPreferences } = useAguiWorkReplayPreferences();
  const { selectedPrimarySpace, selectedSecondarySpace } = useWorkspaceSpaces();
  const activeWorkspaceCode = (selectedSecondarySpace ?? selectedPrimarySpace)?.workspaceCode ?? "";
  useCreateChatTab(activeWorkspaceCode,draftProjectCode,draftDirectoryCode);
  const promoteDraftSession = useStableCallback((session: ChatSession) => {
    reassignRightPanelOwner(session.sessionCode);
    setOptimisticActiveSession(session);
    setSessionGroups((current) => insertSession(current, session));
    activeSessionCodeRef.current = session.sessionCode;
    setActiveSessionCode(session.sessionCode);
    if (selectedModelCode) {
      setSessionModelSelections((current) => ({
        ...current,
        [sessionModelSelectionKey(session.sessionCode, session.agentCode ?? selectedAgentCode)]: selectedModelCode,
      }));
    }
    const sessionParams = new URLSearchParams({ sessionCode: session.sessionCode, workspaceCode: activeWorkspaceCode });
    const path = `/chats?${sessionParams}`;
    useTabStore.getState().updateTab(scopeTabId, {
      path, activeNav: `chat-session:${session.sessionCode}`,
      title: session.title, titleTranslationKey: undefined,
    });
    setCurrentSearchText(sessionParams.toString());
    if (useTabStore.getState().activeTabId === scopeTabId) navigate(path, { replace: true });
    window.dispatchEvent(new CustomEvent("tron:chat-session-created", { detail: session }));
    if (session.projectCode) {
      window.dispatchEvent(new CustomEvent("tron:topic-location-changed", { detail: { oldProjectCode: null, projectCode: session.projectCode } }));
    }
  });
  const createRuntimeSession = useStableCallback((input: Parameters<typeof createChatSession>[0]) => (
    draftResources.createSession(async () => {
      setCreatingSession(true);
      try {
        // Localized composer labels are not user-assigned session titles.
        const session = await createChatSession({ ...input, title: undefined, projectCode: draftProjectCode, directoryCode: draftDirectoryCode, workingDirectoryCode: draftDirectoryCode ?? draftDirectorySelection, executionMode });
        promoteDraftSession(session);
        return session;
      } finally {
        setCreatingSession(false);
      }
    })
  ));
  const chatTransportAdapter = useMemo(
    () => getServiceDataSource("chat") === "mock"
      ? createTronMockChatAdapter()
      : createInternalChatAdapter({
          workspaceCode: activeWorkspaceCode,
          getAccessToken: getApiAccessToken,
          getHeaders: () => applyCurrentLocaleHeader(new Headers()),
        }),
    [activeWorkspaceCode],
  );
  const chatRuntimeAdapter = useMemo(() => draftResources.adapt({
    ...chatTransportAdapter, createSession: createRuntimeSession,
  }), [chatTransportAdapter, createRuntimeSession, draftResources]);
  const [activeChatDataRefreshKey, setActiveChatDataRefreshKey] = useState(0);
  const [activeChatReplayRefreshKey, setActiveChatReplayRefreshKey] = useState(0);
  const [shortTermMemoryRefreshKey, setShortTermMemoryRefreshKey] = useState(0);
  const [goalPollKey, setGoalPollKey] = useState(0);
  const groups = useChatSessionGroupList(activeWorkspaceCode, activeChatDataRefreshKey);
  const [sessionGroups, setSessionGroups] = useState<ChatSessionGroup[]>(() => orderSessionGroups(groups));
  const [activeSessionCode, setActiveSessionCode] = useState("");
  const [composerDraftsBySessionCode, setComposerDraftsBySessionCode] = useState<Record<string, string>>(() => (
    initialChatMessage && requestedDraftCode ? { [requestedDraftCode]: initialChatMessage } : {}
  ));
  const [optimisticActiveSession, setOptimisticActiveSession] = useState<ChatSession | null>(null);
  useEffect(() => {
    const onUpdated = (event: Event) => {
      const updated = (event as CustomEvent<{ session?: ChatSession }>).detail?.session;
      if (!updated || updated.workspaceCode !== activeWorkspaceCode) return;
      setSessionGroups((current) => replaceSession(current, updated));
      setOptimisticActiveSession((current) => current?.sessionCode === updated.sessionCode ? updated : current);
      setActiveChatDataRefreshKey((current) => current + 1);
    };
    window.addEventListener("tron:chat-session-updated", onUpdated);
    return () => window.removeEventListener("tron:chat-session-updated", onUpdated);
  }, [activeWorkspaceCode]);
  const requestedSession = requestedSessionCode ? findSession(sessionGroups, requestedSessionCode) : undefined;
  const optimisticSession = optimisticActiveSession
    && optimisticActiveSession.sessionCode === (requestedSessionCode || activeSessionCode)
    ? optimisticActiveSession
    : undefined;
  const resolvedActiveSessionCode = requestedSession?.sessionCode ?? optimisticSession?.sessionCode ?? activeSessionCode;
  const selectedSession = isDraft ? undefined : requestedSession
    ?? optimisticSession
    ?? (!requestedSessionCode && resolvedActiveSessionCode ? findSession(sessionGroups, resolvedActiveSessionCode) : undefined);
  const activeSession = selectedSession ?? (
    !isDraft && !requestedSessionCode && !requestedSourceTab && !activeSessionCode
      ? firstSessionForSourceTab(sessionGroups, defaultChatSessionSourceTab)
      : undefined
  );
  const [runtimeRunState, setRuntimeRunState] = useState<ChatRuntimeRunState | null>(null);
  // A plan-mode turn that finished is waiting for the user to confirm its plan.
  const [planReview, setPlanReview] = useState<{ sessionCode: string } | null>(null);
  const [planReviewApproving, setPlanReviewApproving] = useState(false);
  const planRunRef = useRef<{ sessionId: string; planned: boolean } | null>(null);
  const { mode: executionMode, saving: savingExecutionMode, changeMode: changeExecutionMode, applyMode: applyExecutionMode } = useChatExecutionMode(
    activeSession,
    isDraft ? requestedDraftCode : "",
    useStableCallback((updated: ChatSession) => {
      setSessionGroups((current) => replaceSession(current, updated));
      setOptimisticActiveSession((current) => current?.sessionCode === updated.sessionCode ? updated : current);
      setActiveChatDataRefreshKey((current) => current + 1);
      window.dispatchEvent(new Event("tron:chat-session-updated"));
    }),
  );
  const currentSessionRunActive = Boolean(runtimeRunState?.active && runtimeRunState.sessionCode === activeSession?.sessionCode);
  // The run captures its tool policy when it starts, so remember whether this
  // turn began in plan mode before offering its plan for review.
  const executionModeRef = useRef(executionMode);
  executionModeRef.current = executionMode;
  useEffect(() => {
    setPlanReview(null);
    planRunRef.current = null;
  }, [activeSession?.sessionCode]);
  useChatGeneratedTitle(activeSession, useStableCallback((updated: ChatSession) => {
    window.dispatchEvent(new CustomEvent("tron:chat-session-updated", {
      detail: { session: updated },
    }));
  }));
  const shortTermMemoryTodosState = useChatSessionTodosState(activeSession?.sessionCode ?? "", shortTermMemoryRefreshKey);
  const latestCompactionState = useChatLatestCompactionState(activeSession?.sessionCode ?? "", shortTermMemoryRefreshKey);
  const activeGoalState = useChatSessionLatestGoalState(activeSession?.sessionCode ?? "", shortTermMemoryRefreshKey + goalPollKey);
  const shortTermMemoryTodos = shortTermMemoryTodosState.data;
  const latestCompaction = latestCompactionState.data;
  const activeGoal = activeGoalState.data;
  const shortTermMemoryTodosStatus = useMemo<QueryStatus>(() => ({
    loading: shortTermMemoryTodosState.loading,
    error: shortTermMemoryTodosState.error,
    retry: shortTermMemoryTodosState.retry,
  }), [shortTermMemoryTodosState.error, shortTermMemoryTodosState.loading, shortTermMemoryTodosState.retry]);
  const latestCompactionStatus = useMemo<QueryStatus>(() => ({
    loading: latestCompactionState.loading,
    error: latestCompactionState.error,
    retry: latestCompactionState.retry,
  }), [latestCompactionState.error, latestCompactionState.loading, latestCompactionState.retry]);
  const goalStatus = useMemo<QueryStatus>(() => ({
    loading: activeGoalState.loading,
    error: activeGoalState.error,
    retry: activeGoalState.retry,
  }), [activeGoalState.error, activeGoalState.loading, activeGoalState.retry]);
  // Context inspection is an explicit, separate read from ordinary public history.
  const contextInspectionSessionCode = contextPanelOpen ? activeSession?.sessionCode ?? "" : "";
  const liveContextRefreshKey = useChatContextRefreshInterval(contextInspectionSessionCode, currentSessionRunActive);
  const aguiEvents = useChatAguiEvents(contextInspectionSessionCode, activeChatDataRefreshKey + shortTermMemoryRefreshKey + liveContextRefreshKey);
  const aguiProtocolEvents = useChatAguiProtocolEvents(contextInspectionSessionCode, activeChatReplayRefreshKey + shortTermMemoryRefreshKey + liveContextRefreshKey);
  const visibleMessages = useChatMessageListBySession(contextInspectionSessionCode, activeChatDataRefreshKey, { suppressErrorToast: true });
  const [creatingSession, setCreatingSession] = useState(false);
  const [chatModels, setChatModels] = useState<ModelSelectOption[]>([]);
  const [selectedModelCode, setSelectedModelCode] = useState("");
  const [selectedReasoningEffort, setSelectedReasoningEffort] = useState<ReasoningEffort | null>(null);
  const [sessionModelSelections, setSessionModelSelections] = useState<Record<string, string>>(() => readChatSessionModelSelections());
  const [modelsLoading, setModelsLoading] = useState(false);
  const [chatAgents, setChatAgents] = useState<ChatAgentOption[]>([]);
  const [selectedAgentCode, setSelectedAgentCode] = useState(mainChatAgentOption.agentCode);
  const agentSelectionSessionRef = useRef("");
  const [agentsLoading, setAgentsLoading] = useState(false);
  const compactingSessionCode = runtimeRunState?.active && runtimeRunState.operationKind === "ContextCompaction" ? runtimeRunState.sessionCode : "";
  const [goalActionSessionCode, setGoalActionSessionCode] = useState("");
  const [pendingAttachmentsBySessionCode, setPendingAttachmentsBySessionCode] = useState<
    Record<string, ChatMessageAttachment[]>
  >({});
  const [attachmentUploadPending, setAttachmentUploadPending] = useState(false);
  const [rightPanelSnapshots, setRightPanelSnapshots] = useState<
    Record<string, ChatRightPanelSnapshot>
  >({});
  const desktopBridge = useMemo(() => new DesktopBridge(), []);

  const composerSessionCode = draftComposerKey.current || activeSession?.sessionCode || "";
  const composer = composerSessionCode
    ? composerDraftsBySessionCode[composerSessionCode] ?? ""
    : "";
  const pendingAttachments = composerSessionCode
    ? pendingAttachmentsBySessionCode[composerSessionCode] ?? []
    : [];
  const setComposer = useCallback(
    (value: string | ((current: string) => string)) => {
      if (!composerSessionCode) return;
      setComposerDraftsBySessionCode((current) => {
        const previous = current[composerSessionCode] ?? "";
        const nextValue = typeof value === "function" ? value(previous) : value;
        if (nextValue === previous) return current;
        return { ...current, [composerSessionCode]: nextValue };
      });
    },
    [composerSessionCode],
  );
  const setPendingAttachments = useCallback(
    (
      value: ChatMessageAttachment[] | ((current: ChatMessageAttachment[]) => ChatMessageAttachment[]),
    ) => {
      if (!composerSessionCode) return;
      setPendingAttachmentsBySessionCode((current) => {
        const previous = current[composerSessionCode] ?? [];
        const nextValue = typeof value === "function" ? value(previous) : value;
        if (nextValue === previous) return current;
        return { ...current, [composerSessionCode]: nextValue };
      });
    },
    [composerSessionCode],
  );

  const { openTab, reassignOwner: reassignRightPanelOwner } = useRightPanelTabs(activeSession?.sessionCode);

  const openMarkdownInternalLink = useCallback((url: string) => {
    const normalizedUrl = url.trim();
    if (!normalizedUrl) return;
    openTab({
      key: `markdown-page:${normalizedUrl}`,
      stableId: `markdown-page:${normalizedUrl}`,
      label: normalizedUrl,
      url: normalizedUrl,
      children: null,
    });
  }, [openTab]);
  const openMarkdownExternalLink = useCallback((url: string) => {
    if (!desktopBridge.isDesktop()) {
      window.open(url, "_blank", "noopener,noreferrer");
      return;
    }

    // 桌面端：走 IPC 让主进程打开系统浏览器
    // fire-and-forget：主进程立即返回，不等浏览器打开，
    // 避免渲染进程后续处理抢回焦点导致浏览器窗口层级变低
    void desktopBridge.openTargetPage({ mode: "browser", url }).catch(() => {
      toast.error(t("chat.markdownLink.openExternalFailed"));
    });
  }, [desktopBridge, toast, t]);

  const activeSessionCodeRef = useRef("");
  const modelSelectionSessionRef = useRef("");
  const requestedSessionCodeSyncedRef = useRef("");
  const composerRef = useRef<HTMLTextAreaElement | null>(null);
  const submitterRef = useRef<AgentChatSubmitter | null>(null);
  const attachmentFileInputRef = useRef<HTMLInputElement | null>(null);
  const selectedAgent = chatAgents.find((agent) => agent.agentCode === selectedAgentCode) ?? null;
  const agentScopedModels = useMemo(
    () => modelsForAgent(chatModels, selectedAgent, t(autoModelRoutes.TEXT.labelKey)),
    [chatModels, selectedAgent, t],
  );
  const selectedModel = agentScopedModels.find((model) => model.optionCode === selectedModelCode) ?? null;
  const selectedModelAgentCode = inferChatAgentCodeFromModel(selectedModel, selectedAgentCode || mainChatAgentOption.agentCode);
  const effectiveAgentCode = selectedModelAgentCode || selectedAgentCode || mainChatAgentOption.agentCode;
  const selectedMainAgentModel = selectedModel?.source === "internal-agent" ? selectedModel : null;
  const selectedReasoningEffortOptions = useMemo(() => {
    const supported = new Set(selectedMainAgentModel?.supportedReasoningEfforts ?? []);
    return reasoningEffortOptions
      .filter((option) => supported.has(option.value))
      .map((option) => ({
        ...option,
        label: t(`common:reasoningEffort.values.${option.value}`, option.label),
      }));
  }, [selectedMainAgentModel?.supportedReasoningEfforts, t]);
  const selectedModelDefaultReasoningEffort = selectedMainAgentModel?.defaultReasoningEffort ?? null;
  const hasActiveSession = Boolean(activeSession);
  const sessionStarted = Boolean(activeSession?.lastRunCode);
  const agentSelectionLocked = Boolean(activeSession && sessionStarted);
  const modelEmptyLabel = isMainChatAgent(effectiveAgentCode)
    ? t("chat.management.modelEmpty.noTextModel")
    : t("chat.management.modelEmpty.noneForAgent", { name: selectedAgent?.name ?? effectiveAgentCode });
  const currentUserDisplayName = authSession?.user.displayName || authSession?.user.username || "";
  const { createInspectionAttachmentUpload } = useInspection();

  function createSendInput(
    session: ChatSession,
    content: string,
    attachments: ChatMessageAttachment[],
  ): SendChatMessageInput | null {
    const taskLaunch = taskExecutionDispatchRef.current === session.sessionCode
      && taskExecutionLaunch?.sessionCode === session.sessionCode
      ? taskExecutionLaunch
      : null;
    if (taskLaunch) taskExecutionDispatchRef.current = "";
    if (savingExecutionMode || (!taskLaunch && (!selectedModel || !selectedModel.enabled))) return null;
    const input: SendChatMessageInput = {
      agentCode: taskLaunch?.agentCode ?? effectiveAgentCode,
      appCode: session.appCode,
      clientCapabilities: { userInputRequests: true },
      content,
      taskCode: taskLaunch?.taskCode ?? session.taskCode,
      workspaceCode: session.workspaceCode,
    };
    if (taskLaunch) {
      input.runCode = taskLaunch.runCode;
      input.taskExecution = true;
      if (taskLaunch.modelCode) {
        input.modelCode = taskLaunch.modelCode;
        input.systemModelId = taskLaunch.modelCode;
      }
    }
    if (attachments.length) input.attachments = attachments;
    if (!taskLaunch?.modelCode && selectedModel?.source === "system-agent") {
      input.systemModelId = selectedModel.modelName;
    } else if (!taskLaunch?.modelCode && selectedModel) {
      // The selector always sends the complete selection. In particular Auto
      // intentionally does not include a physical modelCode.
      input.modelSelection = selectedModel.modelSelection;
      input.reasoningEffort = selectedReasoningEffort;
    }
    return input;
  }

  function toPageChatSession(session: RuntimeChatSession): ChatSession {
    return {
      ...session,
      sourceType: session.sourceType ?? undefined,
    };
  }

  // `/plan` mirrors the composer command menu. A bare `/plan` toggles plan mode;
  // `/plan <description>` switches plan mode on before the description is sent,
  // so the run is admitted with the mode already stored on the session.
  const runPlanCommand = useStableCallback(async (session: RuntimeChatSession, intent: "toggle" | "ensure") => {
    const current = session.executionMode ?? executionMode;
    const next: ToolExecutionMode = intent === "ensure" ? "PLAN" : current === "PLAN" ? "CONTROLLED" : "PLAN";
    if (!(await applyExecutionMode(session, next))) return false;
    if (next !== current) {
      toast.info(next === "PLAN"
        ? t("chat.management.composer.planModeEntered")
        : t("chat.management.composer.planModeExited"));
    }
    return true;
  });

  const createRuntimeSendInput = useCallback(
    (session: RuntimeChatSession, content: string, attachments: ChatMessageAttachment[]) => (
      createSendInput(toPageChatSession(session), content, attachments)
    ),
    [effectiveAgentCode, selectedModel, selectedReasoningEffort, savingExecutionMode, taskExecutionLaunch],
  );

  useEffect(() => {
    if (!initialChatMessage || initialMessageSubmitted.current || !isDraft || !tabVisible) return;
    if (composer !== initialChatMessage) {
      initialMessageSubmitted.current = true;
      useTabStore.getState().updateTab(scopeTabId, { initialChatMessage: undefined });
      return;
    }
    if (!selectedModel?.enabled || savingExecutionMode || creatingSession || !submitterRef.current) return;
    // Claim before submitting: StrictMode and model updates must not send twice.
    initialMessageSubmitted.current = true;
    useTabStore.getState().updateTab(scopeTabId, { initialChatMessage: undefined });
    setComposer("");
    const restoreDraft = () => setComposer((current) => current || initialChatMessage);
    void submitterRef.current.submit({ content: initialChatMessage }).then((accepted) => {
      if (!accepted) restoreDraft();
    }).catch((error: unknown) => {
      restoreDraft();
      toast.error(error instanceof Error ? error.message : String(error));
    });
  }, [initialChatMessage, isDraft, tabVisible, composer, selectedModel, savingExecutionMode, creatingSession, scopeTabId, setComposer, toast]);

  useEffect(() => {
    const launch = taskExecutionLaunch;
    if (!launch?.prompt || activeSession?.sessionCode !== launch.sessionCode || !tabVisible
      || creatingSession || savingExecutionMode || !submitterRef.current
      || taskExecutionSubmittedRef.current === launch.runCode) return;

    // A task launch is submitted through Agent UI's normal stream adapter so
    // assistant text and AG-UI work events appear in the task session live.
    taskExecutionSubmittedRef.current = launch.runCode;
    taskExecutionDispatchRef.current = launch.sessionCode;
    navigate(`${location.pathname}${location.search}`, { replace: true, state: null });
    const restoreDraft = () => setComposer((current) => current || launch.prompt!);
    void submitterRef.current.submit({ content: launch.prompt }).then((accepted) => {
      if (!accepted) {
        taskExecutionDispatchRef.current = "";
        restoreDraft();
        toast.error(t("chat.management.toasts.chatSendFailedTitle"));
      }
    }).catch((error: unknown) => {
      taskExecutionDispatchRef.current = "";
      restoreDraft();
      toast.error(error instanceof Error ? error.message : String(error));
    });
  }, [activeSession?.sessionCode, creatingSession, location.pathname, location.search, navigate, savingExecutionMode, setComposer, tabVisible, taskExecutionLaunch, toast, t]);

  const handleRuntimeSessionChange = useCallback((nextSession: RuntimeChatSession | null) => {
    if (!nextSession) return;
    if (nextSession) {
      setSessionGroups((current) => replaceSession(current, toPageChatSession(nextSession)));
      window.dispatchEvent(new Event("tron:chat-session-updated"));
    }
    setActiveChatDataRefreshKey((current) => current + 1);
    setActiveChatReplayRefreshKey((current) => current + 1);
  }, []);

  const sending = currentSessionRunActive;
  useEffect(() => {
    if (!currentSessionRunActive || !activeSession?.sessionCode) return;
    const timer = window.setInterval(() => setGoalPollKey((key) => key + 1), 3000);
    return () => window.clearInterval(timer);
  }, [activeSession?.sessionCode, currentSessionRunActive]);
  const handleRuntimeRunStateChange = useStableCallback((state: ChatRuntimeRunState) => {
    setRuntimeRunState(state);
    if (state.sessionCode) {
      setActiveChatDataRefreshKey((current) => current + 1);
      setShortTermMemoryRefreshKey((current) => current + 1);
    }
    if (state.active) {
      // Latch the mode on the first active event so a later mode change mid-run
      // neither invents nor hides the review.
      if (state.sessionCode && !planRunRef.current) {
        planRunRef.current = { sessionId: state.sessionCode, planned: executionModeRef.current === "PLAN" };
        setPlanReview(null);
      }
      return;
    }
    const run = planRunRef.current;
    if (!run || !state.sessionCode || run.sessionId !== state.sessionCode) return;
    planRunRef.current = null;
    if (run.planned) setPlanReview({ sessionCode: state.sessionCode });
  });

  const approvePlanReview = useStableCallback(async () => {
    const session = activeSession;
    if (!session || !planReview || planReview.sessionCode !== session.sessionCode || planReviewApproving) return;
    setPlanReviewApproving(true);
    try {
      if (!(await applyExecutionMode(session, "CONTROLLED"))) return;
      setPlanReview(null);
      const sent = await (submitterRef.current?.submit({ content: t("chat.management.planReview.followUp") }) ?? false);
      if (sent) {
        toast.success(t("chat.management.planReview.approved"));
        return;
      }
      // The next run reads the persisted mode, so restore plan mode before retrying.
      await applyExecutionMode({ sessionCode: session.sessionCode, executionMode: "CONTROLLED" }, "PLAN");
      setPlanReview({ sessionCode: session.sessionCode });
      toast.error(t("chat.management.planReview.approveFailed"));
    } finally {
      setPlanReviewApproving(false);
    }
  });

  const keepPlanning = useStableCallback(() => {
    setPlanReview(null);
    composerRef.current?.focus();
  });

  const planReviewVisible = Boolean(activeSession)
    && executionMode === "PLAN"
    && planReview?.sessionCode === activeSession?.sessionCode
    && !sending
    && !savingExecutionMode
    && activeSession?.lastRunStatus !== "AWAITING_USER_INPUT";

  async function uploadAttachmentForActiveSession(
    file: File,
    options: Omit<Parameters<typeof uploadChatSessionAttachment>[1], "file"> = {},
  ): Promise<ChatMessageAttachment | null> {
    const targetSession = activeSession;
    if (!targetSession && isDraft) return draftResources.stage({ ...options, file });
    if (!targetSession) {
      toast.warning(t("chat.management.toasts.attachNoSession"), { title: t("chat.management.toasts.sessionRequiredTitle") });
      return null;
    }

    setAttachmentUploadPending(true);
    try {
      const attachment = await uploadChatSessionAttachment(targetSession, {
        ...options,
        file,
      });
      return activeSessionCodeRef.current === targetSession.sessionCode
        ? attachment
        : null;
    } catch (error) {
      toast.error(error instanceof Error ? error.message : t("chat.management.toasts.attachmentUploadFailed"), {
        title: t("chat.management.toasts.attachmentUploadFailed"),
      });
      return null;
    } finally {
      setAttachmentUploadPending(false);
    }
  }

  async function handleAttachmentFile(file: File) {
    const attachment = await uploadAttachmentForActiveSession(file, {
      kind: file.type.startsWith("image/") ? "image" : "file",
      summary: file.name,
    });
    if (!attachment) return;
    setPendingAttachments((current) => [...current, attachment]);
  }

  async function handleAttachmentFileInputChange(
    event: ChangeEvent<HTMLInputElement>,
  ) {
    const file = event.currentTarget.files?.item(0);
    event.currentTarget.value = "";
    if (!file) return;
    await handleAttachmentFile(file);
  }

  async function handleInspectionConfirm(result: TargetPageFrameInspectionResult) {
    try {
      const inspection = await createInspectionAttachmentUpload(result);
      const attachment = await uploadAttachmentForActiveSession(inspection.file, {
        kind: inspection.kind,
        metadata: inspection.metadata,
        summary: inspection.summary,
      });
      if (!attachment) return;

      const content = inspectionMessageContent(result);
      if (inspectionSubmitMode(result) === "send" && selectedModel) {
        const accepted = await submitterRef.current?.submit({ content, attachments: [attachment] });
        if (accepted) return;
      }
      if (content) setComposer(content);
      setPendingAttachments((current) => [...current, attachment]);
      if (inspectionSubmitMode(result) === "send" && !selectedModel) {
        toast.warning(t("chat.management.toasts.selectModelFirst"), { title: t("chat.management.toasts.modelRequiredTitle") });
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : t("chat.management.toasts.annotationUploadFailed"), {
        title: t("chat.management.toasts.attachmentUploadFailed"),
      });
    }
  }

  const handleInspectionConfirmForRightPanel = useStableCallback(handleInspectionConfirm);

  const replaceChatSearchParams = useCallback((update: (next: URLSearchParams) => void) => {
    if (useTabStore.getState().activeTabId !== scopeTabId) return;
    const currentPathname = "/chats";
    const currentSearch = currentSearchText;
    const next = new URLSearchParams(currentSearch);
    update(next);
    const nextSearch = next.toString();
    if (nextSearch === currentSearch) {
      if (currentSearchText !== nextSearch) setCurrentSearchText(nextSearch);
      return;
    }
    setCurrentSearchText(nextSearch);
    navigate(`${currentPathname}${nextSearch ? `?${nextSearch}` : ""}`, { replace: true });
  }, [currentSearchText, navigate, scopeTabId]);

  useEffect(() => {
    setCurrentSearchText(routerSearchText);
  }, [routerSearchText]);

  function selectSourceTab(sourceTab: ChatSessionSourceTab) {
    writeStoredChatSessionSourceTab(sourceTab);
  }

  useEffect(() => {
    setSessionGroups(orderSessionGroups(groups));
  }, [groups]);

  useEffect(() => {
    const activeSession = requestedSessionCode
      ? findSession(sessionGroups, requestedSessionCode)
      : undefined;
    if (!activeSession?.title.trim()) return;
    const tab = useTabStore
      .getState()
      .getAllTabs().find((item) => item.activeNav === `chat-session:${activeSession.sessionCode}`);
    if (tab && (tab.title !== activeSession.title || tab.titleTranslationKey)) {
      useTabStore.getState().updateTab(tab.id, {
        title: activeSession.title,
        titleTranslationKey: undefined,
      });
    }
  }, [requestedSessionCode, sessionGroups]);

  useEffect(() => {
    const refresh = () => setActiveChatDataRefreshKey((current) => current + 1);
    window.addEventListener(CHAT_SESSIONS_REMOVED_EVENT, refresh);
    return () => window.removeEventListener(CHAT_SESSIONS_REMOVED_EVENT, refresh);
  }, []);


  useEffect(() => {
    if (!optimisticActiveSession) return;
    if (requestedSession?.sessionCode === optimisticActiveSession.sessionCode) {
      setOptimisticActiveSession(null);
    }
  }, [optimisticActiveSession, requestedSession?.sessionCode]);

  useEffect(() => {
    if (!requestedSessionCode) {
      requestedSessionCodeSyncedRef.current = "";
      return;
    }
    if (requestedSessionCodeSyncedRef.current === requestedSessionCode && activeSessionCode === requestedSessionCode) return;
    if (!findSession(sessionGroups, requestedSessionCode)) return;
    activeSessionCodeRef.current = requestedSessionCode;
    setActiveSessionCode(requestedSessionCode);
    const sourceTab = sourceTabForSessionCode(sessionGroups, requestedSessionCode);
    if (sourceTab) selectSourceTab(sourceTab);
    requestedSessionCodeSyncedRef.current = requestedSessionCode;
  }, [activeSessionCode, requestedSessionCode, sessionGroups]);

  useEffect(() => {
    if (isDraft || !requestedSourceTab) return;
    selectSourceTab(requestedSourceTab);
    if (!sessionGroups.length) return;

    if (requestedSessionCode) {
      const requestedSessionSourceTab = sourceTabForSessionCode(sessionGroups, requestedSessionCode);
      if (requestedSessionSourceTab && requestedSessionSourceTab !== requestedSourceTab) {
        replaceChatSearchParams((next) => {
          next.set("sourceTab", requestedSessionSourceTab);
        });
      }
      return;
    }

    const nextSession = firstSessionForSourceTab(sessionGroups, requestedSourceTab);
    if (nextSession) {
      activeSessionCodeRef.current = nextSession.sessionCode;
      setActiveSessionCode(nextSession.sessionCode);
      replaceChatSearchParams((next) => {
        next.set("sessionCode", nextSession.sessionCode);
        next.set("sourceTab", requestedSourceTab);
      });
      return;
    }

    if (!requestedSessionCode && !activeSessionCodeRef.current) return;
    activeSessionCodeRef.current = "";
    setActiveSessionCode("");
    replaceChatSearchParams((next) => {
      next.delete("sessionCode");
      next.set("sourceTab", requestedSourceTab);
    });
  }, [isDraft, replaceChatSearchParams, requestedSessionCode, requestedSourceTab, sessionGroups]);

  useEffect(() => {
    let active = true;

    async function loadModels() {
      if (!activeWorkspaceCode) {
        setChatModels([]);
        setSelectedModelCode("");
        return;
      }

      setModelsLoading(true);
      try {
        const options = await fetchModelSelectOptions(activeWorkspaceCode);
        if (!active) return;
        const models = availableChatModels(options);
        setChatModels(models);
      } catch (error) {
        if (active) {
          setChatModels([]);
          setSelectedModelCode("");
          toast.error(error instanceof Error ? error.message : t("chat.management.toasts.modelLoadFailed"), { title: t("chat.management.toasts.modelLoadFailedTitle") });
        }
      } finally {
        if (active) setModelsLoading(false);
      }
    }

    void loadModels();

    return () => {
      active = false;
    };
  }, [activeWorkspaceCode, toast]);

  useEffect(() => {
    let active = true;

    async function loadAgents() {
      if (!activeWorkspaceCode) {
        setChatAgents([]);
        setSelectedAgentCode(mainChatAgentOption.agentCode);
        return;
      }

      setAgentsLoading(true);
      try {
        const [internalRecords, systemRecords] = getServiceDataSource("agent") === "api"
          ? await Promise.all([
            fetchWorkspaceInternalAgents(activeWorkspaceCode),
            fetchWorkspaceSystemAgents(activeWorkspaceCode),
          ])
          : [
            listWorkspaceInternalAgentsMock(activeWorkspaceCode),
            listWorkspaceSystemAgentsMock(activeWorkspaceCode),
          ];
        if (!active) return;
        const agents = availableChatAgents(internalRecords, systemRecords);
        setChatAgents(agents);
        setSelectedAgentCode((current) => (
          current && agents.some((agent) => agent.agentCode === current)
            ? current
            : mainChatAgentOption.agentCode
        ));
      } catch (error) {
        if (active) {
          setChatAgents([]);
          setSelectedAgentCode(mainChatAgentOption.agentCode);
          toast.error(error instanceof Error ? error.message : t("chat.management.toasts.agentLoadFailed"), { title: t("chat.management.toasts.agentLoadFailedTitle") });
        }
      } finally {
        if (active) setAgentsLoading(false);
      }
    }

    const refreshAgents = () => { void loadAgents(); };
    refreshAgents();
    window.addEventListener(INTERNAL_AGENT_REGISTRY_CHANGED_EVENT, refreshAgents);

    return () => {
      active = false;
      window.removeEventListener(INTERNAL_AGENT_REGISTRY_CHANGED_EVENT, refreshAgents);
    };
  }, [activeWorkspaceCode, toast]);

  useEffect(() => {
    const sessionAgentCode = activeSession?.agentCode ?? mainChatAgentOption.agentCode;
    const identity = `${activeWorkspaceCode}:${composerSessionCode}:${sessionAgentCode}`;
    const shouldSync = agentSelectionSessionRef.current !== identity
      && chatAgents.some((agent) => agent.agentCode === sessionAgentCode);
    if (shouldSync) agentSelectionSessionRef.current = identity;
    setSelectedAgentCode((current) => shouldSync ? sessionAgentCode
      : current && chatAgents.some((agent) => agent.agentCode === current)
        ? current : mainChatAgentOption.agentCode);
  }, [activeWorkspaceCode, activeSession?.agentCode, composerSessionCode, chatAgents]);

  useEffect(() => {
    const sessionCode = composerSessionCode;
    const sessionAgentCode = (activeSession?.agentCode ?? selectedAgentCode ?? mainChatAgentOption.agentCode).toLowerCase();
    const selectionKey = sessionCode ? sessionModelSelectionKey(sessionCode, sessionAgentCode) : "";
    const switchedSession = modelSelectionSessionRef.current !== sessionCode;

    if (switchedSession) {
      modelSelectionSessionRef.current = sessionCode;
      const persistedOptionCode = selectionKey ? sessionModelSelections[selectionKey] : "";
      const defaultModel = defaultChatModelOption(agentScopedModels);
      const restoredOptionCode = persistedOptionCode && agentScopedModels.some((model) => model.optionCode === persistedOptionCode)
        ? persistedOptionCode
        : (defaultModel?.optionCode ?? "");
      setSelectedModelCode(restoredOptionCode);
      return;
    }

    setSelectedModelCode((current) => {
      if (current && agentScopedModels.some((model) => model.optionCode === current)) {
        return current;
      }
      const persistedOptionCode = selectionKey ? sessionModelSelections[selectionKey] : "";
      if (persistedOptionCode && agentScopedModels.some((model) => model.optionCode === persistedOptionCode)) {
        return persistedOptionCode;
      }
      return defaultChatModelOption(agentScopedModels)?.optionCode ?? "";
    });
  }, [activeSession?.agentCode, composerSessionCode, agentScopedModels, chatModels, selectedAgentCode, sessionModelSelections]);

  useEffect(() => {
    const sessionCode = activeSession?.sessionCode;
    if (!sessionCode || !selectedModelCode) return;
    const activeSessionAgentCode = activeSession.agentCode ?? mainChatAgentOption.agentCode;
    if (selectedAgentCode !== activeSessionAgentCode) return;
    if (!agentScopedModels.some((model) => model.optionCode === selectedModelCode)) return;
    const selectionKey = sessionModelSelectionKey(sessionCode, activeSessionAgentCode);
    setSessionModelSelections((current) => (
      current[selectionKey] === selectedModelCode
        ? current
        : {
          ...current,
          [selectionKey]: selectedModelCode,
        }
    ));
  }, [
    activeSession?.agentCode,
    activeSession?.sessionCode,
    agentScopedModels,
    selectedAgentCode,
    selectedModelCode,
  ]);

  useEffect(() => {
    writeChatSessionModelSelections(sessionModelSelections);
  }, [sessionModelSelections]);

  useEffect(() => {
    setSelectedReasoningEffort(null);
  }, [composerSessionCode, selectedModel?.optionCode, selectedMainAgentModel?.optionCode]);

  useEffect(() => {
    if (
      selectedReasoningEffort &&
      !selectedReasoningEffortOptions.some((option) => option.value === selectedReasoningEffort)
    ) {
      setSelectedReasoningEffort(null);
    }
  }, [selectedReasoningEffort, selectedReasoningEffortOptions]);

  useEffect(() => {
    if (isDraft || requestedSessionCode || requestedSourceTab || activeSessionCode) return;
    const defaultSession = firstSessionForSourceTab(sessionGroups, defaultChatSessionSourceTab);
    if (defaultSession) {
      setActiveSessionCode(defaultSession.sessionCode);
      selectSourceTab(defaultChatSessionSourceTab);
      replaceChatSearchParams((next) => {
        next.set("sessionCode", defaultSession.sessionCode);
      });
    }
  }, [
    activeSessionCode,
    isDraft,
    replaceChatSearchParams,
    requestedSessionCode,
    requestedSourceTab,
    sessionGroups,
  ]);

  useEffect(() => {
    activeSessionCodeRef.current = activeSessionCode;
  }, [activeSessionCode]);

  useEffect(() => {
    const htmlElement = document.documentElement;
    const bodyElement = document.body;

    htmlElement.classList.add("tron-chat-scroll-lock");
    bodyElement.classList.add("tron-chat-scroll-lock");

    return () => {
      htmlElement.classList.remove("tron-chat-scroll-lock");
      bodyElement.classList.remove("tron-chat-scroll-lock");
    };
  }, []);

  const contextAguiEvents = useMemo<TranscriptAguiEvent[]>(() => (
    mergeTranscriptAguiEvents<TranscriptAguiEvent>(aguiEvents, aguiProtocolEvents)
  ), [aguiEvents, aguiProtocolEvents]);
  const handleCreateCompactionForRightPanel = useStableCallback(handleCreateCompaction);
  const handleCancelGoalForRightPanel = useStableCallback(handleCancelGoal);
  const handleClearGoalForRightPanel = useStableCallback(handleClearGoal);
  const handleCreateGoalForRightPanel = useStableCallback(handleCreateGoal);
  const handlePauseGoalForRightPanel = useStableCallback(handlePauseGoal);
  const handleResumeGoalForRightPanel = useStableCallback(handleResumeGoal);
  const handleUpdateGoalForRightPanel = useStableCallback(handleUpdateGoal);
  const workspaceGoalRunning = Boolean(activeGoal?.goal.status === "ACTIVE" && currentSessionRunActive);
  const activeRightPanelSnapshot = useMemo<ChatRightPanelSnapshot | null>(() => {
    if (!activeSession) return null;

    return {
      compactionBusy: compactingSessionCode === activeSession.sessionCode,
      contextEvents: contextAguiEvents,
      goal: activeGoal,
      goalBusy: goalActionSessionCode === activeSession.sessionCode || compactingSessionCode === activeSession.sessionCode,
      goalStatus,
      latestCompaction,
      latestCompactionStatus,
      latestRequestEstimate: null,
      latestUsageAudit: null,
      messages: visibleMessages,
      model: selectedMainAgentModel,
      selectedModel,
      session: activeSession,
      todos: shortTermMemoryTodos,
      todosStatus: shortTermMemoryTodosStatus,
      workspaceGoalRunning,
    };
  }, [
    activeSession,
    activeGoal,
    compactingSessionCode,
    contextAguiEvents,
    goalActionSessionCode,
    goalStatus,
    latestCompaction,
    latestCompactionStatus,
    selectedMainAgentModel,
    selectedModel,
    shortTermMemoryTodos,
    shortTermMemoryTodosStatus,
    visibleMessages,
    workspaceGoalRunning,
  ]);

  useEffect(() => {
    if (!activeRightPanelSnapshot) return;
    const sessionCode = activeRightPanelSnapshot.session.sessionCode;
    setRightPanelSnapshots((current) => (
      current[sessionCode] === activeRightPanelSnapshot
        ? current
        : { ...current, [sessionCode]: activeRightPanelSnapshot }
    ));
  }, [activeRightPanelSnapshot]);

  const rightPanel = useMemo(() => {
    if (!hasActiveSession) return undefined;
    const snapshot = rightPanelSnapshots[activeSession?.sessionCode ?? ""];
    if (!snapshot) return undefined;
    return (
      <ChatSessionWorkspacePanel active snapshot={snapshot} onCancelGoal={handleCancelGoalForRightPanel} onClearGoal={handleClearGoalForRightPanel} onCreateGoal={handleCreateGoalForRightPanel} onPauseGoal={handlePauseGoalForRightPanel} onResumeGoal={handleResumeGoalForRightPanel} onUpdateGoal={handleUpdateGoalForRightPanel} />
    );
  }, [
    activeSession?.sessionCode,
    handleCancelGoalForRightPanel,
    handleClearGoalForRightPanel,
    handleCreateCompactionForRightPanel,
    handleCreateGoalForRightPanel,
    handlePauseGoalForRightPanel,
    handleResumeGoalForRightPanel,
    handleUpdateGoalForRightPanel,
    hasActiveSession,
    rightPanelSnapshots,
  ]);
  // Discover summaries while the sidebar is open, including before its tab exists.
  // The panel consumes this same result instead of starting a second polling loop.
  const subagents = useChatSubagents(
    activeSession?.sessionCode ?? "",
    activeSession?.workspaceCode ?? "",
    contextPanelOpen && Boolean(activeSession?.workspaceCode),
  );
  const rightPanelTabs = useMemo<EditableTab[]>(() => {
    const snapshot = rightPanelSnapshots[activeSession?.sessionCode ?? ""];
    if (!snapshot) return [];
    const tabs: EditableTab[] = [
      { key: "workspace", label: <><WorkspaceTabState goal={snapshot.goal} running={snapshot.workspaceGoalRunning} /><span>{t("chat.shortTermMemory.workspaceTab")}</span></>, closable: false, children: rightPanel },
      { key: "context", label: t("chat.shortTermMemory.shortTermMemoryTab"), closable: false, children: <ChatSessionContextPanel snapshot={snapshot} onCreateCompaction={handleCreateCompactionForRightPanel} /> },
    ];
    if (snapshot.session.workspaceCode && subagents.items.length > 0) {
      tabs.push({ key: "subagents", label: t("chat.subagents.title"), closable: false, children: <ChatSubagentsPanel key={snapshot.session.sessionCode} sessionCode={snapshot.session.sessionCode} workspaceCode={snapshot.session.workspaceCode} data={subagents} workRailLabels={workRailLabels} historyLabels={historyLabels} /> });
    }
    return tabs;
  }, [activeSession?.sessionCode, handleCreateCompactionForRightPanel, rightPanel, rightPanelSnapshots, subagents, workRailLabels, historyLabels, t]);
  const downloadPendingAttachment = useCallback(
    (attachment: ChatMessageAttachment) => {
      const file = draftResources.file(attachment);
      if (file) return Promise.resolve(file);
      if (!activeSession) return Promise.reject(new Error("No active chat session"));
      return downloadChatSessionAttachment(activeSession, attachment).then((response) => response.blob);
    },
    [activeSession, draftResources],
  );
  const transcriptWorkReplayOptions = useMemo(() => ({
    recentTurnLimit: aguiWorkReplayPreferences.recentTurnLimit,
  }), [aguiWorkReplayPreferences.recentTurnLimit]);
  const transcriptFooterAddon = useMemo(() => (
    <fieldset disabled={compactingSessionCode === activeSession?.sessionCode} className="m-0 flex min-w-0 max-w-full flex-wrap items-center gap-1 border-0 p-0">
        <input
          className="hidden"
          ref={attachmentFileInputRef}
          type="file"
          onChange={(event) => { void handleAttachmentFileInputChange(event); }}
        />
        <ChatAttachmentButton
          disabled={(!activeSession && !isDraft) || creatingSession}
          uploading={attachmentUploadPending}
          onClick={() => attachmentFileInputRef.current?.click()}
        />
        <ChatExecutionModeSelector
          value={executionMode}
          disabled={(!activeSession && !isDraft) || activeSession?.status === "ARCHIVED" || creatingSession || savingExecutionMode}
          onChange={(mode) => { void changeExecutionMode(mode); }}
        />
        <ChatModelEffortSelector
          defaultEffort={selectedModelDefaultReasoningEffort}
          disabled={modelsLoading || agentScopedModels.length === 0}
          effort={selectedReasoningEffort}
          effortOptions={selectedReasoningEffortOptions}
          emptyLabel={modelEmptyLabel}
          loading={modelsLoading}
          models={agentScopedModels}
          onSelectEffort={setSelectedReasoningEffort}
          onSelectModel={handleModelSelect}
          selectedModelCode={selectedModelCode}
        />
    </fieldset>
  ), [
    compactingSessionCode,
    agentScopedModels,
    agentSelectionLocked,
    agentsLoading,
    activeSession,
    isDraft,
    creatingSession,
    executionMode,
    savingExecutionMode,
    changeExecutionMode,
    attachmentUploadPending,
    chatAgents,
    handleAttachmentFileInputChange,
    modelEmptyLabel,
    modelsLoading,
    selectedAgentCode,
    selectedModelCode,
    selectedModelDefaultReasoningEffort,
    selectedReasoningEffort,
    selectedReasoningEffortOptions,
    t,
  ]);
  const formatAnnotationCount = useCallback(
    (count: number) => t("chat.attachments.annotationCount", { count }),
    [t],
  );
  const attachmentLabels = useMemo<ChatAttachmentLabels>(() => ({
    annotationPreview: t("chat.attachments.annotationPreview"),
    attachments: t("chat.attachments.attachments"),
  }), [i18n.language, t]);

  useTabPanelConfig({
    onPageInspectionConfirm: handleInspectionConfirmForRightPanel,
    rightPanelTabs,
    rightPanelBrowserOnly: false,
    rightPanelLabel: t("chat.shortTermMemory.workspaceTab"),
  });

  function handleAgentSelect(agentCode: string) {
    setSelectedAgentCode(agentCode);
    if (!activeSession) return;
    const nextSession = {
      ...activeSession,
      agentCode,
    };
    setSessionGroups((current) => replaceSession(current, nextSession));
    window.dispatchEvent(new CustomEvent("tron:chat-session-updated", {
      detail: { session: nextSession },
    }));
  }

  function handleModelSelect(optionCode: string) {
    setSelectedModelCode(optionCode);
    const selectedOption = agentScopedModels.find((model) => model.optionCode === optionCode) ?? null;
    const nextAgentCode = inferChatAgentCodeFromModel(selectedOption, selectedAgentCode || mainChatAgentOption.agentCode);
    setSelectedAgentCode((current) => (current === nextAgentCode ? current : nextAgentCode));
    if (activeSession) {
      const nextSession = {
        ...activeSession,
        agentCode: nextAgentCode,
      };
      setSessionGroups((current) => replaceSession(current, nextSession));
      window.dispatchEvent(new CustomEvent("tron:chat-session-updated", {
        detail: { session: nextSession },
      }));
    }
    const sessionCode = activeSession?.sessionCode;
    if (!sessionCode) return;
    const selectionKey = sessionModelSelectionKey(sessionCode, nextAgentCode);
    setSessionModelSelections((current) => (
      current[selectionKey] === optionCode
        ? current
        : {
          ...current,
          [selectionKey]: optionCode,
        }
    ));
  }

  async function handleCreateCompaction() {
    if (!activeSession || sending) return;
    await submitterRef.current?.submit({ content: "/compact" });
  }

  async function runGoalAction(action: () => Promise<unknown>, successMessage: string) {
    if (!activeSession || goalActionSessionCode) return false;
    const sessionCode = activeSession.sessionCode;
    setGoalActionSessionCode(sessionCode);
    try {
      await action();
      setShortTermMemoryRefreshKey((current) => current + 1);
      toast.success(successMessage, {
        title: t("chat.shortTermMemory.messages.goalUpdatedTitle"),
      });
      return true;
    } catch (error) {
      toast.error(error instanceof Error ? error.message : t("chat.shortTermMemory.messages.goalFailed"), {
        title: t("chat.shortTermMemory.messages.goalFailedTitle"),
      });
      return false;
    } finally {
      setGoalActionSessionCode("");
    }
  }

  async function handleUpdateGoal(input: UpdateChatSessionGoalInput) {
    return runGoalAction(
      () => updateChatSessionGoal(input),
      t("chat.shortTermMemory.messages.goalSaved"),
    );
  }

  async function handleCreateGoal(input: SetChatSessionGoalInput, showWorkspace = true) {
    if (sending) return false;
    if (!selectedModel) {
      toast.warning(t("chat.management.toasts.selectModelFirst"), { title: t("chat.management.toasts.modelRequiredTitle") });
      return false;
    }
    const created = await runGoalAction(
      () => setChatSessionGoal(input),
      t("chat.shortTermMemory.messages.goalSet"),
    );
    if (created) {
      if (showWorkspace) openRightSidebar();
      await submitterRef.current?.submit({ content: input.objective });
    }
    return created;
  }

  async function handlePauseGoal(input: ChatSessionGoalCommand) {
    return runGoalAction(
      () => pauseChatSessionGoal(input),
      t("chat.shortTermMemory.messages.goalPaused"),
    );
  }

  async function handleResumeGoal(input: ChatSessionGoalCommand) {
    if (sending) return false;
    if (!selectedModel) {
      toast.warning(t("chat.management.toasts.selectModelFirst"), { title: t("chat.management.toasts.modelRequiredTitle") });
      return false;
    }
    const resumed = await runGoalAction(
      () => resumeChatSessionGoal(input),
      t("chat.shortTermMemory.messages.goalResumed"),
    );
    if (resumed) {
      await submitterRef.current?.submit({ content: t("chat.shortTermMemory.goalResumePrompt") });
    }
    return resumed;
  }

  async function handleCancelGoal(input: ChatSessionGoalCommand) {
    return runGoalAction(
      () => cancelChatSessionGoal(input),
      t("chat.shortTermMemory.messages.goalCancelled"),
    );
  }

  async function handleClearGoal(input: ChatSessionGoalCommand) {
    return runGoalAction(
      () => clearChatSessionGoal(input),
      t("chat.shortTermMemory.messages.goalCleared"),
    );
  }

  return (
    <AgentTerminalSessionContext.Provider value={activeSession?.sessionCode ?? activeSessionCode}>
    <div className="relative z-0 flex h-full min-h-0 min-w-0 overflow-hidden overscroll-none bg-[var(--color-elevated-surface)]">
      <main className="flex min-h-0 min-w-0 flex-1 flex-col">
        {activeSession || isDraft ? (
          <AgentChatWindow
            speechScope={requestedSessionCode || requestedDraftCode || activeSession?.sessionCode}
            agentIdentity={selectedAgent?.kind === "system" && (!activeSession || activeSession.agentCode === selectedAgent.agentCode)
              ? { agentCode: selectedAgent.agentCode, name: selectedAgent.name } : undefined}
            allowDraftSession={isDraft}
            agentReplyEffect={agentReplyEffect}
            agentReplySpeed={agentReplySpeed}
            adapter={chatRuntimeAdapter}
            attachmentLabels={attachmentLabels}
            attachmentDownloader={downloadPendingAttachment}
            autoFocusComposer={shouldAutoFocusChatComposer()}
            composerRef={composerRef}
            submitterRef={submitterRef}
            onPlanCommand={runPlanCommand}
            config={{
              agentCode: effectiveAgentCode,
              disabledInput: creatingSession || savingExecutionMode
                || (!selectedModel?.enabled && taskExecutionLaunch?.sessionCode !== activeSession?.sessionCode),
              floating: false,
              inputPlaceholder: t("chat.management.composer.inputPlaceholder"),
              showAgui: true,
              storageKey: "tron.chat.management.runtime",
              title: activeSession?.title ?? t("shell:chatSidebar.newChat"),
              userInputLabels,
              workRailLabels,
              historyLabels,
              workspaceCode: activeWorkspaceCode,
            }}
            createSendInput={createRuntimeSendInput}
            currentUserDisplayName={currentUserDisplayName}
            debugOptions={transcriptWorkReplayOptions}
            formatAnnotationCount={formatAnnotationCount}
            draft={composer}
            initialSession={activeSession ?? null}
            onDraftChange={setComposer}
            onPendingAttachmentsChange={setPendingAttachments}
            onPasteAttachment={handleAttachmentFile}
            onRunStateChange={handleRuntimeRunStateChange}
            onSessionChange={handleRuntimeSessionChange}
            pendingAttachments={pendingAttachments}
            showAttachmentControl={false}
            showHeader={false}
            composerTop={<>
              {planReviewVisible && <ChatPlanReviewCard
                approving={planReviewApproving}
                approveDisabled={!selectedModel?.enabled || creatingSession}
                onApprove={() => { void approvePlanReview(); }}
                onKeepPlanning={keepPlanning}
              />}
              <fieldset disabled={compactingSessionCode === activeSession?.sessionCode}
              className="m-0 flex min-w-0 flex-wrap items-center gap-1 border-0 px-1 pb-2 pt-0">
              {(isDraft || activeSession?.sourceType?.toUpperCase() === "MANUAL") ? <ChatWorkingDirectoryControl
              session={activeSession} workspaceCode={activeWorkspaceCode} draftProjectCode={draftProjectCode} draftDirectoryCode={draftDirectoryCode}
              draftSelection={draftDirectorySelection} disabled={creatingSession || currentSessionRunActive}
              onDraftSelection={(value) => setDraftDirectories((current) => ({ ...current, [requestedDraftCode]: value }))}
              /> : null}
              <div className="min-w-0 max-w-[12rem]">
                <ChatAgentSelector locked={agentSelectionLocked} loading={agentsLoading}
                  agents={chatAgents} selectedAgentCode={selectedAgentCode} onSelect={handleAgentSelect} />
              </div>
            </fieldset>
            </>}
            footerAddon={transcriptFooterAddon}
            goal={activeGoal && activeGoal.goal.sessionCode === activeSession?.sessionCode && activeGoal.goal.status !== "CLEARED" ? {
              label: t("chat.shortTermMemory.goal"),
              objective: activeGoal.goal.objectiveMarkdown,
              status: t(`chat.shortTermMemory.goalStatuses.${activeGoal.goal.status}`),
              progress: t("chat.shortTermMemory.goalRunCount", { used: activeGoal.goal.runsUsed, max: activeGoal.goal.maxRuns }),
              busy: goalActionSessionCode === activeSession.sessionCode || compactingSessionCode === activeSession.sessionCode,
              labels: {
                pause: t("chat.shortTermMemory.pauseGoal"),
                resume: t("chat.shortTermMemory.resumeGoal"),
                restart: t("chat.shortTermMemory.restartGoal"),
                delete: t("chat.shortTermMemory.deleteGoal"),
              },
              onPause: activeGoal.goal.status === "ACTIVE" ? () => { void handlePauseGoal({ sessionCode: activeGoal.goal.sessionCode, goalCode: activeGoal.goal.goalCode }); } : undefined,
              resumeDisabled: sending,
              onResume: activeGoal.goal.status === "PAUSED" || activeGoal.goal.status === "BLOCKED" ? () => { void handleResumeGoal({ sessionCode: activeGoal.goal.sessionCode, goalCode: activeGoal.goal.goalCode }); } : undefined,
              onRestart: canStartNewGoalFromStatus(activeGoal.goal.status) && !sending ? () => { void handleCreateGoal({ sessionCode: activeGoal.goal.sessionCode, objective: activeGoal.goal.objectiveMarkdown, maxRuns: activeGoal.goal.maxRuns }, false); } : undefined,
              onDelete: () => { void handleClearGoal({ sessionCode: activeGoal.goal.sessionCode, goalCode: activeGoal.goal.goalCode }); },
            } : null}
            renderMarkdownLink={({ href, children }) => (
              <MarkdownLink
                href={href}
                onOpenInternal={openMarkdownInternalLink}
                onOpenExternal={openMarkdownExternalLink}
              >
                {children}
              </MarkdownLink>
            )}
          />
        ) : null}
      </main>
    </div>
    </AgentTerminalSessionContext.Provider>
  );
}
