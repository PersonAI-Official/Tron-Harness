import { ShieldCheck, ShieldEllipsis, ShieldQuestionMark } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ToolExecutionMode } from "@tron/chat-core";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

interface ChatExecutionModeSelectorProps {
  value: ToolExecutionMode;
  disabled?: boolean;
  onChange: (value: ToolExecutionMode) => void;
}

export function ChatExecutionModeSelector({ value, disabled, onChange }: ChatExecutionModeSelectorProps) {
  const { t } = useTranslation("pages");
  const options = [
    { value: "CONTROLLED", icon: ShieldQuestionMark, label: t("chat.management.composer.controlledExecution"), description: t("chat.management.composer.controlledExecutionHint") },
    { value: "AUTO_APPROVE", icon: ShieldCheck, label: t("chat.management.composer.autoApprove"), description: t("chat.management.composer.autoApproveHint") },
    { value: "PLAN", icon: ShieldEllipsis, label: t("chat.management.composer.planMode"), description: t("chat.management.composer.planModeHint") },
  ] as const;
  const selected = options.find((option) => option.value === value) ?? options[0];
  return (
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger render={
          <DropdownMenuTrigger
            render={<Button variant="ghost" size="icon" className="size-8 rounded-lg text-muted-foreground hover:text-foreground data-popup-open:bg-muted data-popup-open:text-foreground" />}
            aria-label={t("chat.management.composer.executionModeCurrent", { mode: selected.label })}
            disabled={disabled}
          />
        }>
          <selected.icon aria-hidden="true" className="size-4" />
        </TooltipTrigger>
        <TooltipContent side="top" sideOffset={8}>
          <span className="block font-medium">{selected.label}</span>
          <span className="mt-1 block">{selected.description}</span>
        </TooltipContent>
      </Tooltip>
      <DropdownMenuContent align="start" side="top" sideOffset={8} className="w-72 max-w-[calc(100vw-2rem)]">
        <DropdownMenuRadioGroup value={value} onValueChange={(next) => {
          const option = options.find((item) => item.value === next);
          if (option) onChange(option.value);
        }}>
          {options.map((option) => (
            <DropdownMenuRadioItem key={option.value} value={option.value} label={option.label} className="items-start gap-2.5 py-2 pl-2">
              <option.icon aria-hidden="true" className="mt-0.5 size-4" />
              <span>
                <span className="block font-medium">{option.label}</span>
                <span className="mt-1 block text-xs text-muted-foreground">{option.description}</span>
              </span>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
