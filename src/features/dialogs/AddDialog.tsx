import { useState } from "react";
import { Dialog } from "../../components/Dialog";
import { Icons } from "../../components/icons";
import { Select } from "../../components/Select";
import { Spinner } from "../../components/Spinner";
import { t as messages, useT } from "../../i18n";
import { api } from "../../ipc/client";
import type { Project } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { baseName } from "../../lib/path";
import { useAppStore } from "../../stores/app";
import { useProjectStore } from "../../stores/projects";
import { toast } from "../../stores/toasts";
import { useUiStore } from "../../stores/ui";
import styles from "./dialogs.module.css";

export async function addFolder(maxDepth: number | null = null): Promise<boolean> {
  const m = messages();
  const path = await api.pickFolder({ title: m.add.chooseFolder });
  if (!path) return false;
  const before = new Set(useProjectStore.getState().ids);
  const workspace = await api.addScanRoot({ path, maxDepth });
  useAppStore.getState().applyWorkspace(workspace);
  const added = workspace.projects.filter((project) => !before.has(project.id)).length;
  if (added === 0) toast.warning(m.add.noneFound, { description: m.add.noneFoundHint });
  else toast.success(m.add.found(added));
  return true;
}

const comparable = (path: string) => path.replace(/[\\/]+$/, "").replace(/^\/private(?=\/)/, "");

export async function publishFolder(): Promise<boolean> {
  const m = messages();
  const path = await api.pickFolder({ title: m.add.choosePublish });
  if (!path) return false;
  const samePath = (project: Project) => comparable(project.path) === comparable(path);
  let project = Object.values(useProjectStore.getState().byId).find(samePath);
  if (!project) {
    const before = new Set(useProjectStore.getState().ids);
    const workspace = await api.addProject({ path });
    useAppStore.getState().applyWorkspace(workspace);
    project = workspace.projects.find((item) => !before.has(item.id)) ?? workspace.projects.find(samePath);
  }
  if (!project) return false;
  const [fresh] = await api.refreshProjects({ projectIds: [project.id] });
  if (fresh) {
    useProjectStore.getState().upsert(fresh);
    project = fresh;
  }
  const ui = useUiStore.getState();
  if (project.status && project.status.remotes.length > 0) {
    toast.info(m.add.alreadyRemote(project.name));
    ui.closeDialog();
    ui.openProject(project.id);
    return true;
  }
  ui.openDialog({ type: "publish", projectIds: [project.id] });
  return true;
}

export function AddDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const m = useT();
  const t = m.add;
  const navigate = useUiStore((state) => state.navigate);
  const [depth, setDepth] = useState("4");
  const [busy, setBusy] = useState<"folder" | "single" | "publish" | null>(null);

  const run = async (kind: "folder" | "single" | "publish") => {
    setBusy(kind);
    try {
      if (kind === "folder") {
        if (await addFolder(Number(depth))) onClose();
      } else if (kind === "publish") {
        await publishFolder();
      } else {
        const path = await api.pickFolder({ title: t.chooseProject });
        if (!path) return;
        useAppStore.getState().applyWorkspace(await api.addProject({ path }));
        toast.success(t.added(baseName(path)));
        onClose();
      }
    } catch (error) {
      notifyError(error);
    } finally {
      setBusy(null);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(next) => !next && busy === null && onClose()} title={t.title} size="md">
      <div className={styles.options}>
        <button type="button" className={styles.option} disabled={busy !== null} onClick={() => void run("folder")}>
          <span className={styles.optionIcon}>{busy === "folder" ? <Spinner size={16} /> : <Icons.folderSearch size={17} />}</span>
          <span className={styles.optionText}>
            <span className={styles.optionTitle}>{t.folderTitle}</span>
            <p className={styles.optionBody}>{t.folderBody}</p>
          </span>
        </button>
        <div className={styles.inline}>
          <span className={styles.hint}>{t.depthLabel}</span>
          <Select
            size="sm"
            value={depth}
            onChange={setDepth}
            options={["2", "3", "4", "5", "6"].map((value) => ({ value, label: t.depthHint(Number(value)) }))}
            ariaLabel={t.depthLabel}
          />
        </div>
        <button type="button" className={styles.option} disabled={busy !== null} onClick={() => void run("single")}>
          <span className={styles.optionIcon}>{busy === "single" ? <Spinner size={16} /> : <Icons.folderPlus size={17} />}</span>
          <span className={styles.optionText}>
            <span className={styles.optionTitle}>{t.singleTitle}</span>
            <p className={styles.optionBody}>{t.singleBody}</p>
          </span>
        </button>
        <button type="button" className={styles.option} disabled={busy !== null} onClick={() => void run("publish")}>
          <span className={styles.optionIcon}>{busy === "publish" ? <Spinner size={16} /> : <Icons.cloudUpload size={17} />}</span>
          <span className={styles.optionText}>
            <span className={styles.optionTitle}>{t.publishTitle}</span>
            <p className={styles.optionBody}>{t.publishBody}</p>
          </span>
        </button>
        <button
          type="button"
          className={styles.option}
          disabled={busy !== null}
          onClick={() => {
            onClose();
            navigate({ name: "github" });
          }}
        >
          <span className={styles.optionIcon}>
            <Icons.github size={17} />
          </span>
          <span className={styles.optionText}>
            <span className={styles.optionTitle}>{m.projects.cloneFromGithub}</span>
          </span>
          <Icons.chevronRight size={15} />
        </button>
      </div>
    </Dialog>
  );
}
