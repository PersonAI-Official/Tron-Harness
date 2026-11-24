import type {
  AutomationDetailData,
  AutomationExecutionListItem,
  AutomationExecutionListQuery,
  AutomationExecutionStatus,
  AutomationConcurrencyMode,
  AutomationLifecycleResult,
  AutomationListItem,
  AutomationListQuery,
  AutomationOperationalStatus,
  AutomationOptions,
  AutomationPage,
  AutomationStatus,
  CreateAutomationPayload,
  UpdateAutomationPayload,
  UpdateAutomationStatusPayload,
} from "@/domain/automation/model";

const NOW = "2026-05-19T12:00:00Z";
const EMPTY_PAGE_SIZE = 15;

interface AutomationSeed {
  agentCode: string;
  archivedAt: string | null;
  automationCode: string;
  createdAt: string;
  cronExpression: string;
  concurrencyMode: AutomationConcurrencyMode;
  description: string | null;
  id: string;
  lastExecutionCode: string | null;
  lastExecutionStatus: AutomationExecutionStatus | null;
  lastExecutionSummary: string | null;
  lastFinishedAt: string | null;
  lastStartedAt: string | null;
  modelCode: string | null;
  name: string;
  previousStatus: AutomationStatus | null;
  prompt: string;
  schedulerExpression: string;
  status: AutomationStatus;
  timezone: string;
  updatedAt: string;
  workspaceCode: string;
}

const automationOptions: AutomationOptions = {
  defaultStatus: "Paused",
  defaultConcurrencyMode: "Serial",
  concurrencyModes: ["Serial", "Parallel"],
  executionStatuses: ["Pending", "Running", "Success", "Failed", "Canceled", "Skipped"],
  statuses: ["Active", "Paused", "Archived"],
  timezones: ["UTC"],
};

let nextAutomationId = 4;
let nextExecutionId = 4;

let automations: AutomationSeed[] = [
  {
    agentCode: "main-agent",
    archivedAt: null,
    automationCode: "AUT-734600000000000001",
    concurrencyMode: "Serial",
    createdAt: "2026-05-19T06:00:00Z",
    cronExpression: "0 9 * * *",
    description: "Summarize open delivery tasks and the most recent operator-visible risks.",
    id: "734600000000000001",
    lastExecutionCode: "AEX-734600000000000001",
    lastExecutionStatus: "Success",
    lastExecutionSummary: "Prompt dispatch recorded for the automation.",
    lastFinishedAt: "2026-05-19T09:00:08Z",
    lastStartedAt: "2026-05-19T09:00:02Z",
    modelCode: "MODEL-734600000000000007",
    name: "Daily workspace release digest",
    previousStatus: null,
    prompt: "Write a detailed workspace release digest with risks, decisions, and next actions.",
    schedulerExpression: "0 0 9 * * *",
    status: "Active",
    timezone: "UTC",
    updatedAt: "2026-05-19T09:00:08Z",
    workspaceCode: "TRON_CORE",
  },
  {
    agentCode: "main-agent",
    archivedAt: null,
    automationCode: "AUT-734600000000000002",
    concurrencyMode: "Serial",
    createdAt: "2026-05-19T07:00:00Z",
    cronExpression: "*/30 * * * *",
    description: "Check whether queued workspaces need prompt regeneration before operator review.",
    id: "734600000000000002",
    lastExecutionCode: "AEX-734600000000000002",
    lastExecutionStatus: "Skipped",
    lastExecutionSummary: "Scheduler tick skipped because the automation is not Active.",
    lastFinishedAt: "2026-05-19T08:30:04Z",
    lastStartedAt: "2026-05-19T08:30:03Z",
    modelCode: "MODEL-734600000000000007",
    name: "Prompt queue readiness check",
    previousStatus: null,
    prompt: "Inspect the workspace backlog and list queues that are not ready for prompt generation.",
    schedulerExpression: "0 */30 * * * *",
    status: "Paused",
    timezone: "UTC",
    updatedAt: "2026-05-19T08:30:04Z",
    workspaceCode: "TRON_CORE",
  },
  {
    agentCode: "main-agent",
    archivedAt: "2026-05-19T05:15:00Z",
    automationCode: "AUT-734600000000000003",
    concurrencyMode: "Serial",
    createdAt: "2026-05-18T22:00:00Z",
    cronExpression: "0 */6 * * *",
    description: "Legacy audit fan-out kept only for historical inspection.",
    id: "734600000000000003",
    lastExecutionCode: "AEX-734600000000000003",
    lastExecutionStatus: "Failed",
    lastExecutionSummary: "Execution recorded with status Failed.",
    lastFinishedAt: "2026-05-19T05:00:12Z",
    lastStartedAt: "2026-05-19T05:00:05Z",
    modelCode: "MODEL-734600000000000007",
    name: "Archived governance audit",
    previousStatus: "Paused",
    prompt: "Run the legacy governance audit against archived compliance artifacts.",
    schedulerExpression: "0 0 */6 * * *",
    status: "Archived",
    timezone: "UTC",
    updatedAt: "2026-05-19T05:15:00Z",
    workspaceCode: "TRON_CORE",
  },
  {
    agentCode: "main-agent",
    archivedAt: null,
    automationCode: "AUT-734600000000000004",
    concurrencyMode: "Serial",
    createdAt: "2026-05-19T07:15:00Z",
    cronExpression: "0 12 * * *",
    description: "Send the platform team a noon summary for their own workspace.",
    id: "734600000000000004",
    lastExecutionCode: null,
    lastExecutionStatus: null,
    lastExecutionSummary: null,
    lastFinishedAt: null,
    lastStartedAt: null,
    modelCode: "MODEL-734600000000000001",
    name: "Platform noon brief",
    previousStatus: null,
    prompt: "Summarize platform delivery health and blockers for the noon handoff.",
    schedulerExpression: "0 0 12 * * *",
    status: "Active",
    timezone: "UTC",
    updatedAt: "2026-05-19T07:15:00Z",
    workspaceCode: "PLATFORM",
  },
];

let executionRecords: AutomationExecutionListItem[] = [
  {
    automationCode: "AUT-734600000000000001",
    createdAt: "2026-05-19T09:00:08Z",
    executionCode: "AEX-734600000000000001",
    finishedAt: "2026-05-19T09:00:08Z",
    scheduledAt: "2026-05-19T09:00:00Z",
    sessionCode: "SESSION-734600000000000001",
    startedAt: "2026-05-19T09:00:02Z",
    status: "Success",
    summary: "Prompt dispatch recorded for the automation.",
    triggerSource: "Scheduler",
    workspaceCode: "TRON_CORE",
  },
  {
    automationCode: "AUT-734600000000000002",
    createdAt: "2026-05-19T08:30:04Z",
    executionCode: "AEX-734600000000000002",
    finishedAt: "2026-05-19T08:30:04Z",
    scheduledAt: "2026-05-19T08:30:00Z",
    sessionCode: null,
    startedAt: "2026-05-19T08:30:03Z",
    status: "Skipped",
    summary: "Scheduler tick skipped because the automation is not Active.",
    triggerSource: "Scheduler",
    workspaceCode: "TRON_CORE",
  },
  {
    automationCode: "AUT-734600000000000003",
    createdAt: "2026-05-19T05:00:12Z",
    executionCode: "AEX-734600000000000003",
    finishedAt: "2026-05-19T05:00:12Z",
    scheduledAt: "2026-05-19T05:00:00Z",
    sessionCode: null,
    startedAt: "2026-05-19T05:00:05Z",
    status: "Failed",
    summary: "Execution recorded with status Failed.",
    triggerSource: "Scheduler",
    workspaceCode: "TRON_CORE",
  },
];

const TERMINAL_EXECUTION_STATUSES = new Set<AutomationExecutionStatus>([
  "Canceled",
  "Failed",
  "Skipped",
  "Success",
]);

export function getAutomationOptions() {
  return automationOptions;
}

export function listAutomations(
  query: AutomationListQuery,
): AutomationPage<AutomationListItem> {
  const workspaceCode = query.workspaceCode.trim();
  if (!workspaceCode) return emptyPage();

  const includeArchived = query.includeArchived === true;
  const search = query.search?.trim().toLowerCase() ?? "";
  const current = Math.max(1, query.current ?? 1);
  const size = Math.min(100, Math.max(1, query.size ?? EMPTY_PAGE_SIZE));
  const records = automations
    .filter((automation) => automation.workspaceCode === workspaceCode)
    .filter((automation) => {
      if (query.status) return automation.status === query.status;
      return includeArchived || automation.status !== "Archived";
    })
    .filter((automation) => {
      if (!search) return true;
      return [
        automation.automationCode,
        automation.name,
        automation.description ?? "",
        automation.lastExecutionSummary ?? "",
      ]
        .join(" ")
        .toLowerCase()
        .includes(search);
    })
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    .map(toListItem);

  return paginate(records, current, size);
}

export function getAutomationDetail(automationCode = automations[0]?.automationCode ?? "") {
  const automation = automations.find((item) => item.automationCode === automationCode) ?? automations[0];
  return toDetail(automation);
}

export function createMockAutomation(payload: CreateAutomationPayload) {
  const workspaceCode = requiredText(payload.workspaceCode, "workspaceCode is required");
  const name = requiredText(payload.name, "automation name is required");
  const prompt = requiredText(payload.prompt, "automation prompt is required");
  const description = optionalText(payload.description);
  const cronExpression = requiredText(payload.cronExpression, "automation cron expression is invalid");
  const schedulerExpression = normalizedSchedulerExpression(cronExpression);
  const status = payload.status ?? automationOptions.defaultStatus;
  const concurrencyMode = payload.concurrencyMode ?? automationOptions.defaultConcurrencyMode;
  const agentCode = optionalText(payload.agentCode) ?? "main-agent";
  const modelCode = optionalText(payload.modelCode);

  if (status === "Archived") {
    throw new Error("automation status is invalid");
  }

  const id = String(734600000000000000 + nextAutomationId++);
  const automationCode = `AUT-${id}`;
  const timestamp = timestampNow();
  const record: AutomationSeed = {
    agentCode,
    archivedAt: null,
    automationCode,
    concurrencyMode,
    createdAt: timestamp,
    cronExpression,
    description,
    id,
    lastExecutionCode: null,
    lastExecutionStatus: null,
    lastExecutionSummary: null,
    lastFinishedAt: null,
    lastStartedAt: null,
    modelCode,
    name,
    previousStatus: null,
    prompt,
    schedulerExpression,
    status,
    timezone: "UTC",
    updatedAt: timestamp,
    workspaceCode,
  };

  automations = [record, ...automations];
  return getAutomationDetail(automationCode);
}

export function updateMockAutomation(
  automationCode: string,
  payload: UpdateAutomationPayload,
) {
  const current = requireAutomation(automationCode);
  if (current.status === "Archived") {
    throw new Error("restore archived automation before modifying it");
  }

  const nextName = payload.name === undefined
    ? current.name
    : requiredText(payload.name, "automation name is required");
  const nextPrompt = payload.prompt === undefined
    ? current.prompt
    : requiredText(payload.prompt, "automation prompt is required");
  const nextDescription = payload.description === undefined
    ? current.description
    : optionalText(payload.description);
  const nextCronExpression = payload.cronExpression === undefined
    ? current.cronExpression
    : requiredText(payload.cronExpression, "automation cron expression is invalid");
  const nextSchedulerExpression = payload.cronExpression === undefined
    ? current.schedulerExpression
    : normalizedSchedulerExpression(nextCronExpression);
  const nextAgentCode = payload.agentCode === undefined
    ? current.agentCode
    : (optionalText(payload.agentCode) ?? "main-agent");
  const nextModelCode = payload.modelCode === undefined
    ? current.modelCode
    : optionalText(payload.modelCode);
  const nextConcurrencyMode = payload.concurrencyMode ?? current.concurrencyMode;

  automations = automations.map((automation) => automation.automationCode !== automationCode
    ? automation
    : {
      ...automation,
      agentCode: nextAgentCode,
      cronExpression: nextCronExpression,
      concurrencyMode: nextConcurrencyMode,
      description: nextDescription,
      modelCode: nextModelCode,
      name: nextName,
      prompt: nextPrompt,
      schedulerExpression: nextSchedulerExpression,
      updatedAt: timestampNow(),
    });

  return getAutomationDetail(automationCode);
}

export function updateMockAutomationStatus(
  automationCode: string,
  payload: UpdateAutomationStatusPayload,
) {
  const current = requireAutomation(automationCode);
  if (current.status === "Archived") {
    throw new Error("restore archived automation before modifying it");
  }

  const status = payload.status;

  automations = automations.map((automation) => automation.automationCode !== automationCode
    ? automation
    : {
      ...automation,
      status,
      updatedAt: timestampNow(),
    });

  return getAutomationDetail(automationCode);
}

export function archiveMockAutomation(automationCode: string): AutomationLifecycleResult {
  const current = requireAutomation(automationCode);
  if (current.status === "Archived") {
    return {
      archivedAt: current.archivedAt,
      automationCode: current.automationCode,
      previousStatus: current.previousStatus,
      status: current.status,
    };
  }

  const archivedAt = timestampNow();
  automations = automations.map((automation) => automation.automationCode !== automationCode
    ? automation
    : {
      ...automation,
      archivedAt,
      previousStatus: automation.status,
      status: "Archived",
      updatedAt: archivedAt,
    });

  const updated = requireAutomation(automationCode);
  return {
    archivedAt: updated.archivedAt,
    automationCode: updated.automationCode,
    previousStatus: updated.previousStatus,
    status: updated.status,
  };
}

export function restoreMockAutomation(automationCode: string): AutomationLifecycleResult {
  const current = requireAutomation(automationCode);
  if (current.status !== "Archived") {
    return {
      archivedAt: current.archivedAt,
      automationCode: current.automationCode,
      previousStatus: current.previousStatus,
      status: current.status,
    };
  }

  const restoredStatus = (current.previousStatus ?? automationOptions.defaultStatus) as AutomationOperationalStatus;
  automations = automations.map((automation) => automation.automationCode !== automationCode
    ? automation
    : {
      ...automation,
      archivedAt: null,
      previousStatus: null,
      status: restoredStatus,
      updatedAt: timestampNow(),
    });

  const updated = requireAutomation(automationCode);
  return {
    archivedAt: updated.archivedAt,
    automationCode: updated.automationCode,
    previousStatus: updated.previousStatus,
    status: updated.status,
  };
}

export function executeMockAutomation(automationCode: string) {
  const current = requireAutomation(automationCode);
  if (current.status === "Archived") {
    throw new Error("restore archived automation before modifying it");
  }

  const timestamp = timestampNow();
  const executionCode = `AEX-${734600000000000000 + nextExecutionId++}`;
  const summary = `Automation agent final reply for ${current.name}.`;
  const record: AutomationExecutionListItem = {
    automationCode,
    createdAt: timestamp,
    executionCode,
    finishedAt: timestamp,
    scheduledAt: null,
    sessionCode: `SESSION-${executionCode.slice(4)}`,
    startedAt: timestamp,
    status: "Success",
    summary,
    triggerSource: "Manual",
    workspaceCode: current.workspaceCode,
  };

  executionRecords = [record, ...executionRecords];
  automations = automations.map((automation) => automation.automationCode !== automationCode
    ? automation
    : {
      ...automation,
      lastExecutionCode: executionCode,
      lastExecutionStatus: "Success",
      lastExecutionSummary: summary,
      lastFinishedAt: timestamp,
      lastStartedAt: timestamp,
      updatedAt: timestamp,
    });

  return getAutomationDetail(automationCode);
}

export function listAutomationExecutions(
  automationCode: string,
  query: AutomationExecutionListQuery,
): AutomationPage<AutomationExecutionListItem> {
  const current = Math.max(1, query.current ?? 1);
  const size = Math.min(100, Math.max(1, query.size ?? EMPTY_PAGE_SIZE));
  const records = executionRecords
    .filter((record) => record.automationCode === automationCode)
    .sort((left, right) => right.startedAt.localeCompare(left.startedAt));

  return paginate(records, current, size);
}

export function deleteMockAutomationExecution(
  automationCode: string,
  executionCode: string,
) {
  const deleted = executionRecords.find((record) => (
    record.automationCode === automationCode && record.executionCode === executionCode
  ));
  if (!deleted) {
    throw new Error("automation execution was not found");
  }

  executionRecords = executionRecords.filter((record) => (
    record.automationCode !== automationCode || record.executionCode !== executionCode
  ));
  const latest = executionRecords
    .filter((record) => record.automationCode === automationCode)
    .sort((left, right) => right.startedAt.localeCompare(left.startedAt))[0] ?? null;
  automations = automations.map((automation) => automation.automationCode !== automationCode
    ? automation
    : {
      ...automation,
      lastExecutionCode: latest?.executionCode ?? null,
      lastExecutionStatus: latest?.status ?? null,
      lastExecutionSummary: latest?.summary ?? null,
      lastFinishedAt: latest?.finishedAt ?? null,
      lastStartedAt: latest?.startedAt ?? null,
      updatedAt: timestampNow(),
    });

  return deleted;
}

function emptyPage<T>(): AutomationPage<T> {
  return {
    current: 1,
    records: [],
    size: EMPTY_PAGE_SIZE,
    total: 0,
  };
}

function paginate<T>(
  records: T[],
  current: number,
  size: number,
): AutomationPage<T> {
  const start = (current - 1) * size;
  return {
    current,
    records: records.slice(start, start + size),
    size,
    total: records.length,
  };
}

function requireAutomation(automationCode: string) {
  const automation = automations.find((item) => item.automationCode === automationCode);
  if (!automation) {
    throw new Error("automation was not found");
  }
  return automation;
}

function toListItem(automation: AutomationSeed): AutomationListItem {
  const terminalExecutions = executionRecords.filter((execution) => (
    execution.automationCode === automation.automationCode
      && TERMINAL_EXECUTION_STATUSES.has(execution.status)
  ));
  const successCount = terminalExecutions.filter((execution) => execution.status === "Success").length;
  const totalCount = terminalExecutions.length;

  return {
    agentCode: automation.agentCode,
    archivedAt: automation.archivedAt,
    automationCode: automation.automationCode,
    cronExpression: automation.cronExpression,
    concurrencyMode: automation.concurrencyMode,
    description: automation.description,
    executionSuccessCount: successCount,
    executionSuccessRate: totalCount ? successCount / totalCount : null,
    executionTotalCount: totalCount,
    lastExecutionCode: automation.lastExecutionCode,
    lastExecutionStatus: automation.lastExecutionStatus,
    lastExecutionSummary: automation.lastExecutionSummary,
    lastFinishedAt: automation.lastFinishedAt,
    lastStartedAt: automation.lastStartedAt,
    modelCode: automation.modelCode,
    name: automation.name,
    nextRunAt: nextRunAt(automation.schedulerExpression, automation.status),
    status: automation.status,
    timezone: automation.timezone,
    updatedAt: automation.updatedAt,
    workspaceCode: automation.workspaceCode,
  };
}

function toDetail(automation?: AutomationSeed): AutomationDetailData {
  if (!automation) {
    return {
      agentCode: "main-agent",
      archivedAt: null,
      automationCode: "",
      createdAt: NOW,
      cronExpression: "",
      concurrencyMode: "Serial",
      description: null,
      id: "",
      lastExecutionCode: null,
      lastExecutionStatus: null,
      lastExecutionSummary: null,
      lastFinishedAt: null,
      lastStartedAt: null,
      modelCode: null,
      name: "",
      nextRunAt: null,
      previousStatus: null,
      prompt: "",
      schedulerExpression: "",
      status: "Paused",
      timezone: "UTC",
      updatedAt: NOW,
      workspaceCode: "",
    };
  }

  return {
    ...automation,
    nextRunAt: nextRunAt(automation.schedulerExpression, automation.status),
  };
}

function requiredText(value: string | undefined | null, message: string) {
  const normalized = value?.trim();
  if (!normalized) {
    throw new Error(message);
  }
  return normalized;
}

function optionalText(value: string | null | undefined) {
  const normalized = value?.trim();
  return normalized ? normalized : null;
}

function normalizedSchedulerExpression(cronExpression: string) {
  const parts = cronExpression.split(/\s+/).filter(Boolean);
  if (parts.length !== 5 && parts.length !== 6) {
    throw new Error("automation cron expression is invalid");
  }

  const valid = parts.every((part) => /^[\d*/,\-]+$/.test(part));
  if (!valid) {
    throw new Error("automation cron expression is invalid");
  }

  return parts.length === 5 ? `0 ${parts.join(" ")}` : parts.join(" ");
}

function nextRunAt(
  schedulerExpression: string,
  status: AutomationStatus,
) {
  if (status !== "Active") return null;

  const parts = schedulerExpression.split(/\s+/).filter(Boolean);
  if (parts.length !== 6) return timestampNow();
  const [, minute, hour] = parts;
  const base = new Date(NOW);

  if (minute.startsWith("*/")) {
    const interval = Number(minute.slice(2));
    if (Number.isFinite(interval) && interval > 0) {
      base.setUTCMinutes(base.getUTCMinutes() + interval, 0, 0);
      return base.toISOString();
    }
  }

  if (hour.startsWith("*/")) {
    const interval = Number(hour.slice(2));
    if (Number.isFinite(interval) && interval > 0) {
      base.setUTCHours(base.getUTCHours() + interval, Number(minute) || 0, 0, 0);
      return base.toISOString();
    }
  }

  if (hour === "*" && /^\d+$/.test(minute)) {
    base.setUTCHours(base.getUTCHours() + 1, Number(minute), 0, 0);
    return base.toISOString();
  }

  if (/^\d+$/.test(hour) && /^\d+$/.test(minute)) {
    base.setUTCDate(base.getUTCDate() + 1);
    base.setUTCHours(Number(hour), Number(minute), 0, 0);
    return base.toISOString();
  }

  base.setUTCMinutes(base.getUTCMinutes() + 60, 0, 0);
  return base.toISOString();
}

function timestampNow() {
  return new Date(NOW).toISOString();
}
