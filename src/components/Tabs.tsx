import { motion } from "motion/react";
import { useId, type ReactNode } from "react";
import { spring } from "../design/motion";
import styles from "./Tabs.module.css";

export interface TabItem<T extends string> {
  value: T;
  label: ReactNode;
  badge?: ReactNode;
}

interface TabsProps<T extends string> {
  value: T;
  onChange: (value: T) => void;
  items: TabItem<T>[];
  ariaLabel?: string;
}

export function Tabs<T extends string>({ value, onChange, items, ariaLabel }: TabsProps<T>) {
  const id = useId();
  return (
    <div className={styles.tabs} role="tablist" aria-label={ariaLabel}>
      {items.map((item) => {
        const selected = item.value === value;
        return (
          <button
            key={item.value}
            type="button"
            role="tab"
            aria-selected={selected}
            tabIndex={selected ? 0 : -1}
            className={styles.tab}
            data-selected={selected || undefined}
            onClick={() => onChange(item.value)}
            onKeyDown={(event) => {
              if (event.key !== "ArrowRight" && event.key !== "ArrowLeft") return;
              const index = items.findIndex((entry) => entry.value === value);
              const next = items[(index + (event.key === "ArrowRight" ? 1 : -1) + items.length) % items.length];
              if (next) {
                onChange(next.value);
                const sibling = event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>("[role=tab]")[items.indexOf(next)];
                sibling?.focus();
              }
            }}
          >
            <span>{item.label}</span>
            {item.badge !== undefined && item.badge !== null && <span className={styles.badge}>{item.badge}</span>}
            {selected && <motion.span layoutId={`tab-${id}`} className={styles.underline} transition={spring.snappy} />}
          </button>
        );
      })}
    </div>
  );
}
