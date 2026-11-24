import type {
  ChatAguiEvent,
  ChatContextBudgetBreakdown,
  ChatMessage,
  ChatSendResult,
  ChatTokenEstimateSnapshot,
  ChatUsageAuditSnapshot,
  ChatSessionCompaction,
  ChatSessionGoalView,
  ChatSession,
  ChatSessionGroup,
  ChatSessionTodo,
  ChatSessionGoalCommand,
  CreateChatCompactionInput,
  CreateChatSessionInput,
  SendChatMessageInput,
  SetChatSessionGoalInput,
  UpdateChatSessionGoalInput,
  UpdateChatSessionInput,
} from "@/domain/chat/model/ChatModel";

const baseTime = "2026-05-27T09:30:00Z";

const mockContextBudgetBreakdown: ChatContextBudgetBreakdown = {
  checkpointBudgetTokens: 10_000,
  checkpointTokens: 3_900,
  compressedHistoryMessageCount: 0,
  compressedHistoryTokens: 0,
  compactionTriggerRatioBps: 8_000,
  compactionTriggerReached: false,
  compactionTriggerTokens: 89_600,
  contextWindowLimit: 128_000,
  currentSessionHistoryBudgetTokens: 16_000,
  effectiveInputBudget: 112_000,
  effectiveRetainedHistoryTokens: 8_000,
  estimatedInputTokens: 22_600,
  historySourceMessageCount: 18,
  inputBudgetExceeded: false,
  outputTokenReserve: 16_000,
  promptPrefixBudgetTokens: 4_000,
  promptPrefixTokens: 2_100,
  rawTailMessageCount: 18,
  rawTailStartSeq: 1,
  rawTailTranscriptTokens: 10_800,
  recentTranscriptTokens: 10_800,
  retainedHistoryRatioBps: 5_000,
  toolSchemaBudgetTokens: 6_000,
  toolSchemaTokens: 1_800,
  workingStateBudgetTokens: 5_000,
  workingStateTokens: 1_400,
};

const mockRequestEstimate: ChatTokenEstimateSnapshot = {
  checkpointCode: "ATM-MOCK-1",
  compactionTriggerReached: false,
  contextWindowLimit: 128_000,
  effectiveInputBudget: 112_000,
  estimatedInputTokens: 22_600,
  messageTokenEstimate: 20_800,
  modelCode: "mock-gpt-5-codex",
  modelName: "gpt-5-codex",
  modelProtocol: "responses",
  outputTokenReserve: 16_000,
  overBudget: false,
  rebased: true,
  toolSchemaTokenEstimate: 1_800,
  usageConfidence: "estimated",
  usageSource: "local_estimate",
};

const mockUsageAudit: ChatUsageAuditSnapshot = {
  cumulativeRunUsage: {
    inputTokens: 24_100,
    outputTokens: 2_400,
    totalTokens: 26_500,
  },
  inputTokenDelta: 1_500,
  inputTokenRatio: 1.066,
  providerUsage: {
    cachedInputTokens: 6_800,
    inputTokens: 24_100,
    outputTokens: 2_400,
    reasoningTokens: 1_900,
    totalTokens: 26_500,
  },
  requestEstimate: mockRequestEstimate,
  responseId: "resp_mock",
  usageConfidence: "actual",
  usageSource: "provider",
};

const sessions: ChatSession[] = [
  {
    id: 735200000000000001,
    sessionCode: "SESSION-8842",
    workspaceCode: "TRON_CORE",
    userCode: "USER-AUTH",
    appCode: "TRON",
    taskCode: "TASK-8842",
    sourceCode: "TASK-8842",
    sourceType: "Task",
    agentCode: "main-agent",
    systemAgentSessionId: null,
    title: "Primary investigation",
    status: "ACTIVE",
    lastRunCode: "RUN-42",
    lastRunStatus: "COMPLETED",
    createdAt: baseTime,
    updatedAt: "2026-05-27T10:44:00Z",
  },
  {
    id: 735200000000000002,
    sessionCode: "SESSION-REFACTOR",
    workspaceCode: "TRON_CORE",
    userCode: "USER-AUTH",
    appCode: "TRON",
    taskCode: "TASK-8842",
    sourceCode: "TASK-8842",
    sourceType: "Task",
    agentCode: "codex",
    systemAgentSessionId: "thr_codex_mock_39",
    title: "Refactor discussion",
    status: "ACTIVE",
    lastRunCode: "RUN-39",
    lastRunStatus: "COMPLETED",
    createdAt: baseTime,
    updatedAt: "2026-05-27T09:50:00Z",
  },
  {
    id: 735200000000000003,
    sessionCode: "SESSION-UNASSIGNED",
    workspaceCode: "TRON_CORE",
    userCode: "USER-AUTH",
    appCode: null,
    taskCode: null,
    sourceCode: null,
    sourceType: "Manual",
    agentCode: "main-agent",
    systemAgentSessionId: null,
    title: "Historical trace review",
    status: "ACTIVE",
    lastRunCode: null,
    lastRunStatus: null,
    createdAt: baseTime,
    updatedAt: "2026-05-27T08:20:00Z",
  },
];

const messagesBySession: Record<string, ChatMessage[]> = {
  "SESSION-8842": [
    {
      id: 1,
      messageCode: "MSG-1",
      sessionCode: "SESSION-8842",
      runCode: "RUN-42",
      role: "USER",
      channel: "TASK_AGENT",
      actor: "User",
      content: "Auth latency spiked after the token validator deployment. Find the narrowest rollback path.",
      createdAt: "2026-05-27T10:42:00Z",
    },
    {
      id: 2,
      messageCode: "MSG-2",
      sessionCode: "SESSION-8842",
      runCode: "RUN-42",
      role: "ASSISTANT",
      channel: "TASK_AGENT",
      actor: "Assistant",
      content: "The narrowest rollback is to restore the fast-path cache check behind a feature flag, then watch eu-west-1 and eu-central-1 latency before widening rollout.",
      inputTokens: 64,
      outputTokens: 42,
      totalTokens: 106,
      createdAt: "2026-05-27T10:44:00Z",
    },
  ],
  "SESSION-REFACTOR": [
    {
      id: 3,
      messageCode: "MSG-3",
      sessionCode: "SESSION-REFACTOR",
      runCode: "RUN-39",
      role: "USER",
      channel: "TASK_AGENT",
      actor: "User",
      content: "Keep the refactor focused on the connector boundary.",
      createdAt: "2026-05-27T09:47:00Z",
    },
    {
      id: 4,
      messageCode: "MSG-4",
      sessionCode: "SESSION-REFACTOR",
      runCode: "RUN-39",
      role: "ASSISTANT",
      channel: "TASK_AGENT",
      actor: "Assistant",
      content: "I will keep persistence and runtime dispatch untouched, and only move connector mapping behind the existing adapter seam.",
      inputTokens: 38,
      outputTokens: 23,
      totalTokens: 61,
      createdAt: "2026-05-27T09:50:00Z",
    },
  ],
  "SESSION-UNASSIGNED": [],
};

const todosBySession: Record<string, ChatSessionTodo[]> = {
  "SESSION-8842": [
    {
      id: 735210000000000001,
      todoCode: "TODO-MOCK-1",
      sessionCode: "SESSION-8842",
      workspaceCode: "TRON_CORE",
      position: 0,
      content: "Validate transcript storage after the next chat turn",
      status: "in_progress",
      priority: "high",
      createdByRunCode: "RUN-42",
      updatedByRunCode: "RUN-42",
      createdAt: "2026-05-27T10:42:30Z",
      updatedAt: "2026-05-27T10:44:00Z",
    },
    {
      id: 735210000000000002,
      todoCode: "TODO-MOCK-2",
      sessionCode: "SESSION-8842",
      workspaceCode: "TRON_CORE",
      position: 1,
      content: "Review compaction checkpoint output",
      status: "pending",
      priority: "medium",
      createdByRunCode: "RUN-42",
      updatedByRunCode: "RUN-42",
      createdAt: "2026-05-27T10:43:00Z",
      updatedAt: "2026-05-27T10:44:00Z",
    },
  ],
  "SESSION-REFACTOR": [],
  "SESSION-UNASSIGNED": [],
};

const compactionsBySession: Record<string, ChatSessionCompaction | null> = {
  "SESSION-8842": {
    sessionCode: "SESSION-8842",
    summaryCode: "summary-mock-1",
    sourceRevision: 8,
    coveredMessages: 8,
    summary: "Reviewed the implementation and its current progress.",
    createdAt: Date.parse("2026-05-27T10:45:00Z"),
  },
  "SESSION-REFACTOR": null,
  "SESSION-UNASSIGNED": null,
};

const goalsBySession: Record<string, ChatSessionGoalView | null> = {
  "SESSION-8842": {
    goal: {
      id: 735220000000000001,
      goalCode: "GOAL-MOCK-1",
      sessionCode: "SESSION-8842",
      workspaceCode: "TRON_CORE",
      userCode: "USER-AUTH",
      taskCode: "TASK-8842",
      activeFamilyCode: null,
      objectiveMarkdown: "Finish the auth latency rollback investigation with evidence.",
      status: "ACTIVE",
      runsUsed: 2,
      maxRuns: 20,
      lastRunCode: "RUN-42",
      lastInputCode: null,
      lastVerdict: "CONTINUE",
      lastReason: "Rollback path is identified; verification evidence is still pending.",
      blockedReason: null,
      requiredUserInput: null,
      consecutiveJudgeFailures: 0,
      createdByRunCode: "RUN-42",
      createdAt: "2026-05-27T10:40:00Z",
      updatedAt: "2026-05-27T10:44:00Z",
      pausedAt: null,
      blockedAt: null,
      completedAt: null,
      cancelledAt: null,
      clearedAt: null,
      metadataJson: null,
    },
  },
  "SESSION-REFACTOR": null,
  "SESSION-UNASSIGNED": null,
};

export function getChatSessionGroups(): ChatSessionGroup[] {
  const taskCodes = Array.from(new Set(
    sessions
      .filter((session) => session.taskCode && session.status === "ACTIVE")
      .map((session) => session.taskCode as string),
  ));
  return [
    ...taskCodes.map((taskCode) => {
      const taskSessions = sessions.filter((session) => session.taskCode === taskCode && session.status === "ACTIVE");
      return {
        taskCode,
        taskTitle: taskCode === "TASK-8842" ? "Auth latency rollback" : taskSessions[0]?.title ?? taskCode,
        sourceCode: taskCode,
        sourceType: "Task",
        sessions: taskSessions,
      };
    }),
    {
      taskCode: null,
      taskTitle: "Chat",
      sourceCode: null,
      sourceType: "Manual",
      sessions: sessions.filter((session) => !session.taskCode && session.status === "ACTIVE"),
    },
  ];
}

export function getChatMessages(sessionCode: string): ChatMessage[] {
  return messagesBySession[sessionCode] ?? [];
}

export function getMockChatSessionTodos(sessionCode: string): ChatSessionTodo[] {
  return [...(todosBySession[sessionCode] ?? [])];
}

export function getMockChatLatestCompaction(sessionCode: string): ChatSessionCompaction | null {
  const compaction = compactionsBySession[sessionCode] ?? null;
  return compaction ? { ...compaction } : null;
}

export function createMockChatCompaction(input: CreateChatCompactionInput): ChatSessionCompaction {
  const now = Date.now();
  const messages = messagesBySession[input.sessionCode] ?? [];
  const compaction: ChatSessionCompaction = {
    sessionCode: input.sessionCode,
    summaryCode: `summary-mock-${now}`,
    sourceRevision: messages.length,
    coveredMessages: messages.length,
    summary: "Conversation progress summary.",
    createdAt: now,
  };
  compactionsBySession[input.sessionCode] = compaction;
  return { ...compaction };
}

export function getMockChatSessionGoal(sessionCode: string): ChatSessionGoalView | null {
  const goal = goalsBySession[sessionCode] ?? null;
  if (!goal) return null;
  return ["ACTIVE", "PAUSED", "BLOCKED"].includes(goal.goal.status) ? cloneGoalView(goal) : null;
}

export function getMockChatSessionLatestGoal(sessionCode: string): ChatSessionGoalView | null {
  const goal = goalsBySession[sessionCode] ?? null;
  return goal ? cloneGoalView(goal) : null;
}

export function setMockChatSessionGoal(input: SetChatSessionGoalInput): ChatSessionGoalView {
  const session = sessions.find((item) => item.sessionCode === input.sessionCode);
  if (!session) throw new Error("chat session was not found");
  if (!input.objective.trim()) throw new Error("goal objective is required");
  if (session.workspaceCode === null) throw new Error("Mock workspace goals require a workspace");
  const now = new Date().toISOString();
  const goalCode = `GOAL-MOCK-${Date.now()}`;
  const goal: ChatSessionGoalView = {
    goal: {
      id: Date.now(),
      goalCode,
      sessionCode: input.sessionCode,
      workspaceCode: session.workspaceCode,
      userCode: session.userCode,
      taskCode: session.taskCode,
      activeFamilyCode: null,
      objectiveMarkdown: input.objective.trim(),
      status: "ACTIVE",
      runsUsed: 0,
      maxRuns: input.maxRuns ?? 20,
      lastRunCode: null,
      lastInputCode: null,
      lastVerdict: null,
      lastReason: null,
      blockedReason: null,
      requiredUserInput: null,
      consecutiveJudgeFailures: 0,
      createdByRunCode: null,
      createdAt: now,
      updatedAt: now,
      pausedAt: null,
      blockedAt: null,
      completedAt: null,
      cancelledAt: null,
      clearedAt: null,
      metadataJson: { outerLoopStatus: "not_started", startPolicy: "after_next_run" },
    },
  };
  goalsBySession[input.sessionCode] = goal;
  return cloneGoalView(goal);
}

export function transitionMockChatSessionGoal(
  input: ChatSessionGoalCommand,
  status: ChatSessionGoalView["goal"]["status"],
): ChatSessionGoalView {
  const goal = goalsBySession[input.sessionCode];
  if (!goal || (input.goalCode && goal.goal.goalCode !== input.goalCode)) {
    throw new Error("session goal was not found");
  }
  goal.goal.status = status;
  goal.goal.updatedAt = new Date().toISOString();
  return cloneGoalView(goal);
}

export function updateMockChatSessionGoal(input: UpdateChatSessionGoalInput): ChatSessionGoalView {
  const goal = goalsBySession[input.sessionCode];
  if (!goal || goal.goal.goalCode !== input.goalCode) {
    throw new Error("session goal was not found");
  }
  if (!["ACTIVE", "PAUSED", "BLOCKED"].includes(goal.goal.status)) {
    throw new Error("goal command is invalid");
  }
  const objective = input.objective.trim();
  if (!objective) throw new Error("goal objective is required");
  const now = new Date().toISOString();
  goal.goal.objectiveMarkdown = objective;
  goal.goal.maxRuns = Math.max(1, input.maxRuns ?? goal.goal.maxRuns);
  goal.goal.updatedAt = now;
  return cloneGoalView(goal);
}

function cloneGoalView(goal: ChatSessionGoalView): ChatSessionGoalView {
  return {
    goal: { ...goal.goal },
  };
}

export function getChatAguiEvents(sessionCode: string): ChatAguiEvent[] {
  const session = sessions.find((item) => item.sessionCode === sessionCode);
  if (!session?.lastRunCode) return [];
  return [
    {
      id: "EVT-1",
      type: "agent.run.started",
      eventName: "agent.run.started",
      runCode: session.lastRunCode,
      streamKey: sessionCode,
      occurredAt: session.updatedAt,
      outcome: session.lastRunStatus ?? "COMPLETED",
      metadata: {
        appCode: session.appCode ?? "unavailable",
        budgetBreakdown: JSON.stringify(mockContextBudgetBreakdown),
        taskCode: session.taskCode ?? "unavailable",
        localDirectory: "/Users/clouds3n/.codex/worktrees/Tron/feature/clouds3n/chat-management/base",
        branchName: "feature/clouds3n/chat-management/base",
        usage_audit_json: JSON.stringify(mockUsageAudit),
        usage_estimate_json: JSON.stringify(mockRequestEstimate),
      },
    },
    {
      id: "EVT-2",
      type: "agent.skill.used",
      eventName: "agent.skill.used",
      runCode: session.lastRunCode,
      streamKey: sessionCode,
      occurredAt: session.updatedAt,
      outcome: "COMPLETED",
      metadata: {
        skillName: "eliteforge-codex-superpower",
      },
    },
    {
      id: "EVT-3",
      type: "agent.compaction.completed",
      eventName: "agent.compaction.completed",
      runCode: session.lastRunCode,
      streamKey: sessionCode,
      occurredAt: session.updatedAt,
      outcome: "COMPLETED",
      metadata: {
        context_event_json: JSON.stringify({
          checkpointMessageCode: "ATM-MOCK-1",
          summaryTokenEstimate: 840,
        }),
      },
    },
  ];
}

export function createMockChatSession(input: CreateChatSessionInput): ChatSession {
  const now = new Date().toISOString();
  const id = Date.now();
  const session: ChatSession = {
    executionMode: input.executionMode ?? "CONTROLLED",
    id,
    sessionCode: `SESSION-${id}`,
    workspaceCode: input.workspaceCode,
    userCode: "USER-AUTH",
    appCode: input.appCode ?? null,
    taskCode: input.taskCode ?? null,
    agentCode: input.agentCode ?? "main-agent",
    systemAgentSessionId: null,
    title: input.title?.trim() || "New chat",
    status: "ACTIVE",
    lastRunCode: null,
    lastRunStatus: null,
    createdAt: now,
    updatedAt: now,
  };
  sessions.unshift(session);
  messagesBySession[session.sessionCode] = [];
  todosBySession[session.sessionCode] = [];
  compactionsBySession[session.sessionCode] = null;
  goalsBySession[session.sessionCode] = null;
  return session;
}

export function appendMockTaskRunTranscript(
  sessionCode: string,
  runCode: string,
  prompt: string,
  finalOutput: string,
): void {
  const session = sessions.find((item) => item.sessionCode === sessionCode);
  if (!session) return;
  const now = new Date().toISOString();
  const channel = session.taskCode ? "TASK_AGENT" : "MAIN_AGENT";
  const userMessage: ChatMessage = {
    id: Date.now(),
    messageCode: `MSG-${Date.now()}`,
    sessionCode,
    runCode,
    role: "USER",
    channel,
    actor: "User",
    content: prompt,
    createdAt: now,
  };
  const assistantMessage: ChatMessage = {
    id: Date.now() + 1,
    messageCode: `MSG-${Date.now() + 1}`,
    sessionCode,
    runCode,
    role: "ASSISTANT",
    channel,
    actor: "Assistant",
    content: finalOutput,
    createdAt: now,
  };
  session.lastRunCode = runCode;
  session.lastRunStatus = "COMPLETED";
  session.updatedAt = now;
  messagesBySession[sessionCode] = [
    ...(messagesBySession[sessionCode] ?? []),
    userMessage,
    assistantMessage,
  ];
}

export function archiveMockChatSession(sessionCode: string): ChatSession {
  const session = sessions.find((item) => item.sessionCode === sessionCode);
  if (!session) {
    throw new Error("chat session was not found");
  }

  session.status = "ARCHIVED";
  session.updatedAt = new Date().toISOString();
  return { ...session };
}

export function restoreMockChatSession(sessionCode: string): ChatSession {
  const session = sessions.find((item) => item.sessionCode === sessionCode);
  if (!session) throw new Error("chat session was not found");
  session.status = "ACTIVE";
  session.updatedAt = new Date().toISOString();
  return { ...session };
}

export function deleteMockChatSession(sessionCode: string): ChatSession {
  const sessionIndex = sessions.findIndex((item) => item.sessionCode === sessionCode);
  if (sessionIndex < 0) {
    throw new Error("chat session was not found");
  }

  const [session] = sessions.splice(sessionIndex, 1);
  delete messagesBySession[sessionCode];
  delete todosBySession[sessionCode];
  delete compactionsBySession[sessionCode];
  delete goalsBySession[sessionCode];
  return { ...session };
}

export function updateMockChatSession(sessionCode: string, input: UpdateChatSessionInput): ChatSession {
  const session = sessions.find((item) => item.sessionCode === sessionCode);
  if (!session) {
    throw new Error("chat session was not found");
  }

  const title = input.title?.trim();
  if (input.title !== undefined && !title) {
    throw new Error("session title is required");
  }

  if (title !== undefined) session.title = title;
  if (input.executionMode !== undefined) session.executionMode = input.executionMode;
  session.updatedAt = new Date().toISOString();
  return { ...session };
}

export function createMockChatSendResult(session: ChatSession, input: SendChatMessageInput): ChatSendResult {
  const now = new Date().toISOString();
  const runCode = `RUN-${Date.now()}`;
  const finalOutput = `**Recorded in ${session.sessionCode}:**\n\n${input.content}`;
  const agentCode = input.agentCode ?? session.agentCode ?? "main-agent";
  const systemAgentSessionId = agentCode !== "main-agent"
    ? session.systemAgentSessionId ?? `ses-${session.sessionCode.toLowerCase()}`
    : null;
  const nextSession: ChatSession = {
    ...session,
    agentCode,
    lastRunCode: runCode,
    lastRunStatus: "COMPLETED",
    systemAgentSessionId,
    updatedAt: now,
  };
  const nextMessages: ChatMessage[] = [
    {
      id: Date.now(),
      messageCode: `MSG-${Date.now()}`,
      sessionCode: session.sessionCode,
      runCode,
      role: "USER",
      channel: session.taskCode ? "TASK_AGENT" : "MAIN_AGENT",
      actor: "User",
      content: input.content,
      createdAt: now,
    },
    {
      id: Date.now() + 1,
      messageCode: `MSG-${Date.now() + 1}`,
      sessionCode: session.sessionCode,
      runCode,
      role: "ASSISTANT",
      channel: session.taskCode ? "TASK_AGENT" : "MAIN_AGENT",
      actor: "Assistant",
      content: finalOutput,
      inputTokens: 32,
      outputTokens: 24,
      totalTokens: 56,
      createdAt: now,
    },
  ];
  const sessionIndex = sessions.findIndex((item) => item.sessionCode === session.sessionCode);
  if (sessionIndex >= 0) sessions[sessionIndex] = nextSession;
  messagesBySession[session.sessionCode] = [
    ...(messagesBySession[session.sessionCode] ?? []),
    ...nextMessages,
  ];

  return {
    session: nextSession,
    run: {
      runCode,
      conversationCode: session.sessionCode,
      workspaceCode: input.workspaceCode,
      status: "COMPLETED",
      finalOutput,
      latestRequestEstimate: mockRequestEstimate,
      latestUsageAudit: mockUsageAudit,
      stepCount: 1,
      tokenUsage: {
        inputTokens: 32,
        outputTokens: 24,
        totalTokens: 56,
      },
      systemAgentSessionId,
    },
    messages: nextMessages,
  };
}
