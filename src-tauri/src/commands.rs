use crate::{core::Core, error::Result};
use serde_json::Value;
use std::sync::Arc;
type Backend<'a> = tauri::State<'a, Arc<Core>>;
#[tauri::command]
pub async fn get_app_state(core: Backend<'_>) -> Result<Value> {
    Ok(core.app_state().await)
}
#[tauri::command]
pub async fn get_environment(core: Backend<'_>) -> Result<Value> {
    Ok(core.environment().await)
}
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle, title: Option<String>) -> Result<Option<String>> {
    crate::system::pick(app, title).await
}
#[tauri::command]
pub async fn open_url(app: tauri::AppHandle, url: String) -> Result<()> {
    crate::system::open_url(&app, &url)
}
#[tauri::command]
pub async fn open_project(
    core: Backend<'_>,
    app: tauri::AppHandle,
    project_id: String,
    target: String,
    relative_path: Option<String>,
) -> Result<()> {
    crate::system::open(&core, &app, &project_id, &target, relative_path).await
}
#[tauri::command]
pub async fn list_editors() -> Result<Vec<Value>> {
    Ok(crate::system::editors())
}
#[tauri::command]
pub async fn update_ui_settings(core: Backend<'_>, ui: Value) -> Result<()> {
    core.update_ui(ui).await
}
#[tauri::command]
pub async fn update_general_settings(core: Backend<'_>, patch: Value) -> Result<Value> {
    core.update_general(patch).await
}
#[tauri::command]
pub async fn add_scan_root(
    core: Backend<'_>,
    path: String,
    max_depth: Option<u64>,
) -> Result<Value> {
    core.registry_change(
        "add_root",
        Value::String(path),
        serde_json::json!(max_depth),
    )
    .await
}
#[tauri::command]
pub async fn update_scan_root(core: Backend<'_>, root_id: String, patch: Value) -> Result<Value> {
    core.registry_change("update_root", Value::String(root_id), patch)
        .await
}
#[tauri::command]
pub async fn remove_scan_root(core: Backend<'_>, root_id: String) -> Result<Value> {
    core.registry_change("remove_root", Value::String(root_id), Value::Null)
        .await
}
#[tauri::command]
pub async fn add_project(core: Backend<'_>, path: String) -> Result<Value> {
    core.registry_change("add_project", Value::String(path), Value::Null)
        .await
}
#[tauri::command]
pub async fn exclude_project(core: Backend<'_>, project_id: String) -> Result<Value> {
    core.registry_change("exclude", Value::String(project_id), Value::Null)
        .await
}
#[tauri::command]
pub async fn restore_excluded_path(core: Backend<'_>, path: String) -> Result<Value> {
    core.registry_change("restore", Value::String(path), Value::Null)
        .await
}
#[tauri::command]
pub async fn update_project_settings(
    core: Backend<'_>,
    project_id: String,
    patch: Value,
) -> Result<Value> {
    core.update_project(&project_id, patch).await
}
#[tauri::command]
pub async fn set_branch_target(
    core: Backend<'_>,
    project_id: String,
    branch: String,
    kind: String,
    target: Value,
) -> Result<Value> {
    core.set_target(&project_id, &branch, &kind, target).await
}
#[tauri::command]
pub async fn scan_projects(core: Backend<'_>) -> Result<Value> {
    core.scan().await
}
#[tauri::command]
pub async fn refresh_projects(
    core: Backend<'_>,
    project_ids: Option<Vec<String>>,
) -> Result<Vec<Value>> {
    core.refresh(project_ids).await
}
#[tauri::command]
pub async fn get_project_detail(core: Backend<'_>, project_id: String) -> Result<Value> {
    core.detail(&project_id).await
}
#[tauri::command]
pub async fn get_changes(core: Backend<'_>, project_id: String) -> Result<Vec<Value>> {
    core.changes(&project_id).await
}
#[tauri::command]
pub async fn get_diff(
    core: Backend<'_>,
    project_id: String,
    path: String,
    source: Value,
) -> Result<Value> {
    core.diff(&project_id, path, source).await
}
#[tauri::command]
pub async fn get_commit_log(
    core: Backend<'_>,
    project_id: String,
    rev: Option<String>,
    limit: u32,
    skip: u32,
) -> Result<Vec<Value>> {
    core.commit_log(&project_id, rev, limit, skip).await
}
#[tauri::command]
pub async fn get_commit_detail(
    core: Backend<'_>,
    project_id: String,
    sha: String,
) -> Result<Value> {
    core.commit_detail(&project_id, sha).await
}
#[tauri::command]
pub async fn get_conflicts(core: Backend<'_>, project_id: String) -> Result<Value> {
    core.conflicts(&project_id).await
}
#[tauri::command]
pub async fn init_repository(
    core: Backend<'_>,
    project_id: String,
    initial_branch: Option<String>,
) -> Result<Value> {
    core.init(&project_id, initial_branch).await
}
#[tauri::command]
pub async fn plan_operation(core: Backend<'_>, request: Value) -> Result<Value> {
    core.plan(request).await
}
#[tauri::command]
pub async fn start_operation(core: Backend<'_>, request: Value) -> Result<Value> {
    core.inner().start(request).await
}
#[tauri::command]
pub async fn cancel_operation(core: Backend<'_>, operation_id: String) -> Result<()> {
    core.cancel(&operation_id)
}
#[tauri::command]
pub async fn get_operation(core: Backend<'_>, operation_id: String) -> Result<Option<Value>> {
    Ok(core.operation(&operation_id))
}
#[tauri::command]
pub async fn get_history(
    core: Backend<'_>,
    limit: usize,
    before: Option<String>,
    project_id: Option<String>,
) -> Result<Vec<Value>> {
    core.history(limit, before, project_id).await
}
#[tauri::command]
pub async fn clear_history(core: Backend<'_>) -> Result<()> {
    core.clear_history().await
}

#[tauri::command]
pub async fn create_branch(
    core: Backend<'_>,
    project_id: String,
    name: String,
    start_point: Option<String>,
    checkout: bool,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "create_branch",
        serde_json::json!({"name":name,"startPoint":start_point,"checkout":checkout}),
    )
    .await
}

#[tauri::command]
pub async fn checkout_branch(
    core: Backend<'_>,
    project_id: String,
    branch: String,
    if_dirty: String,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "checkout_branch",
        serde_json::json!({"branch":branch,"ifDirty":if_dirty}),
    )
    .await
}

#[tauri::command]
pub async fn rename_branch(
    core: Backend<'_>,
    project_id: String,
    from: String,
    to: String,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "rename_branch",
        serde_json::json!({"from":from,"to":to}),
    )
    .await
}

#[tauri::command]
pub async fn delete_branch(
    core: Backend<'_>,
    project_id: String,
    name: String,
    force: bool,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "delete_branch",
        serde_json::json!({"name":name,"force":force}),
    )
    .await
}

#[tauri::command]
pub async fn delete_remote_branch(
    core: Backend<'_>,
    project_id: String,
    remote: String,
    branch: String,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "delete_remote_branch",
        serde_json::json!({"remote":remote,"branch":branch}),
    )
    .await
}

#[tauri::command]
pub async fn set_upstream(
    core: Backend<'_>,
    project_id: String,
    branch: String,
    upstream: Option<Value>,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "set_upstream",
        serde_json::json!({"branch":branch,"upstream":upstream}),
    )
    .await
}

#[tauri::command]
pub async fn add_remote(
    core: Backend<'_>,
    project_id: String,
    name: String,
    url: String,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "add_remote",
        serde_json::json!({"name":name,"url":url}),
    )
    .await
}

#[tauri::command]
pub async fn set_remote_url(
    core: Backend<'_>,
    project_id: String,
    name: String,
    url: String,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "set_remote_url",
        serde_json::json!({"name":name,"url":url}),
    )
    .await
}

#[tauri::command]
pub async fn rename_remote(
    core: Backend<'_>,
    project_id: String,
    from: String,
    to: String,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "rename_remote",
        serde_json::json!({"from":from,"to":to}),
    )
    .await
}

#[tauri::command]
pub async fn remove_remote(core: Backend<'_>, project_id: String, name: String) -> Result<Value> {
    core.mutate(
        &project_id,
        "remove_remote",
        serde_json::json!({"name":name}),
    )
    .await
}

#[tauri::command]
pub async fn mark_resolved(
    core: Backend<'_>,
    project_id: String,
    paths: Vec<String>,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "mark_resolved",
        serde_json::json!({"paths":paths}),
    )
    .await
}

#[tauri::command]
pub async fn continue_in_progress(core: Backend<'_>, project_id: String) -> Result<Value> {
    core.mutate(&project_id, "continue_in_progress", serde_json::json!({}))
        .await
}

#[tauri::command]
pub async fn abort_in_progress(core: Backend<'_>, project_id: String) -> Result<Value> {
    core.mutate(&project_id, "abort_in_progress", serde_json::json!({}))
        .await
}

#[tauri::command]
pub async fn rebase(
    core: Backend<'_>,
    project_id: String,
    onto: String,
    fingerprint: String,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "rebase",
        serde_json::json!({"onto":onto,"fingerprint":fingerprint}),
    )
    .await
}

#[tauri::command]
pub async fn cherry_pick(
    core: Backend<'_>,
    project_id: String,
    shas: Vec<String>,
) -> Result<Value> {
    core.mutate(&project_id, "cherry_pick", serde_json::json!({"shas":shas}))
        .await
}

#[tauri::command]
pub async fn reset(
    core: Backend<'_>,
    project_id: String,
    target: String,
    mode: String,
    fingerprint: String,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "reset",
        serde_json::json!({"target":target,"mode":mode,"fingerprint":fingerprint}),
    )
    .await
}

#[tauri::command]
pub async fn discard_changes(
    core: Backend<'_>,
    project_id: String,
    paths: Option<Vec<String>>,
    include_untracked: bool,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "discard_changes",
        serde_json::json!({"paths":paths,"includeUntracked":include_untracked}),
    )
    .await
}

#[tauri::command]
pub async fn stash_save(
    core: Backend<'_>,
    project_id: String,
    message: Option<String>,
    include_untracked: bool,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "stash_save",
        serde_json::json!({"message":message,"includeUntracked":include_untracked}),
    )
    .await
}

#[tauri::command]
pub async fn stash_apply(
    core: Backend<'_>,
    project_id: String,
    stash_sha: String,
    drop: bool,
) -> Result<Value> {
    core.mutate(
        &project_id,
        "stash_apply",
        serde_json::json!({"stashSha":stash_sha,"drop":drop}),
    )
    .await
}

#[tauri::command]
pub async fn stash_drop(core: Backend<'_>, project_id: String, stash_sha: String) -> Result<Value> {
    core.mutate(
        &project_id,
        "stash_drop",
        serde_json::json!({"stashSha":stash_sha}),
    )
    .await
}

#[tauri::command]
pub async fn preview_merge(
    core: Backend<'_>,
    project_id: String,
    source: String,
    target: String,
) -> Result<Value> {
    core.preview_merge(&project_id, &source, &target).await
}

#[tauri::command]
pub async fn preview_rebase(core: Backend<'_>, project_id: String, onto: String) -> Result<Value> {
    core.preview_rebase(&project_id, &onto).await
}

#[tauri::command]
pub async fn preview_reset(
    core: Backend<'_>,
    project_id: String,
    target: String,
    mode: String,
) -> Result<Value> {
    core.preview_reset(&project_id, &target, &mode).await
}

#[tauri::command]
pub async fn github_status(core: Backend<'_>, refresh: bool) -> Result<Value> {
    core.github_status(refresh).await
}
#[tauri::command]
pub async fn github_login_start(core: Backend<'_>, scopes: Option<Vec<String>>) -> Result<Value> {
    core.github_login_start(scopes).await
}
#[tauri::command]
pub async fn github_login_cancel(core: Backend<'_>, login_id: String) -> Result<()> {
    core.github_login_cancel(&login_id);
    Ok(())
}
#[tauri::command]
pub async fn github_logout(core: Backend<'_>) -> Result<Value> {
    core.github_logout().await
}
#[tauri::command]
pub async fn github_list_repos(core: Backend<'_>, refresh: bool) -> Result<Vec<Value>> {
    core.github_list_repos(refresh).await
}
#[tauri::command]
pub async fn github_list_owners(core: Backend<'_>) -> Result<Vec<Value>> {
    core.github_list_owners().await
}

#[tauri::command]
pub async fn pick_background_image(
    filter_label: Option<String>,
    core: Backend<'_>,
    app: tauri::AppHandle,
) -> Result<Option<Value>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(
            crate::background::normalize_filter_label(filter_label.as_deref()),
            &["png", "jpg", "jpeg", "webp", "gif", "avif"],
        )
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    match rx
        .await
        .map_err(|_| crate::error::AppError::new("io_error"))?
    {
        Some(path) => {
            let path = path
                .into_path()
                .map_err(|_| crate::error::AppError::new("invalid_input"))?;
            core.copy_background_image(&path).await.map(Some)
        }
        None => Ok(None),
    }
}
#[tauri::command]
pub async fn read_background_image(core: Backend<'_>, id: String) -> Result<tauri::ipc::Response> {
    core.read_background_image(&id)
        .await
        .map(tauri::ipc::Response::new)
}
#[tauri::command]
pub async fn clear_background_image(core: Backend<'_>) -> Result<()> {
    core.clear_background_image().await
}

#[tauri::command]
pub async fn check_update(core: Backend<'_>) -> Result<Value> {
    core.check_update().await
}
#[tauri::command]
pub async fn download_update(core: Backend<'_>) -> Result<Value> {
    core.download_update().await
}
#[tauri::command]
pub async fn cancel_update_download(core: Backend<'_>) -> Result<()> {
    core.cancel_update_download();
    Ok(())
}
#[tauri::command]
pub async fn install_update(core: Backend<'_>, app: tauri::AppHandle) -> Result<()> {
    core.install_update(&app).await
}

#[tauri::command]
pub async fn preview_removal(core: Backend<'_>, project_id: String) -> Result<Value> {
    core.preview_removal(&project_id).await
}
#[tauri::command]
pub async fn check_repository_deletion(core: Backend<'_>, project_id: String) -> Result<Value> {
    core.check_repository_deletion(&project_id).await
}
#[tauri::command]
pub async fn remove_git(
    core: Backend<'_>,
    project_id: String,
    fingerprint: String,
    confirm: String,
) -> Result<Value> {
    core.remove_git(&project_id, &fingerprint, &confirm).await
}
#[tauri::command]
pub async fn delete_project(
    core: Backend<'_>,
    project_id: String,
    delete_repository: bool,
    fingerprint: String,
    confirm: String,
) -> Result<Value> {
    core.delete_project(&project_id, delete_repository, &fingerprint, &confirm)
        .await
}

#[tauri::command]
pub async fn scan_ignore_candidates(core: Backend<'_>, project_id: String) -> Result<Value> {
    core.scan_ignore_candidates(&project_id).await
}
