import { useEffect, useMemo, useState } from "react";
import { Button } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import { ErrorNote, Note } from "../../components/ErrorNote";
import { Field, SearchField, TextField } from "../../components/Field";
import { Icons } from "../../components/icons";
import { Select } from "../../components/Select";
import { Spinner } from "../../components/Spinner";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import type { MergeMode, PullMode } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { useGithubStore } from "../../stores/github";
import { useOperationStore } from "../../stores/operations";
import { useProjectStore } from "../../stores/projects";
import { toast } from "../../stores/toasts";
import { useAsync } from "../detail/hooks";
import { TargetEditor } from "../detail/TargetEditor";
import styles from "./dialogs.module.css";

export function BatchMergeDialog({ projectIds, onClose }: { projectIds: string[] | null; onClose: () => void }) {
  const m = useT();
  const prepare = useOperationStore((state) => state.prepare);
  const byId = useProjectStore((state) => state.byId);
  const [source, setSource] = useState("");
  const [target, setTarget] = useState("");
  const [mode, setMode] = useState<MergeMode>("default");

  useEffect(() => {
    if (!projectIds) return;
    const counts = new Map<string, number>();
    for (const id of projectIds) {
      const branch = byId[id]?.status?.branch;
      if (branch) counts.set(branch, (counts.get(branch) ?? 0) + 1);
    }
    setTarget([...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? "main");
    setSource("");
    setMode("default");
  }, [projectIds]);

  const valid = source.trim() !== "" && target.trim() !== "" && source.trim() !== target.trim();
  const submit = () => {
    if (!projectIds || !valid) return;
    onClose();
    void prepare({
      kind: "merge",
      items: projectIds.map((id) => ({ itemId: id, projectId: id, source: source.trim(), target: target.trim(), mode })),
    });
  };

  return (
    <Dialog
      open={projectIds !== null}
      onOpenChange={(next) => !next && onClose()}
      title={m.ops.title.merge(projectIds?.length ?? 0)}
      description={m.merge.conflictNote}
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant="primary" disabled={!valid} onClick={submit}>
            {m.github.publish.review}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <div className={styles.columns}>
          <Field label={m.merge.source}>
            <TextField mono autoFocus value={source} onChange={(event) => setSource(event.target.value)} placeholder="develop" leading={<Icons.branch size={13} />} />
          </Field>
          <Field label={m.merge.target}>
            <TextField mono value={target} onChange={(event) => setTarget(event.target.value)} leading={<Icons.branch size={13} />} onKeyDown={(event) => event.key === "Enter" && submit()} />
          </Field>
        </div>
        <Field label={m.merge.mode.label} hint={m.merge.mode[`${mode}Hint` as const]}>
          <Select value={mode} onChange={(value: MergeMode) => setMode(value)} options={(["default", "no_ff", "ff_only"] as const).map((value) => ({ value, label: m.merge.mode[value] }))} />
        </Field>
      </div>
    </Dialog>
  );
}

export function DivergedDialog({ projectId, onClose }: { projectId: string | null; onClose: () => void }) {
  const m = useT();
  const [shownId, setShownId] = useState<string | null>(null);
  const id = projectId ?? shownId;
  const project = useProjectStore((state) => (id ? state.byId[id] : undefined));
  const prepare = useOperationStore((state) => state.prepare);
  const [mode, setMode] = useState<Exclude<PullMode, "ff_only">>("merge");

  useEffect(() => {
    if (!projectId) return;
    setShownId(projectId);
    const preferred = useProjectStore.getState().byId[projectId]?.settings.pullMode;
    setMode(preferred === "rebase" ? "rebase" : "merge");
  }, [projectId]);

  const status = project?.status;
  const submit = () => {
    if (!id) return;
    onClose();
    void prepare({ kind: "pull", items: [{ itemId: id, projectId: id, mode }] });
  };
  const options: { value: Exclude<PullMode, "ff_only">; title: string; body: string }[] = [
    { value: "merge", title: m.settingsProject.pullModes.merge, body: m.merge.mode.no_ffHint },
    { value: "rebase", title: m.settingsProject.pullModes.rebase, body: m.settingsProject.rebaseHint },
  ];

  return (
    <Dialog
      open={projectId !== null}
      onOpenChange={(next) => !next && onClose()}
      title={status ? m.headline.diverged(status.ahead ?? 0, status.behind ?? 0) : m.state.diverged}
      description={`${project?.name ?? ""} — ${m.guidance.diverged}`}
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant="primary" onClick={submit}>
            {m.github.publish.review}
          </Button>
        </>
      }
    >
      <div className={styles.options} role="radiogroup">
        {options.map((option) => (
          <button key={option.value} type="button" role="radio" aria-checked={mode === option.value} className={styles.option} data-selected={mode === option.value || undefined} onClick={() => setMode(option.value)}>
            <span className={styles.optionIcon}>{option.value === "merge" ? <Icons.merge size={17} /> : <Icons.arrowDownUp size={17} />}</span>
            <span className={styles.optionText}>
              <span className={styles.optionTitle}>{option.title}</span>
              <p className={styles.optionBody}>{option.body}</p>
            </span>
          </button>
        ))}
        {(status?.changes.total ?? 0) > 0 && <Note tone="warning">{m.merge.dirtyBlock}</Note>}
      </div>
    </Dialog>
  );
}

export function TargetDialog({ projectId, onClose }: { projectId: string | null; onClose: () => void }) {
  const m = useT();
  const [shownId, setShownId] = useState<string | null>(null);
  const id = projectId ?? shownId;
  const project = useProjectStore((state) => (id ? state.byId[id] : undefined));
  useEffect(() => {
    if (projectId) setShownId(projectId);
  }, [projectId]);
  const detail = useAsync(() => (projectId ? api.getProjectDetail({ projectId }) : Promise.resolve(null)), [projectId]);

  return (
    <Dialog
      open={projectId !== null}
      onOpenChange={(next) => !next && onClose()}
      title={m.settingsProject.targets}
      description={project?.status?.branch ? `${project.name} — ${m.settingsProject.targetsHint(project.status.branch)}` : project?.name}
      size="md"
      footer={
        <Button variant="primary" onClick={onClose}>
          {m.common.done}
        </Button>
      }
    >
      {project && detail.data ? (
        <TargetEditor project={project} branches={detail.data.branches} />
      ) : detail.error ? (
        <ErrorNote error={detail.error} />
      ) : (
        <div className={styles.centered}>
          <Spinner size={16} />
        </div>
      )}
    </Dialog>
  );
}

export function LinkDialog({ projectId, onClose }: { projectId: string | null; onClose: () => void }) {
  const m = useT();
  const t = m.github.link;
  const status = useGithubStore((state) => state.status);
  const repos = useGithubStore((state) => state.repos);
  const reposLoading = useGithubStore((state) => state.reposLoading);
  const loadRepos = useGithubStore((state) => state.loadRepos);
  const [query, setQuery] = useState("");
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!projectId) return;
    setQuery("");
    setUrl("");
    if (status?.authenticated && repos === null) void loadRepos();
  }, [projectId]);

  const matches = useMemo(() => {
    const words = query.trim().toLowerCase();
    return (repos ?? []).filter((repo) => repo.nameWithOwner.toLowerCase().includes(words)).slice(0, 40);
  }, [repos, query]);

  const submit = async () => {
    if (!projectId || url.trim() === "") return;
    setBusy(true);
    try {
      const detail = await api.addRemote({ projectId, name: "origin", url: url.trim() });
      useProjectStore.getState().upsert(detail.project);
      toast.success(t.done);
      onClose();
    } catch (error) {
      notifyError(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={projectId !== null}
      onOpenChange={(next) => !next && onClose()}
      title={t.title}
      description={t.body}
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant="primary" disabled={url.trim() === ""} loading={busy} onClick={() => void submit()}>
            {t.submit}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        {status?.authenticated && (
          <Field label={t.pick}>
            <SearchField value={query} onChange={setQuery} placeholder={m.github.repos.searchPlaceholder} clearLabel={m.common.close} />
            <div className={styles.repoList}>
              {reposLoading && repos === null ? (
                <div className={styles.centered}>
                  <Spinner size={15} />
                </div>
              ) : matches.length === 0 ? (
                <p className={styles.hint} style={{ padding: 10 }}>
                  {m.github.repos.noMatch}
                </p>
              ) : (
                matches.map((repo) => (
                  <button key={repo.id} type="button" className={styles.repoItem} data-selected={url === repo.cloneUrl || undefined} onClick={() => setUrl(repo.cloneUrl)}>
                    {repo.visibility === "public" ? <Icons.globe size={13} /> : <Icons.lock size={13} />}
                    <span className="truncate">{repo.nameWithOwner}</span>
                  </button>
                ))
              )}
            </div>
          </Field>
        )}
        <Field label={status?.authenticated ? t.orUrl : m.settingsProject.remoteUrl}>
          <TextField mono value={url} onChange={(event) => setUrl(event.target.value)} placeholder={m.settingsProject.remoteUrlPlaceholder} />
        </Field>
      </div>
    </Dialog>
  );
}
