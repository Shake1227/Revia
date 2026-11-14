import type { ReviaApi } from "./api";
import { createTauriApi } from "./tauri";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function lazyApi(load: () => Promise<ReviaApi>): ReviaApi {
  let loading: Promise<ReviaApi> | null = null;
  return new Proxy({} as ReviaApi, {
    get(_target, property) {
      if (typeof property !== "string" || property === "then") return undefined;
      return (...args: unknown[]) =>
        (loading ??= load()).then((api) => (api[property as keyof ReviaApi] as (...a: unknown[]) => unknown)(...args));
    },
  });
}

export const api: ReviaApi = isTauri
  ? createTauriApi()
  : lazyApi(() => import("./mock").then((module) => module.createMockApi()));
