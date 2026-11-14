import { useEffect, useMemo, useState } from "react";
import { Button, IconButton } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import { ErrorNote } from "../../components/ErrorNote";
import { Field } from "../../components/Field";
import { Icons } from "../../components/icons";
import { Select } from "../../components/Select";
import { Spinner } from "../../components/Spinner";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import type { MergeMode, OperationRequest } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { runAndWait } from "../../stores/operations";
import { toast } from "../../stores/toasts";
import { useProjectStore } from "../../stores/projects";
import { useAsync } from "../detail/hooks";
import styles from "./dialogs.module.css";

interface MergeDialogProps {
  request: { projectId: string; source: string | null; target: string | null } | null;
  onClose: () => void;
}

export function MergeDialog({ request, onClose }: MergeDialogProps) {
  const m = useT();
  const t = m.merge;
  const [shown, setShown] = useState<MergeDialogProps["request"]>(null);
  const [source, setSource] = useState<string | undefined>(undefined);
  const [target, setTarget] = useState<string | undefined>(undefined);
  const [mode, setMode] = useState<MergeMode>("default");
  const current = request ?? shown;
  const projectId = current?.projectId ?? "";
  const project = useProjectStore((state) => state.byId[projectId]);

  useEffect(() => {
    if (!request) return;
    setShown(request);
    setSource(request.source ?? undefined);
    setTarget(request.target ?? undefined);
    setMode("default");
  }, [request]);

  const detail = useAsync(() => (request ? api.getProjectDetail({ projectId: request.projectId }) : Promise.resolve(null)), [request?.projectId]);
  const branches = detail.data?.branches ?? [];
  const local = branches.filter((branch) => branch.kind === "local");
  const sourceOptions = useMemo(
    () =>
      branches
        .filter((branch) => branch.name !== target)
        .map((branch) => ({ value: branch.name, label: branch.name, group: branch.kind === "local" ? m.branches.local : m.branches.remote })),
    [branches, target, m],
  );
  const targetOptions = useMemo(
    () => local.filter((branch) => branch.name !== source).map((branch) => ({ value: branch.name, label: branch.current ? `${branch.name}（${m.branches.current}）` : branch.name })),
    [local, source, m],
  );

  const ready = request !== null && source !== undefined && target !== undefined && source !== target;
  const preview = useAsync(
    () => (ready ? api.previewMerge({ projectId, source: source!, target: target! }) : Promise.resolve(null)),
    [ready, projectId, source, target],
  );
  const data = ready ? preview.data : null;
  const blocked = data?.blocker ?? null;
  const ffOnlyImpossible = mode === "ff_only" && data !== null && !data.fastForward && !data.upToDate;
  const canRun = data !== null && !preview.loading && !blocked && !data.upToDate && !ffOnlyImpossible;
  const sourceIsLocal = local.some((branch) => branch.name === source);

  const [starting, setStarting] = useState(false);
  const submit = async () => {
    if (!data || !source || !target || starting) return;
    const request: OperationRequest = { kind: "merge", items: [{ itemId: projectId, projectId, source, target, mode }] };
    setStarting(true);
    try {
      const plan = await api.planOperation({ request });
      const item = plan.items[0];
      const step = item?.steps.find((entry) => entry.code === "merge");
      if (!item?.executable || step?.code !== "merge" || step.commits !== data.commitCount) {
        toast.warning(m.error.state_changed.title, { description: m.error.state_changed.hint });
        preview.reload();
        return;
      }
      onClose();
      await runAndWait(request, plan);
    } catch (error) {
      notifyError(error);
    } finally {
      setStarting(false);
    }
  };

  return (
    <Dialog
      open={request !== null}
      onOpenChange={(next) => !next && onClose()}
      title={t.title}
      description={project?.name}
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant="primary" disabled={!canRun} loading={starting} onClick={() => void submit()}>
            {t.submit}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <div className={styles.mergePick}>
          <Field label={t.source}>
            <Select mono value={source} onChange={setSource} options={sourceOptions} placeholder={m.common.select} leading={<Icons.branch size={13} />} />
          </Field>
          <IconButton
            label={t.swap}
            disabled={!source || !target || !sourceIsLocal}
            onClick={() => {
              setSource(target);
              setTarget(source);
            }}
          >
            <Icons.arrowRight size={15} />
          </IconButton>
          <Field label={t.target}>
            <Select mono value={target} onChange={setTarget} options={targetOptions} placeholder={m.common.select} leading={<Icons.branch size={13} />} />
          </Field>
        </div>

        <div className={styles.preview} aria-live="polite">
          {!ready ? (
            <p className={styles.hint}>{t.preview}</p>
          ) : preview.error ? (
            <ErrorNote error={preview.error} compact />
          ) : !data || preview.loading ? (
            <p className={styles.waiting}>
              <Spinner size={13} />
              {t.checking}
            </p>
          ) : data.upToDate ? (
            <p className={styles.previewLine} data-tone="ok">
              <Icons.circleCheck size={14} />
              {t.upToDate}
            </p>
          ) : (
            <>
              <p className={styles.previewLine}>
                <Icons.commit size={14} />
                <span>
                  {t.commits(data.commitCount)}{m.common.dot}{t.files(data.filesChanged)}
                </span>
              </p>
              <p className={styles.previewLine}>
                <Icons.merge size={14} />
                {data.fastForward && mode !== "no_ff" ? t.fastForward : t.mergeCommit}
              </p>
              {data.needsCheckout && (
                <p className={styles.previewLine}>
                  <Icons.branch size={14} />
                  {t.needsCheckout(data.target.name)}
                </p>
              )}
              {data.conflicts.status === "none" && (
                <p className={styles.previewLine} data-tone="ok">
                  <Icons.circleCheck size={14} />
                  {t.conflictsNone}
                </p>
              )}
              {data.conflicts.status === "conflicts" && (
                <p className={styles.previewLine} data-tone="warn">
                  <Icons.warning size={14} />
                  <span>
                    {t.conflictsFound(data.conflicts.paths.length)}
                    <span className="mono selectable" style={{ display: "block", fontSize: 11.5, opacity: 0.85 }}>
                      {data.conflicts.paths.slice(0, 5).join("\n")}
                    </span>
                    {t.conflictNote}
                  </span>
                </p>
              )}
              {data.conflicts.status === "unknown" && (
                <p className={styles.previewLine} data-tone="warn">
                  <Icons.circleAlert size={14} />
                  <span>
                    {t.conflictsUnknown} {t.conflictNote}
                  </span>
                </p>
              )}
              {blocked && (
                <p className={styles.previewLine} data-tone="danger">
                  <Icons.circleX size={14} />
                  {blocked === "dirty_worktree" ? t.dirtyBlock : m.ops.blocker[blocked]}
                </p>
              )}
              {ffOnlyImpossible && (
                <p className={styles.previewLine} data-tone="danger">
                  <Icons.circleX size={14} />
                  {m.ops.blocker.diverged}
                </p>
              )}
              {data.commits.length > 0 && (
                <div className={styles.commitList}>
                  {data.commits.slice(0, 5).map((commit) => (
                    <p key={commit.sha} className="truncate">
                      <span className="mono">{commit.shortSha}</span>
                      {commit.subject}
                    </p>
                  ))}
                  {data.commitCount > 5 && <p>… +{data.commitCount - 5}</p>}
                </div>
              )}
            </>
          )}
        </div>

        <Field label={t.mode.label} hint={t.mode[`${mode}Hint` as const]}>
          <Select
            value={mode}
            onChange={(value: MergeMode) => setMode(value)}
            options={(["default", "no_ff", "ff_only"] as const).map((value) => ({ value, label: t.mode[value] }))}
          />
        </Field>
      </div>
    </Dialog>
  );
}
