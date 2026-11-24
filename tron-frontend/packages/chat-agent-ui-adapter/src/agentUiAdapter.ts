import type * as Ui from "@personai/agent-ui-chat-core";
import type { ChatAdapter, ChatClientConfig, ChatStreamHandlers } from "@tron/chat-core";
import { fromUiAnswer, fromUiAttachment, fromUiInput, fromUiResume, fromUiSession, toUiAttachment, toUiEvent, toUiHistory, toUiMessage, toUiMessageWithAgent, toUiQuestion, toUiResult, toUiSession, uiRequest } from "./agentUiMapping";

/** Transport/auth/locale stay in the existing product adapter. No API is called by UI components. */
export function createAgentUiAdapter(host: ChatAdapter, config: ChatClientConfig, identity?: { agentCode: string; name: string; sessionId?: string }): Ui.ChatAdapter {
  const nameForSession = (sessionId: string) => identity && (!identity.sessionId || identity.sessionId === sessionId) ? identity.name : undefined;
  const nameForResult = (result: { session: { agentCode?: string | null; sessionCode: string } }) =>
    result.session.agentCode === identity?.agentCode ? nameForSession(result.session.sessionCode) : undefined;
  const handlers = (target?: Ui.ChatStreamHandlers): ChatStreamHandlers => ({
    onDelta: target?.onDelta,
    onAguiEvent: (event) => target?.onAguiEvent?.(toUiEvent(event, event.threadId ? nameForSession(event.threadId) : undefined)),
  });
  const workspaceCodeForSession = (session: Ui.ChatSession) => {
    const metadata = session.metadata as { tron?: { workspaceCode?: unknown } } | undefined;
    const workspaceCode = metadata?.tron?.workspaceCode;
    return typeof workspaceCode === "string" ? workspaceCode.trim() : "";
  };
  const artifactContentUrl = (session: Ui.ChatSession, input: { runId?: string | null; itemId: string; disposition: "inline" | "attachment" }) => {
    const query = new URLSearchParams({ disposition: input.disposition });
    const workspaceCode = workspaceCodeForSession(session);
    if (workspaceCode) query.set("workspaceCode", workspaceCode);
    return `/api/v1/chats/sessions/${encodeURIComponent(session.sessionId)}/artifacts/${encodeURIComponent(input.runId ?? "")}/${encodeURIComponent(input.itemId)}/content?${query}`;
  };
  return {
    artifactContentUrl,
    artifactAction: async (session, input) => {
      const host = window as unknown as {
        tronDesktop?: { invoke: (command: string, payload?: unknown) => Promise<unknown> };
        __tronAccessToken?: string;
      };
      const token = host.__tronAccessToken;
      if (host.tronDesktop?.invoke && token) {
        const command = input.action === "open"
          ? "artifact:open"
          : input.action === "reveal" ? "artifact:reveal" : "artifact:save";
        try {
          const value = await host.tronDesktop.invoke(command, {
            sessionCode: session.sessionId,
            runCode: input.runId ?? "",
            itemId: input.itemId,
            workspaceCode: workspaceCodeForSession(session),
            token,
            suggestedName: input.itemId,
          }) as { status?: string; message?: string } | null;
          if (value?.status === "saved" || value?.status === "opened" || value?.status === "revealed") {
            return { status: "completed" as const };
          }
          if (value?.status === "cancelled") return { status: "cancelled" as const };
          return { status: "failed" as const, message: value?.message ?? "Artifact action failed" };
        } catch (error) {
          return { status: "failed" as const, message: error instanceof Error ? error.message : "Artifact action failed" };
        }
      }
      if (input.action !== "save") {
        return { status: "failed" as const, message: "Artifact action is unavailable on this host" };
      }
      const anchor = document.createElement("a");
      anchor.href = artifactContentUrl(session, { runId: input.runId, itemId: input.itemId, disposition: "attachment" });
      anchor.download = "";
      document.body.append(anchor);
      anchor.click();
      anchor.remove();
      return { status: "completed" as const };
    },
    capabilities: host.capabilities,
    createSession: async (input) => {
      if (!config.workspaceCode?.trim()) throw new Error("Workspace code is required before starting a session");
      const session = await uiRequest(host.createSession({
        workspaceCode: config.workspaceCode, appCode: config.appCode, taskCode: config.taskCode,
        agentCode: config.agentCode, title: input.title,
      }));
      return toUiSession(session, identity && session.agentCode === identity.agentCode ? identity.name : undefined);
    },
    deleteSession: async (...args) => toUiSession(await uiRequest(host.deleteSession(...args))),
    loadA2uiState: host.loadA2uiState ? (...args) => uiRequest(host.loadA2uiState!(...args)) : undefined,
    sendA2uiActionStream: host.sendA2uiActionStream ? async (session,input,events,signal) => { const result = await uiRequest(host.sendA2uiActionStream!(fromUiSession(session),input,handlers(events),signal)); return toUiResult(result, nameForResult(result)); } : undefined,
    loadMessages: async (id) => (await uiRequest(host.loadMessages(id))).map((message) => toUiMessageWithAgent(message, nameForSession(id))),
    deleteTurn: host.deleteTurn ? (...args) => uiRequest(host.deleteTurn!(...args)) : undefined,
    loadHistory: host.loadHistory ? async (...args) => toUiHistory(await uiRequest(host.loadHistory!(...args)), nameForSession(args[0])) : undefined,
    loadAguiEvents: async (id) => (await uiRequest(host.loadAguiEvents(id))).map(({ runCode, ...event }) => ({ ...event, runId: runCode })),
    loadProtocolEvents: host.loadProtocolEvents ? async (id) => (await uiRequest(host.loadProtocolEvents!(id))).map((event) => toUiEvent(event, nameForSession(id))) : undefined,
    loadUserInputRequests: host.loadUserInputRequests ? async (...args) => (await uiRequest(host.loadUserInputRequests!(...args))).map(toUiQuestion) : undefined,
    getActiveRun: host.getActiveRun ? async (...args) => {
      const value = await uiRequest(host.getActiveRun!(...args));
      return { sessionId: value.sessionCode, activeRun: value.activeRun ? {
        runId: value.activeRun.runCode, runtimeAdapterKey: value.activeRun.runtimeAdapterKey,
        state: value.activeRun.state, supportsSteer: value.activeRun.supportsSteer,
        ...(value.activeRun.operationKind ? { operationKind: value.activeRun.operationKind } : {}),
      } : null };
    } : undefined,
    streamMessage: async (session, input, events, signal) => {
      const result = await uiRequest(host.streamMessage(fromUiSession(session), fromUiInput(input), handlers(events), signal));
      return toUiResult(result, nameForResult(result));
    },
    steerMessage: host.steerMessage ? async (session, input, context, signal) => {
      const value = await uiRequest(host.steerMessage!(fromUiSession(session), fromUiInput(input),
        { expectedRunCode: context.expectedRunId }, signal));
      return { sessionId: value.sessionCode, runId: value.runCode, message: toUiMessage(value.message) };
    } : undefined,
    stopRun: host.stopRun ? async (session, context, signal) => {
      const value = await uiRequest(host.stopRun!(fromUiSession(session), { expectedRunCode: context.expectedRunId }, signal));
      return { sessionId: value.sessionCode, runId: value.runCode };
    } : undefined,
    uploadAttachment: host.uploadAttachment ? async (session, input, signal) =>
      toUiAttachment(await uiRequest(host.uploadAttachment!(fromUiSession(session), input, signal))) : undefined,
    downloadAttachment: host.downloadAttachment ? (session, attachment, signal) =>
      uiRequest(host.downloadAttachment!(fromUiSession(session), fromUiAttachment(attachment), signal)) : undefined,
    answerUserInputRequestStream: host.answerUserInputRequestStream ? async (session, id, input, events, signal) => {
      const answers = input.answers.map(fromUiAnswer);
      // The payload is a product API contract too; mapping only the outer answers loses resume data.
      const resume = input.resume ? (input.answers.length ? fromUiResume(input.resume) : input.resume) : undefined;
      const result = await uiRequest(host.answerUserInputRequestStream!(fromUiSession(session), id,
        { answers, resume, clientCapabilities:input.clientCapabilities, a2uiMetadata:input.a2uiMetadata }, handlers(events), signal));
      return toUiResult(result, nameForResult(result));
    } : undefined,
  };
}
