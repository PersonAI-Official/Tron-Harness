export type TaskStatus = string;

export type TaskPriority = "Low" | "Medium" | "High" | "Extra High";
export type TaskType = "Feature" | "Bug";
export type TaskSourceType = "Internal" | "GitHub" | "GitLab";
export interface TaskFlowStatus {
  name: string;
  position: number;
  required: boolean;
  statusCode: TaskStatus;
}

export interface TaskFlow {
  flowCode: string;
  name: string;
  statuses: TaskFlowStatus[];
}

export interface TaskFlowPayload {
  flowCode: string;
  name: string;
  statuses: Array<Pick<TaskFlowStatus, "name" | "statusCode">>;
}

export interface WorkspaceTaskFlow {
  flow: TaskFlow;
  workspaceCode: string;
}

export interface TaskOptions {
  flow: TaskFlow;
  priorities: TaskPriority[];
  sourceTypes: TaskSourceType[];
  statuses: TaskFlowStatus[];
  taskTypes: TaskType[];
}

export interface QuickReference { tips: string[]; variables: string[]; }
export interface TaskSummary { priority: TaskPriority; progress: number; status: TaskStatus; taskCode: string; taskType: TaskType; title: string; }
export interface TaskDetailData { agentCode: string; agentCodes: string[]; appCodes: string[]; archivedAt: string | null; assigneeUserCodes: string[]; createdAt: string; description: string | null; executionResult: string | null; flowCode: string; id: string; modelCode: string | null; parent: TaskSummary | null; parentTaskCode: string | null; previousStatus: TaskStatus | null; priority: TaskPriority; progress: number; sourceType: TaskSourceType; sourceUrl: string | null; status: TaskStatus; subtasks: TaskSummary[]; taskCode: string; taskType: TaskType; title: string; updatedAt: string; workspaceCode: string; }
export interface TaskListItem { agentCode: string; agentCodes: string[]; appCodes: string[]; archivedAt?: string | null; assigneeUserCodes: string[]; flowCode: string; modelCode: string | null; parentTaskCode: string | null; priority: TaskPriority; progress: number; sourceType: TaskSourceType; sourceUrl: string | null; status: TaskStatus; subtaskCount: number; taskCode: string; taskType: TaskType; title: string; updatedAt: string; workspaceCode: string; }
export interface KanbanColumn { status: TaskStatus; title: string; tasks: TaskListItem[]; }
export interface TaskPage<T> { current: number; records: T[]; size: number; total: number; }
export interface CreateTaskPayload { agentCode?: string; agentCodes: string[]; appCodes: string[]; assigneeUserCodes: string[]; description?: string | null; modelCode?: string | null; parentTaskCode?: string | null; priority: TaskPriority; sourceType: TaskSourceType; sourceUrl?: string | null; taskType: TaskType; title: string; workspaceCode: string; }
export type UpdateTaskPayload = Partial<Omit<CreateTaskPayload, "parentTaskCode" | "workspaceCode">>;
export interface UpdateTaskStatusPayload { progress?: number; status: TaskStatus; }
export interface TaskLifecycleResult { archivedAt: string | null; previousStatus: TaskStatus | null; status: TaskStatus; taskCode: string; }
export interface TaskExecutionResult { agentCode: string; modelCode: string | null; runCode: string; sessionCode: string; summary: string; taskCode: string; }
export type TaskExecutionStatus = "Running" | "Success" | "Failed";
export interface TaskExecutionRecord { createdAt: string; executionCode: string; finishedAt: string | null; sessionCode: string | null; startedAt: string; status: TaskExecutionStatus; summary: string | null; taskCode: string; }

function taskPriorityRank(priority: TaskPriority) {
  if (priority === "Extra High") return 0;
  if (priority === "High") return 1;
  if (priority === "Medium") return 2;
  return 3;
}

function taskCodeSequence(taskCode: string) {
  const matched = taskCode.match(/(\d+)(?!.*\d)/);
  if (!matched) return null;
  const sequence = Number.parseInt(matched[1], 10);
  return Number.isFinite(sequence) ? sequence : null;
}

export function compareTaskListItems(left: TaskListItem, right: TaskListItem) {
  const priorityComparison = taskPriorityRank(left.priority) - taskPriorityRank(right.priority);
  if (priorityComparison !== 0) return priorityComparison;

  const leftSequence = taskCodeSequence(left.taskCode);
  const rightSequence = taskCodeSequence(right.taskCode);
  if (leftSequence !== null && rightSequence !== null && leftSequence !== rightSequence) {
    return rightSequence - leftSequence;
  }
  if (leftSequence !== null && rightSequence === null) return -1;
  if (leftSequence === null && rightSequence !== null) return 1;
  return right.taskCode.localeCompare(left.taskCode);
}

export function statusLabel(value: TaskStatus): string {
  if (value === "NOT_STARTED") return "未开始";
  if (value === "CLOSED") return "关闭";
  return value;
}
