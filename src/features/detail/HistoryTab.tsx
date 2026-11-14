import clsx from "clsx";
import { useEffect, useMemo, useState } from "react";
import { Button, IconButton } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { ErrorNote } from "../../components/ErrorNote";
import { Icons } from "../../components/icons";
import { Menu, MenuContent, MenuItem, MenuSeparator, MenuTrigger } from "../../components/Menu";
import { Select } from "../../components/Select";
import { Spinner } from "../../components/Spinner";
import { useT } from "../../i18n";
import { api } from "../../ipc/client";
import type { BranchInfo, CommitInfo, Project } from "../../ipc/types";
import { notifyError } from "../../lib/notify";
import { copyText } from "../../lib/platform";
import { formatDateTime, relativeTime } from "../../lib/time";
import { toast } from "../../stores/toasts";
import { cherryPickCommit, resetToCommit } from "./advanced";
import changeStyles from "./ChangesTab.module.css";
import { DiffView } from "./DiffView";
import styles from "./HistoryTab.module.css";
import { useAsync } from "./hooks";

const PAGE = 60;

interface HistoryTabProps {
  project: Project;
  signature: string;
  branches: BranchInfo[];
}

export function HistoryTab({ project, signature, branches }: HistoryTabProps) {
  const m = useT();
  const t = m.detail.history;
  const projectId = project.id;
  const current = project.status?.branch ?? null;
  const [rev, setRev] = useState<string | null>(null);
  const [commits, setCommits] = useState<CommitInfo[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [activeSha, setActiveSha] = useState<string | null>(null);
  const [activePath, setActivePath] = useState<string | null>(null);

  const first = useAsync(() => api.getCommitLog({ projectId, rev, limit: PAGE, skip: 0 }), [projectId, rev, signature]);
  useEffect(() => {
    if (!first.data) return;
    setCommits(first.data);
    setHasMore(first.data.length === PAGE);
    setActiveSha((previous) => (previous && first.data!.some((commit) => commit.sha === previous) ? previous : (first.data![0]?.sha ?? null)));
  }, [first.data]);

  const detail = useAsync(
    () => (activeSha ? api.getCommitDetail({ projectId, sha: activeSha }) : Promise.resolve(null)),
    [projectId, activeSha],
  );
  useEffect(() => {
    const files = detail.data?.files ?? [];
    setActivePath((previous) => (previous && files.some((file) => file.path === previous) ? previous : (files[0]?.path ?? null)));
  }, [detail.data]);

  const diff = useAsync(
    () =>
      activeSha && activePath && detail.data?.commit.sha === activeSha
        ? api.getDiff({ projectId, path: activePath, source: { kind: "commit", sha: activeSha } })
        : Promise.resolve(null),
    [projectId, activeSha, activePath, detail.data?.commit.sha],
  );

  const loadMore = async () => {
    setLoadingMore(true);
    try {
      const next = await api.getCommitLog({ projectId, rev, limit: PAGE, skip: commits.length });
      setCommits((previous) => [...previous, ...next]);
      setHasMore(next.length === PAGE);
    } catch (error) {
      notifyError(error);
    } finally {
      setLoadingMore(false);
    }
  };

  const branchOptions = useMemo(
    () => [
      { value: "__head__", label: current ?? "HEAD", group: m.branches.current },
      ...branches
        .filter((branch) => !(branch.kind === "local" && branch.name === current))
        .map((branch) => ({ value: branch.name, label: branch.name, group: branch.kind === "local" ? m.branches.local : m.branches.remote })),
    ],
    [branches, current, m],
  );

  const viewingCurrent = rev === null;
  const active = detail.data?.commit.sha === activeSha ? detail.data : null;

  if (first.error && commits.length === 0) {
    return <ErrorNote error={first.error} actions={<Button size="sm" onClick={first.reload}>{m.common.retry}</Button>} />;
  }
  if (first.loading && first.data === null) {
    return (
      <div className={changeStyles.loading}>
        <Spinner size={16} />
      </div>
    );
  }
  if (commits.length === 0 && viewingCurrent) {
    return <EmptyState compact icon={<Icons.commit size={20} />} title={t.empty} />;
  }

  return (
    <div className={changeStyles.changes}>
      <div className={changeStyles.side}>
        <div className={changeStyles.listHead}>
          <Select
            size="sm"
            mono
            value={rev ?? "__head__"}
            onChange={(value) => setRev(value === "__head__" ? null : value)}
            options={branchOptions}
            leading={<Icons.branch size={12} />}
            ariaLabel={m.detail.tabs.branches}
            className={styles.revSelect}
          />
        </div>
        <ul className={clsx(changeStyles.files, styles.commits)} role="listbox">
          {commits.map((commit) => {
            const isActive = commit.sha === activeSha;
            const refs = commit.refs.map((ref) => ref.replace(/^HEAD -> /, "").replace(/^tag: /, "")).filter((ref) => ref !== "HEAD");
            return (
              <li
                key={commit.sha}
                role="option"
                aria-selected={isActive}
                className={styles.commit}
                data-active={isActive || undefined}
                onClick={() => setActiveSha(commit.sha)}
              >
                <span className={styles.node} data-unpushed={commit.pushed === false || undefined} />
                <div className={styles.commitMain}>
                  <p className={clsx(styles.subject, "truncate")}>{commit.subject}</p>
                  <p className={clsx(styles.commitMeta, "truncate")}>
                    <span>{commit.authorName}</span>
                    <span>{relativeTime(commit.at, m)}</span>
                    {commit.pushed === false && <span className={styles.unpushed}>{t.unpushed}</span>}
                    {refs.slice(0, 2).map((ref) => (
                      <span key={ref} className={clsx(styles.ref, "mono")}>
                        {ref}
                      </span>
                    ))}
                  </p>
                </div>
              </li>
            );
          })}
          {hasMore && (
            <li className={styles.more}>
              <Button size="sm" variant="ghost" loading={loadingMore} onClick={() => void loadMore()}>
                {t.loadMore}
              </Button>
            </li>
          )}
        </ul>
      </div>

      <div className={changeStyles.diffPane}>
        {active ? (
          <>
            <div className={styles.detailHead}>
              <div className={styles.detailText}>
                <p className={clsx(styles.detailSubject, "selectable")}>{active.commit.subject}</p>
                {active.commit.body && <p className={clsx(styles.detailBody, "selectable")}>{active.commit.body}</p>}
                <p className={styles.detailMeta}>
                  <span className="mono selectable">{active.commit.shortSha}</span>
                  <span>{active.commit.authorName}</span>
                  <span>{formatDateTime(active.commit.at, m)}</span>
                  <span>{t.files(active.files.length)}</span>
                </p>
              </div>
              <Menu>
                <MenuTrigger asChild>
                  <IconButton label={m.common.more} size="sm" noTooltip>
                    <Icons.ellipsis size={15} />
                  </IconButton>
                </MenuTrigger>
                <MenuContent minWidth={250}>
                  <MenuItem
                    icon={<Icons.copy size={14} />}
                    onSelect={() => void copyText(active.commit.sha).then((ok) => ok && toast.success(m.common.copied))}
                  >
                    {t.copySha}
                  </MenuItem>
                  <MenuSeparator />
                  {!viewingCurrent && current && (
                    <MenuItem icon={<Icons.arrowDown size={14} />} onSelect={() => void cherryPickCommit(project, active.commit)}>
                      {t.cherryPick}
                    </MenuItem>
                  )}
                  {viewingCurrent && current && commits[0]?.sha !== active.commit.sha && (
                    <MenuItem icon={<Icons.undo size={14} />} danger onSelect={() => void resetToCommit(project, active.commit)}>
                      {t.resetTo}
                    </MenuItem>
                  )}
                </MenuContent>
              </Menu>
            </div>
            <div className={styles.fileTabs}>
              {active.files.map((file) => (
                <button
                  key={file.path}
                  type="button"
                  className={clsx(styles.fileTab, "mono")}
                  data-active={file.path === activePath || undefined}
                  onClick={() => setActivePath(file.path)}
                  title={file.path}
                >
                  <span className={changeStyles.mark} data-kind={file.kind}>
                    {file.kind === "added" ? "A" : file.kind === "deleted" ? "D" : file.kind === "renamed" ? "R" : "M"}
                  </span>
                  <span className="truncate">{file.path.split("/").pop()}</span>
                </button>
              ))}
            </div>
            <DiffView diff={diff.data} loading={diff.loading} error={diff.error} placeholder={m.detail.changes.pickFile} />
          </>
        ) : (
          <DiffView diff={null} loading={detail.loading} error={detail.error} placeholder={t.pickCommit} />
        )}
      </div>
    </div>
  );
}
