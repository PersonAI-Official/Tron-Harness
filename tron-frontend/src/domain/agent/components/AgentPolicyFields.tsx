import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Field, SelectInput, TextInput } from "@/components/forms";
import { SelectItem } from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { InfoTooltip } from "@/components/info-tooltip";
import type { NativeToolSchemeRecord, PlatformToolRecord, WorkspaceInternalAgentRecord } from "../model";
import { AssistantToolPolicyFields } from "./AssistantToolPolicyFields";

export type AgentPolicyDraft = Pick<WorkspaceInternalAgentRecord, "toolAssignmentMode" | "maxSteps" | "maxToolCalls" | "maxParallelToolCalls" | "mcpAccessMode" | "selectedToolCodes" | "selectedServerCodes" | "selectedCategoryCodes" | "nativeToolSchemeCode">;
export const newAgentPolicyDraft = (): AgentPolicyDraft => ({ toolAssignmentMode: "all_platform_tools", maxSteps: 0, maxToolCalls: 0, maxParallelToolCalls: 10, mcpAccessMode: "all_enabled_servers", selectedCategoryCodes: [], selectedToolCodes: [], selectedServerCodes: [], nativeToolSchemeCode: "" });
const labels = { mcpAccessMode: "MCP access mode", toolAssignmentMode: "Tool assignment mode", nativeToolSchemeCode: "Native tool scheme", maxSteps: "Max steps", maxToolCalls: "Max total tool calls", maxParallelToolCalls: "Max parallel tool calls" };
export function AgentPolicyFields({ panel, draft, disabled, onChange, onAssignment, children, tools = [], nativeSchemes = [], audience = "agent", showNativeScheme = true }: {
  tools?: PlatformToolRecord[];
  nativeSchemes?: NativeToolSchemeRecord[];
  showNativeScheme?: boolean;
  audience?: "agent" | "assistant";
  panel: "resources" | "advanced"; draft: AgentPolicyDraft; disabled: boolean;
  onChange: (patch: Partial<AgentPolicyDraft>) => void; onAssignment?: (kind: "mcp" | "tools") => void; children?: ReactNode;
}) {
  const { t, i18n } = useTranslation("pages");
  const categories = [...new Map(tools.filter((tool) => tool.category).map((tool) => [tool.category!.code, { code: tool.category!.code, name: tool.category!.names[i18n.resolvedLanguage ?? i18n.language] ?? tool.category!.names["en-US"] ?? tool.category!.code }])).values()];
  const text = (key: string, fallback: string, options?: Record<string, unknown>) => t(`workspaceAgents.internal.${key}`, { defaultValue: fallback, ...options });
  const label = (key: keyof typeof labels) => <span className="inline-flex items-center gap-1.5"><span>{text(`labels.${key}`, labels[key])}</span><InfoTooltip ariaLabel={text(`help.${key}.ariaLabel`, `Explain ${labels[key]}`)}><p>{text(`help.${key}.summary`, "")}</p><p>{text(`help.${key}.optionOne`, "")}</p><p>{text(`help.${key}.optionTwo`, "")}</p>{key === "toolAssignmentMode" ? <p>{text(`help.${key}.optionThree`, "Select declared read-only tools automatically each run.")}</p> : null}</InfoTooltip></span>;
  if (panel === "advanced") return <div className="grid gap-3 lg:grid-cols-2">
    {showNativeScheme ? <Field label={label("nativeToolSchemeCode")}>
      <SelectInput
        disabled={disabled}
        value={draft.nativeToolSchemeCode || "__default__"}
        onValueChange={(value) => onChange({ nativeToolSchemeCode: value === "__default__" ? "" : value })}
      >
        <SelectItem value="__default__">{text("nativeScheme.inherit", "Use global default")}</SelectItem>
        {nativeSchemes.map((scheme) => <SelectItem key={scheme.code} value={scheme.code}>{scheme.name}</SelectItem>)}
      </SelectInput>
    </Field> : null}
    {(["maxSteps", "maxToolCalls", "maxParallelToolCalls"] as const).map((key) => {
      const allowsUnlimited = key !== "maxParallelToolCalls";
      return <Field key={key} label={label(key)}><TextInput
        aria-label={text(`labels.${key}`, labels[key])}
        disabled={disabled} type="text" inputMode="numeric"
        placeholder={allowsUnlimited ? text("unlimited", "Unlimited") : undefined}
        value={Number.isFinite(draft[key]) && !(allowsUnlimited && draft[key] === 0) ? draft[key] : ""}
        onChange={(event) => {
          const raw = event.currentTarget.value;
          if (!/^\d*$/.test(raw)) return;
          const value = raw === "" ? (allowsUnlimited ? 0 : NaN) : Number(raw);
          onChange({ [key]: value });
        }}
      /></Field>;
    })}
  </div>;
  const mcpLabel = audience === "assistant" ? <span className="inline-flex items-center gap-1.5">{text("assistantToolPolicy.mcpLabel", "MCP servers")}<InfoTooltip ariaLabel={text("assistantToolPolicy.mcpHelpLabel", "About MCP servers")}>{text("assistantToolPolicy.mcpHelp", "Choose which enabled platform MCP servers PersonAI can use.")}</InfoTooltip></span> : label("mcpAccessMode");
  const mcpField = <Field label={mcpLabel} actions={draft.mcpAccessMode === "selected_servers" ? <Button type="button" size="sm" variant="outline" disabled={disabled} onClick={() => onAssignment?.("mcp")}>{text("assignmentDialog.editServers", "Choose servers ({{count}})", { count: draft.selectedServerCodes.length })}</Button> : undefined}>
      <SelectInput disabled={disabled} value={draft.mcpAccessMode} onValueChange={(value) => value === "selected_servers" ? onAssignment?.("mcp") : onChange({ mcpAccessMode: value as AgentPolicyDraft["mcpAccessMode"] })}>
        <SelectItem value="disabled">{text("modes.mcpDisabled", "Disabled")}</SelectItem><SelectItem value="all_enabled_servers">{text("modes.allEnabledServers", "All enabled servers")}</SelectItem><SelectItem value="selected_servers">{text("modes.selectedServers", "Selected servers")}</SelectItem>
      </SelectInput>
    </Field>;
  if (audience === "assistant") return <div className="space-y-5">
    <AssistantToolPolicyFields draft={draft} tools={tools} disabled={disabled} onChange={onChange} onChooseTools={() => onAssignment?.("tools")} />
    <div className="border-t border-[var(--color-outline-variant)] pt-4">{mcpField}</div>
    {children}
  </div>;
  return <div className="grid gap-3 lg:grid-cols-2">
    {mcpField}
    <Field label={label("toolAssignmentMode")} actions={draft.toolAssignmentMode === "selected_tools" ? <Button type="button" size="sm" variant="outline" disabled={disabled} onClick={() => onAssignment?.("tools")}>{text("assignmentDialog.editTools", "Choose tools ({{count}})", { count: draft.selectedToolCodes.length })}</Button> : undefined}>
      <SelectInput disabled={disabled} value={draft.toolAssignmentMode} onValueChange={(value) => value === "selected_tools" ? onAssignment?.("tools") : onChange({ toolAssignmentMode: value as AgentPolicyDraft["toolAssignmentMode"] })}>
        <SelectItem value="all_platform_tools">{text("modes.allTools", "All platform tools")}</SelectItem><SelectItem value="all_read_only_tools">{text("modes.allReadOnlyTools", "All read-only tools")}</SelectItem><SelectItem value="selected_categories">{text("modes.selectedCategories", "Selected categories")}</SelectItem><SelectItem value="selected_tools">{text("modes.selectedTools", "Selected tools")}</SelectItem>
      </SelectInput>
    </Field>
    {showNativeScheme ? <div className="lg:col-span-2"><Field label={label("nativeToolSchemeCode")}>
      <SelectInput
        disabled={disabled}
        value={draft.nativeToolSchemeCode || "__default__"}
        onValueChange={(value) => onChange({ nativeToolSchemeCode: value === "__default__" ? "" : value })}
      >
        <SelectItem value="__default__">{text("nativeScheme.inherit", "Use global default")}</SelectItem>
        {nativeSchemes.map((scheme) => <SelectItem key={scheme.code} value={scheme.code}>{scheme.name}</SelectItem>)}
      </SelectInput>
    </Field></div> : null}
    {draft.toolAssignmentMode === "selected_categories" && <div className="lg:col-span-2"><Field label={text("labels.toolCategories", "Tool categories")} hintAriaLabel={text("help.toolCategories.ariaLabel", "Explain tool categories")} hint={<p>{text("categorySelectionHint", "Tools from any selected category are available; new tools in those categories are included automatically.")}</p>}>
      <div className="flex flex-wrap gap-3">{[...categories, {code: "readonly", name: t("settingsTools.categories.readonly", { defaultValue: "Read-only tools" })}].map((category) => <label key={category.code} className="inline-flex items-center gap-2 text-sm"><input type="checkbox" disabled={disabled} checked={(draft.selectedCategoryCodes ?? []).includes(category.code)} onChange={(event) => { const codes = new Set(draft.selectedCategoryCodes ?? []); if (event.target.checked) codes.add(category.code); else codes.delete(category.code); onChange({selectedCategoryCodes: [...codes]}); }} />{category.name}</label>)}</div>
    </Field></div>}
    {children}
  </div>;
}
