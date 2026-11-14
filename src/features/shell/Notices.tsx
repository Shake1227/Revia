import { AnimatePresence, motion } from "motion/react";
import { IconButton } from "../../components/Button";
import { Icons } from "../../components/icons";
import { spring } from "../../design/motion";
import { useT } from "../../i18n";
import { displayPath } from "../../lib/path";
import { useAppStore } from "../../stores/app";
import styles from "./Notices.module.css";

export function Notices() {
  const m = useT();
  const recovery = useAppStore((state) => state.recovery);
  const environment = useAppStore((state) => state.environment);
  const dismissRecovery = useAppStore((state) => state.dismissRecovery);
  const gitMissing = environment !== null && !environment.git.installed;

  return (
    <AnimatePresence initial={false}>
      {recovery && (
        <motion.div
          key="recovery"
          className={styles.notice}
          data-tone="warning"
          role="status"
          initial={{ height: 0, opacity: 0 }}
          animate={{ height: "auto", opacity: 1 }}
          exit={{ height: 0, opacity: 0 }}
          transition={spring.smooth}
        >
          <div className={styles.inner}>
            <Icons.warning size={15} />
            <p>
              {recovery.kind === "restored_backup" ? m.notice.recoveryRestored : m.notice.recoveryReset}
              {recovery.corruptCopyPath && <span className="selectable"> {m.notice.recoveryCopy(displayPath(recovery.corruptCopyPath))}</span>}
            </p>
            <IconButton label={m.common.close} size="sm" onClick={dismissRecovery}>
              <Icons.x size={14} />
            </IconButton>
          </div>
        </motion.div>
      )}
      {gitMissing && (
        <motion.div key="git" className={styles.notice} data-tone="danger" role="alert" initial={{ height: 0 }} animate={{ height: "auto" }} exit={{ height: 0 }}>
          <div className={styles.inner}>
            <Icons.circleAlert size={15} />
            <p>{m.notice.gitMissing}</p>
            <IconButton label={m.common.retry} size="sm" onClick={() => void useAppStore.getState().refreshEnvironment()}>
              <Icons.retry size={14} />
            </IconButton>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
