import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../ipc/client";
import { toAppError } from "../../ipc/errors";
import type { AppError, Project } from "../../ipc/types";

export function statusSignature(project: Project | undefined): string {
  const status = project?.status;
  if (!status) return "none";
  return [
    status.state,
    status.branch,
    status.headSha,
    status.ahead,
    status.behind,
    status.changes.total,
    status.changes.conflicted,
    status.changes.staged,
    status.inProgress,
    status.remotes.map((remote) => `${remote.name}=${remote.fetchUrl}`).join(","),
    status.targets.push.target ? `${status.targets.push.target.remote}/${status.targets.push.target.branch}` : "-",
    status.targets.pull.target ? `${status.targets.pull.target.remote}/${status.targets.pull.target.branch}` : "-",
  ].join("|");
}

interface AsyncState<T> {
  data: T | null;
  loading: boolean;
  error: AppError | null;
  reload: () => void;
  setData: (data: T) => void;
}

export function useAsync<T>(load: () => Promise<T>, deps: unknown[]): AsyncState<T> {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<AppError | null>(null);
  const [tick, setTick] = useState(0);
  const latest = useRef(0);
  const loadRef = useRef(load);
  loadRef.current = load;

  useEffect(() => {
    const id = ++latest.current;
    setLoading(true);
    loadRef.current().then(
      (value) => {
        if (latest.current !== id) return;
        setData(value);
        setError(null);
        setLoading(false);
      },
      (reason) => {
        if (latest.current !== id) return;
        setError(toAppError(reason));
        setLoading(false);
      },
    );
  }, [...deps, tick]);

  const reload = useCallback(() => setTick((value) => value + 1), []);
  return { data, loading, error, reload, setData };
}

export function useProjectDetail(projectId: string, signature: string) {
  return useAsync(() => api.getProjectDetail({ projectId }), [projectId, signature]);
}
