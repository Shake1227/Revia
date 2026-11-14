import clsx from "clsx";
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import styles from "./Button.module.css";
import { Spinner } from "./Spinner";
import { Tooltip } from "./Tooltip";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger" | "dangerSolid";
  size?: "sm" | "md" | "lg";
  icon?: ReactNode;
  trailing?: ReactNode;
  loading?: boolean;
  block?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "md", icon, trailing, loading = false, block, className, children, disabled, type = "button", ...props },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={clsx(styles.button, styles[variant], styles[size], block && styles.block, className)}
      disabled={disabled || loading}
      data-loading={loading || undefined}
      {...props}
    >
      {loading ? <Spinner size={size === "sm" ? 12 : 14} className={styles.spinner} /> : icon}
      {children !== undefined && children !== null && <span className={styles.label}>{children}</span>}
      {trailing}
    </button>
  );
});

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  hint?: ReactNode;
  shortcut?: string[];
  size?: "sm" | "md" | "lg";
  active?: boolean;
  tooltipSide?: "top" | "right" | "bottom" | "left";
  noTooltip?: boolean;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { label, hint, shortcut, size = "md", active, tooltipSide, noTooltip, className, children, type = "button", ...props },
  ref,
) {
  const button = (
    <button
      ref={ref}
      type={type}
      aria-label={label}
      className={clsx(styles.iconButton, styles[`icon-${size}`], className)}
      data-active={active || undefined}
      {...props}
    >
      {children}
    </button>
  );
  if (noTooltip) return button;
  return (
    <Tooltip label={label} hint={hint} shortcut={shortcut} side={tooltipSide}>
      {button}
    </Tooltip>
  );
});
