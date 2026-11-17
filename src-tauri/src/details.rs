use crate::{
    core::Core,
    error::{AppError, Result},
    git, status,
};
use serde_json::{Value, json};
use std::path::Path;
impl Core {
    pub async fn changes(&self, id: &str) -> Result<Vec<Value>> {
        let p = self.project(id)?;
        if !Path::new(p["path"].as_str().unwrap()).join(".git").exists() {
            return Ok(vec![]);
        }
        status::changes(&self.options.git, Path::new(p["path"].as_str().unwrap())).await
    }
    pub async fn detail(&self, id: &str) -> Result<Value> {
        self.refresh(Some(vec![id.into()])).await?;
        let project = self.project(id)?;
        let p = Path::new(project["path"].as_str().unwrap());
        let g = &self.options.git;
        if !p.join(".git").exists() {
            return Ok(json!({"project":project,"branches":[],"stashes":[]}));
        }
        let refs=g.local(p,&["for-each-ref","--format=%(refname)%00%(objectname)%00%(subject)%00%(authorname)%00%(committerdate:unix)%00%(upstream)%00%(upstream:track)","refs/heads","refs/remotes"]).await?;
        let remotes = project["status"]["remotes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let merged = g
            .local(
                p,
                &[
                    "for-each-ref",
                    "--merged=HEAD",
                    "--format=%(refname)",
                    "refs/heads",
                    "refs/remotes",
                ],
            )
            .await
            .unwrap_or_default();
        let merged = merged.lines().collect::<std::collections::BTreeSet<_>>();
        let mut branches = vec![];
        for line in refs.lines() {
            let f = line.split('\0').collect::<Vec<_>>();
            if f.len() < 5 || f[0].ends_with("/HEAD") {
                continue;
            }
            let local = f[0].starts_with("refs/heads/");
            let name = f[0]
                .strip_prefix(if local {
                    "refs/heads/"
                } else {
                    "refs/remotes/"
                })
                .unwrap_or(f[0]);
            let remote = if local {
                None
            } else {
                remotes
                    .iter()
                    .filter_map(|r| r["name"].as_str())
                    .filter(|r| name.starts_with(&format!("{r}/")))
                    .max_by_key(|r| r.len())
            };
            let short = remote
                .and_then(|r| name.strip_prefix(&format!("{r}/")))
                .unwrap_or(name);
            let upstream = f.get(5).copied().unwrap_or("");
            let up = remotes
                .iter()
                .filter_map(|r| {
                    let remote = r["name"].as_str()?;
                    Some((
                        remote,
                        upstream.strip_prefix(&format!("refs/remotes/{remote}/"))?,
                    ))
                })
                .max_by_key(|(r, _)| r.len())
                .map(|(remote, branch)| json!({"remote":remote,"branch":branch}));
            let track = f.get(6).copied().unwrap_or("");
            let exists = up.is_some() && track != "[gone]";
            let number = |prefix: &str| {
                track
                    .trim_matches(['[', ']'])
                    .split(", ")
                    .find_map(|s| s.strip_prefix(prefix).and_then(|n| n.parse::<u64>().ok()))
                    .unwrap_or(0)
            };
            let ahead = exists.then(|| number("ahead "));
            let behind = exists.then(|| number("behind "));
            branches.push(json!({"name":name,"kind":if local{"local"}else{"remote"},"remote":remote,"shortName":short,"current":local&&project["status"]["branch"]==name,"sha":f[1],"upstream":up,"upstreamGone":up.is_some()&&!exists,"ahead":ahead,"behind":behind,"mergedIntoCurrent":merged.contains(f[0]),"lastCommit":{"subject":f[2],"authorName":f[3],"at":f[4].parse::<u64>().unwrap_or(0)*1000}}));
        }
        let stashes = g
            .local(p, &["stash", "list", "--format=%H%x00%gs%x00%ct"])
            .await?;
        let stashes=stashes.lines().enumerate().filter_map(|(index,l)|{let f=l.split('\0').collect::<Vec<_>>();(f.len()==3).then(||json!({"sha":f[0],"index":index,"message":f[1],"branch":f[1].strip_prefix("WIP on ").or_else(||f[1].strip_prefix("On ")).and_then(|v|v.split(':').next()),"at":f[2].parse::<u64>().unwrap_or(0)*1000}))}).collect::<Vec<_>>();
        Ok(json!({"project":project,"branches":branches,"stashes":stashes}))
    }
    pub async fn conflicts(&self, id: &str) -> Result<Value> {
        let p = self.project(id)?;
        let root = Path::new(p["path"].as_str().unwrap());
        let g = &self.options.git;
        if !root.join(".git").exists() {
            return Ok(json!({"operation":null,"incoming":null,"files":[],"canContinue":false}));
        }
        let operation = status::in_progress(g, root).await;
        let raw = g.local(root, &["status", "--porcelain=v1", "-z"]).await?;
        let mut files = vec![];
        let mut records = raw.split('\0');
        while let Some(r) = records.next() {
            if r.len() < 4 {
                continue;
            }
            let code = &r.as_bytes()[..2];
            if code.contains(&b'R') || code.contains(&b'C') {
                records.next();
                continue;
            }
            let kind = match code {
                b"UU" => "both_modified",
                b"AA" => "both_added",
                b"DD" => "both_deleted",
                b"AU" => "added_by_us",
                b"UA" => "added_by_them",
                b"DU" => "deleted_by_us",
                b"UD" => "deleted_by_them",
                _ => continue,
            };
            let path = &r[3..];
            let has = git::relative(root, path)
                .ok()
                .and_then(|p| crate::markers::file(&p).ok())
                .unwrap_or(true);
            files.push(json!({"path":path,"kind":kind,"hasMarkers":has}));
        }
        let incoming = if let Some(kind) = operation.as_deref() {
            let dir = status::git_dir(g, root).await;
            let merge_name = dir
                .as_ref()
                .and_then(|d| std::fs::read_to_string(d.join("MERGE_MSG")).ok())
                .and_then(|m| {
                    m.lines()
                        .next()
                        .and_then(|l| {
                            l.strip_prefix("Merge branch '")
                                .or_else(|| l.strip_prefix("Merge remote-tracking branch '"))
                        })
                        .and_then(|s| s.split('\'').next())
                        .map(str::to_owned)
                });
            let reference = match kind {
                "merge" => "MERGE_HEAD",
                "cherry_pick" => "CHERRY_PICK_HEAD",
                "revert" => "REVERT_HEAD",
                _ => "REBASE_HEAD",
            };
            if kind == "merge" && merge_name.is_some() {
                merge_name
            } else if let Ok(sha) = g.local(root, &["rev-parse", "--short", reference]).await {
                let name = g
                    .local(root, &["name-rev", "--name-only", reference])
                    .await
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| s != "undefined");
                Some(name.unwrap_or_else(|| sha.trim().into()))
            } else {
                None
            }
        } else {
            None
        };
        Ok(
            json!({"operation":operation,"incoming":incoming,"canContinue":operation.is_some()&&files.is_empty(),"files":files}),
        )
    }
    pub async fn commit_log(
        &self,
        id: &str,
        rev: Option<String>,
        limit: u32,
        skip: u32,
    ) -> Result<Vec<Value>> {
        if !(1..=200).contains(&limit) {
            return Err(AppError::new("invalid_input"));
        }
        let project = self.project(id)?;
        let root = Path::new(project["path"].as_str().unwrap());
        let g = &self.options.git;
        if rev.is_none()
            && g.local(root, &["rev-parse", "--verify", "HEAD"])
                .await
                .is_err()
        {
            return Ok(vec![]);
        }
        let rev = rev.unwrap_or("HEAD".into());
        let sha = resolve(g, root, &rev).await?;
        self.commit_log_resolved(id, &sha, limit, skip).await
    }
    pub(crate) async fn commit_log_resolved(
        &self,
        id: &str,
        sha: &str,
        limit: u32,
        skip: u32,
    ) -> Result<Vec<Value>> {
        let project = self.project(id)?;
        let root = Path::new(project["path"].as_str().unwrap());
        let g = &self.options.git;
        let raw = g
            .local(
                root,
                &[
                    "log",
                    "--format=%H%x00%h%x00%s%x00%b%x00%an%x00%ct%x00%P%x00%D%x00%x1e",
                    &format!("--max-count={limit}"),
                    &format!("--skip={skip}"),
                    sha,
                    "--",
                ],
            )
            .await?;
        let target = &project["status"]["targets"]["push"]["target"];
        let unpushed = if target["exists"] == true {
            Some(
                g.local(
                    root,
                    &["rev-list", &format!("^{}", status::tracking(target)), sha],
                )
                .await?
                .lines()
                .map(str::to_owned)
                .collect::<std::collections::BTreeSet<_>>(),
            )
        } else {
            None
        };
        let mut out = vec![];
        for record in raw.split('\u{1e}') {
            let f = record
                .trim_start_matches(['\r', '\n'])
                .split('\0')
                .collect::<Vec<_>>();
            if f.len() < 8 {
                continue;
            }
            let pushed = unpushed.as_ref().map(|set| !set.contains(f[0]));
            out.push(json!({"sha":f[0],"shortSha":f[1],"subject":f[2],"body":f[3],"authorName":f[4],"at":f[5].parse::<u64>().unwrap_or(0)*1000,"parents":f[6].split_whitespace().collect::<Vec<_>>(),"refs":f[7].split(", ").filter(|s|!s.is_empty()).map(str::to_owned).collect::<Vec<_>>(),"pushed":pushed}));
        }
        Ok(out)
    }
    pub async fn commit_detail(&self, id: &str, sha: String) -> Result<Value> {
        let commit = self
            .commit_log(id, Some(sha.clone()), 1, 0)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| AppError::new("invalid_input"))?;
        let project = self.project(id)?;
        let root = Path::new(project["path"].as_str().unwrap());
        let sha = resolve(&self.options.git, root, &sha).await?;
        let raw = self
            .options
            .git
            .local(
                root,
                &[
                    "diff-tree",
                    "--root",
                    "--no-commit-id",
                    "--name-status",
                    "--diff-merges=first-parent",
                    "-r",
                    "-M",
                    "-z",
                    &sha,
                    "--",
                ],
            )
            .await?;
        let stats_raw = self
            .options
            .git
            .local(
                root,
                &[
                    "show",
                    "--format=",
                    "--diff-merges=first-parent",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--numstat",
                    "-z",
                    "--find-renames",
                    &sha,
                    "--",
                ],
            )
            .await?;
        let all_stats = status::numstats(&stats_raw);
        let mut parts = raw.split('\0');
        let mut files = vec![];
        while let Some(code) = parts.next() {
            if code.is_empty() {
                continue;
            }
            let Some(first) = parts.next() else {
                break;
            };
            let (old, path) = if code.starts_with(['R', 'C']) {
                (Some(first), parts.next().unwrap_or(first))
            } else {
                (None, first)
            };
            let (additions, deletions, binary) =
                all_stats.get(path).copied().unwrap_or((None, None, false));
            files.push(json!({"path":path,"oldPath":old,"kind":match code.as_bytes()[0]{b'A'=>"added",b'D'=>"deleted",b'R'=>"renamed",b'C'=>"copied",b'T'=>"type_changed",_=>"modified"},"staged":false,"unstaged":false,"additions":additions,"deletions":deletions,"binary":binary}));
        }
        Ok(json!({"commit":commit,"files":files}))
    }
    pub async fn diff(&self, id: &str, path: String, source: Value) -> Result<Value> {
        let project = self.project(id)?;
        let root = Path::new(project["path"].as_str().unwrap());
        if !root.join(".git").exists() {
            return Err(AppError::new("not_a_repository"));
        }
        let full = git::git_path(root, &path)?;
        let mut input_truncated = false;
        let g = &self.options.git;
        let mut old_path = Value::Null;
        let raw = match source["kind"].as_str() {
            Some("commit") => {
                let sha = resolve(
                    g,
                    root,
                    source["sha"]
                        .as_str()
                        .ok_or_else(|| AppError::new("invalid_input"))?,
                )
                .await?;
                let metadata = g
                    .local(
                        root,
                        &[
                            "diff-tree",
                            "--root",
                            "--no-commit-id",
                            "--name-status",
                            "--diff-merges=first-parent",
                            "-r",
                            "-M",
                            "-z",
                            &sha,
                            "--",
                        ],
                    )
                    .await?;
                let mut names = metadata.split('\0');
                while let Some(code) = names.next() {
                    if code.is_empty() {
                        continue;
                    }
                    let Some(first) = names.next() else {
                        break;
                    };
                    if code.starts_with(['R', 'C']) {
                        let next = names.next().unwrap_or(first);
                        if next == path {
                            old_path = json!(first);
                        }
                    }
                }
                let mut args = vec![
                    "show",
                    "--format=",
                    "--diff-merges=first-parent",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--find-renames",
                    &sha,
                    "--",
                    &path,
                ];
                if let Some(old) = old_path.as_str() {
                    args.push(old);
                }
                g.local(root, &args).await?
            }
            Some("worktree") => {
                let changes = status::change_names(g, root).await?;
                let change = changes.iter().find(|f| f["path"] == path);
                if let Some(c) = change {
                    old_path = c["oldPath"].clone();
                }
                if change.is_some_and(|c| c["kind"] == "untracked")
                    && std::fs::symlink_metadata(&full).is_ok_and(|m| m.file_type().is_symlink())
                {
                    g.symlink_diff(root, &path).await?
                } else if change.is_some_and(|c| c["kind"] == "untracked") {
                    git::relative(root, &path)?;
                    use std::io::Read;
                    let mut bytes = vec![];
                    std::fs::File::open(&full)?
                        .take(512 * 1024 + 1)
                        .read_to_end(&mut bytes)?;
                    input_truncated = bytes.len() > 512 * 1024;
                    bytes.truncate(512 * 1024);
                    if bytes.contains(&0) {
                        return Ok(
                            json!({"path":path,"oldPath":null,"binary":true,"truncated":input_truncated,"hunks":[]}),
                        );
                    }
                    let text = String::from_utf8_lossy(&bytes);
                    let mut raw = format!("@@ -0,0 +1,{} @@\n", text.lines().count());
                    for l in text.lines() {
                        raw.push('+');
                        raw.push_str(l);
                        raw.push('\n');
                    }
                    raw
                } else {
                    let head = if g
                        .local(root, &["rev-parse", "--verify", "HEAD"])
                        .await
                        .is_ok()
                    {
                        "HEAD"
                    } else {
                        "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
                    };
                    let mut args = vec![
                        "diff",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--find-renames",
                        head,
                        "--",
                        &path,
                    ];
                    if let Some(old) = old_path.as_str() {
                        args.push(old);
                    }
                    g.local(root, &args).await?
                }
            }
            _ => return Err(AppError::new("invalid_input")),
        };
        let binary = raw
            .lines()
            .any(|l| l.starts_with("Binary files ") || l == "GIT binary patch");
        let mut hunks = vec![];
        let mut lines = vec![];
        let mut header = String::new();
        let (mut old, mut new) = (0u64, 0u64);
        let mut bytes = 0;
        let mut count = 0;
        let mut truncated = input_truncated;
        for l in raw.lines() {
            bytes += l.len() + 1;
            count += 1;
            if bytes > 512 * 1024 || count > 5000 {
                truncated = true;
                break;
            }
            if l.starts_with("@@ ") {
                if !header.is_empty() {
                    hunks.push(json!({"header":header,"lines":lines}));
                    lines = vec![];
                }
                header = l.into();
                let fields = l.split_whitespace().collect::<Vec<_>>();
                old = fields
                    .get(1)
                    .and_then(|s| s.trim_start_matches('-').split(',').next())
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                new = fields
                    .get(2)
                    .and_then(|s| s.trim_start_matches('+').split(',').next())
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                continue;
            }
            if header.is_empty() {
                continue;
            }
            let (kind, o, n) = match l.as_bytes().first() {
                Some(b'+') => {
                    let n = new;
                    new += 1;
                    ("add", None, Some(n))
                }
                Some(b'-') => {
                    let o = old;
                    old += 1;
                    ("del", Some(o), None)
                }
                Some(b' ') => {
                    let (o, n) = (old, new);
                    old += 1;
                    new += 1;
                    ("context", Some(o), Some(n))
                }
                _ => continue,
            };
            lines.push(json!({"kind":kind,"oldNo":o,"newNo":n,"text":&l[1..]}));
        }
        if !header.is_empty() {
            hunks.push(json!({"header":header,"lines":lines}));
        }
        Ok(
            json!({"path":path,"oldPath":old_path,"binary":binary,"truncated":truncated,"hunks":hunks}),
        )
    }
}
pub async fn resolve(g: &git::Git, root: &Path, name: &str) -> Result<String> {
    if name.is_empty() || name.starts_with('-') || name.chars().any(char::is_control) {
        return Err(AppError::new("invalid_ref_name"));
    }
    Ok(g.local(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{name}^{{commit}}"),
        ],
    )
    .await?
    .trim()
    .into())
}
