use crate::{
    core::Core,
    details::resolve,
    error::{AppError, Result},
    git, settings, status,
};
use serde_json::{Value, json};
use std::path::Path;
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v[k].as_str().ok_or_else(|| AppError::new("invalid_input"))
}
fn boolean(v: &Value, k: &str) -> Result<bool> {
    v[k].as_bool().ok_or_else(|| AppError::new("invalid_input"))
}
impl Core {
    pub async fn mutate(&self, id: &str, action: &str, args: Value) -> Result<Value> {
        let lock = self.lock(id);
        let _guard = lock.lock().await;
        self.refresh(Some(vec![id.into()])).await?;
        let p = self.project(id)?;
        let branch = p["status"]["branch"].clone();
        let result = self.mutate_inner(id, action, &args, &p).await;
        let refresh = self.refresh(Some(vec![id.into()])).await;
        if let Ok(p) = self.project(id) {
            self.options.events.emit("revia:project-updated", p);
        }
        let mut params = json!({});
        for key in [
            "target", "source", "remote", "mode", "sha", "name", "onto", "from", "to", "stashSha",
        ] {
            if !args[key].is_null() {
                params[key] = args[key].clone();
            }
        }
        if let Some(paths) = args["paths"].as_array() {
            params["files"] = json!(paths.len());
        }
        if let Some(shas) = args["shas"].as_array()
            && result.is_ok()
        {
            params["commits"] = json!(shas.len());
        }
        if !args["stashSha"].is_null() {
            params["sha"] = args["stashSha"].clone();
        }
        if let Some(branch) = args["branch"].as_str() {
            params["target"] = json!(if let Some(remote) = args["remote"].as_str() {
                format!("{remote}/{branch}")
            } else {
                branch.into()
            });
        }

        params["branch"] = branch;
        self.history_record_params(
            None,
            id,
            action,
            if result.is_ok() {
                "succeeded"
            } else if result.as_ref().is_err_and(|e| e.code == "merge_conflict") {
                "conflict"
            } else {
                "failed"
            },
            result.as_ref().err(),
            params,
        )
        .await?;
        let output = result?;
        refresh?;
        match action {
            "mark_resolved" => self.conflicts(id).await,
            "discard_changes" => Ok(json!(self.changes(id).await?)),
            _ => {
                let _ = output;
                self.detail(id).await
            }
        }
    }
    async fn stash_index(&self, root: &Path, sha: &str) -> Result<String> {
        if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AppError::new("invalid_input"));
        }
        let raw = self
            .options
            .git
            .local(root, &["stash", "list", "--format=%H"])
            .await?;
        raw.lines()
            .position(|s| s == sha)
            .map(|i| format!("stash@{{{i}}}"))
            .ok_or_else(|| AppError::new("state_changed"))
    }
    async fn migrate_settings(&self, id: &str, action: &str, from: &str, to: &str) -> Result<()> {
        let project = self.project(id)?;
        let path = project["path"].as_str().unwrap();
        let _guard = self.settings_lock.lock().await;
        let mut state = self.state.lock().unwrap();
        let mut next = state.settings.clone();
        let ps = next
            .project_settings
            .entry(path.into())
            .or_insert_with(settings::project_defaults);
        match action {
            "rename_branch" => {
                if let Some(target) = ps["branchTargets"]
                    .as_object_mut()
                    .and_then(|o| o.remove(from))
                {
                    ps["branchTargets"][to] = target;
                }
                if ps["defaultBranch"] == from {
                    ps["defaultBranch"] = json!(to);
                }
            }
            "rename_remote" => {
                if ps["defaultRemote"] == from {
                    ps["defaultRemote"] = json!(to);
                }
                for v in ps["branchTargets"]
                    .as_object_mut()
                    .into_iter()
                    .flat_map(|o| o.values_mut())
                {
                    for k in ["push", "pull"] {
                        if v[k]["remote"] == from {
                            v[k]["remote"] = json!(to);
                        }
                    }
                }
            }
            _ => {}
        }
        let ps = ps.clone();
        settings::save(&self.options.config_dir, &next)?;
        state.settings = next;
        if let Some(p) = state.projects.iter_mut().find(|p| p["id"] == id) {
            p["settings"] = ps;
        }
        Ok(())
    }
    async fn mutate_inner(&self, id: &str, action: &str, a: &Value, p: &Value) -> Result<Value> {
        let root = Path::new(p["path"].as_str().unwrap());
        let g = &self.options.git;
        if !root.join(".git").exists() {
            return Err(AppError::new("not_a_repository"));
        }
        let in_progress = !p["status"]["inProgress"].is_null();
        if in_progress
            && !matches!(
                action,
                "mark_resolved" | "continue_in_progress" | "abort_in_progress"
            )
        {
            return Err(AppError::new("operation_in_progress"));
        }
        match action {
            "create_branch" => {
                let name = text(a, "name")?;
                g.reference(root, name).await?;
                if g.local(
                    root,
                    &[
                        "show-ref",
                        "--verify",
                        "--quiet",
                        &format!("refs/heads/{name}"),
                    ],
                )
                .await
                .is_ok()
                {
                    return Err(AppError::new("branch_exists"));
                }
                let start = resolve(g, root, a["startPoint"].as_str().unwrap_or("HEAD")).await?;
                if boolean(a, "checkout")? {
                    g.local_write(root, &["checkout", "-b", name, &start])
                        .await?;
                } else {
                    g.local(root, &["branch", name, &start]).await?;
                }
            }
            "checkout_branch" => {
                let branch = text(a, "branch")?;
                let strategy = text(a, "ifDirty")?;
                if !matches!(strategy, "abort" | "carry" | "stash") {
                    return Err(AppError::new("invalid_input"));
                }
                g.reference(root, branch).await?;
                let dirty = p["status"]["changes"]["total"].as_u64().unwrap_or(0) > 0;
                if dirty && strategy == "abort" {
                    return Err(AppError::new("dirty_worktree"));
                }
                let mut command = vec!["checkout".to_string()];
                if g.local(
                    root,
                    &[
                        "show-ref",
                        "--verify",
                        "--quiet",
                        &format!("refs/heads/{branch}"),
                    ],
                )
                .await
                .is_ok()
                {
                    command.push(branch.into());
                } else {
                    let remotes = p["status"]["remotes"].as_array().unwrap();
                    let short = remotes
                        .iter()
                        .filter_map(|r| branch.strip_prefix(&format!("{}/", r["name"].as_str()?)))
                        .next()
                        .ok_or_else(|| AppError::new("branch_not_found"))?;
                    let remote = resolve(g, root, &format!("refs/remotes/{branch}")).await?;
                    let _ = remote;
                    if g.local(
                        root,
                        &[
                            "show-ref",
                            "--verify",
                            "--quiet",
                            &format!("refs/heads/{short}"),
                        ],
                    )
                    .await
                    .is_ok()
                    {
                        command.push(short.into());
                    } else {
                        command.extend([
                            "--track".into(),
                            "-b".into(),
                            short.into(),
                            format!("refs/remotes/{branch}"),
                        ]);
                    }
                }
                let stashed = dirty && strategy == "stash";
                let mut stash_sha = None;
                if stashed {
                    let before = resolve(g, root, "refs/stash").await.ok();
                    g.local_write(
                        root,
                        &[
                            "stash",
                            "push",
                            "--include-untracked",
                            "-m",
                            "Revia checkout",
                        ],
                    )
                    .await?;
                    let after = resolve(g, root, "refs/stash").await.ok();
                    if after != before {
                        stash_sha = after;
                    }
                }
                if let Err(e) = g.write(root, &command).await {
                    if let Some(sha) = stash_sha {
                        let index = self.stash_index(root, &sha).await?;
                        match g.local_write(root,&["stash","apply","--index",&index]).await {
                            Ok(_)=>{g.local_write(root,&["stash","drop",&index]).await?;}
                            Err(restore)=>return Err(AppError::new("dirty_worktree").detail(&format!("Checkout failed and the stash could not be restored. The stash was kept.\n{}",restore.detail.as_deref().unwrap_or("")))),
                        }
                    }
                    return Err(e);
                }
            }
            "rename_branch" => {
                let from = text(a, "from")?;
                let to = text(a, "to")?;
                g.reference(root, from).await?;
                g.reference(root, to).await?;
                if g.local(
                    root,
                    &[
                        "show-ref",
                        "--verify",
                        "--quiet",
                        &format!("refs/heads/{to}"),
                    ],
                )
                .await
                .is_ok()
                {
                    return Err(AppError::new("branch_exists"));
                }
                g.local(root, &["branch", "-m", from, to]).await?;
                self.migrate_settings(id, action, from, to).await?;
            }
            "delete_branch" => {
                let branch = text(a, "name")?;
                g.reference(root, branch).await?;
                if p["status"]["branch"] == branch {
                    return Err(AppError::new("invalid_input"));
                }
                let force = boolean(a, "force")?;
                if let Err(e) = g
                    .local(root, &["branch", if force { "-D" } else { "-d" }, branch])
                    .await
                {
                    if !force
                        && e.detail
                            .as_deref()
                            .is_some_and(|s| s.contains("not fully merged"))
                    {
                        let count = status::count(g, root, &format!("HEAD..refs/heads/{branch}"))
                            .await
                            .unwrap_or(0);
                        return Err(AppError::new("branch_not_merged")
                            .detail(&format!("Unmerged commits: {count}")));
                    }
                    return Err(e);
                }
            }
            "delete_remote_branch" => {
                let remote = text(a, "remote")?;
                let branch = text(a, "branch")?;
                g.reference(root, remote).await?;
                g.reference(root, branch).await?;
                let result = g
                    .run(
                        root,
                        &[
                            "push".into(),
                            remote.into(),
                            "--delete".into(),
                            branch.into(),
                        ],
                        true,
                        None,
                    )
                    .await;
                self.network_result(id, &result);
                result?;
            }
            "set_upstream" => {
                let branch = text(a, "branch")?;
                g.reference(root, branch).await?;
                if a["upstream"].is_null() {
                    g.local(root, &["branch", "--unset-upstream", branch])
                        .await?;
                } else {
                    let remote = text(&a["upstream"], "remote")?;
                    let name = text(&a["upstream"], "branch")?;
                    g.reference(root, remote).await?;
                    g.reference(root, name).await?;
                    g.local(
                        root,
                        &[
                            "branch",
                            &format!("--set-upstream-to={remote}/{name}"),
                            branch,
                        ],
                    )
                    .await?;
                }
            }
            "add_remote" | "set_remote_url" | "rename_remote" | "remove_remote" => {
                let name = text(
                    a,
                    if action == "rename_remote" {
                        "from"
                    } else {
                        "name"
                    },
                )?;
                g.reference(root, name).await?;
                let exists = p["status"]["remotes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["name"] == name);
                if action == "add_remote" && exists {
                    return Err(AppError::new("remote_exists"));
                }
                if action != "add_remote" && !exists {
                    return Err(AppError::new("remote_not_found"));
                }
                match action {
                    "add_remote" | "set_remote_url" => {
                        let url = text(a, "url")?;
                        git::validate_url(url)?;
                        g.local(
                            root,
                            &[
                                "remote",
                                if action == "add_remote" {
                                    "add"
                                } else {
                                    "set-url"
                                },
                                name,
                                url,
                            ],
                        )
                        .await?;
                    }
                    "rename_remote" => {
                        let to = text(a, "to")?;
                        g.reference(root, to).await?;
                        if p["status"]["remotes"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|r| r["name"] == to)
                        {
                            return Err(AppError::new("remote_exists"));
                        }
                        g.local(root, &["remote", "rename", name, to]).await?;
                        self.migrate_settings(id, action, name, to).await?;
                    }
                    _ => {
                        g.local(root, &["remote", "remove", name]).await?;
                    }
                }
            }
            "mark_resolved" => {
                let paths = a["paths"]
                    .as_array()
                    .ok_or_else(|| AppError::new("invalid_input"))?;
                if paths.is_empty() {
                    return Err(AppError::new("invalid_input"));
                }
                let mut args = vec!["add".into(), "--".into()];
                for p in paths {
                    let path = p.as_str().ok_or_else(|| AppError::new("invalid_path"))?;
                    git::git_path(root, path)?;
                    args.push(path.into());
                }
                g.write(root, &args).await?;
            }
            "continue_in_progress" | "abort_in_progress" => {
                let kind = p["status"]["inProgress"]
                    .as_str()
                    .ok_or_else(|| AppError::new("invalid_input"))?;
                if action == "continue_in_progress"
                    && !self.conflicts(id).await?["files"]
                        .as_array()
                        .unwrap()
                        .is_empty()
                {
                    return Err(AppError::new("merge_conflict"));
                }
                let sub = match kind {
                    "cherry_pick" => "cherry-pick",
                    "merge" => "merge",
                    "rebase" => "rebase",
                    "revert" => "revert",
                    _ => return Err(AppError::new("invalid_input")),
                };
                let args = if action == "abort_in_progress" {
                    vec![sub, "--abort"]
                } else if kind == "merge" {
                    vec!["-c", "core.editor=true", "commit", "--no-edit"]
                } else {
                    vec!["-c", "core.editor=true", sub, "--continue"]
                };
                let result = g.local_write(root, &args).await;
                self.conflict_result(id, result).await?;
            }
            "rebase" | "reset" => {
                let preview = if action == "rebase" {
                    self.preview_rebase(id, text(a, "onto")?).await?
                } else {
                    self.preview_reset(id, text(a, "target")?, text(a, "mode")?)
                        .await?
                };
                if let Some(blocker) = preview["blocker"].as_str() {
                    return Err(AppError::new(match blocker {
                        "in_progress" => "operation_in_progress",
                        _ => blocker,
                    }));
                }
                if text(a, "fingerprint")? != preview["fingerprint"].as_str().unwrap() {
                    return Err(AppError::new("state_changed"));
                }
                let args = if action == "rebase" {
                    vec![
                        "rebase".into(),
                        preview["onto"]["sha"].as_str().unwrap().into(),
                    ]
                } else {
                    vec![
                        "reset".into(),
                        format!("--{}", text(a, "mode")?),
                        preview["target"]["sha"].as_str().unwrap().into(),
                    ]
                };
                if action == "reset" && a["mode"] == "hard" && !self.hard_reset_allowed() {
                    return Err(AppError::new("feature_disabled"));
                }
                let result = g.write(root, &args).await;
                self.conflict_result(id, result).await?;
            }
            "cherry_pick" => {
                if let Some(b) = Self::local_blocker(p, true) {
                    return Err(AppError::new(b));
                }
                let shas = a["shas"]
                    .as_array()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| AppError::new("invalid_input"))?;
                let mut args = vec!["cherry-pick".into()];
                for sha in shas {
                    args.push(
                        resolve(
                            g,
                            root,
                            sha.as_str().ok_or_else(|| AppError::new("invalid_input"))?,
                        )
                        .await?,
                    );
                }
                let result = g.write(root, &args).await;
                self.conflict_result(id, result).await?;
            }
            "discard_changes" => {
                let include = boolean(a, "includeUntracked")?;
                let paths = if a["paths"].is_null() {
                    None
                } else {
                    Some(
                        a["paths"]
                            .as_array()
                            .ok_or_else(|| AppError::new("invalid_input"))?,
                    )
                };
                if let Some(paths) = paths {
                    for path in paths {
                        git::git_path(
                            root,
                            path.as_str().ok_or_else(|| AppError::new("invalid_path"))?,
                        )?;
                    }
                }
                let selected = |path: &str| {
                    paths.is_none_or(|paths| {
                        paths
                            .iter()
                            .filter_map(Value::as_str)
                            .any(|s| path == s || Path::new(path).starts_with(s))
                    })
                };
                let tracked = g.local(root, &["ls-files", "-z", "--cached"]).await?;
                let head_files = g
                    .local(root, &["ls-tree", "-r", "--name-only", "-z", "HEAD"])
                    .await?;
                let head_set = head_files
                    .split('\0')
                    .filter(|s| !s.is_empty())
                    .collect::<std::collections::BTreeSet<_>>();
                let mut restore = vec![
                    "restore".into(),
                    "--source=HEAD".into(),
                    "--staged".into(),
                    "--worktree".into(),
                    "--".into(),
                ];
                for path in head_set.iter().copied().filter(|s| selected(s)) {
                    git::git_path(root, path)?;
                    if paths.is_some() {
                        restore.push(path.into());
                    }
                }
                if paths.is_none() && !head_set.is_empty() {
                    restore.push(".".into());
                }
                for path in tracked
                    .split('\0')
                    .filter(|s| !s.is_empty() && selected(s) && !head_set.contains(s))
                {
                    let full = git::git_path(root, path)?;
                    g.local_write(root, &["rm", "--cached", "-f", "--", path])
                        .await?;
                    if std::fs::symlink_metadata(&full).is_ok() {
                        std::fs::remove_file(full)?;
                    }
                }
                if restore.len() > 5 {
                    g.write(root, &restore).await?;
                }
                if include {
                    let untracked = g
                        .local(root, &["ls-files", "--others", "--exclude-standard", "-z"])
                        .await?;
                    for path in untracked
                        .split('\0')
                        .filter(|s| !s.is_empty() && selected(s))
                    {
                        let full = git::git_path(root, path)?;
                        std::fs::remove_file(full)?;
                    }
                }
            }
            "stash_save" => {
                let mut args = vec!["stash".into(), "push".into()];
                if boolean(a, "includeUntracked")? {
                    args.push("--include-untracked".into());
                }
                if let Some(message) = a["message"].as_str() {
                    args.extend(["-m".into(), message.into()]);
                }
                g.write(root, &args).await?;
            }
            "stash_apply" | "stash_drop" => {
                let drop = if action == "stash_apply" {
                    boolean(a, "drop")?
                } else {
                    false
                };
                let index = self.stash_index(root, text(a, "stashSha")?).await?;
                if action == "stash_drop" {
                    g.local_write(root, &["stash", "drop", &index]).await?;
                } else {
                    let result = g.local_write(root, &["stash", "apply", &index]).await;
                    self.conflict_result(id, result).await?;
                    if drop {
                        g.local_write(root, &["stash", "drop", &index]).await?;
                    }
                }
            }
            _ => return Err(AppError::new("invalid_input")),
        }
        Ok(Value::Null)
    }
    async fn conflict_result<T>(&self, id: &str, result: Result<T>) -> Result<T> {
        if result
            .as_ref()
            .is_err_and(|e| !matches!(e.code.as_str(), "repo_locked" | "timeout"))
            && !self.conflicts(id).await?["files"]
                .as_array()
                .unwrap()
                .is_empty()
        {
            return Err(AppError::new("merge_conflict"));
        }
        result
    }
}
