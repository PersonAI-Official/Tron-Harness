import { requestApi } from "@/shared/api/httpClient";
import type { CreateTaskPayload, KanbanColumn, TaskDetailData, TaskExecutionRecord, TaskExecutionResult, TaskFlow, TaskFlowPayload, TaskLifecycleResult, TaskListItem, TaskOptions, TaskPage, UpdateTaskPayload, UpdateTaskStatusPayload, WorkspaceTaskFlow } from "@/domain/task/model";

export const taskApiEndpoints = { archiveTask: "POST /api/v1/tasks/{taskCode}/archive", createTask: "POST /api/v1/tasks", createTaskFlow: "POST /api/v1/admin/task-flows", deleteTask: "DELETE /api/v1/tasks/{taskCode}", deleteTaskFlow: "DELETE /api/v1/admin/task-flows/{flowCode}", executeTask: "POST /api/v1/tasks/{taskCode}/execute", getTaskDetail: "GET /api/v1/tasks/{taskCode}", getTaskFlow: "GET /api/v1/admin/task-flows/{flowCode}", getTaskOptions: "GET /api/v1/tasks/options", getTaskQuickReference: "GET /api/v1/tasks/quick-reference", getWorkspaceTaskFlow: "GET /api/v1/task-flows/workspaces/{workspaceCode}", listKanbanColumns: "GET /api/v1/tasks/kanban", listTaskExecutions: "GET /api/v1/tasks/{taskCode}/executions", listTaskFlows: "GET /api/v1/admin/task-flows", listTasks: "GET /api/v1/tasks", restoreTask: "POST /api/v1/tasks/{taskCode}/restore", switchWorkspaceTaskFlow: "PUT /api/v1/task-flows/workspaces/{workspaceCode}", updateTask: "PATCH /api/v1/tasks/{taskCode}", updateTaskFlow: "PATCH /api/v1/admin/task-flows/{flowCode}", updateTaskStatus: "POST /api/v1/tasks/{taskCode}/status" } as const;

type TaskQueryParams = Record<string, string | number | boolean | null | undefined>;
export interface TaskListQuery { assigneeUserCode?: string; current?: number; includeArchived?: boolean; parentTaskCode?: string; priority?: string; search?: string; size?: number; sourceType?: string; status?: string; taskType?: string; workspaceCode: string; }
function taskQueryParams(query: TaskListQuery): TaskQueryParams { return { ...query }; }
export function fetchTaskOptions(workspaceCode: string, signal?: AbortSignal) { return requestApi<TaskOptions>(taskApiEndpoints.getTaskOptions, { queryParams: { workspaceCode }, signal }); }
export function fetchTaskKanban(query: TaskListQuery, signal?: AbortSignal) { return requestApi<KanbanColumn[]>(taskApiEndpoints.listKanbanColumns, { queryParams: taskQueryParams(query), signal }); }
export function fetchTaskPage(query: TaskListQuery, signal?: AbortSignal) { return requestApi<TaskPage<TaskListItem>>(taskApiEndpoints.listTasks, { queryParams: taskQueryParams(query), signal }); }
export function fetchTaskDetail(taskCode: string, signal?: AbortSignal) { return requestApi<TaskDetailData>(taskApiEndpoints.getTaskDetail, { pathParams: { taskCode }, signal }); }
export function createTask(payload: CreateTaskPayload) { return requestApi<TaskDetailData>(taskApiEndpoints.createTask, { body: payload }); }
export function updateTask(taskCode: string, payload: UpdateTaskPayload) { return requestApi<TaskDetailData>(taskApiEndpoints.updateTask, { body: payload, pathParams: { taskCode } }); }
export function updateTaskStatus(taskCode: string, payload: UpdateTaskStatusPayload) { return requestApi<TaskDetailData>(taskApiEndpoints.updateTaskStatus, { body: payload, pathParams: { taskCode } }); }
export function executeTask(taskCode: string) { return requestApi<TaskExecutionResult>(taskApiEndpoints.executeTask, { pathParams: { taskCode } }); }
export function fetchTaskExecutionPage(taskCode: string, query: { current: number; size: number }, signal?: AbortSignal) { return requestApi<TaskPage<TaskExecutionRecord>>(taskApiEndpoints.listTaskExecutions, { pathParams: { taskCode }, queryParams: query, signal }); }
export function archiveTask(taskCode: string) { return requestApi<TaskLifecycleResult>(taskApiEndpoints.archiveTask, { pathParams: { taskCode } }); }
export function restoreTask(taskCode: string) { return requestApi<TaskLifecycleResult>(taskApiEndpoints.restoreTask, { pathParams: { taskCode } }); }
export function deleteTask(taskCode: string) { return requestApi<{ taskCode: string }>(taskApiEndpoints.deleteTask, { pathParams: { taskCode } }); }
export function fetchTaskFlows(signal?: AbortSignal) { return requestApi<TaskFlow[]>(taskApiEndpoints.listTaskFlows, { signal }); }
export function fetchTaskFlow(flowCode: string, signal?: AbortSignal) { return requestApi<TaskFlow>(taskApiEndpoints.getTaskFlow, { pathParams: { flowCode }, signal }); }
export function createTaskFlow(payload: TaskFlowPayload) { return requestApi<TaskFlow>(taskApiEndpoints.createTaskFlow, { body: payload }); }
export function updateTaskFlow(flowCode: string, payload: TaskFlowPayload) { return requestApi<TaskFlow>(taskApiEndpoints.updateTaskFlow, { body: payload, pathParams: { flowCode } }); }
export function deleteTaskFlow(flowCode: string) { return requestApi<void>(taskApiEndpoints.deleteTaskFlow, { pathParams: { flowCode } }); }
export function fetchWorkspaceTaskFlow(workspaceCode: string, signal?: AbortSignal) { return requestApi<WorkspaceTaskFlow>(taskApiEndpoints.getWorkspaceTaskFlow, { pathParams: { workspaceCode }, signal }); }
export function switchWorkspaceTaskFlow(workspaceCode: string, flowCode: string) { return requestApi<WorkspaceTaskFlow>(taskApiEndpoints.switchWorkspaceTaskFlow, { body: { flowCode }, pathParams: { workspaceCode } }); }
