use crate::error::{AppError, Result, sanitize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command};
#[derive(Clone)]
pub struct Git {
    pub executable: PathBuf,
    pub gh: PathBuf,
    pub env: BTreeMap<String, String>,
}
impl Default for Git {
    fn default() -> Self {
        Self {
            executable: find_tool("git"),
            gh: find_tool("gh"),
            env: BTreeMap::new(),
        }
    }
}
pub fn tool_candidates(name: &str, extensions: Option<&str>) -> Vec<String> {
    let mut names = vec![name.to_owned()];
    if let Some(extensions) = extensions
        && Path::new(name).extension().is_none()
    {
        names.extend(
            extensions
                .split(';')
                .filter(|e| !e.is_empty())
                .map(|e| format!("{name}{e}")),
        );
    }
    names
}
pub fn find_tool(name: &str) -> PathBuf {
    let mut roots =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    #[cfg(target_os = "macos")]
    roots.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    #[cfg(target_os = "linux")]
    {
        roots.extend(
            [
                "/usr/bin",
                "/usr/local/bin",
                "/snap/bin",
                "/home/linuxbrew/.linuxbrew/bin",
            ]
            .map(PathBuf::from),
        );
        if let Some(home) = crate::platform::home() {
            roots.push(home.join(".local/bin"));
        }
    }
    #[cfg(windows)]
    {
        if let Some(root) = std::env::var_os("SystemRoot") {
            roots.push(PathBuf::from(root).join("System32"));
        }
        for key in [
            "ProgramFiles",
            "ProgramFiles(x86)",
            "LOCALAPPDATA",
            "USERPROFILE",
        ] {
            if let Some(base) = std::env::var_os(key).map(PathBuf::from) {
                let suffixes: &[&str] = match key {
                    "LOCALAPPDATA" => &[
                        "Programs/Git/cmd",
                        "Microsoft/WinGet/Links",
                        "Microsoft/WindowsApps",
                        "Programs/Microsoft VS Code/bin",
                        "Programs/cursor/resources/app/bin",
                        "Programs/Zed",
                        "Programs/Sublime Text",
                    ],
                    "USERPROFILE" => &["scoop/shims"],
                    _ => &[
                        "Git/cmd",
                        "GitHub CLI",
                        "Microsoft VS Code/bin",
                        "Cursor/resources/app/bin",
                        "Zed",
                        "Sublime Text",
                    ],
                };
                roots.extend(suffixes.iter().map(|s| base.join(s)));
            }
        }
    }
    #[cfg(windows)]
    let extensions =
        Some(std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into()));
    #[cfg(not(windows))]
    let extensions: Option<String> = None;
    roots
        .into_iter()
        .flat_map(|p| {
            tool_candidates(name, extensions.as_deref())
                .into_iter()
                .map(move |n| p.join(n))
        })
        .find(|p| executable(p))
        .map(|p| {
            crate::platform::canonicalize(&p).unwrap_or_else(|_| crate::platform::normalize(&p))
        })
        .unwrap_or_else(|| name.into())
}
fn credential_helper(path: &Path) -> String {
    let path = crate::platform::normalize(path)
        .to_string_lossy()
        .into_owned();
    #[cfg(windows)]
    let path = path.replace('\\', "/");
    format!("!'{}' auth git-credential", path.replace('\'', "'\\''"))
}
pub(crate) fn executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}
impl Git {
    pub async fn run(
        &self,
        path: &Path,
        args: &[String],
        network: bool,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<Vec<u8>> {
        self.run_progress(path, args, network, cancel, None).await
    }
    pub async fn run_progress(
        &self,
        path: &Path,
        args: &[String],
        network: bool,
        cancel: Option<Arc<AtomicBool>>,
        progress: Option<Arc<dyn Fn(f64) + Send + Sync>>,
    ) -> Result<Vec<u8>> {
        self.run_mode(
            path,
            args,
            network,
            false,
            cancel,
            progress,
            limits(network, false),
        )
        .await
    }
    pub async fn write(&self, path: &Path, args: &[String]) -> Result<Vec<u8>> {
        self.run_mode(path, args, false, true, None, None, limits(false, true))
            .await
    }
    pub async fn local_write(&self, path: &Path, args: &[&str]) -> Result<String> {
        let args = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        Ok(String::from_utf8_lossy(&self.write(path, &args).await?).into_owned())
    }
    #[allow(clippy::too_many_arguments)]
    async fn run_mode(
        &self,
        path: &Path,
        args: &[String],
        network: bool,
        write: bool,
        cancel: Option<Arc<AtomicBool>>,
        progress: Option<Arc<dyn Fn(f64) + Send + Sync>>,
        timeout: Duration,
    ) -> Result<Vec<u8>> {
        let mut cmd = self.command(path, args);
        let cloning = args.first().is_some_and(|a| a == "clone");
        if network {
            let all_urls = if cloning {
                format!(
                    "origin\t{} (fetch)",
                    args.get(2).map(String::as_str).unwrap_or("")
                )
            } else {
                self.local(path, &["remote", "-v"]).await?
            };
            let selected = args
                .iter()
                .position(|a| a == "fetch" || a == "push")
                .and_then(|i| args[i + 1..].iter().find(|a| !a.starts_with('-')));
            let urls = all_urls
                .lines()
                .filter(|line| {
                    selected
                        .is_none_or(|name| line.split_whitespace().next() == Some(name.as_str()))
                })
                .collect::<Vec<_>>()
                .join("\n");
            for line in urls.lines() {
                if let Some(url) = line.split_whitespace().nth(1) {
                    validate_url(url)?;
                }
            }
            if self.gh.is_absolute()
                && self.gh.is_file()
                && urls.lines().any(|l| {
                    l.split_whitespace().nth(1).is_some_and(|u| {
                        crate::error::strip_userinfo(u).starts_with("https://github.com/")
                    })
                })
            {
                let helper = credential_helper(&self.gh);
                let mut all = vec![
                    "-c".into(),
                    "credential.https://github.com.helper=".into(),
                    "-c".into(),
                    format!("credential.https://github.com.helper={helper}"),
                ];
                all.extend_from_slice(args);
                cmd = self.command(path, &all);
            }
        }
        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::new("git_not_installed")
            } else {
                e.into()
            }
        })?;
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let out = tokio::spawn(async move {
            let mut b = Vec::new();
            stdout.read_to_end(&mut b).await.map(|_| b)
        });
        let err = tokio::spawn(async move {
            let mut b = Vec::new();
            let mut chunk = [0; 4096];
            let mut pending = Vec::new();
            let mut last = tokio::time::Instant::now() - Duration::from_millis(150);
            loop {
                let n = stderr.read(&mut chunk).await?;
                if n == 0 {
                    break;
                }
                b.extend_from_slice(&chunk[..n]);
                for byte in &chunk[..n] {
                    if *byte == b'\r' || *byte == b'\n' {
                        let line = String::from_utf8_lossy(&pending);
                        if (!cloning || line.trim().starts_with("Receiving objects:"))
                            && last.elapsed() >= Duration::from_millis(150)
                            && let Some(value) = parse_progress(&line)
                        {
                            if let Some(cb) = &progress {
                                cb(value);
                            }
                            last = tokio::time::Instant::now();
                        }
                        pending.clear();
                    } else if pending.len() < 8192 {
                        pending.push(*byte);
                    }
                }
            }
            Ok::<_, std::io::Error>(b)
        });
        let deadline = tokio::time::Instant::now() + timeout;
        let status = loop {
            if network && cancel.as_ref().is_some_and(|c| c.load(Ordering::SeqCst)) {
                let _ = child.kill().await;
                out.abort();
                err.abort();
                return Err(AppError::new("cancelled"));
            }
            if !cloning && tokio::time::Instant::now() >= deadline {
                if write {
                    #[cfg(unix)]
                    if let Some(pid) = child.id() {
                        let _ = crate::platform::command("/bin/kill")
                            .args(["-TERM", &pid.to_string()])
                            .status()
                            .await;
                    }
                    #[cfg(windows)]
                    let _ = child.start_kill();
                    if tokio::time::timeout(Duration::from_secs(5), child.wait())
                        .await
                        .is_err()
                    {
                        let _ = child.kill().await;
                    }
                } else {
                    let _ = child.kill().await;
                }
                out.abort();
                err.abort();
                return Err(AppError::new("timeout"));
            }
            if let Some(status) = child.try_wait()? {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        let output = out.await.map_err(|_| AppError::new("io_error"))??;
        let errors = err.await.map_err(|_| AppError::new("io_error"))??;
        if status.success() {
            Ok(output)
        } else {
            Err(classify(&String::from_utf8_lossy(&errors)))
        }
    }
    fn command(&self, path: &Path, args: &[String]) -> Command {
        let mut c = crate::platform::command(&self.executable);
        for key in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_COMMON_DIR",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        ] {
            c.env_remove(key);
        }
        let ssh = self
            .env
            .get("GIT_SSH_COMMAND")
            .cloned()
            .or_else(|| std::env::var("GIT_SSH_COMMAND").ok())
            .unwrap_or_else(|| "ssh".into());
        c.current_dir(path)
            .args([
                "-c",
                "core.quotepath=false",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "protocol.ext.allow=never",
            ])
            .args(args)
            .envs(&self.env)
            .env("GH_PROMPT_DISABLED", "1")
            .env("NO_COLOR", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .env("CLICOLOR", "0")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_SSH_COMMAND", format!("{ssh} -o BatchMode=yes"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if args.iter().any(|a| a == "--") {
            c.env("GIT_LITERAL_PATHSPECS", "1");
        } else {
            c.env_remove("GIT_LITERAL_PATHSPECS");
        }
        c
    }
    pub async fn local(&self, path: &Path, args: &[&str]) -> Result<String> {
        let args = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let output =
            tokio::time::timeout(Duration::from_secs(60), self.command(path, &args).output())
                .await
                .map_err(|_| AppError::new("timeout"))?
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        AppError::new("git_not_installed")
                    } else {
                        e.into()
                    }
                })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into())
        } else {
            Err(classify(&String::from_utf8_lossy(&output.stderr)))
        }
    }
    pub(crate) async fn scan_entries(
        &self,
        path: &Path,
        args: &[String],
        limit: usize,
    ) -> Result<(String, bool)> {
        let mut child = self.command(path, args).spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::new("git_not_installed")
            } else {
                e.into()
            }
        })?;
        let mut stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let errors = tokio::spawn(async move {
            let mut bytes = vec![];
            stderr
                .take(64 * 1024)
                .read_to_end(&mut bytes)
                .await
                .map(|_| bytes)
        });
        let mut bytes = Vec::new();
        let mut count = 0;
        let mut truncated = false;
        let mut buffer = [0; 4096];
        'read: loop {
            let n = stdout.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            for b in &buffer[..n] {
                bytes.push(*b);
                if *b == 0 {
                    count += 1;
                }
                if count >= limit || bytes.len() >= 8 * 1024 * 1024 {
                    truncated = true;
                    break 'read;
                }
            }
        }
        if truncated {
            let _ = child.kill().await;
        }
        let status = child.wait().await?;
        let error_bytes = errors.await.map_err(|_| AppError::new("io_error"))??;
        if !truncated && !status.success() {
            return Err(classify(&String::from_utf8_lossy(&error_bytes)));
        }
        if !bytes.ends_with(&[0]) {
            bytes.truncate(bytes.iter().rposition(|b| *b == 0).map_or(0, |i| i + 1));
        }
        Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
    }
    pub async fn symlink_diff(&self, path: &Path, name: &str) -> Result<String> {
        let args = [
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-index",
            "--",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
            name,
        ]
        .map(str::to_owned);
        let output =
            tokio::time::timeout(Duration::from_secs(60), self.command(path, &args).output())
                .await
                .map_err(|_| AppError::new("timeout"))??;
        if output.status.success() || output.status.code() == Some(1) {
            Ok(String::from_utf8_lossy(&output.stdout).into())
        } else {
            Err(classify(&String::from_utf8_lossy(&output.stderr)))
        }
    }
    pub async fn merge_conflicts(&self, path: &Path, left: &str, right: &str) -> serde_json::Value {
        let args = [
            "merge-tree",
            "--write-tree",
            "--name-only",
            "-z",
            left,
            right,
        ]
        .map(str::to_owned);
        let output =
            tokio::time::timeout(Duration::from_secs(60), self.command(path, &args).output()).await;
        match output {
            Ok(Ok(o)) if o.status.success() => serde_json::json!({"status":"none","paths":[]}),
            Ok(Ok(o)) if o.status.code() == Some(1) => {
                let text = String::from_utf8_lossy(&o.stdout);
                let mut parts = text.split('\0');
                parts.next();
                serde_json::json!({"status":"conflicts","paths":parts.take_while(|s|!s.is_empty()).collect::<Vec<_>>()})
            }
            _ => serde_json::json!({"status":"unknown","paths":[]}),
        }
    }
    pub async fn reference(&self, path: &Path, name: &str) -> Result<()> {
        if name.is_empty()
            || name.starts_with('-')
            || name.chars().any(char::is_control)
            || self
                .local(path, &["check-ref-format", &format!("refs/heads/{name}")])
                .await
                .is_err()
        {
            return Err(AppError::new("invalid_ref_name"));
        }
        Ok(())
    }
}
pub fn validate_url(url: &str) -> Result<()> {
    if url.is_empty()
        || url.starts_with('-')
        || url.contains("::")
        || url.chars().any(char::is_control)
    {
        return Err(AppError::new("invalid_input")
            .detail("URL contains a forbidden transport specification or character"));
    }
    if let Some((scheme, rest)) = url.split_once("://") {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        if let Some((userinfo, _)) = authority.rsplit_once('@')
            && (matches!(scheme, "https" | "http" | "git")
                || (scheme == "ssh" && userinfo.contains(':')))
        {
            return Err(
                AppError::new("invalid_input").detail("Remote URL credentials are not allowed")
            );
        }
    } else if !Path::new(url).is_absolute()
        && let Some((userinfo, host_path)) = url.rsplit_once('@')
        && host_path
            .split_once(':')
            .is_some_and(|(host, _)| !host.is_empty() && !host.contains('/'))
        && userinfo.contains(':')
    {
        return Err(AppError::new("invalid_input").detail("Remote URL credentials are not allowed"));
    }
    let scp = !url.contains("://")
        && url.split_once(':').is_some_and(|(host, path)| {
            !host.is_empty() && !host.contains(['/', ' ']) && !path.is_empty()
        });
    if ["https://", "http://", "ssh://", "git://", "file://"]
        .iter()
        .any(|s| url.starts_with(s))
        || Path::new(url).is_absolute()
        || scp
    {
        Ok(())
    } else {
        Err(AppError::new("invalid_input").detail("Unsupported remote URL"))
    }
}

pub fn relative(root: &Path, value: &str) -> Result<PathBuf> {
    relative_impl(root, value, false)
}
pub fn git_path(root: &Path, value: &str) -> Result<PathBuf> {
    relative_impl(root, value, true)
}
fn relative_impl(root: &Path, value: &str, allow_leaf: bool) -> Result<PathBuf> {
    let p = Path::new(value);
    if value.is_empty()
        || p.is_absolute()
        || p.components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(AppError::new("invalid_path"));
    }
    let mut next = root.to_path_buf();
    for part in p.components() {
        next.push(part);
        if !(allow_leaf && next == root.join(p))
            && std::fs::symlink_metadata(&next).is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(AppError::new("invalid_path"));
        }
    }
    Ok(next)
}
fn classify(s: &str) -> AppError {
    let l = s.to_lowercase();
    let code = if (l.contains(".lock") && l.contains("file exists"))
        || l.contains("another git process seems to be running")
    {
        "repo_locked"
    } else if l.contains("authentication failed") || l.contains("could not read username") {
        "auth_failed"
    } else if l.contains("permission denied") || http_status(&l, "403") {
        "permission_denied"
    } else if l.contains("repository not found")
        || l.contains("does not appear to be a git repository")
    {
        "remote_not_found"
    } else if l.contains("could not resolve")
        || l.contains("unable to access")
        || l.contains("connection refused")
        || l.contains("timed out")
        || l.contains("network is unreachable")
        || l.contains("failed to connect")
    {
        "network_unreachable"
    } else if l.contains("non-fast-forward")
        || l.contains("[rejected]")
        || l.contains("fetch first")
    {
        "push_rejected"
    } else if l.contains("no space left") {
        "disk_full"
    } else if l.contains("would be overwritten")
        || l.contains("unstaged changes")
        || l.contains("please commit")
    {
        "dirty_worktree"
    } else if s.contains("CONFLICT (") {
        "merge_conflict"
    } else if l.contains("unable to auto-detect email") || l.contains("author identity unknown") {
        "identity_missing"
    } else if l.contains("not possible to fast-forward") {
        "not_fast_forward"
    } else {
        "unknown"
    };
    AppError::new(code).detail(&sanitize(s))
}

pub fn parse_progress(line: &str) -> Option<f64> {
    let value = line
        .trim()
        .strip_prefix("Receiving objects:")
        .or_else(|| line.trim().strip_prefix("Writing objects:"))?
        .trim()
        .split('%')
        .next()?
        .parse::<u32>()
        .ok()?;
    (value <= 100).then_some(value as f64 / 100.0)
}

pub(crate) fn http_status(text: &str, code: &str) -> bool {
    [
        format!("http {code}"),
        format!("http/{code}"),
        format!("error: {code}"),
        format!("status code {code}"),
    ]
    .iter()
    .any(|pattern| {
        text.match_indices(pattern).any(|(i, p)| {
            text[i + p.len()..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_ascii_alphanumeric())
        })
    })
}

#[cfg(test)]
mod platform_tests {
    use super::*;
    #[test]
    fn pathext_candidates() {
        assert_eq!(
            tool_candidates("git", Some(".COM;.EXE;.BAT;.CMD")),
            ["git", "git.COM", "git.EXE", "git.BAT", "git.CMD"]
        );
        assert_eq!(
            tool_candidates("code", Some(".exe;;.cmd")),
            ["code", "code.exe", "code.cmd"]
        );
        assert_eq!(tool_candidates("code.cmd", Some(".EXE")), ["code.cmd"]);
        assert_eq!(tool_candidates("git", None), ["git"]);
    }
}

fn limits(network: bool, write: bool) -> Duration {
    Duration::from_secs(if network {
        180
    } else if write {
        600
    } else {
        60
    })
}
#[cfg(test)]
mod safety_tests {
    use super::*;
    #[test]
    fn diagnostics() {
        for (text, code) in [
            (
                "fatal: Unable to create index.lock: File exists",
                "repo_locked",
            ),
            ("Another git process seems to be running", "repo_locked"),
            (
                "CONFLICT (content): Merge conflict in file",
                "merge_conflict",
            ),
            (
                "conflict.txt would be overwritten by merge",
                "dirty_worktree",
            ),
            ("a path contains conflict", "unknown"),
        ] {
            assert_eq!(classify(text).code, code);
        }
    }
    #[test]
    fn credentials() {
        for url in [
            "https://user@host/x.git",
            "https://user:pass@host/x.git",
            "https://token@github.com/o/r.git",
            "ssh://git:pass@host/x.git",
            "git:pass@host:x",
            "http://user@host/r",
            "git://user@host/r",
        ] {
            let error = validate_url(url).unwrap_err();
            assert_eq!(error.code, "invalid_input");
            assert_eq!(
                error.detail.as_deref(),
                Some("Remote URL credentials are not allowed")
            );
        }
        for url in [
            "ssh://git@host/x.git",
            "git@github.com:o/r.git",
            "host:o/r@revision.git",
            "https://github.com/o/r.git",
            if cfg!(windows) {
                r"C:\tmp\repo"
            } else {
                "/tmp/repo"
            },
        ] {
            assert!(validate_url(url).is_ok());
        }
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn explicit_write_deadline_and_term() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("git");
        std::fs::write(
            &script,
            "#!/bin/sh\ntrap 'echo terminated > \"$MARKER\"; exit 0' TERM\nwhile :; do :; done\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut git = Git {
            executable: script,
            ..Git::default()
        };
        let marker = dir.path().join("marker");
        git.env
            .insert("MARKER".into(), marker.to_string_lossy().into_owned());
        let short = Duration::from_millis(500);
        assert_eq!(limits(false, true), limits(false, false) * 10);
        let error = git
            .run_mode(dir.path(), &[], false, false, None, None, short)
            .await
            .unwrap_err();
        assert_eq!(error.code, "timeout");
        assert!(!marker.exists());
        let error = git
            .run_mode(dir.path(), &[], false, true, None, None, short)
            .await
            .unwrap_err();
        assert_eq!(error.code, "timeout");
        assert!(marker.exists());
        std::fs::write(
            &git.executable,
            "#!/bin/sh\ntrap '' TERM\nwhile :; do :; done\n",
        )
        .unwrap();
        let started = std::time::Instant::now();
        let error = git
            .run_mode(dir.path(), &[], false, true, None, None, short)
            .await
            .unwrap_err();
        assert_eq!(error.code, "timeout");
        assert!(started.elapsed() >= Duration::from_secs(5));
        std::fs::write(&git.executable, "#!/bin/sh\nexec sleep 0.75\n").unwrap();
        assert!(
            git.run_mode(dir.path(), &[], false, true, None, None, short * 10)
                .await
                .is_ok()
        );
    }
}
