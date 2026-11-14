import type {
  AppSettings,
  AppState,
  BackgroundImage,
  ChangedFile,
  CommitDetail,
  CommitInfo,
  ConflictState,
  DiffSource,
  EditorInfo,
  Environment,
  FileDiff,
  GeneralSettingsPatch,
  GithubOwner,
  GithubRepo,
  GithubStatus,
  HistoryEntry,
  IgnoreScan,
  MergePreview,
  OpenTarget,
  OperationPlan,
  OperationRequest,
  OperationSnapshot,
  Project,
  ProjectDetail,
  ProjectSettingsPatch,
  RebasePreview,
  RemoteRef,
  ResetMode,
  ResetPreview,
  RemovalPreview,
  RemovalResult,
  RepositoryDeletionCheck,
  ReviaEvents,
  UpdateDownload,
  UpdateInfo,
  Workspace,
} from "./types";

export interface ReviaApi {
  getAppState(): Promise<AppState>;
  getEnvironment(): Promise<Environment>;
  pickFolder(args: { title: string | null }): Promise<string | null>;
  openProject(args: { projectId: string; target: OpenTarget; relativePath: string | null }): Promise<void>;
  openUrl(args: { url: string }): Promise<void>;
  listEditors(): Promise<EditorInfo[]>;
  pickBackgroundImage(args: { filterLabel: string | null }): Promise<BackgroundImage | null>;
  readBackgroundImage(args: { id: string }): Promise<ArrayBuffer>;
  clearBackgroundImage(): Promise<void>;

  updateUiSettings(args: { ui: Record<string, unknown> }): Promise<void>;
  updateGeneralSettings(args: { patch: GeneralSettingsPatch }): Promise<AppSettings>;
  addScanRoot(args: { path: string; maxDepth: number | null }): Promise<Workspace>;
  updateScanRoot(args: { rootId: string; patch: { enabled?: boolean; maxDepth?: number } }): Promise<Workspace>;
  removeScanRoot(args: { rootId: string }): Promise<Workspace>;
  addProject(args: { path: string }): Promise<Workspace>;
  excludeProject(args: { projectId: string }): Promise<Workspace>;
  restoreExcludedPath(args: { path: string }): Promise<Workspace>;
  updateProjectSettings(args: { projectId: string; patch: ProjectSettingsPatch }): Promise<Project>;
  setBranchTarget(args: {
    projectId: string;
    branch: string;
    kind: "push" | "pull";
    target: RemoteRef | null;
  }): Promise<Project>;

  scanProjects(): Promise<Workspace>;
  refreshProjects(args: { projectIds: string[] | null }): Promise<Project[]>;

  getProjectDetail(args: { projectId: string }): Promise<ProjectDetail>;
  getChanges(args: { projectId: string }): Promise<ChangedFile[]>;
  getDiff(args: { projectId: string; path: string; source: DiffSource }): Promise<FileDiff>;
  getCommitLog(args: { projectId: string; rev: string | null; limit: number; skip: number }): Promise<CommitInfo[]>;
  getCommitDetail(args: { projectId: string; sha: string }): Promise<CommitDetail>;
  getConflicts(args: { projectId: string }): Promise<ConflictState>;

  planOperation(args: { request: OperationRequest }): Promise<OperationPlan>;
  startOperation(args: { request: OperationRequest }): Promise<OperationSnapshot>;
  cancelOperation(args: { operationId: string }): Promise<void>;
  getOperation(args: { operationId: string }): Promise<OperationSnapshot | null>;

  initRepository(args: { projectId: string; initialBranch: string | null }): Promise<Project>;
  createBranch(args: {
    projectId: string;
    name: string;
    startPoint: string | null;
    checkout: boolean;
  }): Promise<ProjectDetail>;
  checkoutBranch(args: {
    projectId: string;
    branch: string;
    ifDirty: "abort" | "carry" | "stash";
  }): Promise<ProjectDetail>;
  renameBranch(args: { projectId: string; from: string; to: string }): Promise<ProjectDetail>;
  deleteBranch(args: { projectId: string; name: string; force: boolean }): Promise<ProjectDetail>;
  deleteRemoteBranch(args: { projectId: string; remote: string; branch: string }): Promise<ProjectDetail>;
  setUpstream(args: { projectId: string; branch: string; upstream: RemoteRef | null }): Promise<ProjectDetail>;
  previewMerge(args: { projectId: string; source: string; target: string }): Promise<MergePreview>;

  addRemote(args: { projectId: string; name: string; url: string }): Promise<ProjectDetail>;
  setRemoteUrl(args: { projectId: string; name: string; url: string }): Promise<ProjectDetail>;
  renameRemote(args: { projectId: string; from: string; to: string }): Promise<ProjectDetail>;
  removeRemote(args: { projectId: string; name: string }): Promise<ProjectDetail>;

  markResolved(args: { projectId: string; paths: string[] }): Promise<ConflictState>;
  continueInProgress(args: { projectId: string }): Promise<ProjectDetail>;
  abortInProgress(args: { projectId: string }): Promise<ProjectDetail>;

  previewRebase(args: { projectId: string; onto: string }): Promise<RebasePreview>;
  rebase(args: { projectId: string; onto: string; fingerprint: string }): Promise<ProjectDetail>;
  cherryPick(args: { projectId: string; shas: string[] }): Promise<ProjectDetail>;
  previewReset(args: { projectId: string; target: string; mode: ResetMode }): Promise<ResetPreview>;
  reset(args: { projectId: string; target: string; mode: ResetMode; fingerprint: string }): Promise<ProjectDetail>;
  discardChanges(args: {
    projectId: string;
    paths: string[] | null;
    includeUntracked: boolean;
  }): Promise<ChangedFile[]>;
  stashSave(args: { projectId: string; message: string | null; includeUntracked: boolean }): Promise<ProjectDetail>;
  stashApply(args: { projectId: string; stashSha: string; drop: boolean }): Promise<ProjectDetail>;
  stashDrop(args: { projectId: string; stashSha: string }): Promise<ProjectDetail>;

  githubStatus(args: { refresh: boolean }): Promise<GithubStatus>;
  installGithubCli(): Promise<GithubStatus>;
  githubLoginStart(args: { scopes: string[] | null }): Promise<{ loginId: string }>;
  githubLoginCancel(args: { loginId: string }): Promise<void>;
  githubLogout(): Promise<GithubStatus>;
  githubListRepos(args: { refresh: boolean }): Promise<GithubRepo[]>;
  githubListOwners(): Promise<GithubOwner[]>;

  scanIgnoreCandidates(args: { projectId: string }): Promise<IgnoreScan>;

  previewRemoval(args: { projectId: string }): Promise<RemovalPreview>;
  checkRepositoryDeletion(args: { projectId: string }): Promise<RepositoryDeletionCheck>;
  removeGit(args: { projectId: string; fingerprint: string; confirm: string }): Promise<Project>;
  deleteProject(args: {
    projectId: string;
    deleteRepository: boolean;
    fingerprint: string;
    confirm: string;
  }): Promise<RemovalResult>;

  getHistory(args: { limit: number; before: string | null; projectId: string | null }): Promise<HistoryEntry[]>;
  clearHistory(): Promise<void>;

  checkUpdate(): Promise<UpdateInfo>;
  downloadUpdate(): Promise<UpdateDownload>;
  cancelUpdateDownload(): Promise<void>;
  installUpdate(): Promise<void>;

  on<E extends keyof ReviaEvents>(event: E, handler: (payload: ReviaEvents[E]) => void): Promise<() => void>;
}

export type CommandName = Exclude<keyof ReviaApi, "on">;
