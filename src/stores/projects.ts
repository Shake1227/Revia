import { create } from "zustand";
import { api } from "../ipc/client";
import type { Project, ScanProgress } from "../ipc/types";

interface ProjectStore {
  byId: Record<string, Project>;
  ids: string[];
  refreshing: boolean;
  lastRefreshedAt: number | null;
  scans: Record<string, ScanProgress>;
  selected: Record<string, true>;
  anchorId: string | null;
  cursorId: string | null;

  replaceAll(projects: Project[]): void;
  upsert(project: Project): void;
  refresh(projectIds?: string[] | null): Promise<void>;
  setScan(progress: ScanProgress): void;

  toggle(id: string): void;
  selectRange(toId: string, orderedIds: string[]): void;
  selectMany(ids: string[]): void;
  clearSelection(): void;
  setCursor(id: string | null): void;
}

export const useProjectStore = create<ProjectStore>((set, get) => ({
  byId: {},
  ids: [],
  refreshing: false,
  lastRefreshedAt: null,
  scans: {},
  selected: {},
  anchorId: null,
  cursorId: null,

  replaceAll(projects) {
    const byId: Record<string, Project> = {};
    const previous = get().byId;
    for (const project of projects) {
      byId[project.id] = project.status === null && previous[project.id]?.status
        ? { ...project, status: previous[project.id]!.status }
        : project;
    }
    const selected: Record<string, true> = {};
    for (const id of Object.keys(get().selected)) if (byId[id]) selected[id] = true;
    set({
      byId,
      ids: projects.map((project) => project.id),
      selected,
      cursorId: get().cursorId && byId[get().cursorId!] ? get().cursorId : null,
    });
  },

  upsert(project) {
    set((state) => ({
      byId: { ...state.byId, [project.id]: project },
      ids: state.byId[project.id] ? state.ids : [...state.ids, project.id],
    }));
  },

  async refresh(projectIds = null) {
    if (get().refreshing && projectIds === null) return;
    if (projectIds === null) set({ refreshing: true });
    try {
      const projects = await api.refreshProjects({ projectIds });
      set((state) => {
        const byId = { ...state.byId };
        for (const project of projects) if (byId[project.id]) byId[project.id] = project;
        return { byId, lastRefreshedAt: projectIds === null ? Date.now() : state.lastRefreshedAt };
      });
    } finally {
      if (projectIds === null) set({ refreshing: false });
    }
  },

  setScan(progress) {
    set((state) => {
      const scans = { ...state.scans };
      if (progress.done) delete scans[progress.rootId];
      else scans[progress.rootId] = progress;
      return { scans };
    });
  },

  toggle(id) {
    set((state) => {
      const selected = { ...state.selected };
      if (selected[id]) delete selected[id];
      else selected[id] = true;
      return { selected, anchorId: id };
    });
  },

  selectRange(toId, orderedIds) {
    const anchor = get().anchorId ?? toId;
    const from = orderedIds.indexOf(anchor);
    const to = orderedIds.indexOf(toId);
    if (from < 0 || to < 0) return get().toggle(toId);
    const [start, end] = from < to ? [from, to] : [to, from];
    set((state) => {
      const selected = { ...state.selected };
      for (const id of orderedIds.slice(start, end + 1)) selected[id] = true;
      return { selected };
    });
  },

  selectMany(ids) {
    const selected: Record<string, true> = {};
    for (const id of ids) selected[id] = true;
    set({ selected, anchorId: ids[0] ?? null });
  },

  clearSelection() {
    if (Object.keys(get().selected).length > 0) set({ selected: {}, anchorId: null });
  },

  setCursor(id) {
    if (get().cursorId !== id) set({ cursorId: id });
  },
}));

export const selectedIds = () => Object.keys(useProjectStore.getState().selected);
export const getProject = (id: string) => useProjectStore.getState().byId[id];
