import {
  useCallback,
  useEffect,
  useState,
} from "react";

import { fetchTaskExecutionPage } from "@/domain/task/api";
import { listMockTaskExecutionPage } from "@/domain/task/mock/data";
import type { TaskExecutionRecord, TaskPage } from "@/domain/task/model";
import { getServiceDataSource } from "@/shared/api/serviceConfig";

const PAGE_SIZE = 5;

function emptyPage(current: number, size: number): TaskPage<TaskExecutionRecord> {
  return { current, records: [], size, total: 0 };
}

export function useTaskExecutionHistory(taskCode: string) {
  const dataSource = getServiceDataSource("task");
  const [current, setCurrent] = useState(1);
  const [size, setSize] = useState(PAGE_SIZE);
  const [page, setPage] = useState(() => emptyPage(1, PAGE_SIZE));
  const [loading, setLoading] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);

  useEffect(() => {
    setCurrent(1);
  }, [taskCode]);

  useEffect(() => {
    if (!taskCode) {
      setPage(emptyPage(current, size));
      return;
    }
    if (dataSource !== "api") {
      setPage(listMockTaskExecutionPage(taskCode, current, size));
      return;
    }

    const controller = new AbortController();
    setLoading(true);
    fetchTaskExecutionPage(taskCode, { current, size }, controller.signal)
      .then(setPage)
      .catch((error: unknown) => {
        if (controller.signal.aborted) return;
        console.warn("[task] Task execution history request failed.", error);
        setPage(emptyPage(current, size));
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });

    return () => controller.abort();
  }, [current, dataSource, reloadKey, size, taskCode]);

  useEffect(() => {
    if (!page.records.some((record) => record.status === "Running")) return;
    const timeout = window.setTimeout(() => setReloadKey((value) => value + 1), 5_000);
    return () => window.clearTimeout(timeout);
  }, [page.records]);

  const refresh = useCallback(() => setReloadKey((value) => value + 1), []);

  return {
    current,
    loading,
    page,
    refresh,
    setCurrent,
    setSize,
    size,
  };
}
