import { useEffect, useState } from "react";
import { Button } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import { Field, TextField } from "../../components/Field";
import { Icons } from "../../components/icons";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import type { GithubRepo } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { displayPath, joinPath } from "../../lib/path";
import { useAppStore } from "../../stores/app";
import { useGithubStore } from "../../stores/github";
import { runAndWait } from "../../stores/operations";
import { useUiStore } from "../../stores/ui";
import styles from "./dialogs.module.css";

export function CloneDialog({ repo, onClose }: { repo: GithubRepo | null; onClose: () => void }) {
  const m = useT();
  const t = m.github.clone;
  const roots = useAppStore((state) => state.settings?.roots ?? []);
  const lastCloneDir = useAppStore((state) => state.ui.lastCloneDir);
  const gitProtocol = useGithubStore((state) => state.status?.gitProtocol);
  const [shown, setShown] = useState<GithubRepo | null>(null);
  const [parent, setParent] = useState<string | null>(null);
  const [folder, setFolder] = useState("");

  useEffect(() => {
    if (!repo) return;
    setShown(repo);
    setFolder(repo.name);
    setParent(lastCloneDir ?? roots.find((root) => root.exists && root.enabled)?.path ?? null);
  }, [repo]);

  const target = repo ?? shown;
  const folderValid = folder.trim() !== "" && !/[\\/]/.test(folder) && !folder.startsWith(".");
  const destination = parent && folderValid ? joinPath(parent, folder.trim()) : null;

  const choose = async () => {
    try {
      const picked = await api.pickFolder({ title: t.chooseDestination });
      if (picked) setParent(picked);
    } catch (error) {
      notifyError(error);
    }
  };

  const submit = async () => {
    if (!target || !parent || !destination) return;
    useAppStore.getState().setUi({ lastCloneDir: parent });
    onClose();
    const result = await runAndWait({
      kind: "clone",
      items: [{ itemId: `clone:${target.nameWithOwner}`, url: gitProtocol === "ssh" ? target.sshUrl : target.cloneUrl, parentDir: parent, folderName: folder.trim() }],
    });
    const item = result?.items[0];
    if (item?.state === "succeeded") {
      void useGithubStore.getState().loadRepos(true);
      if (item.projectId) useUiStore.getState().openProject(item.projectId);
    }
  };

  return (
    <Dialog
      open={repo !== null}
      onOpenChange={(next) => !next && onClose()}
      title={t.title}
      description={target?.nameWithOwner}
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button variant="primary" disabled={!destination} onClick={() => void submit()}>
            {t.submit}
          </Button>
        </>
      }
    >
      <div className={styles.form}>
        <Field label={t.destination} hint={destination ? t.willCreate(displayPath(destination)) : t.needDestination}>
          <div className={styles.pathBox}>
            <Icons.folder size={14} />
            <span className="mono truncate selectable">{parent ? displayPath(parent) : "—"}</span>
            <Button size="sm" variant="ghost" onClick={() => void choose()}>
              {m.common.change}
            </Button>
          </div>
        </Field>
        <Field label={t.folderName}>
          <TextField mono value={folder} onChange={(event) => setFolder(event.target.value)} invalid={!folderValid} onKeyDown={(event) => event.key === "Enter" && void submit()} />
        </Field>
      </div>
    </Dialog>
  );
}
