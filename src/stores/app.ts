import { create } from "zustand";
import { detectLanguage, setLanguage, type Language } from "../i18n";
import { api } from "../ipc/client";
import { toAppError } from "../ipc/errors";
import type {
  AppError,
  AppSettings,
  BackgroundImage,
  Environment,
  GeneralSettingsPatch,
  SettingsRecovery,
  Workspace,
} from "../ipc/types";
import { useProjectStore } from "./projects";

export type ThemeSetting = "system" | "dark" | "light" | "sky";
export type MotionSetting = "system" | "full" | "reduced";
export type Density = "comfortable" | "compact";
export type Typeface = "plex" | "mplus" | "system";
export type SortKey = "name" | "state" | "updated" | "path";
export type SummaryFilter = "all" | "synced" | "dirty" | "ahead" | "behind" | "unpublished" | "attention";
export type SceneSetting = "lake" | "hoshi" | "plain" | "image";

export interface ImageFrame {
  x: number;
  y: number;
  w: number;
  h: number;
  aspect: number;
  fit: "fill" | "show";
  edge: string | null;
}

export interface BackgroundSettings {
  scene: SceneSetting;
  image: BackgroundImage | null;
  frame: ImageFrame | null;
  dim: number;
  blur: number;
  panel: number;
  animate: boolean;
  timeAuto: boolean;
  hour: number;
}

export interface UiSettings {
  theme: ThemeSetting;
  language: Language;
  motion: MotionSetting;
  typeface: Typeface;
  background: BackgroundSettings;
  density: Density;
  sort: { key: SortKey; dir: "asc" | "desc" };
  filter: SummaryFilter;
  favoritesFirst: boolean;
  favoritesOnly: boolean;
  notifyOperationDone: boolean;
  notifyErrors: boolean;
  lastCloneDir: string | null;
  publishAutoIgnore: boolean;
  updateAutoCheck: boolean;
  updateCheckedAt: number | null;
  updateNotifiedVersion: string | null;
}

export const defaultUi: UiSettings = {
  theme: "system",
  language: detectLanguage(),
  motion: "system",
  typeface: "plex",
  background: { scene: "lake", image: null, frame: null, dim: 35, blur: 8, panel: 78, animate: true, timeAuto: true, hour: 19.5 },
  density: "comfortable",
  sort: { key: "name", dir: "asc" },
  filter: "all",
  favoritesFirst: true,
  favoritesOnly: false,
  notifyOperationDone: true,
  notifyErrors: true,
  lastCloneDir: null,
  publishAutoIgnore: true,
  updateAutoCheck: true,
  updateCheckedAt: null,
  updateNotifiedVersion: null,
};

const oneOf = <T extends string>(value: unknown, allowed: readonly T[], fallback: T): T =>
  allowed.includes(value as T) ? (value as T) : fallback;
const bool = (value: unknown, fallback: boolean) => (typeof value === "boolean" ? value : fallback);
const range = (value: unknown, min: number, max: number, fallback: number) =>
  typeof value === "number" && Number.isFinite(value) ? Math.min(max, Math.max(min, Math.round(value))) : fallback;

function normalizeImage(value: unknown): BackgroundImage | null {
  if (typeof value !== "object" || value === null) return null;
  const image = value as Record<string, unknown>;
  if (typeof image.id !== "string" || typeof image.mime !== "string") return null;
  return {
    id: image.id,
    name: typeof image.name === "string" ? image.name : image.id,
    mime: image.mime,
    size: typeof image.size === "number" ? image.size : 0,
  };
}

function normalizeFrame(raw: unknown): ImageFrame | null {
  if (typeof raw !== "object" || raw === null) return null;
  const source = raw as Record<string, unknown>;
  const unit = (value: unknown) => (typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= 1 ? value : null);
  const [x, y, w, h] = [unit(source.x), unit(source.y), unit(source.w), unit(source.h)];
  const aspect = typeof source.aspect === "number" && Number.isFinite(source.aspect) ? source.aspect : null;
  if (x === null || y === null || w === null || h === null || aspect === null) return null;
  if (w < 0.01 || h < 0.01 || x + w > 1.0001 || y + h > 1.0001 || aspect < 0.05 || aspect > 20) return null;
  const edge = typeof source.edge === "string" && source.edge.startsWith("data:image/") && source.edge.length < 40000 ? source.edge : null;
  return { x, y, w, h, aspect, fit: oneOf(source.fit, ["fill", "show"] as const, "fill"), edge };
}

function normalizeBackground(raw: unknown, legacyAurora: unknown): BackgroundSettings {
  const source = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
  const fallback = defaultUi.background;
  const image = normalizeImage(source.image);
  const legacy = source.scene === "yoi" || source.scene === "asagiri" ? source.scene : null;
  const scene = oneOf(source.scene, ["lake", "hoshi", "plain", "image"] as const, fallback.scene);
  const hour = typeof source.hour === "number" && Number.isFinite(source.hour) ? source.hour : legacy === "asagiri" ? 6.5 : fallback.hour;
  return {
    scene: scene === "image" && !image ? "lake" : scene,
    timeAuto: bool(source.timeAuto, legacy === null),
    hour: Math.min(23.999, Math.max(0, hour)),
    image,
    frame: image ? normalizeFrame(source.frame) : null,
    dim: range(source.dim, 0, 90, fallback.dim),
    blur: range(source.blur, 0, 40, fallback.blur),
    panel: range(source.panel, 50, 100, fallback.panel),
    animate: bool(source.animate, legacyAurora === false ? false : fallback.animate),
  };
}

export function normalizeUi(raw: Record<string, unknown> | null | undefined): UiSettings {
  const source = raw ?? {};
  const sort = (typeof source.sort === "object" && source.sort !== null ? source.sort : {}) as Record<string, unknown>;
  return {
    theme: oneOf(source.theme, ["system", "dark", "light", "sky"] as const, defaultUi.theme),
    language: oneOf(source.language, ["ja", "en"] as const, defaultUi.language),
    motion: oneOf(source.motion, ["system", "full", "reduced"] as const, defaultUi.motion),
    typeface: oneOf(source.typeface, ["plex", "mplus", "system"] as const, defaultUi.typeface),
    background: normalizeBackground(source.background, source.aurora),
    density: oneOf(source.density, ["comfortable", "compact"] as const, defaultUi.density),
    sort: {
      key: oneOf(sort.key, ["name", "state", "updated", "path"] as const, defaultUi.sort.key),
      dir: oneOf(sort.dir, ["asc", "desc"] as const, defaultUi.sort.dir),
    },
    filter: oneOf(
      source.filter,
      ["all", "synced", "dirty", "ahead", "behind", "unpublished", "attention"] as const,
      defaultUi.filter,
    ),
    favoritesFirst: bool(source.favoritesFirst, defaultUi.favoritesFirst),
    favoritesOnly: bool(source.favoritesOnly, defaultUi.favoritesOnly),
    notifyOperationDone: bool(source.notifyOperationDone, defaultUi.notifyOperationDone),
    notifyErrors: bool(source.notifyErrors, defaultUi.notifyErrors),
    lastCloneDir: typeof source.lastCloneDir === "string" ? source.lastCloneDir : null,
    publishAutoIgnore: bool(source.publishAutoIgnore, defaultUi.publishAutoIgnore),
    updateAutoCheck: bool(source.updateAutoCheck, defaultUi.updateAutoCheck),
    updateCheckedAt: typeof source.updateCheckedAt === "number" ? source.updateCheckedAt : null,
    updateNotifiedVersion: typeof source.updateNotifiedVersion === "string" ? source.updateNotifiedVersion : null,
  };
}

interface AppStore {
  phase: "loading" | "ready" | "failed";
  bootError: AppError | null;
  environment: Environment | null;
  settings: AppSettings | null;
  rawUi: Record<string, unknown>;
  ui: UiSettings;
  recovery: SettingsRecovery | null;
  boot(): Promise<void>;
  setUi(patch: Partial<UiSettings>): void;
  updateGeneral(patch: GeneralSettingsPatch): Promise<void>;
  applyWorkspace(workspace: Workspace): void;
  refreshEnvironment(): Promise<void>;
  dismissRecovery(): void;
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;

export const useAppStore = create<AppStore>((set, get) => ({
  phase: "loading",
  bootError: null,
  environment: null,
  settings: null,
  rawUi: {},
  ui: defaultUi,
  recovery: null,

  async boot() {
    set({ phase: "loading", bootError: null });
    try {
      const state = await api.getAppState();
      const ui = normalizeUi(state.settings.ui);
      setLanguage(ui.language);
      useProjectStore.getState().replaceAll(state.projects);
      set({
        phase: "ready",
        environment: state.environment,
        settings: state.settings,
        rawUi: state.settings.ui,
        ui,
        recovery: state.recovery,
      });
    } catch (error) {
      set({ phase: "failed", bootError: toAppError(error) });
    }
  },

  setUi(patch) {
    const ui = { ...get().ui, ...patch };
    const rawUi = { ...get().rawUi, ...ui };
    set({ ui, rawUi });
    if (patch.language) setLanguage(patch.language);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      api.updateUiSettings({ ui: get().rawUi }).catch(() => {
      });
    }, 400);
  },

  async updateGeneral(patch) {
    const settings = await api.updateGeneralSettings({ patch });
    set({ settings });
  },

  applyWorkspace(workspace) {
    set({ settings: workspace.settings });
    useProjectStore.getState().replaceAll(workspace.projects);
  },

  async refreshEnvironment() {
    const environment = await api.getEnvironment();
    set({ environment });
  },

  dismissRecovery() {
    set({ recovery: null });
  },
}));
