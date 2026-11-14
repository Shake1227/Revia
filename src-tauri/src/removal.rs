use crate::{
    core::Core,
    error::{AppError, Result},
    settings,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub fn validate_deletion_path(
    path: &Path,
    symlink: bool,
    home: Option<&Path>,
    protected: &[PathBuf],
    projects: &[PathBuf],
) -> Result<()> {
    if !path.is_absolute()
        || symlink
        || path.parent().is_none()
        || home.is_some_and(|h| h.starts_with(path))
        || protected.iter().any(|p| p.starts_with(path))
    {
        return Err(AppError::new("invalid_path"));
    }
    let nested = projects
        .iter()
        .filter(|p| p.as_path() != path && p.starts_with(path))
        .count();
    if nested > 0 {
        return Err(AppError::new("invalid_input")
            .detail(&format!("Folder contains {nested} registered projects")));
    }
    Ok(())
}

pub fn has_delete_scope(output: &str) -> Option<bool> {
    let mut active = true;
    let mut found = None;
    for line in output.lines() {
        let line = line.to_ascii_lowercase();
        if let Some((_, value)) = line.split_once("active account:") {
            active = value.trim() == "true";
        }
        if active && let Some((_, scopes)) = line.split_once("scopes:") {
            found = Some(
                scopes
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .any(|s| s.trim_matches(['\'', '"']) == "delete_repo"),
            );
        }
    }
    found
}
fn protected_path(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(AppError::new("invalid_path"));
    }
    let mut ancestor = path;
    let mut suffix = Vec::new();
    loop {
        match crate::platform::canonicalize(ancestor) {
            Ok(mut resolved) => {
                for component in suffix.into_iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                suffix.push(
                    ancestor
                        .file_name()
                        .ok_or_else(|| AppError::new("invalid_path"))?
                        .to_os_string(),
                );
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| AppError::new("invalid_path"))?;
            }
            Err(e) => return Err(e.into()),
        }
    }
}
impl Core {
    fn removal_busy(&self, id: &str) -> bool {
        self.state.lock().unwrap().operations.values().any(|o| {
            o["state"] != "finished"
                && o["items"].as_array().is_some_and(|items| {
                    items.iter().any(|i| {
                        i["projectId"] == id
                            && matches!(i["state"].as_str(), Some("queued" | "running"))
                    })
                })
        })
    }
    pub async fn preview_removal(&self, id: &str) -> Result<Value> {
        let project = self.project(id)?;
        let path = Path::new(project["path"].as_str().unwrap());
        if !path.is_dir() {
            return Err(AppError::new("path_not_found"));
        }
        let nested: Vec<String> = {
            let s = self.state.lock().unwrap();
            let mut paths: Vec<_> = s
                .projects
                .iter()
                .filter_map(|p| p["path"].as_str())
                .filter(|p| Path::new(p) != path && Path::new(p).starts_with(path))
                .map(str::to_owned)
                .collect();
            paths.sort();
            paths
        };
        let repository = path.join(".git").exists();
        let mut preview = json!({"projectId":id,"path":project["path"],"repository":repository,"uncommittedFiles":0,"unpushedCommits":0,"localOnlyBranches":[],"stashes":0,"remotes":[],"github":null,"nestedProjects":nested,"fingerprint":""});
        let mut head = String::new();
        let mut branch = String::new();
        if repository {
            let git = &self.options.git;
            let raw = git
                .local(
                    path,
                    &[
                        "status",
                        "--porcelain=v2",
                        "--branch",
                        "-z",
                        "--untracked-files=normal",
                    ],
                )
                .await?;
            let mut changes = 0;
            let mut records = raw.split('\0');
            while let Some(record) = records.next() {
                if let Some(value) = record.strip_prefix("# branch.oid ") {
                    head = value.into();
                }
                if let Some(value) = record.strip_prefix("# branch.head ") {
                    branch = value.into();
                }
                if matches!(record.as_bytes().first(), Some(b'1' | b'2' | b'u' | b'?')) {
                    changes += 1;
                }
                if record.starts_with('2') {
                    records.next();
                }
            }
            preview["uncommittedFiles"] = json!(changes);
            let config = git
                .local(
                    path,
                    &["config", "--get-regexp", "^remote\\..*\\.(url|pushurl)$"],
                )
                .await;
            let config = match config {
                Ok(v) => v,
                Err(e) if e.code == "unknown" => String::new(),
                Err(e) => return Err(e),
            };
            let mut urls = BTreeMap::<String, (String, Option<String>)>::new();
            for line in config.lines() {
                let Some((key, value)) = line.split_once(' ') else {
                    continue;
                };
                let Some(key) = key.strip_prefix("remote.") else {
                    continue;
                };
                if let Some(name) = key.strip_suffix(".pushurl") {
                    urls.entry(name.into()).or_default().1 = Some(value.into());
                } else if let Some(name) = key.strip_suffix(".url") {
                    let entry = urls.entry(name.into()).or_default();
                    if entry.0.is_empty() {
                        entry.0 = value.into();
                    }
                }
            }
            let remotes: Vec<_> = urls.into_iter().map(|(name,(url,push))| json!({"name":name,"fetchUrl":crate::error::strip_userinfo(&url),"pushUrl":crate::error::strip_userinfo(push.as_deref().unwrap_or(&url)),"github":crate::status::github(&url)})).collect();
            preview["github"] = remotes
                .iter()
                .find(|r| r["name"] == "origin" && !r["github"].is_null())
                .or_else(|| remotes.iter().find(|r| !r["github"].is_null()))
                .map(|r| r["github"].clone())
                .unwrap_or(Value::Null);
            let refs = git
                .local(
                    path,
                    &[
                        "for-each-ref",
                        "--format=%(refname)",
                        "refs/heads",
                        "refs/remotes",
                        "refs/stash",
                    ],
                )
                .await?;
            let mut local = vec![];
            let mut remote = vec![];
            for line in refs.lines() {
                if let Some(name) = line.strip_prefix("refs/heads/") {
                    local.push(name.to_string());
                }
                if let Some(name) = line.strip_prefix("refs/remotes/") {
                    if !remotes
                        .iter()
                        .any(|r| name.starts_with(&format!("{}/", r["name"].as_str().unwrap())))
                        && let Some((_, branch)) = name.split_once('/')
                    {
                        remote.push(branch.to_string());
                    }
                    for r in &remotes {
                        if let Some(b) =
                            name.strip_prefix(&format!("{}/", r["name"].as_str().unwrap()))
                        {
                            remote.push(b.to_string());
                        }
                    }
                }
            }
            local.retain(|b| !remote.contains(b));
            local.sort();
            preview["localOnlyBranches"] = json!(local);
            preview["remotes"] = json!(remotes);
            preview["unpushedCommits"] = json!(
                git.local(
                    path,
                    &["rev-list", "--count", "--branches", "--not", "--remotes"]
                )
                .await
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
            );
            preview["stashes"] = json!(
                git.local(path, &["stash", "list", "--format=%H"])
                    .await?
                    .lines()
                    .count()
            );
        }
        let identity = json!([
            head,
            branch,
            preview["repository"],
            preview["uncommittedFiles"],
            preview["stashes"],
            preview["remotes"],
            preview["unpushedCommits"]
        ]);
        preview["fingerprint"] = json!(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&identity)?)
        ));
        Ok(preview)
    }
    fn removal_repo(preview: &Value) -> Result<String> {
        let owner = preview["github"]["owner"]
            .as_str()
            .ok_or_else(|| AppError::new("invalid_input"))?;
        let repo = preview["github"]["repo"]
            .as_str()
            .ok_or_else(|| AppError::new("invalid_input"))?;
        if [owner, repo].iter().any(|s| {
            s.is_empty()
                || s.starts_with('-')
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        }) {
            return Err(AppError::new("invalid_input"));
        }
        Ok(format!("{owner}/{repo}"))
    }
    pub async fn check_repository_deletion(&self, id: &str) -> Result<Value> {
        let preview = self.preview_removal(id).await?;
        let repo = Self::removal_repo(&preview)?;
        let mut result =
            json!({"nameWithOwner":repo,"visibility":null,"isFork":null,"blocker":null});
        let scopes = match self
            .gh(&["auth", "status", "--hostname", "github.com"])
            .await
        {
            Ok((out, err)) => format!("{out}\n{err}"),
            Err(e)
                if matches!(
                    e.code.as_str(),
                    "gh_not_installed" | "not_authenticated" | "auth_failed" | "unknown"
                ) =>
            {
                result["blocker"] = json!(if e.code == "gh_not_installed" {
                    "gh_not_installed"
                } else {
                    "not_authenticated"
                });
                return Ok(result);
            }
            Err(e) => return Err(e),
        };
        let (out, _) = match self
            .gh(&[
                "repo",
                "view",
                &repo,
                "--json",
                "nameWithOwner,visibility,isFork,viewerCanAdminister",
            ])
            .await
        {
            Ok(v) => v,
            Err(e) if matches!(e.code.as_str(), "remote_not_found" | "permission_denied") => {
                result["blocker"] = json!(if e.code == "remote_not_found" {
                    "not_found"
                } else {
                    "no_permission"
                });
                return Ok(result);
            }
            Err(e) => return Err(e),
        };
        let view: Value = serde_json::from_str(&out)?;
        result["visibility"] = view["visibility"]
            .as_str()
            .map(|v| json!(v.to_ascii_lowercase()))
            .unwrap_or(Value::Null);
        result["isFork"] = view["isFork"].clone();
        result["blocker"] = if view["viewerCanAdminister"] != true {
            json!("no_permission")
        } else if has_delete_scope(&scopes) == Some(false) {
            json!("scope_missing")
        } else {
            Value::Null
        };
        Ok(result)
    }
    async fn trash_path(&self, path: &Path) -> Result<()> {
        let trash = self.options.trash.clone();
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            if std::fs::symlink_metadata(&path)?.file_type().is_symlink()
                || crate::platform::canonicalize(&path)? != path
            {
                return Err(std::io::Error::other(
                    "Removal path changed before moving to trash",
                ));
            }
            trash.move_to_trash(&path)
        })
        .await
        .map_err(|_| AppError::new("io_error"))??;
        Ok(())
    }
    pub async fn remove_git(&self, id: &str, fingerprint: &str, confirm: &str) -> Result<Value> {
        let lock = self.lock(id);
        let _guard = lock.try_lock().map_err(|_| AppError::new("busy"))?;
        if self.removal_busy(id) {
            return Err(AppError::new("busy"));
        }
        let p = self.project(id)?;
        if p["name"] != confirm {
            return Err(AppError::new("invalid_input"));
        }
        let preview = self.preview_removal(id).await?;
        if preview["fingerprint"] != fingerprint {
            return Err(AppError::new("state_changed"));
        }
        if preview["repository"] != true {
            return Err(AppError::new("not_a_repository"));
        }
        let dotgit = Path::new(p["path"].as_str().unwrap()).join(".git");
        if !std::fs::symlink_metadata(&dotgit)?.is_dir() {
            return Err(AppError::new("invalid_input").detail("The .git entry is not a directory; linked worktrees and submodules are unsupported"));
        }
        let result = self.trash_path(&dotgit).await;
        if result.is_ok() {
            let _settings = self.settings_lock.lock().await;
            let mut state = self.state.lock().unwrap();
            let path = p["path"].as_str().unwrap();
            let s = state
                .settings
                .project_settings
                .entry(path.into())
                .or_insert_with(settings::project_defaults);
            s["branchTargets"] = json!({});
            s["defaultRemote"] = Value::Null;
            s["defaultBranch"] = Value::Null;
            for project in &mut state.projects {
                if project["id"] == id {
                    project["settings"]["branchTargets"] = json!({});
                    project["settings"]["defaultRemote"] = Value::Null;
                    project["settings"]["defaultBranch"] = Value::Null;
                }
            }
            settings::save(&self.options.config_dir, &state.settings)?;
        }
        self.refresh(Some(vec![id.into()])).await?;
        self.options
            .events
            .emit("revia:project-updated", self.project(id)?);
        self.history_record_params(
            None,
            id,
            "remove_git",
            if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
            result.as_ref().err(),
            json!({"name":p["name"]}),
        )
        .await?;
        result?;
        self.cache();
        self.project(id)
    }
    pub async fn delete_project(
        &self,
        id: &str,
        delete_repository: bool,
        fingerprint: &str,
        confirm: &str,
    ) -> Result<Value> {
        let lock = self.lock(id);
        let _guard = lock.try_lock().map_err(|_| AppError::new("busy"))?;
        if self.removal_busy(id) {
            return Err(AppError::new("busy"));
        }
        let _scan = self.scan_lock.lock().await;
        let settings_guard = self.settings_lock.lock().await;
        let p = self.project(id)?;
        let path = Path::new(p["path"].as_str().unwrap());
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(AppError::new("invalid_path"));
        }
        let preview = self.preview_removal(id).await?;
        let protected = [
            &self.options.config_dir,
            &self.options.data_dir,
            &self.options.cache_dir,
        ]
        .iter()
        .map(|p| protected_path(p))
        .collect::<Result<Vec<_>>>()?;
        let home = protected_path(&crate::platform::home().ok_or_else(|| {
            AppError::new("invalid_path").detail("Cannot locate the home directory")
        })?)?;
        let projects = {
            let state = self.state.lock().unwrap();
            state
                .projects
                .iter()
                .filter_map(|p| p["path"].as_str())
                .chain(state.settings.manual_paths.iter().map(String::as_str))
                .chain(state.settings.project_settings.keys().map(String::as_str))
                .map(PathBuf::from)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        };
        validate_deletion_path(path, false, Some(&home), &protected, &projects)?;
        let repo = if delete_repository {
            Some(Self::removal_repo(&preview)?)
        } else {
            None
        };
        if confirm != repo.as_deref().unwrap_or(p["name"].as_str().unwrap()) {
            return Err(AppError::new("invalid_input"));
        }
        if preview["fingerprint"] != fingerprint {
            return Err(AppError::new("state_changed"));
        }
        let mut params = json!({"name":p["name"]});
        if let Some(repo) = &repo {
            params["repo"] = json!(repo);
            if let Err(e) = self.gh(&["repo", "delete", repo, "--yes"]).await {
                drop(settings_guard);
                self.refresh(Some(vec![id.into()])).await?;
                self.options
                    .events
                    .emit("revia:project-updated", self.project(id)?);
                self.history_record_snapshot(
                    None,
                    id,
                    "delete_repository",
                    "failed",
                    Some(&e),
                    params,
                    &p,
                )
                .await?;
                return Err(e);
            }
            self.invalidate_github();
        }
        let local = self.trash_path(path).await;
        if local.is_ok() {
            let mut state = self.state.lock().unwrap();
            let mut settings = state.settings.clone();
            settings.manual_paths.retain(|s| Path::new(s) != path);
            settings.project_settings.remove(path.to_str().unwrap());
            settings.settings["roots"]
                .as_array_mut()
                .unwrap()
                .retain(|r| r["path"] != p["path"]);
            settings::save(&self.options.config_dir, &settings)?;
            state.settings = settings;
            state.projects.retain(|p| p["id"] != id);
            state.remote_errors.remove(id);
            state.fetched.remove(id);
        }
        drop(settings_guard);
        let repository_history = if delete_repository {
            self.history_record_snapshot(
                None,
                id,
                "delete_repository",
                "succeeded",
                None,
                params.clone(),
                &p,
            )
            .await
        } else {
            Ok(())
        };
        let local_history = self
            .history_record_snapshot(
                None,
                id,
                "delete_local",
                if local.is_ok() { "succeeded" } else { "failed" },
                local.as_ref().err(),
                params,
                &p,
            )
            .await;
        if let Err(mut e) = local {
            self.refresh(Some(vec![id.into()])).await?;
            self.options
                .events
                .emit("revia:project-updated", self.project(id)?);
            if repository_history.is_err() || local_history.is_err() {
                e = e.detail(
                    "Moving the folder to trash failed; recording removal history also failed",
                );
            }
            if !delete_repository {
                return Err(e);
            }
            return Ok(
                json!({"repository":"deleted","local":"failed","error":e,"workspace":self.workspace()}),
            );
        }
        self.cache();
        let workspace = self.workspace();
        self.options
            .events
            .emit("revia:projects-changed", workspace["projects"].clone());
        repository_history?;
        local_history?;
        Ok(
            json!({"repository":if delete_repository {"deleted"} else {"skipped"},"local":"trashed","error":null,"workspace":workspace}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn removal_rejects_locked_and_queued_operations() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::new(crate::core::Options {
            oauth: None,
            config_dir: temp.path().join("config"),
            data_dir: temp.path().join("data"),
            cache_dir: temp.path().join("cache"),
            git: crate::git::Git::default(),
            events: std::sync::Arc::new(crate::NoEvents),
            trash: std::sync::Arc::new(crate::trash::MoveTo(temp.path().join("trash"))),
        })
        .unwrap();
        let lock = core.lock("test");
        let guard = lock.lock().await;
        assert_eq!(
            core.remove_git("test", "fingerprint", "name")
                .await
                .unwrap_err()
                .code,
            "busy"
        );
        assert_eq!(
            core.delete_project("test", false, "fingerprint", "name")
                .await
                .unwrap_err()
                .code,
            "busy"
        );
        drop(guard);
        for state in ["queued", "running"] {
            core.state.lock().unwrap().operations.insert(
                "operation".into(),
                json!({"state":"running","items":[{"projectId":"test","state":state}]}),
            );
            assert_eq!(
                core.remove_git("test", "fingerprint", "name")
                    .await
                    .unwrap_err()
                    .code,
                "busy"
            );
            assert_eq!(
                core.delete_project("test", false, "fingerprint", "name")
                    .await
                    .unwrap_err()
                    .code,
                "busy"
            );
        }
    }
    #[test]
    fn scopes_are_exact_and_only_active_accounts_count() {
        for (output, expected) in [
            ("Token scopes: 'repo', 'delete_repo'", Some(true)),
            ("Token scopes: 'repo', 'delete_repository'", Some(false)),
            ("Token: hidden", None),
            (
                "Active account: true\nToken scopes: 'repo'\nActive account: false\nToken scopes: 'delete_repo'",
                Some(false),
            ),
            ("Active account: false\nToken scopes: 'delete_repo'", None),
        ] {
            assert_eq!(has_delete_scope(output), expected);
        }
    }
    #[test]
    fn protected_paths_and_nested_projects() {
        let root = if cfg!(windows) {
            Path::new("C:\\")
        } else {
            Path::new("/")
        };
        let home = root.join("users/test");
        let project = home.join("project");
        let protected = vec![
            home.join(".config/revia"),
            home.join(".local/revia"),
            home.join(".cache/revia"),
        ];
        for p in [
            root.to_path_buf(),
            home.clone(),
            root.join("users"),
            home.join(".config"),
            home.join(".local"),
            home.join(".cache"),
        ] {
            assert_eq!(
                validate_deletion_path(&p, false, Some(&home), &protected, &[])
                    .unwrap_err()
                    .code,
                "invalid_path"
            );
        }
        assert!(
            validate_deletion_path(
                &project,
                false,
                Some(&home),
                &protected,
                std::slice::from_ref(&project)
            )
            .is_ok()
        );
        assert_eq!(
            validate_deletion_path(&project, true, Some(&home), &protected, &[])
                .unwrap_err()
                .code,
            "invalid_path"
        );
        assert_eq!(
            validate_deletion_path(
                &project,
                false,
                Some(&home),
                &protected,
                &[project.join("child")]
            )
            .unwrap_err()
            .code,
            "invalid_input"
        );
        assert!(
            validate_deletion_path(
                &project,
                false,
                Some(&home),
                &protected,
                &[home.join("project-other")]
            )
            .is_ok()
        );
    }
}
