import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { spring } from "../design/motion";
import { useT } from "../i18n";
import { useToastStore, type Toast, type ToastKind } from "../stores/toasts";
import { Icons, type IconComponent } from "./icons";
import styles from "./Toaster.module.css";

const icons: Record<ToastKind, IconComponent> = {
  success: Icons.circleCheck,
  error: Icons.circleX,
  warning: Icons.circleAlert,
  info: Icons.info,
};

function ToastCard({ toast }: { toast: Toast }) {
  const m = useT();
  const dismiss = useToastStore((state) => state.dismiss);
  const [showDetail, setShowDetail] = useState(false);
  const [paused, setPaused] = useState(false);
  const remaining = useRef(toast.duration);
  const startedAt = useRef(0);

  useEffect(() => {
    if (paused || showDetail) return;
    startedAt.current = Date.now();
    const timer = setTimeout(() => dismiss(toast.id), remaining.current);
    return () => {
      clearTimeout(timer);
      remaining.current = Math.max(1200, remaining.current - (Date.now() - startedAt.current));
    };
  }, [paused, showDetail, dismiss, toast.id]);

  const Icon = icons[toast.kind];
  return (
    <motion.li
      layout
      className={styles.toast}
      data-kind={toast.kind}
      role={toast.kind === "error" ? "alert" : "status"}
      initial={{ opacity: 0, y: 16, scale: 0.96 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, x: 24, scale: 0.97, transition: { duration: 0.16 } }}
      transition={spring.smooth}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
    >
      <span className={styles.icon}>
        <Icon size={16} />
      </span>
      <div className={styles.body}>
        <p className={styles.title}>{toast.title}</p>
        {toast.description && <p className={styles.description}>{toast.description}</p>}
        {toast.detail && showDetail && <pre className={`${styles.detail} selectable mono`}>{toast.detail}</pre>}
        {(toast.action || toast.detail) && (
          <div className={styles.actions}>
            {toast.action && (
              <button
                type="button"
                className={styles.action}
                onClick={() => {
                  toast.action?.run();
                  dismiss(toast.id);
                }}
              >
                {toast.action.label}
              </button>
            )}
            {toast.detail && (
              <button type="button" className={styles.link} onClick={() => setShowDetail((value) => !value)}>
                {showDetail ? m.common.hideDetails : m.common.showDetails}
              </button>
            )}
          </div>
        )}
      </div>
      <button type="button" className={styles.close} aria-label={m.common.close} onClick={() => dismiss(toast.id)}>
        <Icons.x size={13} />
      </button>
    </motion.li>
  );
}

export function Toaster() {
  const toasts = useToastStore((state) => state.toasts);
  return (
    <ol className={styles.viewport} aria-live="polite">
      <AnimatePresence mode="popLayout" initial={false}>
        {toasts.map((toast) => (
          <ToastCard key={toast.id} toast={toast} />
        ))}
      </AnimatePresence>
    </ol>
  );
}
