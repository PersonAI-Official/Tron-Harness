import * as Ui from "@personai/agent-ui-chat-core";
import { FetchTransportError, isChatSendResult, sessionAgentTagLabel, type ChatSession, type ChatMessage, type ChatMessageAttachment, type ChatSendResult, type ChatUserInputRequest, type ChatUserInputAnswer, type ChatAguiStreamEvent, type SendChatMessageInput } from "@tron/chat-core";

export type ArtifactPreviewKind = "image" | "pdf" | "html" | "text" | "unsupported";

export function resolveArtifactPreview(item: Pick<Ui.ChatArtifactItem, "display" | "mediaType">): {
  kind: ArtifactPreviewKind;
  mimeType: string;
} {
  let mimeType = item.mediaType?.split(";")[0]?.trim().toLowerCase() ?? "";
  if (!mimeType || mimeType === "application/octet-stream") {
    const extension = item.display.split(".").at(-1)?.toLowerCase() ?? "";
    mimeType = ({
      html: "text/html", htm: "text/html", pdf: "application/pdf",
      png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", webp: "image/webp",
      gif: "image/gif", svg: "image/svg+xml",
      txt: "text/plain", log: "text/plain", md: "text/markdown", markdown: "text/markdown",
      json: "application/json", xml: "application/xml", csv: "text/csv",
      tsv: "text/tab-separated-values", yaml: "text/yaml", yml: "text/yaml",
    } as Record<string, string>)[extension] ?? mimeType;
  }

  if (mimeType.startsWith("image/")) return { kind: "image", mimeType };
  if (mimeType === "application/pdf") return { kind: "pdf", mimeType };
  if (mimeType === "text/html" || mimeType === "application/xhtml+xml") return { kind: "html", mimeType };
  if (mimeType.startsWith("text/") || mimeType === "application/json" || mimeType.endsWith("+xml")) {
    return { kind: "text", mimeType };
  }
  return { kind: "unsupported", mimeType: mimeType || "application/octet-stream" };
}

/** Product fields remain host-owned; the public UI only reads its neutral contract. */
interface MappedSession extends Ui.ChatSession {
  metadata: { tron: ChatSession };
}
interface MappedInput extends Ui.SendChatMessageInput {
  metadata: { tron: SendChatMessageInput };
}

export function toUiSession(session: ChatSession, agentName?: string): MappedSession {
  return {
    id: session.id, sessionId: session.sessionCode, userId: session.userCode,
    groupId: session.taskCode, displayName: agentName || sessionAgentTagLabel(session.agentCode),
    title: session.title, status: session.status, lastRunId: session.lastRunCode,
    lastRunStatus: session.lastRunStatus, createdAt: session.createdAt, updatedAt: session.updatedAt,
    metadata: { tron: session, agentId:session.agentCode } as MappedSession["metadata"],
  };
}

function isMappedSession(session: Ui.ChatSession): session is MappedSession {
  const host = session.metadata?.tron;
  return typeof host === "object" && host !== null
    && "sessionCode" in host && host.sessionCode === session.sessionId
    && "workspaceCode" in host && (typeof host.workspaceCode === "string" || host.workspaceCode === null);
}

export function fromUiSession(session: Ui.ChatSession): ChatSession {
  if (!isMappedSession(session)) throw new Error("Chat session is missing its Tron host context");
  return {
    ...session.metadata.tron, title: session.title, status: session.status,
    lastRunCode: session.lastRunId, lastRunStatus: session.lastRunStatus,
    updatedAt: session.updatedAt,
  };
}

export function toUiAttachment(value: ChatMessageAttachment): Ui.ChatMessageAttachment {
  const { resourceCode, ...rest } = value;
  return { ...rest, resourceId: resourceCode };
}
export function fromUiAttachment(value: Ui.ChatMessageAttachment): ChatMessageAttachment {
  const { resourceId, ...rest } = value;
  return { ...rest, resourceCode: resourceId };
}
export function toUiMessage(value: ChatMessage): Ui.ChatMessage {
  const { messageCode, sessionCode, runCode, attachments, artifacts, ...rest } = value;
  return { ...rest, messageId: messageCode, sessionId: sessionCode, runId: runCode,
    ...(artifacts ? { artifacts } : {}),
    attachments: attachments?.map(toUiAttachment) };
}
export function fromUiAnswer(value: Ui.ChatUserInputAnswer): ChatUserInputAnswer {
  const { questionId, optionId, ...rest } = value;
  return { ...rest, questionCode: questionId, optionCode: optionId };
}
function toUiAnswer(value: ChatUserInputAnswer): Ui.ChatUserInputAnswer {
  const { questionCode, optionCode, ...rest } = value;
  return { ...rest, questionId: questionCode, optionId: optionCode };
}
export function toUiQuestion(value: ChatUserInputRequest): Ui.ChatUserInputRequest {
  const { requestCode, sessionCode, runCode, assistantMessageCode, questions, answers, ...rest } = value;
  return { ...rest, requestId: requestCode, sessionId: sessionCode, runId: runCode,
    assistantMessageId: assistantMessageCode,
    questions: questions.map(({ questionCode, options, ...question }) => ({
      ...question, questionId: questionCode,
      options: options.map(({ optionCode, ...option }) => ({ ...option, optionId: optionCode })),
    })), answers: answers == null ? answers : answers.map(toUiAnswer),
  };
}
export function toUiResult(value: ChatSendResult, agentName?: string): Ui.ChatSendResult {
  const { runCode, conversationCode, workspaceCode, systemAgentSessionId, ...run } = value.run;
  return { session: toUiSession(value.session, agentName), messages: value.messages.map((message) => toUiMessageWithAgent(message, agentName)),
    run: { ...run, runId: runCode, conversationId: conversationCode,
      metadata: { workspaceCode, systemAgentSessionId } },
    userInputRequest: value.userInputRequest ? toUiQuestion(value.userInputRequest) : value.userInputRequest };
}
export function toUiMessageWithAgent(value: ChatMessage, agentName?: string): Ui.ChatMessage {
  const message = toUiMessage(value);
  return agentName && message.role === "ASSISTANT" && !message.provenance && (!message.actor || message.actor === "Assistant")
    ? { ...message, actor: agentName } : message;
}

export function toUiHistory(value: Ui.SessionHistory & { compactionDetails?: Record<string, unknown> }, agentName?: string): Ui.SessionHistory {
  const messages = value.records.messages.map((message) => {
    const structured = message.structured;
    // Older external runs stored the entire Tron result as assistant output.
    const internalResult = message.role === "Assistant" && isRecord(structured)
      && structured.runCode === message.runId && Array.isArray(structured.observations);
    return { ...message, ...(internalResult ? { structured: null } : {}),
      ...(agentName && message.role === "Assistant" && !message.provenance
        ? { provenance: { sender: { kind: "agent", displayName: agentName } } } : {}) };
  });
  // A run can fail before it produces any transcript record. The live failure
  // notice is transient, so project the failed run once more into history: the
  // public renderer keeps an assistant message's run error visible after reload.
  const assistantRuns = new Set(messages
    .filter((message) => message.role === "Assistant").map((message) => message.runId));
  const failedRuns = value.runs
    .filter((run) => run.status === "Failed" && run.error && !assistantRuns.has(run.id))
    .map((run): Ui.HistoryMessage => ({
      annotations: [], requestedSkills: [], id: `failed-run:${run.id}`, runId: run.id,
      stepId: null, role: "Assistant", phase: null, outputIndex: null,
      content: [], structured: null, status: "Failed",
      createdAtMs: run.finishedAtMs ?? run.startedAtMs,
    }));
  return { ...value, records: { ...value.records,
    messages: failedRuns.length
      ? [...messages, ...failedRuns].sort((left, right) => left.createdAtMs - right.createdAtMs)
      : messages,
    steps: value.records.steps.map((step) => {
      const blocks = Ui.parseCompactionBlocks(value.compactionDetails?.[step.id]);
      return {
        ...step, name: step.name === "framework.context.compaction" ? "agent.context.compaction" : step.name,
        ...(blocks ? { compaction: { blocks } } : {}),
      };
    }) } };
}
export function toUiInput(input: SendChatMessageInput): MappedInput {
  const model = input.modelSelection;
  return { content: input.content, annotations: input.annotations ?? [], requestedSkills: input.requestedSkills ?? [], modelId: input.modelCode,
    reasoningEffort: input.reasoningEffort, clientCapabilities: input.clientCapabilities, a2uiMetadata: input.a2uiMetadata,
    attachments: input.attachments?.map(toUiAttachment),
    modelSelection: model ? {
      kind: model.kind === "Fixed" ? "fixed" : model.kind === "ProviderRoute" ? "route" : "auto",
      providerId: model.providerCode, modelId: model.modelCode, routeId: model.routeCode,
    } : model,
    metadata: { tron: input, agentId: input.agentCode } as MappedInput["metadata"] };
}
function isMappedInput(input: Ui.SendChatMessageInput): input is MappedInput {
  const host = input.metadata?.tron;
  return typeof host === "object" && host !== null && "workspaceCode" in host
    && (typeof host.workspaceCode === "string" || host.workspaceCode === null);
}
export function fromUiInput(input: Ui.SendChatMessageInput): SendChatMessageInput {
  if (!isMappedInput(input)) throw new Error("Chat input is missing its Tron host context");
  const desktop = typeof window !== "undefined"
    && Boolean((window as { tronDesktop?: { invoke?: unknown } }).tronDesktop?.invoke);
  const artifacts = {
    actions: (desktop
      ? ["preview", "open", "reveal", "save"]
      : ["preview", "save"]) as ("preview" | "open" | "reveal" | "save")[],
  };
  return { ...input.metadata.tron, content: input.content, annotations: input.annotations ?? [], requestedSkills: input.requestedSkills ?? [], clientCapabilities: { ...(input.clientCapabilities ?? {}), artifacts }, a2uiMetadata: input.a2uiMetadata,
    attachments: input.attachments?.map(fromUiAttachment) };
}

const eventNames: Record<string, string> = {
  "tron.short_term_memory.context.compaction_requested": "agent.memory.context.compaction_requested",
  "tron.short_term_memory.compaction.started": "agent.memory.compaction.started",
  "tron.short_term_memory.compaction.completed": "agent.memory.compaction.completed",
  "tron.short_term_memory.compaction.failed": "agent.memory.compaction.failed",
};
type ProtocolEvent = Pick<ChatAguiStreamEvent, "type" | "outcome" | "threadId" | "runId" | "timestamp"> & {
  result?: unknown;
  [key: string]: unknown;
};

function mapProtocolResult(event: ProtocolEvent, result: unknown, agentName?: string): unknown {
  const productCompletion = event.productCompletion === true && !isRecord(event.productCompletionError)
    && (event.type === "RUN_FINISHED" || result !== undefined);
  // Unmarked native JSON may use these keys too; only a valid product envelope is legacy.
  const validProduct = isChatSendResult(result);
  if (!productCompletion && !validProduct) return result;
  if (!validProduct) throw new Error("Invalid product completion result");
  return toUiResult(result, agentName);
}

export function toUiEvent(event: ProtocolEvent, agentName?: string): Ui.ChatAguiStreamEvent {
  const { result, ...rest } = event;
  const host = isRecord(event.metadata) && isRecord(event.metadata.tron) ? event.metadata.tron : null;
  const traceId = typeof host?.traceId === "string" ? host.traceId : "";
  const diagnostic = /^[a-f0-9]{32}$/.test(traceId) && !/^0+$/.test(traceId) ? Ui.parseChatRunDiagnostic({
    traceId,
    href: `/settings/observability/traces?${new URLSearchParams({
      category: "Agent", traceId,
      ...(typeof host?.workspaceCode === "string" ? { workspaceCode: host.workspaceCode } : {}),
    })}`,
  }) : undefined;
  return { ...rest, ...(diagnostic ? { diagnostic } : {}), ...(typeof event.name === "string" ? { name: eventNames[event.name] ?? event.name } : {}),
    outcome: event.outcome?.type === "interrupt" ? {
      ...event.outcome,
      interrupts: event.outcome.interrupts.map((entry) => ({
        ...entry, metadata: entry.metadata ? mapQuestionMetadata(entry.metadata) : entry.metadata,
      })),
    } : event.outcome,
    result: mapProtocolResult(event, result, agentName) };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
function mapQuestionMetadata(value: Record<string, unknown>): Record<string, unknown> {
  return { ...value, requestId: value.requestCode ?? value.requestId,
    questions: Array.isArray(value.questions) ? value.questions.map((question: unknown) => {
      if (!isRecord(question)) return question;
      return { ...question, questionId: question.questionCode ?? question.questionId,
        options: Array.isArray(question.options) ? question.options.map((option: unknown) =>
          isRecord(option) ? { ...option, optionId: option.optionCode ?? option.optionId } : option) : question.options };
    }) : value.questions };
}

export function fromUiResume(entries: Ui.ChatAguiResumeEntry[]): Ui.ChatAguiResumeEntry[] {
  return entries.map((entry) => {
    if (!isRecord(entry.payload) || !Array.isArray(entry.payload.answers)) return entry;
    return { ...entry, payload: { ...entry.payload, answers: entry.payload.answers.map((answer: unknown) => {
      if (!isRecord(answer) || typeof answer.questionId !== "string") throw new Error("Invalid terminal question answer");
      const { questionId, optionId, ...rest } = answer;
      return { ...rest, questionCode: questionId, optionCode: optionId };
    }) } };
  });
}

export async function uiRequest<T>(request: Promise<T>): Promise<T> {
  try { return await request; } catch (error) {
    if (error instanceof FetchTransportError) {
      throw new Ui.ChatAdapterError(Ui.chatErrorPresentation(error.message, typeof document !== "undefined" && document.documentElement.lang.toLowerCase().startsWith("en") ? "Run failed" : "运行失败", error.code).summary, {
        code: error.code === "CHAT_RUN_ALREADY_ACTIVE" ? "RUN_ALREADY_ACTIVE" : error.code?.replace(/^CHAT_STEER_/, "STEER_"),
        details: error.data, status: error.status,
        // These composer errors are emitted before a user message is persisted.
        // Never infer safe restoration from an arbitrary HTTP 4xx alone.
        rejectedBeforeAdmission: ["101032001", "101032002", "101032003", "101032004", "101032006", "CHAT_DIRECTORY_INVALID"].includes(error.code ?? ""),
      });
    }
    throw error;
  }
}
