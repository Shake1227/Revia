import { useEffect, useMemo, useRef } from "react";
import { Button, IconButton } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { SearchField } from "../../components/Field";
import { Icons } from "../../components/icons";
import { Kbd } from "../../components/Kbd";
import {
  Menu,
  MenuCheckItem,
  MenuContent,
  MenuItem,
  MenuLabel,
  MenuRadioGroup,
  MenuRadioItem,
  MenuSeparator,
  MenuTrigger,
} from "../../components/Menu";
import { Spinner } from "../../components/Spinner";
import { useT } from "../../i18n";
import { notifyError } from "../../lib/notify";
import { relativeTime } from "../../lib/time";
import { useAppStore, type Density, type SortKey } from "../../stores/app";
import { useOperationStore } from "../../stores/operations";
import { useProjectStore } from "../../stores/projects";
import { useUiStore } from "../../stores/ui";
import { PageHeader } from "../shell/PageHeader";
import { batchRequest } from "./model";
import { Onboarding } from "./Onboarding";
import { ProjectList } from "./ProjectList";
import styles from "./ProjectsPage.module.css";
import { SelectionBar } from "./SelectionBar";
import { SummaryStrip } from "./SummaryStrip";
import { useVisibleProjects } from "./useVisibleProjects";

export const SEARCH_INPUT_ID = "project-search";

export function ProjectsPage() {
  const m = useT();
  const { all, visible, counts, branches } = useVisibleProjects();
  const branchFilter = useUiStore((state) => state.branchFilter);
  const setBranchFilter = useUiStore((state) => state.setBranchFilter);
  const query = useUiStore((state) => state.query);
  const setQuery = useUiStore((state) => state.setQuery);
  const openDialog = useUiStore((state) => state.openDialog);
  const ui = useAppStore((state) => state.ui);
  const setUi = useAppStore((state) => state.setUi);
  const refreshing = useProjectStore((state) => state.refreshing);
  const lastRefreshedAt = useProjectStore((state) => state.lastRefreshedAt);
  const scans = useProjectStore((state) => state.scans);
  const prepare = useOperationStore((state) => state.prepare);
  const scrollRef = useRef<HTMLDivElement>(null);
  const visibleIds = useMemo(() => visible.map((project) => project.id), [visible]);

  const scanning = Object.values(scans);
  const scanFound = scanning.reduce((sum, scan) => sum + scan.found, 0);
  const roots = useAppStore((state) => state.settings?.roots.length ?? 0);
  const filtered = ui.filter !== "all" || query.trim() !== "" || ui.favoritesOnly || branchFilter !== null;

  useEffect(() => () => useProjectStore.getState().clearSelection(), []);

  const syncAll = () => {
    const targets = all.filter(
      (project) =>
        project.settings.includeInBatch &&
        project.status !== null &&
        !["missing", "not_initialized", "no_remote"].includes(project.status.state),
    );
    if (targets.length > 0) void prepare(batchRequest("sync", targets.map((project) => project.id)));
  };

  const clearFilters = () => {
    setQuery("");
    setBranchFilter(null);
    setUi({ filter: "all", favoritesOnly: false });
  };

  return (
    <div className={styles.page}>
      <PageHeader
        title={m.projects.title}
        meta={
          scanning.length > 0 ? (
            <span className={styles.scanning}>
              <Spinner size={12} />
              {m.projects.scanning(scanFound)}
            </span>
          ) : lastRefreshedAt ? (
            m.projects.lastRefreshed(relativeTime(lastRefreshedAt, m))
          ) : null
        }
        actions={
          <>
            <SearchField
              id={SEARCH_INPUT_ID}
              value={query}
              onChange={setQuery}
              placeholder={m.projects.searchPlaceholder}
              clearLabel={m.common.close}
              className={styles.search}
              trailing={<Kbd keys={["mod", "F"]} />}
              aria-label={m.common.search}
            />
            <IconButton
              label={m.action.refresh}
              shortcut={["mod", "R"]}
              onClick={() => void useProjectStore.getState().refresh().catch(notifyError)}
              disabled={refreshing}
            >
              <Icons.retry size={15} className={refreshing ? styles.spinning : undefined} />
            </IconButton>
            <Menu>
              <MenuTrigger asChild>
                <IconButton label={m.common.pair(m.projects.sort.label, m.projects.view.label)} noTooltip active={ui.favoritesOnly || branchFilter !== null}>
                  <Icons.sliders size={15} />
                </IconButton>
              </MenuTrigger>
              <MenuContent minWidth={210}>
                <MenuLabel>{m.projects.sort.label}</MenuLabel>
                <MenuRadioGroup
                  value={ui.sort.key}
                  onValueChange={(key) => setUi({ sort: { key: key as SortKey, dir: ui.sort.key === key && ui.sort.dir === "asc" ? "desc" : "asc" } })}
                >
                  {(["name", "state", "updated", "path"] as const).map((key) => (
                    <MenuRadioItem key={key} value={key}>
                      {m.projects.sort[key]}
                    </MenuRadioItem>
                  ))}
                </MenuRadioGroup>
                <MenuSeparator />
                <MenuItem
                  icon={<Icons.arrowDownUp size={14} />}
                  onSelect={() => setUi({ sort: { key: ui.sort.key, dir: ui.sort.dir === "asc" ? "desc" : "asc" } })}
                >
                  {ui.sort.dir === "asc" ? m.projects.sort.desc : m.projects.sort.asc}
                </MenuItem>
                <MenuSeparator />
                <MenuLabel>{m.projects.view.label}</MenuLabel>
                <MenuRadioGroup value={ui.density} onValueChange={(density) => setUi({ density: density as Density })}>
                  <MenuRadioItem value="comfortable">{m.projects.view.comfortable}</MenuRadioItem>
                  <MenuRadioItem value="compact">{m.projects.view.compact}</MenuRadioItem>
                </MenuRadioGroup>
                <MenuSeparator />
                <MenuCheckItem checked={ui.favoritesFirst} onCheckedChange={(favoritesFirst) => setUi({ favoritesFirst })}>
                  {m.projects.view.favoritesFirst}
                </MenuCheckItem>
                <MenuCheckItem checked={ui.favoritesOnly} onCheckedChange={(favoritesOnly) => setUi({ favoritesOnly })}>
                  {m.projects.filter.favorites}
                </MenuCheckItem>
                {branches.length > 1 && (
                  <>
                    <MenuSeparator />
                    <MenuLabel>{m.projects.filter.branch}</MenuLabel>
                    <MenuRadioGroup value={branchFilter ?? "__all__"} onValueChange={(value) => setBranchFilter(value === "__all__" ? null : value)}>
                      <MenuRadioItem value="__all__">{m.projects.filter.allBranches}</MenuRadioItem>
                      {branches.map((branch) => (
                        <MenuRadioItem key={branch} value={branch}>
                          <span className="mono">{branch}</span>
                        </MenuRadioItem>
                      ))}
                    </MenuRadioGroup>
                  </>
                )}
              </MenuContent>
            </Menu>
            <Button icon={<Icons.plus size={14} />} onClick={() => openDialog({ type: "add" })}>
              {m.projects.add}
            </Button>
            <Button variant="primary" icon={<Icons.refresh size={14} />} onClick={syncAll} disabled={all.length === 0}>
              {m.action.syncAll}
            </Button>
          </>
        }
      />

      {all.length > 0 && <SummaryStrip counts={counts} />}
      <div className={`${styles.scroll} rv-sheet`} ref={scrollRef}>
        <div className={styles.content}>
          {all.length === 0 && roots === 0 && scanning.length === 0 ? (
            <Onboarding />
          ) : all.length === 0 ? (
            <EmptyState
              icon={<Icons.folderSearch size={20} />}
              title={m.projects.empty.title}
              body={m.projects.empty.body}
              action={
                <Button variant="primary" icon={<Icons.folderPlus size={14} />} onClick={() => openDialog({ type: "add" })}>
                  {m.projects.empty.action}
                </Button>
              }
            />
          ) : visible.length === 0 ? (
            <EmptyState
              icon={<Icons.search size={20} />}
              title={m.projects.noMatch.title}
              body={m.projects.noMatch.body}
              action={filtered ? <Button onClick={clearFilters}>{m.projects.noMatch.action}</Button> : undefined}
            />
          ) : (
            <ProjectList projects={visible} compact={ui.density === "compact"} scrollRef={scrollRef} />
          )}
        </div>
      </div>

      <SelectionBar visibleIds={visibleIds} />
    </div>
  );
}
