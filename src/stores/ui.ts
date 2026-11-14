import { create } from "zustand";
import type { GithubRepo } from "../ipc/types";

export type DetailTab = "changes" | "history" | "branches" | "settings";

export type View =
  | { name: "projects" }
  | { name: "project"; projectId: string; tab: DetailTab }
  | { name: "github" }
  | { name: "history" }
  | { name: "settings" };

export type GlobalDialog =
  | { type: "add" }
  | { type: "login" }
  | { type: "license" }
  | { type: "backdropFrame" }
  | { type: "clone"; repo: GithubRepo }
  | { type: "publish"; projectIds: string[] }
  | { type: "batchCommit"; projectIds: string[] }
  | { type: "batchMerge"; projectIds: string[] }
  | { type: "diverged"; projectId: string }
  | { type: "target"; projectId: string }
  | { type: "link"; projectId: string }
  | { type: "unlink"; projectId: string }
  | { type: "delete"; projectId: string }
  | { type: "merge"; projectId: string; source: string | null; target: string | null };

const depth: Record<View["name"], number> = { projects: 0, project: 1, github: 2, history: 3, settings: 4 };

interface UiStore {
  view: View;
  direction: 1 | -1;
  previous: View | null;
  query: string;
  branchFilter: string | null;
  paletteOpen: boolean;
  dialog: GlobalDialog | null;
  celebrations: number;

  navigate(view: View): void;
  openProject(projectId: string, tab?: DetailTab): void;
  setTab(tab: DetailTab): void;
  back(): void;
  setQuery(query: string): void;
  setBranchFilter(branch: string | null): void;
  setPaletteOpen(open: boolean): void;
  celebrate(): void;
  openDialog(dialog: GlobalDialog): void;
  closeDialog(): void;
}

export const useUiStore = create<UiStore>((set, get) => ({
  view: { name: "projects" },
  direction: 1,
  previous: null,
  query: "",
  branchFilter: null,
  paletteOpen: false,
  dialog: null,
  celebrations: 0,

  navigate(view) {
    const current = get().view;
    if (current.name === view.name && view.name !== "project") return;
    set({
      view,
      direction: depth[view.name] >= depth[current.name] ? 1 : -1,
      previous: view.name === "project" && current.name !== "project" ? current : get().previous,
    });
  },

  openProject(projectId, tab = "changes") {
    const current = get().view;
    if (current.name === "project" && current.projectId === projectId) {
      set({ view: { ...current, tab } });
      return;
    }
    set({
      view: { name: "project", projectId, tab },
      direction: 1,
      previous: current.name === "project" ? get().previous : current,
    });
  },

  setTab(tab) {
    const current = get().view;
    if (current.name === "project" && current.tab !== tab) set({ view: { ...current, tab } });
  },

  back() {
    const current = get().view;
    if (current.name !== "project") return;
    set({ view: get().previous ?? { name: "projects" }, direction: -1, previous: null });
  },

  setQuery(query) {
    set({ query });
  },

  setBranchFilter(branchFilter) {
    set({ branchFilter });
  },

  setPaletteOpen(open) {
    set({ paletteOpen: open });
  },

  celebrate() {
    set((state) => ({ celebrations: state.celebrations + 1 }));
  },

  openDialog(dialog) {
    set({ dialog, paletteOpen: false });
  },

  closeDialog() {
    set({ dialog: null });
  },
}));
