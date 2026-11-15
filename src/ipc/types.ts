export type Timestamp = number;

export type ErrorCode =
  | "git_not_installed"
  | "gh_not_installed"
  | "identity_missing"
  | "not_authenticated"
  | "auth_failed"
  | "permission_denied"
  | "network_unreachable"
  | "remote_not_found"
  | "rate_limited"
  | "timeout"
  | "push_rejected"
  | "not_fast_forward"
  | "no_remote"
  | "remote_exists"
  | "target_unresolved"
  | "target_invalid"
  | "branch_not_found"
  | "branch_exists"
  | "branch_not_merged"
  | "invalid_ref_name"
  | "dirty_worktree"
  | "merge_conflict"
  | "operation_in_progress"
  | "nothing_to_commit"
  | "detached_head"
  | "unborn_branch"
  | "project_not_found"
  | "path_not_found"
  | "not_a_repository"
  | "invalid_path"
  | "already_registered"
  | "destination_exists"
  | "repo_exists"
  | "scope_missing"
  | "repo_locked"
  | "state_changed"
  | "feature_disabled"
  | "busy"
  | "cancelled"
  | "verification_failed"
  | "disk_full"
  | "io_error"
  | "settings_corrupt"
  | "invalid_input"
  | "unknown";

export interface AppError {
  code: ErrorCode;
  message: string;
  detail: string | null;
}

export type SyncState =
  | "missing"
  | "not_initialized"
  | "conflicted"
  | "in_progress"
  | "detached"
  | "no_remote"
  | "remote_not_found"
  | "auth_error"
  | "dirty"
  | "unreachable"
  | "diverged"
  | "behind"
  | "ahead"
  | "unpublished"
  | "synced"
  | "unknown";

export interface ChangeCounts {
  staged: number;
  unstaged: number;
  untracked: number;
  conflicted: number;
  total: number;
}

export interface RemoteRef {
  remote: string;
  branch: string;
}

export type TargetSource = "saved" | "upstream" | "remote_default" | "inferred";

export interface ResolvedTarget extends RemoteRef {
  source: TargetSource;
  exists: boolean;
}

export type TargetInvalidReason = "remote_missing" | "branch_missing";

export interface TargetState {
  target: ResolvedTarget | null;
  saved: RemoteRef | null;
  savedInvalid: TargetInvalidReason | null;
}

export interface BranchTargets {
  push: TargetState;
  pull: TargetState;
}

export interface GithubRepoRef {
  owner: string;
  repo: string;
  url: string;
}

export interface RemoteInfo {
  name: string;
  fetchUrl: string;
  pushUrl: string;
  github: GithubRepoRef | null;
}

export type InProgressKind = "merge" | "rebase" | "cherry_pick" | "revert" | "bisect";
export type RemoteErrorKind = "auth" | "unreachable" | "not_found";

export interface ProjectStatus {
  state: SyncState;
  branch: string | null;
  detached: boolean;
  unborn: boolean;
  headSha: string | null;
  upstream: RemoteRef | null;
  targets: BranchTargets;
  ahead: number | null;
  behind: number | null;
  divergence: Divergence | null;
  changes: ChangeCounts;
  inProgress: InProgressKind | null;
  remotes: RemoteInfo[];
  github: GithubRepoRef | null;
  remoteError: RemoteErrorKind | null;
  lastCommitAt: Timestamp | null;
  checkedAt: Timestamp;
  fetchedAt: Timestamp | null;
  error: AppError | null;
}

export interface Divergence {
  localAt: Timestamp | null;
  remoteAt: Timestamp | null;
}

export type PullMode = "ff_only" | "merge" | "rebase";

export interface BranchTargetSetting {
  push: RemoteRef | null;
  pull: RemoteRef | null;
}

export interface ProjectSettings {
  favorite: boolean;
  includeInBatch: boolean;
  defaultRemote: string | null;
  defaultBranch: string | null;
  pullMode: PullMode;
  branchTargets: Record<string, BranchTargetSetting>;
}

export type ProjectSettingsPatch = Partial<
  Pick<ProjectSettings, "favorite" | "includeInBatch" | "defaultRemote" | "defaultBranch" | "pullMode">
>;

export type ProjectOrigin = { kind: "scan"; rootId: string } | { kind: "manual" };

export interface Project {
  id: string;
  name: string;
  path: string;
  origin: ProjectOrigin;
  markers: string[];
  settings: ProjectSettings;
  status: ProjectStatus | null;
}

export interface ScanRoot {
  id: string;
  path: string;
  maxDepth: number;
  enabled: boolean;
  exists: boolean;
}

export interface GeneralSettings {
  autoRefresh: {
    onFocus: boolean;
    intervalMinutes: number;
    fetch: boolean;
  };
  concurrency: number;
  editor: string | null;
  safety: {
    allowForcePush: boolean;
    allowHardReset: boolean;
  };
  sync: SyncSettings;
}

export type DivergedPolicy = "stop" | "merge" | "prefer_local" | "prefer_remote";

export interface SyncSettings {
  commit: boolean;
  commitMessage: string;
  diverged: DivergedPolicy;
}

export type GeneralSettingsPatch = {
  autoRefresh?: Partial<GeneralSettings["autoRefresh"]>;
  concurrency?: number;
  editor?: string | null;
  safety?: Partial<GeneralSettings["safety"]>;
  sync?: Partial<SyncSettings>;
};

export interface AppSettings {
  roots: ScanRoot[];
  excludedPaths: string[];
  general: GeneralSettings;
  ui: Record<string, unknown>;
}

export interface ToolInfo {
  installed: boolean;
  version: string | null;
}

export interface Environment {
  platform: "macos" | "windows" | "linux";
  appVersion: string;
  git: ToolInfo;
  gh: ToolInfo;
}

export interface SettingsRecovery {
  kind: "restored_backup" | "reset";
  corruptCopyPath: string | null;
}

export interface Workspace {
  settings: AppSettings;
  projects: Project[];
}

export interface AppState extends Workspace {
  environment: Environment;
  recovery: SettingsRecovery | null;
}

export interface EditorInfo {
  id: string;
  name: string;
}

export type OpenTarget = "file_manager" | "terminal" | "editor" | "github";

export interface BackgroundImage {
  id: string;
  name: string;
  mime: string;
  size: number;
}

export interface BranchInfo {
  name: string;
  kind: "local" | "remote";
  remote: string | null;
  shortName: string;
  current: boolean;
  sha: string;
  upstream: RemoteRef | null;
  upstreamGone: boolean;
  ahead: number | null;
  behind: number | null;
  mergedIntoCurrent: boolean;
  lastCommit: { subject: string; authorName: string; at: Timestamp } | null;
}

export interface StashEntry {
  sha: string;
  index: number;
  message: string;
  branch: string | null;
  at: Timestamp;
}

export interface ProjectDetail {
  project: Project;
  branches: BranchInfo[];
  stashes: StashEntry[];
}

export type FileChangeKind =
  | "modified"
  | "added"
  | "deleted"
  | "renamed"
  | "copied"
  | "type_changed"
  | "untracked"
  | "conflicted";

export interface ChangedFile {
  path: string;
  oldPath: string | null;
  kind: FileChangeKind;
  staged: boolean;
  unstaged: boolean;
  additions: number | null;
  deletions: number | null;
  binary: boolean;
}

export type DiffSource = { kind: "worktree" } | { kind: "commit"; sha: string };

export interface DiffLine {
  kind: "context" | "add" | "del";
  oldNo: number | null;
  newNo: number | null;
  text: string;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  oldPath: string | null;
  binary: boolean;
  truncated: boolean;
  hunks: DiffHunk[];
}

export interface CommitInfo {
  sha: string;
  shortSha: string;
  subject: string;
  body: string;
  authorName: string;
  at: Timestamp;
  parents: string[];
  refs: string[];
  pushed: boolean | null;
}

export interface CommitDetail {
  commit: CommitInfo;
  files: ChangedFile[];
}

export type ConflictKind =
  | "both_modified"
  | "both_added"
  | "both_deleted"
  | "added_by_us"
  | "added_by_them"
  | "deleted_by_us"
  | "deleted_by_them";

export interface ConflictFile {
  path: string;
  kind: ConflictKind;
  hasMarkers: boolean;
}

export interface ConflictState {
  operation: InProgressKind | null;
  incoming: string | null;
  files: ConflictFile[];
  canContinue: boolean;
}

export type MergeMode = "default" | "no_ff" | "ff_only";
export type ResetMode = "soft" | "mixed" | "hard";

export interface RefPoint {
  name: string;
  sha: string;
}

export interface MergePreview {
  source: RefPoint;
  target: RefPoint;
  needsCheckout: boolean;
  worktreeDirty: boolean;
  upToDate: boolean;
  fastForward: boolean;
  commits: CommitInfo[];
  commitCount: number;
  filesChanged: number;
  conflicts: { status: "none" | "conflicts" | "unknown"; paths: string[] };
  blocker: PlanBlocker | null;
  fingerprint: string;
}

export interface RebasePreview {
  onto: RefPoint;
  branch: string;
  commits: CommitInfo[];
  commitCount: number;
  publishedCount: number;
  worktreeDirty: boolean;
  blocker: PlanBlocker | null;
  fingerprint: string;
}

export interface ResetPreview {
  target: RefPoint;
  branch: string;
  mode: ResetMode;
  commits: CommitInfo[];
  commitCount: number;
  unpushedCount: number;
  discardedFiles: number;
  blocker: PlanBlocker | null;
  fingerprint: string;
}

export type OperationKind =
  | "fetch"
  | "pull"
  | "push"
  | "commit"
  | "sync"
  | "merge"
  | "publish"
  | "clone";

interface ItemBase {
  itemId: string;
  fingerprint?: string | null;
}

interface ProjectItemBase extends ItemBase {
  projectId: string;
}

export type FetchItem = ProjectItemBase;
export type SyncItem = ProjectItemBase;

export interface PullItem extends ProjectItemBase {
  mode: PullMode;
  prefer?: PreferSide | null;
}

export interface PushItem extends ProjectItemBase {
  force: boolean;
  setUpstream: boolean;
}

export interface CommitItem extends ProjectItemBase {
  message: string;
  paths: string[] | null;
}

export interface MergeItem extends ProjectItemBase {
  source: string;
  target: string;
  mode: MergeMode;
}

export interface PublishItem extends ProjectItemBase {
  owner: string | null;
  name: string;
  visibility: "private" | "public";
  description: string | null;
  initialCommitMessage: string | null;
  ignore: string[] | null;
}

export interface CloneItem extends ItemBase {
  url: string;
  parentDir: string;
  folderName: string;
}

export type OperationRequest =
  | { kind: "fetch"; items: FetchItem[] }
  | { kind: "pull"; items: PullItem[] }
  | { kind: "push"; items: PushItem[] }
  | { kind: "commit"; items: CommitItem[] }
  | { kind: "sync"; items: SyncItem[] }
  | { kind: "merge"; items: MergeItem[] }
  | { kind: "publish"; items: PublishItem[] }
  | { kind: "clone"; items: CloneItem[] };

export type PlanStep =
  | { code: "init" }
  | { code: "ignore"; patterns: number }
  | { code: "initial_commit"; files: number }
  | { code: "fetch" }
  | { code: "pull"; mode: PullMode; from: RemoteRef; commits: number; prefer: PreferSide | null }
  | {
      code: "push";
      to: RemoteRef;
      commits: number | null;
      createsRemoteBranch: boolean;
      force: boolean;
      setUpstream: boolean;
    }
  | { code: "commit"; files: number }
  | { code: "checkout"; branch: string }
  | { code: "merge"; source: string; target: string; mode: MergeMode; commits: number; fastForward: boolean }
  | { code: "create_repo"; nameWithOwner: string; visibility: "private" | "public" }
  | { code: "add_remote"; name: string; url: string }
  | { code: "clone"; url: string; destination: string };

export type PreferSide = "local" | "remote";

export type StepCode = PlanStep["code"];

export type PlanBlocker =
  | "missing"
  | "not_initialized"
  | "unborn"
  | "no_remote"
  | "target_unresolved"
  | "target_invalid"
  | "detached"
  | "in_progress"
  | "conflicted"
  | "diverged"
  | "dirty_worktree"
  | "nothing_to_do"
  | "not_authenticated"
  | "identity_missing"
  | "feature_disabled"
  | "remote_exists"
  | "already_published"
  | "repo_exists"
  | "destination_exists"
  | "branch_not_found"
  | "invalid_input"
  | "state_changed";

export type RiskLevel = "safe" | "caution" | "danger";

export type PlanWarning =
  | { code: "uncommitted_changes"; files: number }
  | { code: "stale_remote"; fetchedAt: Timestamp | null }
  | { code: "creates_remote_branch"; to: RemoteRef }
  | { code: "conflicts_likely"; paths: string[] }
  | { code: "conflicts_resolved"; prefer: PreferSide; paths: string[] }
  | { code: "skips_sensitive_files"; paths: string[] }
  | { code: "rewrites_published_history"; commits: number }
  | { code: "public_repo" }
  | { code: "sensitive_files"; paths: string[] }
  | { code: "no_gitignore" }
  | { code: "large_files"; paths: string[] }
  | { code: "many_files"; files: number };

export interface PlanItem {
  itemId: string;
  projectId: string | null;
  executable: boolean;
  blocker: PlanBlocker | null;
  steps: PlanStep[];
  risk: RiskLevel;
  warnings: PlanWarning[];
  fingerprint: string | null;
}

export interface OperationPlan {
  kind: OperationKind;
  items: PlanItem[];
}

export type ItemState =
  | "queued"
  | "running"
  | "succeeded"
  | "skipped"
  | "failed"
  | "needs_confirmation"
  | "conflict"
  | "cancelled";

export interface OperationItem {
  itemId: string;
  projectId: string | null;
  state: ItemState;
  step: StepCode | null;
  progress: number | null;
  completed: PlanStep[];
  reason: PlanBlocker | null;
  error: AppError | null;
}

export interface OperationSnapshot {
  operationId: string;
  kind: OperationKind;
  state: "running" | "cancelling" | "finished";
  startedAt: Timestamp;
  finishedAt: Timestamp | null;
  items: OperationItem[];
}

export type OperationEvent =
  | { operationId: string; kind: "item"; item: OperationItem }
  | { operationId: string; kind: "state"; snapshot: OperationSnapshot };

export interface GithubAccount {
  login: string;
  name: string | null;
  avatarUrl: string | null;
}

export interface GithubStatus {
  cliInstalled: boolean;
  authenticated: boolean;
  account: GithubAccount | null;
  gitProtocol: "https" | "ssh" | null;
  missingScopes: string[];
  tokenStorage: "keyring" | "file" | "environment" | null;
}

export type GithubLoginEvent =
  | { loginId: string; kind: "code"; userCode: string; verificationUrl: string }
  | { loginId: string; kind: "succeeded"; status: GithubStatus }
  | { loginId: string; kind: "failed"; error: AppError }
  | { loginId: string; kind: "cancelled" };

export interface GithubRepo {
  id: string;
  nameWithOwner: string;
  owner: string;
  name: string;
  description: string | null;
  visibility: "public" | "private" | "internal";
  url: string;
  cloneUrl: string;
  sshUrl: string;
  defaultBranch: string | null;
  updatedAt: Timestamp;
  fork: boolean;
  archived: boolean;
  localProjectIds: string[];
}

export interface GithubOwner {
  login: string;
  kind: "user" | "organization";
  avatarUrl: string | null;
}

export type HistoryAction =
  | OperationKind
  | "init"
  | "checkout"
  | "create_branch"
  | "rename_branch"
  | "delete_branch"
  | "delete_remote_branch"
  | "set_upstream"
  | "add_remote"
  | "set_remote_url"
  | "rename_remote"
  | "remove_remote"
  | "rebase"
  | "cherry_pick"
  | "reset"
  | "discard"
  | "stash_save"
  | "stash_apply"
  | "stash_drop"
  | "continue"
  | "abort"
  | "remove_git"
  | "delete_local"
  | "delete_repository";

export type HistoryOutcome =
  | "succeeded"
  | "skipped"
  | "failed"
  | "needs_confirmation"
  | "conflict"
  | "cancelled";

export interface HistoryEntry {
  id: string;
  at: Timestamp;
  operationId: string | null;
  projectId: string | null;
  projectName: string;
  action: HistoryAction;
  outcome: HistoryOutcome;
  params: Record<string, string | number | null>;
  errorCode: ErrorCode | null;
}

export type IgnoreCategory = "secret" | "personal" | "system" | "dependency" | "build" | "scratch";

export type IgnoreReason =
  | "env_file"
  | "key_file"
  | "credential_file"
  | "secret_in_content"
  | "personal_data_file"
  | "personal_in_content"
  | "database_file"
  | "os_file"
  | "editor_file"
  | "dependency_dir"
  | "build_output"
  | "cache_dir"
  | "log_file"
  | "temp_file"
  | "scratch_file";

export interface IgnoreSuggestion {
  pattern: string;
  category: IgnoreCategory;
  reason: IgnoreReason;
  examples: string[];
  matches: number;
}

export interface IgnoreScan {
  projectId: string;
  suggestions: IgnoreSuggestion[];
  truncated: boolean;
}

export interface RemovalPreview {
  projectId: string;
  path: string;
  repository: boolean;
  uncommittedFiles: number;
  unpushedCommits: number | null;
  localOnlyBranches: string[];
  stashes: number;
  remotes: RemoteInfo[];
  github: GithubRepoRef | null;
  nestedProjects: string[];
  fingerprint: string;
}

export type RepositoryDeletionBlocker =
  | "gh_not_installed"
  | "not_authenticated"
  | "scope_missing"
  | "no_permission"
  | "not_found";

export interface RepositoryDeletionCheck {
  nameWithOwner: string;
  visibility: "public" | "private" | "internal" | null;
  isFork: boolean | null;
  blocker: RepositoryDeletionBlocker | null;
}

export interface RemovalResult {
  repository: "deleted" | "skipped";
  local: "trashed" | "failed";
  error: AppError | null;
  workspace: Workspace;
}

export type UpdateAssetKind = "dmg" | "nsis" | "appimage" | "deb" | "rpm";

export interface UpdateAsset {
  name: string;
  size: number;
  kind: UpdateAssetKind;
  hasDigest: boolean;
}

export interface UpdateRelease {
  version: string;
  name: string | null;
  notes: string | null;
  publishedAt: Timestamp | null;
  url: string;
  asset: UpdateAsset | null;
}

export interface UpdateInfo {
  currentVersion: string;
  checkedAt: Timestamp;
  latest: UpdateRelease | null;
  available: boolean;
}

export interface UpdateDownload {
  name: string;
  size: number;
  path: string;
  verified: boolean;
}

export interface UpdateProgress {
  received: number;
  total: number | null;
}

export interface ScanProgress {
  rootId: string;
  directories: number;
  found: number;
  done: boolean;
}

export interface ReviaEvents {
  "revia:project-updated": Project;
  "revia:projects-changed": Project[];
  "revia:operation": OperationEvent;
  "revia:scan-progress": ScanProgress;
  "revia:github-login": GithubLoginEvent;
  "revia:update-progress": UpdateProgress;
}
