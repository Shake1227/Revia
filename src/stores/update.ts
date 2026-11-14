import { create } from "zustand";
import { t } from "../i18n";
import { api } from "../ipc/client";
import { toAppError } from "../ipc/errors";
import type { AppError, UpdateDownload, UpdateInfo, UpdateProgress } from "../ipc/types";
import { useAppStore } from "./app";
import { toast } from "./toasts";
import { useUiStore } from "./ui";

const DAY = 24 * 60 * 60 * 1000;

export type UpdatePhase = "idle" | "checking" | "downloading" | "ready" | "opened";

interface UpdateStore {
  info: UpdateInfo | null;
  phase: UpdatePhase;
  progress: UpdateProgress | null;
  download: UpdateDownload | null;
  error: AppError | null;
  check(): Promise<void>;
  autoCheck(): Promise<void>;
  startDownload(): Promise<void>;
  cancelDownload(): Promise<void>;
  install(): Promise<void>;
  handleProgress(progress: UpdateProgress): void;
}

export const useUpdateStore = create<UpdateStore>((set, get) => ({
  info: null,
  phase: "idle",
  progress: null,
  download: null,
  error: null,

  async check() {
    if (get().phase === "checking" || get().phase === "downloading") return;
    set({ phase: "checking", error: null });
    try {
      const info = await api.checkUpdate();
      set({ info, phase: "idle", download: null, progress: null });
      useAppStore.getState().setUi({ updateCheckedAt: info.checkedAt });
    } catch (error) {
      set({ phase: "idle", error: toAppError(error) });
    }
  },

  async autoCheck() {
    const { ui, setUi } = useAppStore.getState();
    if (!ui.updateAutoCheck || get().phase !== "idle") return;
    if (ui.updateCheckedAt !== null && Date.now() - ui.updateCheckedAt < DAY) return;
    set({ phase: "checking" });
    try {
      const info = await api.checkUpdate();
      set({ info, phase: "idle" });
      const version = info.available ? (info.latest?.version ?? null) : null;
      const notify = version !== null && version !== ui.updateNotifiedVersion;
      setUi({ updateCheckedAt: info.checkedAt, ...(notify ? { updateNotifiedVersion: version } : {}) });
      if (notify) {
        toast.info(t().settings.update.toast(version), {
          action: { label: t().settings.update.toastAction, run: () => useUiStore.getState().navigate({ name: "settings" }) },
          duration: 12000,
        });
      }
    } catch {
      set({ phase: "idle" });
    }
  },

  async startDownload() {
    if (get().phase !== "idle" || !get().info?.latest?.asset) return;
    set({ phase: "downloading", progress: null, error: null });
    try {
      const download = await api.downloadUpdate();
      set({ phase: "ready", download, progress: null });
    } catch (error) {
      const appError = toAppError(error);
      set({ phase: "idle", progress: null, error: appError.code === "cancelled" ? null : appError });
    }
  },

  async cancelDownload() {
    if (get().phase !== "downloading") return;
    await api.cancelUpdateDownload().catch(() => undefined);
  },

  async install() {
    if (get().phase !== "ready" && get().phase !== "opened") return;
    set({ error: null });
    try {
      await api.installUpdate();
      set({ phase: "opened" });
    } catch (error) {
      set({ error: toAppError(error) });
    }
  },

  handleProgress(progress) {
    if (get().phase === "downloading") set({ progress });
  },
}));
