import { requestApi } from "@/shared/api/httpClient";

export function previewAutomationSchedule(cronExpression: string) {
  return requestApi<{ executionTimes: number[] }>("GET /api/v1/automations/schedule-preview", { queryParams: { cronExpression } });
}
import type {
  AutomationDetailData,
  AutomationExecutionListItem,
  AutomationExecutionListQuery,
  AutomationLifecycleResult,
  AutomationListItem,
  AutomationListQuery,
  AutomationOptions,
  AutomationPage,
  CreateAutomationPayload,
  UpdateAutomationPayload,
  UpdateAutomationStatusPayload,
} from "@/domain/automation/model";

export const automationApiEndpoints = {
  archiveAutomation: "POST /api/v1/automations/{automationCode}/archive",
  createAutomation: "POST /api/v1/automations",
  deleteAutomation: "DELETE /api/v1/automations/{automationCode}",
  deleteAutomationExecution: "DELETE /api/v1/automations/{automationCode}/executions/{executionCode}",
  executeAutomation: "POST /api/v1/automations/{automationCode}/execute",
  getAutomationDetail: "GET /api/v1/automations/{automationCode}",
  getAutomationOptions: "GET /api/v1/automations/options",
  listAutomationExecutions: "GET /api/v1/automations/{automationCode}/executions",
  listAutomations: "GET /api/v1/automations",
  restoreAutomation: "POST /api/v1/automations/{automationCode}/restore",
  updateAutomation: "PATCH /api/v1/automations/{automationCode}",
  updateAutomationStatus: "PATCH /api/v1/automations/{automationCode}/status",
} as const;

type AutomationQueryParams = Record<string, string | number | boolean | null | undefined>;

function automationQueryParams(
  query: AutomationListQuery | AutomationExecutionListQuery,
): AutomationQueryParams {
  return { ...query };
}

export function fetchAutomationOptions(signal?: AbortSignal) {
  return requestApi<AutomationOptions>(automationApiEndpoints.getAutomationOptions, { signal });
}

export function fetchAutomationPage(
  query: AutomationListQuery,
  signal?: AbortSignal,
) {
  return requestApi<AutomationPage<AutomationListItem>>(
    automationApiEndpoints.listAutomations,
    {
      queryParams: automationQueryParams(query),
      signal,
    },
  );
}

export function fetchAutomationDetail(
  automationCode: string,
  signal?: AbortSignal,
) {
  return requestApi<AutomationDetailData>(
    automationApiEndpoints.getAutomationDetail,
    {
      pathParams: { automationCode },
      signal,
    },
  );
}

export function createAutomation(payload: CreateAutomationPayload) {
  return requestApi<AutomationDetailData>(
    automationApiEndpoints.createAutomation,
    { body: payload },
  );
}

export function updateAutomation(
  automationCode: string,
  payload: UpdateAutomationPayload,
) {
  return requestApi<AutomationDetailData>(
    automationApiEndpoints.updateAutomation,
    {
      body: payload,
      pathParams: { automationCode },
    },
  );
}

export function deleteAutomation(automationCode: string) {
  return requestApi<{ automationCode: string }>(
    automationApiEndpoints.deleteAutomation,
    { pathParams: { automationCode } },
  );
}

export function updateAutomationStatus(
  automationCode: string,
  payload: UpdateAutomationStatusPayload,
) {
  return requestApi<AutomationDetailData>(
    automationApiEndpoints.updateAutomationStatus,
    {
      body: payload,
      pathParams: { automationCode },
    },
  );
}

export function archiveAutomation(automationCode: string) {
  return requestApi<AutomationLifecycleResult>(
    automationApiEndpoints.archiveAutomation,
    { pathParams: { automationCode } },
  );
}

export function restoreAutomation(automationCode: string) {
  return requestApi<AutomationLifecycleResult>(
    automationApiEndpoints.restoreAutomation,
    { pathParams: { automationCode } },
  );
}

export function executeAutomation(automationCode: string) {
  return requestApi<AutomationDetailData>(
    automationApiEndpoints.executeAutomation,
    { pathParams: { automationCode } },
  );
}

export function fetchAutomationExecutionPage(
  automationCode: string,
  query: AutomationExecutionListQuery,
  signal?: AbortSignal,
) {
  return requestApi<AutomationPage<AutomationExecutionListItem>>(
    automationApiEndpoints.listAutomationExecutions,
    {
      pathParams: { automationCode },
      queryParams: automationQueryParams(query),
      signal,
    },
  );
}

export function deleteAutomationExecution(
  automationCode: string,
  executionCode: string,
) {
  return requestApi<AutomationExecutionListItem>(
    automationApiEndpoints.deleteAutomationExecution,
    { pathParams: { automationCode, executionCode } },
  );
}
