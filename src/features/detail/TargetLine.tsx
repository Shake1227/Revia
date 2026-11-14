import clsx from "clsx";
import { Icons } from "../../components/icons";
import { Tooltip } from "../../components/Tooltip";
import { useT } from "../../i18n";
import type { Project, TargetState } from "../../ipc/types";
import { useUiStore } from "../../stores/ui";
import { remoteLabel } from "../projects/model";
import styles from "./TargetLine.module.css";

function TargetChip({ label, state, onReselect }: { label: string; state: TargetState; onReselect: () => void }) {
  const m = useT();
  if (state.savedInvalid && state.saved) {
    return (
      <button type="button" className={clsx(styles.chip, styles.invalid)} onClick={onReselect}>
        <span className={styles.chipLabel}>{label}</span>
        <span className="mono">{remoteLabel(state.saved)}</span>
        <span>
          {m.detail.target.invalid[state.savedInvalid]} — {m.detail.target.reselect}
        </span>
      </button>
    );
  }
  if (!state.target) {
    return (
      <button type="button" className={clsx(styles.chip, styles.unset)} onClick={onReselect}>
        <span className={styles.chipLabel}>{label}</span>
        <span>{m.detail.target.unset}</span>
      </button>
    );
  }
  return (
    <Tooltip label={m.detail.target.source[state.target.source]} hint={m.detail.target.sourceHint[state.target.source]} side="bottom">
      <button type="button" className={styles.chip} onClick={onReselect}>
        <span className={styles.chipLabel}>{label}</span>
        <span className="mono">{remoteLabel(state.target)}</span>
        {!state.target.exists && <span className={styles.newTag}>{m.detail.target.newBranch}</span>}
      </button>
    </Tooltip>
  );
}

export function TargetLine({ project }: { project: Project }) {
  const m = useT();
  const openDialog = useUiStore((state) => state.openDialog);
  const status = project.status;
  if (!status || !status.branch || status.remotes.length === 0) {
    if (!status?.branch && status?.detached && status.headSha) {
      return (
        <div className={styles.line}>
          <span className={clsx(styles.branch, styles.detached)}>
            <Icons.commit size={13} />
            <span className="mono">{status.headSha}</span>
          </span>
        </div>
      );
    }
    if (!status?.branch) return null;
  }
  const reselect = () => openDialog({ type: "target", projectId: project.id });
  const same =
    status.targets.push.target &&
    status.targets.pull.target &&
    !status.targets.push.savedInvalid &&
    !status.targets.pull.savedInvalid &&
    remoteLabel(status.targets.push.target) === remoteLabel(status.targets.pull.target);

  return (
    <div className={styles.line}>
      <span className={styles.branch}>
        <Icons.branch size={13} />
        <span className="mono truncate">{status.branch}</span>
      </span>
      {status.remotes.length > 0 && (
        <>
          <Icons.arrowLeftRight size={13} className={styles.arrow} />
          {same ? (
            <TargetChip label={m.common.pair(m.detail.target.push, m.detail.target.pull)} state={status.targets.push} onReselect={reselect} />
          ) : (
            <>
              <TargetChip label={m.detail.target.push} state={status.targets.push} onReselect={reselect} />
              <TargetChip label={m.detail.target.pull} state={status.targets.pull} onReselect={reselect} />
            </>
          )}
        </>
      )}
    </div>
  );
}
