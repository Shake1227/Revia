import clsx from "clsx";
import { AnimatePresence, motion } from "motion/react";
import { useEffect } from "react";
import { Button, IconButton } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { ErrorNote } from "../../components/ErrorNote";
import { Icons } from "../../components/icons";
import { Menu, MenuContent, MenuTrigger } from "../../components/Menu";
import { StatusGlyph, stateVisual } from "../../components/StatusGlyph";
import { Tabs } from "../../components/Tabs";
import { Tooltip } from "../../components/Tooltip";
import { ease } from "../../design/motion";
import { useT } from "../../i18n";
import { displayPath } from "../../lib/path";
import { copyText } from "../../lib/platform";
import { relativeTime } from "../../lib/time";
import { useOperationStore } from "../../stores/operations";
import { useProjectStore } from "../../stores/projects";
import { toast } from "../../stores/toasts";
import { useUiStore, type DetailTab } from "../../stores/ui";
import { batchRequest, primaryAction } from "../projects/model";
import { runPrimary, toggleFavorite } from "../projects/projectActions";
import { ProjectMenuItems } from "../projects/ProjectMenu";
import { BranchesTab } from "./BranchesTab";
import { ChangesTab } from "./ChangesTab";
import { ConflictPanel } from "./ConflictPanel";
import { HistoryTab } from "./HistoryTab";
import { statusSignature, useProjectDetail } from "./hooks";
import styles from "./ProjectDetailPage.module.css";
import { SettingsTab } from "./SettingsTab";
import { SetupGuide } from "./SetupGuide";
import { describeProject } from "./summary";
import { TargetLine } from "./TargetLine";

export function ProjectDetailPage({ projectId, tab }: { projectId: string; tab: DetailTab }) {
  const m = useT();
  const project = useProjectStore((state) => state.byId[projectId]);
  const back = useUiStore((state) => state.back);
  const setTab = useUiStore((state) => state.setTab);
  const quick = useOperationStore((state) => state.quick);
  const activity = useOperationStore((state) => state.activity[projectId]);
  const flash = useOperationStore((state) => state.flash[projectId]);
  const signature = statusSignature(project);
  const detail = useProjectDetail(projectId, signature);

  useEffect(() => {
    void useProjectStore.getState().refresh([projectId]).catch(() => undefined);
  }, [projectId]);

  if (!project) {
    return (
      <div className={styles.page}>
        <EmptyState
          icon={<Icons.folderX size={20} />}
          title={m.error.project_not_found.title}
          action={<Button onClick={back}>{m.common.back}</Button>}
        />
      </div>
    );
  }

  const status = project.status;
  const state = status?.state ?? null;
  const tone = state ? stateVisual[state].tone : "neutral";
  const { headline, guidance } = describeProject(project, m);
  const action = primaryAction(project, m);
  const busy = Boolean(activity);
  const isRepo = status !== null && !["not_initialized", "missing"].includes(status.state);
  const hasRemote = isRepo && status.remotes.length > 0;
  const needsSetup = status !== null && (status.state === "not_initialized" || status.state === "no_remote" || (isRepo && status.unborn));
  const showConflict = state === "conflicted" || state === "in_progress";
  const showAction = action && !(action.type === "open" && (action.tab === tab || showConflict || needsSetup)) && !showConflict;

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <Tooltip label={m.common.back} shortcut={["esc"]}>
          <button type="button" className={styles.back} onClick={back} aria-label={m.common.back}>
            <Icons.chevronLeft size={16} />
            <span>{m.nav.projects}</span>
          </button>
        </Tooltip>
        <span className={styles.grow} />
        <Tooltip label={project.settings.favorite ? m.action.unfavorite : m.action.favorite}>
          <button
            type="button"
            className={styles.star}
            data-active={project.settings.favorite || undefined}
            aria-pressed={project.settings.favorite}
            aria-label={m.action.favorite}
            onClick={() => void toggleFavorite(project)}
          >
            <Icons.star size={15} fill={project.settings.favorite ? "currentColor" : "none"} />
          </button>
        </Tooltip>
        {hasRemote && (
          <Tooltip label={m.action.fetch} hint={m.actionHint.fetch}>
            <Button icon={<Icons.download size={14} />} disabled={busy} onClick={() => void quick(batchRequest("fetch", [projectId]))}>
              {m.action.fetch}
            </Button>
          </Tooltip>
        )}
        {showAction && action && (
          <Tooltip label={action.hint} side="bottom">
            <Button variant={action.emphasis ? "primary" : "secondary"} disabled={busy} onClick={() => runPrimary(project, action)}>
              {action.label}
            </Button>
          </Tooltip>
        )}
        <Menu>
          <MenuTrigger asChild>
            <IconButton label={m.common.more} noTooltip>
              <Icons.ellipsis size={16} />
            </IconButton>
          </MenuTrigger>
          <MenuContent minWidth={220}>
            <ProjectMenuItems project={project} showOpenDetail={false} />
          </MenuContent>
        </Menu>
      </header>

      <section className={styles.hero} data-tone={tone}>
        <StatusGlyph state={state} size="lg" busy={busy} progress={activity?.progress} flash={flash?.state ?? null} />
        <div className={styles.heroMain}>
          <div className={styles.titleRow}>
            <h1 className={clsx(styles.name, "truncate")}>{project.name}</h1>
            {state && <span className={styles.stateTag}>{m.state[state]}</span>}
          </div>
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={busy ? `busy-${activity?.step}` : headline}
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -3 }}
              transition={{ duration: 0.16, ease: ease.out }}
            >
              <p className={styles.headline}>{busy && activity?.step ? m.ops.stepRunning[activity.step] : headline}</p>
              {!busy && guidance && <p className={styles.guidance}>{guidance}</p>}
            </motion.div>
          </AnimatePresence>
        </div>
      </section>

      <div className={`${styles.scroll} rv-sheet`}>
        <div className={styles.inner}>
          <div className={styles.meta}>
            <TargetLine project={project} />
            <button
              type="button"
              className={clsx(styles.path, "mono truncate")}
              title={project.path}
              onClick={() => void copyText(project.path).then((ok) => ok && toast.success(m.common.copied))}
            >
              {displayPath(project.path)}
            </button>
            {status && (
              <span className={styles.times}>
                {m.detail.lastChecked(relativeTime(status.checkedAt, m))}
                {hasRemote && `${m.common.dot}${status.fetchedAt ? m.detail.lastFetched(relativeTime(status.fetchedAt, m)) : m.detail.neverFetched}`}
              </span>
            )}
          </div>
          {status?.error && <ErrorNote error={status.error} />}
          {showConflict && <ConflictPanel project={project} signature={signature} />}
          {needsSetup && !showConflict && <SetupGuide project={project} />}

          {isRepo ? (
            <>
              <Tabs<DetailTab>
                value={tab}
                onChange={setTab}
                ariaLabel={project.name}
                items={[
                  { value: "changes", label: m.detail.tabs.changes, badge: status.changes.total > 0 ? status.changes.total : undefined },
                  { value: "history", label: m.detail.tabs.history },
                  { value: "branches", label: m.detail.tabs.branches },
                  { value: "settings", label: m.detail.tabs.settings },
                ]}
              />
              <div className={styles.tabBody} role="tabpanel">
                {tab === "changes" && <ChangesTab project={project} signature={signature} />}
                {tab === "history" && <HistoryTab project={project} signature={signature} branches={detail.data?.branches ?? []} />}
                {tab === "branches" && (
                  <BranchesTab project={project} detail={detail.data} loading={detail.loading} onDetail={(next) => { detail.setData(next); useProjectStore.getState().upsert(next.project); }} />
                )}
                {tab === "settings" && (
                  <SettingsTab project={project} detail={detail.data} onDetail={(next) => { detail.setData(next); useProjectStore.getState().upsert(next.project); }} />
                )}
              </div>
            </>
          ) : (
            status?.state !== "not_initialized" && <SettingsTab project={project} detail={null} onDetail={() => undefined} />
          )}
        </div>
      </div>
    </div>
  );
}
