use crate::{
    core::Core,
    error::{AppError, Result},
    git::{self, Git},
};
use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Classification {
    pub category: &'static str,
    pub reason: &'static str,
    pub pattern: String,
}
fn test_code(relative: &str) -> bool {
    let p = relative.to_lowercase();
    let base = p.rsplit('/').next().unwrap_or("");
    p.rsplit_once('/').is_some_and(|(parent, _)| {
        parent
            .split('/')
            .any(|s| matches!(s, "tests" | "test" | "__tests__"))
    }) || matches!(base, "tests" | "__tests__" | "test.py")
        || base.contains(".test.")
        || base.contains(".spec.")
        || (base.ends_with(".py") && (base.starts_with("test_") || base.ends_with("_test.py")))
        || base.ends_with("_test.go")
}
pub fn path_pattern(relative: &str, is_dir: bool) -> String {
    let mut out = String::from("/");
    let path = relative.trim_end_matches('/');
    let trailing = path.trim_end_matches(' ').len();
    for (i, c) in path.char_indices() {
        if matches!(c, '\\' | '*' | '?' | '[')
            || (i == 0 && matches!(c, '#' | '!'))
            || (c == ' ' && i >= trailing)
        {
            out.push('\\');
        }
        out.push(c);
    }
    if is_dir {
        out.push('/');
    }
    out
}
fn classified(
    relative: &str,
    is_dir: bool,
    category: &'static str,
    reason: &'static str,
    pattern: Option<String>,
) -> Option<Classification> {
    Some(Classification {
        category,
        reason,
        pattern: pattern.unwrap_or_else(|| path_pattern(relative, is_dir)),
    })
}
pub fn classify_path(relative: &str, is_dir: bool) -> Option<Classification> {
    let tests = test_code(relative)
        || (is_dir
            && relative
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .is_some_and(|s| s.eq_ignore_ascii_case("test")));
    let raw = relative.trim_end_matches('/').rsplit('/').next()?;
    let name = raw.to_lowercase();
    let n = name.as_str();
    let ext = n.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let raw_ext = raw.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let root = |cat, reason| classified(relative, is_dir, cat, reason, None);
    let named =
        |cat, reason, pattern: String| classified(relative, is_dir, cat, reason, Some(pattern));
    if !is_dir {
        if (n == ".env" || n.starts_with(".env."))
            && ![".env.example", ".env.sample", ".env.template", ".env.dist"].contains(&n)
        {
            return root("secret", "env_file");
        }
        if !n.ends_with(".pub")
            && (["pem", "key", "p12", "pfx", "jks", "keystore", "ppk"].contains(&ext)
                || ["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"]
                    .iter()
                    .any(|s| n.starts_with(s)))
        {
            return root("secret", "key_file");
        }
        if [
            ".npmrc",
            ".netrc",
            ".pypirc",
            ".htpasswd",
            "credentials.json",
            "terraform.tfstate",
        ]
        .contains(&n)
            || (n.ends_with(".json")
                && (n.starts_with("client_secret")
                    || n.starts_with("service-account")
                    || n.split_once("-firebase-adminsdk-").is_some()))
            || n.starts_with("secrets.")
            || n.starts_with("terraform.tfstate.")
            || ["tfvars", "kdbx"].contains(&ext)
        {
            return root("secret", "credential_file");
        }
        if !tests
            && (ext == "vcf"
                || (["csv", "tsv", "xlsx", "xls", "json", "sql", "txt", "numbers"].contains(&ext)
                    && [
                        "contacts",
                        "customers",
                        "addressbook",
                        "address_book",
                        "members",
                        "顧客",
                        "名簿",
                        "住所録",
                        "連絡先",
                        "個人情報",
                    ]
                    .iter()
                    .any(|s| n.contains(s))))
        {
            return root("personal", "personal_data_file");
        }
        if !tests
            && n != "thumbs.db"
            && (["sqlite", "sqlite3", "db", "mdb", "accdb", "dump"].contains(&ext)
                || n.ends_with(".sql.gz")
                || n == "dump.sql")
        {
            return root("personal", "database_file");
        }
        if [".ds_store", "thumbs.db", "desktop.ini"].contains(&n) || n.starts_with("._") {
            return named(
                "system",
                "os_file",
                if n.starts_with("._") {
                    "._*".into()
                } else {
                    raw.into()
                },
            );
        }
        if ["swp", "swo", "iml"].contains(&ext) || n.ends_with('~') {
            return named(
                "system",
                "editor_file",
                if n.ends_with('~') {
                    "*~".into()
                } else {
                    format!("*.{raw_ext}")
                },
            );
        }
    }
    if is_dir && n == ".idea" {
        return named("system", "editor_file", format!("{raw}/"));
    }
    if is_dir
        && [
            "node_modules",
            "bower_components",
            ".venv",
            "venv",
            "pods",
            ".gradle",
            "__pypackages__",
        ]
        .contains(&n)
    {
        return named("dependency", "dependency_dir", format!("{raw}/"));
    }
    if is_dir
        && [
            "dist",
            "build",
            "out",
            "target",
            ".next",
            ".nuxt",
            ".output",
            ".svelte-kit",
            ".turbo",
            "deriveddata",
            "coverage",
            "__pycache__",
        ]
        .contains(&n)
    {
        return named("build", "build_output", format!("{raw}/"));
    }
    if !is_dir && ext == "pyc" {
        return named("build", "build_output", format!("*.{raw_ext}"));
    }
    if (is_dir
        && [
            ".cache",
            ".parcel-cache",
            ".pytest_cache",
            ".mypy_cache",
            ".ruff_cache",
            ".sass-cache",
        ]
        .contains(&n))
        || (!is_dir && n == ".eslintcache")
    {
        return named(
            "build",
            "cache_dir",
            format!("{raw}{}", if is_dir { "/" } else { "" }),
        );
    }
    if tests {
        return None;
    }
    if !is_dir && ext == "log" {
        return named("scratch", "log_file", format!("*.{raw_ext}"));
    }
    if !is_dir && ["tmp", "temp", "bak", "old", "orig"].contains(&ext) {
        return named("scratch", "temp_file", format!("*.{raw_ext}"));
    }
    if is_dir && ["tmp", "temp"].contains(&n) {
        return named("scratch", "temp_file", format!("{raw}/"));
    }
    let stem = n.rsplit_once('.').map(|(s, _)| s).unwrap_or(n);
    if !is_dir
        && (([
            "test", "tmp", "temp", "scratch", "aaa", "hoge", "fuga", "piyo",
        ]
        .contains(&stem)
            && (!n.contains('.') || ["txt", "md"].contains(&ext)))
            || ["untitled", "無題", "新規"]
                .iter()
                .any(|s| n.starts_with(s)))
    {
        return root("scratch", "scratch_file");
    }
    None
}
struct ContentRules {
    secret: Regex,
    email: Regex,
    phone: Regex,
}
fn rules() -> &'static ContentRules {
    static RULES: OnceLock<ContentRules> = OnceLock::new();
    RULES.get_or_init(|| ContentRules {
        secret: Regex::new(r"-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----|\bAKIA[A-Z0-9]{16}\b|\bgh[pousr]_[A-Za-z0-9]{36}\b|\bgithub_pat_[A-Za-z0-9]{22}_[A-Za-z0-9]{59}\b|\bxox[baprs]-[A-Za-z0-9-]{10,}\b|\bAIza[A-Za-z0-9_-]{35}\b|\bsk_live_[A-Za-z0-9]{24,}\b").unwrap(),
        email: Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").unwrap(),
        phone: Regex::new(r"(?:\+?[0-9]{1,3}[- .]?)?\(?[0-9]{2,4}\)?[- .][0-9]{2,4}[- .][0-9]{3,4}\b").unwrap(),
    })
}
pub fn classify_content(relative: &str, content: &[u8]) -> Option<Classification> {
    if test_code(relative) || content.len() > 256 * 1024 || content.contains(&0) {
        return None;
    }
    let text = String::from_utf8_lossy(content);
    let r = rules();
    if r.secret.is_match(&text) {
        return classified(relative, false, "secret", "secret_in_content", None);
    }
    let ext = relative.rsplit('.').next()?.to_lowercase();
    if [
        "csv", "tsv", "json", "ndjson", "sql", "txt", "xml", "yaml", "yml",
    ]
    .contains(&ext.as_str())
        && (r
            .email
            .find_iter(&text)
            .map(|m| m.as_str().to_lowercase())
            .collect::<HashSet<_>>()
            .len()
            >= 20
            || r.phone.find_iter(&text).take(20).count() >= 20)
    {
        return classified(relative, false, "personal", "personal_in_content", None);
    }
    None
}

pub(crate) struct Scratch(pub PathBuf);
impl Scratch {
    pub fn outside(root: &Path) -> Result<Self> {
        let mut bases = vec![std::env::temp_dir()];
        #[cfg(unix)]
        bases.extend([PathBuf::from("/tmp"), PathBuf::from("/var/tmp")]);
        #[cfg(windows)]
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            bases.push(PathBuf::from(local).join("Temp"));
        }
        for base in bases {
            let Ok(base) = crate::platform::canonicalize(base) else {
                continue;
            };
            if base.starts_with(root) {
                continue;
            }
            let path = base.join(format!("revia-ignore-{}", uuid::Uuid::new_v4()));
            crate::settings::private_dir(&path)?;
            return Ok(Self(path));
        }
        Err(AppError::new("invalid_path")
            .detail("No temporary directory is available outside the project"))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl Core {
    pub async fn scan_ignore_candidates(&self, project_id: &str) -> Result<Value> {
        let project = self.project(project_id)?;
        let root = crate::registry::directory(
            project["path"]
                .as_str()
                .ok_or_else(|| AppError::new("invalid_path"))?,
        )?;
        scan(&self.options.git, &root, project_id).await
    }
}
pub async fn scan(g: &Git, root: &Path, project_id: &str) -> Result<Value> {
    if !root.is_dir() {
        return Err(AppError::new("path_not_found"));
    }
    let root = &crate::platform::canonicalize(root)?;
    if std::fs::symlink_metadata(root.join(".git")).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(AppError::new("invalid_path"));
    }
    let start = Instant::now();
    let scratch = if root.join(".git").exists() {
        None
    } else {
        Some(Scratch::outside(root)?)
    };
    let mut prefix = vec![];
    if let Some(s) = &scratch {
        match tokio::time::timeout(Duration::from_secs(3), g.local(&s.0, &["init", "--bare"])).await
        {
            Ok(r) => {
                r?;
            }
            Err(_) => return Ok(json!({"projectId":project_id,"suggestions":[],"truncated":true})),
        }
        prefix = vec![
            format!("--git-dir={}", s.0.display()),
            format!("--work-tree={}", root.display()),
        ];
    }
    let mut pending = vec![String::new()];
    let mut grouped: BTreeMap<(usize, String), Value> = BTreeMap::new();
    let mut entries = 0;
    let mut visited = 0;
    let mut content_files = 0;
    let mut content_bytes = 0;
    let mut truncated = false;
    let mut seen = HashSet::new();
    while let Some(dir) = pending.pop() {
        let Some(_) = Duration::from_secs(3).checked_sub(start.elapsed()) else {
            truncated = true;
            break;
        };
        let full_dir = if dir.is_empty() {
            root.to_path_buf()
        } else {
            match git::relative(root, dir.trim_end_matches('/')) {
                Ok(p) => p,
                Err(_) => continue,
            }
        };
        let Ok(children) = std::fs::read_dir(&full_dir) else {
            continue;
        };
        let mut paths = Vec::new();
        for child in children.flatten() {
            if visited >= 20_000 || start.elapsed() >= Duration::from_secs(3) {
                truncated = true;
                break;
            }
            let Some(name) = child.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if name == ".git" {
                continue;
            }
            visited += 1;
            paths.push(format!("{dir}{name}"));
        }
        paths.sort();
        let mut output = String::new();
        let mut query_truncated = false;
        let mut offset = 0;
        while offset < paths.len() {
            let begin = offset;
            let mut units = 0;
            while offset < paths.len() && offset - begin < 64 {
                let next = paths[offset].encode_utf16().count() + 3;
                if offset > begin && units + next > 8192 {
                    break;
                }
                units += next;
                offset += 1;
            }
            let batch = &paths[begin..offset];
            let mut args = prefix.clone();
            args.extend(
                [
                    "ls-files",
                    "--others",
                    "--exclude-standard",
                    "--directory",
                    "-z",
                    "--",
                ]
                .map(str::to_owned),
            );
            args.extend(batch.iter().cloned());
            let Some(left) = Duration::from_secs(3).checked_sub(start.elapsed()) else {
                truncated = true;
                break;
            };
            match tokio::time::timeout(
                left,
                g.scan_entries(
                    root,
                    &args,
                    20_000 - entries - output.bytes().filter(|b| *b == 0).count(),
                ),
            )
            .await
            {
                Ok(r) => {
                    let (text, limited) = r?;
                    output.push_str(&text);
                    if limited {
                        query_truncated = true;
                        break;
                    }
                }
                Err(_) => {
                    truncated = true;
                    break;
                }
            }
        }
        for path in output.split('\0').filter(|p| !p.is_empty()) {
            if entries >= 20_000 || start.elapsed() >= Duration::from_secs(3) {
                truncated = true;
                break;
            }
            entries += 1;
            let mut path = path.to_owned();
            let parts = path.trim_end_matches('/').split('/').collect::<Vec<_>>();
            for i in 1..parts.len() {
                let ancestor = parts[..i].join("/");
                if classify_path(&ancestor, true)
                    .is_some_and(|c| matches!(c.category, "dependency" | "build"))
                {
                    path = format!("{ancestor}/");
                    break;
                }
            }
            if !seen.insert(path.clone()) {
                continue;
            }
            let is_dir = path.ends_with('/');
            let relative = path.trim_end_matches('/');
            if relative.split('/').count() > 12 {
                truncated = true;
                continue;
            }
            let Ok(full) = git::relative(root, relative) else {
                continue;
            };
            let Ok(meta) = std::fs::symlink_metadata(&full) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            let mut class = classify_path(relative, is_dir);
            if !is_dir && meta.is_file() && meta.len() <= 256 * 1024 {
                if content_files < 2000 && content_bytes + meta.len() <= 32 * 1024 * 1024 {
                    content_files += 1;
                    let remaining = 32 * 1024 * 1024 - content_bytes;
                    let mut bytes = vec![];
                    if let Ok(file) = open_regular(&full, false)
                        && file
                            .take(remaining.min(256 * 1024 + 1))
                            .read_to_end(&mut bytes)
                            .is_ok()
                    {
                        content_bytes += bytes.len() as u64;
                        if bytes.len() as u64 == remaining {
                            truncated = true;
                        }
                        let content = classify_content(relative, &bytes);
                        if let Some(c) = content
                            && (class.is_none()
                                || priority(c.reason) < priority(class.as_ref().unwrap().reason))
                        {
                            class = Some(c);
                        }
                    }
                } else {
                    truncated = true;
                }
            }
            if is_dir
                && !class
                    .as_ref()
                    .is_some_and(|c| matches!(c.category, "dependency" | "build"))
            {
                if relative.split('/').count() >= 12 {
                    truncated = true;
                } else {
                    pending.push(path.clone());
                }
            }
            if let Some(c) = class {
                let rank = [
                    "secret",
                    "personal",
                    "system",
                    "dependency",
                    "build",
                    "scratch",
                ]
                .iter()
                .position(|s| *s == c.category)
                .unwrap();
                let entry = grouped.entry((rank, c.pattern.clone())).or_insert_with(|| json!({"pattern":c.pattern,"category":c.category,"reason":c.reason,"matches":0,"examples":[]}));
                entry["matches"] = json!(entry["matches"].as_u64().unwrap() + 1);
                let examples = entry["examples"].as_array_mut().unwrap();
                if examples.len() < 5 {
                    examples.push(json!(path));
                }
            }
        }
        if entries >= 20_000 || visited >= 20_000 || query_truncated {
            truncated = true;
            break;
        }
    }
    Ok(
        json!({"projectId":project_id,"suggestions":grouped.into_values().collect::<Vec<_>>(),"truncated":truncated}),
    )
}
fn priority(reason: &str) -> usize {
    [
        "env_file",
        "key_file",
        "credential_file",
        "secret_in_content",
        "personal_data_file",
        "personal_in_content",
        "database_file",
        "os_file",
        "editor_file",
        "dependency_dir",
        "build_output",
        "cache_dir",
        "log_file",
        "temp_file",
        "scratch_file",
    ]
    .iter()
    .position(|s| *s == reason)
    .unwrap()
}

pub(crate) fn patterns(input: &Value) -> Result<Vec<String>> {
    if input.is_null() {
        return Ok(vec![]);
    }
    let values = input
        .as_array()
        .ok_or_else(|| AppError::new("invalid_input"))?;
    if values.len() > 500 {
        return Err(AppError::new("invalid_input"));
    }
    let mut out = vec![];
    for v in values {
        let p = v.as_str().ok_or_else(|| AppError::new("invalid_input"))?;
        if p.is_empty()
            || p.chars().count() > 300
            || p.chars().any(char::is_control)
            || p.trim_start() != p
            || (p.trim_end() != p && !p.ends_with("\\ "))
            || p.starts_with(['!', '#'])
            || p.split(['/', '\\']).any(|s| s == "..")
        {
            return Err(AppError::new("invalid_input"));
        }
        if !out.iter().any(|s| s == p) {
            out.push(p.to_owned());
        }
    }
    Ok(out)
}
pub(crate) fn additions(root: &Path, patterns: &[String]) -> Result<(Vec<u8>, Vec<String>)> {
    if patterns.is_empty() {
        return Ok((vec![], vec![]));
    }
    let path = root.join(".gitignore");
    let bytes = match std::fs::symlink_metadata(&path) {
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => {
            return Err(AppError::new("invalid_path"));
        }
        Ok(_) => {
            let mut bytes = vec![];
            open_regular(&path, false)?.read_to_end(&mut bytes)?;
            bytes
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(e.into()),
    };
    let lines = bytes
        .split(|b| *b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
        .collect::<Vec<_>>();
    let new = patterns
        .iter()
        .filter(|p| !lines.contains(&p.as_bytes()))
        .cloned()
        .collect();
    Ok((bytes, new))
}
pub(crate) fn append(root: &Path, patterns: &[String]) -> Result<()> {
    use std::io::Write;
    let (bytes, new) = additions(root, patterns)?;
    if new.is_empty() {
        return Ok(());
    }
    let path = root.join(".gitignore");
    let mut file = open_regular(&path, true).map_err(|_| AppError::new("invalid_path"))?;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        file.write_all(b"\n")?;
    }
    file.write_all(b"# Added by Revia\n")?;
    for p in new {
        writeln!(file, "{p}")?;
    }
    Ok(())
}

pub(crate) fn open_regular(path: &Path, append: bool) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    if append {
        options.append(true);
        if !path.exists() {
            options.create_new(true);
        }
    } else {
        options.read(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(if cfg!(target_os = "macos") {
            0x100 | 0x4
        } else {
            0x20000 | 0x800
        });
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::other("Expected a regular file"));
    }
    Ok(file)
}
