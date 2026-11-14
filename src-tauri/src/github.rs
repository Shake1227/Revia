use crate::{
    core::Core,
    error::{AppError, Result},
    git,
};
use serde_json::{Value, json};
use std::{
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{io::AsyncReadExt, process::Command};

#[derive(Default)]
pub(crate) struct GithubState {
    pub status: Option<(Instant, Value)>,
    pub repos: Option<(Instant, Vec<Value>)>,
    pub login: Option<(String, Arc<AtomicBool>)>,
}
fn installed(core: &Core) -> bool {
    core.options.git.gh.is_absolute() && core.options.git.gh.is_file()
}
pub(crate) fn command(core: &Core, args: &[&str]) -> Result<Command> {
    if !installed(core) {
        return Err(AppError::new("gh_not_installed"));
    }
    let mut cmd = crate::platform::command(&core.options.git.gh);
    cmd.args(args)
        .envs(&core.options.git.env)
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("CLICOLOR", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    Ok(cmd)
}
pub(crate) fn classify(text: &str) -> AppError {
    let l = text.to_ascii_lowercase();
    let code = if l.contains("delete_repo") {
        "scope_missing"
    } else if git::http_status(&l, "401") {
        "auth_failed"
    } else if git::http_status(&l, "404")
        || l.contains("could not resolve to a repository")
        || l.contains("repository not found")
    {
        "remote_not_found"
    } else if l.contains("rate limit") {
        "rate_limited"
    } else if git::http_status(&l, "403") {
        "permission_denied"
    } else if l.contains("not logged") || l.contains("gh auth login") {
        "not_authenticated"
    } else if l.contains("already exists") || l.contains("name already exists") {
        "repo_exists"
    } else if l.contains("could not resolve")
        || l.contains("dial tcp")
        || l.contains("error connecting")
        || l.contains("no such host")
        || l.contains("name resolution")
        || l.contains("timed out")
        || l.contains("network is unreachable")
        || l.contains("connection refused")
        || l.contains("failed to connect")
        || l.contains("timeout")
    {
        "network_unreachable"
    } else {
        "unknown"
    };
    AppError::new(code).detail(text)
}
impl Core {
    pub(crate) async fn gh(&self, args: &[&str]) -> Result<(String, String)> {
        let o = tokio::time::timeout(Duration::from_secs(60), command(self, args)?.output())
            .await
            .map_err(|_| AppError::new("timeout"))??;
        let out = String::from_utf8_lossy(&o.stdout).into_owned();
        let err = String::from_utf8_lossy(&o.stderr).into_owned();
        if !o.status.success() {
            return Err(classify(&format!("{err}\n{out}")));
        }
        Ok((out, err))
    }
    pub(crate) fn invalidate_github(&self) {
        let mut s = self.github.lock().unwrap();
        s.status = None;
        s.repos = None;
    }
    pub async fn github_status(&self, refresh: bool) -> Result<Value> {
        if !refresh
            && let Some((at, status)) = &self.github.lock().unwrap().status
            && at.elapsed() < Duration::from_secs(300)
        {
            return Ok(status.clone());
        }
        let mut status = json!({"cliInstalled":installed(self),"authenticated":false,"account":null,"gitProtocol":null,"missingScopes":[],"tokenStorage":null});
        if installed(self) {
            match self
                .gh(&["auth", "status", "--hostname", "github.com"])
                .await
            {
                Ok((out, err)) => {
                    status["authenticated"] = json!(true);
                    let user = self.gh(&["api", "user"]).await?;
                    let u: Value = safe_value(serde_json::from_str(&user.0)?);
                    status["account"] =
                        json!({"login":u["login"],"name":u["name"],"avatarUrl":u["avatar_url"]});
                    if let Ok((p, _)) = self
                        .gh(&["config", "get", "git_protocol", "--host", "github.com"])
                        .await
                        && matches!(p.trim(), "ssh" | "https")
                    {
                        status["gitProtocol"] = json!(p.trim());
                    }
                    let scopes = format!("{out}\n{err}");
                    status["tokenStorage"] = json!(token_storage(&scopes));
                    let repo_scope = has_repo_scope(&scopes);
                    if !repo_scope {
                        status["missingScopes"] = json!(["repo"]);
                    }
                }
                Err(e)
                    if matches!(
                        e.code.as_str(),
                        "not_authenticated" | "unknown" | "auth_failed"
                    ) => {}
                Err(e) => return Err(e),
            }
        }
        self.github.lock().unwrap().status = Some((Instant::now(), status.clone()));
        Ok(status)
    }
    pub(crate) async fn authenticated(&self) -> Result<Value> {
        if !installed(self) {
            return Err(AppError::new("gh_not_installed"));
        }
        let s = self.github_status(false).await?;
        if s["authenticated"] != true {
            return Err(AppError::new("not_authenticated"));
        }
        Ok(s)
    }
    pub async fn github_logout(&self) -> Result<Value> {
        let result = self
            .gh(&["auth", "logout", "--hostname", "github.com"])
            .await;
        self.invalidate_github();
        result?;
        self.github_status(true).await
    }
    pub async fn github_list_repos(&self, refresh: bool) -> Result<Vec<Value>> {
        self.authenticated().await?;
        let cached = if refresh {
            None
        } else {
            self.github
                .lock()
                .unwrap()
                .repos
                .as_ref()
                .filter(|(at, _)| at.elapsed() < Duration::from_secs(300))
                .map(|(_, v)| v.clone())
        };
        let mut repos = if let Some(v) = cached {
            v
        } else {
            let (out, _) = self.gh(&["api", "--paginate", "user/repos?per_page=100&sort=updated&affiliation=owner,collaborator,organization_member"]).await?;
            let values = pages(&out)?;
            let repos = values.into_iter().take(1000).map(|r| json!({
                "id":r["id"].as_u64().map(|i| i.to_string()).or_else(||r["id"].as_str().map(str::to_owned)).unwrap_or_default(),
                "nameWithOwner":r["full_name"],"owner":r["owner"]["login"],"name":r["name"],"description":r["description"],
                "visibility":r["visibility"].as_str().unwrap_or(if r["private"]==true {"private"} else {"public"}),
                "url":crate::error::strip_userinfo(r["html_url"].as_str().unwrap_or("")),
                "cloneUrl":crate::error::strip_userinfo(r["clone_url"].as_str().unwrap_or("")),"sshUrl":r["ssh_url"],
                "defaultBranch":r["default_branch"],"updatedAt":timestamp(r["updated_at"].as_str().unwrap_or("")),"fork":r["fork"].as_bool().unwrap_or(false),"archived":r["archived"].as_bool().unwrap_or(false),"localProjectIds":[]
            })).collect::<Vec<_>>();
            self.github.lock().unwrap().repos = Some((Instant::now(), repos.clone()));
            repos
        };
        let projects = self.state.lock().unwrap().projects.clone();
        let mut locals = Vec::new();
        for p in projects {
            let root = std::path::Path::new(p["path"].as_str().unwrap());
            if let Ok(urls) = self.options.git.local(root, &["remote", "-v"]).await {
                for line in urls.lines() {
                    if let Some(r) = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(crate::status::github)
                    {
                        locals.push((
                            format!(
                                "{}/{}",
                                r["owner"].as_str().unwrap(),
                                r["repo"].as_str().unwrap()
                            ),
                            p["id"].clone(),
                        ));
                    }
                }
            }
        }
        for r in &mut repos {
            let mut ids = Vec::new();
            for (name, id) in &locals {
                if name.eq_ignore_ascii_case(r["nameWithOwner"].as_str().unwrap_or(""))
                    && !ids.contains(id)
                {
                    ids.push(id.clone());
                }
            }
            r["localProjectIds"] = json!(ids);
        }
        Ok(repos)
    }
    pub async fn github_list_owners(&self) -> Result<Vec<Value>> {
        let s = self.authenticated().await?;
        let mut owners = vec![
            json!({"login":s["account"]["login"],"kind":"user","avatarUrl":s["account"]["avatarUrl"]}),
        ];
        let (out, _) = self.gh(&["api", "--paginate", "user/orgs"]).await?;
        owners.extend(pages(&out)?.into_iter().map(
            |o| json!({"login":o["login"],"kind":"organization","avatarUrl":o["avatar_url"]}),
        ));
        Ok(owners)
    }
    pub async fn github_login_start(
        self: &Arc<Self>,
        scopes: Option<Vec<String>>,
    ) -> Result<Value> {
        if let Some(scopes) = &scopes {
            if scopes.as_slice() != ["delete_repo"] {
                return Err(AppError::new("invalid_input"));
            }
            if self.github.lock().unwrap().login.is_some() {
                return Err(AppError::new("busy"));
            }
            if !installed(self) {
                return Err(AppError::new("gh_not_installed"));
            }
            if self.github_status(true).await?["authenticated"] != true {
                return Err(AppError::new("not_authenticated"));
            }
        }
        if !installed(self) {
            return Err(AppError::new("gh_not_installed"));
        }
        let mut state = self.github.lock().unwrap();
        if state.login.is_some() {
            return Err(AppError::new("busy"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        state.login = Some((id.clone(), cancel.clone()));
        drop(state);
        self.invalidate_github();
        let core = self.clone();
        let login_id = id.clone();
        tokio::spawn(async move {
            let result = tokio::select! {
                biased;
                _ = crate::oauth::cancelled(&cancel) => Err(AppError::new("cancelled")),
                result = async {
                    if let Some(config) = &core.options.oauth {
                        if crate::oauth::login(&core, config, &login_id, scopes.is_some()).await? {
                            core.legacy_login(&login_id, scopes.is_some(), &cancel).await?;
                        }
                    } else {
                        core.legacy_login(&login_id, scopes.is_some(), &cancel).await?;
                    }
                    core.invalidate_github();
                    core.github_status(true).await
                } => result,
            };
            let event = match result {
                Ok(status) => json!({"loginId":login_id,"kind":"succeeded","status":status}),
                Err(e) if e.code == "cancelled" => json!({"loginId":login_id,"kind":"cancelled"}),
                Err(e) => json!({"loginId":login_id,"kind":"failed","error":e}),
            };
            core.github.lock().unwrap().login = None;
            core.options.events.emit("revia:github-login", event);
        });
        Ok(json!({"loginId":id}))
    }
    async fn legacy_login(&self, id: &str, additional: bool, cancel: &AtomicBool) -> Result<()> {
        let mut cmd = if additional {
            command(
                self,
                &[
                    "auth",
                    "refresh",
                    "--hostname",
                    "github.com",
                    "--scopes",
                    "delete_repo",
                ],
            )?
        } else {
            command(
                self,
                &[
                    "auth",
                    "login",
                    "--hostname",
                    "github.com",
                    "--git-protocol",
                    "https",
                    "--web",
                    "--skip-ssh-key",
                ],
            )?
        };
        #[cfg(unix)]
        {
            let noop = git::find_tool("true");
            if !noop.is_file() {
                return Err(AppError::new("invalid_input")
                    .detail("Cannot find true to suppress browser launch"));
            }
            cmd.env("GH_BROWSER", &noop).env("BROWSER", &noop);
        }
        #[cfg(windows)]
        cmd.env("GH_BROWSER", "cmd /c exit 0")
            .env("BROWSER", "cmd /c exit 0");
        let mut child = cmd.spawn()?;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let out = read_login(child.stdout.take().unwrap(), tx.clone());
        let err = read_login(child.stderr.take().unwrap(), tx);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
        let mut buffer = String::new();
        let mut sent = false;
        let result: Result<()> = loop {
            if cancel.load(Ordering::SeqCst) {
                let _ = child.kill().await;
                break Err(AppError::new("cancelled"));
            }
            if tokio::time::Instant::now() >= deadline {
                let _ = child.kill().await;
                break Err(AppError::new("timeout"));
            }
            while let Ok(bytes) = rx.try_recv() {
                buffer.push_str(&String::from_utf8_lossy(&bytes));
                if !sent && let Some(code) = device_code(&buffer) {
                    self.options.events.emit("revia:github-login", json!({"loginId":id,"kind":"code","userCode":code,"verificationUrl":"https://github.com/login/device"}));
                    sent = true;
                }
                if buffer.len() > 32768 {
                    buffer = buffer
                        .chars()
                        .rev()
                        .take(8192)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect();
                }
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    break if status.success() {
                        Ok(())
                    } else {
                        Err(classify(&buffer))
                    };
                }
                Err(e) => break Err(e.into()),
                _ => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        };
        if result
            .as_ref()
            .is_err_and(|e| matches!(e.code.as_str(), "cancelled" | "timeout"))
        {
            out.abort();
            err.abort();
        }
        let _ = out.await;
        let _ = err.await;
        while let Some(bytes) = rx.recv().await {
            buffer.push_str(&String::from_utf8_lossy(&bytes));
        }
        if !sent && let Some(code) = device_code(&buffer) {
            self.options.events.emit("revia:github-login", json!({"loginId":id,"kind":"code","userCode":code,"verificationUrl":"https://github.com/login/device"}));
        }

        let result = match result {
            Err(e) if e.code == "unknown" => Err(classify(&buffer)),
            other => other,
        };
        result.map_err(|mut error| {
            if let Some(code) = device_code(&buffer)
                && let Some(detail) = &mut error.detail
            {
                *detail = detail.replace(&code, "[redacted]");
            }
            error
        })
    }
    pub fn github_login_cancel(&self, id: &str) {
        if let Some((current, cancel)) = &self.github.lock().unwrap().login
            && current == id
        {
            cancel.store(true, Ordering::SeqCst);
        }
    }
}
fn read_login<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
    mut stream: R,
    tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut b = [0; 1024];
        while let Ok(n) = stream.read(&mut b).await {
            if n == 0 {
                break;
            }
            if tx.send(b[..n].to_vec()).is_err() {
                break;
            }
        }
    })
}
fn device_code(text: &str) -> Option<String> {
    if !text.contains("https://github.com/login/device") {
        return None;
    }
    text.split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .find(|s| {
            s.len() == 9
                && s.as_bytes()[4] == b'-'
                && s.bytes()
                    .enumerate()
                    .all(|(i, b)| i == 4 || b.is_ascii_uppercase() || b.is_ascii_digit())
        })
        .map(str::to_owned)
}
fn pages(text: &str) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for value in serde_json::Deserializer::from_str(text).into_iter::<Value>() {
        let v = value?;
        let a = v.as_array().ok_or_else(|| AppError::new("invalid_input"))?;
        for r in a {
            if let Some(page) = r.as_array() {
                rows.extend(page.iter().cloned().map(safe_value));
            } else {
                rows.push(safe_value(r.clone()));
            }
        }
    }
    Ok(rows)
}
fn timestamp(text: &str) -> u64 {
    let parts = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<i64>().ok())
        .collect::<Vec<_>>();
    if parts.len() < 6 {
        return 0;
    }
    let (mut y, m, d) = (parts[0], parts[1], parts[2]);
    y -= i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + (153 * mp + 2) / 5 + d - 1 - 719468;
    ((days * 86400 + parts[3] * 3600 + parts[4] * 60 + parts[5]).max(0) as u64) * 1000
}

fn safe_value(mut v: Value) -> Value {
    match &mut v {
        Value::String(s) => *s = crate::error::sanitize(s),
        Value::Array(a) => a.iter_mut().for_each(|v| *v = safe_value(v.take())),
        Value::Object(o) => o.values_mut().for_each(|v| *v = safe_value(v.take())),
        _ => {}
    }
    v
}

fn has_repo_scope(output: &str) -> bool {
    let mut active = true;
    output.lines().any(|line| {
        let line = line.to_ascii_lowercase();
        if let Some((_, value)) = line.split_once("active account:") {
            active = value.trim() == "true";
        }
        active
            && line.split_once("scopes:").is_some_and(|(_, scopes)| {
                scopes
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .any(|s| s.trim_matches(['\'', '"']) == "repo")
            })
    })
}
#[cfg(test)]
mod tests {
    #[test]
    fn only_full_repo_scope_on_active_account_satisfies_publish() {
        for (output, expected) in [
            ("Token scopes: 'repo', 'read:org'", true),
            ("Token scopes: 'repo:status', 'public_repo'", false),
            (
                "Active account: true\nToken scopes: 'repo:status'\nActive account: false\nToken scopes: 'repo'",
                false,
            ),
        ] {
            assert_eq!(super::has_repo_scope(output), expected);
        }
    }
}

fn token_storage(text: &str) -> Option<&'static str> {
    let mut storage = None;
    for line in text.lines() {
        if line.contains("Logged in to github.com account ") {
            storage = line
                .rsplit_once('(')
                .and_then(|(_, s)| s.split_once(')'))
                .filter(|(s, _)| !s.is_empty())
                .map(|(s, _)| match s {
                    "keyring" => "keyring",
                    "GH_TOKEN" | "GITHUB_TOKEN" => "environment",
                    _ => "file",
                });
        } else if line.contains("Active account: true") {
            return storage;
        }
    }
    None
}
#[cfg(test)]
mod storage_tests {
    use super::token_storage;
    #[test]
    fn active_account_storage() {
        for (source, expected) in [
            ("keyring", "keyring"),
            ("GH_TOKEN", "environment"),
            ("GITHUB_TOKEN", "environment"),
            ("/home/u/.config/gh/hosts.yml", "file"),
            (r"C:\Users\u\AppData\Roaming\GitHub CLI\hosts.yml", "file"),
        ] {
            assert_eq!(
                token_storage(&format!(
                    "Logged in to github.com account first (keyring)\nActive account: false\nLogged in to github.com account second ({source})\nActive account: true"
                )),
                Some(expected)
            );
        }
        assert_eq!(token_storage("Not logged in"), None);
    }
}
