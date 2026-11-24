import { Dialog, DialogContent } from "@/components/ui/dialog";
import { ConfirmDialog } from "@/components/confirm-dialog";
import { Archive, Maximize2, RotateCcw, Trash2, X } from "lucide-react";
import { useLocaleFormatters } from "@/shared/hooks/useLocaleFormatters";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  Link,
  useNavigate,
  useSearchParams,
} from "react-router";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Field, SelectInput, TextArea, TextInput } from "@/components/forms";
import { MarkdownContent } from "@/components/markdown-content";
import { OptionSelect } from "@/components/option-select";
import {
  PageHeader,
  SurfaceCard,
  useTabPanelConfig,
} from "@/shared";
import { useOpenRightSidebar } from "@/shared/hooks/useOpenRightSidebar";
import { InspectorPanel } from "@/components/inspector";
import { useToast } from "@/components/ui/toast";

import {
  archiveTask,
  createTask,
  deleteTask,
  executeTask,
  restoreTask,
  updateTask,
  updateTaskStatus,
} from "@/domain/task/api";
import {
  archiveMockTask,
  createMockTask,
  deleteMockTask,
  executeMockTask,
  restoreMockTask,
  updateMockTask,
  updateMockTaskStatus,
} from "@/domain/task/mock/data";
import {
  useTaskDetail,
  useTaskOptions,
  useTaskWorkspaceOptions,
} from "@/domain/task/hooks";
import { TaskAppSelectionField, TaskExecutionHistoryPanel, TaskExecutionModelSelect } from "@/domain/task/components";
import {
  taskFlowStatusDisplayName,
  type TaskPriority,
  type TaskSourceType,
  type TaskStatus,
  type TaskType,
  type UpdateTaskPayload,
} from "@/domain/task/model";
import { getServiceDataSource } from "@/shared/api/serviceConfig";
import { fetchWorkspaceInternalAgents, fetchWorkspaceSystemAgents } from "@/domain/agent/api";
import { listWorkspaceInternalAgentsMock, listWorkspaceSystemAgentsMock } from "@/domain/agent/mock";
import { fetchModelSelectOptions } from "@/domain/model/api/modelApi";
import {
  automationModelCodeFromOption,
  automationModelOptionsForAgent,
  availableAutomationAgents,
  mainAutomationAgentOption,
  optionCodeForAutomationModel,
} from "@/domain/automation/pages/automationExecutionOptions";
import type {
  AutomationAgentOption,
  AutomationModelOption,
} from "@/domain/automation/pages/automationExecutionOptions";
import { SelectItem } from "@/components/ui/select";

function ProgressBar({ value }: { value: number }) {
  return (
    <div className="h-1.5 overflow-hidden rounded-full bg-[var(--color-surface-container-high)]">
      <div className="h-full rounded-full bg-[var(--color-primary)]" style={{ width: `${Math.min(100, Math.max(0, value))}%` }} />
    </div>
  );
}

function toggleCode(codes: string[], code: string) {
  return codes.includes(code) ? codes.filter((item) => item !== code) : [...codes, code];
}

function sameCodes(left: string[], right: string[]) {
  return left.length === right.length && left.every((item, index) => item === right[index]);
}

function progressValue(value: string) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return 0;
  return Math.min(100, Math.max(0, parsed));
}

function taskDetailHref(taskCode: string) {
  return `/tasks/detail?taskCode=${encodeURIComponent(taskCode)}`;
}

function ReadValue({ children, noneLabel = "None" }: { children: ReactNode; noneLabel?: string }) {
  return (
    <div className="min-h-6 min-w-0 max-w-full py-1 text-sm leading-6 text-[var(--color-on-surface)] whitespace-pre-wrap break-words [overflow-wrap:anywhere]">
      {children || <span className="text-[var(--color-on-surface-variant)]">{noneLabel}</span>}
    </div>
  );
}

export function TaskDetailsPage() {
  const { formatDateTime } = useLocaleFormatters();
  const { t } = useTranslation(["pages"]);
  const pageT = useCallback((key: string, defaultValue: string, options?: Record<string, unknown>) => (
    t(`tasks.${key}`, { defaultValue, ...options })
  ), [t]);
  const flowT = useCallback(
    (key: string, defaultValue: string) => pageT(`taskFlows.${key}`, defaultValue),
    [pageT],
  );
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const openRightSidebar = useOpenRightSidebar();
  const taskCode = searchParams.get("taskCode") || "TSK-SEED-FEATURE";
  const { task, setTask, refresh, loading } = useTaskDetail(taskCode);
  const options = useTaskOptions(task.workspaceCode);
  const toast = useToast();
  const dataSource = getServiceDataSource("task");
  const { appOptions, loading: workspaceOptionsLoading } = useTaskWorkspaceOptions(task.workspaceCode);
  const [title, setTitle] = useState(task.title);
  const [description, setDescription] = useState(task.description ?? "");
  const [taskType, setTaskType] = useState<TaskType>(task.taskType);
  const [sourceType, setSourceType] = useState<TaskSourceType>(task.sourceType);
  const [sourceUrl, setSourceUrl] = useState(task.sourceUrl ?? "");
  const [priority, setPriority] = useState<TaskPriority>(task.priority);
  const [status, setStatus] = useState<TaskStatus>(task.status);
  const [progress, setProgress] = useState(String(task.progress));
  const [agentOptions, setAgentOptions] = useState<AutomationAgentOption[]>(() => [mainAutomationAgentOption]);
  const [selectedAgentCode, setSelectedAgentCode] = useState(task.agentCode || mainAutomationAgentOption.agentCode);
  const [selectedModelOptionCode, setSelectedModelOptionCode] = useState(optionCodeForAutomationModel(task.agentCode || mainAutomationAgentOption.agentCode, task.modelCode ?? null));
  const [workspaceModelOptions, setWorkspaceModelOptions] = useState<Awaited<ReturnType<typeof fetchModelSelectOptions>>>(() => []);
  const [executionOptionsLoading, setExecutionOptionsLoading] = useState(false);
  const [executing, setExecuting] = useState(false);
  const [lifecyclePending, setLifecyclePending] = useState(false);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const [selectedAppCodes, setSelectedAppCodes] = useState(task.appCodes);
  const [childTitle, setChildTitle] = useState("Follow-up subtask");
  const [descriptionFullscreenOpen, setDescriptionFullscreenOpen] = useState(false);
  const [executionResultFullscreenOpen, setExecutionResultFullscreenOpen] = useState(false);
  const [editing, setEditing] = useState(false);
  const skipNextTaskSync = useRef(false);
  const taskRef = useRef(task);
  const statusRef = useRef(status);
  const progressRef = useRef(progress);
  taskRef.current = task;
  statusRef.current = status;
  progressRef.current = progress;
  const archived = Boolean(task.archivedAt);
  const availableStatuses = options.statuses;
  const taskStatusLabel = useCallback((statusCode: TaskStatus) => {
    const statusOption = availableStatuses.find((item) => item.statusCode === statusCode);
    return statusOption
      ? taskFlowStatusDisplayName(statusOption, flowT, options.flow.flowCode)
      : statusCode;
  }, [availableStatuses, flowT, options.flow.flowCode]);
  const availableAppCodeSet = useMemo(() => new Set(appOptions.map((app) => app.appCode)), [appOptions]);
  const legacyAppCodes = selectedAppCodes.filter((code) => !availableAppCodeSet.has(code));
  const configuredAgentOption = useMemo<AutomationAgentOption>(() => {
    const configuredAgentCode = selectedAgentCode || mainAutomationAgentOption.agentCode;
    const matched = agentOptions.find((agent) => agent.agentCode === configuredAgentCode);
    if (matched) return matched;
    if (configuredAgentCode === mainAutomationAgentOption.agentCode) return mainAutomationAgentOption;
    return {
      agentCode: configuredAgentCode,
      description: "Configured task execution agent.",
      models: [],
      name: configuredAgentCode,
    };
  }, [agentOptions, selectedAgentCode]);
  const visibleAgentOptions = useMemo(
    () => agentOptions.some((agent) => agent.agentCode === configuredAgentOption.agentCode)
      ? agentOptions
      : [configuredAgentOption, ...agentOptions],
    [agentOptions, configuredAgentOption],
  );
  const executionAgentSelectOptions = useMemo(
    () => visibleAgentOptions.map((agent) => ({
      label: `${agent.name} (${agent.agentCode})`,
      value: agent.agentCode,
    })),
    [visibleAgentOptions],
  );
  const executionAgentLabel = useMemo(() => {
    const { agentCode, name } = configuredAgentOption;
    return name && name !== agentCode ? `${name} (${agentCode})` : agentCode;
  }, [configuredAgentOption]);
  const modelOptions = useMemo(
    () => automationModelOptionsForAgent(workspaceModelOptions, configuredAgentOption, pageT("values.automaticModel", "Conversation Automatic Routing")),
    [configuredAgentOption, pageT, workspaceModelOptions],
  );
  const selectedModelCode = useMemo(() => {
    const resolvedModelCode = automationModelCodeFromOption(modelOptions, selectedModelOptionCode);
    if (resolvedModelCode) return resolvedModelCode;
    const configuredOptionCode = optionCodeForAutomationModel(
      selectedAgentCode,
      task.modelCode ?? null,
      configuredAgentOption.modelOptionSource,
    );
    if (task.modelCode && selectedModelOptionCode === configuredOptionCode && !modelOptions.some((option) => option.optionCode === selectedModelOptionCode)) {
      return task.modelCode;
    }
    return null;
  }, [configuredAgentOption.modelOptionSource, modelOptions, selectedAgentCode, selectedModelOptionCode, task.modelCode]);
  const visibleModelOptions = useMemo<AutomationModelOption[]>(() => {
    if (!selectedModelOptionCode || modelOptions.some((option) => option.optionCode === selectedModelOptionCode)) {
      return modelOptions;
    }
    if (!task.modelCode) return modelOptions;
    return [{
      label: task.modelCode,
      modelCode: task.modelCode,
      optionCode: selectedModelOptionCode,
    }, ...modelOptions];
  }, [modelOptions, selectedModelOptionCode, task.modelCode]);
  useEffect(() => {
    if (skipNextTaskSync.current) {
      skipNextTaskSync.current = false;
      return;
    }
    setTitle(task.title);
    setDescription(task.description ?? "");
    setTaskType(task.taskType);
    setSourceType(task.sourceType);
    setSourceUrl(task.sourceUrl ?? "");
    setPriority(task.priority);
    setStatus(task.status);
    setProgress(String(task.progress));
    setSelectedAgentCode(task.agentCode || mainAutomationAgentOption.agentCode);
    setSelectedModelOptionCode(optionCodeForAutomationModel(task.agentCode || mainAutomationAgentOption.agentCode, task.modelCode ?? null));
    setSelectedAppCodes(task.appCodes);
    setDescriptionFullscreenOpen(false);
    setExecutionResultFullscreenOpen(false);
    setEditing(false);
  }, [task]);

  useEffect(() => {
    if (!task.workspaceCode) {
      setAgentOptions([]);
      setWorkspaceModelOptions([]);
      return;
    }

    let active = true;
    setExecutionOptionsLoading(true);
    const agentsPromise = getServiceDataSource("agent") === "api"
      ? fetchWorkspaceSystemAgents(task.workspaceCode)
      : Promise.resolve(listWorkspaceSystemAgentsMock(task.workspaceCode));
    const internalAgentsPromise = getServiceDataSource("agent") === "api"
      ? fetchWorkspaceInternalAgents(task.workspaceCode)
      : Promise.resolve(listWorkspaceInternalAgentsMock(task.workspaceCode));
    const modelsPromise = fetchModelSelectOptions(task.workspaceCode);

    Promise.all([agentsPromise, internalAgentsPromise, modelsPromise])
      .then(([agents, internalAgents, models]) => {
        if (!active) return;
        setAgentOptions(availableAutomationAgents(agents, internalAgents));
        setWorkspaceModelOptions(models);
      })
      .catch((error) => {
        if (!active) return;
        setAgentOptions([]);
        setWorkspaceModelOptions([]);
        toast.error(error instanceof Error ? error.message : pageT("errors.executionOptions", "Execution options could not be loaded"));
      })
      .finally(() => {
        if (active) setExecutionOptionsLoading(false);
      });

    return () => {
      active = false;
    };
  }, [pageT, task.workspaceCode, toast]);

  useEffect(() => {
    if (configuredAgentOption.modelOptionSource !== "workspace") return;
    setSelectedModelOptionCode(optionCodeForAutomationModel(
      selectedAgentCode,
      task.modelCode ?? null,
      configuredAgentOption.modelOptionSource,
    ));
  }, [configuredAgentOption.modelOptionSource, selectedAgentCode, task.modelCode]);

  useEffect(() => {
    if (visibleAgentOptions.some((agent) => agent.agentCode === selectedAgentCode)) return;
    setSelectedAgentCode(mainAutomationAgentOption.agentCode);
  }, [selectedAgentCode, visibleAgentOptions]);

  useEffect(() => {
    if (!visibleModelOptions.length) {
      setSelectedModelOptionCode("");
      return;
    }
    if (visibleModelOptions.some((option) => option.optionCode === selectedModelOptionCode)) return;
    setSelectedModelOptionCode(visibleModelOptions[0].optionCode);
  }, [selectedModelOptionCode, visibleModelOptions]);


  const detailEditable = editing && !archived;
  const updatesEditable = !archived;
  const progressManagedBySubtasks = task.subtasks.length > 0;
  const nextAppCodes = Array.from(new Set(selectedAppCodes.map((code) => code.trim()).filter(Boolean)));
  const detailDirty = title !== task.title
    || description !== (task.description ?? "")
    || taskType !== task.taskType
    || sourceType !== task.sourceType
    || sourceUrl !== (task.sourceUrl ?? "")
    || selectedAgentCode !== (task.agentCode || mainAutomationAgentOption.agentCode)
    || selectedModelCode !== task.modelCode;

  const rightPanel = useMemo(() => (
    <InspectorPanel title={pageT("detail.metadata", "Task Metadata")}>
      <SurfaceCard className="space-y-5 p-2">
        <div>
          <h3 className="mb-3 text-[11px] font-semibold uppercase text-[var(--color-on-surface-variant)]">{pageT("detail.metadata", "Task Metadata")}</h3>
          <div className="space-y-3 text-sm">
            <div className="flex justify-between gap-3"><span className="text-[var(--color-on-surface-variant)]">{pageT("detail.workspace", "Workspace")}</span><span className="font-medium">{task.workspaceCode}</span></div>
            <div className="flex justify-between gap-3">
              <span className="text-[var(--color-on-surface-variant)]">{pageT("detail.parent", "Parent")}</span>
              {task.parent ? (
                <Link className="text-right font-medium text-[var(--color-primary)] hover:underline" to={taskDetailHref(task.parent.taskCode)}>
                  {task.parent.taskCode}
                </Link>
              ) : (
                <span className="font-medium">{pageT("detail.none", "None")}</span>
              )}
            </div>
            <div className="flex justify-between gap-3"><span className="text-[var(--color-on-surface-variant)]">{pageT("detail.created", "Created")}</span><span className="font-medium">{formatDateTime(task.createdAt, { dateStyle: "medium", timeStyle: "short" })}</span></div>
            <div className="flex justify-between gap-3"><span className="text-[var(--color-on-surface-variant)]">{pageT("detail.updated", "Updated")}</span><span className="font-medium">{formatDateTime(task.updatedAt, { dateStyle: "medium", timeStyle: "short" })}</span></div>
            <div className="flex justify-between gap-3"><span className="text-[var(--color-on-surface-variant)]">{pageT("detail.archivedAt", "Archived At")}</span><span className="font-medium">{task.archivedAt ? formatDateTime(task.archivedAt, { dateStyle: "medium", timeStyle: "short" }) : pageT("detail.notArchived", "Not archived")}</span></div>
          </div>
        </div>

        <div>
          <h3 className="mb-3 border-b border-[var(--color-outline-variant)] pb-2 text-[11px] font-semibold uppercase text-[var(--color-on-surface-variant)]">{pageT("detail.subtasks", "Subtasks")}</h3>
          <div className="space-y-2">
            {task.subtasks.map((item) => (
              <Link className="block rounded-lg border border-[var(--color-outline-variant)] p-3 hover:border-[var(--color-primary)]" key={item.taskCode} to={taskDetailHref(item.taskCode)}>
                <div className="font-mono text-[11px] text-[var(--color-on-surface-variant)]">{item.taskCode}</div>
                <div className="mt-1 text-sm font-medium">{item.title}</div>
                <div className="mt-2 flex items-center justify-between gap-3">
                  <span className="text-xs text-[var(--color-on-surface-variant)]">{taskStatusLabel(item.status)}</span>
                  <span className="text-xs text-[var(--color-on-surface-variant)]">{item.progress}%</span>
                </div>
                <div className="mt-2"><ProgressBar value={item.progress} /></div>
              </Link>
            ))}
            {!task.subtasks.length ? <p className="text-sm text-[var(--color-on-surface-variant)]">{pageT("detail.noSubtasks", "No active subtasks.")}</p> : null}
          </div>
          {!archived ? (
            <>
              <div className="mt-4">
                <Field label={pageT("detail.newSubtaskTitle", "New Subtask Title")}>
                  <TextInput aria-label={pageT("detail.newSubtaskTitle", "New subtask title")} value={childTitle} onChange={(event) => setChildTitle(event.target.value)} />
                </Field>
              </div>
              <Button className="mt-3 w-full" disabled={!childTitle.trim()} variant="secondary" onClick={addSubtask}>{pageT("detail.addSubtask", "Add Subtask")}</Button>
            </>
          ) : null}
        </div>
      </SurfaceCard>
    </InspectorPanel>
  ), [
    archived,
    availableAppCodeSet,
    childTitle,
    dataSource,
    pageT,
    selectedAppCodes,
    task,
  ]);

  const rightPanelTabs = useMemo(() => ([
    {
      key: "task-history",
      label: pageT("detail.executionHistory.title", "Task History"),
      closable: false,
      children: <TaskExecutionHistoryPanel taskCode={task.taskCode} />,
    },
  ]), [pageT, task.taskCode]);

  useTabPanelConfig({ rightPanel, rightPanelTabs });

  useEffect(() => {
    openRightSidebar();
  }, [openRightSidebar, taskCode]);
  const breadcrumbs = [
    { label: pageT("title", "Tasks"), href: "/tasks" },
    ...(task.parent ? [{ label: task.parent.taskCode, href: taskDetailHref(task.parent.taskCode) }] : []),
    { label: task.taskCode },
  ];

  function resetDetailFields() {
    setTitle(task.title);
    setDescription(task.description ?? "");
    setTaskType(task.taskType);
    setSourceType(task.sourceType);
    setSourceUrl(task.sourceUrl ?? "");
    setDescriptionFullscreenOpen(false);
    setExecutionResultFullscreenOpen(false);
    setEditing(false);
  }

  async function saveDetailFields() {
    if (archived) {
      toast.warning(pageT("warnings.restoreBeforeEditing", "Restore archived task before editing"));
      return;
    }
    try {
      const payload = {
        agentCode: selectedAgentCode,
        description,
        modelCode: selectedModelCode,
        sourceType,
        sourceUrl,
        taskType,
        title,
      };
      const updated = dataSource === "api" ? await updateTask(task.taskCode, payload) : updateMockTask(task.taskCode, payload);
      setTask(updated);
      setDescriptionFullscreenOpen(false);
      setExecutionResultFullscreenOpen(false);
      setEditing(false);
      toast.success(pageT("messages.detailsSaved", "Task {{code}} details saved", { code: updated.taskCode }));
    } catch (error) {
      toast.error(error instanceof Error ? error.message : pageT("errors.update", "Task details could not be updated"));
    }
  }

  async function saveUpdateFields(overrides: Partial<{
    appCodes: string[];
    priority: TaskPriority;
    progress: number;
    status: TaskStatus;
  }> = {}, options: { onError?: () => void } = {}) {

    const draft = {
      appCodes: nextAppCodes,
      priority,
      progress: progressValue(progress),
      status,
      ...overrides,
    };
    const hasChanges = draft.priority !== task.priority
      || draft.status !== task.status
      || draft.progress !== task.progress
      || !sameCodes(draft.appCodes, task.appCodes);

    if (!hasChanges) return;

    try {
      if (archived) return;

      const appCodesChanged = !sameCodes(draft.appCodes, task.appCodes);
      if (appCodesChanged && draft.appCodes.some((code) => !availableAppCodeSet.has(code))) {
        options.onError?.();
        toast.warning(pageT("warnings.removeLegacyAppCodes", "Remove legacy app codes before changing app selection"));
        return;
      }

      const payload: UpdateTaskPayload = {};
      if (appCodesChanged) payload.appCodes = draft.appCodes;
      if (draft.priority !== task.priority) payload.priority = draft.priority;

      let updated = task;
      if (Object.keys(payload).length > 0) {
        updated = dataSource === "api" ? await updateTask(task.taskCode, payload) : updateMockTask(task.taskCode, payload);
      }
      if (draft.status !== task.status || draft.progress !== task.progress) {
        updated = dataSource === "api"
          ? await updateTaskStatus(task.taskCode, { progress: draft.progress, status: draft.status })
          : updateMockTaskStatus(task.taskCode, { progress: draft.progress, status: draft.status });
      }
      skipNextTaskSync.current = true;
      setTask(updated);
      toast.success(pageT("messages.updatesSaved", "Task {{code}} updates saved", { code: updated.taskCode }));
    } catch (error) {
      options.onError?.();
      toast.error(error instanceof Error ? error.message : pageT("errors.updates", "Updates could not be saved"));
    }
  }

  async function handleStatusChange(nextStatus: TaskStatus) {
    const currentTask = taskRef.current;
    const currentProgress = progressRef.current;
    const previousStatus = statusRef.current;
    const archivedNow = Boolean(currentTask.archivedAt);
    setStatus(nextStatus);
    statusRef.current = nextStatus;
    if (nextStatus === previousStatus) return;

    try {
      if (archivedNow) throw new Error(pageT("errors.archived", "Restore the archived task before updating it"));

      const updated = dataSource === "api"
        ? await updateTaskStatus(currentTask.taskCode, { progress: progressValue(currentProgress), status: nextStatus })
        : updateMockTaskStatus(currentTask.taskCode, { progress: progressValue(currentProgress), status: nextStatus });
      skipNextTaskSync.current = true;
      setTask(updated);
      setStatus(updated.status);
      statusRef.current = updated.status;
      setProgress(String(updated.progress));
      progressRef.current = String(updated.progress);
      toast.success(pageT("messages.updatesSaved", "Task {{code}} updates saved", { code: updated.taskCode }));
    } catch (error) {
      setStatus(previousStatus);
      statusRef.current = previousStatus;
      toast.error(error instanceof Error ? error.message : pageT("errors.updates", "Updates could not be saved"));
    }
  }

  function handlePriorityChange(nextPriority: TaskPriority) {
    setPriority(nextPriority);
    void saveUpdateFields({ priority: nextPriority });
  }

  function handleProgressBlur() {
    const raw = progressRef.current;
    if (raw === "" || !/^\d+$/.test(raw) || Number(raw) > 100) return;
    void saveUpdateFields({ progress: progressValue(raw) });
  }

  function handleProgressChange(nextProgress: string) {
    progressRef.current = nextProgress;
    setProgress(nextProgress);
  }

  function handleAppToggle(appCode: string) {
    const nextCodes = toggleCode(selectedAppCodes, appCode);
    setSelectedAppCodes(nextCodes);
    void saveUpdateFields({ appCodes: nextCodes }, {
      onError: () => setSelectedAppCodes(taskRef.current.appCodes),
    });
  }

  function handleLegacyAppRemove(appCode: string) {
    const nextCodes = selectedAppCodes.filter((code) => code !== appCode);
    setSelectedAppCodes(nextCodes);
    void saveUpdateFields({ appCodes: nextCodes }, {
      onError: () => setSelectedAppCodes(taskRef.current.appCodes),
    });
  }

  async function addSubtask() {
    if (archived) {
      toast.warning(pageT("warnings.restoreBeforeSubtasks", "Restore archived task before adding subtasks"));
      return;
    }
    try {
      const payload = {
        agentCodes: [],
        agentCode: task.agentCode,
        appCodes: selectedAppCodes.filter((code) => availableAppCodeSet.has(code)),
        assigneeUserCodes: [],
        description: `Subtask for ${task.taskCode}`,
        modelCode: task.modelCode,
        parentTaskCode: task.taskCode,
        priority: "Low" as TaskPriority,
        sourceType: "Internal" as TaskSourceType,
        sourceUrl: "",
        taskType: task.taskType,
        title: childTitle,
        workspaceCode: task.workspaceCode,
      };
      const child = dataSource === "api"
        ? await createTask(payload)
        : createMockTask(payload);
      refresh();
      toast.success(pageT("messages.subtaskCreated", "Subtask {{code}} created", { code: child.taskCode }));
    } catch (error) {
      toast.error(error instanceof Error ? error.message : pageT("errors.subtask", "Subtask could not be created"));
    }
  }

  async function handleArchive() {
    if (lifecyclePending || archived) return;
    setLifecyclePending(true);
    try {
      if (dataSource === "api") await archiveTask(task.taskCode);
      else archiveMockTask(task.taskCode);
      refresh();
      toast.success(pageT("messages.archived", "Task {{code}} archived", { code: task.taskCode }));
    } catch (error) {
      toast.error(error instanceof Error ? error.message : pageT("errors.archive", "Task could not be archived"));
    } finally {
      setLifecyclePending(false);
    }
  }

  async function handleRestore() {
    if (lifecyclePending || !archived) return;
    setLifecyclePending(true);
    try {
      if (dataSource === "api") await restoreTask(task.taskCode);
      else restoreMockTask(task.taskCode);
      refresh();
      toast.success(pageT("messages.restored", "Task {{code}} restored", { code: task.taskCode }));
    } catch (error) {
      toast.error(error instanceof Error ? error.message : pageT("errors.restore", "Task could not be restored"));
    } finally {
      setLifecyclePending(false);
    }
  }

  async function handleDelete() {
    if (lifecyclePending) return;
    setLifecyclePending(true);
    try {
      if (dataSource === "api") await deleteTask(task.taskCode);
      else deleteMockTask(task.taskCode);
      setDeleteConfirmOpen(false);
      toast.success(pageT("messages.deleted", "Task {{code}} deleted", { code: task.taskCode }));
      navigate("/tasks");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : pageT("errors.delete", "Task could not be deleted"));
    } finally {
      setLifecyclePending(false);
    }
  }

  async function handleExecute() {
    if (executing || archived) return;
    setExecuting(true);
    try {
      const result = dataSource === "api" ? await executeTask(task.taskCode) : executeMockTask(task.taskCode);
      toast.success(pageT("messages.executionStarted", "Task {{code}} execution started", { code: result.taskCode }), { title: result.runCode });
      navigate(`/chats?sessionCode=${encodeURIComponent(result.sessionCode)}&sourceTab=task`);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : t("pages:tasks.detail.errors.execution", "Task execution failed"));
    } finally {
      setExecuting(false);
    }
  }

  return (
    <>
      <PageHeader
        breadcrumbs={breadcrumbs}
        title={t("pages:tasks.detail.title", "Task Details")}
        actions={
          <>
            <Button disabled={executing || archived} size="lg" onClick={() => void handleExecute()}>
              {executing ? t("pages:tasks.detail.actions.executing", "Executing...") : t("pages:tasks.detail.actions.execute", "Execute")}
            </Button>
            {detailEditable ? (
              <>
                <Button size="lg" variant="outline" onClick={resetDetailFields}>{t("pages:tasks.actions.cancel", "Cancel")}</Button>
                <Button size="lg" disabled={!detailDirty} onClick={() => void saveDetailFields()}>{t("pages:tasks.actions.save", "Save")}</Button>
              </>
            ) : !archived ? (
              <Button size="lg" variant="outline" onClick={() => setEditing(true)}>{t("pages:tasks.detail.actions.edit", "Edit")}</Button>
            ) : null}
            {archived ? (
              <Button size="lg" variant="outline" disabled={lifecyclePending} onClick={() => void handleRestore()}>
                <RotateCcw size={14} aria-hidden="true" />
                {t("pages:tasks.detail.actions.restore", "Restore")}
              </Button>
            ) : (
              <Button size="lg" variant="outline" disabled={lifecyclePending} onClick={() => void handleArchive()}>
                <Archive size={14} aria-hidden="true" />
                {t("pages:tasks.detail.actions.archive", "Archive")}
              </Button>
            )}
            <Button size="lg" variant="destructive" disabled={lifecyclePending} onClick={() => setDeleteConfirmOpen(true)}>
              <Trash2 size={14} aria-hidden="true" />
              {t("pages:tasks.detail.actions.delete", "Delete")}
            </Button>
          </>
        }
      />
      <div className="space-y-5">
        <SurfaceCard>
          <div className="mb-5">
            <h2 className="text-base font-semibold text-[var(--color-on-surface)]">{t("pages:tasks.detail.title", "Task Details")}</h2>
            <p className="mt-1 font-mono text-sm text-[var(--color-on-surface-variant)]">{task.taskCode}</p>
          </div>
          <div className="grid gap-4 md:grid-cols-2">
            <Field label={t("pages:tasks.detail.status", "Task Status")}>
              <SelectInput aria-label={t("pages:tasks.detail.statusAria", "Task detail status")} disabled={archived} value={status} onValueChange={(event) => {
                void handleStatusChange(event as TaskStatus);
              }}>
                {availableStatuses.map((item) => <SelectItem key={item.statusCode} value={item.statusCode}>{taskFlowStatusDisplayName(item, flowT, options.flow.flowCode)}</SelectItem>)}
              </SelectInput>
            </Field>
            <Field label={t("pages:tasks.fields.priority", "Priority")}>
              {updatesEditable ? (
                <SelectInput aria-label={t("pages:tasks.detail.priorityAria", "Task detail priority")} value={priority} onValueChange={(event) => handlePriorityChange(event as TaskPriority)}>
                  {options.priorities.map((item) => <SelectItem key={item} value={item}>{pageT(`priorities.${item}`, item)}</SelectItem>)}
                </SelectInput>
              ) : (
                <ReadValue>{pageT(`priorities.${task.priority}`, task.priority)}</ReadValue>
              )}
            </Field>
            <Field label={t("pages:tasks.detail.progress", "Progress")}>
              {updatesEditable ? (
                <div className="flex min-w-0 items-center gap-3">
                  <input
                    aria-label={t("pages:tasks.detail.progressSliderAria", "Task detail progress slider")}
                    className="h-5 min-w-0 flex-1 cursor-pointer accent-[var(--color-primary)] disabled:cursor-not-allowed disabled:opacity-50"
                    disabled={progressManagedBySubtasks}
                    max="100"
                    min="0"
                    step="1"
                    type="range"
                    value={progressManagedBySubtasks ? task.progress : progressValue(progress)}
                    onChange={(event) => handleProgressChange(String(event.currentTarget.valueAsNumber))}
                    onPointerUp={(event) => event.currentTarget.blur()}
                    onBlur={handleProgressBlur}
                  />
                  <TextInput
                    aria-label={t("pages:tasks.detail.progressAria", "Task detail progress")}
                    className="w-20 shrink-0 text-right tabular-nums"
                    disabled={progressManagedBySubtasks}
                    max="100"
                    min="0"
                    type="text"
                    inputMode="numeric"
                    value={progressManagedBySubtasks ? String(task.progress) : progress}
                    onChange={(event) => { if (/^\d*$/.test(event.target.value)) handleProgressChange(event.target.value); }}
                    onBlur={handleProgressBlur}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") event.currentTarget.blur();
                    }}
                  />
                  <span className="shrink-0 text-sm text-[var(--color-on-surface-variant)]">%</span>
                </div>
              ) : (
                <ReadValue>{task.progress}%</ReadValue>
              )}
            </Field>
            <Field label={pageT("fields.executionAgent", "Execution Agent")}>
              {detailEditable ? (
                <OptionSelect
                  ariaLabel={pageT("aria.executionAgent", "Task execution agent")}
                  disabled={executionOptionsLoading}
                  loading={executionOptionsLoading}
                  loadingLabel={pageT("values.loadingAgents", "Loading agents...")}
                  options={executionAgentSelectOptions}
                  value={selectedAgentCode}
                  onChange={setSelectedAgentCode}
                />
              ) : (
                <ReadValue>{executionAgentLabel}</ReadValue>
              )}
            </Field>
            <Field label={pageT("fields.executionModel", "Execution Model")}>
              {detailEditable ? (
                <TaskExecutionModelSelect
                  ariaLabel={pageT("aria.executionModel", "Task execution model")}
                  emptyLabel={pageT("values.noModels", "No executable models")}
                  disabled={executionOptionsLoading || !visibleModelOptions.length}
                  loading={executionOptionsLoading}
                  loadingLabel={pageT("values.loadingModels", "Loading models...")}
                  options={visibleModelOptions}
                  value={selectedModelOptionCode}
                  onChange={setSelectedModelOptionCode}
                />
              ) : (
                <ReadValue>{task.modelCode || pageT("values.automaticModel", "Conversation Automatic Routing")}</ReadValue>
              )}
            </Field>
            <Field label={pageT("fields.title", "Title")}>
              {detailEditable ? (
                <TextInput aria-label={pageT("detail.titleAria", "Task detail title")} value={title} onChange={(event) => setTitle(event.target.value)} />
              ) : (
                <ReadValue>{task.title}</ReadValue>
              )}
            </Field>
            <Field label={pageT("fields.taskType", "Task Type")}>
              {detailEditable ? (
                <SelectInput aria-label={pageT("detail.typeAria", "Task detail type")} value={taskType} onValueChange={(event) => setTaskType(event as TaskType)}>
                  {options.taskTypes.map((item) => <SelectItem key={item} value={item}>{pageT(`types.${item}`, item)}</SelectItem>)}
                </SelectInput>
              ) : (
                <ReadValue>{pageT(`types.${task.taskType}`, task.taskType)}</ReadValue>
              )}
            </Field>
            <Field label={pageT("fields.sourceType", "Source Type")}>
              {detailEditable ? (
                <SelectInput aria-label={pageT("detail.sourceTypeAria", "Task detail source type")} value={sourceType} onValueChange={(event) => setSourceType(event as TaskSourceType)}>
                  {options.sourceTypes.map((item) => <SelectItem key={item} value={item}>{pageT(`sourceTypes.${item}`, item)}</SelectItem>)}
                </SelectInput>
              ) : (
                <ReadValue>{pageT(`sourceTypes.${task.sourceType}`, task.sourceType)}</ReadValue>
              )}
            </Field>
            <Field label={pageT("fields.sourceUrl", "Source URL")}>
              {detailEditable ? (
                <TextInput aria-label={pageT("detail.sourceUrlAria", "Task detail source URL")} placeholder={pageT("placeholders.sourceUrl", "Optional GitHub/GitLab URL")} value={sourceUrl} onChange={(event) => setSourceUrl(event.target.value)} />
              ) : (
                <ReadValue noneLabel={pageT("detail.none", "None")}>{task.sourceUrl}</ReadValue>
              )}
            </Field>
            <div className="md:col-span-2">
              <Field label={pageT("fields.apps", "Apps")} hint={pageT("hints.apps", "Only active apps from this workspace can be newly assigned.")}>
                <TaskAppSelectionField
                  availableApps={appOptions}
                  disabled={!updatesEditable}
                  emptyMessage={pageT("values.noActiveApps", "No active apps are available for this workspace.")}
                  legacyAppCodes={legacyAppCodes}
                  loading={workspaceOptionsLoading}
                  selectedAppCodes={selectedAppCodes}
                  onRemoveLegacy={updatesEditable ? handleLegacyAppRemove : undefined}
                  onToggle={updatesEditable ? handleAppToggle : undefined}
                />
              </Field>
            </div>
          </div>
          <div className="mt-4 grid gap-1.5">
            <div className="flex flex-wrap items-center justify-between gap-[12px]">
              <span className="text-[11px] font-semibold uppercase tracking-[0.05em] text-[var(--color-on-surface-variant)]">{pageT("fields.description", "Description")}</span>
              <Button
                size="lg"
                type="button"
                variant="ghost"
                aria-label={pageT("aria.openDescription", "Open description fullscreen")}
                onClick={() => setDescriptionFullscreenOpen(true)}
              >
                <Maximize2 size={14} aria-hidden="true" />
              </Button>
            </div>
            {detailEditable ? (
              <TextArea aria-label={pageT("detail.descriptionAria", "Task detail description")} className="min-h-36 w-full" value={description} onChange={(event) => setDescription(event.target.value)} />
            ) : (
              <MarkdownContent ariaLabel={pageT("detail.descriptionMarkdownAria", "Task description markdown")} className="max-h-80 overflow-y-auto pr-2" content={task.description} />
            )}
          </div>
          <div className="mt-4 grid gap-1.5 border-t border-[var(--color-outline-variant)] pt-4">
            <div className="flex flex-wrap items-center justify-between gap-[12px]">
              <span className="text-[11px] font-semibold uppercase tracking-[0.05em] text-[var(--color-on-surface-variant)]">{pageT("detail.executionResult", "Execution Result")}</span>
              <Button
                size="lg"
                type="button"
                variant="ghost"
                aria-label={pageT("detail.openExecutionResultFullscreen", "Open execution result fullscreen")}
                disabled={!task.executionResult}
                onClick={() => setExecutionResultFullscreenOpen(true)}
              >
                <Maximize2 size={14} aria-hidden="true" />
              </Button>
            </div>
            <MarkdownContent ariaLabel={pageT("detail.executionResultMarkdownAria", "Task execution result markdown")} className="max-h-80 overflow-y-auto pr-2" content={task.executionResult} />
          </div>
        </SurfaceCard>
      </div>
      {descriptionFullscreenOpen ? (
        <Dialog open={descriptionFullscreenOpen} onOpenChange={setDescriptionFullscreenOpen}>
          <DialogContent showCloseButton={false}
            className="gap-0 p-0 sm:max-w-none flex h-[min(88vh,64rem)] w-[min(96vw,72rem)] flex-col rounded-2xl border border-[var(--color-outline-variant)] bg-[var(--color-elevated-surface)] shadow-[0_20px_60px_rgba(0,0,0,0.28)]"
            aria-modal="true"
            aria-label={detailEditable ? pageT("detail.descriptionFullscreenEditorAria", "Description fullscreen editor") : pageT("detail.descriptionFullscreenPreviewAria", "Description fullscreen preview")}
          >
            <div className="flex items-center justify-between gap-3 border-b border-[var(--color-outline-variant)] px-5 py-4">
              <div>
                <h2 className="text-base font-semibold text-[var(--color-on-surface)]">{pageT("fields.description", "Description")}</h2>
                <p className="mt-1 text-[13px] text-[var(--color-on-surface-variant)]">
                  {detailEditable ? pageT("detail.descriptionEditHint", "Edit the task description in a larger workspace.") : pageT("detail.descriptionPreviewHint", "Preview the rendered markdown for this task description.")}
                </p>
              </div>
              <Button
                size="icon"
                type="button"
                variant="ghost"
                aria-label={pageT("aria.closeDescription", "Close description fullscreen")}
                onClick={() => setDescriptionFullscreenOpen(false)}
              >
                <X size={16} aria-hidden="true" />
              </Button>
            </div>
            <div className="min-h-0 flex-1 px-5 py-4">
              {detailEditable ? (
                <TextArea
                  aria-label={pageT("detail.descriptionFullscreenAria", "Task detail description fullscreen")}
                  className="h-full min-h-0 w-full resize-none"
                  value={description}
                  onChange={(event) => setDescription(event.target.value)}
                />
              ) : (
                <div className="h-full overflow-y-auto pr-1">
                  <MarkdownContent ariaLabel={pageT("detail.descriptionMarkdownFullscreenAria", "Task description markdown fullscreen")} className="max-w-none" content={task.description} />
                </div>
              )}
            </div>
          </DialogContent>
        </Dialog>
      ) : null}
      <ConfirmDialog
        open={deleteConfirmOpen}
        title={t("pages:tasks.detail.confirm.deleteTitle", "Delete task")}
        description={t("pages:tasks.detail.confirm.deleteDescription", "Task {{code}} and its execution result will be permanently deleted. This action cannot be undone.", { code: task.taskCode })}
        cancelText={t("pages:tasks.detail.confirm.cancel", "Cancel")}
        confirmText={t("pages:tasks.detail.confirm.delete", "Delete")}
        pendingText={t("pages:tasks.detail.confirm.deleting", "Deleting...")}
        pending={lifecyclePending}
        onCancel={() => { if (!lifecyclePending) setDeleteConfirmOpen(false); }}
        onConfirm={() => void handleDelete()}
      />
      {executionResultFullscreenOpen ? (
        <Dialog open={executionResultFullscreenOpen} onOpenChange={setExecutionResultFullscreenOpen}>
          <DialogContent showCloseButton={false}
            className="gap-0 p-0 sm:max-w-none flex h-[min(88vh,64rem)] w-[min(96vw,72rem)] flex-col rounded-2xl border border-[var(--color-outline-variant)] bg-[var(--color-elevated-surface)] shadow-[0_20px_60px_rgba(0,0,0,0.28)]"
            aria-modal="true"
            aria-label={pageT("detail.executionResultFullscreenAria", "Execution result fullscreen preview")}
          >
            <div className="flex items-center justify-between gap-3 border-b border-[var(--color-outline-variant)] px-5 py-4">
              <div>
                <h2 className="text-base font-semibold text-[var(--color-on-surface)]">{pageT("detail.executionResult", "Execution Result")}</h2>
                <p className="mt-1 text-[13px] text-[var(--color-on-surface-variant)]">{pageT("detail.executionResultHint", "Preview the final agent reply recorded for this task execution.")}</p>
              </div>
              <Button
                size="icon"
                type="button"
                variant="ghost"
                aria-label={pageT("detail.closeExecutionResultFullscreen", "Close execution result fullscreen")}
                onClick={() => setExecutionResultFullscreenOpen(false)}
              >
                <X size={16} aria-hidden="true" />
              </Button>
            </div>
            <div className="min-h-0 flex-1 px-5 py-4">
              <div className="h-full overflow-y-auto pr-1">
                <MarkdownContent ariaLabel={pageT("detail.executionResultMarkdownFullscreenAria", "Task execution result markdown fullscreen")} className="max-w-none" content={task.executionResult} />
              </div>
            </div>
          </DialogContent>
        </Dialog>
      ) : null}
    </>
  );
}
