use crate::{
    error::{Result, strip_userinfo},
    git::Git,
};
use serde_json::{Value, json};
use std::{fs, path::Path};
async fn names(git: &Git, path: &Path, detailed: bool) -> Result<Vec<Value>> {
    let raw = git
        .local(
            path,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                if detailed {
                    "--untracked-files=all"
                } else {
                    "--untracked-files=normal"
                },
            ],
        )
        .await?;
    let mut records = raw.split('\0');
    let mut files = vec![];
    while let Some(record) = records.next() {
        if files.len() >= 5000 {
            break;
        }
        if record.len() < 4 {
            continue;
        }
        let xy = &record.as_bytes()[..2];
        let name = &record[3..];
        let conflict = matches!(xy, b"DD" | b"AU" | b"UD" | b"UA" | b"DU" | b"AA" | b"UU");
        let kind = if conflict {
            "conflicted"
        } else if xy == b"??" {
            "untracked"
        } else if xy.contains(&b'R') {
            "renamed"
        } else if xy.contains(&b'C') {
            "copied"
        } else if xy.contains(&b'D') {
            "deleted"
        } else if xy.contains(&b'A') {
            "added"
        } else if xy.contains(&b'T') {
            "type_changed"
        } else {
            "modified"
        };
        let old = if matches!(kind, "renamed" | "copied") {
            records.next().map(str::to_owned)
        } else {
            None
        };
        let stats = (None::<usize>, None::<usize>, false);
        files.push(json!({"path":name,"oldPath":old,"kind":kind,"staged":xy[0]!=b' ' && xy[0]!=b'?',"unstaged":xy[1]!=b' ' && xy[1]!=b'?',"additions":stats.0,"deletions":stats.1,"binary":stats.2}));
    }
    let mut unique = std::collections::BTreeMap::<String, Value>::new();
    for file in files {
        let key = file["path"].as_str().unwrap().to_string();
        if let Some(existing) = unique.get_mut(&key) {
            existing["staged"] = json!(existing["staged"] == true || file["staged"] == true);
            existing["unstaged"] = json!(true);
            existing["kind"] = json!("modified");
        } else {
            unique.insert(key, file);
        }
    }
    Ok(unique.into_values().collect())
}
pub fn numstats(raw: &str) -> std::collections::BTreeMap<String, (Option<u64>, Option<u64>, bool)> {
    let mut out = std::collections::BTreeMap::new();
    let mut parts = raw.split('\0');
    while let Some(r) = parts.next() {
        let mut f = r.splitn(3, '\t');
        let (Some(a), Some(d), Some(p)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        let path = if p.is_empty() {
            parts.next();
            parts.next().unwrap_or("")
        } else {
            p
        };
        out.insert(path.into(), (a.parse().ok(), d.parse().ok(), a == "-"));
    }
    out
}
pub(crate) async fn change_names(git: &Git, path: &Path) -> Result<Vec<Value>> {
    names(git, path, true).await
}
pub async fn changes(git: &Git, path: &Path) -> Result<Vec<Value>> {
    use std::io::Read;
    let mut files = names(git, path, true).await?;
    let head = if git
        .local(path, &["rev-parse", "--verify", "HEAD"])
        .await
        .is_ok()
    {
        "HEAD"
    } else {
        "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
    };
    let raw = git
        .local(
            path,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--numstat",
                "-z",
                "-M",
                head,
                "--",
            ],
        )
        .await?;
    let stats = numstats(&raw);
    for f in &mut files {
        let name = f["path"].as_str().unwrap();
        let counts =
            if f["kind"] == "untracked" {
                let bytes = crate::git::relative(path, name)
                    .ok()
                    .and_then(|p| {
                        let m = std::fs::metadata(&p).ok()?;
                        if !m.is_file() || m.len() > 1024 * 1024 {
                            return None;
                        }
                        std::fs::File::open(p).ok()
                    })
                    .and_then(|file| {
                        let mut b = vec![];
                        file.take(1024 * 1024 + 1).read_to_end(&mut b).ok()?;
                        Some(b)
                    });
                bytes.map(|b| {
                    if b.len() > 1024 * 1024 {
                        (None, None, false)
                    } else if b.contains(&0) {
                        (None, None, true)
                    } else {
                        (
                            Some(b.split(|v| *v == b'\n').count().saturating_sub(usize::from(
                                b.is_empty() || b.last() == Some(&b'\n'),
                            )) as u64),
                            Some(0),
                            false,
                        )
                    }
                })
            } else {
                stats.get(name).copied()
            };
        if let Some((a, d, b)) = counts {
            f["additions"] = json!(a);
            f["deletions"] = json!(d);
            f["binary"] = json!(b);
        }
    }
    Ok(files)
}
pub async fn upstream(git: &Git, path: &Path, branch: &str) -> Option<Value> {
    let r = git
        .local(
            path,
            &["config", "--get", &format!("branch.{branch}.remote")],
        )
        .await
        .ok()?;
    let b = git
        .local(
            path,
            &["config", "--get", &format!("branch.{branch}.merge")],
        )
        .await
        .ok()?;
    if r.trim() == "." {
        return None;
    }
    Some(json!({"remote":r.trim(),"branch":b.trim().strip_prefix("refs/heads/")?}))
}
pub async fn exists(g: &Git, p: &Path, r: &Value) -> bool {
    g.local(p, &["show-ref", "--verify", "--quiet", &tracking(r)])
        .await
        .is_ok()
}
pub fn tracking(r: &Value) -> String {
    format!(
        "refs/remotes/{}/{}",
        r["remote"].as_str().unwrap_or(""),
        r["branch"].as_str().unwrap_or("")
    )
}
fn target(
    s: &Value,
    branch: Option<&str>,
    remotes: &[Value],
    up: &Option<Value>,
    push_ref: Option<&Value>,
    refs: &std::collections::BTreeSet<String>,
    push: bool,
) -> Value {
    let empty = || json!({"target":null,"saved":null,"savedInvalid":null});
    let Some(branch) = branch else {
        return empty();
    };
    let exists = |r: &Value| refs.contains(&tracking(r));
    let resolved = |r: &Value, source: &str| json!({"remote":r["remote"],"branch":r["branch"],"source":source,"exists":exists(r)});
    let saved = &s["branchTargets"][branch][if push { "push" } else { "pull" }];
    if !saved.is_null() {
        let invalid = if !remotes.iter().any(|r| r["name"] == saved["remote"]) {
            Some("remote_missing")
        } else if !push && !exists(saved) {
            Some("branch_missing")
        } else {
            None
        };
        return json!({"target":if invalid.is_none(){Some(resolved(saved,"saved"))}else{None},"saved":saved,"savedInvalid":invalid});
    }
    for r in [if push { push_ref } else { None }, up.as_ref()]
        .into_iter()
        .flatten()
    {
        if remotes.iter().any(|v| v["name"] == r["remote"]) && exists(r) {
            return json!({"target":resolved(r,"upstream"),"saved":null,"savedInvalid":null});
        }
    }
    let remote = if let Some(r) = s["defaultRemote"].as_str() {
        remotes.iter().find(|v| v["name"] == r)
    } else if remotes.len() == 1 {
        remotes.first()
    } else {
        remotes.iter().find(|v| v["name"] == "origin")
    };
    if let Some(remote) = remote {
        let r = json!({"remote":remote["name"],"branch":branch});
        let ex = exists(&r);
        if push || ex {
            return json!({"target":resolved(&r,if ex{"remote_default"}else{"inferred"}),"saved":null,"savedInvalid":null});
        }
    }
    empty()
}
pub async fn count(g: &Git, p: &Path, range: &str) -> Option<u64> {
    g.local(p, &["rev-list", "--count", range])
        .await
        .ok()?
        .trim()
        .parse()
        .ok()
}
pub async fn git_dir(g: &Git, p: &Path) -> Option<std::path::PathBuf> {
    Some(p.join(g.local(p, &["rev-parse", "--git-dir"]).await.ok()?.trim()))
}
pub fn operation_at(dir: &Path) -> Option<String> {
    [
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
        ("MERGE_HEAD", "merge"),
        ("CHERRY_PICK_HEAD", "cherry_pick"),
        ("REVERT_HEAD", "revert"),
        ("BISECT_LOG", "bisect"),
    ]
    .into_iter()
    .find(|(file, _)| dir.join(file).exists())
    .map(|(_, kind)| kind.into())
}
pub async fn in_progress(g: &Git, p: &Path) -> Option<String> {
    operation_at(&git_dir(g, p).await?)
}
pub fn github(url: &str) -> Option<Value> {
    let clean = strip_userinfo(url);
    let rest = clean
        .strip_prefix("https://github.com/")
        .or_else(|| clean.strip_prefix("git@github.com:"))
        .or_else(|| clean.strip_prefix("ssh://github.com/"))?;
    let parts = rest
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .split('/')
        .collect::<Vec<_>>();
    if parts.len() != 2 || parts.iter().any(|p| p.is_empty() || p.contains(['?', '#'])) {
        return None;
    }
    Some(
        json!({"owner":parts[0],"repo":parts[1],"url":format!("https://github.com/{}/{}",parts[0],parts[1])}),
    )
}
pub async fn read(
    g: &Git,
    project: &Value,
    remote_error: Option<&str>,
    fetched: Option<u64>,
) -> Result<Value> {
    let p = Path::new(project["path"].as_str().unwrap());
    let mut v = empty();
    v["remoteError"] = json!(remote_error);
    v["fetchedAt"] = json!(fetched);
    if !p.is_dir() {
        v["state"] = json!("missing");
        return Ok(v);
    }
    if crate::platform::canonicalize(p).is_ok_and(|actual| actual != p) {
        return Err(crate::error::AppError::new("invalid_path"));
    }
    if !p.join(".git").exists() {
        v["state"] = json!("not_initialized");
        return Ok(v);
    }
    if fs::symlink_metadata(p.join(".git")).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(crate::error::AppError::new("invalid_path"));
    }
    let raw = g
        .local(
            p,
            &[
                "status",
                "--porcelain=v2",
                "--branch",
                "-z",
                "--untracked-files=normal",
            ],
        )
        .await?;
    let field = |prefix: &str| raw.split('\0').find_map(|r| r.strip_prefix(prefix));
    let head = field("# branch.oid ")
        .filter(|v| *v != "(initial)")
        .map(str::to_owned);
    let branch = field("# branch.head ")
        .filter(|v| *v != "(detached)")
        .map(str::to_owned);
    let unborn = head.is_none();
    let detached = branch.is_none() && !unborn;
    let mut files = vec![];
    let mut records = raw.split('\0');
    while let Some(r) = records.next() {
        let (kind, xy) = match r.as_bytes().first() {
            Some(b'1' | b'2' | b'u') => (
                if r.starts_with('u') {
                    "conflicted"
                } else {
                    "modified"
                },
                r.get(2..4).unwrap_or(".."),
            ),
            Some(b'?') => ("untracked", ".."),
            _ => continue,
        };
        let path = match r.as_bytes().first() {
            Some(b'1') => r.splitn(9, ' ').nth(8),
            Some(b'2') => r.splitn(10, ' ').nth(9),
            Some(b'u') => r.splitn(11, ' ').nth(10),
            _ => r.get(2..),
        };
        if r.starts_with('2') {
            records.next();
        }
        files.push(
            json!({"kind":kind,"path":path,"staged":xy.as_bytes()[0]!=b'.',"unstaged":xy.as_bytes()[1]!=b'.'}),
        );
    }
    let conflicts = files.iter().filter(|f| f["kind"] == "conflicted").count();
    let config = g
        .local(
            p,
            &["config", "--get-regexp", "^remote\\..*\\.(url|pushurl)$"],
        )
        .await
        .unwrap_or_default();
    let mut urls = std::collections::BTreeMap::<String, (String, Option<String>)>::new();
    for line in config.lines() {
        let Some((key, url)) = line.split_once(' ') else {
            continue;
        };
        let Some(key) = key.strip_prefix("remote.") else {
            continue;
        };
        if let Some(name) = key.strip_suffix(".pushurl") {
            urls.entry(name.into()).or_default().1 = Some(url.into());
        } else if let Some(name) = key.strip_suffix(".url") {
            let entry = urls.entry(name.into()).or_default();
            if entry.0.is_empty() {
                entry.0 = url.into();
            }
        }
    }
    let remotes=urls.into_iter().map(|(name,(url,push))|json!({"name":name,"fetchUrl":crate::error::strip_userinfo(&url),"pushUrl":crate::error::strip_userinfo(push.as_deref().unwrap_or(&url)),"github":github(&url)})).collect::<Vec<_>>();
    let refraw = g
        .local(
            p,
            &[
                "for-each-ref",
                "--format=%(refname)%00%(upstream)%00%(push)",
                "refs/heads",
                "refs/remotes",
            ],
        )
        .await?;
    let refs = refraw
        .lines()
        .filter_map(|l| l.split('\0').next().map(str::to_owned))
        .collect::<std::collections::BTreeSet<_>>();
    let parse_ref = |full: &str| {
        remotes
            .iter()
            .filter_map(|r| {
                let remote = r["name"].as_str()?;
                Some((
                    remote,
                    full.strip_prefix(&format!("refs/remotes/{remote}/"))?,
                ))
            })
            .max_by_key(|(r, _)| r.len())
            .map(|(remote, branch)| json!({"remote":remote,"branch":branch}))
    };
    let current = refraw.lines().find(|l| {
        l.split('\0').next()
            == branch
                .as_ref()
                .map(|b| format!("refs/heads/{b}"))
                .as_deref()
    });
    let parts = current
        .map(|l| l.split('\0').collect::<Vec<_>>())
        .unwrap_or_default();
    let up = parts.get(1).and_then(|v| parse_ref(v));
    let push_ref = parts.get(2).and_then(|v| parse_ref(v));
    let push = target(
        &project["settings"],
        branch.as_deref(),
        &remotes,
        &up,
        push_ref.as_ref(),
        &refs,
        true,
    );
    let pull = target(
        &project["settings"],
        branch.as_deref(),
        &remotes,
        &up,
        None,
        &refs,
        false,
    );
    let ahead = if push["target"]["exists"] == true {
        count(g, p, &format!("{}..HEAD", tracking(&push["target"]))).await
    } else {
        None
    };
    let behind = if pull["target"]["exists"] == true {
        count(g, p, &format!("HEAD..{}", tracking(&pull["target"]))).await
    } else {
        None
    };
    let divergence = if behind.unwrap_or(0) > 0 && (ahead.unwrap_or(0) > 0 || !files.is_empty()) {
        let mut local_at = if ahead.unwrap_or(0) > 0 {
            commit_time(g, p, "HEAD").await
        } else {
            None
        };
        for file in files.iter().take(500) {
            if let Some(at) = file["path"]
                .as_str()
                .and_then(|path| fs::symlink_metadata(p.join(path)).ok())
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
            {
                local_at = Some(local_at.map_or(at, |current: u64| current.max(at)));
            }
        }
        json!({"localAt":local_at,"remoteAt":commit_time(g, p, &tracking(&pull["target"])).await})
    } else {
        Value::Null
    };
    let dir = git_dir(g, p).await;
    let progress = dir.as_deref().and_then(operation_at);
    let state = if conflicts > 0 {
        "conflicted"
    } else if progress.is_some() {
        "in_progress"
    } else if detached {
        "detached"
    } else if remotes.is_empty() {
        "no_remote"
    } else if remote_error == Some("not_found") {
        "remote_not_found"
    } else if remote_error == Some("auth") {
        "auth_error"
    } else if !files.is_empty() {
        "dirty"
    } else if remote_error == Some("unreachable") {
        "unreachable"
    } else if ahead.unwrap_or(0) > 0 && behind.unwrap_or(0) > 0 {
        "diverged"
    } else if behind.unwrap_or(0) > 0 {
        "behind"
    } else if ahead.unwrap_or(0) > 0 {
        "ahead"
    } else if push["target"].is_null() || push["target"]["exists"] == false {
        "unpublished"
    } else {
        "synced"
    };
    let github = remotes
        .iter()
        .find(|r| r["name"] == push["target"]["remote"])
        .or_else(|| remotes.first())
        .map(|r| r["github"].clone())
        .unwrap_or(Value::Null);
    v["state"] = json!(state);
    v["branch"] = json!(branch);
    v["detached"] = json!(detached);
    v["unborn"] = json!(unborn);
    v["headSha"] = json!(head.as_ref().map(|s| s[..7.min(s.len())].to_string()));
    v["upstream"] = json!(up);
    v["targets"] = json!({"push":push,"pull":pull});
    v["ahead"] = json!(ahead);
    v["behind"] = json!(behind);
    v["divergence"] = divergence;
    v["inProgress"] = json!(progress);
    v["remotes"] = json!(remotes);
    v["github"] = github;
    v["changes"] = json!({"total":files.len(),"staged":files.iter().filter(|f|f["staged"]==true).count(),"unstaged":files.iter().filter(|f|f["unstaged"]==true).count(),"untracked":files.iter().filter(|f|f["kind"]=="untracked").count(),"conflicted":conflicts});
    v["lastCommitAt"] = json!(
        g.local(p, &["log", "-1", "--format=%ct"])
            .await
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|s| s * 1000)
    );
    if fetched.is_none() {
        v["fetchedAt"] = json!(
            dir.as_ref()
                .and_then(|dir| fs::metadata(dir.join("FETCH_HEAD")).ok())
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
        );
    }
    Ok(v)
}

async fn commit_time(g: &Git, p: &Path, reference: &str) -> Option<u64> {
    g.local(p, &["log", "-1", "--format=%ct", reference, "--"])
        .await
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|s| s * 1000)
}
pub fn empty() -> Value {
    json!({"state":"unknown","branch":null,"detached":false,"unborn":false,"headSha":null,"upstream":null,"targets":{"push":{"target":null,"saved":null,"savedInvalid":null},"pull":{"target":null,"saved":null,"savedInvalid":null}},"ahead":null,"behind":null,"divergence":null,"changes":{"staged":0,"unstaged":0,"untracked":0,"conflicted":0,"total":0},"inProgress":null,"remotes":[],"github":null,"remoteError":Option::<String>::None,"lastCommitAt":null,"checkedAt":crate::now(),"fetchedAt":Option::<u64>::None,"error":null})
}
