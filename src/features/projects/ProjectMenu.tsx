import { Icons } from "../../components/icons";
import { MenuItem, MenuSeparator } from "../../components/Menu";
import { useT } from "../../i18n";
import type { Project } from "../../ipc/types";
import { useAppStore } from "../../stores/app";
import { useOperationStore } from "../../stores/operations";
import { useUiStore } from "../../stores/ui";
import { batchRequest } from "./model";
import { copyPath, excludeProject, openIn, toggleFavorite } from "./projectActions";

export function ProjectMenuItems({ project, showOpenDetail = true }: { project: Project; showOpenDetail?: boolean }) {
  const m = useT();
  const quick = useOperationStore((state) => state.quick);
  const openProject = useUiStore((state) => state.openProject);
  const openDialog = useUiStore((state) => state.openDialog);
  const status = project.status;
  const hasGit = status !== null && !["not_initialized", "missing"].includes(status.state);
  const hasRemote = hasGit && status.remotes.length > 0;
  const exists = status?.state !== "missing";
  const platform = useAppStore((state) => state.environment?.platform ?? "macos");
  const revealLabel =
    platform === "macos" ? m.action.openInFinder : platform === "windows" ? m.action.openInExplorer : m.action.openInFileManager;

  return (
    <>
      {showOpenDetail && (
        <>
          <MenuItem icon={<Icons.arrowUpRight size={14} />} onSelect={() => openProject(project.id)} shortcut={["enter"]}>
            {m.action.showDetail}
          </MenuItem>
          <MenuSeparator />
        </>
      )}
      {hasRemote && (
        <>
          <MenuItem icon={<Icons.refresh size={14} />} hint={m.actionHint.sync} onSelect={() => void quick(batchRequest("sync", [project.id]))}>
            {m.action.sync}
          </MenuItem>
          <MenuItem icon={<Icons.download size={14} />} onSelect={() => void quick(batchRequest("fetch", [project.id]))}>
            {m.action.fetch}
          </MenuItem>
          <MenuItem icon={<Icons.arrowDown size={14} />} onSelect={() => void quick(batchRequest("pull", [project.id]))}>
            {m.action.pull}
          </MenuItem>
          <MenuItem icon={<Icons.arrowUp size={14} />} onSelect={() => void quick(batchRequest("push", [project.id]))}>
            {m.action.push}
          </MenuItem>
          <MenuSeparator />
        </>
      )}
      {exists && (
        <>
          <MenuItem icon={<Icons.folderOpen size={14} />} onSelect={() => void openIn(project.id, "file_manager")}>
            {revealLabel}
          </MenuItem>
          <MenuItem icon={<Icons.code size={14} />} onSelect={() => void openIn(project.id, "editor")}>
            {m.action.openInEditor}
          </MenuItem>
          <MenuItem icon={<Icons.terminal size={14} />} onSelect={() => void openIn(project.id, "terminal")}>
            {m.action.openInTerminal}
          </MenuItem>
        </>
      )}
      {status?.github && (
        <MenuItem icon={<Icons.github size={14} />} onSelect={() => void openIn(project.id, "github")}>
          {m.action.openOnGithub}
        </MenuItem>
      )}
      <MenuItem icon={<Icons.copy size={14} />} onSelect={() => void copyPath(project)}>
        {m.action.copyPath}
      </MenuItem>
      <MenuSeparator />
      <MenuItem icon={<Icons.star size={14} />} onSelect={() => void toggleFavorite(project)}>
        {project.settings.favorite ? m.action.unfavorite : m.action.favorite}
      </MenuItem>
      <MenuItem icon={<Icons.archive size={14} />} onSelect={() => void excludeProject(project)}>
        {m.action.exclude}…
      </MenuItem>
      {hasGit && (
        <MenuItem icon={<Icons.unlink size={14} />} onSelect={() => openDialog({ type: "unlink", projectId: project.id })}>
          {m.action.unlink}…
        </MenuItem>
      )}
      {exists && (
        <MenuItem icon={<Icons.trash size={14} />} danger onSelect={() => openDialog({ type: "delete", projectId: project.id })}>
          {m.action.remove}…
        </MenuItem>
      )}
    </>
  );
}
