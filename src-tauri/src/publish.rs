use crate::{
    core::Core,
    error::{AppError, Result},
    git, registry, settings,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| AppError::new("invalid_input"))
}
pub(crate) fn clone_destination(input: &Value) -> Result<PathBuf> {
    git::validate_url(text(input, "url")?)?;
    let name = text(input, "folderName")?;
    if name.is_empty()
        || name.contains(['/', '\\'])
        || name.contains("..")
        || name == "."
        || name.chars().any(char::is_control)
    {
        return Err(AppError::new("invalid_input"));
    }
    Ok(registry::directory(text(input, "parentDir")?)?.join(name))
}
fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}
fn check_publish(input: &Value) -> bool {
    let valid = |name: &str| {
        !name.is_empty()
            && name != "."
            && name != ".."
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    };
    input["name"].as_str().is_some_and(valid)
        && input["owner"].as_str().is_none_or(|o| {
            !o.is_empty() && o.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        && matches!(input["visibility"].as_str(), Some("private" | "public"))
        && input["initialCommitMessage"]
            .as_str()
            .is_none_or(|s| !s.trim().is_empty())
}
pub(crate) fn sensitive_name(path: &str) -> bool {
    let base = Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    base == ".env"
        || (base.starts_with(".env.") && base != ".env.example")
        || [".pem", ".key", ".p12"].iter().any(|s| base.ends_with(s))
        || base.starts_with("id_rsa")
        || base.starts_with("id_ed25519")
        || [".npmrc", ".netrc", "credentials.json"].contains(&base.as_str())
}
impl Core {
    pub(crate) fn clone_plan(&self, input: &Value) -> Result<Value> {
        let dest = clone_destination(input)?;
        let blocker = if exists(&dest) {
            Some("destination_exists")
        } else {
            None
        };
        Ok(
            json!({"itemId":input["itemId"],"projectId":null,"executable":blocker.is_none(),"blocker":blocker,"steps":[{"code":"clone","url":crate::error::sanitize(text(input,"url")?),"destination":dest}],"risk":"safe","warnings":[],"fingerprint":null}),
        )
    }
    async fn publish_files(
        &self,
        root: &Path,
        initialized: bool,
        unborn: bool,
        patterns: &[String],
    ) -> Result<Vec<String>> {
        let g = &self.options.git;
        let mut exclude = vec![
            "ls-files".to_owned(),
            "--others".into(),
            "--exclude-standard".into(),
            "-z".into(),
        ];
        let pattern_scratch = if patterns.is_empty() {
            None
        } else {
            let scratch = crate::ignore::Scratch::outside(root)?;
            let file = scratch.0.join("patterns");
            std::fs::write(&file, format!("{}\n", patterns.join("\n")))?;
            exclude.push(format!("--exclude-from={}", file.display()));
            Some(scratch)
        };
        let exclude_refs = exclude.iter().map(String::as_str).collect::<Vec<_>>();
        let output = if initialized {
            let mut out = g.local(root, &["ls-files", "-z"]).await?;
            if unborn {
                out.push_str(&g.local(root, &exclude_refs).await?);
            }
            out
        } else {
            let scratch = crate::ignore::Scratch::outside(root)?;
            g.local(&scratch.0, &["init", "--bare"]).await?;
            let mut args = vec![
                format!("--git-dir={}", scratch.0.display()),
                format!("--work-tree={}", root.display()),
            ];
            args.extend(exclude);
            g.local(root, &args.iter().map(String::as_str).collect::<Vec<_>>())
                .await?
        };
        drop(pattern_scratch);
        let mut files = output
            .split('\0')
            .filter(|s| !s.is_empty())
            .take(20000)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        files.sort();
        files.dedup();
        Ok(files)
    }
    pub(crate) async fn publish_plan(&self, input: &Value, p: &Value) -> Result<Value> {
        let root = Path::new(text(p, "path")?);
        let g = &self.options.git;
        let initialized = root.join(".git").exists();
        let unborn = !initialized
            || g.local(root, &["rev-parse", "--verify", "HEAD"])
                .await
                .is_err();
        let auth = self
            .github
            .lock()
            .unwrap()
            .status
            .as_ref()
            .map(|(_, s)| s.clone());
        let authenticated = auth.as_ref().is_some_and(|s| s["authenticated"] == true);
        let mut blocker = if !root.is_dir() {
            Some("missing")
        } else if !check_publish(input) {
            Some("invalid_input")
        } else if !authenticated {
            Some("not_authenticated")
        } else {
            None
        };
        if initialized
            && g.local(root, &["remote", "get-url", "origin"])
                .await
                .is_ok()
        {
            blocker = Some("remote_exists");
        }
        if initialized && p["status"]["detached"] == true {
            blocker = Some("detached");
        }
        if initialized && !p["status"]["inProgress"].is_null() {
            blocker = Some("in_progress");
        }
        let patterns = if unborn && input["initialCommitMessage"].is_string() {
            crate::ignore::patterns(&input["ignore"])?
        } else {
            vec![]
        };
        let (ignore_bytes, new_patterns) = crate::ignore::additions(root, &patterns)?;
        let mut files = if root.is_dir() {
            self.publish_files(root, initialized, unborn, &patterns)
                .await?
        } else {
            vec![]
        };
        if !new_patterns.is_empty() && !files.iter().any(|p| p == ".gitignore") {
            let scratch = crate::ignore::Scratch::outside(root)?;
            g.local(&scratch.0, &["init"]).await?;
            let mut content = ignore_bytes.clone();
            if !content.is_empty() && !content.ends_with(b"\n") {
                content.push(b'\n');
            }
            content.extend_from_slice(
                format!("# Added by Revia\n{}\n", new_patterns.join("\n")).as_bytes(),
            );
            std::fs::write(scratch.0.join(".gitignore"), content)?;
            let included = g
                .local(
                    &scratch.0,
                    &[
                        "ls-files",
                        "--others",
                        "--exclude-standard",
                        "-z",
                        "--",
                        ".gitignore",
                    ],
                )
                .await?;
            if included.split('\0').any(|p| p == ".gitignore") {
                files.push(".gitignore".into());
                files.sort();
            }
        }
        let mut warnings = vec![];
        let mut sensitive = vec![];
        let mut large = vec![];
        let mut stamps = Vec::new();
        for path in &files {
            if sensitive_name(path) {
                sensitive.push(path.clone());
            }
            if let Ok(full) = git::relative(root, path)
                && let Ok(meta) = std::fs::symlink_metadata(full)
            {
                if meta.is_file() && meta.len() > 50 * 1024 * 1024 {
                    large.push(path.clone());
                }
                stamps.push(json!([
                    path,
                    meta.len(),
                    meta.modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_nanos().to_string())
                ]));
            }
        }
        let danger = !sensitive.is_empty();
        if danger {
            warnings.push(json!({"code":"sensitive_files","paths":sensitive}));
        }
        if patterns.is_empty()
            && !std::fs::symlink_metadata(root.join(".gitignore")).is_ok_and(|m| m.is_file())
        {
            warnings.push(json!({"code":"no_gitignore"}));
        }
        if !large.is_empty() {
            warnings.push(json!({"code":"large_files","paths":large}));
        }
        if files.len() > 5000 {
            warnings.push(json!({"code":"many_files","files":files.len()}));
        }
        if input["visibility"] == "public" {
            warnings.push(json!({"code":"public_repo"}));
        }
        let owner = input["owner"]
            .as_str()
            .or_else(|| auth.as_ref().and_then(|s| s["account"]["login"].as_str()))
            .unwrap_or("");
        let full = format!("{}/{}", owner, input["name"].as_str().unwrap_or(""));
        let ssh = auth.as_ref().is_some_and(|s| s["gitProtocol"] == "ssh");
        let url = if ssh {
            format!("git@github.com:{full}.git")
        } else {
            format!("https://github.com/{full}.git")
        };
        let mut steps = vec![];
        if !initialized {
            steps.push(json!({"code":"init"}));
        }
        if !new_patterns.is_empty() {
            steps.push(json!({"code":"ignore","patterns":new_patterns.len()}));
        }
        if unborn && input["initialCommitMessage"].is_string() {
            steps.push(json!({"code":"initial_commit","files":files.len()}));
        }
        steps.push(
            json!({"code":"create_repo","nameWithOwner":full,"visibility":input["visibility"]}),
        );
        steps.push(json!({"code":"add_remote","name":"origin","url":url}));
        if !unborn || input["initialCommitMessage"].is_string() {
            steps.push(json!({"code":"push","to":{"remote":"origin","branch":p["status"]["branch"].as_str().unwrap_or("main")},"commits":null,"createsRemoteBranch":true,"force":false,"setUpstream":true}));
        }
        let head = g.local(root, &["rev-parse", "--verify", "HEAD"]).await.ok();
        let porcelain = if initialized {
            g.local(root, &["status", "--porcelain=v1", "-z"])
                .await
                .ok()
        } else {
            None
        };
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&json!([
                p["id"],
                head,
                porcelain,
                stamps,
                steps,
                Sha256::digest(&ignore_bytes).to_vec()
            ]))?)
        );
        Ok(
            json!({"itemId":input["itemId"],"projectId":input["projectId"],"executable":blocker.is_none(),"blocker":blocker,"steps":steps,"risk":if danger {"danger"} else {"caution"},"warnings":warnings,"fingerprint":fingerprint}),
        )
    }
    pub(crate) async fn execute_publish(
        self: &Arc<Self>,
        input: &Value,
        item: &mut Value,
        cancel: Arc<AtomicBool>,
        op: &str,
        index: usize,
    ) -> Result<()> {
        let id = text(input, "projectId")?;
        self.refresh(Some(vec![id.into()])).await?;
        let p = self.project(id)?;
        self.github_status(true).await?;
        let plan = self.publish_plan(input, &p).await?;
        if let Some(b) = plan["blocker"].as_str() {
            item["state"] = json!("needs_confirmation");
            item["reason"] = json!(b);
            return Ok(());
        }
        if input["fingerprint"] != plan["fingerprint"] {
            item["state"] = json!("needs_confirmation");
            item["reason"] = json!("state_changed");
            return Ok(());
        }
        let root = Path::new(text(&p, "path")?);
        let g = &self.options.git;
        for planned in plan["steps"].as_array().unwrap() {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::new("cancelled"));
            }
            let code = text(planned, "code")?;
            self.step_event(op, index, item, code);
            let mut done = planned.clone();
            match code {
                "init" => {
                    g.local(root, &["init", "-b", "main"]).await?;
                }
                "ignore" => {
                    let patterns = crate::ignore::patterns(&input["ignore"])?;
                    crate::ignore::append(root, &patterns)?;
                }
                "initial_commit" => {
                    for key in ["user.name", "user.email"] {
                        if g.local(root, &["config", "--get", key])
                            .await
                            .ok()
                            .is_none_or(|s| s.trim().is_empty())
                        {
                            return Err(AppError::new("identity_missing"));
                        }
                    }
                    g.local_write(root, &["add", "-A", "--", "."]).await?;
                    g.local_write(
                        root,
                        &["commit", "-m", text(input, "initialCommitMessage")?],
                    )
                    .await?;
                    done["files"] = json!(
                        g.local(root, &["ls-files", "-z"])
                            .await?
                            .split('\0')
                            .filter(|s| !s.is_empty())
                            .count()
                    );
                }
                "create_repo" => {
                    let mut args = vec![
                        "repo",
                        "create",
                        text(planned, "nameWithOwner")?,
                        if input["visibility"] == "public" {
                            "--public"
                        } else {
                            "--private"
                        },
                    ];
                    if let Some(d) = input["description"].as_str() {
                        args.extend(["--description", d]);
                    }
                    self.gh(&args).await?;
                    self.github.lock().unwrap().repos = None;
                }
                "add_remote" => {
                    git::validate_url(text(planned, "url")?)?;
                    g.local(root, &["remote", "add", "origin", text(planned, "url")?])
                        .await?;
                }
                "push" => {
                    let branch = g.local(root, &["symbolic-ref", "--short", "HEAD"]).await?;
                    g.reference(root, branch.trim()).await?;
                    let core = self.clone();
                    let operation = op.to_string();
                    let progress = Arc::new(move |v| {
                        let mut current =
                            core.operation(&operation).unwrap()["items"][index].clone();
                        current["progress"] = json!(v);
                        core.item_event(&operation, index, current);
                    });
                    let result = g
                        .run_progress(
                            root,
                            &[
                                "push".into(),
                                "--progress".into(),
                                "-u".into(),
                                "origin".into(),
                                branch.trim().into(),
                            ],
                            true,
                            Some(cancel.clone()),
                            Some(progress),
                        )
                        .await;
                    self.network_result(id, &result);
                    result?;
                    item["progress"] =
                        self.operation(op).unwrap()["items"][index]["progress"].clone();
                    done["to"]["branch"] = json!(branch.trim());
                    done["commits"] = json!(crate::status::count(g, root, "HEAD").await);
                }
                _ => return Err(AppError::new("invalid_input")),
            }
            item["completed"].as_array_mut().unwrap().push(done);
            self.item_event(op, index, item.clone());
        }
        item["state"] = json!("succeeded");
        Ok(())
    }
    pub(crate) async fn execute_clone(
        self: &Arc<Self>,
        input: &Value,
        item: &mut Value,
        cancel: Arc<AtomicBool>,
        op: &str,
        index: usize,
    ) -> Result<()> {
        let dest = clone_destination(input)?;
        if input["fingerprint"].is_string() {
            item["state"] = json!("needs_confirmation");
            item["reason"] = json!("state_changed");
            return Ok(());
        }
        if exists(&dest) {
            item["state"] = json!("needs_confirmation");
            item["reason"] = json!("destination_exists");
            return Ok(());
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::new("cancelled"));
        }
        std::fs::create_dir(&dest).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                AppError::new("destination_exists")
            } else {
                e.into()
            }
        })?;
        let reserved = std::fs::symlink_metadata(&dest)?;
        self.step_event(op, index, item, "clone");
        let core = self.clone();
        let operation = op.to_string();
        let progress = Arc::new(move |v| {
            let mut current = core.operation(&operation).unwrap()["items"][index].clone();
            current["progress"] = json!(v);
            core.item_event(&operation, index, current);
        });
        let result = self
            .options
            .git
            .run_progress(
                dest.parent().unwrap(),
                &[
                    "clone".into(),
                    "--progress".into(),
                    text(input, "url")?.into(),
                    dest.to_string_lossy().into_owned(),
                ],
                true,
                Some(cancel.clone()),
                Some(progress),
            )
            .await;
        if result.as_ref().is_err_and(|e| e.code == "cancelled") || cancel.load(Ordering::SeqCst) {
            if std::fs::symlink_metadata(&dest).is_ok_and(|m| {
                m.is_dir() && !m.file_type().is_symlink() && same_directory(&reserved, &m)
            }) {
                std::fs::remove_dir_all(&dest)?;
            }
            return Err(AppError::new("cancelled"));
        }
        result?;
        if crate::platform::canonicalize(&dest)? != dest
            || std::fs::symlink_metadata(dest.join(".git"))
                .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(AppError::new("invalid_path"));
        }
        item["completed"].as_array_mut().unwrap().push(json!({"code":"clone","url":crate::error::sanitize(text(input,"url")?),"destination":dest}));
        let new_id = registry::id(&dest.to_string_lossy());
        {
            let _guard = self.settings_lock.lock().await;
            let mut state = self.state.lock().unwrap();
            let mut next = state.settings.clone();
            let root = next.settings["roots"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| {
                    r["enabled"] == true
                        && Path::new(r["path"].as_str().unwrap()).is_dir()
                        && dest.starts_with(r["path"].as_str().unwrap())
                })
                .cloned();
            let origin = if let Some(r) = root {
                json!({"kind":"scan","rootId":r["id"]})
            } else {
                next.manual_paths.push(dest.to_string_lossy().into_owned());
                json!({"kind":"manual"})
            };
            next.settings["excludedPaths"]
                .as_array_mut()
                .unwrap()
                .retain(|v| !dest.starts_with(v.as_str().unwrap_or("")));
            next.project_settings
                .entry(dest.to_string_lossy().into_owned())
                .or_insert_with(settings::project_defaults);
            settings::save(&self.options.config_dir, &next)?;
            let project = registry::project(&dest, origin, &next);
            state.settings = next;
            if !state.projects.iter().any(|p| p["id"] == new_id) {
                state.projects.push(project);
            }
        }
        item["projectId"] = json!(new_id);
        self.refresh(Some(vec![new_id])).await?;
        self.options.events.emit(
            "revia:projects-changed",
            self.workspace()["projects"].clone(),
        );
        item["state"] = json!("succeeded");
        Ok(())
    }
}

#[cfg(unix)]
fn same_directory(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}
#[cfg(not(unix))]
fn same_directory(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    a.created()
        .ok()
        .zip(b.created().ok())
        .is_some_and(|(a, b)| a == b)
}
