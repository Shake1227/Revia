import { useEffect, useState, type ReactNode } from "react";
import { Button } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import { ErrorNote, Note } from "../../components/ErrorNote";
import { Field, TextField } from "../../components/Field";
import { Icons } from "../../components/icons";
import { Select } from "../../components/Select";
import { Spinner } from "../../components/Spinner";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import { toAppError } from "../../ipc/errors";
import type { AppError, Project, RemovalPreview, RepositoryDeletionCheck } from "../../ipc/types";
import { displayPath } from "../../lib/path";
import { useAppStore } from "../../stores/app";
import { useGithubStore } from "../../stores/github";
import { useProjectStore } from "../../stores/projects";
import { toast } from "../../stores/toasts";
import { useUiStore } from "../../stores/ui";
import styles from "./dialogs.module.css";
import { LoginProgress } from "./LoginDialog";

function usePreview(projectId: string | null) {
  const [preview, setPreview] = useState<RemovalPreview | null>(null);
  const [error, setError] = useState<AppError | null>(null);

  useEffect(() => {
    setPreview(null);
    setError(null);
    if (!projectId) return;
    let stale = false;
    api
      .previewRemoval({ projectId })
      .then((value) => !stale && setPreview(value))
      .catch((reason: unknown) => !stale && setError(toAppError(reason)));
    return () => {
      stale = true;
    };
  }, [projectId]);

  return { preview, error };
}

function LocalOnly({ preview }: { preview: RemovalPreview }) {
  const m = useT();
  const t = m.removal.lose;
  const lines: string[] = [];
  if (preview.uncommittedFiles > 0) lines.push(t.uncommitted(preview.uncommittedFiles));
  if (preview.unpushedCommits === null) lines.push(t.unpushedUnknown);
  else if (preview.unpushedCommits > 0) lines.push(t.unpushed(preview.unpushedCommits));
  if (preview.localOnlyBranches.length > 0) lines.push(t.branches(preview.localOnlyBranches.join(m.common.dot)));
  if (preview.stashes > 0) lines.push(t.stashes(preview.stashes));

  if (lines.length === 0) return <p className={styles.hint}>{t.none}</p>;
  return (
    <Note tone="warning">
      <p>{t.title}</p>
      <ul className={styles.loseList}>
        {lines.map((line) => (
          <li key={line}>{line}</li>
        ))}
      </ul>
    </Note>
  );
}

function Choice({
  selected,
  disabled,
  icon,
  title,
  body,
  onSelect,
}: {
  selected: boolean;
  disabled?: boolean;
  icon: ReactNode;
  title: string;
  body: ReactNode;
  onSelect: () => void;
}) {
  return (
    <button type="button" role="radio" aria-checked={selected} className={styles.option} data-selected={selected || undefined} disabled={disabled} onClick={onSelect}>
      <span className={styles.optionIcon}>{icon}</span>
      <span className={styles.optionText}>
        <span className={styles.optionTitle}>{title}</span>
        <p className={styles.optionBody}>{body}</p>
      </span>
    </button>
  );
}

function ConfirmField({ expected, value, onChange }: { expected: string; value: string; onChange: (value: string) => void }) {
  const m = useT();
  return (
    <Field label={m.removal.confirmLabel(expected)}>
      <TextField mono value={value} onChange={(event) => onChange(event.target.value)} autoComplete="off" spellCheck={false} />
    </Field>
  );
}

function leaveIfViewing(projectId: string) {
  const { view, back } = useUiStore.getState();
  if (view.name === "project" && view.projectId === projectId) back();
}

export function UnlinkDialog({ projectId, onClose }: { projectId: string | null; onClose: () => void }) {
  const m = useT();
  const t = m.removal.unlink;
  const project = useProjectStore((state) => (projectId ? state.byId[projectId] : undefined));
  const { preview, error } = usePreview(projectId);
  const [shown, setShown] = useState<Project | null>(null);
  const [mode, setMode] = useState<"remote" | "git">("remote");
  const [remote, setRemote] = useState("");
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<AppError | null>(null);

  useEffect(() => {
    if (project) setShown(project);
  }, [project]);

  useEffect(() => {
    setTyped("");
    setFailure(null);
    setBusy(false);
  }, [projectId]);

  useEffect(() => {
    if (!preview) return;
    const names = preview.remotes.map((item) => item.name);
    setRemote(names.includes("origin") ? "origin" : (names[0] ?? ""));
    setMode(names.length > 0 ? "remote" : "git");
  }, [preview]);

  const target = project ?? shown;
  const name = target?.name ?? "";
  const hasRemote = (preview?.remotes.length ?? 0) > 0;
  const canSubmit = preview !== null && !busy && (mode === "remote" ? hasRemote && remote !== "" : preview.repository && typed === name);

  const submit = async () => {
    if (!projectId || !preview) return;
    setBusy(true);
    setFailure(null);
    try {
      if (mode === "remote") {
        await api.removeRemote({ projectId, name: remote });
        toast.success(t.doneRemote(name, remote));
      } else {
        useProjectStore.getState().upsert(await api.removeGit({ projectId, fingerprint: preview.fingerprint, confirm: typed }));
        toast.success(t.doneGit(name), { description: t.trashHint });
      }
      onClose();
    } catch (reason) {
      setFailure(toAppError(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={projectId !== null}
      onOpenChange={(next) => !next && !busy && onClose()}
      title={t.title(name)}
      size="md"
      tone={mode === "git" ? "danger" : "default"}
      dismissible={!busy}
      footer={
        <>
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant={mode === "git" ? "danger" : "primary"} disabled={!canSubmit} loading={busy} onClick={() => void submit()}>
            {mode === "git" ? t.submitGit : t.submitRemote}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        {error && <ErrorNote error={error} />}
        {!preview && !error && (
          <div className={styles.centered}>
            <Spinner size={16} />
            <span>{m.removal.loading}</span>
          </div>
        )}
        {preview && (
          <>
            <div className={styles.options} role="radiogroup">
              <Choice
                selected={mode === "remote"}
                disabled={!hasRemote || busy}
                icon={<Icons.unlink size={16} />}
                title={t.remoteTitle}
                body={hasRemote ? t.remoteBody : t.remoteNone}
                onSelect={() => setMode("remote")}
              />
              <Choice
                selected={mode === "git"}
                disabled={!preview.repository || busy}
                icon={<Icons.archive size={16} />}
                title={t.gitTitle}
                body={t.gitBody}
                onSelect={() => setMode("git")}
              />
            </div>
            {mode === "remote" && preview.remotes.length > 1 && (
              <Field label={t.remoteLabel}>
                <Select
                  value={remote}
                  onChange={setRemote}
                  options={preview.remotes.map((item) => ({ value: item.name, label: `${item.name}${m.common.dot}${item.fetchUrl}` }))}
                />
              </Field>
            )}
            {mode === "remote" && preview.remotes.length === 1 && (
              <p className={`${styles.pathBox} mono selectable`}>{preview.remotes[0]?.fetchUrl}</p>
            )}
            {mode === "git" && (
              <>
                <LocalOnly preview={preview} />
                <ConfirmField expected={name} value={typed} onChange={setTyped} />
              </>
            )}
            {failure && <ErrorNote error={failure} />}
          </>
        )}
      </div>
    </Dialog>
  );
}

export function DeleteDialog({ projectId, onClose }: { projectId: string | null; onClose: () => void }) {
  const m = useT();
  const t = m.removal.delete;
  const project = useProjectStore((state) => (projectId ? state.byId[projectId] : undefined));
  const login = useGithubStore((state) => state.login);
  const startLogin = useGithubStore((state) => state.startLogin);
  const cancelLogin = useGithubStore((state) => state.cancelLogin);
  const authenticated = useGithubStore((state) => state.status?.authenticated ?? false);
  const { preview, error } = usePreview(projectId);
  const [shown, setShown] = useState<Project | null>(null);
  const [mode, setMode] = useState<"local" | "both">("local");
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<AppError | null>(null);
  const [check, setCheck] = useState<RepositoryDeletionCheck | null>(null);
  const [checkError, setCheckError] = useState<AppError | null>(null);
  const [checkRun, setCheckRun] = useState(0);

  useEffect(() => {
    if (project) setShown(project);
  }, [project]);

  useEffect(() => {
    setMode("local");
    setTyped("");
    setFailure(null);
    setBusy(false);
    setCheck(null);
    setCheckError(null);
  }, [projectId]);

  useEffect(() => {
    setTyped("");
    setFailure(null);
  }, [mode]);

  useEffect(() => {
    if (!projectId || mode !== "both" || login !== null) return;
    let stale = false;
    setCheck(null);
    setCheckError(null);
    api
      .checkRepositoryDeletion({ projectId })
      .then((value) => !stale && setCheck(value))
      .catch((reason: unknown) => !stale && setCheckError(toAppError(reason)));
    return () => {
      stale = true;
    };
  }, [projectId, mode, login, authenticated, checkRun]);

  const target = project ?? shown;
  const name = target?.name ?? "";
  const repo = preview?.github ? `${preview.github.owner}/${preview.github.repo}` : null;
  const nested = preview?.nestedProjects.length ?? 0;
  const expected = mode === "both" && repo ? repo : name;
  const repositoryReady = mode === "local" || (check !== null && check.blocker === null);
  const canSubmit = preview !== null && nested === 0 && !busy && login === null && repositoryReady && typed === expected;

  const close = () => {
    if (login !== null) void cancelLogin();
    onClose();
  };

  const submit = async () => {
    if (!projectId || !preview) return;
    setBusy(true);
    setFailure(null);
    try {
      const result = await api.deleteProject({ projectId, deleteRepository: mode === "both", fingerprint: preview.fingerprint, confirm: typed });
      useAppStore.getState().applyWorkspace(result.workspace);
      if (result.local === "failed") {
        toast.error(t.partial(repo ?? name), { detail: result.error?.detail, duration: 14000 });
      } else {
        leaveIfViewing(projectId);
        toast.success(result.repository === "deleted" ? t.doneBoth(repo ?? name) : t.doneLocal(name));
      }
      onClose();
    } catch (reason) {
      const appError = toAppError(reason);
      setFailure(appError);
      if (appError.code === "scope_missing" || appError.code === "not_authenticated") setCheckRun((value) => value + 1);
    } finally {
      setBusy(false);
    }
  };

  const blocker = check?.blocker ?? null;

  return (
    <Dialog
      open={projectId !== null}
      onOpenChange={(next) => !next && !busy && close()}
      title={t.title(name)}
      size="md"
      tone="danger"
      dismissible={!busy}
      footer={
        <>
          <Button variant="ghost" disabled={busy} onClick={close}>
            {m.common.cancel}
          </Button>
          <Button variant="danger" disabled={!canSubmit} loading={busy} onClick={() => void submit()}>
            {mode === "both" ? t.submitBoth : t.submitLocal}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        {error && <ErrorNote error={error} />}
        {!preview && !error && (
          <div className={styles.centered}>
            <Spinner size={16} />
            <span>{m.removal.loading}</span>
          </div>
        )}
        {preview && (
          <>
            <p className={`${styles.pathBox} mono selectable`}>{displayPath(preview.path)}</p>
            {nested > 0 ? (
              <Note tone="warning">{t.nested(nested)}</Note>
            ) : (
              <>
                <div className={styles.options} role="radiogroup">
                  <Choice
                    selected={mode === "local"}
                    disabled={busy}
                    icon={<Icons.trash size={16} />}
                    title={t.localTitle}
                    body={repo ? `${t.localBody}${t.localKeepsRepo(repo)}` : t.localBody}
                    onSelect={() => setMode("local")}
                  />
                  {repo && (
                    <Choice
                      selected={mode === "both"}
                      disabled={busy}
                      icon={<Icons.github size={16} />}
                      title={t.bothTitle}
                      body={`${t.bothBody(repo)}${t.irreversible}`}
                      onSelect={() => setMode("both")}
                    />
                  )}
                </div>

                {mode === "both" && login !== null && <LoginProgress onRetry={() => void startLogin(["delete_repo"])} />}
                {mode === "both" && login === null && (
                  <>
                    {!check && !checkError && (
                      <p className={styles.waiting}>
                        <Spinner size={13} />
                        {t.checking}
                      </p>
                    )}
                    {checkError && (
                      <ErrorNote
                        error={checkError}
                        actions={
                          <Button size="sm" onClick={() => setCheckRun((value) => value + 1)}>
                            {m.common.retry}
                          </Button>
                        }
                      />
                    )}
                    {check && blocker && (
                      <Note tone="warning">
                        {t.blocker[blocker]}
                        {(blocker === "scope_missing" || blocker === "not_authenticated") && (
                          <div className={styles.noteAction}>
                            <Button size="sm" icon={<Icons.github size={13} />} onClick={() => void startLogin(blocker === "scope_missing" ? ["delete_repo"] : undefined)}>
                              {blocker === "scope_missing" ? t.addScope : m.github.connect.action}
                            </Button>
                          </div>
                        )}
                      </Note>
                    )}
                    {check && !blocker && (
                      <p className={styles.hint}>
                        {[check.nameWithOwner, check.visibility ? t.visibility[check.visibility] : null, check.isFork ? t.fork : null]
                          .filter((part): part is string => Boolean(part))
                          .join(m.common.dot)}
                      </p>
                    )}
                  </>
                )}

                <LocalOnly preview={preview} />
                {repositoryReady && login === null && <ConfirmField expected={expected} value={typed} onChange={setTyped} />}
                {failure && <ErrorNote error={failure} />}
              </>
            )}
          </>
        )}
      </div>
    </Dialog>
  );
}
