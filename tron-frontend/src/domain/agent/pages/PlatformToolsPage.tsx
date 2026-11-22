import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";

import type { PlatformToolRecord, ToolCatalogDomainRecord } from "@/domain/agent/model";
import { DataTable } from "@/components/data-table";
import { SearchField } from "@/components/search-field";
import { TablePagination } from "@/components/table-pagination";
import {
  PageHeader,
  useTablePagination,
} from "@/shared";
import { usePlatformToolSettings } from "../hooks/usePlatformToolSettings";
import { Button } from "@/components/ui/button";
import { ToolPermissionControl } from "../components/ToolPermissionControl";

type CategoryDomain = ToolCatalogDomainRecord & { categoryCode: string; tools: PlatformToolRecord[] };

export function PlatformToolsPage() {
  const { t } = useTranslation(["pages", "common"]);
  const navigate = useNavigate();
  const { domains, tools, categories, catalogIssues, loadErrors, loading, saving, loaded, permissions, dirty, refreshTools, savePermissions, discard, setPermission } = usePlatformToolSettings();
  const [categoryFilter, setCategoryFilter] = useState("all");
  const [domainSearch, setDomainSearch] = useState("");
  const [debouncedDomainSearch, setDebouncedDomainSearch] = useState("");
  const [expandedDomainCodes, setExpandedDomainCodes] = useState<Set<string>>(() => new Set());

  const pageT = (key: string, defaultValue: string, options?: Record<string, unknown>) => (
    t(`pages:settingsTools.${key}`, { defaultValue, ...options })
  );

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDebouncedDomainSearch(domainSearch.trim().toLowerCase());
    }, 260);

    return () => window.clearTimeout(timer);
  }, [domainSearch]);

  const visibleDomains = useMemo(() => {
    const grouped = new Map<string, CategoryDomain>();
    const categoriesByDomain = new Map<string, Set<string>>();
    for (const tool of tools) {
      const codes = categoriesByDomain.get(tool.domainCode) ?? new Set<string>();
      codes.add(tool.category?.code ?? "uncategorized");
      categoriesByDomain.set(tool.domainCode, codes);
    }
    for (const tool of tools) {
      const categoryCode = tool.category?.code ?? "uncategorized";
      if (categoryFilter === "readonly" ? tool.operationKind !== "query" : categoryFilter !== "all" && categoryFilter !== categoryCode) continue;
      const categoryName = categories.find((item) => item.code === categoryCode)?.name ?? pageT("categories.uncategorized", "Uncategorized");
      const domain = domains.find((item) => item.domainCode === tool.domainCode);
      const description = (categoriesByDomain.get(tool.domainCode)?.size ?? 0) > 1 ? "" : domain?.description ?? "";
      const domainMatches = [categoryCode, categoryName, tool.domainCode, domain?.name, description].join(" ").toLowerCase().includes(debouncedDomainSearch);
      const toolMatches = [tool.toolCode, tool.toolName, tool.name, tool.description, ...Object.values(tool.source ?? {})].join(" ").toLowerCase().includes(debouncedDomainSearch);
      if (debouncedDomainSearch && !domainMatches && !toolMatches) continue;
      const key = `${categoryCode}:${tool.domainCode}`;
      let group = grouped.get(key);
      if (!group) {
        group = { domainCode: tool.domainCode, name: domain?.name ?? tool.domainCode, description, sampleTools: [], hasMutationTools: false, toolCount: 0, categoryCode, tools: [] };
        grouped.set(key, group);
      }
      group.tools.push(tool);
      group.toolCount++;
      group.hasMutationTools ||= tool.operationKind === "command";
    }
    return [...grouped.values()];
  }, [categories, categoryFilter, debouncedDomainSearch, domains, tools, t]);

  const domainPagination = useTablePagination(visibleDomains);
  function toggleDomain(domainCode: string) {
    setExpandedDomainCodes((current) => {
      const next = new Set(current);
      if (next.has(domainCode)) {
        next.delete(domainCode);
      } else {
        next.add(domainCode);
      }
      return next;
    });
  }

  return (
    <div className="space-y-6">
      <PageHeader title={pageT("title", "Platform Tools")} actions={<div className="flex flex-wrap items-center gap-2">
        <span role="status" className="text-xs text-[var(--color-on-surface-variant)]">{loading ? pageT("permissions.loading", "Loading tool permissions…") : dirty ? pageT("permissions.dirty", "Unsaved changes") : ""}</span>
        <Button variant="outline" onClick={() => navigate("/settings/tools/native-schemes")}>{pageT("nativeSchemes.open", "Native tool schemes")}</Button>
        <Button variant="outline" disabled={loading || saving || dirty} onClick={() => void refreshTools()}>{pageT("permissions.refresh", "Refresh")}</Button>
        <Button variant="outline" disabled={!dirty || saving || loading} onClick={discard}>{pageT("permissions.discard", "Discard")}</Button>
        <Button disabled={!dirty || !loaded || loading || saving} onClick={() => void savePermissions()}>{pageT(saving ? "permissions.saving" : "permissions.save", saving ? "Saving…" : "Save")}</Button>
      </div>} />

      {(catalogIssues.length > 0 || loadErrors.length > 0) && (
        <section role="status" className="rounded-md border border-[var(--color-warning)] p-3 text-sm">
          <p className="font-medium text-[var(--color-warning)]">{pageT("catalogIssues.title", "Some tools could not be loaded")}</p>
          <p className="mt-1 text-[var(--color-on-surface-variant)]">{pageT("catalogIssues.hint", "Other tools remain visible. Fix the affected tool or provider, then refresh.")}</p>
          <ul className="mt-2 max-h-56 space-y-2 overflow-y-auto [overflow-wrap:anywhere]">
            {loadErrors.map((message) => <li key={message}>{message}</li>)}
            {catalogIssues.map((issue, index) => (
              <li key={`${issue.packId}:${issue.providerId}:${issue.toolCode}:${index}`}>
                <span className="font-medium">{issue.toolName ?? issue.toolCode ?? issue.providerId ?? issue.packId ?? pageT("loadParts.diagnostics", "Tool discovery")}</span>
                {issue.packId && <span className="ml-2 text-xs text-[var(--color-on-surface-variant)]">{issue.packId}</span>}
                <p className="mt-0.5 whitespace-pre-wrap text-[var(--color-on-surface-variant)]">{issue.message}</p>
              </li>
            ))}
          </ul>
        </section>
      )}

      <div className="flex flex-wrap gap-2" role="group" aria-label={pageT("categories.label", "Tool categories")}>
        {[
          { code: "all", name: pageT("categories.all", "All tools"), count: tools.length },
          ...[...categories].sort((left, right) => {
            const order = ["tron.platform", "tron.assistant", "system.command", "document.media", "network", "agent.collaboration", "interaction", "execution"];
            const index = (code: string) => { const found = order.indexOf(code); return found < 0 ? order.length : found; };
            return index(left.code) - index(right.code) || left.name.localeCompare(right.name);
          }).map((category) => ({ ...category, count: category.toolCount })),
          { code: "readonly", name: pageT("categories.readonly", "Read-only tools"), count: tools.filter((tool) => tool.operationKind === "query").length },
        ].map((category) => <Button key={category.code} variant={categoryFilter === category.code ? "default" : "outline"} aria-pressed={categoryFilter === category.code} onClick={() => { setCategoryFilter(category.code); domainPagination.paginationProps.onPageChange(1); }}>{category.name} ({category.count})</Button>)}
      </div>

        <DataTable<CategoryDomain>
          rowKey={(row) => `${row.categoryCode}:${row.domainCode}`}
          rows={domainPagination.visibleItems}
          tableClassName="min-w-[640px] table-fixed [&_td]:px-3 [&_td]:py-2 [&_th]:px-3 [&_th]:py-2"
          expandedRowClassName="[--tron-table-row-bg:var(--color-elevated-surface)] [--tron-table-row-stripe-bg:var(--color-elevated-surface)] [--tron-table-row-hover-bg:var(--color-elevated-surface)] [&>td]:pt-0 [&>td]:pb-4"
          isRowExpanded={(row) => expandedDomainCodes.has(`${row.categoryCode}:${row.domainCode}`)}
          onRowClick={(row) => toggleDomain(`${row.categoryCode}:${row.domainCode}`)}
          pagination={
            <TablePagination
              itemLabel={pageT("domains.itemLabel", "tool domains")}
              {...domainPagination.paginationProps}
            />
          }
          renderExpandedRow={(domain) => {
            const domainTools = domain.tools;
            return domainTools.length ? (
              <div className="overflow-x-auto bg-[var(--color-elevated-surface)]">
                <table className="w-full min-w-[600px] table-fixed whitespace-normal text-left text-sm [&_td]:break-words [&_td]:[overflow-wrap:anywhere]">
                  <colgroup>
                    <col className="w-[160px]" />
                    <col />
                    <col className="w-[70px]" />
                    <col className="w-[220px]" />
                  </colgroup>
                  <thead className="border-b border-[var(--color-card-border)] text-xs font-medium text-[var(--color-on-surface-variant)]">
                    <tr>
                      <th className="px-4 py-2.5">{pageT("domains.tools.columns.name", "工具")}</th>
                      <th className="px-4 py-2.5">{pageT("domains.tools.columns.description", "描述")}</th>
                      <th className="whitespace-nowrap px-4 py-2.5">{pageT("domains.tools.columns.type", "类型")}</th>
                      <th className="px-4 py-2.5">{pageT("permissions.title", "Execution permission")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {domainTools.map((tool) => (
                      <tr className="border-b border-[var(--color-card-border)] align-top last:border-b-0" key={tool.toolCode}>
                        <td className="px-4 py-2.5">
                          <p className="font-medium text-[var(--color-on-surface)]">{tool.name || tool.toolName}</p>
                          {tool.name && tool.name !== tool.toolName && <p className="mt-1 break-all font-mono text-xs text-[var(--color-on-surface-variant)]">{tool.toolName}</p>}
                          {tool.assignable === false && <p className="mt-1 text-xs text-[var(--color-on-surface-variant)]">{tool.disabledReason || pageT("categories.unavailable", "Unavailable")}</p>}
                          <details className="mt-1 text-xs text-[var(--color-on-surface-variant)]">
                            <summary>{pageT("categories.source", "Source")}: {tool.source?.providerId ?? tool.source?.pluginId ?? pageT("categories.unknown", "Unknown")}</summary>
                            <dl className="mt-1 space-y-0.5">
                              {([
                                ["pack", tool.source?.packId],
                                ["plugin", tool.source?.pluginId],
                              ] as const).filter(([, value]) => value).map(([key, value]) => (
                                <div key={key}><dt className="inline">{pageT(`categories.${key}`, key)}: </dt><dd className="inline break-all">{value}</dd></div>
                              ))}
                            </dl>
                          </details>
                        </td>
                        <td className="px-4 py-2.5 text-[13px] text-[var(--color-on-surface-variant)]">{tool.description}</td>
                        <td className="whitespace-nowrap px-4 py-2.5">{tool.operationKind === "unknown" ? pageT("categories.unknownKind", "Undeclared") : t(`common:toolOperationKind.${tool.operationKind}`, { defaultValue: tool.operationKind })}</td>
                        <td className="px-4 py-2.5"><ToolPermissionControl name={tool.name} disabled={loading || saving || !loaded || tool.assignable === false}
                          value={permissions.find((item) => item.toolCode === tool.toolCode) ?? { toolCode: tool.toolCode, permission: "run", confirmLevel: "low" }}
                          onChange={setPermission} /></td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : <p className="py-2 text-sm text-[var(--color-on-surface-variant)]">{pageT("domains.tools.empty", "该领域没有可用工具。")}</p>;
          }}
          toolbar={
            <SearchField
              ariaLabel={pageT("domains.searchAria", "Search tool domains")}
              className="min-w-0 grow basis-full sm:basis-0 sm:max-w-[360px]"
              disabled={loading}
              placeholder={pageT("domains.searchPlaceholder", "Search tool domains...")}
              value={domainSearch}
              onChange={(value) => { setDomainSearch(value); domainPagination.paginationProps.onPageChange(1); }}
            />
          }
          title={pageT("domains.title", "Tool Domains")}
          emptyState={pageT("domains.empty", "No domains match the current filters.")}
          columns={[
            {
              id: "domain",
              header: pageT("domains.columns.domainCode", "Domain"),
              width: "25%",
              cellClassName: "whitespace-normal",
              render: (row) => <Button variant="link" aria-expanded={expandedDomainCodes.has(`${row.categoryCode}:${row.domainCode}`)} aria-label={`${row.domainCode}: ${row.name}`} onClick={(event) => { event.stopPropagation(); toggleDomain(`${row.categoryCode}:${row.domainCode}`); }} className="h-auto max-w-full justify-start whitespace-normal px-0 text-left font-medium text-[var(--color-on-surface)] hover:no-underline"><span className="min-w-0 break-all [overflow-wrap:anywhere]">{row.domainCode}</span></Button>,
            },
            {
              id: "description",
              header: pageT("domains.columns.description", "Description"),
              width: "45%",
              cellClassName: "whitespace-normal",
              render: (row) => (
                <p className="break-words text-[13px] text-[var(--color-on-surface-variant)] [overflow-wrap:anywhere]">
                  {row.description}
                </p>
              ),
            },
            {
              id: "tools",
              header: pageT("domains.columns.toolCount", "Tools"),
              width: "12%",
              align: "right",
              render: (row) => <span>{row.toolCount}</span>,
            },
            {
              id: "mutation",
              header: pageT("domains.columns.mutation", "Mutation"),
              width: "18%",
              cellClassName: "whitespace-normal break-words",
              render: (row) => (
                <span>
                  {row.hasMutationTools
                    ? pageT("domains.mutationYes", "Has mutations")
                    : row.tools.some((tool) => tool.operationKind === "unknown")
                      ? pageT("categories.unknownKind", "Undeclared")
                      : pageT("domains.mutationNo", "Read-only")}
                </span>
              ),
            },
          ]}
        />
    </div>
  );
}
