import { useComposerDisabled } from "@personai/agent-ui-composer-react";
import { defaultChatAnnotationLabels, zhChatAnnotationLabels } from "@personai/agent-ui-chat-core";
import { lazy, Suspense, useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState, type Ref } from "react";
import { useTranslation } from "react-i18next";
import { useChatWorkRailLabels, useChatHistoryLabels } from "../hooks/useChatWorkRailLabels";
import { ChatRuntimeWindow, type ChatRuntimeWindowProps, type ChatRuntimeSubmitter } from "@personai/agent-ui-chat-react";
import type * as Ui from "@personai/agent-ui-chat-core";
import type { ChatAdapter, ChatClientConfig, ChatMessageAttachment, ChatRuntimeRunState, ChatSession, SendChatMessageInput } from "@tron/chat-core";
import { createAgentUiAdapter } from "@tron/chat-agent-ui-adapter";
import { fromUiAttachment, fromUiSession, toUiAttachment, toUiInput, toUiSession } from "@tron/chat-agent-ui-adapter";
import { useChatContextRefreshInterval } from "../hooks/useChatContextRefreshInterval";
import { useChatInputSuggestions } from "@/domain/chat/hooks/useChatInputSuggestions";
import { useDeleteTurnConfirmation } from "../hooks/useDeleteTurnConfirmation";
import { useChatFeedback } from "@/domain/chat/hooks/useChatFeedback";
import { useAguiWorkReplayPreferences } from "@/domain/settings/model/aguiWorkReplayPreferences";
import { latestChatComposerMetrics, type ChatComposerMetrics as ComposerMetrics } from "../model/chatComposerMetrics";
import type { ContextAssemblySnapshot } from "../model/contextAssemblyReport";
import { ChatComposerMetrics } from "./ChatComposerMetrics";
import { OPEN_CHAT_CONTEXT_EVENT } from "@/shared/lib/shellEvents";
import { requestApi } from "@/shared/api/httpClient";
import { chatWorkspaceForSession } from "@/shared/api/chatWorkspaceScope";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { ChatArtifactPreviewDialog } from "./ChatArtifactPreviewDialog";
import "./AgentChatWindow.css";
import { SudoPasswordDialog } from "./SudoPasswordDialog";
import { ChatSpeechButton } from "./ChatSpeechButton";
import { useTabVisibility } from "@/components/editable-tabs";
import { ChatAttachmentButton } from "./ChatAttachmentButton";

const ChatContextInspector = lazy(() => import("./ChatContextInspector").then(module => ({ default: module.ChatContextInspector })));

type MappedProps = "adapter" | "attachmentDownloader" | "submitterRef" | "config" | "initialSession" | "pendingAttachments" | "onPendingAttachmentsChange" | "onSessionChange" | "onRunStateChange" | "createSendInput";
export interface AgentChatSubmitter {
  submit(input: Pick<SendChatMessageInput, "content" | "attachments">): Promise<boolean>;
}
export interface AgentChatWindowProps extends Omit<ChatRuntimeWindowProps, MappedProps> {
  adapter: ChatAdapter;
  attachmentDownloader?: (attachment: ChatMessageAttachment) => Promise<Blob>;
  submitterRef?: Ref<AgentChatSubmitter>;
  knowledgeBaseId?: string;
  agentIdentity?: { agentCode: string; name: string };
  speechScope?: string;
  config: ChatClientConfig;
  initialSession?: ChatSession | null;
  pendingAttachments?: ChatMessageAttachment[];
  onPendingAttachmentsChange?: (value: ChatMessageAttachment[]) => void;
  onSessionChange?: (value: ChatSession | null) => void;
  onRunStateChange?: (value: ChatRuntimeRunState) => void;
  createSendInput?: (session: ChatSession, content: string, attachments: ChatMessageAttachment[]) => SendChatMessageInput | null;
  /**
   * Hosts that expose plan mode pass this handler; others hide the `/plan`
   * command. `toggle` flips the mode for a bare `/plan`; `ensure` switches plan
   * mode on and reports whether the mode is ready before a described request is
   * sent.
   */
  onPlanCommand?: (session: ChatSession, intent: "toggle" | "ensure") => Promise<boolean>;
}

const planCommandToken = "/plan";

/**
 * `/plan` is a host command that may sit anywhere in the composer text. It must
 * stand as its own token so paths such as `/plan.md`, `docs/plan`, or
 * `https://host/plan` stay literal, while Chinese descriptions typed right after
 * it (no space) still match.
 */
const planCommandPattern = /(^|[^A-Za-z0-9_./\\-])\/plan(?![A-Za-z0-9_./\\-])/i;

function extractPlanCommand(text: string): { content: string; requested: boolean } {
  const match = planCommandPattern.exec(text);
  if (!match) return { content: text, requested: false };
  const start = match.index + match[1].length;
  const before = text.slice(0, start);
  const after = text.slice(start + planCommandToken.length);
  const leading = /\s*$/.exec(before)?.[0] ?? "";
  const trailing = /^\s*/.exec(after)?.[0] ?? "";
  const left = before.slice(0, before.length - leading.length);
  const right = after.slice(trailing.length);
  // Keep one separator when text remains on both sides, preferring a line break.
  const separator = !left || !right ? "" : /\r|\n/.test(leading + trailing) ? "\n" : leading || trailing ? " " : "";
  return { content: `${left}${separator}${right}`.trim(), requested: true };
}

function ComposerControls({ children }: { children: React.ReactNode }) {
  const disabled = useComposerDisabled();
  return <fieldset disabled={disabled} inert={disabled} className="m-0 flex min-w-0 max-w-full flex-wrap items-center gap-1 border-0 p-0">{children}</fieldset>;
}

/** Product context adapter only. Rendering, streaming, queues, stop and replay belong to Agent UI. */
export function AgentChatWindow({ adapter, attachmentDownloader, submitterRef, config, knowledgeBaseId, agentIdentity, speechScope, initialSession, pendingAttachments,
  onPendingAttachmentsChange, onSessionChange, onRunStateChange, createSendInput, onError, onInfo, onArtifactPreview, footerAddon, draft: controlledDraft, onDraftChange, composerRef: hostComposerRef, onPlanCommand, ...props }: AgentChatWindowProps) {
  const feedback = useChatFeedback();
  const tabVisible = useTabVisibility();
  const composerRef = useRef<HTMLTextAreaElement | null>(null);
  const [internalDraft, setInternalDraft] = useState("");
  const draft = controlledDraft ?? internalDraft;
  const draftRef = useRef(draft);
  draftRef.current = draft;
  const changeDraft = useCallback((value: string) => {
    draftRef.current = value;
    if (controlledDraft === undefined) setInternalDraft(value);
    onDraftChange?.(value);
  }, [controlledDraft, onDraftChange]);
  const setComposerRef = useCallback((element: HTMLTextAreaElement | null) => {
    composerRef.current = element;
    if (typeof hostComposerRef === "function") return hostComposerRef(element);
    if (hostComposerRef) hostComposerRef.current = element;
  }, [hostComposerRef]);
  const appendTranscript = useCallback((text: string) => {
    if (!composerRef.current || composerRef.current.disabled) return;
    const current = draftRef.current;
    changeDraft(`${current}${current && !/\s$/.test(current) ? "\n" : ""}${text}`);
    composerRef.current?.focus();
  }, [changeDraft]);
  const runtimeSubmitterRef = useRef<ChatRuntimeSubmitter | null>(null);
  const [activeSessionCode, setActiveSessionCode] = useState<string | null>(initialSession?.sessionCode ?? null);
  const turnDeletion = useDeleteTurnConfirmation(activeSessionCode);
  const [activeSession, setActiveSession] = useState<ChatSession | null>(initialSession ?? null);
  const [artifactPreview, setArtifactPreview] = useState<{ sessionCode: string; workspaceCode: string; runCode: string; item: Ui.ChatArtifactItem } | null>(null);
  const initialSessionRef = useRef(initialSession);
  initialSessionRef.current = initialSession;
  const [contextOpen, setContextOpen] = useState(false);
  const [composerSnapshot, setComposerSnapshot] = useState<{ sessionCode: string; metrics: ComposerMetrics | null } | null>(null);
  const metricsRequestRef = useRef(0);
  const wasRunActiveRef = useRef(false);
  const [metricsRunSessionCode, setMetricsRunSessionCode] = useState<string | null>(null);
  const metricsRefreshKey = useChatContextRefreshInterval(activeSessionCode ?? "", Boolean(activeSessionCode && metricsRunSessionCode === activeSessionCode));
  useEffect(() => {
    setActiveSessionCode(initialSession?.sessionCode ?? null);
    setActiveSession(initialSession ?? null);
    setContextOpen(false);
  }, [initialSession?.sessionCode]);
  const refreshComposerMetrics = useCallback(async (sessionCode: string) => {
    const request = ++metricsRequestRef.current;
    const [eventsResult, contextResult] = await Promise.allSettled([
      adapter.loadAguiEvents(sessionCode),
      requestApi<{ snapshot: ContextAssemblySnapshot | null }>("GET /api/v1/chats/sessions/get-context-assembly", {
        queryParams: { sessionCode },
        headers: activeSession?.ownerAssistantCode ? { "X-Assistant-Code": activeSession.ownerAssistantCode } : undefined,
      }),
    ]);
    const events = eventsResult.status === "fulfilled" ? eventsResult.value : [];
    const contextSnapshot = contextResult.status === "fulfilled" ? contextResult.value.snapshot : null;
    const metrics = latestChatComposerMetrics(events, contextSnapshot);
    if (request === metricsRequestRef.current) {
      setComposerSnapshot({ sessionCode, metrics });
    }
  }, [adapter, activeSession?.ownerAssistantCode]);
  useEffect(() => {
    if (activeSessionCode) void refreshComposerMetrics(activeSessionCode);
  }, [activeSessionCode, metricsRefreshKey, refreshComposerMetrics]);
  useImperativeHandle(submitterRef, () => ({
    submit: (input) => runtimeSubmitterRef.current?.submit({
      content: input.content, attachments: input.attachments?.map(toUiAttachment),
    }) ?? Promise.resolve(false),
  }), []);
  const { preferences: workPreferences } = useAguiWorkReplayPreferences();
  const suggestionAssistantCode = activeSession?.ownerAssistantCode ?? initialSession?.ownerAssistantCode
    ?? (initialSession?.sourceCode?.startsWith("__assistant:") ? initialSession.sourceCode.slice("__assistant:".length) : undefined);
  const inputSuggestions = useChatInputSuggestions(config.agentCode, activeSessionCode ?? initialSession?.sessionCode,
    suggestionAssistantCode ?? undefined, config.workspaceCode ?? activeSession?.workspaceCode, knowledgeBaseId);
  // The server lists every internal command; only hosts that expose plan mode
  // keep `/plan` in the menu, so other composers never send it as chat content.
  const composerSuggestions = useMemo(() => {
    const commands = inputSuggestions.inputSuggestions;
    if (onPlanCommand || !commands?.some((item) => item.kind === "command" && item.name === "plan")) return inputSuggestions;
    return { ...inputSuggestions, inputSuggestions: commands.filter((item) => !(item.kind === "command" && item.name === "plan")) };
  }, [inputSuggestions, onPlanCommand]);
  const { t, i18n } = useTranslation("pages");
  const workRailLabels = useChatWorkRailLabels();
  const historyLabels = useChatHistoryLabels();
  const userInputLabels = useMemo(() => ({
    ariaLabel: t("chat.userInput.ariaLabel"),
    progress: (current: number, total: number) => t("chat.userInput.progress", { current, total }),
    back: t("chat.userInput.back"), next: t("chat.userInput.next"),
    continue: t("chat.userInput.continue"), approve: t("chat.userInput.approve"),
    approvalTitle: t("chat.userInput.approvalTitle"),
    approvalQuestion: (action: string) => t("chat.userInput.approvalQuestion", { action }),
    approvalFallbackQuestion: t("chat.userInput.approvalFallbackQuestion"),
    approveAll: t("chat.userInput.approveAll"),
    rejectAll: t("chat.userInput.rejectAll"),
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
    reject: t("chat.userInput.reject"), continuing: t("chat.userInput.continuing"),
    otherOptionLabel: t("chat.userInput.otherOptionLabel"),
    otherOptionDescription: t("chat.userInput.otherOptionDescription"),
    skipOptionLabel: t("chat.userInput.skipOptionLabel"),
    skipOptionDescription: t("chat.userInput.skipOptionDescription"),
    otherAnswerPlaceholder: t("chat.userInput.otherAnswerPlaceholder"),
    otherAnswerAriaLabel: (index: number) => t("chat.userInput.otherAnswerAriaLabel", { index }),
  }), [t]);
  const identityCode = agentIdentity?.agentCode;
  const identityName = agentIdentity?.name;
  const identitySessionId = initialSession?.sessionCode;
  const uiAdapter = useMemo(() => createAgentUiAdapter(adapter, config,
    identityCode && identityName ? { agentCode: identityCode, name: identityName, sessionId: identitySessionId } : undefined),
    [adapter, config.workspaceCode, config.appCode, config.taskCode, config.agentCode, identityCode, identityName, identitySessionId]);
  const session = useMemo(() => initialSession ? toUiSession(initialSession,
    initialSession.agentCode === identityCode ? identityName : undefined) : initialSession, [initialSession, identityCode, identityName]);
  const attachments = useMemo(() => pendingAttachments?.map(toUiAttachment), [pendingAttachments]);
  const downloadAttachment = useMemo(() => attachmentDownloader
    ? (value: Ui.ChatMessageAttachment) => attachmentDownloader(fromUiAttachment(value))
    : undefined, [attachmentDownloader]);
  const handleSession = useCallback((value: Ui.ChatSession | null) => {
    setActiveSessionCode(value?.sessionId ?? null);
    const mapped = value ? fromUiSession(value) : null;
    const origin = initialSessionRef.current;
    // Generic completion responses omit host ownership. Retain it only for
    // the same session, including approval resumes and metric inspection.
    const nextSession = mapped && origin?.sessionCode === mapped.sessionCode
      ? { ...mapped, ownerAssistantCode: mapped.ownerAssistantCode ?? origin.ownerAssistantCode, purpose: mapped.purpose ?? origin.purpose }
      : mapped;
    setActiveSession(nextSession);
    onSessionChange?.(nextSession);
  }, [onSessionChange]);
  const openContext = useCallback(() => {
    if (config.floating && activeSession?.ownerAssistantCode) {
      setContextOpen(true);
      return;
    }
    window.dispatchEvent(new CustomEvent(OPEN_CHAT_CONTEXT_EVENT, { detail: activeSession }));
  }, [activeSession, config.floating]);
  const handleAttachments = useCallback((value: Ui.ChatMessageAttachment[]) =>
    onPendingAttachmentsChange?.(value.map(fromUiAttachment)), [onPendingAttachmentsChange]);
  const handleArtifactPreview = useCallback((message: Ui.ChatMessage, item: Ui.ChatArtifactItem) => {
    if (onArtifactPreview) {
      onArtifactPreview(message, item);
      return;
    }
    const sessionCode = message.sessionId;
    const workspaceCode = chatWorkspaceForSession(sessionCode) ?? (activeSession?.sessionCode === sessionCode
      ? activeSession.workspaceCode ?? config.workspaceCode ?? ""
      : initialSessionRef.current?.sessionCode === sessionCode
        ? initialSessionRef.current.workspaceCode ?? config.workspaceCode ?? ""
        : config.workspaceCode ?? "");
    setArtifactPreview({ sessionCode, workspaceCode, runCode: message.runId ?? activeSession?.lastRunCode ?? "", item });
  }, [activeSession, config.workspaceCode, onArtifactPreview]);
  // Host callbacks may be inline. Their identity must not trigger another
  // runtime-state notification and a host refresh/render feedback loop.
  const runStateCallback = useRef(onRunStateChange);
  runStateCallback.current = onRunStateChange;
  const handleRun = useCallback((value: Ui.ChatRuntimeRunState) => {
    if (value.sessionId && !value.loading) {
      window.dispatchEvent(new CustomEvent("tron:chat-session-run-state", {
        detail: { sessionCode: value.sessionId, active: value.active },
      }));
    }
    setMetricsRunSessionCode(value.active ? value.sessionId ?? null : null);
    if (wasRunActiveRef.current && !value.active && value.sessionId) {
      void refreshComposerMetrics(value.sessionId);
    }
    wasRunActiveRef.current = value.active;
    runStateCallback.current?.({
      loading: value.loading, active: value.active, runCode: value.runId, sessionCode: value.sessionId, supportsSteer: value.supportsSteer, operationKind: value.operationKind,
    });
  }, [refreshComposerMetrics]);
  const createInput = useCallback((value: Ui.ChatSession, content: string, files: Ui.ChatMessageAttachment[]) => {
    const hostSession = fromUiSession(value);
    if (onPlanCommand) {
      const plan = extractPlanCommand(content);
      if (plan.requested) {
        // `/plan` is a host command, not chat content: strip it and keep the
        // message out of the transcript. The runtime clears the draft on success
        // only, so the command clears it here. A description is sent after the
        // mode is stored, so the run is admitted in plan mode.
        changeDraft("");
        if (!plan.content) {
          void onPlanCommand(hostSession, "toggle").catch(() => undefined);
          return null;
        }
        const restoreDraft = () => {
          if (!draftRef.current) changeDraft(content);
        };
        void onPlanCommand(hostSession, "ensure")
          .then((accepted) => accepted
            ? runtimeSubmitterRef.current?.submit({ content: plan.content, attachments: files }) ?? false
            : false)
          .then((sent) => {
            if (!sent) {
              restoreDraft();
              return;
            }
            if (files.length) onPendingAttachmentsChange?.([]);
          })
          .catch(restoreDraft);
        return null;
      }
    }
    const hostFiles = files.map(fromUiAttachment);
    const input = createSendInput ? createSendInput(hostSession, content, hostFiles) : {
      content, attachments: hostFiles, workspaceCode: hostSession.workspaceCode,
      appCode: hostSession.appCode ?? config.appCode, taskCode: hostSession.taskCode ?? config.taskCode,
      agentCode: config.agentCode, clientCapabilities: { userInputRequests: true },
    };
    return input ? toUiInput(input) : null;
  }, [changeDraft, createSendInput, config.appCode, config.taskCode, config.agentCode, onPlanCommand, onPendingAttachmentsChange]);
  return <>{turnDeletion.dialog}
    {activeSession && tabVisible && (!config.floating || !props.windowState || (props.windowState.open && !props.windowState.collapsed)) && <SudoPasswordDialog
      key={activeSession.sessionCode} sessionCode={activeSession.sessionCode}
      workspaceCode={activeSession.workspaceCode ?? config.workspaceCode ?? ""}
      assistantCode={activeSession.ownerAssistantCode ?? undefined} knowledgeBaseId={knowledgeBaseId} />}
    <ChatRuntimeWindow {...props} adapter={uiAdapter} attachmentDownloader={downloadAttachment} submitterRef={runtimeSubmitterRef}
    onArtifactPreview={handleArtifactPreview}
    draft={draft} onDraftChange={changeDraft} composerRef={setComposerRef}
    renderAttachmentControl={props.renderAttachmentControl ?? ((control) => <ChatAttachmentButton {...control} />)}
    footerAddon={<><ComposerControls>{footerAddon}</ComposerControls><ChatComposerMetrics metrics={composerSnapshot?.sessionCode === activeSessionCode ? composerSnapshot.metrics : null} onOpen={openContext} /><ChatSpeechButton composer={composerRef}
      active={tabVisible && !config.disabledInput && (!config.floating || !props.windowState || (props.windowState.open && !props.windowState.collapsed))}
      scope={`${config.workspaceCode ?? ""}:${speechScope ?? initialSession?.sessionCode ?? activeSessionCode ?? "draft"}`} floating={Boolean(config.floating)} onTranscript={appendTranscript} /></>}
    confirmDeleteTurn={props.confirmDeleteTurn ?? turnDeletion.confirm}
    errorDisplay="external" onError={onError ?? feedback.error} onInfo={onInfo ?? feedback.info}
    config={{ ...config, ...composerSuggestions, defaultSessionId: config.defaultSessionCode,
      turnDeletionLabels: { action: t("chat.management.deleteTurn.action"), confirm: t("chat.management.deleteTurn.description"), failed: t("chat.management.toasts.chatDeleteFailedTitle") },
      resizeHint: t("chat.floating.resizeHint"),
      locale: config.locale ?? i18n.language,
      showReasoningCards: workPreferences.showReasoningCards,
      runningEnterSubmitMode: workPreferences.runningEnterSubmitMode,
      inputPlaceholder: "",
      queuedInputPlaceholder: "",
      steeringInputPlaceholder: "",
      annotationLabels: i18n.language.startsWith("zh") ? zhChatAnnotationLabels : defaultChatAnnotationLabels,
      workRailLabels: config.workRailLabels ?? workRailLabels,
      historyLabels: config.historyLabels ?? historyLabels,
      userInputLabels: config.userInputLabels ?? userInputLabels,
    }}
    initialSession={session} pendingAttachments={attachments} createSendInput={createInput}
    onSessionChange={handleSession} onPendingAttachmentsChange={handleAttachments} onRunStateChange={handleRun} />
    {artifactPreview ? <ChatArtifactPreviewDialog
      open
      onOpenChange={(open) => { if (!open) setArtifactPreview(null); }}
      sessionCode={artifactPreview.sessionCode}
      workspaceCode={artifactPreview.workspaceCode}
      runCode={artifactPreview.runCode}
      item={artifactPreview.item}
    /> : null}
    <Dialog open={contextOpen} onOpenChange={setContextOpen}>
      <DialogContent className="flex h-[min(80dvh,900px)] flex-col sm:max-w-3xl">
        <DialogHeader><DialogTitle>{t("chat.shortTermMemory.shortTermMemoryTab")}</DialogTitle></DialogHeader>
        <div className="relative min-h-0 flex-1">
          {contextOpen && activeSession?.ownerAssistantCode && <Suspense fallback={null}><ChatContextInspector session={activeSession} /></Suspense>}
        </div>
      </DialogContent>
    </Dialog>
  </>;
}
