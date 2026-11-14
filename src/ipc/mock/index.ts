import type { ReviaApi } from "../api";
import { previewPlatform } from "../../lib/platform";
import { appError } from "../errors";
import type {
  AppSettings,
  BranchInfo,
  BranchTargets,
  ChangedFile,
  CommitInfo,
  ConflictState,
  DiffHunk,
  DiffLine,
  FileDiff,
  GeneralSettings,
  GithubRepo,
  GithubStatus,
  HistoryAction,
  HistoryEntry,
  HistoryOutcome,
  ItemState,
  MergePreview,
  OperationItem,
  OperationKind,
  OperationPlan,
  OperationRequest,
  OperationSnapshot,
  PlanBlocker,
  PlanItem,
  PlanStep,
  PlanWarning,
  Project,
  ProjectDetail,
  ProjectStatus,
  RemoteRef,
  ResolvedTarget,
  RemovalPreview,
  ReviaEvents,
  RiskLevel,
  SyncState,
  TargetState,
  UpdateInfo,
  Workspace,
} from "../types";
import { ACCOUNT, HOME, fakeCommits, fakeSha, makeRepo, seedGithubRepos, seedHistory, seedRepos, type MockRepo } from "./seed";

const UI_STORAGE_KEY = "revia.mock.ui";

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
const clone = <T>(value: T): T => structuredClone(value);

export function createMockApi(): ReviaApi {
  const now = Date.now();
  const repos = seedRepos(now);
  let githubRepos = seedGithubRepos(now, repos);
  let history = seedHistory(now, repos);
  let updateInfo: UpdateInfo | null = null;
  let updateDownloading = false;
  let updateCancelled = false;
  let historyCounter = 2000;
  let operationCounter = 0;
  let loginCounter = 0;
  let github: GithubStatus = {
    cliInstalled: true,
    authenticated: true,
    account: { login: ACCOUNT, name: "Demo Dev", avatarUrl: null },
    gitProtocol: "https",
    missingScopes: [],
    tokenStorage: "keyring",
  };
  let pendingLogin: { id: string; cancelled: boolean } | null = null;
  let deleteScopeGranted = false;

  const settings: AppSettings = {
    roots: [
      { id: "root-projects", path: `${HOME}/Projects`, maxDepth: 4, enabled: true, exists: true },
      { id: "root-work", path: `${HOME}/Work`, maxDepth: 3, enabled: true, exists: true },
    ],
    excludedPaths: [`${HOME}/Projects/archive/tmp-playground`],
    general: {
      autoRefresh: { onFocus: true, intervalMinutes: 10, fetch: false },
      concurrency: 4,
      editor: null,
      safety: { allowForcePush: false, allowHardReset: false },
    },
    ui: readStoredUi(),
  };

  const listeners = new Map<string, Set<(payload: unknown) => void>>();
  const emit = <E extends keyof ReviaEvents>(event: E, payload: ReviaEvents[E]) => {
    for (const handler of listeners.get(event) ?? []) handler(clone(payload));
  };

  const operations = new Map<string, { snapshot: OperationSnapshot; cancelled: boolean; request: OperationRequest }>();
  const backgroundImages = new Map<string, ArrayBuffer>();

  const defaultRemote = (repo: MockRepo): string | null => {
    if (repo.settings.defaultRemote && repo.remotes.some((remote) => remote.name === repo.settings.defaultRemote)) {
      return repo.settings.defaultRemote;
    }
    if (repo.remotes.length === 1) return repo.remotes[0]!.name;
    return repo.remotes.some((remote) => remote.name === "origin") ? "origin" : null;
  };

  const remoteBranchExists = (repo: MockRepo, ref: RemoteRef) => repo.remoteBranches[ref.remote]?.includes(ref.branch) ?? false;

  const resolveTarget = (repo: MockRepo, kind: "push" | "pull"): TargetState => {
    const branch = repo.branch;
    const saved = branch ? (repo.settings.branchTargets[branch]?.[kind] ?? null) : null;
    if (!branch) return { target: null, saved, savedInvalid: null };
    if (saved) {
      if (!repo.remotes.some((remote) => remote.name === saved.remote)) {
        return { target: null, saved, savedInvalid: "remote_missing" };
      }
      const exists = remoteBranchExists(repo, saved);
      if (kind === "pull" && !exists) return { target: null, saved, savedInvalid: "branch_missing" };
      return { target: { ...saved, source: "saved", exists }, saved, savedInvalid: null };
    }
    const upstream = repo.branches.find((item) => item.name === branch)?.upstream ?? null;
    if (upstream && repo.remotes.some((remote) => remote.name === upstream.remote)) {
      return { target: { ...upstream, source: "upstream", exists: remoteBranchExists(repo, upstream) }, saved, savedInvalid: null };
    }
    const remote = defaultRemote(repo);
    if (!remote) return { target: null, saved, savedInvalid: null };
    const candidate = { remote, branch };
    if (remoteBranchExists(repo, candidate)) {
      return { target: { ...candidate, source: "remote_default", exists: true }, saved, savedInvalid: null };
    }
    if (kind === "push") return { target: { ...candidate, source: "inferred", exists: false }, saved, savedInvalid: null };
    return { target: null, saved, savedInvalid: null };
  };

  const changeCounts = (repo: MockRepo) => {
    const conflicted = repo.conflicts.length;
    const untracked = repo.files.filter((item) => item.kind === "untracked").length;
    const staged = repo.files.filter((item) => item.staged).length;
    return { staged, unstaged: repo.files.length - untracked - conflicted, untracked, conflicted, total: repo.files.length };
  };

  const deriveState = (repo: MockRepo, targets: BranchTargets): SyncState => {
    if (!repo.exists) return "missing";
    if (!repo.git) return "not_initialized";
    if (repo.conflicts.length > 0) return "conflicted";
    if (repo.inProgress) return "in_progress";
    if (repo.detached) return "detached";
    if (repo.remotes.length === 0) return "no_remote";
    if (repo.remoteError === "not_found") return "remote_not_found";
    if (repo.remoteError === "auth") return "auth_error";
    if (repo.files.length > 0) return "dirty";
    if (repo.remoteError === "unreachable") return "unreachable";
    if (repo.ahead > 0 && repo.behind > 0) return "diverged";
    if (repo.behind > 0) return "behind";
    if (repo.ahead > 0 && targets.push.target?.exists) return "ahead";
    if (!targets.push.target || !targets.push.target.exists) return "unpublished";
    return "synced";
  };

  const statusOf = (repo: MockRepo): ProjectStatus => {
    const targets: BranchTargets = { push: resolveTarget(repo, "push"), pull: resolveTarget(repo, "pull") };
    const upstream = repo.branch ? (repo.branches.find((item) => item.name === repo.branch)?.upstream ?? null) : null;
    const remoteName = targets.push.target?.remote ?? defaultRemote(repo);
    return {
      state: deriveState(repo, targets),
      branch: repo.branch,
      detached: repo.detached,
      unborn: repo.unborn,
      headSha: repo.headSha ? repo.headSha.slice(0, 7) : null,
      upstream,
      targets,
      ahead: !repo.git || !targets.push.target?.exists ? null : repo.ahead,
      behind: !repo.git || !targets.pull.target ? null : repo.behind,
      changes: changeCounts(repo),
      inProgress: repo.inProgress,
      remotes: repo.remotes,
      github: repo.remotes.find((remote) => remote.name === remoteName)?.github ?? repo.remotes[0]?.github ?? null,
      remoteError: repo.remoteError,
      lastCommitAt: repo.lastCommitAt,
      checkedAt: Date.now(),
      fetchedAt: repo.fetchedAt,
      error: null,
    };
  };

  const toProject = (repo: MockRepo): Project => ({
    id: repo.id,
    name: repo.name,
    path: repo.path,
    origin: repo.origin,
    markers: repo.markers,
    settings: clone(repo.settings),
    status: statusOf(repo),
  });

  const visibleRepos = () =>
    repos.filter((repo) => !settings.excludedPaths.some((path) => repo.path === path || repo.path.startsWith(`${path}/`)));
  const workspace = (): Workspace => ({ settings: clone(settings), projects: visibleRepos().map(toProject) });

  const find = (projectId: string): MockRepo => {
    const repo = repos.find((item) => item.id === projectId);
    if (!repo) throw appError("project_not_found", "プロジェクトが見つかりません");
    return repo;
  };

  const touch = (repo: MockRepo) => emit("revia:project-updated", toProject(repo));

  const githubTarget = (repo: MockRepo) =>
    (repo.remotes.find((remote) => remote.name === "origin" && remote.github) ?? repo.remotes.find((remote) => remote.github))?.github ?? null;

  const removalPreview = (repo: MockRepo): RemovalPreview => {
    const onRemote = new Set(Object.values(repo.remoteBranches).flat());
    const localOnlyBranches = repo.git ? repo.branches.map((branch) => branch.name).filter((name) => !onRemote.has(name)).sort() : [];
    const unpushedCommits = !repo.git ? 0 : repo.remotes.length > 0 ? repo.ahead : repo.commits.length;
    return {
      projectId: repo.id,
      path: repo.path,
      repository: repo.git,
      uncommittedFiles: repo.files.length,
      unpushedCommits,
      localOnlyBranches,
      stashes: repo.stashes.length,
      remotes: clone(repo.remotes),
      github: githubTarget(repo),
      nestedProjects: [],
      fingerprint: [repo.headSha, repo.branch, repo.files.length, repo.stashes.length, repo.remotes.map((remote) => remote.fetchUrl).join(","), unpushedCommits].join("|"),
    };
  };

  const branchInfos = (repo: MockRepo): BranchInfo[] => {
    const local: BranchInfo[] = repo.branches.map((branch) => ({
      name: branch.name,
      kind: "local",
      remote: null,
      shortName: branch.name,
      current: branch.name === repo.branch,
      sha: branch.sha,
      upstream: branch.upstream,
      upstreamGone: false,
      ahead: branch.upstream ? (branch.name === repo.branch ? repo.ahead : branch.ahead) : null,
      behind: branch.upstream ? (branch.name === repo.branch ? repo.behind : branch.behind) : null,
      mergedIntoCurrent: branch.name === repo.branch ? true : branch.merged,
      lastCommit: { subject: branch.subject, authorName: "Demo Dev", at: branch.at },
    }));
    const remote: BranchInfo[] = Object.entries(repo.remoteBranches).flatMap(([remoteName, names]) =>
      names.map((name) => ({
        name: `${remoteName}/${name}`,
        kind: "remote" as const,
        remote: remoteName,
        shortName: name,
        current: false,
        sha: fakeSha(),
        upstream: null,
        upstreamGone: false,
        ahead: null,
        behind: null,
        mergedIntoCurrent: name === repo.branch && repo.behind === 0,
        lastCommit: { subject: repo.commits[0]?.subject ?? "", authorName: "Sora Aoki", at: (repo.lastCommitAt ?? now) - 3_600_000 },
      })),
    );
    return [...local, ...remote];
  };

  const detailOf = (repo: MockRepo): ProjectDetail => ({
    project: toProject(repo),
    branches: branchInfos(repo),
    stashes: clone(repo.stashes),
  });

  const record = (
    repo: MockRepo | null,
    action: HistoryAction,
    outcome: HistoryOutcome,
    params: Record<string, string | number> = {},
    errorCode: HistoryEntry["errorCode"] = null,
    operationId: string | null = null,
    name?: string,
  ) => {
    historyCounter += 1;
    history.unshift({
      id: `h${historyCounter}`,
      at: Date.now(),
      operationId,
      projectId: repo?.id ?? null,
      projectName: repo?.name ?? name ?? "",
      action,
      outcome,
      params,
      errorCode,
    });
  };

  const fingerprintOf = (repo: MockRepo, extra = "") =>
    `${repo.headSha.slice(0, 8)}:${repo.branch ?? "-"}:${repo.files.length}:${repo.ahead}:${repo.behind}:${extra}`;

  const networkError = (repo: MockRepo) => {
    switch (repo.networkFailure) {
      case "auth":
        return appError("auth_failed", "GitHub の認証に失敗しました", "remote: Invalid username or token.\nfatal: Authentication failed for 'https://github.com/…'");
      case "unreachable":
        return appError("network_unreachable", "リモートに接続できません", "fatal: unable to access 'https://github.com/…': Could not resolve host: github.com");
      case "not_found":
        return appError("remote_not_found", "リモートのリポジトリが見つかりません", "remote: Repository not found.\nfatal: repository 'https://github.com/…' not found");
      default:
        return null;
    }
  };

  const baseBlocker = (repo: MockRepo, needsRemote: boolean): PlanBlocker | null => {
    if (!repo.exists) return "missing";
    if (!repo.git) return "not_initialized";
    if (repo.conflicts.length > 0) return "conflicted";
    if (repo.inProgress) return "in_progress";
    if (repo.detached) return "detached";
    if (needsRemote && repo.remotes.length === 0) return "no_remote";
    return null;
  };

  const targetBlocker = (state: TargetState): PlanBlocker | null =>
    state.savedInvalid ? "target_invalid" : state.target ? null : "target_unresolved";

  const pushStep = (repo: MockRepo, target: ResolvedTarget, force = false, setUpstream = false): PlanStep => ({
    code: "push",
    to: { remote: target.remote, branch: target.branch },
    commits: target.exists ? repo.ahead : null,
    createsRemoteBranch: !target.exists,
    force,
    setUpstream,
  });

  const planItem = (request: OperationRequest, index: number): PlanItem => {
    const raw = request.items[index]!;
    const blocked = (blocker: PlanBlocker, projectId: string | null = "projectId" in raw ? raw.projectId : null): PlanItem => ({
      itemId: raw.itemId,
      projectId,
      executable: false,
      blocker,
      steps: [],
      risk: "safe",
      warnings: [],
      fingerprint: null,
    });

    if (request.kind === "clone") {
      const item = request.items[index]!;
      const destination = `${item.parentDir}/${item.folderName}`;
      if (repos.some((repo) => repo.path === destination)) return blocked("destination_exists", null);
      return {
        itemId: item.itemId,
        projectId: null,
        executable: true,
        blocker: null,
        steps: [{ code: "clone", url: item.url, destination }],
        risk: "safe",
        warnings: [],
        fingerprint: null,
      };
    }

    const repo = find((raw as { projectId: string }).projectId);
    const status = statusOf(repo);
    const warnings: PlanWarning[] = [];
    let steps: PlanStep[] = [];
    let risk: RiskLevel = "safe";
    let blocker: PlanBlocker | null = null;

    switch (request.kind) {
      case "fetch": {
        blocker = !repo.exists ? "missing" : !repo.git ? "not_initialized" : repo.remotes.length === 0 ? "no_remote" : null;
        steps = [{ code: "fetch" }];
        break;
      }
      case "pull": {
        const item = request.items[index]!;
        blocker = baseBlocker(repo, true) ?? targetBlocker(status.targets.pull);
        const from = status.targets.pull.target;
        if (!blocker && from) {
          steps = [{ code: "fetch" }, { code: "pull", mode: item.mode, from: { remote: from.remote, branch: from.branch }, commits: repo.behind }];
          if (repo.behind === 0 && repo.incoming === 0) blocker = "nothing_to_do";
          else if (repo.ahead > 0 && repo.behind > 0 && item.mode === "ff_only") blocker = "diverged";
          else if (item.mode !== "ff_only") {
            if (repo.files.length > 0) blocker = "dirty_worktree";
            risk = item.mode === "rebase" ? "danger" : "caution";
            if (repo.conflictOnPull) warnings.push({ code: "conflicts_likely", paths: ["src/sync/engine.rs"] });
            if (item.mode === "rebase" && repo.ahead > 0) warnings.push({ code: "rewrites_published_history", commits: 0 });
          }
        }
        break;
      }
      case "push": {
        const item = request.items[index]!;
        blocker = baseBlocker(repo, true) ?? targetBlocker(status.targets.push);
        const to = status.targets.push.target;
        if (!blocker && to) {
          steps = [pushStep(repo, to, item.force, item.setUpstream)];
          if (to.exists && repo.ahead === 0) blocker = "nothing_to_do";
          if (!to.exists) warnings.push({ code: "creates_remote_branch", to: { remote: to.remote, branch: to.branch } });
          if (item.force) {
            risk = "danger";
            warnings.push({ code: "rewrites_published_history", commits: Math.max(repo.behind, 1) });
            if (!settings.general.safety.allowForcePush) blocker = "feature_disabled";
          }
        }
        break;
      }
      case "commit": {
        const item = request.items[index]!;
        blocker = !repo.exists ? "missing" : !repo.git ? "not_initialized" : repo.conflicts.length > 0 ? "conflicted" : null;
        const count = item.paths ? item.paths.length : repo.files.length;
        if (!blocker && count === 0) blocker = "nothing_to_do";
        if (!blocker && item.message.trim() === "") blocker = "invalid_input";
        steps = [{ code: "commit", files: count }];
        break;
      }
      case "sync": {
        blocker = baseBlocker(repo, true) ?? targetBlocker(status.targets.push) ?? (status.targets.pull.savedInvalid ? "target_invalid" : null);
        if (!blocker) {
          steps = [{ code: "fetch" }];
          const from = status.targets.pull.target;
          const to = status.targets.push.target!;
          if (repo.ahead > 0 && repo.behind > 0) {
            blocker = "diverged";
          } else {
            if (repo.behind > 0 && from) {
              steps.push({ code: "pull", mode: "ff_only", from: { remote: from.remote, branch: from.branch }, commits: repo.behind });
            }
            if (repo.ahead > 0 || !to.exists) {
              steps.push(pushStep(repo, to));
              if (!to.exists) warnings.push({ code: "creates_remote_branch", to: { remote: to.remote, branch: to.branch } });
            }
          }
          if (repo.files.length > 0) warnings.push({ code: "uncommitted_changes", files: repo.files.length });
          if (repo.fetchedAt === null || Date.now() - repo.fetchedAt > 24 * 3_600_000) {
            warnings.push({ code: "stale_remote", fetchedAt: repo.fetchedAt });
          }
        }
        break;
      }
      case "merge": {
        const item = request.items[index]!;
        blocker = baseBlocker(repo, false);
        const source = repo.branches.find((branch) => branch.name === item.source);
        const sourceCommits = source?.incoming ?? (item.source.includes("/") ? Math.max(repo.behind, 2) : 0);
        if (!blocker && !source && !item.source.includes("/")) blocker = "branch_not_found";
        if (!blocker && !repo.branches.some((branch) => branch.name === item.target)) blocker = "branch_not_found";
        if (!blocker) {
          if (item.target !== repo.branch) {
            steps.push({ code: "checkout", branch: item.target });
            if (repo.files.length > 0) blocker = "dirty_worktree";
          }
          const fastForward = repo.ahead === 0 && item.mode !== "no_ff";
          steps.push({ code: "merge", source: item.source, target: item.target, mode: item.mode, commits: sourceCommits, fastForward });
          if (sourceCommits === 0) blocker = "nothing_to_do";
          risk = "caution";
          if (source?.conflicts) warnings.push({ code: "conflicts_likely", paths: ["src/config.ts", "README.md"] });
          if (repo.files.length > 0) warnings.push({ code: "uncommitted_changes", files: repo.files.length });
        }
        break;
      }
      case "publish": {
        const item = request.items[index]!;
        if (!repo.exists) blocker = "missing";
        else if (!github.authenticated) blocker = "not_authenticated";
        else if (repo.remotes.some((remote) => remote.name === "origin")) blocker = "remote_exists";
        else if (githubRepos.some((entry) => entry.name === item.name && entry.owner === (item.owner ?? ACCOUNT))) blocker = "repo_exists";
        else if (!/^[A-Za-z0-9._-]+$/.test(item.name)) blocker = "invalid_input";
        const owner = item.owner ?? ACCOUNT;
        if (!repo.git) steps.push({ code: "init" });
        const firstCommit = (repo.unborn || !repo.git) && item.initialCommitMessage !== null;
        const ignored = firstCommit ? (item.ignore ?? []) : [];
        const keepsSecrets = !ignored.includes("/.env") || !ignored.includes("/config/credentials.json");
        if (ignored.length > 0) steps.push({ code: "ignore", patterns: ignored.length });
        if (firstCommit) steps.push({ code: "initial_commit", files: Math.max(1, 42 - ignored.length) });
        steps.push({ code: "create_repo", nameWithOwner: `${owner}/${item.name}`, visibility: item.visibility });
        steps.push({ code: "add_remote", name: "origin", url: `https://github.com/${owner}/${item.name}.git` });
        if ((repo.git && !repo.unborn) || item.initialCommitMessage) {
          steps.push({
            code: "push",
            to: { remote: "origin", branch: repo.branch ?? "main" },
            commits: null,
            createsRemoteBranch: true,
            force: false,
            setUpstream: true,
          });
        }
        risk = "caution";
        if (item.visibility === "public") warnings.push({ code: "public_repo" });
        if (!repo.git) {
          if (ignored.length === 0) warnings.push({ code: "no_gitignore" });
          if (keepsSecrets) {
            warnings.push({ code: "sensitive_files", paths: [".env", "config/credentials.json"].filter((path) => !ignored.includes(`/${path}`)) });
            risk = "danger";
          }
        }
        break;
      }
    }

    return {
      itemId: raw.itemId,
      projectId: repo.id,
      executable: blocker === null,
      blocker,
      steps: blocker && blocker !== "diverged" && blocker !== "nothing_to_do" && blocker !== "dirty_worktree" ? [] : steps,
      risk,
      warnings,
      fingerprint: fingerprintOf(repo, request.kind),
    };
  };

  const runItem = async (
    operationId: string,
    request: OperationRequest,
    index: number,
    update: (patch: Partial<OperationItem>) => void,
    isCancelled: () => boolean,
  ): Promise<void> => {
    const plan = planItem(request, index);
    const finish = (state: ItemState, patch: Partial<OperationItem> = {}) => update({ state, step: null, progress: null, ...patch });
    const raw = request.items[index]!;

    if (request.kind === "clone") {
      const item = request.items[index]!;
      if (!plan.executable) return finish("failed", { error: appError("destination_exists", "同じ名前のフォルダが既にあります") });
      update({ state: "running", step: "clone", progress: 0 });
      for (let tick = 1; tick <= 12; tick++) {
        await sleep(160);
        if (isCancelled()) return finish("cancelled");
        update({ progress: tick / 12 });
      }
      const created = makeRepo({ name: item.folderName, root: "manual" }, Date.now());
      created.path = `${item.parentDir}/${item.folderName}`;
      repos.push(created);
      record(created, "clone", "succeeded", { repo: item.url.replace(/^.*github\.com[/:]/, "").replace(/\.git$/, "") }, null, operationId);
      githubRepos = seedGithubRepos(now, repos).map((entry) => ({ ...entry, updatedAt: githubRepos.find((old) => old.id === entry.id)?.updatedAt ?? entry.updatedAt }));
      emit("revia:projects-changed", visibleRepos().map(toProject));
      return finish("succeeded", { projectId: created.id, completed: [{ code: "clone", url: item.url, destination: created.path }] });
    }

    const repo = find((raw as { projectId: string }).projectId);
    const completed: PlanStep[] = [];
    const done = (state: ItemState, patch: Partial<OperationItem> = {}, outcome: HistoryOutcome = state === "queued" || state === "running" ? "failed" : state) => {
      const params: Record<string, string | number> = {};
      if (repo.branch) params.branch = repo.branch;
      for (const step of completed) {
        if (step.code === "pull") params.pulled = step.commits;
        if (step.code === "push") params.pushed = step.commits ?? 0;
        if (step.code === "commit") params.files = step.files;
        if (step.code === "merge") {
          params.source = step.source;
          params.target = step.target;
          params.commits = step.commits;
        }
        if (step.code === "create_repo") params.repo = step.nameWithOwner;
      }
      record(repo, request.kind, outcome, params, patch.error?.code ?? null, operationId);
      touch(repo);
      finish(state, { completed: clone(completed), ...patch });
    };

    if (raw.fingerprint && raw.fingerprint !== plan.fingerprint) return done("needs_confirmation", { reason: "state_changed" });
    if (!plan.executable) {
      if (plan.blocker === "nothing_to_do") return done("skipped", { reason: "nothing_to_do" });
      if (plan.blocker === "diverged" && request.kind === "sync") {
      } else {
        return done("needs_confirmation", { reason: plan.blocker });
      }
    }

    const step = async (code: PlanStep["code"], ms: number, withProgress = false) => {
      update({ state: "running", step: code, progress: withProgress ? 0 : null });
      const ticks = withProgress ? 8 : 1;
      for (let tick = 1; tick <= ticks; tick++) {
        await sleep(ms / ticks);
        if (withProgress) update({ progress: tick / ticks });
      }
    };

    const doFetch = async (): Promise<boolean> => {
      await step("fetch", 500 + (repo.name.length % 5) * 180, true);
      if (isCancelled()) {
        done("cancelled");
        return false;
      }
      const failure = networkError(repo);
      if (failure) {
        repo.remoteError = repo.networkFailure;
        done("failed", { error: failure });
        return false;
      }
      repo.remoteError = null;
      repo.behind += repo.incoming;
      repo.incoming = 0;
      repo.fetchedAt = Date.now();
      completed.push({ code: "fetch" });
      return true;
    };

    const status = () => statusOf(repo);

    const doPull = async (mode: "ff_only" | "merge" | "rebase"): Promise<boolean> => {
      const from = status().targets.pull.target;
      if (!from || repo.behind === 0) return true;
      if (repo.ahead > 0 && mode === "ff_only") {
        done("needs_confirmation", { reason: "diverged" });
        return false;
      }
      await step("pull", 700);
      const commits = repo.behind;
      if (repo.ahead > 0 && repo.conflictOnPull) {
        repo.inProgress = mode === "rebase" ? "rebase" : "merge";
        repo.incomingLabel = `${from.remote}/${from.branch}`;
        repo.conflicts = [{ path: "src/sync/engine.rs", kind: "both_modified", hasMarkers: true }];
        repo.files = [{ path: "src/sync/engine.rs", oldPath: null, kind: "conflicted", staged: false, unstaged: true, additions: null, deletions: null, binary: false }];
        done("conflict");
        return false;
      }
      repo.behind = 0;
      if (repo.ahead > 0 && mode === "merge") repo.ahead += 1;
      repo.commits = [...fakeCommits(commits, Date.now(), commits), ...repo.commits];
      repo.headSha = repo.commits[0]!.sha;
      repo.lastCommitAt = Date.now();
      completed.push({ code: "pull", mode, from: { remote: from.remote, branch: from.branch }, commits });
      return true;
    };

    const doPush = async (force: boolean, setUpstream: boolean): Promise<boolean> => {
      const to = status().targets.push.target;
      if (!to) return true;
      if (to.exists && repo.ahead === 0) return true;
      await step("push", 900, true);
      if (isCancelled()) {
        done("cancelled");
        return false;
      }
      const failure = networkError(repo);
      if (failure) {
        repo.remoteError = repo.networkFailure;
        done("failed", { error: failure });
        return false;
      }
      if (repo.behind > 0 && !force) {
        done("failed", {
          error: appError("push_rejected", "リモートに新しいコミットがあるため Push できません", " ! [rejected]        main -> main (fetch first)\nerror: failed to push some refs"),
        });
        return false;
      }
      completed.push({ ...pushStep(repo, to, force, setUpstream) });
      const commits = repo.ahead;
      repo.ahead = 0;
      if (force) repo.behind = 0;
      repo.remoteBranches[to.remote] = [...new Set([...(repo.remoteBranches[to.remote] ?? []), to.branch])];
      for (const commit of repo.commits.slice(0, commits)) commit.pushed = true;
      const branch = repo.branches.find((item) => item.name === repo.branch);
      if (branch && setUpstream) branch.upstream = { remote: to.remote, branch: to.branch };
      return true;
    };

    switch (request.kind) {
      case "fetch": {
        if (await doFetch()) done("succeeded");
        return;
      }
      case "pull": {
        const item = request.items[index]!;
        if (!(await doFetch())) return;
        if (repo.behind === 0) return done("skipped", { reason: "nothing_to_do" });
        if (await doPull(item.mode)) done("succeeded");
        return;
      }
      case "push": {
        const item = request.items[index]!;
        if (await doPush(item.force, item.setUpstream)) done("succeeded");
        return;
      }
      case "commit": {
        const item = request.items[index]!;
        await step("commit", 420);
        const paths = item.paths ?? repo.files.map((entry) => entry.path);
        repo.files = repo.files.filter((entry) => !paths.includes(entry.path));
        const sha = fakeSha();
        repo.commits.unshift({
          sha,
          shortSha: sha.slice(0, 7),
          subject: item.message.split("\n")[0]!,
          body: item.message.split("\n").slice(1).join("\n").trim(),
          authorName: "Demo Dev",
          at: Date.now(),
          parents: [repo.headSha],
          refs: [],
          pushed: false,
        });
        repo.headSha = sha;
        repo.unborn = false;
        repo.ahead += 1;
        repo.lastCommitAt = Date.now();
        completed.push({ code: "commit", files: paths.length });
        return done("succeeded");
      }
      case "sync": {
        if (!(await doFetch())) return;
        if (repo.ahead > 0 && repo.behind > 0) return done("needs_confirmation", { reason: "diverged" });
        if (!(await doPull("ff_only"))) return;
        if (!(await doPush(false, false))) return;
        return done("succeeded");
      }
      case "merge": {
        const item = request.items[index]!;
        if (item.target !== repo.branch) {
          await step("checkout", 300);
          repo.branch = item.target;
          completed.push({ code: "checkout", branch: item.target });
        }
        await step("merge", 800);
        const source = repo.branches.find((branch) => branch.name === item.source);
        const commits = source?.incoming ?? Math.max(repo.behind, 2);
        if (source?.conflicts) {
          repo.inProgress = "merge";
          repo.incomingLabel = item.source;
          repo.conflicts = [
            { path: "src/config.ts", kind: "both_modified", hasMarkers: true },
            { path: "README.md", kind: "both_modified", hasMarkers: true },
          ];
          repo.files = [
            ...repo.conflicts.map((conflict) => ({ path: conflict.path, oldPath: null, kind: "conflicted" as const, staged: false, unstaged: true, additions: null, deletions: null, binary: false })),
            ...repo.files,
          ];
          return done("conflict");
        }
        repo.ahead += commits;
        if (source) {
          source.merged = true;
          source.incoming = 0;
        } else {
          repo.behind = 0;
        }
        repo.commits = [...fakeCommits(Math.max(commits, 1), Date.now(), commits), ...repo.commits];
        repo.headSha = repo.commits[0]!.sha;
        repo.lastCommitAt = Date.now();
        completed.push({ code: "merge", source: item.source, target: item.target, mode: item.mode, commits, fastForward: item.mode !== "no_ff" });
        return done("succeeded");
      }
      case "publish": {
        const item = request.items[index]!;
        const owner = item.owner ?? ACCOUNT;
        if (!repo.git) {
          await step("init", 300);
          repo.git = true;
          repo.branch = "main";
          repo.unborn = true;
          repo.markers = [".git", ...repo.markers];
          completed.push({ code: "init" });
        }
        if (repo.unborn && item.initialCommitMessage && item.ignore && item.ignore.length > 0) {
          await step("ignore", 200);
          completed.push({ code: "ignore", patterns: item.ignore.length });
        }
        if (repo.unborn && item.initialCommitMessage) {
          await step("initial_commit", 600);
          repo.commits = fakeCommits(1, Date.now(), 1);
          repo.commits[0]!.subject = item.initialCommitMessage;
          repo.headSha = repo.commits[0]!.sha;
          repo.unborn = false;
          repo.lastCommitAt = Date.now();
          repo.branches = [{ name: "main", sha: repo.headSha, upstream: null, ahead: 0, behind: 0, subject: item.initialCommitMessage, at: Date.now(), merged: true }];
          completed.push({ code: "initial_commit", files: 42 });
        }
        await step("create_repo", 900);
        if (isCancelled()) return done("cancelled");
        const url = `https://github.com/${owner}/${item.name}`;
        completed.push({ code: "create_repo", nameWithOwner: `${owner}/${item.name}`, visibility: item.visibility });
        await step("add_remote", 200);
        repo.remotes = [{ name: "origin", fetchUrl: `${url}.git`, pushUrl: `${url}.git`, github: { owner, repo: item.name, url } }];
        repo.remoteBranches = { origin: [] };
        completed.push({ code: "add_remote", name: "origin", url: `${url}.git` });
        if (!repo.unborn) {
          await step("push", 900, true);
          const branch = repo.branches.find((entry) => entry.name === repo.branch);
          if (branch) branch.upstream = { remote: "origin", branch: branch.name };
          repo.remoteBranches.origin = [repo.branch ?? "main"];
          repo.ahead = 0;
          repo.fetchedAt = Date.now();
          completed.push({ code: "push", to: { remote: "origin", branch: repo.branch ?? "main" }, commits: null, createsRemoteBranch: true, force: false, setUpstream: true });
        }
        githubRepos.unshift({
          id: `R_new_${item.name}`,
          nameWithOwner: `${owner}/${item.name}`,
          owner,
          name: item.name,
          description: item.description,
          visibility: item.visibility,
          url,
          cloneUrl: `${url}.git`,
          sshUrl: `git@github.com:${owner}/${item.name}.git`,
          defaultBranch: repo.branch ?? "main",
          updatedAt: Date.now(),
          fork: false,
          archived: false,
          localProjectIds: [repo.id],
        });
        return done("succeeded");
      }
    }
  };

  const startOperation = (request: OperationRequest): OperationSnapshot => {
    if (request.items.length === 0) throw appError("invalid_input", "対象がありません");
    operationCounter += 1;
    const operationId = `op-${operationCounter}`;
    const snapshot: OperationSnapshot = {
      operationId,
      kind: request.kind,
      state: "running",
      startedAt: Date.now(),
      finishedAt: null,
      items: request.items.map((item) => ({
        itemId: item.itemId,
        projectId: "projectId" in item ? item.projectId : null,
        state: "queued",
        step: null,
        progress: null,
        completed: [],
        reason: null,
        error: null,
      })),
    };
    const entry = { snapshot, cancelled: false, request };
    operations.set(operationId, entry);

    const run = async () => {
      await sleep(60);
      emit("revia:operation", { operationId, kind: "state", snapshot });
      let cursor = 0;
      const worker = async () => {
        while (cursor < request.items.length) {
          const index = cursor++;
          const item = snapshot.items[index]!;
          if (entry.cancelled) {
            Object.assign(item, { state: "cancelled" });
            emit("revia:operation", { operationId, kind: "item", item });
            continue;
          }
          const update = (patch: Partial<OperationItem>) => {
            Object.assign(item, patch);
            emit("revia:operation", { operationId, kind: "item", item });
          };
          try {
            await runItem(operationId, request, index, update, () => entry.cancelled);
          } catch (error) {
            update({ state: "failed", step: null, progress: null, error: appError("unknown", String(error)) });
          }
        }
      };
      await Promise.all(Array.from({ length: Math.min(settings.general.concurrency, request.items.length) }, worker));
      snapshot.state = "finished";
      snapshot.finishedAt = Date.now();
      emit("revia:operation", { operationId, kind: "state", snapshot });
    };
    void run();
    return clone(snapshot);
  };

  const fakeDiff = (path: string, kind: ChangedFile["kind"]): FileDiff => {
    if (/\.(png|jpg|ico|woff2)$/.test(path)) return { path, oldPath: null, binary: true, truncated: false, hunks: [] };
    const name = path.split("/").pop() ?? path;
    const context = (text: string, oldNo: number, newNo: number): DiffLine => ({ kind: "context", oldNo, newNo, text });
    const add = (text: string, newNo: number): DiffLine => ({ kind: "add", oldNo: null, newNo, text });
    const del = (text: string, oldNo: number): DiffLine => ({ kind: "del", oldNo, newNo: null, text });
    if (kind === "untracked" || kind === "added") {
      const lines = [`# ${name.replace(/\.[^.]+$/, "")}`, "", "このファイルは新しく追加されました。", "", "- 主要な操作の一覧", "- ショートカットの対応表", "- よくある質問"];
      return { path, oldPath: null, binary: false, truncated: false, hunks: [{ header: `@@ -0,0 +1,${lines.length} @@`, lines: lines.map((text, index) => add(text, index + 1)) }] };
    }
    if (kind === "deleted") {
      const lines = ["export function parseMarkdown(source: string) {", "  const tokens = tokenize(source);", "  return render(tokens);", "}"];
      return { path, oldPath: null, binary: false, truncated: false, hunks: [{ header: `@@ -1,${lines.length} +0,0 @@`, lines: lines.map((text, index) => del(text, index + 1)) }] };
    }
    const hunks: DiffHunk[] = [
      {
        header: "@@ -12,9 +12,11 @@ export function Toolbar({ editor }: Props) {",
        lines: [
          context("  const [open, setOpen] = useState(false);", 12, 12),
          context("  const items = useToolbarItems(editor);", 13, 13),
          context("", 14, 14),
          del("  const handleSelect = (id: string) => editor.run(id);", 15),
          add("  const handleSelect = useCallback(", 15),
          add("    (id: string) => editor.run(id, { source: \"toolbar\" }),", 16),
          add("    [editor],", 17),
          add("  );", 18),
          context("", 16, 19),
          context("  return (", 17, 20),
          context("    <div className={styles.toolbar} role=\"toolbar\">", 18, 21),
        ],
      },
      {
        header: "@@ -41,6 +43,7 @@ export function Toolbar({ editor }: Props) {",
        lines: [
          context("          key={item.id}", 41, 43),
          context("          label={item.label}", 42, 44),
          del("          onSelect={() => handleSelect(item.id)}", 43),
          add("          shortcut={item.shortcut}", 45),
          add("          onSelect={handleSelect}", 46),
          context("        />", 44, 47),
          context("      ))}", 45, 48),
        ],
      },
    ];
    return { path, oldPath: null, binary: false, truncated: false, hunks };
  };

  const mergePreview = (repo: MockRepo, sourceName: string, target: string): MergePreview => {
    const source = repo.branches.find((branch) => branch.name === sourceName);
    const count = source?.incoming ?? Math.max(repo.behind, 2);
    return {
      source: { name: sourceName, sha: source?.sha ?? fakeSha() },
      target: { name: target, sha: repo.branches.find((branch) => branch.name === target)?.sha ?? repo.headSha },
      needsCheckout: target !== repo.branch,
      worktreeDirty: repo.files.length > 0,
      upToDate: count === 0,
      fastForward: repo.ahead === 0 && count > 0,
      commits: fakeCommits(Math.min(count, 50), Date.now() - 3_600_000, count, sourceName.length),
      commitCount: count,
      filesChanged: count * 3,
      conflicts: source?.conflicts ? { status: "conflicts", paths: ["src/config.ts", "README.md"] } : { status: "none", paths: [] },
      blocker: baseBlocker(repo, false) ?? (count === 0 ? "nothing_to_do" : target !== repo.branch && repo.files.length > 0 ? "dirty_worktree" : null),
      fingerprint: fingerprintOf(repo, "merge"),
    };
  };

  const guardClean = (repo: MockRepo) => {
    if (repo.files.length > 0) throw appError("dirty_worktree", "未コミットの変更があります");
  };

  const api: ReviaApi = {
    async getAppState() {
      await sleep(260);
      return { ...workspace(), environment: environment(), recovery: null };
    },
    async getEnvironment() {
      return environment();
    },
    async pickFolder() {
      await sleep(200);
      const names = ["Playground", "OpenSource", "Clients", "Archive"];
      const used = new Set(settings.roots.map((root) => root.path));
      return names.map((name) => `${HOME}/${name}`).find((path) => !used.has(path)) ?? `${HOME}/Documents`;
    },
    async openProject() {
      await sleep(80);
    },
    async openUrl() {
      await sleep(80);
    },
    async listEditors() {
      return [
        { id: "vscode", name: "Visual Studio Code" },
        { id: "cursor", name: "Cursor" },
        { id: "zed", name: "Zed" },
        { id: "xcode", name: "Xcode" },
      ];
    },

    async pickBackgroundImage() {
      const file = await new Promise<File | null>((resolve) => {
        const input = document.createElement("input");
        input.type = "file";
        input.accept = "image/png,image/jpeg,image/webp,image/gif,image/avif";
        input.onchange = () => resolve(input.files?.[0] ?? null);
        input.oncancel = () => resolve(null);
        input.click();
      });
      if (!file) return null;
      if (file.size > 30 * 1024 * 1024) throw appError("invalid_input", "The image is larger than 30 MB");
      const extension = file.name.split(".").pop()?.toLowerCase() ?? "png";
      const id = `${crypto.randomUUID()}.${extension}`;
      backgroundImages.clear();
      backgroundImages.set(id, await file.arrayBuffer());
      return { id, name: file.name, mime: file.type || "image/png", size: file.size };
    },
    async readBackgroundImage({ id }) {
      const data = backgroundImages.get(id);
      if (!data) throw appError("path_not_found", "背景画像が見つかりません");
      return data;
    },
    async clearBackgroundImage() {
      backgroundImages.clear();
    },

    async updateUiSettings({ ui }) {
      settings.ui = clone(ui);
      try {
        localStorage.setItem(UI_STORAGE_KEY, JSON.stringify(ui));
      } catch {
      }
    },
    async updateGeneralSettings({ patch }) {
      const general: GeneralSettings = settings.general;
      if (patch.autoRefresh) Object.assign(general.autoRefresh, patch.autoRefresh);
      if (patch.safety) Object.assign(general.safety, patch.safety);
      if (patch.concurrency !== undefined) general.concurrency = Math.min(8, Math.max(1, patch.concurrency));
      if (patch.editor !== undefined) general.editor = patch.editor;
      return clone(settings);
    },
    async addScanRoot({ path, maxDepth }) {
      if (settings.roots.some((root) => root.path === path)) throw appError("already_registered", "このフォルダは登録済みです");
      const id = `root-${settings.roots.length + 1}-${Date.now().toString(36)}`;
      settings.roots.push({ id, path, maxDepth: maxDepth ?? 4, enabled: true, exists: true });
      emit("revia:scan-progress", { rootId: id, directories: 0, found: 0, done: false });
      await sleep(500);
      emit("revia:scan-progress", { rootId: id, directories: 64, found: 1, done: false });
      await sleep(500);
      const base = path.split("/").pop() ?? "new";
      for (const name of [`${base.toLowerCase()}-starter`, `${base.toLowerCase()}-notes`]) {
        const repo = makeRepo({ name, remote: null, commits: 6 }, Date.now());
        repo.path = `${path}/${name}`;
        repo.origin = { kind: "scan", rootId: id };
        repos.push(repo);
      }
      emit("revia:scan-progress", { rootId: id, directories: 131, found: 2, done: true });
      return workspace();
    },
    async updateScanRoot({ rootId, patch }) {
      const root = settings.roots.find((item) => item.id === rootId);
      if (!root) throw appError("invalid_input", "フォルダが見つかりません");
      Object.assign(root, patch);
      return workspace();
    },
    async removeScanRoot({ rootId }) {
      settings.roots = settings.roots.filter((root) => root.id !== rootId);
      for (let index = repos.length - 1; index >= 0; index--) {
        const origin = repos[index]!.origin;
        if (origin.kind === "scan" && origin.rootId === rootId) repos.splice(index, 1);
      }
      return workspace();
    },
    async addProject({ path }) {
      if (repos.some((repo) => repo.path === path)) throw appError("already_registered", "このフォルダは登録済みです");
      await sleep(300);
      const repo = makeRepo({ name: path.split("/").pop() ?? "project", root: "manual", git: false, markers: [] }, Date.now());
      repo.path = path;
      repos.push(repo);
      return workspace();
    },
    async excludeProject({ projectId }) {
      const repo = find(projectId);
      settings.excludedPaths.push(repo.path);
      return workspace();
    },
    async restoreExcludedPath({ path }) {
      settings.excludedPaths = settings.excludedPaths.filter((item) => item !== path);
      return workspace();
    },
    async updateProjectSettings({ projectId, patch }) {
      const repo = find(projectId);
      Object.assign(repo.settings, patch);
      touch(repo);
      return toProject(repo);
    },
    async setBranchTarget({ projectId, branch, kind, target }) {
      const repo = find(projectId);
      const current = repo.settings.branchTargets[branch] ?? { push: null, pull: null };
      repo.settings.branchTargets[branch] = { ...current, [kind]: target };
      await sleep(120);
      touch(repo);
      return toProject(repo);
    },

    async scanProjects() {
      for (const root of settings.roots.filter((item) => item.enabled)) {
        emit("revia:scan-progress", { rootId: root.id, directories: 0, found: 0, done: false });
        await sleep(450);
        emit("revia:scan-progress", { rootId: root.id, directories: 212, found: repos.filter((repo) => repo.origin.kind === "scan" && repo.origin.rootId === root.id).length, done: true });
      }
      return workspace();
    },
    async refreshProjects({ projectIds }) {
      await sleep(380);
      const targets = projectIds ? visibleRepos().filter((repo) => projectIds.includes(repo.id)) : visibleRepos();
      return targets.map(toProject);
    },

    async getProjectDetail({ projectId }) {
      await sleep(140);
      return detailOf(find(projectId));
    },
    async getChanges({ projectId }) {
      await sleep(120);
      return clone(find(projectId).files);
    },
    async getDiff({ projectId, path, source }) {
      await sleep(110);
      const repo = find(projectId);
      const kind = source.kind === "worktree" ? (repo.files.find((item) => item.path === path)?.kind ?? "modified") : "modified";
      return fakeDiff(path, kind);
    },
    async getCommitLog({ projectId, limit, skip }) {
      await sleep(160);
      return clone(find(projectId).commits.slice(skip, skip + limit));
    },
    async getCommitDetail({ projectId, sha }) {
      await sleep(140);
      const commit = find(projectId).commits.find((item) => item.sha === sha);
      if (!commit) throw appError("invalid_input", "コミットが見つかりません");
      const files: ChangedFile[] = [
        { path: "src/app/App.tsx", oldPath: null, kind: "modified", staged: false, unstaged: false, additions: 14, deletions: 6, binary: false },
        { path: "src/features/list/Row.tsx", oldPath: null, kind: "modified", staged: false, unstaged: false, additions: 22, deletions: 3, binary: false },
        { path: "src/features/list/sort.ts", oldPath: null, kind: "added", staged: false, unstaged: false, additions: 41, deletions: 0, binary: false },
      ];
      return { commit: clone(commit), files };
    },
    async getConflicts({ projectId }) {
      const repo = find(projectId);
      return conflictState(repo);
    },

    async planOperation({ request }): Promise<OperationPlan> {
      await sleep(180 + Math.min(request.items.length, 10) * 25);
      return { kind: request.kind as OperationKind, items: request.items.map((_, index) => planItem(request, index)) };
    },
    async startOperation({ request }) {
      return startOperation(request);
    },
    async cancelOperation({ operationId }) {
      const entry = operations.get(operationId);
      if (!entry || entry.snapshot.state === "finished") return;
      entry.cancelled = true;
      entry.snapshot.state = "cancelling";
      emit("revia:operation", { operationId, kind: "state", snapshot: entry.snapshot });
    },
    async getOperation({ operationId }) {
      const entry = operations.get(operationId);
      return entry ? clone(entry.snapshot) : null;
    },

    async initRepository({ projectId, initialBranch }) {
      const repo = find(projectId);
      await sleep(300);
      if (!repo.git) {
        repo.git = true;
        repo.unborn = true;
        repo.branch = initialBranch ?? "main";
        repo.markers = [".git", ...repo.markers];
        repo.files = [
          { path: "Cargo.toml", oldPath: null, kind: "untracked", staged: false, unstaged: false, additions: 12, deletions: 0, binary: false },
          { path: "src/main.rs", oldPath: null, kind: "untracked", staged: false, unstaged: false, additions: 28, deletions: 0, binary: false },
          { path: ".gitignore", oldPath: null, kind: "untracked", staged: false, unstaged: false, additions: 2, deletions: 0, binary: false },
        ];
        record(repo, "init", "succeeded", { branch: repo.branch });
      }
      touch(repo);
      return toProject(repo);
    },
    async createBranch({ projectId, name, checkout }) {
      const repo = find(projectId);
      if (!/^[^\s~^:?*[\\-][^\s~^:?*[\\]*$/.test(name)) throw appError("invalid_ref_name", "ブランチ名に使えない文字が含まれています");
      if (repo.branches.some((branch) => branch.name === name)) throw appError("branch_exists", "同じ名前のブランチがあります");
      await sleep(220);
      repo.branches.push({ name, sha: repo.headSha, upstream: null, ahead: 0, behind: 0, subject: repo.commits[0]?.subject ?? "", at: Date.now(), merged: true, incoming: 0 });
      if (checkout) {
        repo.branch = name;
        repo.ahead = 0;
        repo.behind = 0;
      }
      record(repo, "create_branch", "succeeded", { branch: name });
      touch(repo);
      return detailOf(repo);
    },
    async checkoutBranch({ projectId, branch, ifDirty }) {
      const repo = find(projectId);
      if (repo.files.length > 0 && ifDirty === "abort") throw appError("dirty_worktree", "未コミットの変更があります");
      await sleep(320);
      const localName = branch.includes("/") && !repo.branches.some((item) => item.name === branch) ? branch.split("/").slice(1).join("/") : branch;
      let target = repo.branches.find((item) => item.name === localName);
      if (!target) {
        const [remote] = branch.split("/");
        target = { name: localName, sha: fakeSha(), upstream: { remote: remote!, branch: localName }, ahead: 0, behind: 0, subject: repo.commits[0]?.subject ?? "", at: Date.now(), merged: false, incoming: 2 };
        repo.branches.push(target);
      }
      if (repo.files.length > 0 && ifDirty === "stash") {
        repo.stashes.unshift({ sha: fakeSha(), index: 0, message: `切り替え前の変更（${repo.branch ?? ""}）`, branch: repo.branch, at: Date.now() });
        repo.stashes.forEach((stash, index) => (stash.index = index));
        repo.files = [];
      }
      const previous = repo.branches.find((item) => item.name === repo.branch);
      if (previous) {
        previous.ahead = repo.ahead;
        previous.behind = repo.behind;
      }
      repo.branch = target.name;
      repo.detached = false;
      repo.ahead = target.ahead;
      repo.behind = target.behind;
      record(repo, "checkout", "succeeded", { branch: target.name });
      touch(repo);
      return detailOf(repo);
    },
    async renameBranch({ projectId, from, to }) {
      const repo = find(projectId);
      if (repo.branches.some((branch) => branch.name === to)) throw appError("branch_exists", "同じ名前のブランチがあります");
      const branch = repo.branches.find((item) => item.name === from);
      if (!branch) throw appError("branch_not_found", "ブランチが見つかりません");
      branch.name = to;
      if (repo.branch === from) repo.branch = to;
      if (repo.settings.branchTargets[from]) {
        repo.settings.branchTargets[to] = repo.settings.branchTargets[from]!;
        delete repo.settings.branchTargets[from];
      }
      record(repo, "rename_branch", "succeeded", { branch: to, source: from });
      touch(repo);
      return detailOf(repo);
    },
    async deleteBranch({ projectId, name, force }) {
      const repo = find(projectId);
      const branch = repo.branches.find((item) => item.name === name);
      if (!branch) throw appError("branch_not_found", "ブランチが見つかりません");
      if (name === repo.branch) throw appError("invalid_input", "現在のブランチは削除できません");
      if (!branch.merged && !force) throw appError("branch_not_merged", "まだ取り込まれていないコミットがあります", String(branch.incoming ?? 1));
      await sleep(200);
      repo.branches = repo.branches.filter((item) => item.name !== name);
      record(repo, "delete_branch", "succeeded", { branch: name });
      touch(repo);
      return detailOf(repo);
    },
    async deleteRemoteBranch({ projectId, remote, branch }) {
      const repo = find(projectId);
      await sleep(700);
      const failure = networkError(repo);
      if (failure) throw failure;
      repo.remoteBranches[remote] = (repo.remoteBranches[remote] ?? []).filter((name) => name !== branch);
      for (const local of repo.branches) {
        if (local.upstream?.remote === remote && local.upstream.branch === branch) local.upstream = null;
      }
      record(repo, "delete_remote_branch", "succeeded", { remote, branch });
      touch(repo);
      return detailOf(repo);
    },
    async setUpstream({ projectId, branch, upstream }) {
      const repo = find(projectId);
      const local = repo.branches.find((item) => item.name === branch);
      if (!local) throw appError("branch_not_found", "ブランチが見つかりません");
      local.upstream = upstream;
      record(repo, "set_upstream", "succeeded", { branch, target: upstream ? `${upstream.remote}/${upstream.branch}` : "" });
      touch(repo);
      return detailOf(repo);
    },
    async previewMerge({ projectId, source, target }) {
      await sleep(320);
      return mergePreview(find(projectId), source, target);
    },

    async addRemote({ projectId, name, url }) {
      const repo = find(projectId);
      if (repo.remotes.some((remote) => remote.name === name)) throw appError("remote_exists", "同じ名前のリモートがあります");
      const match = url.match(/github\.com[/:]([^/]+)\/([^/]+?)(\.git)?$/);
      repo.remotes.push({ name, fetchUrl: url, pushUrl: url, github: match ? { owner: match[1]!, repo: match[2]!, url: `https://github.com/${match[1]}/${match[2]}` } : null });
      repo.remoteBranches[name] = ["main"];
      record(repo, "add_remote", "succeeded", { remote: name });
      touch(repo);
      return detailOf(repo);
    },
    async setRemoteUrl({ projectId, name, url }) {
      const repo = find(projectId);
      const remote = repo.remotes.find((item) => item.name === name);
      if (!remote) throw appError("invalid_input", "リモートが見つかりません");
      remote.fetchUrl = url;
      remote.pushUrl = url;
      record(repo, "set_remote_url", "succeeded", { remote: name });
      touch(repo);
      return detailOf(repo);
    },
    async renameRemote({ projectId, from, to }) {
      const repo = find(projectId);
      const remote = repo.remotes.find((item) => item.name === from);
      if (!remote) throw appError("invalid_input", "リモートが見つかりません");
      remote.name = to;
      repo.remoteBranches[to] = repo.remoteBranches[from] ?? [];
      delete repo.remoteBranches[from];
      for (const branch of repo.branches) if (branch.upstream?.remote === from) branch.upstream.remote = to;
      record(repo, "rename_remote", "succeeded", { remote: to, source: from });
      touch(repo);
      return detailOf(repo);
    },
    async removeRemote({ projectId, name }) {
      const repo = find(projectId);
      repo.remotes = repo.remotes.filter((remote) => remote.name !== name);
      delete repo.remoteBranches[name];
      for (const branch of repo.branches) if (branch.upstream?.remote === name) branch.upstream = null;
      record(repo, "remove_remote", "succeeded", { remote: name });
      touch(repo);
      return detailOf(repo);
    },

    async markResolved({ projectId, paths }) {
      const repo = find(projectId);
      await sleep(150);
      repo.conflicts = repo.conflicts.filter((conflict) => !paths.includes(conflict.path));
      repo.files = repo.files.map((item) => (paths.includes(item.path) ? { ...item, kind: "modified", staged: true, unstaged: false, additions: 6, deletions: 2 } : item));
      touch(repo);
      return conflictState(repo);
    },
    async continueInProgress({ projectId }) {
      const repo = find(projectId);
      if (repo.conflicts.length > 0) throw appError("merge_conflict", "未解決のコンフリクトがあります");
      await sleep(400);
      repo.inProgress = null;
      repo.incomingLabel = null;
      repo.files = repo.files.filter((item) => !item.staged);
      repo.ahead += 1;
      repo.behind = 0;
      record(repo, "continue", "succeeded", { branch: repo.branch ?? "" });
      touch(repo);
      return detailOf(repo);
    },
    async abortInProgress({ projectId }) {
      const repo = find(projectId);
      if (!repo.inProgress) throw appError("invalid_input", "途中の操作はありません");
      await sleep(300);
      const paths = new Set(repo.conflicts.map((conflict) => conflict.path));
      repo.files = repo.files.filter((item) => !paths.has(item.path) && !item.staged);
      repo.conflicts = [];
      repo.inProgress = null;
      repo.incomingLabel = null;
      record(repo, "abort", "succeeded", { branch: repo.branch ?? "" });
      touch(repo);
      return detailOf(repo);
    },

    async previewRebase({ projectId, onto }) {
      const repo = find(projectId);
      await sleep(260);
      return {
        onto: { name: onto, sha: fakeSha() },
        branch: repo.branch ?? "",
        commits: repo.commits.slice(0, Math.max(repo.ahead, 2)),
        commitCount: Math.max(repo.ahead, 2),
        publishedCount: repo.ahead > 0 ? 0 : 2,
        worktreeDirty: repo.files.length > 0,
        blocker: baseBlocker(repo, false) ?? (repo.files.length > 0 ? "dirty_worktree" : null),
        fingerprint: fingerprintOf(repo, "rebase"),
      };
    },
    async rebase({ projectId, onto }) {
      const repo = find(projectId);
      guardClean(repo);
      await sleep(900);
      repo.behind = 0;
      record(repo, "rebase", "succeeded", { branch: repo.branch ?? "", target: onto });
      touch(repo);
      return detailOf(repo);
    },
    async cherryPick({ projectId, shas }) {
      const repo = find(projectId);
      guardClean(repo);
      await sleep(600);
      repo.ahead += shas.length;
      record(repo, "cherry_pick", "succeeded", { branch: repo.branch ?? "", commits: shas.length });
      touch(repo);
      return detailOf(repo);
    },
    async previewReset({ projectId, target, mode }) {
      const repo = find(projectId);
      await sleep(240);
      const index = Math.max(1, repo.commits.findIndex((commit) => commit.sha === target || commit.shortSha === target));
      return {
        target: { name: target.slice(0, 7), sha: target },
        branch: repo.branch ?? "",
        mode,
        commits: repo.commits.slice(0, index),
        commitCount: index,
        unpushedCount: Math.min(index, repo.ahead),
        discardedFiles: mode === "hard" ? repo.files.length : 0,
        blocker: mode === "hard" && !settings.general.safety.allowHardReset ? "feature_disabled" : baseBlocker(repo, false),
        fingerprint: fingerprintOf(repo, `reset-${mode}`),
      };
    },
    async reset({ projectId, target, mode }) {
      const repo = find(projectId);
      if (mode === "hard" && !settings.general.safety.allowHardReset) throw appError("feature_disabled", "Hard Reset は設定で無効になっています");
      await sleep(500);
      const index = Math.max(1, repo.commits.findIndex((commit) => commit.sha === target));
      repo.commits = repo.commits.slice(index);
      repo.headSha = repo.commits[0]?.sha ?? "";
      repo.ahead = Math.max(0, repo.ahead - index);
      if (mode === "hard") repo.files = [];
      record(repo, "reset", "succeeded", { branch: repo.branch ?? "", mode, sha: target.slice(0, 7) });
      touch(repo);
      return detailOf(repo);
    },
    async discardChanges({ projectId, paths, includeUntracked }) {
      const repo = find(projectId);
      await sleep(260);
      const before = repo.files.length;
      repo.files = repo.files.filter((item) => {
        const selected = paths === null || paths.includes(item.path);
        if (!selected) return true;
        return item.kind === "untracked" && !includeUntracked;
      });
      record(repo, "discard", "succeeded", { files: before - repo.files.length });
      touch(repo);
      return clone(repo.files);
    },
    async stashSave({ projectId, message, includeUntracked }) {
      const repo = find(projectId);
      if (repo.files.length === 0) throw appError("nothing_to_commit", "退避する変更がありません");
      await sleep(260);
      repo.stashes.unshift({ sha: fakeSha(), index: 0, message: message ?? `WIP on ${repo.branch ?? "HEAD"}`, branch: repo.branch, at: Date.now() });
      repo.stashes.forEach((stash, index) => (stash.index = index));
      repo.files = includeUntracked ? [] : repo.files.filter((item) => item.kind === "untracked");
      record(repo, "stash_save", "succeeded", { files: 1 });
      touch(repo);
      return detailOf(repo);
    },
    async stashApply({ projectId, stashSha, drop }) {
      const repo = find(projectId);
      const stash = repo.stashes.find((item) => item.sha === stashSha);
      if (!stash) throw appError("invalid_input", "退避した変更が見つかりません");
      await sleep(260);
      repo.files = [
        ...repo.files,
        { path: "src/app/layout.css", oldPath: null, kind: "modified", staged: false, unstaged: true, additions: 9, deletions: 4, binary: false },
      ];
      if (drop) {
        repo.stashes = repo.stashes.filter((item) => item.sha !== stashSha);
        repo.stashes.forEach((item, index) => (item.index = index));
      }
      record(repo, "stash_apply", "succeeded", {});
      touch(repo);
      return detailOf(repo);
    },
    async stashDrop({ projectId, stashSha }) {
      const repo = find(projectId);
      repo.stashes = repo.stashes.filter((item) => item.sha !== stashSha);
      repo.stashes.forEach((item, index) => (item.index = index));
      record(repo, "stash_drop", "succeeded", {});
      touch(repo);
      return detailOf(repo);
    },

    async githubStatus() {
      await sleep(180);
      return clone(github);
    },
    async githubLoginStart({ scopes }) {
      if (pendingLogin) throw appError("busy", "ログインの手続き中です");
      if (scopes && !github.authenticated) throw appError("not_authenticated", "Not signed in to GitHub");
      if (scopes && (scopes.length !== 1 || scopes[0] !== "delete_repo")) throw appError("invalid_input", "Unsupported scope");
      loginCounter += 1;
      const login = { id: `login-${loginCounter}`, cancelled: false };
      pendingLogin = login;
      void (async () => {
        await sleep(700);
        if (login.cancelled) return;
        emit("revia:github-login", { loginId: login.id, kind: "code", userCode: "7C3F-A91D", verificationUrl: "https://github.com/login/device" });
        await sleep(5200);
        if (login.cancelled) return;
        github = { cliInstalled: true, authenticated: true, account: { login: ACCOUNT, name: "Demo Dev", avatarUrl: null }, gitProtocol: "https", missingScopes: [], tokenStorage: "keyring" };
        if (scopes) deleteScopeGranted = true;
        pendingLogin = null;
        emit("revia:github-login", { loginId: login.id, kind: "succeeded", status: github });
      })();
      return { loginId: login.id };
    },
    async githubLoginCancel({ loginId }) {
      if (pendingLogin?.id === loginId) {
        pendingLogin.cancelled = true;
        pendingLogin = null;
        emit("revia:github-login", { loginId, kind: "cancelled" });
      }
    },
    async githubLogout() {
      await sleep(300);
      github = { cliInstalled: true, authenticated: false, account: null, gitProtocol: null, missingScopes: [], tokenStorage: null };
      return clone(github);
    },
    async githubListRepos() {
      if (!github.authenticated) throw appError("not_authenticated", "GitHub にログインしていません");
      await sleep(520);
      return clone(githubRepos) as GithubRepo[];
    },
    async githubListOwners() {
      if (!github.authenticated) throw appError("not_authenticated", "GitHub にログインしていません");
      await sleep(200);
      return [
        { login: ACCOUNT, kind: "user", avatarUrl: null },
        { login: "aozora-labs", kind: "organization", avatarUrl: null },
      ];
    },

    async getHistory({ limit, before, projectId }) {
      await sleep(160);
      const filtered = projectId ? history.filter((entry) => entry.projectId === projectId) : history;
      const start = before ? filtered.findIndex((entry) => entry.id === before) + 1 : 0;
      return clone(filtered.slice(start, start + limit));
    },
    async clearHistory() {
      history = [];
    },

    async scanIgnoreCandidates({ projectId }) {
      const repo = find(projectId);
      if (!repo.exists) throw appError("path_not_found", "Folder not found");
      await sleep(600);
      return {
        projectId,
        truncated: false,
        suggestions: [
          { pattern: "/.env", category: "secret", reason: "env_file", examples: [".env"], matches: 1 },
          { pattern: "/config/credentials.json", category: "secret", reason: "credential_file", examples: ["config/credentials.json"], matches: 1 },
          { pattern: "/scripts/deploy.sh", category: "secret", reason: "secret_in_content", examples: ["scripts/deploy.sh"], matches: 1 },
          { pattern: "/data/customers.csv", category: "personal", reason: "personal_data_file", examples: ["data/customers.csv"], matches: 1 },
          { pattern: "/data/export-2026.json", category: "personal", reason: "personal_in_content", examples: ["data/export-2026.json"], matches: 1 },
          { pattern: ".DS_Store", category: "system", reason: "os_file", examples: [".DS_Store", "src/.DS_Store", "docs/.DS_Store"], matches: 3 },
          { pattern: "node_modules/", category: "dependency", reason: "dependency_dir", examples: ["node_modules/"], matches: 1 },
          { pattern: "dist/", category: "build", reason: "build_output", examples: ["dist/"], matches: 1 },
          { pattern: "*.log", category: "scratch", reason: "log_file", examples: ["debug.log", "server/error.log"], matches: 2 },
          { pattern: "/test.txt", category: "scratch", reason: "scratch_file", examples: ["test.txt"], matches: 1 },
        ],
      };
    },

    async previewRemoval({ projectId }) {
      const repo = find(projectId);
      if (!repo.exists) throw appError("path_not_found", "Folder not found");
      await sleep(220);
      return removalPreview(repo);
    },
    async checkRepositoryDeletion({ projectId }) {
      const repo = find(projectId);
      const target = githubTarget(repo);
      if (!target) throw appError("invalid_input", "No GitHub repository");
      await sleep(700);
      const nameWithOwner = `${target.owner}/${target.repo}`;
      if (!github.authenticated) return { nameWithOwner, visibility: null, isFork: null, blocker: "not_authenticated" };
      return { nameWithOwner, visibility: "private", isFork: false, blocker: deleteScopeGranted ? null : "scope_missing" };
    },
    async removeGit({ projectId, fingerprint, confirm }) {
      const repo = find(projectId);
      if (!repo.git) throw appError("not_a_repository", "Not a repository");
      if (confirm !== repo.name) throw appError("invalid_input", "Confirmation does not match");
      if (fingerprint !== removalPreview(repo).fingerprint) throw appError("state_changed", "State changed");
      await sleep(500);
      repo.git = false;
      repo.branch = null;
      repo.unborn = false;
      repo.branches = [];
      repo.remotes = [];
      repo.remoteBranches = {};
      repo.files = [];
      repo.stashes = [];
      repo.commits = [];
      repo.ahead = 0;
      repo.behind = 0;
      repo.markers = repo.markers.filter((marker) => marker !== ".git");
      record(repo, "remove_git", "succeeded", { name: repo.name });
      touch(repo);
      return toProject(repo);
    },
    async deleteProject({ projectId, deleteRepository, fingerprint, confirm }) {
      const repo = find(projectId);
      const target = githubTarget(repo);
      if (deleteRepository && !target) throw appError("invalid_input", "No GitHub repository");
      const expected = deleteRepository && target ? `${target.owner}/${target.repo}` : repo.name;
      if (confirm !== expected) throw appError("invalid_input", "Confirmation does not match");
      if (fingerprint !== removalPreview(repo).fingerprint) throw appError("state_changed", "State changed");
      await sleep(deleteRepository ? 1300 : 600);
      if (deleteRepository) {
        if (!github.authenticated) throw appError("not_authenticated", "Not signed in to GitHub");
        if (!deleteScopeGranted) throw appError("scope_missing", "The delete_repo scope is missing", 'This API operation needs the "delete_repo" scope.');
        githubRepos = githubRepos.filter((item) => item.nameWithOwner !== expected);
        record(null, "delete_repository", "succeeded", { name: repo.name, repo: expected }, null, null, repo.name);
      }
      repos.splice(repos.indexOf(repo), 1);
      record(null, "delete_local", "succeeded", { name: repo.name }, null, null, repo.name);
      emit("revia:projects-changed", visibleRepos().map(toProject));
      return { repository: deleteRepository ? "deleted" : "skipped", local: "trashed", error: null, workspace: workspace() };
    },

    async checkUpdate() {
      await sleep(900);
      const mode = new URLSearchParams(window.location.search).get("update");
      if (mode === "error") throw appError("network_unreachable", "Could not reach GitHub");
      const version = mode === "current" ? "0.1.0" : "0.2.0";
      updateInfo = {
        currentVersion: "0.1.0",
        checkedAt: Date.now(),
        latest:
          mode === "none"
            ? null
            : {
                version,
                name: `Revia ${version}`,
                notes:
                  "## What's new\n\n- Branch targets can be set for several projects at once\n- Faster status refresh for large workspaces\n- The history page can be filtered by outcome\n\n## Fixes\n\n- Pull no longer stops on repositories without an upstream\n- The command palette keeps its position after closing a dialog",
                publishedAt: Date.now() - 2 * 86_400_000,
                url: `https://github.com/Shake1227/Revia/releases/tag/v${version}`,
                asset: { name: `Revia_${version}_universal.dmg`, size: 35_065_479, kind: "dmg", hasDigest: true },
              },
        available: mode !== "none" && mode !== "current",
      };
      return clone(updateInfo);
    },
    async downloadUpdate() {
      const asset = updateInfo?.latest?.asset;
      if (!asset) throw appError("invalid_input", "No update to download");
      if (updateDownloading) throw appError("busy", "A download is already running");
      updateDownloading = true;
      updateCancelled = false;
      try {
        for (let step = 1; step <= 24; step++) {
          await sleep(140);
          if (updateCancelled) throw appError("cancelled", "Cancelled");
          emit("revia:update-progress", { received: Math.round((asset.size * step) / 24), total: asset.size });
        }
        return { name: asset.name, size: asset.size, path: `${HOME}/Library/Caches/Revia/updates/${asset.name}`, verified: asset.hasDigest };
      } finally {
        updateDownloading = false;
      }
    },
    async cancelUpdateDownload() {
      updateCancelled = true;
    },
    async installUpdate() {
      await sleep(240);
    },

    async on(event, handler) {
      const set = listeners.get(event) ?? new Set();
      listeners.set(event, set);
      const wrapped = handler as (payload: unknown) => void;
      set.add(wrapped);
      return () => set.delete(wrapped);
    },
  };

  function conflictState(repo: MockRepo): ConflictState {
    return {
      operation: repo.inProgress,
      incoming: repo.incomingLabel,
      files: clone(repo.conflicts),
      canContinue: repo.inProgress !== null && repo.conflicts.length === 0,
    };
  }

  function environment() {
    return {
      platform: previewPlatform() ?? ("macos" as const),
      appVersion: "0.1.0",
      git: { installed: true, version: "2.54.0" },
      gh: { installed: true, version: "2.101.0" },
    };
  }

  return api;
}

function readStoredUi(): Record<string, unknown> {
  try {
    const raw = localStorage.getItem(UI_STORAGE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    return typeof parsed === "object" && parsed !== null ? (parsed as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

