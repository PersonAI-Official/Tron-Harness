import { MoreHorizontal } from "lucide-react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DataTable } from "@/components/data-table";
import { SelectInput } from "@/components/forms";
import { SearchField } from "@/components/search-field";
import { TablePagination } from "@/components/table-pagination";
import { PageHeader } from "@/shared/layout/PageHeader";
import { SurfaceCard } from "@/shared/layout/SurfaceCard";
import { useDismissibleLayer } from "@/shared/hooks/useDismissibleLayer";
import { useLocaleFormatters } from "@/shared/hooks/useLocaleFormatters";
import { useLocaleText } from "@/shared/hooks/useLocaleText";
import { useWorkspaceSpaces } from "@/shared/hooks/useWorkspaceSpaces";
import { useToast } from "@/components/ui/toast";
import { useTabPanelConfig } from "@/shared/layout/content/TabPanelRegistry";
import { DEFAULT_TAB_ID, useTabStore } from "@/store/tabStore";

import {
  archiveAutomation,
  executeAutomation,
  restoreAutomation,
  updateAutomationStatus,
} from "@/domain/automation/api";
import {
  getAutomationExecutionStatusTone,
  getAutomationStatusTone,
} from "@/domain/automation/constants";
import { useAutomationList } from "@/domain/automation/hooks";
import {
  archiveMockAutomation,
  executeMockAutomation,
  restoreMockAutomation,
  updateMockAutomationStatus,
} from "@/domain/automation/mock/data";
import type {
  AutomationListItem,
  AutomationOperationalStatus,
  AutomationStatus,
} from "@/domain/automation/model";
import { getServiceDataSource } from "@/shared/api/serviceConfig";
import { SelectItem } from "@/components/ui/select";

const EMPTY_RIGHT_PANEL_TABS: never[] = [];

function pageBounds(page: number, pageSize: number, total: number, visible: number) {
  if (!total || !visible) return { end: 0, start: 0 };
  const start = (page - 1) * pageSize + 1;
  return {
    end: start + visible - 1,
    start,
  };
}

function StatisticCard({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <SurfaceCard className="p-4">
      <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-[var(--color-on-surface-variant)]">
        {label}
      </p>
      <p className="mt-2 text-3xl font-semibold leading-none text-[var(--color-on-surface)]">
        {value}
      </p>
    </SurfaceCard>
  );
}

function automationDescriptionText(row: AutomationListItem, fallback: string) {
  return row.description?.trim() || fallback;
}

interface RowMenuState {
  automationCode: string;
  left: number;
  top: number;
  triggerElement: HTMLButtonElement;
}

export function AutomationsPage() {
  const navigate = useNavigate();
  const pt = useLocaleText();
  const activeTabId = useTabStore((state) => state.activeTabId ?? DEFAULT_TAB_ID);
  const updateRightPanelState = useTabStore((state) => state.updateRightPanelState);
  const { formatDateTime, formatNumber } = useLocaleFormatters();
  const toast = useToast();
  const dataSource = getServiceDataSource("automation");
  const { selectedPrimarySpace, selectedSecondarySpace } = useWorkspaceSpaces();
  const activeWorkspace = selectedSecondarySpace ?? selectedPrimarySpace;
  const activeWorkspaceCode = activeWorkspace?.workspaceCode ?? "";
  const [searchInput, setSearchInput] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<AutomationStatus | "">("");
  const [current, setCurrent] = useState(1);
  const [size, setSize] = useState(15);
  const [rowBusyKey, setRowBusyKey] = useState("");
  const [openRowMenu, setOpenRowMenu] = useState<RowMenuState | null>(null);
  const rowMenuRef = useRef<HTMLDivElement | null>(null);
  const query = useMemo(() => ({
    current,
    search: debouncedSearch,
    size,
    status: statusFilter || undefined,
    workspaceCode: activeWorkspaceCode,
  }), [activeWorkspaceCode, current, debouncedSearch, size, statusFilter]);
  const { page, refresh: refreshList } = useAutomationList(query);
  useTabPanelConfig({ rightPanel: null, rightPanelTabs: EMPTY_RIGHT_PANEL_TABS });

  useEffect(() => {
    updateRightPanelState(activeTabId, { open: false });
  }, [activeTabId, updateRightPanelState]);

  const { page: workspacePage, refresh: refreshTotal } = useAutomationList({
    workspaceCode: activeWorkspaceCode,
    current: 1,
    size: 1,
  });
  const { page: activePage, refresh: refreshActive } = useAutomationList({
    workspaceCode: activeWorkspaceCode,
    current: 1,
    size: 1,
    status: "Active",
  });
  const { page: pausedPage, refresh: refreshPaused } = useAutomationList({
    workspaceCode: activeWorkspaceCode,
    current: 1,
    size: 1,
    status: "Paused",
  });
  const refresh = () => {
    refreshList();
    refreshTotal();
    refreshActive();
    refreshPaused();
  };
  const totalPages = Math.max(1, Math.ceil(page.total / page.size));
  const bounds = pageBounds(page.current, page.size, page.total, page.records.length);
  const formatTimestamp = (value?: string | null, fallback = "Not available") => (value ? formatDateTime(value, {
    dateStyle: "medium",
    timeStyle: "short",
  }) : pt(fallback));
  const formatSuccessRate = (row: AutomationListItem) => {
    if (!row.executionTotalCount || row.executionSuccessRate === null) return pt("No runs");
    return `${formatNumber(Math.round(row.executionSuccessRate * 100))}%`;
  };
  const formatSuccessRateTitle = (row: AutomationListItem) => {
    if (!row.executionTotalCount) return pt("No completed executions");
    return pt("{{success}}/{{total}} successful executions", {
      success: formatNumber(row.executionSuccessCount),
      total: formatNumber(row.executionTotalCount),
    });
  };
  const openMenuRow = openRowMenu
    ? page.records.find((item) => item.automationCode === openRowMenu.automationCode) ?? null
    : null;

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDebouncedSearch(searchInput.trim());
      setCurrent(1);
    }, 220);

    return () => window.clearTimeout(timer);
  }, [searchInput]);

  useEffect(() => {
    setCurrent(1);
  }, [activeWorkspaceCode, size, statusFilter]);

  useEffect(() => {
    setOpenRowMenu(null);
  }, [current, debouncedSearch, size, statusFilter, activeWorkspaceCode]);

  useDismissibleLayer({
    onDismiss: () => setOpenRowMenu(null),
    open: openRowMenu !== null,
    popupRef: rowMenuRef,
    triggerElement: openRowMenu?.triggerElement,
  });

  function toggleRowMenu(row: AutomationListItem, trigger: HTMLButtonElement) {
    if (rowBusyKey.includes(`:${row.automationCode}`)) return;
    if (openRowMenu?.automationCode === row.automationCode) {
      setOpenRowMenu(null);
      return;
    }

    const menuWidth = 176;
    const estimatedMenuHeight = row.status === "Archived" ? 112 : 196;
    const rect = trigger.getBoundingClientRect();
    const left = Math.min(
      Math.max(12, rect.right - menuWidth),
      window.innerWidth - menuWidth - 12,
    );
    const top = Math.min(
      Math.max(12, rect.bottom + 8),
      window.innerHeight - estimatedMenuHeight - 12,
    );

    setOpenRowMenu({
      automationCode: row.automationCode,
      left,
      top,
      triggerElement: trigger,
    });
  }

  async function handleExecute(row: AutomationListItem) {
    const busyKey = `execute:${row.automationCode}`;
    if (rowBusyKey === busyKey || row.status === "Archived") return;
    setRowBusyKey(busyKey);
    setOpenRowMenu(null);

    try {
      const updated = dataSource === "api"
        ? await executeAutomation(row.automationCode)
        : executeMockAutomation(row.automationCode);
      toast.success(pt("Manual execution recorded for {{code}}", { code: updated.automationCode }));
      refresh();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : pt("Automation execution could not be recorded"));
    } finally {
      setRowBusyKey("");
    }
  }

  async function handleStatusToggle(row: AutomationListItem) {
    const busyKey = `status:${row.automationCode}`;
    if (rowBusyKey === busyKey || row.status === "Archived") return;
    setRowBusyKey(busyKey);
    setOpenRowMenu(null);

    try {
      const status: AutomationOperationalStatus = row.status === "Active" ? "Paused" : "Active";
      const updated = dataSource === "api"
        ? await updateAutomationStatus(row.automationCode, { status })
        : updateMockAutomationStatus(row.automationCode, { status });
      toast.success(pt("Automation {{code}} {{status}}", { code: updated.automationCode, status: status === "Active" ? pt("activated") : pt("paused") }));
      refresh();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : pt("Automation status could not be updated"));
    } finally {
      setRowBusyKey("");
    }
  }

  async function handleArchive(row: AutomationListItem) {
    const busyKey = `archive:${row.automationCode}`;
    if (rowBusyKey === busyKey) return;
    setRowBusyKey(busyKey);
    setOpenRowMenu(null);

    try {
      if (row.status === "Archived") {
        const result = dataSource === "api"
          ? await restoreAutomation(row.automationCode)
          : restoreMockAutomation(row.automationCode);
        toast.success(pt("Automation {{code}} restored to {{status}}", { code: result.automationCode, status: pt(result.status) }));
      } else {
        const result = dataSource === "api"
          ? await archiveAutomation(row.automationCode)
          : archiveMockAutomation(row.automationCode);
        toast.success(pt("Automation {{code}} archived", { code: result.automationCode }));
      }
      refresh();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : pt("Automation lifecycle update failed"));
    } finally {
      setRowBusyKey("");
    }
  }

  return (
    <>
      <PageHeader
        title={pt("Automations")}
        actions={
          <>
            <Button size="lg" variant="outline" onClick={() => {
              refresh();
            }}
            >
              {pt("Refresh")}
            </Button>
            <Button size="lg" disabled={!activeWorkspaceCode} onClick={() => navigate("/automations/new")}>
              {pt("New")}
            </Button>
          </>
        }
      />
      <section className="mb-4 grid gap-3 md:grid-cols-3">
        <StatisticCard label={pt("Total")} value={formatNumber(workspacePage.total)} />
        <StatisticCard label={pt("Active")} value={formatNumber(activePage.total)} />
        <StatisticCard label={pt("Paused")} value={formatNumber(pausedPage.total)} />
      </section>
      <DataTable<AutomationListItem>
        emptyState={activeWorkspaceCode ? pt("No automations yet.") : pt("Choose a workspace in the shell header before managing automations.")}
        onRowClick={(row) => navigate(`/automations/detail?automationCode=${encodeURIComponent(row.automationCode)}`)}
        rows={page.records}
        rowKey={(row) => row.automationCode}
        tableClassName="min-w-[1080px] table-fixed [&_td]:px-3 [&_th]:px-3"
        toolbar={
          <>
            <SearchField
              ariaLabel={pt("Search automations")}
              className="ml-auto w-full min-w-0 sm:w-80"
              placeholder={pt("Filter by code, name, description, or latest result...")}
              value={searchInput}
              onChange={setSearchInput}
            />
            <SelectInput
              className="h-9 w-full sm:w-44"
              aria-label={pt("Automation status filter")}
              value={statusFilter}
              onValueChange={(event) => setStatusFilter(event as AutomationStatus | "")}
            >
              <SelectItem value="">{pt("All active states")}</SelectItem>
              <SelectItem value="Active">{pt("Active")}</SelectItem>
              <SelectItem value="Paused">{pt("Paused")}</SelectItem>
              <SelectItem value="Archived">{pt("Archived")}</SelectItem>
            </SelectInput>
          </>
        }
        pagination={(
          <TablePagination
            endItem={bounds.end}
            itemLabel={pt("automations")}
            page={page.current}
            pageCount={totalPages}
            pageSize={page.size}
            startItem={bounds.start}
            totalItems={page.total}
            onPageChange={(nextPage) => setCurrent(Math.min(totalPages, Math.max(1, nextPage)))}
            onPageSizeChange={(nextSize) => {
              setCurrent(1);
              setSize(nextSize);
            }}
          />
        )}
        columns={[
          {
            header: pt("Name"),
            id: "name",
            width: "15%",
            render: (row) => (
              <p className="min-w-0 truncate font-medium text-[var(--color-on-surface)]" title={row.name}>{row.name}</p>
            ),
          },
          {
            cellClassName: "min-w-0",
            header: pt("Description"),
            id: "description",
            width: "17%",
            render: (row) => {
              const description = automationDescriptionText(row, pt("No description"));

              return (
                <p className="min-w-0 truncate text-sm leading-6 text-[var(--color-on-surface-variant)]" title={description}>
                  {description}
                </p>
              );
            },
          },
          {
            cellClassName: "min-w-0",
            header: pt("Schedule"),
            id: "schedule",
            width: "17%",
            render: (row) => (
              <div className="min-w-0 space-y-1">
                <p className="truncate font-mono text-[13px] text-[var(--color-on-surface)]" title={row.cronExpression}>{row.cronExpression}</p>
                <p className="truncate text-sm text-[var(--color-on-surface-variant)]" title={pt("Next: {{value}}", { value: formatTimestamp(row.nextRunAt, "Not scheduled") })}>{pt("Next: {{value}}", { value: formatTimestamp(row.nextRunAt, "Not scheduled") })}</p>
              </div>
            ),
          },
          {
            cellClassName: "min-w-0",
            header: pt("Success rate"),
            id: "success-rate",
            width: "11%",
            render: (row) => (
              <div className="min-w-0" title={formatSuccessRateTitle(row)}>
                <p className="truncate text-sm font-medium text-[var(--color-on-surface)]">
                  {formatSuccessRate(row)}
                </p>
                {row.executionTotalCount ? (
                  <p className="truncate text-xs text-[var(--color-on-surface-variant)]">
                    {formatNumber(row.executionSuccessCount)} / {formatNumber(row.executionTotalCount)}
                  </p>
                ) : null}
              </div>
            ),
          },
          {
            header: pt("Latest result"),
            id: "latest",
            width: "12%",
            render: (row) => (
              <div>
                {row.lastExecutionStatus ? (
                  <Badge variant={getAutomationExecutionStatusTone(row.lastExecutionStatus)}>
                    {pt(row.lastExecutionStatus)}
                  </Badge>
                ) : (
                  <Badge variant="info">{pt("Never run")}</Badge>
                )}
              </div>
            ),
          },
          {
            header: pt("Status"),
            id: "status",
            width: "9%",
            render: (row) => (
              <Badge variant={getAutomationStatusTone(row.status)}>{pt(row.status)}</Badge>
            ),
          },
          {
            header: pt("Updated"),
            id: "updated",
            width: "11%",
            render: (row) => (
              <span className="text-sm text-[var(--color-on-surface-variant)]">{formatTimestamp(row.updatedAt)}</span>
            ),
          },
          {
            align: "right",
            header: pt("Actions"),
            id: "actions",
            width: "8%",
            render: (row) => {
              const isMenuOpen = openRowMenu?.automationCode === row.automationCode;
              const isBusy = rowBusyKey.includes(`:${row.automationCode}`);

              return (
                <div className="flex justify-end">
                  <Button
                    size="icon"
                    variant="ghost"
                    aria-label={pt("Open actions for {{name}}", { name: row.name })}
                    aria-expanded={isMenuOpen}
                    data-automation-menu-trigger={row.automationCode}
                    disabled={isBusy}
                    onClick={(event) => {
                      event.stopPropagation();
                      toggleRowMenu(row, event.currentTarget);
                    }}
                  >
                    <MoreHorizontal size={16} aria-hidden="true" />
                  </Button>
                </div>
              );
            },
          },
        ]}
      />
      {openRowMenu && openMenuRow
        ? createPortal(
          <div
            ref={rowMenuRef}
            style={{ left: openRowMenu.left, top: openRowMenu.top }}
            className="fixed z-[80] w-44 rounded-xl border border-[var(--color-outline-variant)] bg-[var(--color-elevated-surface)] p-2 text-left text-sm shadow-[0_12px_28px_rgba(0,0,0,0.22)]"
            role="menu"
            aria-label={pt("Actions for {{name}}", { name: openMenuRow.name })}
          >
            {openMenuRow.status !== "Archived" ? (
              <button
                type="button"
                role="menuitem"
                className="block w-full rounded-lg px-3 py-2 text-left hover:bg-[var(--color-hover-surface)] disabled:cursor-not-allowed disabled:opacity-60"
                disabled={rowBusyKey === `execute:${openMenuRow.automationCode}`}
                onClick={() => {
                  void handleExecute(openMenuRow);
                }}
              >
                {pt("Run")}
              </button>
            ) : null}
            <button
              type="button"
              role="menuitem"
              className="block w-full rounded-lg px-3 py-2 text-left hover:bg-[var(--color-hover-surface)]"
              onClick={() => {
                setOpenRowMenu(null);
                navigate(`/automations/detail?automationCode=${encodeURIComponent(openMenuRow.automationCode)}`);
              }}
            >
              {pt("Execution instances")}
            </button>
            {openMenuRow.status !== "Archived" ? (
              <button
                type="button"
                role="menuitem"
                className="block w-full rounded-lg px-3 py-2 text-left hover:bg-[var(--color-hover-surface)] disabled:cursor-not-allowed disabled:opacity-60"
                disabled={rowBusyKey === `status:${openMenuRow.automationCode}`}
                onClick={() => {
                  void handleStatusToggle(openMenuRow);
                }}
              >
                {openMenuRow.status === "Active" ? pt("Pause") : pt("Activate")}
              </button>
            ) : null}
            {openMenuRow.status !== "Archived" ? (
              <button
                type="button"
                role="menuitem"
                className="block w-full rounded-lg px-3 py-2 text-left hover:bg-[var(--color-hover-surface)] disabled:cursor-not-allowed disabled:opacity-60"
                disabled={rowBusyKey === `archive:${openMenuRow.automationCode}`}
                onClick={() => {
                  void handleArchive(openMenuRow);
                }}
              >
                {pt("Archive")}
              </button>
            ) : null}
            {openMenuRow.status === "Archived" ? (
              <button
                type="button"
                role="menuitem"
                className="block w-full rounded-lg px-3 py-2 text-left hover:bg-[var(--color-hover-surface)] disabled:cursor-not-allowed disabled:opacity-60"
                disabled={rowBusyKey === `archive:${openMenuRow.automationCode}`}
                onClick={() => {
                  void handleArchive(openMenuRow);
                }}
              >
                {pt("Restore")}
              </button>
            ) : null}
          </div>,
          document.body,
        )
        : null}
    </>
  );
}
