import clsx from "clsx";
import { memo, useState, type MouseEvent } from "react";
import { Button, IconButton } from "../../components/Button";
import { Checkbox } from "../../components/Checkbox";
import { Icons } from "../../components/icons";
import { Menu, MenuContent, MenuTrigger } from "../../components/Menu";
import { StatusGlyph, stateVisual } from "../../components/StatusGlyph";
import { Tooltip } from "../../components/Tooltip";
import { useT } from "../../i18n";
import { displayPath } from "../../lib/path";
import { isModKey } from "../../lib/platform";
import { relativeTime } from "../../lib/time";
import { useOperationStore } from "../../stores/operations";
import { useProjectStore } from "../../stores/projects";
import { useUiStore } from "../../stores/ui";
import { primaryAction } from "./model";
import { runPrimary } from "./projectActions";
import { ProjectMenuItems } from "./ProjectMenu";
import styles from "./ProjectRow.module.css";

interface ProjectRowProps {
  projectId: string;
  orderedIds: string[];
  selectionMode: boolean;
  compact: boolean;
}

export const ProjectRow = memo(function ProjectRow({ projectId, orderedIds, selectionMode, compact }: ProjectRowProps) {
  const m = useT();
  const project = useProjectStore((state) => state.byId[projectId]);
  const selected = useProjectStore((state) => Boolean(state.selected[projectId]));
  const isCursor = useProjectStore((state) => state.cursorId === projectId);
  const activity = useOperationStore((state) => state.activity[projectId]);
  const flash = useOperationStore((state) => state.flash[projectId]);
  const [menuOpen, setMenuOpen] = useState(false);

  if (!project) return null;
  const status = project.status;
  const state = status?.state ?? null;
  const visual = state ? stateVisual[state] : null;
  const action = primaryAction(project, m);
  const busy = Boolean(activity);

  const handleClick = (event: MouseEvent) => {
    const store = useProjectStore.getState();
    if (event.shiftKey) {
      event.preventDefault();
      store.selectRange(projectId, orderedIds);
    } else if (isModKey(event)) {
      store.toggle(projectId);
    } else {
      useUiStore.getState().openProject(projectId);
    }
  };

  const changes = status && state !== "conflicted" ? status.changes.total : 0;
  const ahead = status?.ahead ?? 0;
  const behind = status?.behind ?? 0;

  return (
    <div
      id={`project-row-${projectId}`}
      role="option"
      aria-selected={selected}
      className={clsx(styles.row, compact && styles.compact)}
      data-selected={selected || undefined}
      data-cursor={isCursor || undefined}
      data-busy={busy || undefined}
      data-flash={flash?.state}
      data-menu-open={menuOpen || undefined}
      data-selection-mode={selectionMode || undefined}
      data-tone={visual?.tone}
      onClick={handleClick}
      onContextMenu={(event) => {
        event.preventDefault();
        setMenuOpen(true);
      }}
    >
      <span className={styles.check} onClick={(event) => event.stopPropagation()}>
        <Checkbox
          checked={selected}
          ariaLabel={`${project.name} ${m.common.select}`}
          tabIndex={-1}
          onClick={(event) => {
            if (event.shiftKey) {
              event.preventDefault();
              useProjectStore.getState().selectRange(projectId, orderedIds);
            }
          }}
          onChange={() => useProjectStore.getState().toggle(projectId)}
        />
      </span>

      <Tooltip
        side="right"
        label={state ? m.stateHint[state] : m.common.loading}
        hint={status ? m.detail.lastChecked(relativeTime(status.checkedAt, m)) : undefined}
      >
        <span className={styles.glyph}>
          <StatusGlyph state={state} busy={busy} progress={activity?.progress} flash={flash?.state ?? null} />
        </span>
      </Tooltip>

      <div className={styles.identity}>
        <div className={styles.nameLine}>
          <span className={clsx(styles.name, "truncate")}>{project.name}</span>
          {project.settings.favorite && <Icons.star size={11} className={styles.star} fill="currentColor" />}
        </div>
        <span className={clsx(styles.path, "truncate")}>{displayPath(project.path)}</span>
      </div>

      <div className={styles.branch}>
        {status?.branch ? (
          <span className={clsx(styles.branchChip, "truncate")}>
            <Icons.branch size={12} />
            <span className="mono truncate">{status.branch}</span>
          </span>
        ) : status?.detached && status.headSha ? (
          <span className={clsx(styles.branchChip, styles.detached)}>
            <Icons.commit size={12} />
            <span className="mono">{status.headSha}</span>
          </span>
        ) : null}
      </div>

      <div className={styles.status}>
        {busy && activity ? (
          <span className={clsx(styles.running, "truncate")}>
            {activity.step ? m.ops.stepRunning[activity.step] : m.ops.state.queued}
          </span>
        ) : (
          <>
            <span className={clsx(styles.stateLabel, "truncate")}>{state ? m.state[state] : ""}</span>
            <span className={styles.counts}>
              {changes > 0 && (
                <Tooltip label={m.projects.row.changes(changes)}>
                  <span className={styles.count} data-kind="dirty">
                    <Icons.pencil size={10} strokeWidth={2.2} />
                    {changes}
                  </span>
                </Tooltip>
              )}
              {ahead > 0 && (
                <Tooltip label={m.projects.row.ahead(ahead)}>
                  <span className={styles.count} data-kind="ahead">
                    <Icons.arrowUp size={10} strokeWidth={2.4} />
                    {ahead}
                  </span>
                </Tooltip>
              )}
              {behind > 0 && (
                <Tooltip label={m.projects.row.behind(behind)}>
                  <span className={styles.count} data-kind="behind">
                    <Icons.arrowDown size={10} strokeWidth={2.4} />
                    {behind}
                  </span>
                </Tooltip>
              )}
            </span>
          </>
        )}
      </div>

      <span className={clsx(styles.time, "tabular")}>{status?.lastCommitAt ? relativeTime(status.lastCommitAt, m) : ""}</span>

      <div className={styles.actions} onClick={(event) => event.stopPropagation()}>
        {action && !busy && (
          <Tooltip label={action.hint} side="top">
            <Button
              size="sm"
              variant="secondary"
              className={clsx(styles.primaryAction, action.emphasis && styles.emphasis)}
              tabIndex={-1}
              onClick={() => runPrimary(project, action)}
            >
              {action.label}
            </Button>
          </Tooltip>
        )}
        <Menu open={menuOpen} onOpenChange={setMenuOpen}>
          <MenuTrigger asChild>
            <IconButton label={m.common.more} size="sm" tabIndex={-1} noTooltip className={styles.more}>
              <Icons.ellipsis size={15} />
            </IconButton>
          </MenuTrigger>
          <MenuContent minWidth={220}>
            <ProjectMenuItems project={project} />
          </MenuContent>
        </Menu>
      </div>

      {busy && (
        <span className={styles.progressLine} data-determinate={activity?.progress != null || undefined}>
          <span style={activity?.progress != null ? { transform: `scaleX(${Math.max(0.04, activity.progress)})` } : undefined} />
        </span>
      )}
    </div>
  );
});
