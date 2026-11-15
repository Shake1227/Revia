use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
macro_rules! wire_enum {($name:ident {$($variant:ident),* $(,)?})=>{#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)] #[serde(rename_all="snake_case")] pub enum $name {$($variant),*}};}
wire_enum!(SyncState {
    Missing,
    NotInitialized,
    Conflicted,
    InProgress,
    Detached,
    NoRemote,
    RemoteNotFound,
    AuthError,
    Dirty,
    Unreachable,
    Diverged,
    Behind,
    Ahead,
    Unpublished,
    Synced,
    Unknown
});
wire_enum!(PullMode {
    FfOnly,
    Merge,
    Rebase
});
wire_enum!(MergeMode {
    Default,
    NoFf,
    FfOnly
});
wire_enum!(TargetSource {
    Saved,
    Upstream,
    RemoteDefault,
    Inferred
});
wire_enum!(TargetInvalidReason {
    RemoteMissing,
    BranchMissing
});
wire_enum!(OperationKind {
    Fetch,
    Pull,
    Push,
    Commit,
    Sync,
    Merge,
    Publish,
    Clone
});
wire_enum!(ItemState {
    Queued,
    Running,
    Succeeded,
    Skipped,
    Failed,
    NeedsConfirmation,
    Conflict,
    Cancelled
});
wire_enum!(PlanBlocker {
    Missing,
    NotInitialized,
    Unborn,
    NoRemote,
    TargetUnresolved,
    TargetInvalid,
    Detached,
    InProgress,
    Conflicted,
    Diverged,
    DirtyWorktree,
    NothingToDo,
    NotAuthenticated,
    IdentityMissing,
    FeatureDisabled,
    RemoteExists,
    AlreadyPublished,
    RepoExists,
    DestinationExists,
    BranchNotFound,
    InvalidInput,
    StateChanged
});
wire_enum!(RiskLevel {
    Safe,
    Caution,
    Danger
});
wire_enum!(InProgressKind {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect
});
wire_enum!(RemoteErrorKind {
    Auth,
    Unreachable,
    NotFound
});
wire_enum!(FileChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflicted
});
wire_enum!(ConflictKind {
    BothModified,
    BothAdded,
    BothDeleted,
    AddedByUs,
    AddedByThem,
    DeletedByUs,
    DeletedByThem
});
macro_rules! dto {($name:ident {$($field:ident:$ty:ty),* $(,)?})=>{#[derive(Debug,Clone,Serialize,Deserialize)] #[serde(rename_all="camelCase")] pub struct $name {$(pub $field:$ty),*}};}
dto!(RemoteRef {
    remote: String,
    branch: String
});
dto!(ResolvedTarget {
    remote: String,
    branch: String,
    source: TargetSource,
    exists: bool
});
dto!(TargetState{target:Option<ResolvedTarget>,saved:Option<RemoteRef>,saved_invalid:Option<TargetInvalidReason>});
dto!(BranchTargets {
    push: TargetState,
    pull: TargetState
});
dto!(ChangeCounts {
    staged: u64,
    unstaged: u64,
    untracked: u64,
    conflicted: u64,
    total: u64
});
dto!(GithubRepoRef {
    owner: String,
    repo: String,
    url: String
});
dto!(RemoteInfo{name:String,fetch_url:String,push_url:String,github:Option<GithubRepoRef>});
dto!(Divergence {
    local_at: Option<u64>,
    remote_at: Option<u64>
});
dto!(ProjectStatus{state:SyncState,branch:Option<String>,detached:bool,unborn:bool,head_sha:Option<String>,upstream:Option<RemoteRef>,targets:BranchTargets,ahead:Option<u64>,behind:Option<u64>,divergence:Option<Divergence>,changes:ChangeCounts,in_progress:Option<InProgressKind>,remotes:Vec<RemoteInfo>,github:Option<GithubRepoRef>,remote_error:Option<RemoteErrorKind>,last_commit_at:Option<u64>,checked_at:u64,fetched_at:Option<u64>,error:Option<crate::error::AppError>});
dto!(BranchTargetSetting{push:Option<RemoteRef>,pull:Option<RemoteRef>});
dto!(ProjectSettings{favorite:bool,include_in_batch:bool,default_remote:Option<String>,default_branch:Option<String>,pull_mode:PullMode,branch_targets:BTreeMap<String,BranchTargetSetting>});
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ProjectOrigin {
    Scan { root_id: String },
    Manual,
}
dto!(Project{id:String,name:String,path:String,origin:ProjectOrigin,markers:Vec<String>,settings:ProjectSettings,status:Option<ProjectStatus>});
dto!(ScanRoot {
    id: String,
    path: String,
    max_depth: u64,
    enabled: bool,
    exists: bool
});
dto!(AutoRefresh {
    on_focus: bool,
    interval_minutes: u64,
    fetch: bool
});
dto!(Safety {
    allow_force_push: bool,
    allow_hard_reset: bool
});
wire_enum!(DivergedPolicy {
    Stop,
    Merge,
    PreferLocal,
    PreferRemote
});
wire_enum!(PreferSide { Local, Remote });
dto!(SyncSettings {
    commit: bool,
    commit_message: String,
    diverged: DivergedPolicy
});
dto!(GeneralSettings{auto_refresh:AutoRefresh,concurrency:u64,editor:Option<String>,safety:Safety,sync:SyncSettings});
dto!(AppSettings{roots:Vec<ScanRoot>,excluded_paths:Vec<String>,general:GeneralSettings,ui:BTreeMap<String,Value>});
dto!(Workspace{settings:AppSettings,projects:Vec<Project>});
dto!(ChangedFile{path:String,old_path:Option<String>,kind:FileChangeKind,staged:bool,unstaged:bool,additions:Option<u64>,deletions:Option<u64>,binary:bool});
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiffSource {
    Worktree,
    Commit { sha: String },
}
dto!(DiffLine{kind:String,old_no:Option<u64>,new_no:Option<u64>,text:String});
dto!(DiffHunk{header:String,lines:Vec<DiffLine>});
dto!(FileDiff{path:String,old_path:Option<String>,binary:bool,truncated:bool,hunks:Vec<DiffHunk>});
dto!(CommitInfo{sha:String,short_sha:String,subject:String,body:String,author_name:String,at:u64,parents:Vec<String>,refs:Vec<String>,pushed:Option<bool>});
dto!(CommitDetail{commit:CommitInfo,files:Vec<ChangedFile>});
dto!(LastCommit {
    subject: String,
    author_name: String,
    at: u64
});
dto!(BranchInfo{name:String,kind:String,remote:Option<String>,short_name:String,current:bool,sha:String,upstream:Option<RemoteRef>,upstream_gone:bool,ahead:Option<u64>,behind:Option<u64>,merged_into_current:bool,last_commit:Option<LastCommit>});
dto!(StashEntry{sha:String,index:u64,message:String,branch:Option<String>,at:u64});
dto!(ProjectDetail{project:Project,branches:Vec<BranchInfo>,stashes:Vec<StashEntry>});
dto!(ConflictFile {
    path: String,
    kind: ConflictKind,
    has_markers: bool
});
dto!(ConflictState{operation:Option<InProgressKind>,incoming:Option<String>,files:Vec<ConflictFile>,can_continue:bool});
dto!(ProjectItem{item_id:String,project_id:String,fingerprint:Option<String>});
dto!(PullItem{item_id:String,project_id:String,fingerprint:Option<String>,mode:PullMode,prefer:Option<PreferSide>});
dto!(PushItem{item_id:String,project_id:String,fingerprint:Option<String>,force:bool,set_upstream:bool});
dto!(CommitItem{item_id:String,project_id:String,fingerprint:Option<String>,message:String,paths:Option<Vec<String>>});
dto!(MergeItem{item_id:String,project_id:String,fingerprint:Option<String>,source:String,target:String,mode:MergeMode});
dto!(PublishItem{item_id:String,project_id:String,fingerprint:Option<String>,owner:Option<String>,name:String,visibility:String,description:Option<String>,initial_commit_message:Option<String>});
dto!(CloneItem{item_id:String,fingerprint:Option<String>,url:String,parent_dir:String,folder_name:String});
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationRequest {
    Fetch { items: Vec<ProjectItem> },
    Pull { items: Vec<PullItem> },
    Push { items: Vec<PushItem> },
    Commit { items: Vec<CommitItem> },
    Sync { items: Vec<ProjectItem> },
    Merge { items: Vec<MergeItem> },
    Publish { items: Vec<PublishItem> },
    Clone { items: Vec<CloneItem> },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "code",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PlanStep {
    Init,
    InitialCommit {
        files: u64,
    },
    Fetch,
    Pull {
        mode: PullMode,
        from: RemoteRef,
        commits: u64,
        prefer: Option<PreferSide>,
    },
    Push {
        to: RemoteRef,
        commits: Option<u64>,
        creates_remote_branch: bool,
        force: bool,
        set_upstream: bool,
    },
    Commit {
        files: u64,
    },
    Checkout {
        branch: String,
    },
    Merge {
        source: String,
        target: String,
        mode: MergeMode,
        commits: u64,
        fast_forward: bool,
    },
    CreateRepo {
        name_with_owner: String,
        visibility: String,
    },
    AddRemote {
        name: String,
        url: String,
    },
    Clone {
        url: String,
        destination: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "code",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PlanWarning {
    UncommittedChanges {
        files: u64,
    },
    StaleRemote {
        fetched_at: Option<u64>,
    },
    CreatesRemoteBranch {
        to: RemoteRef,
    },
    ConflictsLikely {
        paths: Vec<String>,
    },
    ConflictsResolved {
        prefer: PreferSide,
        paths: Vec<String>,
    },
    SkipsSensitiveFiles {
        paths: Vec<String>,
    },
    RewritesPublishedHistory {
        commits: u64,
    },
    PublicRepo,
    SensitiveFiles {
        paths: Vec<String>,
    },
    NoGitignore,
    LargeFiles {
        paths: Vec<String>,
    },
    ManyFiles {
        files: u64,
    },
}
dto!(PlanItem{item_id:String,project_id:Option<String>,executable:bool,blocker:Option<PlanBlocker>,steps:Vec<PlanStep>,risk:RiskLevel,warnings:Vec<PlanWarning>,fingerprint:Option<String>});
dto!(OperationPlan{kind:OperationKind,items:Vec<PlanItem>});
dto!(OperationItem{item_id:String,project_id:Option<String>,state:ItemState,step:Option<String>,progress:Option<f64>,completed:Vec<PlanStep>,reason:Option<PlanBlocker>,error:Option<crate::error::AppError>});
dto!(OperationSnapshot{operation_id:String,kind:OperationKind,state:String,started_at:u64,finished_at:Option<u64>,items:Vec<OperationItem>});
dto!(ToolInfo{installed:bool,version:Option<String>});
dto!(Environment {
    platform: String,
    app_version: String,
    git: ToolInfo,
    gh: ToolInfo
});
dto!(SettingsRecovery{kind:String,corrupt_copy_path:Option<String>});
dto!(AppState{settings:AppSettings,projects:Vec<Project>,environment:Environment,recovery:Option<SettingsRecovery>});
dto!(EditorInfo {
    id: String,
    name: String
});
dto!(HistoryEntry{id:String,at:u64,operation_id:Option<String>,project_id:Option<String>,project_name:String,action:String,outcome:String,params:BTreeMap<String,Value>,error_code:Option<String>});
dto!(ScanProgress {
    root_id: String,
    directories: u64,
    found: u64,
    done: bool
});
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum OperationEvent {
    Item {
        operation_id: String,
        item: OperationItem,
    },
    State {
        operation_id: String,
        snapshot: OperationSnapshot,
    },
}
