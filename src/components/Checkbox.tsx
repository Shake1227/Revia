import clsx from "clsx";
import { Checkbox as RadixCheckbox } from "radix-ui";
import type { ReactNode } from "react";
import styles from "./Checkbox.module.css";

interface CheckboxProps {
  checked: boolean | "indeterminate";
  onChange?: (checked: boolean) => void;
  label?: ReactNode;
  hint?: ReactNode;
  disabled?: boolean;
  ariaLabel?: string;
  className?: string;
  tabIndex?: number;
  onClick?: (event: React.MouseEvent) => void;
}

export function Checkbox({ checked, onChange, label, hint, disabled, ariaLabel, className, tabIndex, onClick }: CheckboxProps) {
  const box = (
    <RadixCheckbox.Root
      className={styles.box}
      checked={checked}
      onCheckedChange={(value) => onChange?.(value === true)}
      disabled={disabled}
      aria-label={ariaLabel}
      tabIndex={tabIndex}
      onClick={onClick}
    >
      <svg viewBox="0 0 14 14" className={styles.mark} aria-hidden>
        {checked === "indeterminate" ? (
          <path d="M3.5 7h7" className={styles.path} pathLength={1} />
        ) : (
          <path d="M3.2 7.300l2.600 2.600 5-5.600" className={styles.path} pathLength={1} />
        )}
      </svg>
    </RadixCheckbox.Root>
  );
  if (!label) return <span className={clsx(styles.bare, className)}>{box}</span>;
  return (
    <label className={clsx(styles.field, className)} data-disabled={disabled || undefined}>
      {box}
      <span className={styles.text}>
        <span className={styles.label}>{label}</span>
        {hint && <span className={styles.hint}>{hint}</span>}
      </span>
    </label>
  );
}
