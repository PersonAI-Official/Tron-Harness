import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { Button } from "@/components/ui/button";
import { SearchField } from "@/components/search-field";
import { PageHeader } from "@/shared";
import { listTools } from "../api";
import {
  getNativeToolConfig,
  replaceNativeToolConfig,
} from "../api/agentManagementApi";
import type {
  NativeToolConfigRecord,
  NativeToolSchemeRecord,
  PlatformToolRecord,
} from "../model";

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
  const [search, setSearch] = useState("");
  const [savedAt, setSavedAt] = useState("");

  const pageT = (key: string, fallback: string, options?: Record<string, unknown>) =>
    t(`pages:settingsTools.nativeSchemes.${key}`, { defaultValue: fallback, ...options });

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    Promise.all([
      getNativeToolConfig(controller.signal),
      listTools({}, controller.signal),
    ])
      .then(([nextConfig, nextTools]) => {
        setConfig(nextConfig);
        setTools(nextTools);
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

  const visibleTools = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return tools.filter((tool) => {
      if (tool.assignable === false) return false;
      if (!needle) return true;
      return [tool.toolName, tool.toolCode, tool.name, tool.description, tool.domainCode]
        .join(" ")
        .toLowerCase()
        .includes(needle);
    });
  }, [search, tools]);

  const groups = useMemo(() => {
    const grouped = new Map<string, PlatformToolRecord[]>();
    for (const tool of visibleTools) {
      const list = grouped.get(tool.domainCode) ?? [];
      list.push(tool);
      grouped.set(tool.domainCode, list);
    }
    return [...grouped.entries()].sort(([left], [right]) => left.localeCompare(right));
  }, [visibleTools]);

  function updateScheme(index: number, change: Partial<NativeToolSchemeRecord>) {
    setConfig((current) => current ? {
      ...current,
      schemes: current.schemes.map((scheme, at) => at === index ? { ...scheme, ...change } : scheme),
    } : current);
  }

  function addScheme() {
    setConfig((current) => {
      if (!current) return current;
      const code = newSchemeCode(current.schemes);
      return {
        ...current,
        schemes: [...current.schemes, {
          code,
          name: pageT("newSchemeName", "New scheme"),
          toolIds: [],
          builtIn: false,
        }],
      };
    });
  }

  function removeScheme(index: number) {
    setConfig((current) => {
      if (!current) return current;
      const scheme = current.schemes[index];
      if (!scheme || scheme.builtIn) return current;
      const schemes = current.schemes.filter((_, at) => at !== index);
      return {
        ...current,
        schemes,
        defaultSchemeCode: current.defaultSchemeCode === scheme.code
          ? schemes[0]?.code ?? ""
          : current.defaultSchemeCode,
      };
    });
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
      const saved = await replaceNativeToolConfig({
        schemes: config.schemes.map((scheme) => ({
          ...scheme,
          name: scheme.name.trim(),
          toolIds: [...new Set(scheme.toolIds)].sort(),
        })),
        defaultSchemeCode: config.defaultSchemeCode,
      });
      setConfig(saved);
      setSavedAt(pageT("saved", "Saved"));
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
        actions={<div className="flex items-center gap-2">
          <Button variant="outline" onClick={() => navigate("/settings/tools")}>{pageT("back", "Back to tools")}</Button>
          <Button disabled={loading || saving || !config} onClick={() => void save()}>{saving ? pageT("saving", "Saving…") : pageT("save", "Save")}</Button>
        </div>}
      />
      <p className="text-sm text-[var(--color-on-surface-variant)]">{pageT("description", "Choose which tools are exposed directly to the model. Tools not in the selected scheme remain available through tool_search.")}</p>

      {error ? <p role="alert" className="rounded-md border border-[var(--color-error)] p-3 text-sm text-[var(--color-error)]">{error}</p> : null}
      {savedAt ? <p role="status" className="text-sm text-[var(--color-on-surface-variant)]">{savedAt}</p> : null}
      {loading || !config ? <p role="status">{pageT("loading", "Loading schemes…")}</p> : (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="outline" onClick={addScheme}>{pageT("add", "Add scheme")}</Button>
            <div className="w-72"><SearchField value={search} onChange={setSearch} placeholder={pageT("search", "Search tools")} /></div>
          </div>

          <div className="space-y-4">
            {config.schemes.map((scheme, index) => (
              <section key={scheme.code} className="rounded-lg border border-[var(--color-outline-variant)] p-4">
                <div className="flex flex-wrap items-end gap-3">
                  <label className="grid min-w-56 flex-1 gap-1 text-sm">
                    <span>{pageT("name", "Name")}</span>
                    <input
                      className="h-9 rounded-md border border-[var(--color-outline)] bg-transparent px-3"
                      value={scheme.name}
                      onChange={(event) => updateScheme(index, { name: event.currentTarget.value })}
                    />
                  </label>
                  <label className="inline-flex items-center gap-2 text-sm">
                    <input
                      type="radio"
                      name="native-default"
                      checked={config.defaultSchemeCode === scheme.code}
                      onChange={() => setConfig({ ...config, defaultSchemeCode: scheme.code })}
                    />
                    {pageT("default", "Default")}
                  </label>
                  <span className="text-xs text-[var(--color-on-surface-variant)]">{scheme.code}</span>
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={scheme.builtIn || config.schemes.length <= 1}
                    onClick={() => removeScheme(index)}
                  >
                    {pageT("remove", "Remove")}
                  </Button>
                </div>

                <div className="mt-4 space-y-3">
                  {groups.map(([domainCode, domainTools]) => (
                    <div key={domainCode}>
                      <p className="mb-1 text-xs font-medium text-[var(--color-on-surface-variant)]">{domainCode}</p>
                      <div className="grid gap-1 md:grid-cols-2 xl:grid-cols-3">
                        {domainTools.map((tool) => {
                          const checked = scheme.toolIds.includes(tool.toolName);
                          return (
                            <label key={tool.toolCode} className="flex items-start gap-2 rounded px-1 py-1 text-sm">
                              <input
                                type="checkbox"
                                className="mt-0.5"
                                checked={checked}
                                onChange={(event) => {
                                  const next = new Set(scheme.toolIds);
                                  if (event.currentTarget.checked) next.add(tool.toolName);
                                  else next.delete(tool.toolName);
                                  updateScheme(index, { toolIds: [...next] });
                                }}
                              />
                              <span className="min-w-0">
                                <span className="block truncate">{tool.name || tool.toolName}</span>
                                <span className="block truncate text-xs text-[var(--color-on-surface-variant)]">{tool.toolName}</span>
                              </span>
                            </label>
                          );
                        })}
                      </div>
                    </div>
                  ))}
                  {!groups.length ? <p className="text-sm text-[var(--color-on-surface-variant)]">{pageT("noTools", "No matching tools.")}</p> : null}
                </div>
              </section>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
