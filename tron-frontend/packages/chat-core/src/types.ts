import type { A2uiActionInput, A2uiActionMetadata, A2uiState, ChatAnnotation, ChatHistoryLabels, HistoryQuery, MessageProvenance, SessionHistory, ToolActionSummary } from "./transportTypes";
export type ChatSessionStatus = "ACTIVE" | "ARCHIVED";
export type ToolExecutionMode = "CONTROLLED" | "AUTO_APPROVE" | "PLAN";
export type ChatSessionSourceType = "MANUAL" | "TASK" | "AUTOMATION" | "TERMINAL";
export type ChatMessageRole = "USER" | "ASSISTANT";
export type ChatMessageChannel = "MAIN_AGENT" | "TASK_AGENT";
export type ChatRunningSubmitMode = "steer" | "queue";
export type ChatSubmitMode = "send" | ChatRunningSubmitMode;
export type ReasoningEffort =
  | "None"
  | "Minimal"
  | "Low"
  | "Medium"
  | "High"
  | "XHigh"
  | "Max"
  | "Ultra";

export interface ChatModelSelection {
  kind: "Fixed" | "ProviderRoute" | "Auto";
  providerCode?: "PersonAi" | null;
  modelCode?: string | null;
  qualityType?: "High" | "Middle" | "Low" | null;
  routeCode?: "ChatAuto" | "EmbeddingAuto" | "MultimodalEmbeddingAuto" | "RerankAuto" | null;
}

export interface ChatSession {
  executionMode?: ToolExecutionMode;
  sourceCode?: string | null;
  id: number | string;
  sessionCode: string;
  workspaceCode: string | null;
  userCode?: string | null;
  appCode?: string | null;
  taskCode?: string | null;
  sourceType?: string | null;
  agentCode?: string | null;
  systemAgentSessionId?: string | null;
  ownerAssistantCode?: string | null;
  purpose?: string | null;
  title: string;
  status: ChatSessionStatus;
  lastRunCode?: string | null;
  lastRunStatus?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ChatSessionGroup {
  taskCode?: string | null;
  taskTitle: string;
  sessions: ChatSession[];
  pendingUserInputCount?: number;
  pendingUserInputSessionCodes?: string[];
}

export interface CreateChatSessionInput {
  executionMode?: ToolExecutionMode;
  workspaceCode: string | null;
  title?: string | null;
  appCode?: string | null;
  taskCode?: string | null;
  sourceType?: ChatSessionSourceType | null;
  agentCode?: string | null;
}

export interface UpdateChatSessionInput {
  title?: string;
  executionMode?: ToolExecutionMode;
}

export interface ChatMessage {
  phase?: string | null;
  annotations?: ChatAnnotation[];
  id: number | string;
  messageCode: string;
  sessionCode: string;
  runCode?: string | null;
  role: ChatMessageRole;
  channel: ChatMessageChannel;
  actor: string;
  provenance?: MessageProvenance;
  content: string;
  attachments?: ChatMessageAttachment[];
  inputTokens?: number | null;
  outputTokens?: number | null;
  totalTokens?: number | null;
  createdAt: string;
  requestedSkills?: string[];
  artifacts?: ChatArtifacts;
}

export type ChatArtifactAction = "preview" | "open" | "reveal" | "save";

export interface ChatArtifactItem {
  id: string;
  display: string;
  description?: string;
  mediaType?: string;
  byteSize?: number;
  actions?: ChatArtifactAction[];
}

export interface ChatArtifacts {
  version: "v1";
  items: ChatArtifactItem[];
}

export type ChatMessageAttachmentKind = "file" | "image" | "inspection";

export interface ChatMessageAttachment {
  id: string;
  kind: ChatMessageAttachmentKind;
  title?: string | null;
  imageUrl?: string | null;
  resourceCode?: string | null;
  fileName?: string | null;
  contentType?: string | null;
  byteSize?: number | null;
  metadata: Record<string, unknown>;
  summary: string;
}

export interface ChatAttachmentLabels {
  attachments: string;
  annotationPreview: string;
}

export interface ChatAttachmentUploadInput {
  file: File;
  kind?: ChatMessageAttachmentKind;
  metadata?: Record<string, unknown>;
  summary?: string;
}

export interface ChatTokenUsage {
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
}

export interface ChatRunSummary {
  runCode: string;
  conversationCode?: string | null;
  workspaceCode: string | null;
  reasoningEffort?: ReasoningEffort | null;
  status: "COMPLETED" | "AWAITING_USER_INPUT" | "STEP_LIMIT" | "TOOL_CALL_LIMIT";
  finalOutput: string;
  finalMessageIds?: string[];
  stepCount: number;
  tokenUsage?: ChatTokenUsage | null;
  systemAgentSessionId?: string | null;
}

export interface ChatSendResult {
  session: ChatSession;
  run: ChatRunSummary;
  messages: ChatMessage[];
  userInputRequest?: ChatUserInputRequest | null;
}

export type ChatUserInputAnswerKind = "Option" | "Other" | "Skipped";

export interface ChatUserInputOption {
  optionCode: string;
  label: string;
  description?: string | null;
}

export interface ChatUserInputQuestion {
  questionCode: string;
  question: string;
  options: ChatUserInputOption[];
}

export interface ChatUserInputAnswer {
  questionCode: string;
  answerKind: ChatUserInputAnswerKind;
  optionCode?: string | null;
  otherText?: string | null;
}

export interface ChatUserInputRequest {
  requestCode: string;
  sessionCode: string;
  runCode: string;
  assistantMessageCode?: string | null;
  intro: string;
  questions: ChatUserInputQuestion[];
  answers?: ChatUserInputAnswer[] | null;
  status: "Pending" | "Answered";
  requestedAt: number;
  answeredAt?: number | null;
}

export interface ChatAguiResumeEntry {
  interruptId: string;
  status: "resolved" | "cancelled";
  payload: unknown;
}

export interface ChatUserInputAnswerInput {
  clientCapabilities?: SendChatMessageInput["clientCapabilities"];
  a2uiMetadata?: SendChatMessageInput["a2uiMetadata"];
  answers: ChatUserInputAnswer[];
  resume?: ChatAguiResumeEntry[];
}

export interface ChatUserInputLabels {
  ariaLabel: string;
  progress: (current: number, total: number) => string;
  back: string;
  next: string;
  continue: string;
  confirm?: string;
  approve?: string;
  reject?: string;
  continuing: string;
  otherOptionLabel: string;
  otherOptionDescription: string;
  skipOptionLabel: string;
  skipOptionDescription: string;
  otherAnswerPlaceholder: string;
  otherAnswerAriaLabel: (index: number) => string;
}

export interface ChatRunNoticeLabels {
  compactionBlock?: (block: { id: string; beforeTokens: number; afterTokens: number; kind: "compressed" | "summary" }) => string;
  compactionNoChanges?: string;
  compactionDetailsUnavailable?: string;
  presentationIncomplete?: string;
  presentationIncompleteDetail?: string;
  traceId?: (traceId: string) => string;
  viewTrace?: string;
  compactionRequested: string;
  compactionStarted: string;
  compactionCompleted: string;
  compactionFailed: string;
  compactionInterrupted: string;
  toolSchemaOverBudget: string;
  toolSchemaBudget: (estimatedTokens: number, limitTokens: number, toolCount: number) => string;
  toolSchemaBudgetHint: string;
}

export interface ChatToolWorkLabels {
  ariaLabel: string;
  completed: (name: string) => string;
  running: (name: string) => string;
  domainExpanded: (name: string) => string;
  domainExpanding: (name: string) => string;
  group: (count: number) => string;
  groupSummary?: (summary: ToolActionSummary) => string;
  statusDone: string;
  statusRunning: string;
  parameters: string;
  result: string;
  log: string;
  noResult: string;
  waitingActivity: string;
  copyParameters: string;
  copyResult: string;
  reasoningCaptured: string;
  reasoningThinking: string;
  reasoningAria: string;
  emptyReasoning: string;
  resolveName: (runtimeName: string, args: unknown) => string;
}

export interface ChatWorkRailLabels {
  ariaLabel: string;
  preparing: string;
  responding: string;
  thinking: string;
  workedFor: (duration: string) => string;
  workingFor: (duration: string) => string;
  failed?: string;
  stopped?: string;
  notices?: Partial<ChatRunNoticeLabels>;
  askingQuestions?: (count: number) => string;
  askedQuestions?: (count: number) => string;
  waitingForAnswer?: string;
  skipped?: string;
  tool?: Partial<ChatToolWorkLabels>;
}

/** A small, immutable snapshot of the UI when the user submits a message. */
export interface ChatPageContext {
  title: string | null;
  path: string | null;
  tabTitle: string | null;
  workspaceCode: string | null;
  workspaceName: string | null;
  target: { key: string; value: string } | null;
}

export interface SendChatMessageInput {
  annotations?: ChatAnnotation[];
  content: string;
  runCode?: string | null;
  taskExecution?: boolean;
  workspaceCode: string | null;
  pageContext?: ChatPageContext | null;
  appCode?: string | null;
  taskCode?: string | null;
  agentCode?: string | null;
  modelSelection?: ChatModelSelection | null;
  modelCode?: string | null;
  systemModelId?: string | null;
  reasoningEffort?: ReasoningEffort | null;
  attachments?: ChatMessageAttachment[];
  a2uiMetadata?: A2uiActionMetadata;
  clientCapabilities?: {
    userInputRequests?: boolean;
    a2ui?: { version: "v0.9.1"; catalogIds: string[] };
    artifacts?: { actions?: ChatArtifactAction[] };
  };
  requestedSkills?: string[];
}



export interface ChatSteerContext {
  expectedRunCode: string;
}

export interface ChatSteerResult {
  sessionCode: string;
  runCode: string;
  message: ChatMessage;
}

export type ChatActiveRunState = "RUNNING" | "FINISHING";

export interface ChatActiveRun {
  runCode: string;
  runtimeAdapterKey: string;
  supportsSteer: boolean;
  operationKind?: string | null;
  state: ChatActiveRunState;
}

export interface ChatRuntimeRunState {
  /** History or authoritative run status is still being resolved. */
  loading?: boolean;
  active: boolean;
  runCode: string;
  sessionCode: string;
  supportsSteer: boolean;
  operationKind?: string | null;
}

export interface ChatActiveRunResult {
  sessionCode: string;
  activeRun: ChatActiveRun | null;
}

export interface ChatStopContext {
  expectedRunCode: string;
}

export interface ChatStopResult {
  sessionCode: string;
  runCode: string;
}

export interface ChatAguiEvent {
  id: string;
  type: string;
  eventName: string;
  runCode?: string | null;
  streamKey: string;
  occurredAt: string;
  outcome: string;
  metadata: Record<string, string>;
}

export interface ChatAguiInterrupt {
  id: string;
  reason: string;
  message?: string;
  toolCallId?: string;
  responseSchema?: unknown;
  metadata?: Record<string, unknown>;
}

export type ChatAguiRunOutcome =
  | { type: "success" }
  | { type: "interrupt"; interrupts: ChatAguiInterrupt[] };

export type ChatAguiStreamEventType =
  | "RUN_STARTED"
  | "RUN_FINISHED"
  | "RUN_ERROR"
  | "TEXT_MESSAGE_START"
  | "TEXT_MESSAGE_CONTENT"
  | "TEXT_MESSAGE_END"
  | "REASONING_START"
  | "REASONING_MESSAGE_START"
  | "REASONING_MESSAGE_CONTENT"
  | "REASONING_TEXT_CONTENT"
  | "REASONING_MESSAGE_END"
  | "REASONING_END"
  | "TOOL_CALL_START"
  | "TOOL_CALL_ARGS"
  | "TOOL_CALL_END"
  | "TOOL_CALL_RESULT"
  | "ACTIVITY_SNAPSHOT"
  | "ACTIVITY_DELTA"
  | string;

export interface ChatAguiStreamEvent {
  type: ChatAguiStreamEventType;
  threadId?: string;
  runId?: string;
  supportsSteer?: boolean;
  activity?: string;
  reasoningEffort?: ReasoningEffort | null;
  messageId?: string;
  role?: string;
  delta?: string | unknown[];
  content?: string;
  code?: string;
  message?: string;
  toolCallId?: string;
  toolCallName?: string;
  toolCallArgs?: unknown;
  parentMessageId?: string;
  timestamp?: number;
  outcome?: ChatAguiRunOutcome | null;
  result?: ChatSendResult;
  [key: string]: unknown;
}


export interface ChatStreamHandlers {
  onAguiEvent?: (event: ChatAguiStreamEvent) => void;
  onDelta?: (delta: string) => void;
}

export interface ChatAdapterCapabilities {
  supportsAbort: boolean;
  supportsActiveRun?: boolean;
  supportsAgui: boolean;
  supportsProtocolReplay: boolean;
  supportsSteer?: boolean;
  supportsStop?: boolean;
  supportsStreaming: boolean;
  supportsUserInputRequests?: boolean;
}

export interface ChatAdapter {
  loadA2uiState?: (id: string, signal?: AbortSignal) => Promise<A2uiState>;
  sendA2uiActionStream?: (session: ChatSession, input: A2uiActionInput, handlers?: ChatStreamHandlers, signal?: AbortSignal) => Promise<ChatSendResult>;
  capabilities: ChatAdapterCapabilities;
  createSession: (input: CreateChatSessionInput) => Promise<ChatSession>;
  deleteSession: (sessionCode: string, signal?: AbortSignal) => Promise<ChatSession>;
  downloadAttachment?: (
    session: ChatSession,
    attachment: ChatMessageAttachment,
    signal?: AbortSignal,
  ) => Promise<Blob>;
  getActiveRun?: (
    sessionCode: string,
    signal?: AbortSignal,
  ) => Promise<ChatActiveRunResult>;
  loadAguiEvents: (sessionCode: string) => Promise<ChatAguiEvent[]>;
  loadMessages: (sessionCode: string) => Promise<ChatMessage[]>;
  deleteTurn?: (sessionCode: string, input: { rootRunId: string; expectedVersion: string }, signal?: AbortSignal) => Promise<void>;
  loadHistory?: (sessionCode: string, query: HistoryQuery, signal?: AbortSignal) => Promise<SessionHistory>;
  loadUserInputRequests?: (
    sessionCode: string,
    signal?: AbortSignal,
  ) => Promise<ChatUserInputRequest[]>;
  answerUserInputRequestStream?: (
    session: ChatSession,
    requestCode: string,
    input: ChatUserInputAnswerInput,
    handlers?: ChatStreamHandlers,
    signal?: AbortSignal,
  ) => Promise<ChatSendResult>;
  loadProtocolEvents?: (sessionCode: string) => Promise<ChatAguiStreamEvent[]>;
  steerMessage?: (
    session: ChatSession,
    input: SendChatMessageInput,
    context: ChatSteerContext,
    signal?: AbortSignal,
  ) => Promise<ChatSteerResult>;
  stopRun?: (
    session: ChatSession,
    context: ChatStopContext,
    signal?: AbortSignal,
  ) => Promise<ChatStopResult>;
  uploadAttachment?: (
    session: ChatSession,
    input: ChatAttachmentUploadInput,
    signal?: AbortSignal,
  ) => Promise<ChatMessageAttachment>;
  streamMessage: (
    session: ChatSession,
    input: SendChatMessageInput,
    handlers?: ChatStreamHandlers,
    signal?: AbortSignal,
  ) => Promise<ChatSendResult>;
}

export interface ChatFloatingState {
  collapsed: boolean;
  height: number;
  open: boolean;
  width: number;
  x: number;
  y: number;
}

export interface ChatClientConfig {
  inputSuggestions?: Array<{ kind: "skill" | "command"; name: string; displayName?: string; description?: string; category?: string }>;
  inputSuggestionLabels?: { commands: string; skills: string; commandsAndSkills: string };
  skillSuggestionTriggers?: Array<"$" | "/">;
  accept?: string;
  agentCode?: string;
  appCode?: string;
  appKey?: string;
  baseUrl?: string;
  defaultSessionCode?: string;
  disabledInput?: boolean;
  disabledToBottom?: boolean;
  draggable?: boolean;
  floating?: boolean;
  fullScreen?: boolean;
  fullScreenMinWidth?: number;
  inputPlaceholder?: string;
  queuedInputPlaceholder?: string;
  steeringInputPlaceholder?: string;
  /** Host locale (e.g. i18n.language) used to localize chat card copy. */
  locale?: string;
  runningEnterSubmitMode?: "steer" | "queue";
  maxFileSize?: number;
  maxLimitFile?: number;
  open?: boolean;
  openAppCode?: string;
  productPrefix?: string;
  resizable?: boolean;
  showAgui?: boolean;
  showSessionList?: boolean;
  showNewSession?: boolean;
  storageKey?: string;
  taskCode?: string;
  title?: string;
  tokenPrefix?: string;
  userInputLabels?: ChatUserInputLabels;
  userCode?: string;
  userId?: string;
  variables?: unknown;
  workRailLabels?: ChatWorkRailLabels;
  historyLabels?: ChatHistoryLabels;
  workspaceCode?: string;
}

export interface ChatEnvOptions {
  fallbackPrefix?: string;
  prefix?: string;
}

export interface ChatDebugOptions {
  debugEventTypes?: string[];
  recentTurnLimit?: number;
}
