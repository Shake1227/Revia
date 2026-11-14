import clsx from "clsx";
import { useState, type ReactNode } from "react";
import { useT } from "../i18n";
import type { AppError } from "../ipc/types";
import styles from "./ErrorNote.module.css";
import { Icons } from "./icons";
import { InstallCliButton } from "./InstallCliButton";

interface ErrorNoteProps {
  error: AppError;
  actions?: ReactNode;
  compact?: boolean;
  className?: string;
}

export function ErrorNote({ error, actions, compact, className }: ErrorNoteProps) {
  const m = useT();
  const [open, setOpen] = useState(false);
  const text = m.error[error.code] ?? m.error.unknown;
  const hint = text.hint;
  const fix = error.code === "gh_not_installed" ? <InstallCliButton variant="secondary" size="sm" /> : null;
  return (
    <div className={clsx(styles.note, compact && styles.compact, className)} role="alert">
      <Icons.circleAlert size={15} className={styles.icon} />
      <div className={styles.body}>
        <p className={styles.title}>{text.title}</p>
        {hint && <p className={styles.hint}>{hint}</p>}
        {error.detail && open && <pre className={`${styles.detail} selectable mono`}>{error.detail}</pre>}
        {(fix || actions || error.detail) && (
          <div className={styles.actions}>
            {fix}
            {actions}
            {error.detail && (
              <button type="button" className={styles.link} onClick={() => setOpen((value) => !value)}>
                {open ? m.common.hideDetails : m.common.showDetails}
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

interface NoteProps {
  tone?: "info" | "warning" | "danger" | "success";
  icon?: ReactNode;
  children: ReactNode;
  className?: string;
}

export function Note({ tone = "info", icon, children, className }: NoteProps) {
  return (
    <div className={clsx(styles.note, styles.plain, className)} data-tone={tone}>
      <span className={styles.icon}>{icon ?? (tone === "info" ? <Icons.info size={15} /> : tone === "success" ? <Icons.circleCheck size={15} /> : <Icons.warning size={15} />)}</span>
      <div className={styles.body}>{children}</div>
    </div>
  );
}
