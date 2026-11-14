import { useEffect, useRef } from "react";
import { api } from "../ipc/client";
import { useAppStore } from "../stores/app";
import { useOperationStore } from "../stores/operations";
import { useProjectStore } from "../stores/projects";

const FOCUS_THROTTLE = 20_000;

export function useAutoRefresh(enabled: boolean) {
  const autoRefresh = useAppStore((state) => state.settings?.general.autoRefresh);
  const lastRun = useRef(0);

  useEffect(() => {
    if (!enabled) return;
    lastRun.current = Date.now();
    const refresh = () => void useProjectStore.getState().refresh().catch(() => undefined);
    const roots = useAppStore.getState().settings?.roots ?? [];
    if (roots.some((root) => root.enabled)) {
      api
        .scanProjects()
        .then((workspace) => useAppStore.getState().applyWorkspace(workspace))
        .catch(refresh);
    } else {
      refresh();
    }
  }, [enabled]);

  useEffect(() => {
    if (!enabled || !autoRefresh?.onFocus) return;
    const onFocus = () => {
      if (Date.now() - lastRun.current < FOCUS_THROTTLE) return;
      lastRun.current = Date.now();
      void useProjectStore.getState().refresh().catch(() => undefined);
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [enabled, autoRefresh?.onFocus]);

  useEffect(() => {
    if (!enabled || !autoRefresh || autoRefresh.intervalMinutes <= 0) return;
    const timer = setInterval(() => {
      if (document.visibilityState !== "visible") return;
      lastRun.current = Date.now();
      if (!autoRefresh.fetch) {
        void useProjectStore.getState().refresh().catch(() => undefined);
        return;
      }
      const { byId, ids } = useProjectStore.getState();
      const busy = useOperationStore.getState().activity;
      const targets = ids.filter((id) => {
        const project = byId[id];
        return (
          project?.status &&
          project.settings.includeInBatch &&
          project.status.remotes.length > 0 &&
          !["missing", "not_initialized", "auth_error", "remote_not_found"].includes(project.status.state) &&
          !busy[id]
        );
      });
      if (targets.length === 0) return;
      void api
        .startOperation({ request: { kind: "fetch", items: targets.map((id) => ({ itemId: id, projectId: id })) } })
        .then((snapshot) => useOperationStore.getState().adopt(snapshot, "silent"))
        .catch(() => undefined);
    }, autoRefresh.intervalMinutes * 60_000);
    return () => clearInterval(timer);
  }, [enabled, autoRefresh]);
}
