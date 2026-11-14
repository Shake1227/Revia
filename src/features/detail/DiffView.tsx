import clsx from "clsx";
import { memo } from "react";
import { Spinner } from "../../components/Spinner";
import { useT } from "../../i18n";
import type { AppError, FileDiff } from "../../ipc/types";
import { ErrorNote } from "../../components/ErrorNote";
import styles from "./DiffView.module.css";

interface DiffViewProps {
  diff: FileDiff | null;
  loading: boolean;
  error: AppError | null;
  placeholder: string;
}

export const DiffView = memo(function DiffView({ diff, loading, error, placeholder }: DiffViewProps) {
  const m = useT();
  if (error) {
    return (
      <div className={styles.message}>
        <ErrorNote error={error} compact />
      </div>
    );
  }
  if (!diff) {
    return <div className={styles.message}>{loading ? <Spinner size={16} /> : <span>{placeholder}</span>}</div>;
  }
  if (diff.binary) return <div className={styles.message}>{m.detail.changes.binary}</div>;
  if (diff.hunks.length === 0) return <div className={styles.message}>{m.detail.changes.noDiff}</div>;

  return (
    <div className={clsx(styles.diff, "mono selectable")} data-loading={loading || undefined}>
      {diff.hunks.map((hunk, index) => (
        <div key={`${hunk.header}-${index}`} className={styles.hunk}>
          <div className={styles.hunkHeader}>{hunk.header}</div>
          {hunk.lines.map((line, lineIndex) => (
            <div key={lineIndex} className={styles.line} data-kind={line.kind}>
              <span className={styles.number}>{line.oldNo ?? ""}</span>
              <span className={styles.number}>{line.newNo ?? ""}</span>
              <span className={styles.sign}>{line.kind === "add" ? "+" : line.kind === "del" ? "−" : ""}</span>
              <span className={styles.text}>{line.text || " "}</span>
            </div>
          ))}
        </div>
      ))}
      {diff.truncated && <div className={styles.truncated}>{m.detail.changes.truncated}</div>}
    </div>
  );
});
