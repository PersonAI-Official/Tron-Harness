import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { Check, ChevronRight, Plus, Trash2, X } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { SearchField } from "@/components/search-field";
import { PageHeader } from "@/shared";
import { listTools } from "../api";
import { getNativeToolConfig, replaceNativeToolConfig } from "../api/agentManagementApi";
import type {
  NativeToolConfigRecord,
  NativeToolSchemeRecord,
  PlatformToolRecord,
} from "../model";

type Candidate = {
  callName: string;
  name: string;
  description: string;
  domain: string;
};

const BUILTIN_TOOLS: Candidate[] = [
  {
    callName: "command_exec",
    name: "Run command",
    description: "Run a complete shell command string.",
    domain: "command",
  },
  {
    callName: "command_exec_read",
    name: "Read command output",
    description: "Read status and output from a running command.",
    domain: "command",
  },
  {
    callName: "command_exec_stop",
    name: "Stop command",
    description: "Stop a running command task.",
    domain: "command",
  },
];

function newSchemeCode(schemes: NativeToolSchemeRecord[]) {
  let index = schemes.length + 1;
  while (schemes.some((scheme) => scheme.code === `scheme-${index}`)) index += 1;
  return `scheme-${index}`;
}

export function NativeToolSchemesPage() {
  const { t } = useTranslation(["pages", "common"]);
  const navigate = useNavigate();
  const [config, setConfig] = useState<NativeToolConfigRecord | null>(null);
  const [tools, setTools] = useState<PlatformToolRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);
  const [activeCode, setActiveCode] = useState("");
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerSearch, setPickerSearch] = useState("");
  const [pickerSelection, setPickerSelection] = useState<string[]>([]);
  const [expandedDomains, setExpandedDomains] = useState<Set<string>>(() => new Set());

  const pageT = (key: string, fallback: string, options?: Record<string, unknown>) =>
    t(`pages:settingsTools.nativeSchemes.${key}`, { defaultValue: fallback, ...options });

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    Promise.all([getNativeToolConfig(controller.signal), listTools({}, controller.signal)])
      .then(([nextConfig, nextTools]) => {
        setConfig(nextConfig);
        setTools(nextTools);
        setActiveCode((current) =>
          nextConfig.schemes.some((scheme) => scheme.code === current)
            ? current
            : nextConfig.defaultSchemeCode || nextConfig.schemes[0]?.code || ""
        );
        setError("");
      })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) setError(String(reason));
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, []);

  const candidates = useMemo(() => {
    const byCallName = new Map<string, Candidate>();
    for (const tool of BUILTIN_TOOLS) byCallName.set(tool.callName, tool);
    for (const tool of tools) {
      if (tool.assignable === false || byCallName.has(tool.toolName)) continue;
      byCallName.set(tool.toolName, {
        callName: tool.toolName,
        name: tool.name || tool.toolName,
        description: tool.description,
        domain: tool.domainCode,
      });
    }
    return [...byCallName.values()].sort((left, right) =>
      left.domain.localeCompare(right.domain) || left.name.localeCompare(right.name)
    );
  }, [tools]);

  const candidateByCallName = useMemo(
    () => new Map(candidates.map((tool) => [tool.callName, tool])),
    [candidates]
  );

  const scheme = config?.schemes.find((item) => item.code === activeCode) ?? config?.schemes[0];
  const isDefault = !!scheme && config?.defaultSchemeCode === scheme.code;
  const selectedTools = scheme?.toolIds ?? [];
  const selectedSet = useMemo(() => new Set(selectedTools), [selectedTools]);

  const groupedCandidates = useMemo(() => {
    const needle = pickerSearch.trim().toLowerCase();
    const grouped = new Map<string, Candidate[]>();
    for (const candidate of candidates) {
      if (
        needle &&
        ![candidate.callName, candidate.name, candidate.description, candidate.domain]
          .join(" ")
          .toLowerCase()
          .includes(needle)
      ) {
        continue;
      }
      const list = grouped.get(candidate.domain) ?? [];
      list.push(candidate);
      grouped.set(candidate.domain, list);
    }
    return [...grouped.entries()].sort(([left], [right]) => left.localeCompare(right));
  }, [candidates, pickerSearch]);

  function updateScheme(index: number, change: Partial<NativeToolSchemeRecord>) {
    setSaved(false);
    setConfig((current) =>
      current
        ? {
            ...current,
            schemes: current.schemes.map((item, at) =>
              at === index ? { ...item, ...change } : item
            ),
          }
        : current
    );
  }

  function addScheme() {
    if (!config) return;
    setSaved(false);
    const code = newSchemeCode(config.schemes);
    setConfig({
      ...config,
      schemes: [
        ...config.schemes,
        {
          code,
          name: pageT("newSchemeName", "Scheme {{count}}", { count: config.schemes.length + 1 }),
          toolIds: [],
          builtIn: false,
        },
      ],
    });
    setActiveCode(code);
  }

  function removeScheme() {
    if (!config || !scheme || scheme.builtIn || config.schemes.length <= 1) return;
    setSaved(false);
    const remaining = config.schemes.filter((item) => item.code !== scheme.code);
    const nextDefault =
      config.defaultSchemeCode === scheme.code
        ? remaining[0]?.code ?? ""
        : config.defaultSchemeCode;
    setConfig({ ...config, schemes: remaining, defaultSchemeCode: nextDefault });
    setActiveCode(remaining[0]?.code ?? "");
  }

  function openPicker() {
    setPickerSelection([...selectedTools]);
    setPickerSearch("");
    setExpandedDomains(new Set(selectedTools.map((callName) => candidateByCallName.get(callName)?.domain).filter(Boolean) as string[]));
    setPickerOpen(true);
  }

  function applyPicker() {
    if (!config || !scheme) return;
    const index = config.schemes.findIndex((item) => item.code === scheme.code);
    if (index >= 0) updateScheme(index, { toolIds: pickerSelection });
    setPickerOpen(false);
  }

  async function save() {
    if (!config || saving) return;
    if (!config.schemes.length || !config.defaultSchemeCode) {
      setError(pageT("validation", "Keep at least one scheme and choose a default."));
      return;
    }
    setSaving(true);
    setError("");
    try {
      const next = await replaceNativeToolConfig({
        schemes: config.schemes.map((item) => ({
          ...item,
          name: item.name.trim(),
          toolIds: [...new Set(item.toolIds)].sort(),
        })),
        defaultSchemeCode: config.defaultSchemeCode,
      });
      setConfig(next);
      setActiveCode((current) =>
        next.schemes.some((item) => item.code === current)
          ? current
          : next.defaultSchemeCode
      );
      setSaved(true);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="space-y-5">
      <PageHeader
        title={pageT("title", "Native Tool Schemes")}
        actions={
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="outline" onClick={() => navigate("/settings/tools")}>
              {pageT("back", "Back to tools")}
            </Button>
            <Button disabled={loading || saving || !config} onClick={() => void save()}>
              {saving ? pageT("saving", "Saving…") : pageT("save", "Save")}
            </Button>
          </div>
        }
      />
      {error ? (
        <p role="alert" className="rounded-md border border-[var(--color-error)] p-3 text-sm text-[var(--color-error)]">
          {error}
        </p>
      ) : null}
      {saved ? (
        <p role="status" className="text-sm text-[var(--color-success)]">
          {pageT("saved", "Saved")}
        </p>
      ) : null}

      {loading || !config || !scheme ? (
        <p role="status">{pageT("loading", "Loading schemes…")}</p>
      ) : (
        <div className="grid gap-5 lg:grid-cols-[240px_minmax(0,1fr)]">
          <aside className="space-y-2">
            <div className="flex items-center justify-between px-1">
              <p className="text-xs font-semibold uppercase tracking-wide text-[var(--color-on-surface-variant)]">
                {pageT("schemes", "Schemes")}
              </p>
              <Button size="sm" variant="ghost" onClick={addScheme} aria-label={pageT("add", "Add scheme")}>
                <Plus className="size-4" />
              </Button>
            </div>
            <div className="space-y-1">
              {config.schemes.map((item) => {
                const active = item.code === scheme.code;
                return (
                  <button
                    key={item.code}
                    type="button"
                    onClick={() => setActiveCode(item.code)}
                    className={`flex w-full items-center gap-2 rounded-md border px-3 py-2 text-left text-sm transition-colors ${
                      active
                        ? "border-[var(--color-primary)] bg-[var(--color-primary-container)]"
                        : "border-transparent hover:bg-[var(--color-surface-container)]"
                    }`}
                  >
                    <span className="min-w-0 flex-1 truncate">{item.name}</span>
                    {config.defaultSchemeCode === item.code ? (
                      <Badge variant="secondary">{pageT("default", "Default")}</Badge>
                    ) : null}
                    <span className="text-xs text-[var(--color-on-surface-variant)]">{item.toolIds.length}</span>
                  </button>
                );
              })}
            </div>
            <Button className="w-full justify-start" variant="outline" onClick={addScheme}>
              <Plus className="mr-2 size-4" />
              {pageT("add", "Add scheme")}
            </Button>
          </aside>

          <section className="min-w-0 space-y-5 rounded-lg border border-[var(--color-outline-variant)] p-5">
            <div className="flex flex-wrap items-start gap-3">
              <label className="grid min-w-56 flex-1 gap-1 text-sm">
                <span className="text-xs font-medium text-[var(--color-on-surface-variant)]">
                  {pageT("name", "Name")}
                </span>
                <input
                  className="h-10 rounded-md border border-[var(--color-outline)] bg-transparent px-3 text-base font-medium"
                  value={scheme.name}
                  onChange={(event) =>
                    updateScheme(
                      config.schemes.findIndex((item) => item.code === scheme.code),
                      { name: event.currentTarget.value }
                    )
                  }
                />
              </label>
              <div className="flex items-center gap-2 pt-5">
                <Button
                  variant={isDefault ? "secondary" : "outline"}
                  disabled={isDefault}
                  onClick={() => {
                    setSaved(false);
                    setConfig({ ...config, defaultSchemeCode: scheme.code });
                  }}
                >
                  <Check className="mr-2 size-4" />
                  {isDefault ? pageT("defaultScheme", "Global default") : pageT("makeDefault", "Make default")}
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  disabled={scheme.builtIn || config.schemes.length <= 1}
                  aria-label={pageT("remove", "Delete scheme")}
                  onClick={removeScheme}
                >
                  <Trash2 className="size-4" />
                </Button>
              </div>
            </div>

            <div className="flex flex-wrap items-center gap-4 text-sm">
              <span>{pageT("directCount", "Direct")}: <strong>{selectedTools.length}</strong></span>
              <span>{pageT("searchCount", "tool_search")}: <strong>{Math.max(0, candidates.length - selectedTools.length)}</strong></span>
            </div>

            <div>
              <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
                <p className="text-sm font-semibold">{pageT("directTools", "Directly exposed tools")}</p>
                <Button variant="outline" onClick={openPicker}>
                  <Plus className="mr-2 size-4" />
                  {pageT("chooseTools", "Choose tools")}
                </Button>
              </div>

              {selectedTools.length ? (
                <div className="flex flex-wrap gap-2">
                  {selectedTools.map((callName) => {
                    const candidate = candidateByCallName.get(callName);
                    return (
                      <span
                        key={callName}
                        className="inline-flex max-w-full items-center gap-1 rounded-full border border-[var(--color-outline-variant)] bg-[var(--color-surface-container-low)] py-1 pl-3 pr-1 text-sm"
                      >
                        <span className="truncate" title={`${candidate?.name ?? callName} (${callName})`}>
                          {candidate?.name ?? callName}
                        </span>
                        <Button
                          variant="ghost"
                          size="icon"
                          className="size-6"
                          aria-label={pageT("removeTool", "Remove {{tool}}", { tool: candidate?.name ?? callName })}
                          onClick={() =>
                            updateScheme(
                              config.schemes.findIndex((item) => item.code === scheme.code),
                              { toolIds: selectedTools.filter((value) => value !== callName) }
                            )
                          }
                        >
                          <X className="size-3.5" />
                        </Button>
                      </span>
                    );
                  })}
                </div>
              ) : (
                <div className="rounded-md border border-dashed border-[var(--color-outline-variant)] p-4 text-center text-sm text-[var(--color-on-surface-variant)]">
                  {pageT("emptyTools", "None")}
                </div>
              )}
            </div>
          </section>
        </div>
      )}

      <Dialog open={pickerOpen} onOpenChange={setPickerOpen}>
        <DialogContent className="max-w-3xl">
          <DialogHeader>
            <DialogTitle>{pageT("pickerTitle", "Choose tools")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-3">
            <SearchField
              value={pickerSearch}
              onChange={(value) => {
                setPickerSearch(value);
                if (value.trim()) setExpandedDomains(new Set(groupedCandidates.map(([domain]) => domain)));
              }}
              placeholder={pageT("search", "Search tools")}
            />
            <p className="text-xs text-[var(--color-on-surface-variant)]">
              {pageT("selectedCount", "{{count}} selected", { count: pickerSelection.length })}
            </p>
            <div className="max-h-[55vh] space-y-1 overflow-y-auto pr-1">
              {groupedCandidates.map(([domain, domainTools]) => {
                const expanded = expandedDomains.has(domain) || !!pickerSearch.trim();
                const selectedInDomain = domainTools.filter((tool) => pickerSelection.includes(tool.callName)).length;
                return (
                  <div key={domain} className="rounded-md border border-[var(--color-outline-variant)]">
                    <button
                      type="button"
                      className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[var(--color-surface-container)]"
                      onClick={() =>
                        setExpandedDomains((current) => {
                          const next = new Set(current);
                          if (next.has(domain)) next.delete(domain);
                          else next.add(domain);
                          return next;
                        })
                      }
                    >
                      <ChevronRight className={`size-4 transition-transform ${expanded ? "rotate-90" : ""}`} />
                      <span className="min-w-0 flex-1 truncate font-medium">{domain}</span>
                      <span className="text-xs text-[var(--color-on-surface-variant)]">
                        {selectedInDomain}/{domainTools.length}
                      </span>
                    </button>
                    {expanded ? (
                      <div className="grid gap-1 border-t border-[var(--color-outline-variant)] p-2 md:grid-cols-2">
                        {domainTools.map((tool) => {
                          const checked = pickerSelection.includes(tool.callName);
                          return (
                            <label
                              key={tool.callName}
                              className="flex cursor-pointer items-start gap-2 rounded px-2 py-1.5 text-sm hover:bg-[var(--color-surface-container)]"
                            >
                              <input
                                type="checkbox"
                                className="mt-0.5"
                                checked={checked}
                                onChange={(event) => {
                                  setPickerSelection((current) =>
                                    event.currentTarget.checked
                                      ? [...current, tool.callName]
                                      : current.filter((value) => value !== tool.callName)
                                  );
                                }}
                              />
                              <span className="min-w-0">
                                <span className="block truncate font-medium">{tool.callName}</span>
                                <span className="block truncate text-xs text-[var(--color-on-surface-variant)]">
                                  {tool.name}
                                </span>
                              </span>
                            </label>
                          );
                        })}
                      </div>
                    ) : null}
                  </div>
                );
              })}
              {!groupedCandidates.length ? (
                <p className="py-8 text-center text-sm text-[var(--color-on-surface-variant)]">
                  {pageT("noTools", "No matching tools.")}
                </p>
              ) : null}
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setPickerOpen(false)}>
              {pageT("cancel", "Cancel")}
            </Button>
            <Button onClick={applyPicker}>{pageT("apply", "Apply")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
