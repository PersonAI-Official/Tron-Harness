import type {
  ChatMessageAttachmentKind,
  ChatUserInputAnswer,
  ChatUserInputAnswerInput,
  ChatUserInputRequest,
  ReasoningEffort,
  ToolExecutionMode,
} from "@tron/chat-core";
import type { ModelSelection } from "@/domain/model/model";

export type {
  ChatUserInputAnswer,
  ChatUserInputAnswerInput,
  ChatUserInputRequest,
};

export type ChatSessionStatus = "ACTIVE" | "ARCHIVED";
export type ChatMessageRole = "USER" | "ASSISTANT";
export type ChatMessageChannel = "MAIN_AGENT" | "TASK_AGENT";

export interface ChatSession {
  executionMode?: ToolExecutionMode;
  id: number | string;
  sessionCode: string;
  workspaceCode: string | null;
  projectCode?: string | null;
  directoryCode?: string | null;
  workingDirectoryCode?: string | null;
  userCode?: string | null;
  appCode?: string | null;
  taskCode?: string | null;
  sourceType?: "Manual" | "Task" | "Automation" | string;
  sourceCode?: string | null;
  agentCode?: string | null;
  systemAgentSessionId?: string | null;
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
  sourceType?: "Manual" | "Task" | "Automation" | string;
  sourceCode?: string | null;
  sessions: ChatSession[];
  pendingUserInputCount?: number;
  pendingUserInputSessionCodes?: string[];
}

export interface CreateChatSessionInput {
  executionMode?: ToolExecutionMode;
  workspaceCode: string | null;
  projectCode?: string | null;
  directoryCode?: string | null;
  workingDirectoryCode?: string | null;
  title?: string | null;
  appCode?: string | null;
  taskCode?: string | null;
  agentCode?: string | null;
}

export interface UpdateChatSessionInput {
  title?: string;
  executionMode?: ToolExecutionMode;
}

export interface ChatMessage {
  id: string | number;
  messageCode: string;
  sessionCode: string;
  runCode?: string | null;
  role: ChatMessageRole;
  channel: ChatMessageChannel;
  actor: string;
  provenance?: import("@personai/agent-ui-chat-core").MessageProvenance;
  content: string;
  attachments?: ChatMessageAttachment[];
  inputTokens?: number | null;
  outputTokens?: number | null;
  totalTokens?: number | null;
  createdAt: string;
}

export interface ChatTokenUsage {
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  cachedInputTokens?: number | null;
  reasoningTokens?: number | null;
}

export interface ChatTokenEstimateSnapshot {
  estimator?: string;
  estimatedInputTokens: number;
  messageTokenEstimate: number;
  toolSchemaTokenEstimate: number;
  contextWindowLimit?: number | null;
  effectiveInputBudget?: number | null;
  outputTokenReserve?: number | null;
  rebased: boolean;
  overBudget: boolean;
  compactionTriggerReached: boolean;
  checkpointCode?: string | null;
  modelCode: string;
  modelName: string;
  modelProtocol: string;
  usageSource: string;
  usageConfidence: string;
}

export interface ChatContextBudgetBreakdown {
  applicationContextTokens?: number;
  applicationContextMessageCount?: number;
  promptPrefixMessageCount?: number | null;
  promptPrefixTokens: number;
  checkpointTokens: number;
  workingStateTokens: number;
  recentTranscriptTokens: number;
  rawTailTranscriptTokens: number;
  compressedHistoryTokens: number;
  toolSchemaTokens: number;
  toolSchemaBudgetTokens?: number | null;
  estimatedInputTokens: number;
  promptPrefixBudgetTokens?: number | null;
  promptPrefixMaxRatioBps?: number | null;
  promptPrefixMaxTokens?: number | null;
  checkpointBudgetTokens?: number | null;
  workingStateBudgetTokens?: number | null;
  effectiveInputBudget?: number | null;
  contextWindowLimit?: number | null;
  outputTokenReserve?: number | null;
  compactionTriggerRatioBps?: number | null;
  compactionTriggerTokens?: number | null;
  compactionTriggerReached: boolean;
  inputBudgetExceeded: boolean;
  retainedHistoryRatioBps?: number | null;
  effectiveRetainedHistoryTokens?: number | null;
  currentSessionHistoryBudgetTokens?: number | null;
  historySourceMessageCount?: number | null;
  rawTailMessageCount?: number | null;
  compressedHistoryMessageCount?: number | null;
  rawTailStartSeq?: number | null;
}

export interface ChatUsageAuditSnapshot {
  requestEstimate: ChatTokenEstimateSnapshot;
  providerUsage?: ChatTokenUsage | null;
  cumulativeRunUsage?: ChatTokenUsage | null;
  inputTokenDelta?: number | null;
  inputTokenRatio?: number | null;
  responseId?: string | null;
  usageSource: string;
  usageConfidence: string;
}

export interface ChatRunSummary {
  runCode: string;
  conversationCode?: string | null;
  workspaceCode: string | null;
  reasoningEffort?: ReasoningEffort | null;
  status: "COMPLETED" | "AWAITING_USER_INPUT";
  finalOutput: string;
  stepCount: number;
  tokenUsage?: ChatTokenUsage | null;
  latestRequestEstimate?: ChatTokenEstimateSnapshot | null;
  latestUsageAudit?: ChatUsageAuditSnapshot | null;
  systemAgentSessionId?: string | null;
}

export interface ChatSendResult {
  session: ChatSession;
  run: ChatRunSummary;
  messages: ChatMessage[];
  userInputRequest?: ChatUserInputRequest | null;
}

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

export interface SendChatMessageInput {
  content: string;
  workspaceCode: string | null;
  runCode?: string | null;
  taskExecution?: boolean;
  appCode?: string | null;
  taskCode?: string | null;
  agentCode?: string | null;
  /** First-party clients send this; modelCode remains a legacy compatibility field. */
  modelSelection?: ModelSelection | null;
  modelCode?: string | null;
  systemModelId?: string | null;
  reasoningEffort?: ReasoningEffort | null;
  attachments?: ChatMessageAttachment[];
  clientCapabilities?: {
    userInputRequests?: boolean;
  };
}

export interface SteerChatMessageInput {
  sessionCode: string;
  expectedRunCode: string;
  content: string;
  attachments: ChatMessageAttachment[];
}

export interface ChatSteerResult {
  sessionCode: string;
  runCode: string;
  message: ChatMessage;
}

export interface ActiveChatRun {
  runCode: string;
  runtimeAdapterKey: string;
  supportsSteer: boolean;
  state: string;
}

export interface GetActiveChatRunResult {
  sessionCode: string;
  activeRun: ActiveChatRun | null;
}

export interface StopChatRunInput {
  sessionCode: string;
  expectedRunCode: string;
}

export interface StopChatRunResult {
  sessionCode: string;
  runCode: string;
}

export type ChatSessionTodoStatus = "pending" | "in_progress" | "completed" | "cancelled";
export type ChatSessionTodoPriority = "high" | "medium" | "low";

export interface ChatSessionTodo {
  id: number | string;
  todoCode: string;
  sessionCode: string;
  workspaceCode: string;
  position: number;
  content: string;
  status: ChatSessionTodoStatus;
  priority: ChatSessionTodoPriority;
  createdByRunCode?: string | null;
  updatedByRunCode?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ChatSessionCompaction {
  sessionCode: string;
  summaryCode: string;
  sourceRevision: number;
  coveredMessages: number;
  summary: string;
  createdAt: number;
  reason?: string | null;
  note?: string | null;
}

export interface CreateChatCompactionInput {
  sessionCode: string;
  modelCode?: string | null;
  reason?: string | null;
  note?: string | null;
}

export type ChatSessionGoalStatus =
  | "ACTIVE"
  | "PAUSED"
  | "BLOCKED"
  | "COMPLETED"
  | "CANCELLED"
  | "CLEARED";

export interface ChatSessionGoal {
  id: number | string;
  goalCode: string;
  sessionCode: string;
  workspaceCode: string;
  userCode?: string | null;
  taskCode?: string | null;
  activeFamilyCode?: string | null;
  objectiveMarkdown: string;
  status: ChatSessionGoalStatus;
  runsUsed: number;
  maxRuns: number;
  lastRunCode?: string | null;
  lastInputCode?: string | null;
  lastVerdict?: string | null;
  lastReason?: string | null;
  blockedReason?: string | null;
  requiredUserInput?: string | null;
  consecutiveJudgeFailures: number;
  createdByRunCode?: string | null;
  createdAt: string;
  updatedAt: string;
  pausedAt?: string | null;
  blockedAt?: string | null;
  completedAt?: string | null;
  cancelledAt?: string | null;
  clearedAt?: string | null;
  metadataJson?: Record<string, unknown> | null;
}

export interface ChatSessionGoalView {
  goal: ChatSessionGoal;
}

export interface SetChatSessionGoalInput {
  sessionCode: string;
  objective: string;
  maxRuns?: number | null;
}

export interface UpdateChatSessionGoalInput {
  sessionCode: string;
  goalCode: string;
  objective: string;
  maxRuns?: number | null;
}

export interface ChatSessionGoalCommand {
  sessionCode: string;
  goalCode?: string | null;
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
  | "ACTIVITY_DELTA"
  | string;

export interface ChatAguiStreamEvent {
  type: ChatAguiStreamEventType;
  name?: string;
  value?: unknown;
  threadId?: string;
  runId?: string;
  supportsSteer?: boolean;
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
  result?: ChatSendResult;
  [key: string]: unknown;
}
