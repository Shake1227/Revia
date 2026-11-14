import clsx from "clsx";
import { useEffect, useMemo, useState } from "react";
import { Button, IconButton } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { ErrorNote } from "../../components/ErrorNote";
import { Icons } from "../../components/icons";
import { Menu, MenuContent, MenuItem, MenuTrigger } from "../../components/Menu";
import { Segmented } from "../../components/Segmented";
import { Select } from "../../components/Select";
import { Spinner } from "../../components/Spinner";
import { itemStateVisual } from "../../components/StatusGlyph";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import type { HistoryEntry } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { formatDay, formatTime, startOfDay } from "../../lib/time";
import { confirm } from "../../stores/confirm";
import { useProjectStore } from "../../stores/projects";
import { useUiStore } from "../../stores/ui";
import { useAsync } from "../detail/hooks";
import { useAllProjects } from "../projects/useVisibleProjects";
import { PageHeader } from "../shell/PageHeader";
import styles from "./HistoryPage.module.css";

const PAGE = 80;
const ISSUES = ["failed", "needs_confirmation", "conflict"];

function paramsText(entry: HistoryEntry, m: ReturnType<typeof useT>): string {
  const p = entry.params;
  const parts: string[] = [];
  if (typeof p.source === "string" && typeof p.target === "string") parts.push(`${p.source} → ${p.target}`);
  else if (typeof p.branch === "string" && p.branch) parts.push(p.branch);
  if (typeof p.target === "string" && typeof p.source !== "string" && p.target) parts.push(`→ ${p.target}`);
  if (typeof p.pulled === "number" && p.pulled > 0) parts.push(m.history.param.pulled(p.pulled));
  if (typeof p.pushed === "number" && p.pushed > 0) parts.push(m.history.param.pushed(p.pushed));
  if (typeof p.commits === "number" && p.commits > 0) parts.push(m.history.param.commits(p.commits));
  if (typeof p.files === "number" && p.files > 0) parts.push(m.history.param.files(p.files));
  if (typeof p.repo === "string") parts.push(p.repo);
  if (typeof p.remote === "string" && parts.length === 0) parts.push(p.remote);
  if (typeof p.sha === "string") parts.push(p.sha);
  return parts.join(m.common.dot);
}

export function HistoryPage() {
  const m = useT();
  const t = m.history;
  const projects = useAllProjects();
  const openProject = useUiStore((state) => state.openProject);
  const [projectId, setProjectId] = useState("__all__");
  const [scope, setScope] = useState<"all" | "issues">("all");
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const filterId = projectId === "__all__" ? null : projectId;

  const first = useAsync(() => api.getHistory({ limit: PAGE, before: null, projectId: filterId }), [filterId]);
  useEffect(() => {
    if (!first.data) return;
    setEntries(first.data);
    setHasMore(first.data.length === PAGE);
  }, [first.data]);

  const loadMore = async () => {
    const last = entries[entries.length - 1];
    if (!last) return;
    setLoadingMore(true);
    try {
      const next = await api.getHistory({ limit: PAGE, before: last.id, projectId: filterId });
      setEntries((previous) => [...previous, ...next]);
      setHasMore(next.length === PAGE);
    } catch (error) {
      notifyError(error);
    } finally {
      setLoadingMore(false);
    }
  };

  const clear = async () => {
    const result = await confirm({ title: m.confirm.clearHistory.title, body: m.confirm.clearHistory.body, level: "caution", confirmLabel: m.confirm.clearHistory.submit });
    if (!result.confirmed) return;
    try {
      await api.clearHistory();
      setEntries([]);
      setHasMore(false);
    } catch (error) {
      notifyError(error);
    }
  };

  const groups = useMemo(() => {
    const shown = scope === "issues" ? entries.filter((entry) => ISSUES.includes(entry.outcome)) : entries;
    const byDay = new Map<number, HistoryEntry[]>();
    for (const entry of shown) {
      const day = startOfDay(entry.at);
      byDay.set(day, [...(byDay.get(day) ?? []), entry]);
    }
    return [...byDay.entries()];
  }, [entries, scope]);

  return (
    <div className={styles.page}>
      <PageHeader
        title={t.title}
        subtitle={t.subtitle}
        actions={
          <>
            <Segmented
              size="sm"
              value={scope}
              onChange={setScope}
              options={[
                { value: "all", label: t.filterAll },
                { value: "issues", label: t.filterIssues },
              ]}
            />
            <Select
              size="sm"
              value={projectId}
              onChange={setProjectId}
              className={styles.projectSelect}
              options={[{ value: "__all__", label: t.allProjects }, ...projects.map((project) => ({ value: project.id, label: project.name }))]}
              ariaLabel={t.allProjects}
            />
            <Menu>
              <MenuTrigger asChild>
                <IconButton label={m.common.more} noTooltip>
                  <Icons.ellipsis size={16} />
                </IconButton>
              </MenuTrigger>
              <MenuContent>
                <MenuItem icon={<Icons.retry size={14} />} onSelect={first.reload}>
                  {m.common.refresh}
                </MenuItem>
                <MenuItem icon={<Icons.trash size={14} />} danger disabled={entries.length === 0} onSelect={() => void clear()}>
                  {t.clear}
                </MenuItem>
              </MenuContent>
            </Menu>
          </>
        }
      />
      <div className={`${styles.scroll} rv-sheet`}>
        {first.error ? (
          <ErrorNote error={first.error} actions={<Button size="sm" onClick={first.reload}>{m.common.retry}</Button>} />
        ) : first.loading && first.data === null ? (
          <div className={styles.center}>
            <Spinner size={18} />
          </div>
        ) : groups.length === 0 ? (
          <EmptyState icon={<Icons.history size={20} />} title={t.empty} body={t.emptyBody} />
        ) : (
          <>
            {groups.map(([day, items]) => (
              <section key={day} className={styles.group}>
                <h2 className={styles.day}>{formatDay(day, m)}</h2>
                <ul className={styles.list}>
                  {items.map((entry) => {
                    const visual = itemStateVisual[entry.outcome];
                    const Icon = visual.icon;
                    const exists = entry.projectId ? Boolean(useProjectStore.getState().byId[entry.projectId]) : false;
                    const detail = paramsText(entry, m);
                    return (
                      <li key={entry.id} className={styles.entry}>
                        <span className={clsx(styles.time, "tabular")}>{formatTime(entry.at, m)}</span>
                        <span className={styles.glyph} data-tone={visual.tone}>
                          <Icon size={11} strokeWidth={2.2} />
                        </span>
                        <div className={styles.main}>
                          <p className={styles.line}>
                            <span className={styles.action}>{t.action[entry.action]}</span>
                            {exists && entry.projectId ? (
                              <button type="button" className={styles.project} onClick={() => openProject(entry.projectId!)}>
                                {entry.projectName}
                              </button>
                            ) : (
                              <span className={styles.projectGone}>{entry.projectName}</span>
                            )}
                            {entry.outcome !== "succeeded" && (
                              <span className={styles.outcome} data-tone={visual.tone}>
                                {t.outcome[entry.outcome]}
                              </span>
                            )}
                          </p>
                          {(detail || entry.errorCode) && (
                            <p className={clsx(styles.sub, "truncate")}>
                              {entry.errorCode && <span className={styles.error}>{(m.error[entry.errorCode] ?? m.error.unknown).title}</span>}
                              {detail && <span className="mono">{detail}</span>}
                            </p>
                          )}
                        </div>
                      </li>
                    );
                  })}
                </ul>
              </section>
            ))}
            {hasMore && scope === "all" && (
              <div className={styles.more}>
                <Button size="sm" variant="ghost" loading={loadingMore} onClick={() => void loadMore()}>
                  {t.loadMore}
                </Button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
