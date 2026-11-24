import { ArrowUpRight, RefreshCw } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router";

import { TablePagination } from "@/components/table-pagination";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useAutomationExecutions } from "@/domain/automation/hooks";
import { getAutomationExecutionStatusTone } from "@/domain/automation/constants";
import { useLocaleText } from "@/shared/hooks/useLocaleText";
import { useLocaleFormatters } from "@/shared/hooks/useLocaleFormatters";

interface AutomationExecutionHistoryPanelProps {
  automationCode: string;
  refreshToken?: number;
}

function chatSessionDetailHref(sessionCode: string) {
  const params = new URLSearchParams({ sessionCode, sourceTab: "automation" });
  return `/chats?${params.toString()}`;
}

export function AutomationExecutionHistoryPanel({
  automationCode,
  refreshToken = 0,
}: AutomationExecutionHistoryPanelProps) {
  const navigate = useNavigate();
  const pt = useLocaleText();
  const { formatDateTime } = useLocaleFormatters();
  const [current, setCurrent] = useState(1);
  const [size, setSize] = useState(10);
  const query = useMemo(() => ({ current, size }), [current, size]);
  const { loading, page, refresh } = useAutomationExecutions(automationCode, query);
  const records = Array.isArray(page.records) ? page.records : [];
  const pageCount = Math.max(1, Math.ceil(page.total / page.size));
  const startItem = page.total ? (page.current - 1) * page.size + 1 : 0;
  const endItem = page.total ? startItem + records.length - 1 : 0;

  useEffect(() => {
    setCurrent(1);
  }, [automationCode]);

  useEffect(() => {
    if (refreshToken > 0) refresh();
  }, [refresh, refreshToken]);

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4">
      <div className="flex justify-end">
        <Button
          aria-label={pt("Refresh")}
          className="shrink-0"
          disabled={loading}
          size="icon-sm"
          type="button"
          variant="ghost"
          onClick={refresh}
        >
          <RefreshCw className={loading ? "animate-spin" : undefined} aria-hidden="true" />
        </Button>
      </div>

      <div className="min-h-0 flex-1 space-y-3 overflow-y-auto pr-1">
        {loading && records.length === 0 ? (
          <p className="py-6 text-center text-sm text-[var(--color-on-surface-variant)]">
            {pt("Loading execution history...")}
          </p>
        ) : records.length ? records.map((record) => (
          <article
            className="space-y-3 rounded-xl border border-[var(--color-outline-variant)] bg-[var(--color-surface-container-low)] p-3"
            key={record.executionCode}
          >
            <div className="flex min-w-0 items-start justify-between gap-2">
              <span className="min-w-0 break-all font-mono text-xs font-semibold text-[var(--color-on-surface)]">
                {record.executionCode}
              </span>
              <Badge size="sm" variant={getAutomationExecutionStatusTone(record.status)}>
                {pt(record.status)}
              </Badge>
            </div>
            <div className="space-y-1 text-xs text-[var(--color-on-surface-variant)]">
              <p>{pt("Started")}: {formatDateTime(record.startedAt, { dateStyle: "medium", timeStyle: "short" })}</p>
              <p>{pt(record.triggerSource)}</p>
              {record.finishedAt ? (
                <p>{pt("Finished")}: {formatDateTime(record.finishedAt, { dateStyle: "medium", timeStyle: "short" })}</p>
              ) : null}
            </div>
            {record.summary ? (
              <p className="line-clamp-4 whitespace-pre-wrap break-words text-xs leading-5 text-[var(--color-on-surface-variant)]">
                {record.summary}
              </p>
            ) : null}
            {record.sessionCode ? (
              <Button
                className="w-full justify-between"
                size="sm"
                type="button"
                variant="secondary"
                onClick={() => navigate(chatSessionDetailHref(record.sessionCode!))}
              >
                {pt("Open session")}
                <ArrowUpRight aria-hidden="true" />
              </Button>
            ) : (
              <p className="text-xs text-[var(--color-on-surface-variant)]">
                {pt("No linked session")}
              </p>
            )}
          </article>
        )) : (
          <p className="rounded-xl border border-dashed border-[var(--color-outline-variant)] px-4 py-8 text-center text-sm text-[var(--color-on-surface-variant)]">
            {pt("No execution history has been recorded yet.")}
          </p>
        )}
      </div>

      {page.total > 0 ? (
        <TablePagination
          endItem={endItem}
          itemLabel={pt("Executions")}
          onPageChange={setCurrent}
          onPageSizeChange={(nextSize) => {
            setSize(nextSize);
            setCurrent(1);
          }}
          page={current}
          pageCount={pageCount}
          pageSize={size}
          pageSizeOptions={[5, 10, 15]}
          startItem={startItem}
          totalItems={page.total}
        />
      ) : null}
    </div>
  );
}
