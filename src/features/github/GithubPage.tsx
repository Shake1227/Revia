import clsx from "clsx";
import { useEffect, useMemo, useState } from "react";
import { Avatar } from "../../components/Avatar";
import { Button, IconButton } from "../../components/Button";
import { EmptyState } from "../../components/EmptyState";
import { ErrorNote, Note } from "../../components/ErrorNote";
import { SearchField } from "../../components/Field";
import { Icons } from "../../components/icons";
import { Menu, MenuContent, MenuItem, MenuLabel, MenuTrigger } from "../../components/Menu";
import { Segmented } from "../../components/Segmented";
import { Spinner } from "../../components/Spinner";
import { t as messages, useT } from "../../i18n";
import { api } from "../../ipc/client";
import { notifyError } from "../../lib/notify";
import { relativeTime } from "../../lib/time";
import { useAppStore } from "../../stores/app";
import { confirm } from "../../stores/confirm";
import { useGithubStore } from "../../stores/github";
import { useUiStore } from "../../stores/ui";
import { useAllProjects } from "../projects/useVisibleProjects";
import { PageHeader } from "../shell/PageHeader";
import styles from "./GithubPage.module.css";

type RepoFilter = "all" | "local" | "remoteOnly";

export async function logoutGithub() {
  const m = messages();
  const result = await confirm({ title: m.confirm.logout.title, body: m.confirm.logout.body, level: "caution", confirmLabel: m.confirm.logout.submit });
  if (!result.confirmed) return;
  try {
    await useGithubStore.getState().logout();
  } catch (error) {
    notifyError(error);
  }
}

export function GithubPage() {
  const m = useT();
  const t = m.github;
  const status = useGithubStore((state) => state.status);
  const statusLoading = useGithubStore((state) => state.statusLoading);
  const repos = useGithubStore((state) => state.repos);
  const reposLoading = useGithubStore((state) => state.reposLoading);
  const reposError = useGithubStore((state) => state.reposError);
  const loadRepos = useGithubStore((state) => state.loadRepos);
  const loadStatus = useGithubStore((state) => state.loadStatus);
  const openDialog = useUiStore((state) => state.openDialog);
  const openProject = useUiStore((state) => state.openProject);
  const projects = useAllProjects();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<RepoFilter>("all");
  const authenticated = status?.authenticated ?? false;
  const platform = useAppStore((state) => state.environment?.platform ?? "macos");

  useEffect(() => {
    if (authenticated && repos === null) void loadRepos();
  }, [authenticated, repos, loadRepos]);

  const visible = useMemo(() => {
    const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    return (repos ?? []).filter((repo) => {
      if (filter === "local" && repo.localProjectIds.length === 0) return false;
      if (filter === "remoteOnly" && repo.localProjectIds.length > 0) return false;
      const haystack = `${repo.nameWithOwner}\n${repo.description ?? ""}`.toLowerCase();
      return words.every((word) => haystack.includes(word));
    });
  }, [repos, query, filter]);

  const publishable = projects.filter((project) => project.status && ["no_remote", "not_initialized"].includes(project.status.state));

  if (!status) {
    return (
      <div className={styles.page}>
        <PageHeader title={t.title} subtitle={t.subtitle} />
        <div className={styles.center}>{statusLoading && <Spinner size={18} />}</div>
      </div>
    );
  }

  if (!authenticated) {
    return (
      <div className={styles.page}>
        <PageHeader title={t.title} subtitle={t.subtitle} />
        <div className={styles.center}>
          <div className={styles.connect}>
            <span className={styles.connectIcon}>
              <Icons.github size={26} />
            </span>
            <h2>{status.cliInstalled ? t.connect.title : t.connect.cliMissing}</h2>
            <p>{status.cliInstalled ? t.connect.body : t.connect.cliMissingBody}</p>
            {status.cliInstalled ? (
              <Button variant="primary" size="lg" icon={<Icons.github size={15} />} onClick={() => openDialog({ type: "login" })}>
                {t.connect.action}
              </Button>
            ) : (
              <>
                <code className={clsx(styles.command, "mono selectable")}>{t.connect.install[platform]}</code>
                <Button loading={statusLoading} onClick={() => void loadStatus(true)}>
                  {t.connect.recheck}
                </Button>
              </>
            )}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={styles.page}>
      <PageHeader
        title={t.title}
        subtitle={t.subtitle}
        meta={repos ? m.common.items(repos.length) : null}
        actions={
          <>
            <SearchField value={query} onChange={setQuery} placeholder={t.repos.searchPlaceholder} clearLabel={m.common.close} className={styles.search} />
            <IconButton label={t.repos.refresh} disabled={reposLoading} onClick={() => void loadRepos(true)}>
              <Icons.retry size={15} className={reposLoading ? styles.spinning : undefined} />
            </IconButton>
            <Menu>
              <MenuTrigger asChild>
                <Button icon={<Icons.plus size={14} />} disabled={publishable.length === 0}>
                  {t.repos.new}
                </Button>
              </MenuTrigger>
              <MenuContent minWidth={240}>
                <MenuLabel>{m.action.publish}</MenuLabel>
                {publishable.slice(0, 12).map((project) => (
                  <MenuItem key={project.id} icon={<Icons.folder size={14} />} onSelect={() => openDialog({ type: "publish", projectIds: [project.id] })}>
                    {project.name}
                  </MenuItem>
                ))}
              </MenuContent>
            </Menu>
            <Menu>
              <MenuTrigger asChild>
                <button type="button" className={styles.account}>
                  <Avatar name={status.account?.login ?? "?"} src={status.account?.avatarUrl} size={22} />
                  <span className="truncate">{status.account?.login}</span>
                  <Icons.chevronDown size={13} />
                </button>
              </MenuTrigger>
              <MenuContent minWidth={200}>
                <MenuLabel>{t.account.connected}</MenuLabel>
                <MenuItem icon={<Icons.external size={14} />} onSelect={() => status.account && void api.openUrl({ url: `https://github.com/${status.account.login}` }).catch(notifyError)}>
                  {m.action.openOnGithub}
                </MenuItem>
                <MenuItem icon={<Icons.logOut size={14} />} danger onSelect={() => void logoutGithub()}>
                  {t.account.logout}
                </MenuItem>
              </MenuContent>
            </Menu>
          </>
        }
      />
      <div className={`${styles.scroll} rv-sheet`}>
        <div className={styles.toolbar}>
          <Segmented
            size="sm"
            value={filter}
            onChange={setFilter}
            options={(["all", "local", "remoteOnly"] as const).map((value) => ({ value, label: t.repos.filter[value] }))}
          />
        </div>
        {status.missingScopes.length > 0 && <Note tone="warning">{t.account.missingScopes(status.missingScopes.join(", "))}</Note>}
        {reposError && <ErrorNote error={reposError} actions={<Button size="sm" onClick={() => void loadRepos(true)}>{m.common.retry}</Button>} />}
        {repos === null && reposLoading ? (
          <div className={styles.center}>
            <Spinner size={18} />
          </div>
        ) : visible.length === 0 ? (
          !reposError && <EmptyState compact icon={<Icons.github size={20} />} title={repos && repos.length > 0 ? t.repos.noMatch : t.repos.empty} />
        ) : (
          <ul className={styles.list}>
            {visible.map((repo) => {
              const localId = repo.localProjectIds[0];
              return (
                <li key={repo.id} className={styles.repo} data-archived={repo.archived || undefined}>
                  <span className={styles.repoIcon}>{repo.visibility === "public" ? <Icons.globe size={15} /> : <Icons.lock size={15} />}</span>
                  <div className={styles.repoMain}>
                    <div className={styles.repoHead}>
                      <span className={clsx(styles.repoName, "truncate")}>
                        <span className={styles.owner}>{repo.owner}/</span>
                        {repo.name}
                      </span>
                      <span className={styles.badge}>{t.repos.visibility[repo.visibility]}</span>
                      {repo.fork && <span className={styles.badge}>{t.repos.fork}</span>}
                      {repo.archived && <span className={styles.badge}>{t.repos.archived}</span>}
                    </div>
                    <p className={clsx(styles.repoSub, "truncate")}>
                      {repo.description && <span>{repo.description}{m.common.dot}</span>}
                      {t.repos.updated(relativeTime(repo.updatedAt, m))}
                    </p>
                  </div>
                  {localId ? (
                    <>
                      <span className={styles.local}>
                        <Icons.check size={12} strokeWidth={2.2} />
                        {t.repos.local}
                      </span>
                      <Button size="sm" variant="ghost" onClick={() => openProject(localId)}>
                        {m.common.open}
                      </Button>
                    </>
                  ) : (
                    <Button size="sm" icon={<Icons.download size={13} />} onClick={() => openDialog({ type: "clone", repo })}>
                      {t.repos.clone}
                    </Button>
                  )}
                  <IconButton label={m.action.openOnGithub} size="sm" onClick={() => void api.openUrl({ url: repo.url }).catch(notifyError)}>
                    <Icons.external size={13} />
                  </IconButton>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
