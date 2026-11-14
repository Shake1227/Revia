import { Tooltip as RadixTooltip } from "radix-ui";
import type { ReactNode } from "react";
import { Kbd } from "./Kbd";
import styles from "./Tooltip.module.css";

interface TooltipProps {
  label: ReactNode;
  hint?: ReactNode;
  shortcut?: string[];
  side?: "top" | "right" | "bottom" | "left";
  align?: "start" | "center" | "end";
  children: ReactNode;
  disabled?: boolean;
}

export function Tooltip({ label, hint, shortcut, side = "bottom", align = "center", children, disabled }: TooltipProps) {
  if (disabled || !label) return <>{children}</>;
  return (
    <RadixTooltip.Root>
      <RadixTooltip.Trigger asChild>{children}</RadixTooltip.Trigger>
      <RadixTooltip.Portal>
        <RadixTooltip.Content className={styles.content} side={side} align={align} sideOffset={7} collisionPadding={8}>
          <span className={styles.row}>
            <span>{label}</span>
            {shortcut && <Kbd keys={shortcut} />}
          </span>
          {hint && <span className={styles.hint}>{hint}</span>}
        </RadixTooltip.Content>
      </RadixTooltip.Portal>
    </RadixTooltip.Root>
  );
}

export const TooltipProvider = RadixTooltip.Provider;
