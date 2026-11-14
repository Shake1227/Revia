import clsx from "clsx";
import { useEffect, useMemo, useState } from "react";
import { Button, IconButton } from "../../components/Button";
import { Checkbox } from "../../components/Checkbox";
import { EmptyState } from "../../components/EmptyState";
import { ErrorNote } from "../../components/ErrorNote";
import { TextArea, TextField } from "../../components/Field";
import { Icons } from "../../components/icons";
import { Menu, MenuContent, MenuItem, MenuTrigger } from "../../components/Menu";
import { Spinner } from "../../components/Spinner";
import { Tooltip } from "../../components/Tooltip";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import type { ChangedFile, FileChangeKind, Project } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { splitFilePath } from "../../lib/path";
import { confirm } from "../../stores/confirm";
import { runAndWait, useOperationStore } from "../../stores/operations";
import { toast } from "../../stores/toasts";
import { openIn } from "../projects/projectActions";
import styles from "./ChangesTab.module.css";
import { DiffView } from "./DiffView";
import { useAsync } from "./hooks";

const kindMark: Record<FileChangeKind, string> = {
  modified: "M",
  added: "A",
  deleted: "D",
  renamed: "R",
  copied: "C",
  type_changed: "T",
  untracked: "N",
  conflicted: "!",
};

export function ChangesTab({ project, signature }: { project: Project; signature: string }) {
  const m = useT();
  const t = m.detail.changes;
  const projectId = project.id;
  const changes = useAsync(() => api.getChanges({ projectId }), [projectId, signature]);
  const files = useMemo(() => changes.data ?? [], [changes.data]);
  const [excluded, setExcluded] = useState<Record<string, true>>({});
  const [activePath, setActivePath] = useState<string | null>(null);
  const [summary, setSummary] = useState("");
  const [description, setDescription] = useState("");
  const [committing, setCommitting] = useState(false);
  const busy = useOperationStore((state) => Boolean(state.activity[projectId]));

  useEffect(() => {
    if (files.length === 0) setActivePath(null);
    else if (!activePath || !files.some((file) => file.path === activePath)) setActivePath(files[0]!.path);
  }, [files, activePath]);

  const active = files.find((file) => file.path === activePath) ?? null;
  const diff = useAsync(
    () => (activePath ? api.getDiff({ projectId, path: activePath, source: { kind: "worktree" } }) : Promise.resolve(null)),
    [projectId, activePath, signature, active?.additions, active?.deletions],
  );

  const committable = files.filter((file) => file.kind !== "conflicted");
  const selected = committable.filter((file) => !excluded[file.path]);
  const allSelected = committable.length > 0 && selected.length === committable.length;
  const status = project.status;
  const blocked = status?.state === "conflicted" || status?.state === "in_progress";
  const canCommit = selected.length > 0 && summary.trim() !== "" && !committing && !busy && !blocked;

  const toggle = (path: string) =>
    setExcluded((previous) => {
      const next = { ...previous };
      if (next[path]) delete next[path];
      else next[path] = true;
      return next;
    });

  const commit = async () => {
    if (!canCommit) return;
    setCommitting(true);
    const message = description.trim() ? `${summary.trim()}\n\n${description.trim()}` : summary.trim();
    const paths = allSelected ? null : selected.flatMap((file) => (file.oldPath ? [file.path, file.oldPath] : [file.path]));
    const result = await runAndWait({ kind: "commit", items: [{ itemId: projectId, projectId, message, paths }] });
    setCommitting(false);
    if (result?.items[0]?.state === "succeeded") {
      setSummary("");
      setDescription("");
      setExcluded({});
    }
  };

  const discard = async (targets: ChangedFile[]) => {
    if (targets.length === 0) return;
    const hasUntracked = targets.some((file) => file.kind === "untracked");
    const result = await confirm({
      title: m.confirm.discard.title,
      body: m.confirm.discard.body(targets.length),
      facts: [
        { label: m.confirm.project, value: project.name },
        {
          label: m.confirm.impact,
          value: (
            <span className="mono selectable">
              {targets.slice(0, 6).map((file) => file.path).join("\n")}
              {targets.length > 6 ? `\n… +${targets.length - 6}` : ""}
            </span>
          ),
        },
      ],
      risk: m.confirm.discard.risk,
      level: "danger",
      confirmLabel: m.confirm.discard.submit,
      alternative: { label: m.confirm.discard.stashInstead, value: "stash" },
      options: hasUntracked ? [{ id: "untracked", label: m.confirm.discard.includeUntracked, defaultChecked: false }] : undefined,
    });
    try {
      if (result.choice === "stash") {
        await api.stashSave({ projectId, message: null, includeUntracked: true });
        toast.success(m.confirm.stash.done);
      } else if (result.confirmed) {
        await api.discardChanges({
          projectId,
          paths: targets.map((file) => file.path),
          includeUntracked: result.options.untracked ?? false,
        });
        toast.success(m.confirm.discard.done);
      } else {
        return;
      }
      changes.reload();
    } catch (error) {
      notifyError(error);
    }
  };

  const stash = async () => {
    const result = await confirm({
      title: m.confirm.stash.title,
      body: m.confirm.stash.body,
      level: "safe",
      confirmLabel: m.confirm.stash.submit,
      options: [{ id: "untracked", label: m.confirm.stash.includeUntracked, defaultChecked: true }],
    });
    if (!result.confirmed) return;
    try {
      await api.stashSave({ projectId, message: null, includeUntracked: result.options.untracked ?? true });
      toast.success(m.confirm.stash.done);
      changes.reload();
    } catch (error) {
      notifyError(error);
    }
  };

  if (changes.error && files.length === 0) {
    return (
      <div className={styles.pad}>
        <ErrorNote error={changes.error} actions={<Button size="sm" onClick={changes.reload}>{m.common.retry}</Button>} />
      </div>
    );
  }
  if (changes.loading && changes.data === null) {
    return (
      <div className={styles.loading}>
        <Spinner size={16} />
      </div>
    );
  }
  if (files.length === 0) {
    return <EmptyState compact icon={<Icons.circleCheck size={20} />} title={t.empty} body={t.emptyBody} />;
  }

  return (
    <div className={styles.changes}>
      <div className={styles.side}>
        <div className={styles.listHead}>
          <Checkbox
            checked={allSelected ? true : selected.length === 0 ? false : "indeterminate"}
            onChange={(checked) => setExcluded(checked ? {} : Object.fromEntries(committable.map((file) => [file.path, true as const])))}
            ariaLabel={t.selectAll}
          />
          <span className="tabular">{t.selected(selected.length, committable.length)}</span>
          <span className={styles.grow} />
          <Menu>
            <MenuTrigger asChild>
              <IconButton label={m.common.more} size="sm" noTooltip>
                <Icons.ellipsis size={15} />
              </IconButton>
            </MenuTrigger>
            <MenuContent>
              <MenuItem icon={<Icons.archive size={14} />} onSelect={() => void stash()}>
                {t.stash}
              </MenuItem>
              <MenuItem icon={<Icons.undo size={14} />} danger disabled={selected.length === 0} onSelect={() => void discard(selected)}>
                {t.discardSelected}
              </MenuItem>
            </MenuContent>
          </Menu>
        </div>
        <ul className={styles.files} role="listbox" aria-label={t.title}>
          {files.map((file) => {
            const { dir, name } = splitFilePath(file.path);
            const isActive = file.path === activePath;
            return (
              <li
                key={file.path}
                role="option"
                aria-selected={isActive}
                className={styles.file}
                data-active={isActive || undefined}
                onClick={() => setActivePath(file.path)}
              >
                <span onClick={(event) => event.stopPropagation()} className={styles.fileCheck}>
                  <Checkbox
                    checked={file.kind !== "conflicted" && !excluded[file.path]}
                    disabled={file.kind === "conflicted"}
                    onChange={() => toggle(file.path)}
                    ariaLabel={file.path}
                  />
                </span>
                <Tooltip label={t.kind[file.kind]} side="top">
                  <span className={styles.mark} data-kind={file.kind}>
                    {kindMark[file.kind]}
                  </span>
                </Tooltip>
                <span className={clsx(styles.fileName, "truncate")} title={file.path}>
                  <span className={styles.dir}>{dir}</span>
                  {name}
                </span>
                {(file.additions !== null || file.deletions !== null) && !file.binary && (
                  <span className={clsx(styles.stats, "tabular")}>
                    {file.additions ? <span className={styles.add}>+{file.additions}</span> : null}
                    {file.deletions ? <span className={styles.del}>−{file.deletions}</span> : null}
                  </span>
                )}
              </li>
            );
          })}
        </ul>
        <div className={styles.commit}>
          <TextField
            value={summary}
            onChange={(event) => setSummary(event.target.value)}
            placeholder={t.summaryPlaceholder}
            aria-label={t.summaryPlaceholder}
            disabled={blocked}
            onKeyDown={(event) => {
              if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) void commit();
            }}
          />
          <TextArea
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder={t.descriptionPlaceholder}
            aria-label={t.descriptionPlaceholder}
            rows={2}
            disabled={blocked}
          />
          <Button variant="primary" block disabled={!canCommit} loading={committing} onClick={() => void commit()}>
            {t.commit(selected.length)}
          </Button>
          {status?.branch && <p className={styles.commitNote}>{t.commitTo(status.branch)}</p>}
        </div>
      </div>

      <div className={styles.diffPane}>
        {active && (
          <div className={styles.diffHead}>
            <span className={styles.mark} data-kind={active.kind}>
              {kindMark[active.kind]}
            </span>
            <span className={clsx("mono selectable truncate", styles.diffPath)}>{active.path}</span>
            <span className={styles.grow} />
            {active.kind !== "deleted" && (
              <IconButton label={t.openFile} size="sm" onClick={() => void openIn(projectId, "editor", active.path)}>
                <Icons.code size={14} />
              </IconButton>
            )}
            {active.kind !== "conflicted" && (
              <IconButton label={t.discard} size="sm" onClick={() => void discard([active])}>
                <Icons.undo size={14} />
              </IconButton>
            )}
          </div>
        )}
        <DiffView diff={diff.data} loading={diff.loading} error={diff.error} placeholder={t.pickFile} />
      </div>
    </div>
  );
}
