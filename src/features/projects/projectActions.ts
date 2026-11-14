import { t } from "../../i18n";
import { api } from "../../ipc/client";
import type { OpenTarget, Project } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { copyText } from "../../lib/platform";
import { useAppStore } from "../../stores/app";
import { confirm } from "../../stores/confirm";
import { useOperationStore } from "../../stores/operations";
import { useProjectStore } from "../../stores/projects";
import { toast } from "../../stores/toasts";
import { useUiStore } from "../../stores/ui";
import type { PrimaryAction } from "./model";

export function runPrimary(project: Project, action: PrimaryAction) {
  if (!action) return;
  const ui = useUiStore.getState();
  switch (action.type) {
    case "operation":
      return void useOperationStore.getState().quick(action.request);
    case "open":
      return ui.openProject(project.id, action.tab);
    case "refresh":
      return void useProjectStore.getState().refresh([project.id]).catch(notifyError);
    case "dialog":
      if (action.dialog === "publish") return ui.openDialog({ type: "publish", projectIds: [project.id] });
      if (action.dialog === "login") return ui.openDialog({ type: "login" });
      return ui.openDialog({ type: action.dialog, projectId: project.id });
  }
}

export async function openIn(projectId: string, target: OpenTarget, relativePath: string | null = null) {
  try {
    await api.openProject({ projectId, target, relativePath });
  } catch (error) {
    notifyError(error);
  }
}

export async function toggleFavorite(project: Project) {
  try {
    const updated = await api.updateProjectSettings({ projectId: project.id, patch: { favorite: !project.settings.favorite } });
    useProjectStore.getState().upsert(updated);
  } catch (error) {
    notifyError(error);
  }
}

export async function copyPath(project: Project) {
  if (await copyText(project.path)) toast.success(t().common.copied);
}

export async function excludeProject(project: Project) {
  const m = t();
  const result = await confirm({
    title: m.confirm.exclude.title,
    body: m.confirm.exclude.body(project.name),
    risk: undefined,
    content: undefined,
    level: "safe",
    confirmLabel: m.confirm.exclude.submit,
  });
  if (!result.confirmed) return;
  try {
    const workspace = await api.excludeProject({ projectId: project.id });
    useAppStore.getState().applyWorkspace(workspace);
    const view = useUiStore.getState().view;
    if (view.name === "project" && view.projectId === project.id) useUiStore.getState().back();
    toast.success(m.confirm.exclude.done(project.name), { description: m.confirm.exclude.undoHint });
  } catch (error) {
    notifyError(error);
  }
}
