import clsx from "clsx";
import { motion } from "motion/react";
import { useId, type ReactNode } from "react";
import { spring } from "../design/motion";
import styles from "./Segmented.module.css";

export interface SegmentedOption<T extends string> {
  value: T;
  label: ReactNode;
  icon?: ReactNode;
  disabled?: boolean;
}

interface SegmentedProps<T extends string> {
  value: T;
  onChange: (value: T) => void;
  options: SegmentedOption<T>[];
  size?: "sm" | "md";
  ariaLabel?: string;
  className?: string;
}

export function Segmented<T extends string>({ value, onChange, options, size = "md", ariaLabel, className }: SegmentedProps<T>) {
  const id = useId();
  return (
    <div className={clsx(styles.root, styles[size], className)} role="radiogroup" aria-label={ariaLabel}>
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            disabled={option.disabled}
            className={styles.option}
            data-selected={selected || undefined}
            onClick={() => onChange(option.value)}
          >
            {selected && <motion.span layoutId={`segmented-${id}`} className={styles.pill} transition={spring.snappy} />}
            <span className={styles.content}>
              {option.icon}
              {option.label}
            </span>
          </button>
        );
      })}
    </div>
  );
}
