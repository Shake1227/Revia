import type { ReactNode } from "react";
import styles from "./PageHeader.module.css";

interface PageHeaderProps {
  title: ReactNode;
  meta?: ReactNode;
  subtitle?: ReactNode;
  leading?: ReactNode;
  actions?: ReactNode;
  children?: ReactNode;
}

export function PageHeader({ title, meta, subtitle, leading, actions, children }: PageHeaderProps) {
  return (
    <header className={styles.header} data-band={subtitle !== undefined || undefined}>
      {leading}
      <div className={styles.heading}>
        <h1 className={styles.title}>{title}</h1>
        {meta !== undefined && meta !== null && <span className={styles.meta}>{meta}</span>}
      </div>
      {children}
      {actions && <div className={styles.actions}>{actions}</div>}
      {subtitle !== undefined && <p className={styles.subtitle}>{subtitle}</p>}
    </header>
  );
}
