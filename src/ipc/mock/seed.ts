import type {
  ChangedFile,
  CommitInfo,
  ConflictFile,
  GithubRepo,
  HistoryEntry,
  InProgressKind,
  ProjectOrigin,
  ProjectSettings,
  RemoteErrorKind,
  RemoteInfo,
  RemoteRef,
  StashEntry,
} from "../types";

export const HOME = "/Users/demo";
export const ACCOUNT = "demo-dev";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export interface MockBranch {
  name: string;
  sha: string;
  upstream: RemoteRef | null;
  ahead: number;
  behind: number;
  subject: string;
  at: number;
  merged: boolean;
  conflicts?: boolean;
  incoming?: number;
}

export interface MockRepo {
  id: string;
  name: string;
  path: string;
  origin: ProjectOrigin;
  markers: string[];
  exists: boolean;
  git: boolean;
  branch: string | null;
  detached: boolean;
  unborn: boolean;
  headSha: string;
  branches: MockBranch[];
  remoteBranches: Record<string, string[]>;
  remotes: RemoteInfo[];
  ahead: number;
  behind: number;
  incoming: number;
  files: ChangedFile[];
  conflicts: ConflictFile[];
  inProgress: InProgressKind | null;
  incomingLabel: string | null;
  remoteError: RemoteErrorKind | null;
  networkFailure: RemoteErrorKind | null;
  conflictOnPull: boolean;
  stashes: StashEntry[];
  commits: CommitInfo[];
  fetchedAt: number | null;
  lastCommitAt: number | null;
  settings: ProjectSettings;
}

let shaCounter = 0x3fa2c1;
export function fakeSha(): string {
  shaCounter = (shaCounter * 1103515245 + 12345) & 0x7fffffff;
  const head = shaCounter.toString(16).padStart(8, "0");
  shaCounter = (shaCounter * 1103515245 + 12345) & 0x7fffffff;
  return (head + shaCounter.toString(16).padStart(8, "0")).repeat(3).slice(0, 40);
}

const SUBJECTS = [
  "一覧の並び替えを修正",
  "README を更新",
  "依存関係を更新",
  "検索の絞り込みを追加",
  "設定の保存処理を整理",
  "エラー表示を分かりやすく",
  "テストを追加",
  "型定義を整理",
  "Fix flaky test on CI",
  "ダークテーマの配色を調整",
  "初期表示を高速化",
  "不要なログを削除",
  "API のレスポンス形式を統一",
  "入力チェックを追加",
  "Refactor storage layer",
  "アイコンを差し替え",
];

const AUTHORS = ["Demo Dev", "Sora Aoki", "Mio Tanaka"];

export function fakeCommits(count: number, newestAt: number, pushedFrom = 0, seed = 0): CommitInfo[] {
  const commits: CommitInfo[] = [];
  let at = newestAt;
  let parent = fakeSha();
  const shas = Array.from({ length: count }, () => fakeSha());
  for (let index = 0; index < count; index++) {
    const sha = shas[index]!;
    commits.push({
      sha,
      shortSha: sha.slice(0, 7),
      subject: SUBJECTS[(index * 5 + seed * 3) % SUBJECTS.length]!,
      body: index % 4 === 0 ? "画面の細部を見直し、関連する処理をまとめました。" : "",
      authorName: AUTHORS[(index + seed) % AUTHORS.length]!,
      at,
      parents: [shas[index + 1] ?? parent],
      refs: [],
      pushed: index >= pushedFrom,
    });
    at -= (3 + ((index * 7 + seed) % 40)) * HOUR;
    parent = sha;
  }
  return commits;
}

function file(path: string, kind: ChangedFile["kind"], additions = 0, deletions = 0, oldPath: string | null = null): ChangedFile {
  return {
    path,
    oldPath,
    kind,
    staged: false,
    unstaged: kind !== "untracked",
    additions: kind === "untracked" ? additions : additions,
    deletions,
    binary: /\.(png|jpg|ico|woff2)$/.test(path),
  };
}

function githubRemote(name: string, repo: string, owner = ACCOUNT): RemoteInfo {
  const url = `https://github.com/${owner}/${repo}.git`;
  return { name, fetchUrl: url, pushUrl: url, github: { owner, repo, url: `https://github.com/${owner}/${repo}` } };
}

const defaultSettings = (): ProjectSettings => ({
  favorite: false,
  includeInBatch: true,
  defaultRemote: null,
  defaultBranch: null,
  pullMode: "ff_only",
  branchTargets: {},
});

interface SeedInput {
  name: string;
  dir?: string;
  root?: "projects" | "work" | "manual";
  markers?: string[];
  git?: boolean;
  exists?: boolean;
  branch?: string | null;
  detached?: boolean;
  remote?: string | null;
  extraRemotes?: RemoteInfo[];
  ahead?: number;
  behind?: number;
  incoming?: number;
  files?: ChangedFile[];
  conflicts?: ConflictFile[];
  inProgress?: InProgressKind | null;
  incomingLabel?: string | null;
  networkFailure?: RemoteErrorKind | null;
  conflictOnPull?: boolean;
  published?: boolean;
  branches?: Partial<MockBranch>[];
  commits?: number;
  age?: number;
  fetchedAgo?: number | null;
  settings?: Partial<ProjectSettings>;
  stashes?: number;
}

let idCounter = 0;

export function makeRepo(input: SeedInput, now: number): MockRepo {
  idCounter += 1;
  const root = input.root ?? "projects";
  const base = root === "work" ? `${HOME}/Work` : root === "manual" ? `${HOME}/Desktop` : `${HOME}/Projects`;
  const path = `${base}/${input.dir ?? input.name}`;
  const git = input.git ?? true;
  const exists = input.exists ?? true;
  const branch = input.detached ? null : input.branch === undefined ? "main" : input.branch;
  const remoteName = input.remote === undefined ? input.name : input.remote;
  const remotes: RemoteInfo[] = git && remoteName ? [githubRemote("origin", remoteName), ...(input.extraRemotes ?? [])] : [];
  const published = input.published ?? true;
  const lastCommitAt = git && (input.commits ?? 24) > 0 ? now - (input.age ?? 2 * HOUR) : null;
  const ahead = input.ahead ?? 0;
  const commits = git ? fakeCommits(input.commits ?? 24, lastCommitAt ?? now, ahead, idCounter) : [];
  const headSha = commits[0]?.sha ?? "";
  const upstream = remotes.length > 0 && branch && published ? { remote: "origin", branch } : null;

  const branches: MockBranch[] = [];
  if (git && branch) {
    branches.push({
      name: branch,
      sha: headSha,
      upstream,
      ahead,
      behind: input.behind ?? 0,
      subject: commits[0]?.subject ?? "",
      at: lastCommitAt ?? now,
      merged: true,
    });
  }
  for (const extra of input.branches ?? []) {
    branches.push({
      name: extra.name ?? "feature",
      sha: fakeSha(),
      upstream: extra.upstream === undefined ? (remotes.length ? { remote: "origin", branch: extra.name ?? "feature" } : null) : extra.upstream,
      ahead: extra.ahead ?? 0,
      behind: extra.behind ?? 0,
      subject: extra.subject ?? SUBJECTS[(idCounter + branches.length * 3) % SUBJECTS.length]!,
      at: extra.at ?? now - (branches.length + 1) * 2 * DAY,
      merged: extra.merged ?? false,
      conflicts: extra.conflicts,
      incoming: extra.incoming ?? 3,
    });
  }

  const remoteBranches: Record<string, string[]> = {};
  for (const remote of remotes) {
    remoteBranches[remote.name] = remote.name === "origin"
      ? branches.filter((item) => item.upstream?.remote === "origin").map((item) => item.upstream!.branch)
      : ["main"];
    if (remote.name === "origin" && !remoteBranches[remote.name]!.includes("main") && published) {
      remoteBranches[remote.name]!.unshift("main");
    }
  }

  if (commits[0] && branch) commits[0].refs = [branch, ...(upstream ? [`origin/${branch}`] : [])].slice(0, ahead > 0 ? 1 : 2);
  if (ahead > 0 && commits[ahead] && upstream) commits[ahead]!.refs = [`origin/${upstream.branch}`];

  const stashes: StashEntry[] = Array.from({ length: input.stashes ?? 0 }, (_, index) => ({
    sha: fakeSha(),
    index,
    message: index === 0 ? "作業中の変更を退避" : "WIP: レイアウト調整",
    branch,
    at: now - (index + 1) * 3 * DAY,
  }));

  return {
    id: `p${idCounter.toString(16).padStart(4, "0")}${input.name.length.toString(16)}a7c${idCounter}e`.slice(0, 16).padEnd(16, "0"),
    name: input.name,
    path,
    origin: root === "manual" ? { kind: "manual" } : { kind: "scan", rootId: root === "work" ? "root-work" : "root-projects" },
    markers: input.markers ?? (git ? [".git", "package.json"] : ["package.json"]),
    exists,
    git,
    branch,
    detached: input.detached ?? false,
    unborn: git && commits.length === 0,
    headSha,
    branches,
    remoteBranches,
    remotes,
    ahead,
    behind: input.behind ?? 0,
    incoming: input.incoming ?? 0,
    files: input.files ?? [],
    conflicts: input.conflicts ?? [],
    inProgress: input.inProgress ?? null,
    incomingLabel: input.incomingLabel ?? null,
    remoteError: input.networkFailure ?? null,
    networkFailure: input.networkFailure ?? null,
    conflictOnPull: input.conflictOnPull ?? false,
    stashes,
    commits,
    fetchedAt: input.fetchedAgo === null || remotes.length === 0 ? null : now - (input.fetchedAgo ?? 12 * MINUTE),
    lastCommitAt,
    settings: { ...defaultSettings(), ...input.settings },
  };
}

export function seedRepos(now: number): MockRepo[] {
  idCounter = 0;
  return [
    makeRepo(
      {
        name: "revia",
        age: 40 * MINUTE,
        settings: { favorite: true },
        branches: [
          { name: "feature/command-palette", ahead: 2, incoming: 2 },
          { name: "fix/scan-depth", merged: true, incoming: 0 },
        ],
        stashes: 1,
      },
      now,
    ),
    makeRepo(
      {
        name: "aozora-notes",
        age: 3 * HOUR,
        files: [
          file("src/editor/Toolbar.tsx", "modified", 24, 9),
          file("src/editor/shortcuts.ts", "modified", 6, 2),
          file("src/styles/editor.css", "modified", 31, 12),
          file("docs/キーボード操作.md", "untracked", 48, 0),
          file("public/og-image.png", "added"),
          file("src/legacy/markdown.ts", "deleted", 0, 86),
        ],
        settings: { favorite: true },
        branches: [{ name: "feature/export-pdf", ahead: 4, conflicts: true, incoming: 4 }],
      },
      now,
    ),
    makeRepo(
      {
        name: "kumo-api",
        branch: "develop",
        ahead: 3,
        age: 25 * MINUTE,
        markers: [".git", "go.mod"],
        settings: { branchTargets: { develop: { push: { remote: "origin", branch: "develop" }, pull: null } } },
        branches: [{ name: "main", merged: false, behind: 0, incoming: 5 }, { name: "feature/auth", ahead: 1, conflicts: true, incoming: 6 }],
      },
      now,
    ),
    makeRepo({ name: "hoshi-ui", behind: 4, age: 2 * DAY, fetchedAgo: 4 * MINUTE }, now),
    makeRepo({ name: "tsubame-cli", ahead: 2, behind: 1, age: 5 * HOUR, markers: [".git", "Cargo.toml"], conflictOnPull: true }, now),
    makeRepo(
      {
        name: "portfolio-site",
        ahead: 1,
        age: 50 * MINUTE,
        files: [file("src/pages/works.astro", "modified", 12, 4), file("src/content/works/revia.md", "untracked", 30, 0)],
      },
      now,
    ),
    makeRepo({ name: "dotfiles", age: 9 * DAY, markers: [".git", "Makefile"], fetchedAgo: 3 * DAY }, now),
    makeRepo({ name: "yuki-design-tokens", age: 4 * DAY, incoming: 2, fetchedAgo: 2 * DAY }, now),
    makeRepo({ name: "sandbox-rust", git: false, markers: ["Cargo.toml"] }, now),
    makeRepo(
      {
        name: "scratch-ideas",
        remote: null,
        commits: 12,
        age: 6 * DAY,
        markers: [".git", "pyproject.toml"],
        files: [file("notes/2026-11.md", "modified", 8, 1)],
      },
      now,
    ),
    makeRepo(
      {
        name: "client-lp-2026",
        root: "work",
        branch: "main",
        age: 20 * MINUTE,
        inProgress: "merge",
        incomingLabel: "origin/main",
        conflicts: [
          { path: "src/components/Hero.tsx", kind: "both_modified", hasMarkers: true },
          { path: "src/styles/tokens.css", kind: "both_modified", hasMarkers: true },
        ],
        files: [
          file("src/components/Hero.tsx", "conflicted"),
          file("src/styles/tokens.css", "conflicted"),
          file("src/components/Footer.tsx", "modified", 4, 4),
        ],
      },
      now,
    ),
    makeRepo(
      {
        name: "ml-experiments",
        branch: "experiment/vit-small",
        published: false,
        ahead: 0,
        commits: 31,
        age: 26 * HOUR,
        markers: [".git", "pyproject.toml"],
        branches: [{ name: "main", merged: true, incoming: 0 }],
      },
      now,
    ),
    makeRepo({ name: "blog", root: "work", networkFailure: "auth", age: 12 * DAY, ahead: 1 }, now),
    makeRepo({ name: "legacy-tools", root: "work", networkFailure: "unreachable", age: 80 * DAY, markers: [".git", "Makefile"] }, now),
    makeRepo({ name: "kotoba-dict", detached: true, age: 15 * DAY, markers: [".git", "Package.swift"] }, now),
    makeRepo({ name: "mikan-app", networkFailure: "not_found", age: 30 * DAY, markers: [".git", "pubspec.yaml"] }, now),
    makeRepo(
      {
        name: "kaze-docs",
        age: 8 * DAY,
        extraRemotes: [],
        settings: { branchTargets: { main: { push: { remote: "backup", branch: "main" }, pull: null } } },
      },
      now,
    ),
    makeRepo({ name: "old-prototype", root: "manual", exists: false, age: 200 * DAY }, now),
  ];
}

export function seedGithubRepos(now: number, repos: MockRepo[]): GithubRepo[] {
  const local = new Map<string, string[]>();
  for (const repo of repos) {
    for (const remote of repo.remotes) {
      if (!remote.github) continue;
      const key = `${remote.github.owner}/${remote.github.repo}`;
      local.set(key, [...(local.get(key) ?? []), repo.id]);
    }
  }
  const entries: [string, string | null, GithubRepo["visibility"], number, boolean?][] = [
    ["revia", "散らばったプロジェクトを、一つの落ち着いた作業空間へ", "private", 40 * MINUTE],
    ["aozora-notes", "Markdown で書けるシンプルなノートアプリ", "public", 3 * HOUR],
    ["kumo-api", "社内向け API サーバー", "private", 5 * HOUR],
    ["hoshi-ui", "React 向けの UI コンポーネント集", "public", 6 * HOUR],
    ["tsubame-cli", "高速なファイル同期 CLI", "public", 8 * HOUR],
    ["portfolio-site", null, "public", DAY],
    ["yuki-design-tokens", "デザイントークンの定義と生成スクリプト", "private", 2 * DAY],
    ["dotfiles", null, "public", 9 * DAY],
    ["ml-experiments", "画像分類モデルの実験", "private", 26 * HOUR],
    ["kaze-docs", "製品ドキュメント", "private", 8 * DAY],
    ["sakura-calendar", "祝日対応のカレンダーライブラリ", "public", 3 * DAY],
    ["minato-infra", "インフラ構成（Terraform）", "private", 4 * DAY],
    ["hikari-icons", "アイコンセット", "public", 11 * DAY],
    ["tanuki-bot", "Slack ボット", "private", 20 * DAY],
    ["uta-player", "音楽プレイヤーの試作", "public", 45 * DAY, true],
    ["advent-of-code-2025", null, "public", 330 * DAY],
  ];
  return entries.map(([name, description, visibility, ago, fork], index) => ({
    id: `R_${index.toString().padStart(4, "0")}`,
    nameWithOwner: `${ACCOUNT}/${name}`,
    owner: ACCOUNT,
    name,
    description,
    visibility,
    url: `https://github.com/${ACCOUNT}/${name}`,
    cloneUrl: `https://github.com/${ACCOUNT}/${name}.git`,
    sshUrl: `git@github.com:${ACCOUNT}/${name}.git`,
    defaultBranch: "main",
    updatedAt: now - ago,
    fork: fork ?? false,
    archived: name === "advent-of-code-2025",
    localProjectIds: local.get(`${ACCOUNT}/${name}`) ?? [],
  }));
}

export function seedHistory(now: number, repos: MockRepo[]): HistoryEntry[] {
  const byName = (name: string) => repos.find((repo) => repo.name === name)!;
  const rows: [string, HistoryEntry["action"], HistoryEntry["outcome"], number, Record<string, string | number>, HistoryEntry["errorCode"]?][] = [
    ["revia", "sync", "succeeded", 40 * MINUTE, { branch: "main", pulled: 0, pushed: 2 }],
    ["client-lp-2026", "pull", "conflict", 22 * MINUTE, { branch: "main", source: "origin/main", files: 2 }],
    ["kumo-api", "commit", "succeeded", 25 * MINUTE, { branch: "develop", files: 4, sha: "a81f3c2" }],
    ["blog", "fetch", "failed", 2 * HOUR, { remote: "origin" }, "auth_failed"],
    ["hoshi-ui", "fetch", "succeeded", 4 * MINUTE, { remote: "origin" }],
    ["aozora-notes", "checkout", "succeeded", 5 * HOUR, { branch: "main" }],
    ["tsubame-cli", "sync", "needs_confirmation", 5 * HOUR, { branch: "main" }],
    ["portfolio-site", "commit", "succeeded", 50 * MINUTE, { branch: "main", files: 3, sha: "4c09be1" }],
    ["revia", "merge", "succeeded", 26 * HOUR, { source: "fix/scan-depth", target: "main", commits: 3 }],
    ["yuki-design-tokens", "push", "succeeded", 2 * DAY, { branch: "main", pushed: 1, target: "origin/main" }],
    ["legacy-tools", "fetch", "failed", 3 * DAY, { remote: "origin" }, "network_unreachable"],
    ["ml-experiments", "create_branch", "succeeded", 27 * HOUR, { branch: "experiment/vit-small" }],
    ["dotfiles", "sync", "succeeded", 3 * DAY, { branch: "main", pulled: 0, pushed: 0 }],
  ];
  return rows
    .map(([name, action, outcome, ago, params, errorCode], index) => {
      const repo = byName(name);
      return {
        id: `h${(1000 - index).toString()}`,
        at: now - ago,
        operationId: null,
        projectId: repo.id,
        projectName: repo.name,
        action,
        outcome,
        params,
        errorCode: errorCode ?? null,
      } satisfies HistoryEntry;
    })
    .sort((a, b) => b.at - a.at);
}
