import clsx from "clsx";
import { useState, type ReactNode } from "react";
import { Button, IconButton } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import { Field, TextField } from "../../components/Field";
import { Icons } from "../../components/icons";
import { Select } from "../../components/Select";
import { Switch } from "../../components/Switch";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import type { Project, ProjectDetail, ProjectSettingsPatch, PullMode, RemoteInfo } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { confirm } from "../../stores/confirm";
import { useProjectStore } from "../../stores/projects";
import { useUiStore } from "../../stores/ui";
import { excludeProject } from "../projects/projectActions";
import { forcePush, rebaseOnto } from "./advanced";
import styles from "./SettingsTab.module.css";
import { TargetEditor } from "./TargetEditor";

interface SettingsTabProps {
  project: Project;
  detail: ProjectDetail | null;
  onDetail: (detail: ProjectDetail) => void;
}

export function Section({ title, hint, children, tone }: { title: string; hint?: ReactNode; children: ReactNode; tone?: "danger" }) {
  return (
    <section className={styles.section} data-tone={tone}>
      <header className={styles.sectionHead}>
        <h3>{title}</h3>
        {hint && <p>{hint}</p>}
      </header>
      <div className={styles.sectionBody}>{children}</div>
    </section>
  );
}

export function Row({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children?: ReactNode }) {
  return (
    <div className={styles.row}>
      <div className={styles.rowText}>
        <p className={styles.rowLabel}>{label}</p>
        {hint && <p className={styles.rowHint}>{hint}</p>}
      </div>
      {children && <div className={styles.rowControl}>{children}</div>}
    </div>
  );
}

export function SettingsTab({ project, detail, onDetail }: SettingsTabProps) {
  const m = useT();
  const t = m.settingsProject;
  const projectId = project.id;
  const openDialog = useUiStore((state) => state.openDialog);
  const [remoteDialog, setRemoteDialog] = useState<{ remote: RemoteInfo | null } | null>(null);
  const [rebaseOpen, setRebaseOpen] = useState(false);
  const status = project.status;
  const remotes = status?.remotes ?? [];
  const branches = detail?.branches ?? [];
  const isRepo = status !== null && !["not_initialized", "missing"].includes(status.state);

  const patch = async (value: ProjectSettingsPatch) => {
    try {
      useProjectStore.getState().upsert(await api.updateProjectSettings({ projectId, patch: value }));
    } catch (error) {
      notifyError(error);
    }
  };

  const removeRemote = async (remote: RemoteInfo) => {
    const d = m.confirm.removeRemote;
    const result = await confirm({
      title: d.title,
      body: d.body(remote.name),
      facts: [
        { label: m.confirm.project, value: project.name },
        { label: t.remoteUrl, value: <span className="mono selectable">{remote.fetchUrl}</span> },
      ],
      risk: d.risk,
      level: "caution",
      confirmLabel: d.submit,
    });
    if (!result.confirmed) return;
    try {
      onDetail(await api.removeRemote({ projectId, name: remote.name }));
    } catch (error) {
      notifyError(error);
    }
  };

  return (
    <div className={styles.tab}>
      {isRepo && remotes.length > 0 && (
        <Section title={t.targets} hint={status?.branch ? t.targetsHint(status.branch) : undefined}>
          <TargetEditor project={project} branches={branches} />
        </Section>
      )}

      {isRepo && (
        <Section title={t.remotes} hint={t.remotesHint}>
          {remotes.length > 0 && (
            <ul className={styles.remotes}>
              {remotes.map((remote) => (
                <li key={remote.name} className={styles.remote}>
                  <span className={styles.remoteIcon}>{remote.github ? <Icons.github size={14} /> : <Icons.globe size={14} />}</span>
                  <div className={styles.remoteMain}>
                    <span className={clsx("mono", styles.remoteName)}>{remote.name}</span>
                    <span className={clsx("mono selectable truncate", styles.remoteUrl)}>{remote.fetchUrl}</span>
                  </div>
                  <IconButton label={m.common.edit} size="sm" onClick={() => setRemoteDialog({ remote })}>
                    <Icons.pencil size={13} />
                  </IconButton>
                  <IconButton label={m.common.remove} size="sm" onClick={() => void removeRemote(remote)}>
                    <Icons.trash size={13} />
                  </IconButton>
                </li>
              ))}
            </ul>
          )}
          <div className={styles.inlineActions}>
            <Button size="sm" icon={<Icons.plus size={13} />} onClick={() => setRemoteDialog({ remote: null })}>
              {t.addRemote}
            </Button>
            {remotes.length === 0 && (
              <>
                <Button size="sm" icon={<Icons.github size={13} />} onClick={() => openDialog({ type: "publish", projectIds: [projectId] })}>
                  {m.action.publish}…
                </Button>
                <Button size="sm" variant="ghost" onClick={() => openDialog({ type: "link", projectId })}>
                  {m.detail.setup.linkAction}
                </Button>
              </>
            )}
          </div>
          {remotes.length > 1 && (
            <Row label={t.defaultRemote} hint={t.defaultRemoteHint}>
              <Select
                size="sm"
                mono
                value={project.settings.defaultRemote ?? "__auto__"}
                onChange={(value) => void patch({ defaultRemote: value === "__auto__" ? null : value })}
                options={[{ value: "__auto__", label: t.auto }, ...remotes.map((remote) => ({ value: remote.name, label: remote.name }))]}
              />
            </Row>
          )}
        </Section>
      )}

      <Section title={t.behavior}>
        <Row label={t.includeInBatch} hint={t.includeInBatchHint}>
          <Switch checked={project.settings.includeInBatch} onChange={(includeInBatch) => void patch({ includeInBatch })} ariaLabel={t.includeInBatch} />
        </Row>
        {isRepo && (
          <Row label={t.pullMode} hint={t.pullModeHint}>
            <Select
              size="sm"
              value={project.settings.pullMode}
              onChange={(pullMode: PullMode) => void patch({ pullMode })}
              options={(["ff_only", "merge", "rebase"] as const).map((mode) => ({ value: mode, label: t.pullModes[mode] }))}
            />
          </Row>
        )}
      </Section>

      {isRepo && status?.branch && (
        <Section title={t.advanced} hint={t.advancedHint} tone="danger">
          <Row label={t.rebase.replace("…", "")} hint={t.rebaseHint}>
            <Button size="sm" variant="danger" onClick={() => setRebaseOpen(true)}>
              {t.rebase}
            </Button>
          </Row>
          {remotes.length > 0 && (
            <Row label={t.forcePush.replace("…", "")} hint={t.forcePushHint}>
              <Button size="sm" variant="danger" onClick={() => void forcePush(project)}>
                {t.forcePush}
              </Button>
            </Row>
          )}
          <Row label="Reset" hint={t.resetHint} />
        </Section>
      )}

      <Section title={t.danger}>
        <Row label={t.exclude.replace("…", "")} hint={t.excludeHint}>
          <Button size="sm" onClick={() => void excludeProject(project)}>
            {t.exclude}
          </Button>
        </Row>
      </Section>

      <RemoteDialog
        state={remoteDialog}
        existing={remotes}
        onClose={() => setRemoteDialog(null)}
        onSubmit={async (original, name, url) => {
          try {
            if (!original) {
              onDetail(await api.addRemote({ projectId, name, url }));
              return true;
            }
            if (url !== original.fetchUrl) {
              const result = await confirm({
                title: m.confirm.setRemoteUrl.title,
                facts: [
                  { label: m.confirm.project, value: project.name },
                  { label: t.remoteUrl, value: <span className="mono selectable">{`${original.fetchUrl}\n→ ${url}`}</span> },
                ],
                risk: m.confirm.setRemoteUrl.risk,
                level: "caution",
                confirmLabel: m.confirm.setRemoteUrl.submit,
              });
              if (!result.confirmed) return false;
              onDetail(await api.setRemoteUrl({ projectId, name: original.name, url }));
            }
            if (name !== original.name) onDetail(await api.renameRemote({ projectId, from: original.name, to: name }));
            return true;
          } catch (error) {
            notifyError(error);
            return false;
          }
        }}
      />
      <RebaseDialog
        open={rebaseOpen}
        current={status?.branch ?? null}
        options={branches.filter((branch) => !branch.current).map((branch) => branch.name)}
        onClose={() => setRebaseOpen(false)}
        onSubmit={(onto) => {
          setRebaseOpen(false);
          void rebaseOnto(project, onto);
        }}
      />
    </div>
  );
}

interface RemoteDialogProps {
  state: { remote: RemoteInfo | null } | null;
  existing: RemoteInfo[];
  onClose: () => void;
  onSubmit: (original: RemoteInfo | null, name: string, url: string) => Promise<boolean>;
}

function RemoteDialog({ state, existing, onClose, onSubmit }: RemoteDialogProps) {
  const m = useT();
  const t = m.settingsProject;
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [shown, setShown] = useState<typeof state>(null);
  if (state && state !== shown) {
    setShown(state);
    setName(state.remote?.name ?? (existing.some((remote) => remote.name === "origin") ? "" : "origin"));
    setUrl(state.remote?.fetchUrl ?? "");
  }
  const current = state ?? shown;
  const original = current?.remote ?? null;
  const trimmedName = name.trim();
  const trimmedUrl = url.trim();
  const duplicate = existing.some((remote) => remote.name === trimmedName && remote.name !== original?.name);
  const unchanged = original !== null && trimmedName === original.name && trimmedUrl === original.fetchUrl;
  const valid = trimmedName !== "" && trimmedUrl !== "" && !duplicate && !/\s/.test(trimmedName) && !unchanged;
  const submit = async () => {
    if (!valid) return;
    setBusy(true);
    const ok = await onSubmit(original, trimmedName, trimmedUrl);
    setBusy(false);
    if (ok) onClose();
  };
  return (
    <Dialog
      open={state !== null}
      onOpenChange={(next) => !next && onClose()}
      title={original ? `${t.remotes}: ${original.name}` : t.addRemote}
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant="primary" disabled={!valid} loading={busy} onClick={() => void submit()}>
            {original ? m.common.save : m.common.add}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <Field label={t.remoteName} error={duplicate ? m.error.remote_exists.title : undefined}>
          <TextField mono value={name} onChange={(event) => setName(event.target.value)} invalid={duplicate} autoFocus={!original} />
        </Field>
        <Field label={t.remoteUrl}>
          <TextField
            mono
            value={url}
            onChange={(event) => setUrl(event.target.value)}
            placeholder={t.remoteUrlPlaceholder}
            autoFocus={Boolean(original)}
            onKeyDown={(event) => event.key === "Enter" && void submit()}
          />
        </Field>
      </div>
    </Dialog>
  );
}

function RebaseDialog({ open, current, options, onClose, onSubmit }: { open: boolean; current: string | null; options: string[]; onClose: () => void; onSubmit: (onto: string) => void }) {
  const m = useT();
  const r = m.confirm.rebase;
  const [onto, setOnto] = useState<string | undefined>(undefined);
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={r.title}
      description={current && onto ? r.body(current, onto) : m.settingsProject.rebaseHint}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant="primary" disabled={!onto} onClick={() => onto && onSubmit(onto)}>
            {m.common.next}
          </Button>
        </>
      }
    >
      <Field label={r.onto}>
        <Select mono value={onto} onChange={setOnto} placeholder={m.common.select} options={options.map((name) => ({ value: name, label: name }))} leading={<Icons.branch size={13} />} />
      </Field>
    </Dialog>
  );
}
