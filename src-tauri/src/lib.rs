mod background;
mod commands;
pub mod core;
mod details;
pub mod dto;
pub mod error;
pub mod git;
mod github;
mod history;
pub mod ignore;
pub mod markers;
mod mutations;
pub mod oauth;
pub mod operations;
pub mod platform;
mod previews;
mod publish;
pub mod registry;
pub mod removal;
pub mod settings;
pub mod status;
mod system;
pub mod tools;
pub mod trash;
pub mod update;
use serde_json::Value;
use tauri::{Emitter, Manager};
pub trait EventSink: Send + Sync {
    fn emit(&self, name: &str, payload: Value);
}
pub struct NoEvents;
impl EventSink for NoEvents {
    fn emit(&self, _: &str, _: Value) {}
}
struct TauriEvents(tauri::AppHandle);
impl EventSink for TauriEvents {
    fn emit(&self, name: &str, payload: Value) {
        let _ = self.0.emit_to("main", name, payload);
    }
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let core = core::Core::new(core::Options {
                oauth: Some(Default::default()),
                trash: std::sync::Arc::new(trash::OsTrash),
                config_dir: app.path().app_config_dir()?,
                data_dir: app.path().app_data_dir()?,
                cache_dir: app.path().app_cache_dir()?,
                git: git::Git::default(),
                events: std::sync::Arc::new(TauriEvents(app.handle().clone())),
            })
            .map_err(|e| std::io::Error::other(e.message))?;
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_ignore_candidates,
            commands::check_update,
            commands::download_update,
            commands::cancel_update_download,
            commands::install_update,
            commands::get_app_state,
            commands::get_environment,
            commands::pick_folder,
            commands::pick_background_image,
            commands::read_background_image,
            commands::clear_background_image,
            commands::open_project,
            commands::open_url,
            commands::list_editors,
            commands::update_ui_settings,
            commands::update_general_settings,
            commands::add_scan_root,
            commands::update_scan_root,
            commands::remove_scan_root,
            commands::add_project,
            commands::exclude_project,
            commands::restore_excluded_path,
            commands::update_project_settings,
            commands::set_branch_target,
            commands::scan_projects,
            commands::refresh_projects,
            commands::get_project_detail,
            commands::get_changes,
            commands::get_diff,
            commands::get_commit_log,
            commands::get_commit_detail,
            commands::get_conflicts,
            commands::plan_operation,
            commands::start_operation,
            commands::cancel_operation,
            commands::get_operation,
            commands::init_repository,
            commands::get_history,
            commands::create_branch,
            commands::checkout_branch,
            commands::rename_branch,
            commands::delete_branch,
            commands::delete_remote_branch,
            commands::set_upstream,
            commands::add_remote,
            commands::set_remote_url,
            commands::rename_remote,
            commands::remove_remote,
            commands::mark_resolved,
            commands::continue_in_progress,
            commands::abort_in_progress,
            commands::rebase,
            commands::cherry_pick,
            commands::reset,
            commands::discard_changes,
            commands::stash_save,
            commands::stash_apply,
            commands::stash_drop,
            commands::preview_merge,
            commands::preview_rebase,
            commands::preview_reset,
            commands::github_status,
            commands::install_github_cli,
            commands::github_login_start,
            commands::preview_removal,
            commands::check_repository_deletion,
            commands::remove_git,
            commands::delete_project,
            commands::github_login_cancel,
            commands::github_logout,
            commands::github_list_repos,
            commands::github_list_owners,
            commands::clear_history
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Revia");
}
