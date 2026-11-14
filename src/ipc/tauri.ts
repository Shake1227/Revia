import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ReviaApi } from "./api";
import { toAppError } from "./errors";

const toSnakeCase = (name: string) => name.replace(/[A-Z]/g, (char) => `_${char.toLowerCase()}`);

export function createTauriApi(): ReviaApi {
  return new Proxy({} as ReviaApi, {
    get(_target, property) {
      if (typeof property !== "string" || property === "then") return undefined;
      if (property === "on") {
        return (event: string, handler: (payload: unknown) => void) =>
          listen(event, (message) => handler(message.payload));
      }
      return (args?: Record<string, unknown>) =>
        invoke(toSnakeCase(property), args).catch((error: unknown) => {
          throw toAppError(error);
        });
    },
  });
}
