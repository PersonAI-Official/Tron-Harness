import { isKnownAppCode } from "@/domain/app/mock";
import { appendMockTaskRunTranscript, createMockChatSession } from "@/domain/chat/mock/data";
import type { TaskListQuery } from "@/domain/task/api";
import {
  compareTaskListItems,
  type CreateTaskPayload,
  type KanbanColumn,
  type QuickReference,
  type TaskDetailData,
  type TaskExecutionRecord,
  type TaskExecutionResult,
  type TaskFlow,
  type TaskFlowPayload,
  type TaskLifecycleResult,
  type TaskListItem,
  type TaskOptions,
  type TaskSummary,
  type TaskStatus,
  type UpdateTaskPayload,
  type UpdateTaskStatusPayload,
  type WorkspaceTaskFlow,
} from "@/domain/task/model";

const NOW = "2026-05-19T12:00:00Z";
const defaultFlow: TaskFlow = {
  flowCode: "DEFAULT",
  name: "默认流程",
  statuses: [
    { statusCode: "NOT_STARTED", name: "未开始", position: 0, required: true },
    { statusCode: "DESIGNING", name: "设计中", position: 1, required: false },
    { statusCode: "RED", name: "开发中", position: 2, required: false },
    { statusCode: "GREEN", name: "验证中", position: 3, required: false },
    { statusCode: "REGRESSIONING", name: "回归中", position: 4, required: false },
    { statusCode: "REVIEW", name: "评审中", position: 5, required: false },
    { statusCode: "IN_PROGRESS", name: "进行中", position: 6, required: false },
    { statusCode: "REOPEN", name: "重新打开", position: 7, required: false },
    { statusCode: "CLOSED", name: "关闭", position: 8, required: true },
  ],
};
let taskFlows: TaskFlow[] = [defaultFlow];
const workspaceFlowCodes: Record<string, string> = { TRON_CORE: defaultFlow.flowCode };
let taskExecutionRecords: TaskExecutionRecord[] = [];

function flowForWorkspace(workspaceCode: string) {
  const flowCode = workspaceFlowCodes[workspaceCode] ?? defaultFlow.flowCode;
  return taskFlows.find((flow) => flow.flowCode === flowCode) ?? defaultFlow;
}

function taskOptionsForWorkspace(workspaceCode = "TRON_CORE"): TaskOptions {
  const flow = flowForWorkspace(workspaceCode);
  return {
    flow,
    statuses: flow.statuses,
  priorities: ["Low", "Medium", "High", "Extra High"],
  sourceTypes: ["Internal", "GitHub", "GitLab"],
  taskTypes: ["Feature", "Bug"],
  };
}

let nextTaskId = 1;
let tasks: TaskDetailData[] = [
  {
    agentCode: "main-agent",
    agentCodes: [],
    appCodes: [],
    archivedAt: null,
    assigneeUserCodes: ["USR-AUTH-0001"],
    createdAt: NOW,
    description: "Capture the initial superpower intent and task boundary.",
    executionResult: null,
    flowCode: "DEFAULT",
    id: "734500000000000101",
    modelCode: null,
    parent: null,
    parentTaskCode: null,
    previousStatus: null,
    priority: "Medium",
    progress: 0,
    sourceType: "Internal",
    sourceUrl: "",
    status: "NOT_STARTED",
    subtasks: [],
    taskCode: "TSK-SEED-FEATURE",
    taskType: "Feature",
    title: "Model task-management superpower flow",
    updatedAt: NOW,
    workspaceCode: "TRON_CORE",
  },
  {
    agentCode: "main-agent",
    agentCodes: ["agent-reviewer"],
    appCodes: ["APP-TRON-API"],
    archivedAt: null,
    assigneeUserCodes: ["USR-AUTH-0001"],
    createdAt: NOW,
    description: "Fix a board rendering gap under the new workflow.",
    executionResult: null,
    flowCode: "DEFAULT",
    id: "734500000000000102",
    modelCode: null,
    parent: null,
    parentTaskCode: null,
    previousStatus: null,
    priority: "High",
    progress: 65,
    sourceType: "GitHub",
    sourceUrl: "https://github.com/clouds3n/tron/issues/42",
    status: "IN_PROGRESS",
    subtasks: [],
    taskCode: "TSK-SEED-BUG",
    taskType: "Bug",
    title: "Fix task board workflow regression",
    updatedAt: NOW,
    workspaceCode: "TRON_CORE",
  },
  {
    agentCode: "main-agent",
    agentCodes: [],
    appCodes: ["APP-TRON-CONSOLE", "github"],
    archivedAt: null,
    assigneeUserCodes: ["USR-AUTH-0001"],
    createdAt: NOW,
    description: "Legacy app codes should remain visible until the selection is explicitly changed.",
    executionResult: null,
    flowCode: "DEFAULT",
    id: "734500000000000104",
    modelCode: null,
    parent: null,
    parentTaskCode: null,
    previousStatus: null,
    priority: "Medium",
    progress: 20,
    sourceType: "Internal",
    sourceUrl: "",
    status: "NOT_STARTED",
    subtasks: [],
    taskCode: "TSK-SEED-LEGACY-APP",
    taskType: "Feature",
    title: "Legacy app selection example",
    updatedAt: NOW,
    workspaceCode: "TRON_CORE",
  },
  {
    agentCode: "main-agent",
    agentCodes: [],
    appCodes: [],
    archivedAt: NOW,
    assigneeUserCodes: [],
    createdAt: NOW,
    description: "Archived example under the new flow.",
    executionResult: null,
    flowCode: "DEFAULT",
    id: "734500000000000103",
    modelCode: null,
    parent: null,
    parentTaskCode: null,
    previousStatus: "REVIEW",
    priority: "Low",
    progress: 15,
    sourceType: "Internal",
    sourceUrl: "",
    status: "CLOSED",
    subtasks: [],
    taskCode: "TSK-SEED-ARCHIVED",
    taskType: "Feature",
    title: "Archived workflow example",
    updatedAt: NOW,
    workspaceCode: "TRON_CORE",
  },
];

export function getTaskOptions(workspaceCode?: string) { return taskOptionsForWorkspace(workspaceCode); }

export function listMockTaskFlows() { return taskFlows.map((flow) => ({ ...flow, statuses: [...flow.statuses] })); }

export function getMockTaskFlow(flowCode: string) {
  const flow = taskFlows.find((item) => item.flowCode === flowCode);
  if (!flow) throw new Error("task flow was not found");
  return { ...flow, statuses: [...flow.statuses] };
}

function normalizeMockFlow(payload: TaskFlowPayload): TaskFlow {
  const flowCode = payload.flowCode.trim().toUpperCase();
  if (!/^[A-Z][A-Z0-9_-]{0,63}$/.test(flowCode) || !payload.name.trim()) throw new Error("任务流编码或名称无效");
  const incoming = payload.statuses.map((status) => ({ ...status, statusCode: status.statusCode.trim().toUpperCase(), name: status.name.trim() }));
  const statusCodes = new Set(incoming.map((status) => status.statusCode));
  if (incoming.some((status) => !/^[A-Z][A-Z0-9_-]{0,63}$/.test(status.statusCode) || !status.name) || statusCodes.size !== incoming.length) throw new Error("状态编码或名称无效");
  const withoutRequired = incoming.filter((status) => status.statusCode !== "NOT_STARTED" && status.statusCode !== "CLOSED");
  return {
    flowCode,
    name: payload.name.trim(),
    statuses: [
      { statusCode: "NOT_STARTED", name: "未开始", position: 0, required: true },
      ...withoutRequired.map((status, index) => ({ ...status, position: index + 1, required: false })),
      { statusCode: "CLOSED", name: "关闭", position: withoutRequired.length + 1, required: true },
    ],
  };
}

export function createMockTaskFlow(payload: TaskFlowPayload) {
  const flow = normalizeMockFlow(payload);
  if (taskFlows.some((item) => item.flowCode === flow.flowCode)) throw new Error("任务流编码已存在");
  taskFlows = [...taskFlows, flow];
  return flow;
}

export function updateMockTaskFlow(flowCode: string, payload: TaskFlowPayload) {
  const flow = normalizeMockFlow(payload);
  if (flow.flowCode !== flowCode || !taskFlows.some((item) => item.flowCode === flowCode)) throw new Error("任务流不存在");
  taskFlows = taskFlows.map((item) => item.flowCode === flowCode ? flow : item);
  return flow;
}

export function deleteMockTaskFlow(flowCode: string) {
  if (flowCode === defaultFlow.flowCode) throw new Error("默认任务流不能删除");
  if (Object.values(workspaceFlowCodes).includes(flowCode) || tasks.some((task) => task.flowCode === flowCode)) throw new Error("任务流正在使用中");
  taskFlows = taskFlows.filter((item) => item.flowCode !== flowCode);
}

export function getMockWorkspaceTaskFlow(workspaceCode: string): WorkspaceTaskFlow {
  return { workspaceCode, flow: flowForWorkspace(workspaceCode) };
}

export function switchMockWorkspaceTaskFlow(workspaceCode: string, flowCode: string): WorkspaceTaskFlow {
  const flow = getMockTaskFlow(flowCode);
  if (tasks.some((task) => task.workspaceCode === workspaceCode && task.status !== "CLOSED")) throw new Error("所有任务关闭后才能切换工作流");
  workspaceFlowCodes[workspaceCode] = flow.flowCode;
  tasks = tasks.map((task) => task.workspaceCode === workspaceCode ? { ...task, flowCode: flow.flowCode } : task);
  return { workspaceCode, flow };
}

export function getTaskQuickReference(): QuickReference {
  return {
    tips: [
      "Start with the target outcome, then state the concrete deliverables the agent must produce.",
      "Include the context, constraints, acceptance checks, and any repos, links, or paths the agent should use.",
      "Reference reusable variables directly in the task body, for example $VAR1, so the agent can substitute workspace-specific values.",
    ],
    variables: ["$VAR1"],
  };
}

export function listKanbanColumns(query: TaskListQuery): KanbanColumn[] {
  const records = filterTasks(query);
  const flow = flowForWorkspace(query.workspaceCode);
  return flow.statuses
    .map((status) => ({
      status: status.statusCode,
      title: status.name,
      tasks: records
        .filter((task) => task.status === status.statusCode)
        .map(toListItem),
    }))
    .map((column) => ({
      ...column,
      tasks: [...column.tasks].sort(compareTaskListItems),
    }));
}

export function getTaskDetail(taskCode = "TSK-SEED-FEATURE") {
  return withRelationships(tasks.find((task) => task.taskCode === taskCode) ?? tasks[0]);
}

export function createMockTask(payload: CreateTaskPayload) {
  validateAppCodes(payload.workspaceCode, payload.appCodes);
  const taskCode = `TSK-MOCK-${nextTaskId++}`;
  const flow = flowForWorkspace(payload.workspaceCode);
  const task: TaskDetailData = {
    agentCode: payload.agentCode?.trim() || "main-agent",
    agentCodes: uniqueCodes(payload.agentCodes),
    appCodes: uniqueCodes(payload.appCodes),
    archivedAt: null,
    assigneeUserCodes: uniqueCodes(payload.assigneeUserCodes),
    createdAt: NOW,
    description: payload.description?.trim() ? payload.description : null,
    executionResult: null,
    flowCode: flow.flowCode,
    id: String(734500000000100000 + nextTaskId),
    modelCode: payload.modelCode?.trim() || null,
    parent: null,
    parentTaskCode: payload.parentTaskCode ?? null,
    previousStatus: null,
    priority: payload.priority,
    progress: 0,
    sourceType: payload.sourceType,
    sourceUrl: payload.sourceUrl?.trim() ?? "",
    status: "NOT_STARTED",
    subtasks: [],
    taskCode,
    taskType: payload.taskType,
    title: payload.title,
    updatedAt: NOW,
    workspaceCode: payload.workspaceCode,
  };
  tasks = [task, ...tasks];
  return getTaskDetail(taskCode);
}

export function updateMockTask(taskCode: string, payload: UpdateTaskPayload) {
  const current = getTaskDetail(taskCode);
  if (payload.appCodes) validateAppCodes(current.workspaceCode, payload.appCodes);
  tasks = tasks.map((task) => task.taskCode !== taskCode || task.archivedAt ? task : {
    ...task,
    ...payload,
    agentCode: payload.agentCode === undefined ? task.agentCode : (payload.agentCode.trim() || "main-agent"),
    agentCodes: payload.agentCodes ? uniqueCodes(payload.agentCodes) : task.agentCodes,
    appCodes: payload.appCodes ? uniqueCodes(payload.appCodes) : task.appCodes,
    assigneeUserCodes: payload.assigneeUserCodes ? uniqueCodes(payload.assigneeUserCodes) : task.assigneeUserCodes,
    description: payload.description === undefined ? task.description : payload.description,
    modelCode: payload.modelCode === undefined ? task.modelCode : (payload.modelCode?.trim() || null),
    sourceUrl: payload.sourceUrl === undefined ? task.sourceUrl : payload.sourceUrl,
    updatedAt: NOW,
  });
  return getTaskDetail(taskCode);
}

export function updateMockTaskStatus(taskCode: string, payload: UpdateTaskStatusPayload) {
  const current = getTaskDetail(taskCode);
  const flow = taskFlows.find((item) => item.flowCode === current.flowCode);
  if (!flow?.statuses.some((status) => status.statusCode === payload.status)) throw new Error("任务状态不属于当前工作流");
  tasks = tasks.map((task) => task.taskCode !== taskCode || task.archivedAt ? task : {
    ...task,
    progress: payload.status === "CLOSED" ? 100 : payload.progress ?? task.progress,
    status: payload.status,
    updatedAt: NOW,
  });
  return getTaskDetail(taskCode);
}

export function archiveMockTask(taskCode: string): TaskLifecycleResult {
  const hasActiveSubtasks = tasks.some((task) => task.parentTaskCode === taskCode && !task.archivedAt);
  if (hasActiveSubtasks) throw new Error("archive active subtasks before archiving the parent task");
  tasks = tasks.map((task) => task.taskCode === taskCode && !task.archivedAt ? {
    ...task,
    archivedAt: NOW,
    previousStatus: task.status,
    status: "CLOSED",
    updatedAt: NOW,
  } : task);
  const task = getTaskDetail(taskCode);
  return { archivedAt: task.archivedAt, previousStatus: task.previousStatus, status: task.status, taskCode };
}

export function restoreMockTask(taskCode: string): TaskLifecycleResult {
  tasks = tasks.map((task) => {
    if (task.taskCode !== taskCode || !task.archivedAt) return task;
    const flow = taskFlows.find((item) => item.flowCode === task.flowCode);
    const nextStatus = task.previousStatus && flow?.statuses.some((status) => status.statusCode === task.previousStatus)
      ? task.previousStatus
      : "NOT_STARTED";
    return {
      ...task,
      archivedAt: null,
      previousStatus: null,
      status: nextStatus,
      updatedAt: NOW,
    };
  });
  const task = getTaskDetail(taskCode);
  return { archivedAt: task.archivedAt, previousStatus: task.previousStatus, status: task.status, taskCode };
}

export function deleteMockTask(taskCode: string): void {
  const task = getTaskDetail(taskCode);
  const hasSubtasks = tasks.some((candidate) => candidate.parentTaskCode === task.taskCode);
  if (hasSubtasks) throw new Error("delete the task subtasks before deleting the parent task");
  tasks = tasks.filter((candidate) => candidate.taskCode !== task.taskCode);
}

export function executeMockTask(taskCode: string): TaskExecutionResult {
  const task = getTaskDetail(taskCode);
  if (task.archivedAt) {
    throw new Error("restore archived task before executing it");
  }
  const runSequence = Date.now();
  const runCode = `RUN-${runSequence}`;
  const summary = `Mock execution completed for ${task.taskCode}.`;
  const session = createMockChatSession({
    taskCode: task.taskCode,
    workspaceCode: task.workspaceCode,
    title: task.title,
    agentCode: task.agentCode || "main-agent",
  });
  const startedAt = new Date(runSequence).toISOString();
  taskExecutionRecords = [
    {
      createdAt: startedAt,
      executionCode: runCode,
      finishedAt: startedAt,
      sessionCode: session.sessionCode,
      startedAt,
      status: "Success",
      summary,
      taskCode: task.taskCode,
    },
    ...taskExecutionRecords,
  ];
  appendMockTaskRunTranscript(
    session.sessionCode,
    runCode,
    task.description || task.title,
    summary,
  );
  tasks = tasks.map((item) => item.taskCode === task.taskCode ? {
    ...item,
    executionResult: summary,
    progress: 100,
    status: "CLOSED",
    updatedAt: NOW,
  } : item);
  return {
    agentCode: task.agentCode || "main-agent",
    modelCode: task.modelCode,
    runCode,
    sessionCode: session.sessionCode,
    summary,
    taskCode: task.taskCode,
  };
}

export function listMockTaskExecutionPage(
  taskCode: string,
  current: number,
  size: number,
) {
  const records = taskExecutionRecords.filter((record) => record.taskCode === taskCode);
  const start = (Math.max(1, current) - 1) * Math.max(1, size);
  return {
    current: Math.max(1, current),
    records: records.slice(start, start + Math.max(1, size)),
    size: Math.max(1, size),
    total: records.length,
  };
}

function filterTasks(query: TaskListQuery) {
  const includeArchived = query.includeArchived === true;
  const search = query.search?.trim().toLowerCase();
  return tasks.filter((task) => {
    if (query.workspaceCode && task.workspaceCode !== query.workspaceCode) return false;
    if (!includeArchived && task.archivedAt) return false;
    if (query.status && task.status !== query.status) return false;
    if (query.priority && task.priority !== query.priority) return false;
    if (query.taskType && task.taskType !== query.taskType) return false;
    if (query.sourceType && task.sourceType !== query.sourceType) return false;
    if (query.assigneeUserCode && !task.assigneeUserCodes.includes(query.assigneeUserCode)) return false;
    if (query.parentTaskCode === "__ROOT__" && task.parentTaskCode) return false;
    if (query.parentTaskCode && query.parentTaskCode !== "__ROOT__" && task.parentTaskCode !== query.parentTaskCode) return false;
    if (!search) return true;
    return [task.taskCode, task.title, task.description ?? "", task.workspaceCode, task.sourceUrl ?? ""].join(" ").toLowerCase().includes(search);
  });
}

function effectiveProgress(task: TaskDetailData): number {
  const activeSubtasks = tasks.filter((candidate) => candidate.parentTaskCode === task.taskCode && !candidate.archivedAt);
  if (!activeSubtasks.length) return task.progress;
  const total = activeSubtasks.reduce((sum, candidate) => sum + effectiveProgress(candidate), 0);
  return Math.round(total / activeSubtasks.length);
}

function toListItem(task: TaskDetailData): TaskListItem {
  return {
    agentCode: task.agentCode,
    agentCodes: task.agentCodes,
    appCodes: task.appCodes,
    archivedAt: task.archivedAt,
    assigneeUserCodes: task.assigneeUserCodes,
    flowCode: task.flowCode,
    modelCode: task.modelCode,
    parentTaskCode: task.parentTaskCode,
    priority: task.priority,
    progress: effectiveProgress(task),
    sourceType: task.sourceType,
    sourceUrl: task.sourceUrl,
    status: task.status,
    subtaskCount: tasks.filter((candidate) => candidate.parentTaskCode === task.taskCode && !candidate.archivedAt).length,
    taskCode: task.taskCode,
    taskType: task.taskType,
    title: task.title,
    updatedAt: task.updatedAt,
    workspaceCode: task.workspaceCode,
  };
}

function summary(task: TaskDetailData): TaskSummary {
  return {
    priority: task.priority,
    progress: effectiveProgress(task),
    status: task.status,
    taskCode: task.taskCode,
    taskType: task.taskType,
    title: task.title,
  };
}

function withRelationships(task: TaskDetailData): TaskDetailData {
  const parent = task.parentTaskCode ? tasks.find((candidate) => candidate.taskCode === task.parentTaskCode) : undefined;
  const subtasks = tasks.filter((candidate) => candidate.parentTaskCode === task.taskCode && !candidate.archivedAt);
  return {
    ...task,
    progress: effectiveProgress(task),
    parent: parent ? summary(parent) : null,
    subtasks: subtasks.map(summary),
  };
}

function uniqueCodes(codes: string[]) {
  return Array.from(new Set(codes.map((code) => code.trim()).filter(Boolean)));
}

function validateAppCodes(workspaceCode: string, appCodes: string[]) {
  if (appCodes.every((code) => isKnownAppCode(workspaceCode, code))) return;
  throw new Error("One or more selected apps are invalid for this workspace");
}
