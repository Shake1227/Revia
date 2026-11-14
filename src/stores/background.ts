import { create } from "zustand";
import { t } from "../i18n";
import { api } from "../ipc/client";
import type { BackgroundImage } from "../ipc/types";
import { notifyError } from "../lib/notify";
import { useAppStore } from "./app";
import { toast } from "./toasts";
import { useUiStore } from "./ui";

interface BackgroundStore {
  url: string | null;
  loadedId: string | null;
  busy: boolean;
  sync(image: BackgroundImage | null): Promise<void>;
  choose(): Promise<void>;
  remove(): Promise<void>;
}

export const useBackgroundStore = create<BackgroundStore>((set, get) => {
  const replace = (url: string | null, loadedId: string | null) => {
    const previous = get().url;
    if (previous) URL.revokeObjectURL(previous);
    set({ url, loadedId });
  };

  return {
    url: null,
    loadedId: null,
    busy: false,

    async sync(image) {
      if (!image) {
        if (get().url) replace(null, null);
        return;
      }
      if (get().loadedId === image.id) return;
      try {
        const data = await api.readBackgroundImage({ id: image.id });
        replace(URL.createObjectURL(new Blob([data], { type: image.mime })), image.id);
      } catch {
        replace(null, null);
      }
    },

    async choose() {
      if (get().busy) return;
      set({ busy: true });
      try {
        const image = await api.pickBackgroundImage({ filterLabel: t().settings.background.imageFilter });
        if (!image) return;
        const { ui, setUi } = useAppStore.getState();
        setUi({ background: { ...ui.background, scene: "image", image, frame: null } });
        await get().sync(image);
        toast.success(t().settings.background.imageSet);
        useUiStore.getState().openDialog({ type: "backdropFrame" });
      } catch (error) {
        notifyError(error);
      } finally {
        set({ busy: false });
      }
    },

    async remove() {
      const { ui, setUi } = useAppStore.getState();
      setUi({ background: { ...ui.background, scene: "lake", image: null, frame: null } });
      replace(null, null);
      try {
        await api.clearBackgroundImage();
      } catch (error) {
        notifyError(error);
      }
    },
  };
});
