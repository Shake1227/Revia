import clsx from "clsx";
import { useState } from "react";
import { Button } from "../../components/Button";
import { Icons } from "../../components/icons";
import { Spinner } from "../../components/Spinner";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import { toAppError } from "../../ipc/errors";
import type { Project } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { confirm } from "../../stores/confirm";
import { toast } from "../../stores/toasts";
import { openIn } from "../projects/projectActions";
import styles from "./ConflictPanel.module.css";
import { useAsync } from "./hooks";

export function ConflictPanel({ project, signature }: { project: Project; signature: string }) {
  const m = useT();
  const t = m.detail.conflict;
  const projectId = project.id;
  const conflicts = useAsync(() => api.getConflicts({ projectId }), [projectId, signature]);
  const [pending, setPending] = useState<string | null>(null);
  const state = conflicts.data;
  const operation = state?.operation ? m.inProgress[state.operation] : project.status?.inProgress ? m.inProgress[project.status.inProgress] : m.inProgress.merge;

  const resolve = async (path: string) => {
    setPending(path);
    try {
      await api.markResolved({ projectId, paths: [path] });
    } catch (error) {
      if (toAppError(error).code === "conflict_markers") {
        toast.warning(t.markersBlocked.title, {
          description: t.markersBlocked.body,
          action: { label: t.openInEditor, run: () => void openIn(projectId, "editor", path) },
        });
      } else {
        notifyError(error);
      }
    } finally {
      setPending(null);
      conflicts.reload();
    }
  };

  const run = async (key: string, action: () => Promise<unknown>, done?: string) => {
    setPending(key);
    try {
      await action();
      if (done) toast.success(done);
      conflicts.reload();
    } catch (error) {
      notifyError(error);
    } finally {
      setPending(null);
    }
  };

  const abort = async () => {
    const result = await confirm({
      title: m.confirm.abort.title(operation),
      body: m.confirm.abort.body(operation),
      facts: [{ label: m.confirm.project, value: project.name }],
      risk: m.confirm.abort.risk,
      level: "caution",
      confirmLabel: m.confirm.abort.submit(operation),
    });
    if (result.confirmed) await run("abort", () => api.abortInProgress({ projectId }), m.confirm.abort.done(operation));
  };

  const files = state?.files ?? [];
  return (
    <section className={styles.panel} aria-live="polite">
      <header className={styles.head}>
        <span className={styles.icon}>
          <Icons.warning size={16} />
        </span>
        <div className={styles.heading}>
          <h2>{t.title(operation)}</h2>
          <p>{files.length > 0 ? t.body : t.resolvedAll}</p>
        </div>
        {state?.incoming && <span className={clsx(styles.incoming, "mono")}>{t.incoming(state.incoming)}</span>}
      </header>

      {conflicts.loading && !state ? (
        <div className={styles.loading}>
          <Spinner size={15} />
        </div>
      ) : (
        files.length > 0 && (
          <ul className={styles.files}>
            {files.map((file) => (
              <li key={file.path} className={styles.file}>
                <div className={styles.fileMain}>
                  <span className={clsx("mono selectable truncate", styles.path)}>{file.path}</span>
                  <span className={styles.kind}>
                    {t.kind[file.kind]}
                    {file.hasMarkers && <span className={styles.markers}>{m.common.dot}{t.markersRemain}</span>}
                  </span>
                </div>
                <Button size="sm" variant="ghost" icon={<Icons.code size={13} />} onClick={() => void openIn(projectId, "editor", file.path)}>
                  {t.openInEditor}
                </Button>
                <Button
                  size="sm"
                  loading={pending === file.path}
                  onClick={() => void resolve(file.path)}
                >
                  {t.markResolved}
                </Button>
              </li>
            ))}
          </ul>
        )
      )}

      <footer className={styles.foot}>
        <span className="tabular">{files.length > 0 ? t.remaining(files.length) : ""}</span>
        <div className={styles.actions}>
          <Button variant="danger" loading={pending === "abort"} onClick={() => void abort()}>
            {t.abort(operation)}
          </Button>
          <Button
            variant="primary"
            disabled={!state?.canContinue}
            loading={pending === "continue"}
            onClick={() => void run("continue", () => api.continueInProgress({ projectId }), m.confirm.continued(operation))}
          >
            {t.continue(operation)}
          </Button>
        </div>
      </footer>
    </section>
  );
}
