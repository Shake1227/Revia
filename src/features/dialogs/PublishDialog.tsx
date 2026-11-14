import { useEffect, useState } from "react";
import { Button } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import { Note } from "../../components/ErrorNote";
import { Field, TextField } from "../../components/Field";
import { Icons } from "../../components/icons";
import { Segmented } from "../../components/Segmented";
import { Select } from "../../components/Select";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import type { Project, PublishItem } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { useAppStore } from "../../stores/app";
import { useGithubStore } from "../../stores/github";
import { useOperationStore } from "../../stores/operations";
import { useProjectStore } from "../../stores/projects";
import { toast } from "../../stores/toasts";
import { useUiStore } from "../../stores/ui";
import styles from "./dialogs.module.css";
import { IgnorePicker, useIgnoreSelection } from "./IgnorePicker";

const repoName = (name: string) => name.trim().replace(/\s+/g, "-").replace(/[^A-Za-z0-9._-]/g, "-");

export function PublishDialog({ projectIds, onClose }: { projectIds: string[] | null; onClose: () => void }) {
  const m = useT();
  const t = m.github.publish;
  const byId = useProjectStore((state) => state.byId);
  const status = useGithubStore((state) => state.status);
  const owners = useGithubStore((state) => state.owners);
  const loadOwners = useGithubStore((state) => state.loadOwners);
  const prepare = useOperationStore((state) => state.prepare);
  const openDialog = useUiStore((state) => state.openDialog);
  const [shownIds, setShownIds] = useState<string[]>([]);
  const [name, setName] = useState("");
  const [owner, setOwner] = useState("__me__");
  const [visibility, setVisibility] = useState<"private" | "public">("private");
  const [description, setDescription] = useState("");
  const [initialMessage, setInitialMessage] = useState("");
  const [initializing, setInitializing] = useState(false);

  const open = projectIds !== null;
  const ids = projectIds ?? shownIds;
  const projects = ids.map((id) => byId[id]).filter((project) => project !== undefined);
  const single = projects.length === 1 ? projects[0]! : null;
  const authenticated = status?.authenticated ?? false;

  useEffect(() => {
    if (!projectIds) return;
    setShownIds(projectIds);
    const first = byId[projectIds[0] ?? ""];
    setName(first ? repoName(first.name) : "");
    setOwner("__me__");
    setVisibility("private");
    setDescription("");
    setInitialMessage(t.initialCommitDefault);
    if (authenticated && owners === null) void loadOwners();
  }, [projectIds]);

  const firstCommit = (project: Project) => project.status?.state === "not_initialized" || project.status?.unborn === true;
  const needsInitialCommit = projects.some(firstCommit);
  const autoIgnore = useAppStore((state) => state.ui.publishAutoIgnore);
  const setUi = useAppStore((state) => state.setUi);
  const ignoreTargets = projects.filter(firstCommit);
  const ignore = useIgnoreSelection(ignoreTargets, open && autoIgnore);
  const nameValid = !single || /^[A-Za-z0-9._-]+$/.test(name.trim());
  const canSubmit = authenticated && projects.length > 0 && nameValid && (!needsInitialCommit || initialMessage.trim() !== "") && !ignore.loading;

  const submit = () => {
    const items: PublishItem[] = projects.map((project) => ({
      itemId: project.id,
      projectId: project.id,
      owner: owner === "__me__" ? null : owner,
      name: single ? name.trim() : repoName(project.name),
      visibility,
      description: single && description.trim() ? description.trim() : null,
      initialCommitMessage: firstCommit(project) ? initialMessage.trim() : null,
      ignore: autoIgnore && firstCommit(project) && (ignore.selection[project.id]?.length ?? 0) > 0 ? (ignore.selection[project.id] ?? null) : null,
    }));
    onClose();
    void prepare({ kind: "publish", items });
  };

  const initOnly = async (project: Project) => {
    setInitializing(true);
    try {
      useProjectStore.getState().upsert(await api.initRepository({ projectId: project.id, initialBranch: null }));
      toast.success(t.initDone(project.name));
      onClose();
      useUiStore.getState().openProject(project.id);
    } catch (error) {
      notifyError(error);
    } finally {
      setInitializing(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={single ? t.title : t.titleBatch(projects.length)}
      description={single ? undefined : t.batchNote}
      size="md"
      footerStart={
        single?.status?.state === "not_initialized" ? (
          <Button variant="ghost" loading={initializing} onClick={() => void initOnly(single)}>
            {t.initOnly}
          </Button>
        ) : undefined
      }
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant="primary" disabled={!canSubmit} onClick={submit}>
            {t.review}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        {!authenticated && (
          <Note tone="warning">
            {m.error.not_authenticated.title}
            <div style={{ marginTop: 8 }}>
              <Button size="sm" icon={<Icons.github size={13} />} onClick={() => openDialog({ type: "login" })}>
                {m.github.connect.action}
              </Button>
            </div>
          </Note>
        )}
        {single && (
          <div className={styles.columns}>
            <Field label={t.owner}>
              <Select
                value={owner}
                onChange={setOwner}
                options={[
                  { value: "__me__", label: status?.account?.login ?? "—" },
                  ...(owners ?? []).filter((item) => item.kind === "organization").map((item) => ({ value: item.login, label: item.login })),
                ]}
              />
            </Field>
            <Field label={t.name} error={!nameValid ? t.nameInvalid : undefined}>
              <TextField mono value={name} onChange={(event) => setName(event.target.value)} invalid={!nameValid} autoFocus />
            </Field>
          </div>
        )}
        {!single && (
          <Field label={t.owner}>
            <Select
              value={owner}
              onChange={setOwner}
              options={[
                { value: "__me__", label: status?.account?.login ?? "—" },
                ...(owners ?? []).filter((item) => item.kind === "organization").map((item) => ({ value: item.login, label: item.login })),
              ]}
            />
          </Field>
        )}
        <Field label={t.visibility} hint={visibility === "private" ? t.privateHint : t.publicHint}>
          <Segmented
            value={visibility}
            onChange={setVisibility}
            options={[
              { value: "private", label: t.private, icon: <Icons.lock size={12} /> },
              { value: "public", label: t.public, icon: <Icons.globe size={12} /> },
            ]}
          />
        </Field>
        {single && (
          <Field label={t.description}>
            <TextField value={description} onChange={(event) => setDescription(event.target.value)} />
          </Field>
        )}
        {needsInitialCommit && (
          <Field label={t.initialCommit}>
            <TextField value={initialMessage} onChange={(event) => setInitialMessage(event.target.value)} />
          </Field>
        )}
        {needsInitialCommit && (
          <IgnorePicker
            projects={ignoreTargets}
            auto={autoIgnore}
            onAutoChange={(publishAutoIgnore) => setUi({ publishAutoIgnore })}
            scans={ignore.scans}
            selection={ignore.selection}
            onToggle={ignore.toggle}
          />
        )}
      </div>
    </Dialog>
  );
}
