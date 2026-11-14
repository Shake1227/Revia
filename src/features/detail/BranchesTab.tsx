import clsx from "clsx";
import { useMemo, useState } from "react";
import { Button, IconButton } from "../../components/Button";
import { Checkbox } from "../../components/Checkbox";
import { Dialog } from "../../components/Dialog";
import { Field, TextField } from "../../components/Field";
import { Icons } from "../../components/icons";
import { Menu, MenuContent, MenuItem, MenuSeparator, MenuTrigger } from "../../components/Menu";
import { Select } from "../../components/Select";
import { Spinner } from "../../components/Spinner";
import { Tooltip } from "../../components/Tooltip";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import { toAppError } from "../../ipc/errors";
import type { AppError, BranchInfo, Project, ProjectDetail, StashEntry } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { relativeTime } from "../../lib/time";
import { confirm } from "../../stores/confirm";
import { toast } from "../../stores/toasts";
import { useUiStore } from "../../stores/ui";
import { remoteLabel } from "../projects/model";
import styles from "./BranchesTab.module.css";

interface BranchesTabProps {
  project: Project;
  detail: ProjectDetail | null;
  loading: boolean;
  onDetail: (detail: ProjectDetail) => void;
}

type BranchDialog = { type: "create"; from: string | null } | { type: "rename"; branch: BranchInfo } | { type: "upstream"; branch: BranchInfo } | null;

export function BranchesTab({ project, detail, loading, onDetail }: BranchesTabProps) {
  const m = useT();
  const t = m.branches;
  const projectId = project.id;
  const openDialog = useUiStore((state) => state.openDialog);
  const [dialog, setDialog] = useState<BranchDialog>(null);
  const [pending, setPending] = useState<string | null>(null);
  const branches = detail?.branches ?? [];
  const local = branches.filter((branch) => branch.kind === "local");
  const remote = branches.filter((branch) => branch.kind === "remote");
  const current = project.status?.branch ?? null;

  const act = async (key: string, action: () => Promise<ProjectDetail>, done?: string) => {
    setPending(key);
    try {
      onDetail(await action());
      if (done) toast.success(done);
      return true;
    } catch (error) {
      const appError = toAppError(error);
      if (appError.code === "merge_conflict") {
        toast.warning(m.error.merge_conflict.title, { description: m.error.merge_conflict.hint });
        useUiStore.getState().openProject(projectId, "changes");
      } else {
        notifyError(appError);
      }
      return false;
    } finally {
      setPending(null);
    }
  };

  const switchTo = async (branch: BranchInfo) => {
    const dirty = project.status?.changes.total ?? 0;
    const target = branch.kind === "remote" ? branch.shortName : branch.name;
    let ifDirty: "abort" | "carry" | "stash" = "abort";
    if (dirty > 0) {
      const result = await confirm({
        title: t.dirtySwitch.title,
        body: t.dirtySwitch.body(dirty, target),
        facts: [{ label: m.confirm.project, value: project.name }],
        risk: m.common.sentences(`${t.dirtySwitch.carry}: ${t.dirtySwitch.carryHint}`, `${t.dirtySwitch.stash}: ${t.dirtySwitch.stashHint}`),
        level: "caution",
        confirmLabel: t.dirtySwitch.carry,
        alternative: { label: t.dirtySwitch.stash, value: "stash" },
      });
      if (result.confirmed) ifDirty = "carry";
      else if (result.choice === "stash") ifDirty = "stash";
      else return;
    }
    await act(branch.name, () => api.checkoutBranch({ projectId, branch: branch.name, ifDirty }), t.switched(target));
  };

  const remove = async (branch: BranchInfo) => {
    const d = m.confirm.deleteBranch;
    const merged = branch.mergedIntoCurrent;
    const result = await confirm({
      title: d.title,
      body: d.body(branch.name),
      facts: [
        { label: m.confirm.project, value: project.name },
        { label: m.confirm.branch, value: <span className="mono">{branch.name}</span> },
      ],
      risk: merged ? d.mergedNote : d.unmergedRisk,
      level: merged ? "caution" : "danger",
      confirmLabel: merged ? d.submit : d.forceSubmit,
      acknowledge: !merged,
    });
    if (!result.confirmed) return;
    await act(branch.name, () => api.deleteBranch({ projectId, name: branch.name, force: !merged }), d.done(branch.name));
  };

  const removeRemote = async (branch: BranchInfo) => {
    if (!branch.remote) return;
    const d = m.confirm.deleteRemoteBranch;
    const result = await confirm({
      title: d.title,
      body: d.body(branch.name),
      facts: [
        { label: m.confirm.project, value: project.name },
        { label: m.confirm.branch, value: <span className="mono">{branch.name}</span> },
      ],
      risk: d.risk,
      level: "danger",
      confirmLabel: d.submit,
      acknowledge: true,
    });
    if (!result.confirmed) return;
    await act(branch.name, () => api.deleteRemoteBranch({ projectId, remote: branch.remote!, branch: branch.shortName }), d.done(branch.name));
  };

  const applyStash = (stash: StashEntry) => act(stash.sha, () => api.stashApply({ projectId, stashSha: stash.sha, drop: true }), m.confirm.stash.applied);
  const dropStash = async (stash: StashEntry) => {
    const result = await confirm({
      title: m.confirm.stash.dropTitle,
      body: m.confirm.stash.dropBody,
      facts: [{ label: m.confirm.impact, value: stash.message }],
      level: "danger",
      confirmLabel: m.confirm.stash.dropSubmit,
    });
    if (result.confirmed) await act(stash.sha, () => api.stashDrop({ projectId, stashSha: stash.sha }));
  };

  const row = (branch: BranchInfo) => {
    const isCurrent = branch.current;
    const isRemote = branch.kind === "remote";
    return (
      <li key={`${branch.kind}:${branch.name}`} className={styles.row} data-current={isCurrent || undefined}>
        <span className={styles.indicator}>{isCurrent ? <Icons.check size={13} strokeWidth={2.2} /> : pending === branch.name ? <Spinner size={12} /> : null}</span>
        <div className={styles.main}>
          <div className={styles.head}>
            <span className={clsx(styles.name, "mono truncate")}>{branch.name}</span>
            {isCurrent && <span className={styles.tag}>{t.current}</span>}
            {!isCurrent && !isRemote && branch.mergedIntoCurrent && <span className={styles.tagMuted}>{t.merged}</span>}
            {branch.upstream && (
              <span className={clsx(styles.upstream, "mono truncate")}>
                → {remoteLabel(branch.upstream)}
                {(branch.ahead ?? 0) + (branch.behind ?? 0) > 0 && <span className={styles.track}> {t.aheadBehind(branch.ahead ?? 0, branch.behind ?? 0)}</span>}
              </span>
            )}
            {branch.upstreamGone && <span className={styles.gone}>{t.upstreamGone}</span>}
          </div>
          {branch.lastCommit && (
            <p className={clsx(styles.sub, "truncate")}>
              {branch.lastCommit.subject}
              <span>{m.common.dot}{relativeTime(branch.lastCommit.at, m)}</span>
            </p>
          )}
        </div>
        <div className={styles.actions}>
          {!isCurrent && (
            <Tooltip label={isRemote ? t.checkoutRemote : t.switchTo(branch.name)} side="top">
              <Button size="sm" className={styles.hoverOnly} onClick={() => void switchTo(branch)}>
                {t.switch}
              </Button>
            </Tooltip>
          )}
          <Menu>
            <MenuTrigger asChild>
              <IconButton label={m.common.more} size="sm" noTooltip>
                <Icons.ellipsis size={15} />
              </IconButton>
            </MenuTrigger>
            <MenuContent minWidth={230}>
              {!isCurrent && current && (
                <MenuItem icon={<Icons.merge size={14} />} onSelect={() => openDialog({ type: "merge", projectId, source: branch.name, target: current })}>
                  {t.mergeInto(current)}
                </MenuItem>
              )}
              <MenuItem icon={<Icons.branch size={14} />} onSelect={() => setDialog({ type: "create", from: branch.name })}>
                {t.new}…
              </MenuItem>
              {!isRemote && (
                <>
                  <MenuItem icon={<Icons.pencil size={14} />} onSelect={() => setDialog({ type: "rename", branch })}>
                    {t.rename}
                  </MenuItem>
                  <MenuItem icon={<Icons.link size={14} />} onSelect={() => setDialog({ type: "upstream", branch })}>
                    {t.setUpstream}
                  </MenuItem>
                </>
              )}
              <MenuSeparator />
              {isRemote ? (
                <MenuItem icon={<Icons.trash size={14} />} danger onSelect={() => void removeRemote(branch)}>
                  {t.deleteRemote}
                </MenuItem>
              ) : (
                <MenuItem icon={<Icons.trash size={14} />} danger disabled={isCurrent} onSelect={() => void remove(branch)}>
                  {t.delete}
                </MenuItem>
              )}
            </MenuContent>
          </Menu>
        </div>
      </li>
    );
  };

  return (
    <div className={styles.tab}>
      <div className={styles.toolbar}>
        <Button icon={<Icons.plus size={14} />} onClick={() => setDialog({ type: "create", from: current })}>
          {t.new}
        </Button>
        <Button
          icon={<Icons.merge size={14} />}
          disabled={!current || branches.length < 2}
          onClick={() => openDialog({ type: "merge", projectId, source: null, target: current })}
        >
          {m.action.merge}…
        </Button>
        {loading && <Spinner size={14} />}
      </div>

      <section className={styles.section}>
        <h3 className={styles.sectionTitle}>
          {t.local}
          <span className="tabular">{local.length}</span>
        </h3>
        {local.length === 0 ? <p className={styles.empty}>{t.empty}</p> : <ul className={styles.list}>{local.map(row)}</ul>}
      </section>

      {remote.length > 0 && (
        <section className={styles.section}>
          <h3 className={styles.sectionTitle}>
            {t.remote}
            <span className="tabular">{remote.length}</span>
          </h3>
          <ul className={styles.list}>{remote.map(row)}</ul>
        </section>
      )}

      {detail && detail.stashes.length > 0 && (
        <section className={styles.section}>
          <h3 className={styles.sectionTitle}>
            {t.stashes}
            <span className="tabular">{detail.stashes.length}</span>
          </h3>
          <ul className={styles.list}>
            {detail.stashes.map((stash) => (
              <li key={stash.sha} className={styles.row}>
                <span className={styles.indicator}>{pending === stash.sha ? <Spinner size={12} /> : <Icons.archive size={13} />}</span>
                <div className={styles.main}>
                  <span className={clsx(styles.name, "truncate")}>{stash.message}</span>
                  <p className={styles.sub}>
                    {stash.branch && <span className="mono">{stash.branch}</span>}
                    <span>{m.common.dot}{relativeTime(stash.at, m)}</span>
                  </p>
                </div>
                <div className={styles.actions}>
                  <Button size="sm" onClick={() => void applyStash(stash)}>
                    {t.stashApply}
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => void dropStash(stash)}>
                    {t.stashDrop}
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        </section>
      )}

      <CreateBranchDialog
        open={dialog?.type === "create"}
        from={dialog?.type === "create" ? dialog.from : null}
        branches={branches}
        onClose={() => setDialog(null)}
        onSubmit={(name, startPoint, checkout) =>
          act("create", () => api.createBranch({ projectId, name, startPoint, checkout }), t.create.created(name))
        }
      />
      <RenameBranchDialog
        branch={dialog?.type === "rename" ? dialog.branch : null}
        onClose={() => setDialog(null)}
        onSubmit={(from, to) => act(from, () => api.renameBranch({ projectId, from, to }))}
      />
      <UpstreamDialog
        branch={dialog?.type === "upstream" ? dialog.branch : null}
        remotes={remote}
        onClose={() => setDialog(null)}
        onSubmit={(branch, upstream) => act(branch, () => api.setUpstream({ projectId, branch, upstream }))}
      />
    </div>
  );
}

interface CreateProps {
  open: boolean;
  from: string | null;
  branches: BranchInfo[];
  onClose: () => void;
  onSubmit: (name: string, startPoint: string | null, checkout: boolean) => Promise<boolean>;
}

function CreateBranchDialog({ open, from, branches, onClose, onSubmit }: CreateProps) {
  const m = useT();
  const t = m.branches.create;
  const [name, setName] = useState("");
  const [start, setStart] = useState<string | null>(null);
  const [checkout, setCheckout] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [seenOpen, setSeenOpen] = useState(false);
  if (open !== seenOpen) {
    setSeenOpen(open);
    if (open) {
      setName("");
      setStart(from);
      setCheckout(true);
      setError(null);
    }
  }
  const trimmed = name.trim();
  const duplicate = branches.some((branch) => branch.kind === "local" && branch.name === trimmed);
  const invalid = trimmed !== "" && (/[\s~^:?*[\\]/.test(trimmed) || trimmed.startsWith("-") || trimmed.endsWith("/") || trimmed.includes(".."));
  const options = useMemo(
    () => branches.map((branch) => ({ value: branch.name, label: branch.name, group: branch.kind === "local" ? m.branches.local : m.branches.remote })),
    [branches, m],
  );
  const submit = async () => {
    if (!trimmed || duplicate || invalid || busy) return;
    setBusy(true);
    setError(null);
    const ok = await onSubmit(trimmed, start, checkout);
    setBusy(false);
    if (ok) onClose();
  };
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={t.title}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant="primary" disabled={!trimmed || duplicate || invalid} loading={busy} onClick={() => void submit()}>
            {t.submit}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <Field label={t.name} error={duplicate ? m.error.branch_exists.title : invalid ? m.error.invalid_ref_name.hint : error ? m.error[error.code].title : undefined}>
          <TextField
            mono
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={t.namePlaceholder}
            invalid={duplicate || invalid}
            onKeyDown={(event) => event.key === "Enter" && void submit()}
          />
        </Field>
        {options.length > 0 && (
          <Field label={t.from}>
            <Select mono value={start ?? undefined} onChange={setStart} options={options} leading={<Icons.branch size={13} />} />
          </Field>
        )}
        <Checkbox checked={checkout} onChange={setCheckout} label={t.checkout} />
      </div>
    </Dialog>
  );
}

function RenameBranchDialog({ branch, onClose, onSubmit }: { branch: BranchInfo | null; onClose: () => void; onSubmit: (from: string, to: string) => Promise<boolean> }) {
  const m = useT();
  const t = m.branches.renameDialog;
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [shown, setShown] = useState<BranchInfo | null>(null);
  if (branch && branch !== shown) {
    setShown(branch);
    setName(branch.name);
  }
  const target = branch ?? shown;
  const trimmed = name.trim();
  const submit = async () => {
    if (!target || !trimmed || trimmed === target.name) return;
    setBusy(true);
    const ok = await onSubmit(target.name, trimmed);
    setBusy(false);
    if (ok) onClose();
  };
  return (
    <Dialog
      open={branch !== null}
      onOpenChange={(next) => !next && onClose()}
      title={t.title}
      description={t.note}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant="primary" disabled={!trimmed || trimmed === target?.name} loading={busy} onClick={() => void submit()}>
            {t.submit}
          </Button>
        </>
      }
    >
      <Field label={t.newName}>
        <TextField mono autoFocus value={name} onChange={(event) => setName(event.target.value)} onKeyDown={(event) => event.key === "Enter" && void submit()} />
      </Field>
    </Dialog>
  );
}

interface UpstreamProps {
  branch: BranchInfo | null;
  remotes: BranchInfo[];
  onClose: () => void;
  onSubmit: (branch: string, upstream: { remote: string; branch: string } | null) => Promise<boolean>;
}

function UpstreamDialog({ branch, remotes, onClose, onSubmit }: UpstreamProps) {
  const m = useT();
  const t = m.branches.upstreamDialog;
  const [value, setValue] = useState("__none__");
  const [busy, setBusy] = useState(false);
  const [shown, setShown] = useState<BranchInfo | null>(null);
  if (branch && branch !== shown) {
    setShown(branch);
    setValue(branch.upstream ? remoteLabel(branch.upstream) : "__none__");
  }
  const target = branch ?? shown;
  const options = [{ value: "__none__", label: t.none }, ...remotes.map((remote) => ({ value: remote.name, label: remote.name }))];
  const submit = async () => {
    if (!target) return;
    const selected = remotes.find((remote) => remote.name === value);
    setBusy(true);
    const ok = await onSubmit(target.name, selected?.remote ? { remote: selected.remote, branch: selected.shortName } : null);
    setBusy(false);
    if (ok) onClose();
  };
  return (
    <Dialog
      open={branch !== null}
      onOpenChange={(next) => !next && onClose()}
      title={t.title}
      description={t.body}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant="primary" loading={busy} onClick={() => void submit()}>
            {t.submit}
          </Button>
        </>
      }
    >
      <Field label={`${target?.name ?? ""} → ${m.branches.upstream}`}>
        <Select mono value={value} onChange={setValue} options={options} />
      </Field>
    </Dialog>
  );
}
