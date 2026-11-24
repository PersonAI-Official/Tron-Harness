import { ArrowUpRight, RefreshCw } from "lucide-react";
import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";

import { TablePagination } from "@/components/table-pagination";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useTaskExecutionHistory } from "@/domain/task/hooks/useTaskExecutionHistory";
import type { TaskExecutionStatus } from "@/domain/task/model";
import { useLocaleFormatters } from "@/shared/hooks/useLocaleFormatters";

function taskExecutionStatusTone(status: TaskExecutionStatus) {
  if (status === "Success") return "success";
  if (status === "Failed") return "error";
  return "info";
}

export function TaskExecutionHistoryPanel({ taskCode }: { taskCode: string }) {
  const navigate = useNavigate();
  const { t } = useTranslation(["pages"]);
  const { formatDateTime } = useLocaleFormatters();
  const pageT = (key: string, defaultValue: string) => t(`tasks.detail.executionHistory.${key}`, { defaultValue });
  const {
    current,
    loading,
    page,
    refresh,
    setCurrent,
    setSize,
  } = useTaskExecutionHistory(taskCode);
  const pageCount = Math.max(1, Math.ceil(page.total / page.size));
  const startItem = page.total ? (page.current - 1) * page.size + 1 : 0;
  const endItem = page.total ? startItem + page.records.length - 1 : 0;
  const statusLabels: Record<TaskExecutionStatus, string> = {
    Failed: pageT("failed", "Failed"),
    Running: pageT("running", "Running"),
    Success: pageT("success", "Success"),
  };

  const openSession = (sessionCode: string) => {
    const params = new URLSearchParams({ sessionCode, sourceTab: "task" });
    navigate(`/chats?${params.toString()}`);
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4">
      <div className="flex items-center justify-between gap-2">
        <p className="text-xs text-[var(--color-on-surface-variant)]">
          {pageT("description", "Task execution history and linked sessions.")}
        </p>
        <Button
          aria-label={pageT("refresh", "Refresh")}
          className="shrink-0"
          disabled={loading}
          size="icon-sm"
          variant="ghost"
          onClick={refresh}
        >
          <RefreshCw className={loading ? "animate-spin" : undefined} aria-hidden="true" />
        </Button>
      </div>

      <div className="min-h-0 flex-1 space-y-3 overflow-y-auto pr-1">
        {loading && page.records.length === 0 ? (
          <p className="py-6 text-center text-sm text-[var(--color-on-surface-variant)]">
            {pageT("loading", "Loading execution history...")}
          </p>
        ) : page.records.length ? page.records.map((record) => (
          <article
            className="space-y-3 rounded-xl border border-[var(--color-outline-variant)] bg-[var(--color-surface-container-low)] p-3"
            key={record.executionCode}
          >
            <div className="flex min-w-0 items-start justify-between gap-2">
              <span className="min-w-0 break-all font-mono text-xs font-semibold text-[var(--color-on-surface)]">
                {record.executionCode}
              </span>
              <Badge size="sm" variant={taskExecutionStatusTone(record.status)}>
                {statusLabels[record.status]}
              </Badge>
            </div>
            <div className="text-xs text-[var(--color-on-surface-variant)]">
              {formatDateTime(record.startedAt, { dateStyle: "medium", timeStyle: "short" })}
            </div>
            {record.summary ? (
              <p className="line-clamp-3 whitespace-pre-wrap break-words text-xs leading-5 text-[var(--color-on-surface-variant)]">
                {record.summary}
              </p>
            ) : null}
            {record.sessionCode ? (
              <Button
                className="w-full justify-between"
                size="sm"
                variant="secondary"
                onClick={() => openSession(record.sessionCode!)}
              >
                {pageT("openSession", "View execution")}
                <ArrowUpRight aria-hidden="true" />
              </Button>
            ) : (
              <p className="rounded-md border border-dashed border-[var(--color-outline-variant)] px-3 py-2 text-xs text-[var(--color-on-surface-variant)]">
                {pageT("noSession", "No linked session")}
              </p>
            )}
          </article>
        )) : (
          <p className="rounded-xl border border-dashed border-[var(--color-outline-variant)] px-4 py-8 text-center text-sm text-[var(--color-on-surface-variant)]">
            {pageT("empty", "No execution records yet.")}
          </p>
        )}
      </div>

      {page.total > 0 ? (
        <TablePagination
          endItem={endItem}
          itemLabel={pageT("itemLabel", "runs")}
          onPageChange={setCurrent}
          onPageSizeChange={(nextSize) => {
            setSize(nextSize);
            setCurrent(1);
          }}
          page={current}
          pageCount={pageCount}
          pageSize={page.size}
          pageSizeOptions={[5, 10, 15]}
          startItem={startItem}
          totalItems={page.total}
        />
      ) : null}
    </div>
  );
}
