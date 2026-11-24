import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useNavigate } from "react-router";
import {
  Archive,
  Plus,
  RefreshCcw,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/confirm-dialog";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { SelectInput } from "@/components/forms";
import { SearchField } from "@/components/search-field";
import {
  useTabPanelConfig,
  useWorkspaceSpaces,
} from "@/shared";
import { useToast } from "@/components/ui/toast";

import {
  useTaskKanbanColumns,
  useTaskOptions,
} from "@/domain/task/hooks";
import type {
  KanbanColumn,
  TaskListItem,
  TaskPriority,
  TaskStatus,
  TaskType,
} from "@/domain/task/model";
import {
  compareTaskListItems,
  taskFlowErrorMessage,
  taskFlowStatusDisplayName,
} from "@/domain/task/model";
import {
  archiveTask,
  deleteTask,
  executeTask,
  fetchWorkspaceTaskFlow,
  restoreTask,
  updateTaskStatus,
} from "@/domain/task/api";
import {
  archiveMockTask,
  deleteMockTask,
  executeMockTask,
  getMockWorkspaceTaskFlow,
  restoreMockTask,
  updateMockTaskStatus,
} from "@/domain/task/mock/data";
import { getServiceDataSource } from "@/shared/api/serviceConfig";
import { cn } from "@/shared/lib/utils";
import { SelectItem } from "@/components/ui/select";
import { useTranslation } from "react-i18next";
import { AgentAvatar } from "@/domain/agent/components/AgentAvatarPicker";
import { fetchWorkspaceInternalAgents } from "@/domain/agent/api";
import { listWorkspaceInternalAgentsMock } from "@/domain/agent/mock";
import type { WorkspaceInternalAgentRecord } from "@/domain/agent/model";
import { DEFAULT_TAB_ID, useTabStore } from "@/store/tabStore";

type BoardScalePreset = "fit" | "100" | "90" | "80" | "70" | "60" | "50";
type ArchivedFilter = "all" | "archived" | "active";

const BOARD_SCALE_STORAGE_KEY = "tron.tasks.boardScale";
const BOARD_BASE_COLUMN_WIDTH = 320;
const BOARD_BASE_GAP = 16;
const BOARD_BASE_COLUMN_PADDING = 12;
const ROOT_PARENT_TASK_FILTER = "__ROOT__";
const BOARD_SCALE_PRESETS: Array<{ label: string; value: Exclude<BoardScalePreset, "fit"> }> = [
  { label: "100%", value: "100" },
  { label: "90%", value: "90" },
  { label: "80%", value: "80" },
  { label: "70%", value: "70" },
  { label: "60%", value: "60" },
  { label: "50%", value: "50" },
];
const EMPTY_RIGHT_PANEL_TABS: never[] = [];

function resolveStoredBoardScale(value: string | null): BoardScalePreset {
  if (value === "100" || value === "90" || value === "80" || value === "70" || value === "60" || value === "50") return value;
  return "fit";
}

function presetScaleFactor(preset: BoardScalePreset) {
  if (preset === "fit") return null;
  return Number(preset) / 100;
}

function ProgressBar({ className, value }: { className?: string; value: number }) {
  return (
    <div className={cn("h-1.5 overflow-hidden rounded-full bg-[var(--color-surface-container-high)]", className)}>
      <div className="h-full rounded-full bg-[var(--color-primary)]" style={{ width: `${Math.min(100, Math.max(0, value))}%` }} />
    </div>
  );
}

function taskTypeMarkerClass(taskType: TaskType) {
  if (taskType === "Bug") return "bg-[var(--color-error)]";
  return "bg-[var(--color-primary)]";
}

function taskPriorityTone(priority: TaskPriority) {
  if (priority === "Extra High" || priority === "High") return "error";
  if (priority === "Medium") return "warning";
  return "outline";
}

function reorderKanbanColumns(
  columns: KanbanColumn[],
  task: TaskListItem,
  nextStatus: TaskStatus,
) {
  return columns.map((column) => {
    if (column.status === task.status && column.status === nextStatus) {
      return column;
    }

    if (column.status === task.status) {
      return {
        ...column,
        tasks: column.tasks.filter((item) => item.taskCode !== task.taskCode),
      };
    }

    if (column.status === nextStatus) {
      const movedTask = {
        ...task,
        status: nextStatus,
      };
      return {
        ...column,
        tasks: [...column.tasks.filter((item) => item.taskCode !== task.taskCode), movedTask].sort(compareTaskListItems),
      };
    }

    return column;
  });
}

function buildTaskDragPreview(card: HTMLButtonElement) {
  const bounds = card.getBoundingClientRect();
  const previewCard = card.cloneNode(true);
  if (!(previewCard instanceof HTMLButtonElement)) return null;

  const preview = document.createElement("div");
  const previewPadding = 18;
  preview.setAttribute("aria-hidden", "true");
  Object.assign(preview.style, {
    background: "transparent",
    left: "-1000px",
    padding: `${previewPadding}px`,
    pointerEvents: "none",
    position: "fixed",
    top: "-1000px",
    width: `${bounds.width + previewPadding * 2}px`,
    zIndex: "9999",
  });

  previewCard.setAttribute("aria-hidden", "true");
  previewCard.tabIndex = -1;
  Object.assign(previewCard.style, {
    background: "var(--color-elevated-surface-strong)",
    borderColor: "var(--color-primary)",
    boxShadow: "0 18px 40px rgba(15, 23, 42, 0.18)",
    margin: "0",
    opacity: "1",
    transform: "translateY(-1px)",
    width: `${bounds.width}px`,
  });

  preview.appendChild(previewCard);
  document.body.appendChild(preview);
  return {
    bounds,
    preview,
    previewOffsetX: previewPadding + Math.min(bounds.width / 2, 96),
    previewOffsetY: previewPadding + 24,
  };
}

function KanbanCard({
  compact,
  dense,
  dragging,
  onArchive,
  onDelete,
  onDragEnd,
  onDragStart,
  onExecute,
  onRestore,
  onSelect,
  pending,
  selected,
  agentAvatarId,
  agentName,
  archived,
  task,
}: {
  task: TaskListItem;
  agentAvatarId?: string;
  agentName: string;
  archived: boolean;
  selected: boolean;
  compact: boolean;
  dense: boolean;
  dragging: boolean;
  pending: boolean;
  onArchive: () => void;
  onDelete: () => void;
  onDragEnd: () => void;
  onDragStart: (event: React.DragEvent<HTMLButtonElement>) => void;
  onExecute: () => void;
  onRestore: () => void;
  onSelect: () => void;
}) {
  const { t } = useTranslation(["pages"]);
  const priorityLabel = t(`pages:tasks.priorities.${task.priority}`, task.priority);
  const taskTypeLabel = t(`pages:tasks.types.${task.taskType}`, task.taskType);
  return (
    <ContextMenu>
      <ContextMenuTrigger className="block w-full text-left">
    <button
      aria-busy={pending}
      className={cn(
        "relative z-0 w-full rounded-xl border bg-[var(--color-elevated-surface)] text-left shadow-[0_2px_6px_rgba(15,23,42,0.05)] transition-[box-shadow,border-color,background-color,opacity] duration-150 hover:z-10 hover:border-[var(--color-outline)] hover:shadow-[0_3px_10px_rgba(15,23,42,0.06)]",
        dense ? "p-2" : compact ? "p-2.5" : "p-3",
        selected ? "border-[var(--color-primary)] shadow-[0_2px_6px_rgba(15,23,42,0.05)]" : "border-[var(--color-outline-variant)]",
        dragging ? "translate-y-0 border-dashed border-[var(--color-primary)]/35 bg-[var(--color-primary)]/5 shadow-none hover:translate-y-0 hover:shadow-none" : "",
        pending ? "cursor-wait opacity-70" : "cursor-grab active:cursor-grabbing",
        archived
          ? "cursor-pointer overflow-hidden border-dashed border-[var(--color-outline)] bg-[var(--color-surface-container)] shadow-none hover:border-[var(--color-outline)] hover:shadow-none"
          : "",
      )}
      data-theme-anchor="card"
      data-archived={archived || undefined}
      data-testid={`task-kanban-card-${task.taskCode}`}
      draggable={!pending && !archived}
      type="button"
      onDragEnd={onDragEnd}
      onDragStart={onDragStart}
      onClick={onSelect}
    >
      {archived ? (
        <span
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-[inherit] bg-[var(--color-surface-container)]/35"
          style={{
            backgroundImage:
              "repeating-linear-gradient(135deg, var(--color-outline-variant) 0 1px, transparent 1px 10px)",
          }}
        >
          <Archive className="size-7 text-[var(--color-on-surface-variant)] opacity-20" />
        </span>
      ) : null}
      <div className={cn("flex min-w-0 items-center justify-between gap-[12px]", dense ? "mb-1" : compact ? "mb-1.5" : "mb-2")}>
        <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-[var(--color-on-surface-variant)]">{task.taskCode}</span>
        <div className="flex shrink-0 items-center gap-1">
          {archived ? (
            <span className="sr-only">{t("pages:tasks.kanban.archivedBadge", "Archived")}</span>
          ) : null}
          <Badge className="max-w-[5.5rem] shrink-0 truncate whitespace-nowrap px-2 text-[11px]" title={priorityLabel} variant={taskPriorityTone(task.priority)}>{priorityLabel}</Badge>
        </div>
      </div>
      <div className={cn("flex min-w-0 items-start gap-[12px]", dense ? "mb-1.5" : compact ? "mb-2" : "mb-3")}>
        <span
          aria-label={taskTypeLabel}
          className={cn("mt-1 h-3 w-1 shrink-0 rounded-full", taskTypeMarkerClass(task.taskType))}
          title={taskTypeLabel}
        />
        <h3 className={cn("min-w-0 flex-1 font-medium leading-5", dense ? "text-[12px]" : compact ? "text-[13px]" : "text-sm")}>{task.title}</h3>
      </div>
      <ProgressBar value={task.progress} />
      <div className={cn("flex items-center justify-between gap-[12px]", dense ? "mt-2" : compact ? "mt-2.5" : "mt-3")}>
        <span className="text-[11px] text-[var(--color-on-surface-variant)]">{t("pages:tasks.kanban.subtasks", "{{count}} subtasks", { count: task.subtaskCount })}</span>
        <AgentAvatar
          alt={agentName}
          avatarId={agentAvatarId}
          className="size-6 shrink-0 rounded-full border border-[var(--color-outline-variant)]"
        />
      </div>
    </button>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem disabled={pending || archived} onClick={onExecute}>
          {t("pages:tasks.detail.actions.execute", "Execute")}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem disabled={pending || archived} onClick={onArchive}>
          {t("pages:tasks.detail.actions.archive", "Archive")}
        </ContextMenuItem>
        <ContextMenuItem disabled={pending || !archived} onClick={onRestore}>
          {t("pages:tasks.detail.actions.restore", "Restore")}
        </ContextMenuItem>
        <ContextMenuItem
          className="text-[var(--color-error)]"
          disabled={pending}
          onClick={onDelete}
        >
          {t("pages:tasks.detail.actions.delete", "Delete")}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

export function TasksKanbanPage() {
  const { t } = useTranslation(["pages"]);
  const flowT = useCallback(
    (key: string, defaultValue: string) => t(`pages:tasks.taskFlows.${key}`, defaultValue),
    [t],
  );
  const [searchInput, setSearchInput] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [taskType, setTaskType] = useState<TaskType | "">("");
  const [priority, setPriority] = useState("");
  const [archivedFilter, setArchivedFilter] = useState<ArchivedFilter>("active");
  const [selectedTaskCode, setSelectedTaskCode] = useState<string | null>(null);
  const [draggedTask, setDraggedTask] = useState<TaskListItem | null>(null);
  const [dragOverState, setDragOverState] = useState<{ allowed: boolean; status: TaskStatus } | null>(null);
  const [pendingTaskCode, setPendingTaskCode] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<TaskListItem | null>(null);
  const [deletePending, setDeletePending] = useState(false);
  const [boardScalePreset, setBoardScalePreset] = useState<BoardScalePreset>(() => {
    if (typeof window === "undefined") return "fit";
    return resolveStoredBoardScale(window.localStorage.getItem(BOARD_SCALE_STORAGE_KEY));
  });
  const [boardViewportWidth, setBoardViewportWidth] = useState(0);
  const [workspaceInternalAgents, setWorkspaceInternalAgents] = useState<WorkspaceInternalAgentRecord[]>([]);
  const navigate = useNavigate();
  const activeTabId = useTabStore((state) => state.activeTabId ?? DEFAULT_TAB_ID);
  const updateRightPanelState = useTabStore((state) => state.updateRightPanelState);
  const toast = useToast();
  const { selectedPrimarySpace, selectedSecondarySpace } = useWorkspaceSpaces();
  const boardViewportRef = useRef<HTMLDivElement | null>(null);
  const dragPreviewRef = useRef<HTMLDivElement | null>(null);
  const dataSource = getServiceDataSource("task");
  const agentDataSource = getServiceDataSource("agent");
  const activeWorkspace = selectedSecondarySpace ?? selectedPrimarySpace;
  const workspaceCode = activeWorkspace?.workspaceCode ?? "";
  const options = useTaskOptions(workspaceCode);
  const [workspaceFlowCode, setWorkspaceFlowCode] = useState("");

  useTabPanelConfig({ rightPanel: null, rightPanelTabs: EMPTY_RIGHT_PANEL_TABS });

  useEffect(() => {
    updateRightPanelState(activeTabId, { open: false });
  }, [activeTabId, updateRightPanelState]);

  useEffect(() => {
    if (!workspaceCode) {
      setWorkspaceInternalAgents([]);
      return;
    }

    const controller = new AbortController();
    const loadAgents = agentDataSource === "api"
      ? fetchWorkspaceInternalAgents(workspaceCode, controller.signal)
      : Promise.resolve(listWorkspaceInternalAgentsMock(workspaceCode));

    loadAgents
      .then((agents) => {
        if (!controller.signal.aborted) setWorkspaceInternalAgents(agents);
      })
      .catch((error: unknown) => {
        if (controller.signal.aborted) return;
        console.warn("[task] Internal agent avatars could not be loaded.", error);
        setWorkspaceInternalAgents([]);
      });

    return () => controller.abort();
  }, [agentDataSource, workspaceCode]);

  useEffect(() => {
    if (!workspaceCode) {
      setWorkspaceFlowCode("");
      return;
    }
    const controller = new AbortController();
    const loadFlows = async () => {
      try {
        setWorkspaceFlowCode("");
        const workspaceFlow = dataSource === "api"
          ? await fetchWorkspaceTaskFlow(workspaceCode, controller.signal)
          : getMockWorkspaceTaskFlow(workspaceCode);
        if (controller.signal.aborted) return;
        setWorkspaceFlowCode(workspaceFlow.flow.flowCode);
      } catch (error) {
        if (!controller.signal.aborted) {
          toast.error(taskFlowErrorMessage(error, flowT, "errors.load", "Task flows could not be loaded"));
        }
      }
    };
    void loadFlows();
    return () => controller.abort();
  }, [dataSource, flowT, toast, workspaceCode]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDebouncedSearch(searchInput.trim());
    }, 280);

    return () => window.clearTimeout(timer);
  }, [searchInput]);

  const taskQuery = useMemo(() => ({
    includeArchived: archivedFilter === "active" ? undefined : true,
    parentTaskCode: ROOT_PARENT_TASK_FILTER,
    priority: priority || undefined,
    search: debouncedSearch,
    taskType: taskType || undefined,
    workspaceCode,
  }), [archivedFilter, debouncedSearch, priority, taskType, workspaceCode]);
  const { columns, loading, refresh } = useTaskKanbanColumns(taskQuery);
  const [optimisticColumns, setOptimisticColumns] = useState<KanbanColumn[] | null>(null);
  const internalAgentsByCode = useMemo(
    () => new Map(workspaceInternalAgents.map((agent) => [agent.agentCode, agent])),
    [workspaceInternalAgents],
  );

  useEffect(() => {
    setOptimisticColumns(null);
  }, [columns]);

  const displayedColumns = optimisticColumns ?? columns;
  const rootTaskColumns = useMemo(() => (
    displayedColumns.map((column) => ({
      ...column,
      title: taskFlowStatusDisplayName(
        { name: column.title, statusCode: column.status },
        flowT,
        workspaceFlowCode,
      ),
      tasks: column.tasks.filter((task) => {
        if (task.parentTaskCode) return false;
        if (archivedFilter === "archived") return Boolean(task.archivedAt);
        if (archivedFilter === "active") return !task.archivedAt;
        return true;
      }),
    }))
  ), [archivedFilter, displayedColumns, flowT, workspaceFlowCode]);

  useEffect(() => {
    setSelectedTaskCode(null);
  }, [archivedFilter, taskType, workspaceCode]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    window.localStorage.setItem(BOARD_SCALE_STORAGE_KEY, boardScalePreset);
  }, [boardScalePreset]);

  useEffect(() => {
    const viewport = boardViewportRef.current;
    if (!viewport) return undefined;

    const updateViewportWidth = () => {
      setBoardViewportWidth(viewport.clientWidth);
    };

    updateViewportWidth();

    const observer = new ResizeObserver((entries) => {
      const nextWidth = entries[0]?.contentRect.width ?? viewport.clientWidth;
      setBoardViewportWidth(nextWidth);
    });

    observer.observe(viewport);
    return () => observer.disconnect();
  }, []);

  const fitBoardScale = useMemo(() => {
    if (!columns.length || boardViewportWidth <= 0) return 1;
    const naturalBoardWidth = columns.length * BOARD_BASE_COLUMN_WIDTH + Math.max(0, columns.length - 1) * BOARD_BASE_GAP;
    return Math.min(1, boardViewportWidth / naturalBoardWidth);
  }, [boardViewportWidth, columns.length]);

  const boardScale = presetScaleFactor(boardScalePreset) ?? fitBoardScale;
  const boardScalePercent = Math.round(boardScale * 100);
  const boardLayout = useMemo(() => ({
    compact: boardScale < 0.86,
    dense: boardScale < 0.68,
    columnGap: BOARD_BASE_GAP * boardScale,
    columnPadding: BOARD_BASE_COLUMN_PADDING * boardScale,
    columnWidth: BOARD_BASE_COLUMN_WIDTH * boardScale,
  }), [boardScale]);

  const parseDraggedTask = useCallback((event: React.DragEvent<HTMLElement>) => {
    if (draggedTask) return draggedTask;
    const serialized = event.dataTransfer.getData("application/json");
    if (!serialized) return null;
    try {
      return JSON.parse(serialized) as TaskListItem;
    } catch {
      return null;
    }
  }, [draggedTask]);

  const canMoveTaskToStatus = useCallback((_task: TaskListItem, _status: TaskStatus) => true, []);

  const cleanupDragPreview = useCallback(() => {
    dragPreviewRef.current?.remove();
    dragPreviewRef.current = null;
  }, []);

  const clearDragState = useCallback(() => {
    cleanupDragPreview();
    setDraggedTask(null);
    setDragOverState(null);
  }, [cleanupDragPreview]);

  useEffect(() => cleanupDragPreview, [cleanupDragPreview]);

  const handleCardDragStart = useCallback((task: TaskListItem, event: React.DragEvent<HTMLButtonElement>) => {
    if (pendingTaskCode || task.archivedAt) {
      event.preventDefault();
      return;
    }
    clearDragState();
    setDraggedTask(task);
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", task.taskCode);
    event.dataTransfer.setData("application/json", JSON.stringify(task));
    const dragPreview = buildTaskDragPreview(event.currentTarget);
    if (!dragPreview) return;
    dragPreviewRef.current = dragPreview.preview;
    event.dataTransfer.setDragImage(dragPreview.preview, dragPreview.previewOffsetX, dragPreview.previewOffsetY);
  }, [clearDragState, pendingTaskCode]);

  const runTaskLifecycle = useCallback(async (
    task: TaskListItem,
    action: () => Promise<unknown>,
    successMessage: string,
    errorMessage: string,
  ) => {
    if (pendingTaskCode) return;
    setPendingTaskCode(task.taskCode);
    try {
      await action();
      refresh();
      toast.success(successMessage);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : errorMessage);
    } finally {
      setPendingTaskCode(null);
    }
  }, [pendingTaskCode, refresh, toast]);

  const handleArchiveTask = useCallback((task: TaskListItem) => {
    void runTaskLifecycle(
      task,
      () => dataSource === "api" ? archiveTask(task.taskCode) : Promise.resolve(archiveMockTask(task.taskCode)),
      t("pages:tasks.messages.archived", "Task {{code}} archived", { code: task.taskCode }),
      t("pages:tasks.errors.archive", "Task could not be archived"),
    );
  }, [dataSource, runTaskLifecycle, t]);

  const handleRestoreTask = useCallback((task: TaskListItem) => {
    void runTaskLifecycle(
      task,
      () => dataSource === "api" ? restoreTask(task.taskCode) : Promise.resolve(restoreMockTask(task.taskCode)),
      t("pages:tasks.messages.restored", "Task {{code}} restored", { code: task.taskCode }),
      t("pages:tasks.errors.restore", "Task could not be restored"),
    );
  }, [dataSource, runTaskLifecycle, t]);

  const handleExecuteTask = useCallback(async (task: TaskListItem) => {
    if (pendingTaskCode || task.archivedAt) return;
    setPendingTaskCode(task.taskCode);
    try {
      const result = dataSource === "api"
        ? await executeTask(task.taskCode)
        : executeMockTask(task.taskCode);
      refresh();
      toast.success(
        t("pages:tasks.messages.executionStarted", "Task {{code}} execution started", { code: result.taskCode }),
        { title: result.runCode },
      );
      navigate(`/chats?sessionCode=${encodeURIComponent(result.sessionCode)}&sourceTab=task`, { state: { taskExecution: result } });
    } catch (error) {
      toast.error(error instanceof Error ? error.message : t("pages:tasks.detail.errors.execution", "Task execution failed"));
    } finally {
      setPendingTaskCode(null);
    }
  }, [dataSource, navigate, pendingTaskCode, refresh, t, toast]);

  const handleDeleteTask = useCallback(async () => {
    const target = deleteTarget;
    if (!target || deletePending) return;
    setDeletePending(true);
    try {
      if (dataSource === "api") await deleteTask(target.taskCode);
      else deleteMockTask(target.taskCode);
      setDeleteTarget(null);
      refresh();
      toast.success(t("pages:tasks.messages.deleted", "Task {{code}} deleted", { code: target.taskCode }));
    } catch (error) {
      toast.error(error instanceof Error ? error.message : t("pages:tasks.errors.delete", "Task could not be deleted"));
    } finally {
      setDeletePending(false);
    }
  }, [dataSource, deletePending, deleteTarget, refresh, t, toast]);

  const handleColumnDragOver = useCallback((status: TaskStatus, event: React.DragEvent<HTMLElement>) => {
    const task = parseDraggedTask(event);
    if (!task || pendingTaskCode) return;
    event.preventDefault();
    if (task.status === status) {
      setDragOverState(null);
      event.dataTransfer.dropEffect = "move";
      return;
    }
    const allowed = canMoveTaskToStatus(task, status);
    event.dataTransfer.dropEffect = "move";
    setDragOverState({ allowed, status });
  }, [canMoveTaskToStatus, parseDraggedTask, pendingTaskCode]);

  const handleColumnDrop = useCallback(async (status: TaskStatus, event: React.DragEvent<HTMLElement>) => {
    event.preventDefault();
    setDragOverState(null);

    const task = parseDraggedTask(event);
    cleanupDragPreview();
    setDraggedTask(null);
    if (!task || pendingTaskCode) return;
    if (task.status === status) return;

    if (!canMoveTaskToStatus(task, status)) {
      toast.warning(t("pages:tasks.taskFlows.messages.statusMoveDenied", "Task {{code}} cannot change status", { code: task.taskCode }));
      return;
    }

    setPendingTaskCode(task.taskCode);
    try {
      if (dataSource === "api") {
        await updateTaskStatus(task.taskCode, { status });
      } else {
        updateMockTaskStatus(task.taskCode, { status });
      }
      setOptimisticColumns(reorderKanbanColumns(displayedColumns, task, status));
      refresh();
      toast.success(t("pages:tasks.taskFlows.messages.statusMoved", "Task {{code}} moved", { code: task.taskCode }));
    } catch (error) {
      setOptimisticColumns(null);
      refresh();
      toast.error(taskFlowErrorMessage(error, flowT, "errors.statusNotInFlow", "Task status is unavailable in the current workflow"));
    } finally {
      setPendingTaskCode(null);
    }
  }, [canMoveTaskToStatus, cleanupDragPreview, dataSource, displayedColumns, flowT, parseDraggedTask, pendingTaskCode, refresh, t, toast]);

  return (
    <div className="@container flex min-h-full min-w-0 flex-col">
      <div className="mb-4 flex min-w-0 items-center gap-3 overflow-x-auto pb-1">
        <div className="min-w-48 flex-1">
          <SearchField ariaLabel={t("pages:tasks.kanban.search", "Search")} placeholder={t("pages:tasks.kanban.searchPlaceholder", "Filter task code, title, or description...")} value={searchInput} onChange={setSearchInput} />
        </div>
        <div className="w-40 shrink-0">
          <SelectInput aria-label={t("pages:tasks.kanban.taskTypeFilter", "Task type filter")} value={taskType} onValueChange={(event) => setTaskType(event as TaskType | "")}>
            <SelectItem value="">{t("pages:tasks.kanban.allTypes", "All")}</SelectItem>
            {options.taskTypes.map((item) => <SelectItem key={item} value={item}>{t(`pages:tasks.types.${item}`, item)}</SelectItem>)}
          </SelectInput>
        </div>
        <div className="w-40 shrink-0">
          <SelectInput aria-label={t("pages:tasks.kanban.priorityFilter", "Task priority filter")} value={priority} onValueChange={(event) => setPriority(event!)}>
            <SelectItem value="">{t("pages:tasks.kanban.allPriorities", "All priorities")}</SelectItem>
            {options.priorities.map((item) => <SelectItem key={item} value={item}>{t(`pages:tasks.priorities.${item}`, item)}</SelectItem>)}
          </SelectInput>
        </div>
        <div className="w-40 shrink-0">
          <SelectInput aria-label={t("pages:tasks.kanban.archivedFilter", "Task archive filter")} value={archivedFilter} onValueChange={(event) => setArchivedFilter(event as ArchivedFilter)}>
            <SelectItem value="all">{t("pages:tasks.kanban.archivedFilterAll", "All")}</SelectItem>
            <SelectItem value="archived">{t("pages:tasks.kanban.archivedFilterArchived", "Archived")}</SelectItem>
            <SelectItem value="active">{t("pages:tasks.kanban.archivedFilterActive", "Not archived")}</SelectItem>
          </SelectInput>
        </div>
        <SelectInput aria-label={t("pages:tasks.kanban.scale", "Task board scale")} className="ml-auto w-40 shrink-0 bg-[var(--color-elevated-surface)]" value={boardScalePreset} onValueChange={(event) => setBoardScalePreset(event as BoardScalePreset)}>
          <SelectItem value="fit">{t("pages:tasks.taskFlows.fit", "Fit ({{percent}}%)", { percent: boardScalePercent })}</SelectItem>
          {BOARD_SCALE_PRESETS.map((preset) => <SelectItem key={preset.value} value={preset.value}>{preset.label}</SelectItem>)}
        </SelectInput>
        <Button size="lg" variant="outline" className="shrink-0" onClick={() => refresh()}>{flowT("actions.refresh", "Refresh")}</Button>
        <Button size="lg" className="shrink-0" onClick={() => navigate("/tasks/new")}>{t("pages:tasks.actions.new", "New")}</Button>
      </div>
      <div className="flex min-h-64 min-w-0 flex-1 overflow-x-auto overscroll-x-contain pb-4" ref={boardViewportRef} style={{ gap: `${boardLayout.columnGap}px` }}>
        {rootTaskColumns.map((column) => (
          <section
            aria-label={t("pages:tasks.taskFlows.aria.taskColumn", "{{name}} task column", { name: column.title })}
            className={cn(
              "flex min-w-0 shrink-0 grow flex-col overflow-hidden rounded-2xl bg-[var(--color-surface-container)] transition-[background-color,box-shadow,border-color] duration-150",
              dragOverState?.status === column.status
                ? dragOverState.allowed
                  ? "bg-[var(--color-primary)]/6 ring-1 ring-inset ring-[var(--color-primary)]/20"
                  : "bg-[var(--color-error-container)]/40 ring-1 ring-inset ring-[var(--color-error)]/15"
                : "",
            )}
            data-theme-anchor="panel"
            data-task-kanban-column={column.status}
            data-testid={`task-kanban-dropzone-${column.status}`}
            key={column.status}
            onDragLeave={(event) => {
              if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
                setDragOverState(null);
              }
            }}
            onDragOver={(event) => handleColumnDragOver(column.status, event)}
            onDrop={(event) => handleColumnDrop(column.status, event)}
            style={{
              gap: `${boardLayout.columnGap}px`,
              padding: `${boardLayout.columnPadding}px`,
              width: `${boardLayout.columnWidth}px`,
            }}
          >
            <div className="flex items-center justify-between border-b-2 border-[var(--color-outline-variant)] px-1 py-1">
              <h2 className="min-w-0 truncate text-[11px] font-semibold uppercase text-[var(--color-on-surface-variant)]" title={column.title}>{column.title}</h2>
              <span
                className={cn(
                  "rounded-full px-2.5 py-0.5 text-[11px] transition-colors",
                  dragOverState?.status === column.status
                    ? dragOverState.allowed
                      ? "bg-[var(--color-primary)] text-[var(--color-on-primary)]"
                      : "bg-[var(--color-error-container)] text-[var(--color-on-error-container)]"
                    : "bg-[var(--color-surface-container-high)]",
                )}
              >
                {column.tasks.length}
              </span>
            </div>
            <div
              className="min-h-0 flex-1 space-y-3 overflow-y-auto"
              onDragOver={(event) => handleColumnDragOver(column.status, event)}
              onDrop={(event) => handleColumnDrop(column.status, event)}
            >
              {dragOverState?.status === column.status ? (
                <div
                  className={cn(
                    "rounded-xl border border-dashed px-3 py-2 text-[11px] font-medium transition-colors",
                    dragOverState.allowed
                      ? "border-[var(--color-primary)]/35 bg-[var(--color-elevated-surface-strong)] text-[var(--color-primary)]"
                      : "border-[var(--color-error)]/30 bg-[var(--color-error-container)]/45 text-[var(--color-error)]",
                  )}
                >
                  {dragOverState.allowed
                    ? t("pages:tasks.taskFlows.drag.releaseToMove", "Release to move into {{status}}", { status: column.title })
                    : t("pages:tasks.taskFlows.drag.moveDenied", "Task {{code}} cannot move into {{status}}", { code: draggedTask?.taskCode ?? "Task", status: column.title })}
                </div>
              ) : null}
              {column.tasks.map((task) => {
                const agent = internalAgentsByCode.get(task.agentCode);
                return (
                  <KanbanCard
                    key={task.taskCode}
                    task={task}
                    archived={Boolean(task.archivedAt)}
                    onArchive={() => handleArchiveTask(task)}
                    onDelete={() => setDeleteTarget(task)}
                    onExecute={() => void handleExecuteTask(task)}
                    onRestore={() => handleRestoreTask(task)}
                    agentAvatarId={agent?.avatarId}
                    agentName={agent?.name || task.agentCode}
                    compact={boardLayout.compact}
                    dense={boardLayout.dense}
                    dragging={draggedTask?.taskCode === task.taskCode}
                    pending={pendingTaskCode === task.taskCode}
                    selected={selectedTaskCode === task.taskCode}
                    onDragEnd={() => {
                      clearDragState();
                    }}
                    onDragStart={(event) => handleCardDragStart(task, event)}
                    onSelect={() => {
                      setSelectedTaskCode(task.taskCode);
                      navigate(`/tasks/detail?taskCode=${encodeURIComponent(task.taskCode)}`);
                    }}
                  />
                );
              })}
            </div>
          </section>
        ))}
      </div>
      <ConfirmDialog
        open={Boolean(deleteTarget)}
        title={t("pages:tasks.detail.confirm.deleteTitle", "Delete task")}
        description={t("pages:tasks.detail.confirm.deleteDescription", "Task {{code}} and its execution result will be permanently deleted. This action cannot be undone.", { code: deleteTarget?.taskCode ?? "" })}
        cancelText={t("pages:tasks.detail.confirm.cancel", "Cancel")}
        confirmText={t("pages:tasks.detail.confirm.delete", "Delete")}
        pendingText={t("pages:tasks.detail.confirm.deleting", "Deleting...")}
        pending={deletePending}
        onCancel={() => { if (!deletePending) setDeleteTarget(null); }}
        onConfirm={() => void handleDeleteTask()}
      />
    </div>
  );
}
