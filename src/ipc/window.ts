import { isMac, previewPlatform } from "../lib/platform";
import { isTauri } from "./client";

export interface WindowFrame {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  close(): Promise<void>;
  isMaximized(): Promise<boolean>;
  onResized(handler: () => void): Promise<() => void>;
}

export const ownsWindowFrame = isTauri ? !isMac : (previewPlatform() ?? "macos") !== "macos";

function tauriFrame(): WindowFrame {
  let loading: Promise<import("@tauri-apps/api/window").Window> | null = null;
  const current = () => (loading ??= import("@tauri-apps/api/window").then((module) => module.getCurrentWindow()));
  return {
    minimize: () => current().then((frame) => frame.minimize()),
    toggleMaximize: () => current().then((frame) => frame.toggleMaximize()),
    close: () => current().then((frame) => frame.close()),
    isMaximized: () => current().then((frame) => frame.isMaximized()),
    onResized: (handler) => current().then((frame) => frame.onResized(() => handler())),
  };
}

function previewFrame(): WindowFrame {
  let maximized = false;
  const handlers = new Set<() => void>();
  return {
    minimize: async () => undefined,
    toggleMaximize: async () => {
      maximized = !maximized;
      handlers.forEach((handler) => handler());
    },
    close: async () => undefined,
    isMaximized: async () => maximized,
    onResized: async (handler) => {
      handlers.add(handler);
      return () => {
        handlers.delete(handler);
      };
    },
  };
}

export const windowFrame: WindowFrame = isTauri ? tauriFrame() : previewFrame();
