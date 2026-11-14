import { useMemo } from "react";
import type { Project } from "../../ipc/types";
import { useAppStore } from "../../stores/app";
import { useProjectStore } from "../../stores/projects";
import { useUiStore } from "../../stores/ui";
import { countSummary, matchesFilter, matchesQuery, sortProjects } from "./model";

export function useAllProjects(): Project[] {
  const byId = useProjectStore((state) => state.byId);
  const ids = useProjectStore((state) => state.ids);
  return useMemo(() => ids.map((id) => byId[id]!).filter(Boolean), [byId, ids]);
}

export function useVisibleProjects() {
  const all = useAllProjects();
  const query = useUiStore((state) => state.query);
  const branchFilter = useUiStore((state) => state.branchFilter);
  const filter = useAppStore((state) => state.ui.filter);
  const sort = useAppStore((state) => state.ui.sort);
  const favoritesFirst = useAppStore((state) => state.ui.favoritesFirst);
  const favoritesOnly = useAppStore((state) => state.ui.favoritesOnly);

  const counts = useMemo(() => countSummary(all), [all]);
  const visible = useMemo(() => {
    const filtered = all.filter(
      (project) =>
        matchesFilter(project, filter) &&
        matchesQuery(project, query) &&
        (!favoritesOnly || project.settings.favorite) &&
        (branchFilter === null || project.status?.branch === branchFilter),
    );
    return sortProjects(filtered, sort.key, sort.dir, favoritesFirst);
  }, [all, filter, query, sort.key, sort.dir, favoritesFirst, favoritesOnly, branchFilter]);

  const branches = useMemo(() => {
    const tally = new Map<string, number>();
    for (const project of all) {
      const branch = project.status?.branch;
      if (branch) tally.set(branch, (tally.get(branch) ?? 0) + 1);
    }
    return [...tally.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).slice(0, 8).map(([name]) => name);
  }, [all]);

  return { all, visible, counts, branches };
}
