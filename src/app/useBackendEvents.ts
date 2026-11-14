import { useEffect } from "react";
import { api } from "../ipc/client";
import { useGithubStore } from "../stores/github";
import { useOperationStore } from "../stores/operations";
import { useProjectStore } from "../stores/projects";
import { useUpdateStore } from "../stores/update";

export function useBackendEvents(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    let disposed = false;
    const unlisten: (() => void)[] = [];
    const add = (promise: Promise<() => void>) =>
      promise.then((off) => {
        if (disposed) off();
        else unlisten.push(off);
      });

    void add(api.on("revia:project-updated", (project) => useProjectStore.getState().upsert(project)));
    void add(api.on("revia:projects-changed", (projects) => useProjectStore.getState().replaceAll(projects)));
    void add(api.on("revia:operation", (event) => useOperationStore.getState().handleEvent(event)));
    void add(api.on("revia:scan-progress", (progress) => useProjectStore.getState().setScan(progress)));
    void add(api.on("revia:github-login", (event) => useGithubStore.getState().handleLoginEvent(event)));
    void add(api.on("revia:update-progress", (progress) => useUpdateStore.getState().handleProgress(progress)));

    return () => {
      disposed = true;
      for (const off of unlisten) off();
    };
  }, [enabled]);
}
