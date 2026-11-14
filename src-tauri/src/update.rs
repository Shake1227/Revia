use crate::{
    core::Core,
    error::{AppError, Result},
};
use reqwest::{Client, Url};
use semver::Version;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::watch,
};

const LIMIT: u64 = 1_000_000_000;
#[derive(Clone)]
pub struct Config {
    pub api_base_url: String,
    pub release_prefix: String,
    pub download_prefix: String,
    pub download_hosts: Vec<String>,
    pub allow_http: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            api_base_url: "https://api.github.com".into(),
            release_prefix: "https://github.com/Shake1227/Revia/releases/".into(),
            download_prefix: "https://github.com/Shake1227/Revia/releases/download/".into(),
            download_hosts: [
                "github.com",
                "objects.githubusercontent.com",
                "release-assets.githubusercontent.com",
            ]
            .map(str::to_owned)
            .to_vec(),
            allow_http: false,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Target {
    pub os: String,
    pub cpu: String,
    pub linux_format: String,
}
impl Target {
    pub fn new(os: &str, cpu: &str, linux_format: &str) -> Self {
        Self {
            os: os.into(),
            cpu: cpu.into(),
            linux_format: linux_format.into(),
        }
    }
    pub fn current() -> Self {
        let format = if std::env::var_os("APPIMAGE").is_some() {
            "appimage"
        } else if tool_available("dpkg") {
            "deb"
        } else if tool_available("rpm") {
            "rpm"
        } else {
            "appimage"
        };
        Self::new(std::env::consts::OS, std::env::consts::ARCH, format)
    }
}
fn tool_available(name: &str) -> bool {
    let path = crate::git::find_tool(name);
    path.is_absolute() && crate::git::executable(&path)
}
#[derive(Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
    pub digest: Option<String>,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<Asset>,
}
#[derive(Default)]
pub(crate) struct UpdateState {
    asset: Option<Asset>,
    downloaded: Option<PathBuf>,
    cancel: Option<watch::Sender<bool>>,
}
pub fn valid_filename(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 120
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
}
pub fn valid_digest(digest: &str) -> bool {
    digest
        .strip_prefix("sha256:")
        .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}
pub(crate) fn digest(asset: &Asset) -> Option<&str> {
    asset
        .digest
        .as_deref()
        .filter(|d| valid_digest(d))
        .map(|d| &d[7..])
}
fn kind(asset: &Asset, target: &Target) -> Option<(&'static str, u8)> {
    let name = asset.name.to_ascii_lowercase();
    let arm = matches!(target.cpu.as_str(), "aarch64" | "arm64");
    let x64 = matches!(target.cpu.as_str(), "x86_64" | "x64" | "amd64");
    let cpu = if arm {
        name.contains("aarch64") || name.contains("arm64")
    } else if x64 {
        name.contains("x86_64")
            || (target.os == "macos" && name.contains("x64"))
            || (target.os == "linux" && name.contains("amd64"))
    } else {
        false
    };
    match target.os.as_str() {
        "macos" if name.ends_with(".dmg") && name.contains("universal") => Some(("dmg", 0)),
        "macos" if name.ends_with(".dmg") && cpu => Some(("dmg", 1)),
        "windows"
            if name.ends_with("-setup.exe")
                && ((arm && name.contains("arm64")) || (x64 && name.contains("x64"))) =>
        {
            Some(("nsis", 0))
        }
        "linux" if cpu => match target.linux_format.as_str() {
            "deb" if name.ends_with(".deb") => Some(("deb", 0)),
            "rpm" if name.ends_with(".rpm") => Some(("rpm", 0)),
            "appimage" if name.ends_with(".appimage") => Some(("appimage", 0)),
            _ => None,
        },
        _ => None,
    }
}
fn select(release: &Release, target: &Target, config: &Config) -> Option<Asset> {
    release
        .assets
        .iter()
        .filter(|a| {
            valid_filename(&a.name) && a.browser_download_url.starts_with(&config.download_prefix)
        })
        .filter_map(|a| kind(a, target).map(|(_, rank)| (a, rank)))
        .min_by_key(|(_, rank)| *rank)
        .map(|(a, _)| a.clone())
}
fn timestamp(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return None;
    }
    let n = |a, z| std::str::from_utf8(&b[a..z]).ok()?.parse::<i64>().ok();
    let (mut y, m, d, h, min, sec) = (
        n(0, 4)?,
        n(5, 7)?,
        n(8, 10)?,
        n(11, 13)?,
        n(14, 16)?,
        n(17, 19)?,
    );
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&m)
        || !(1..=days[(m - 1) as usize]).contains(&d)
        || !(0..24).contains(&h)
        || !(0..60).contains(&min)
        || !(0..60).contains(&sec)
    {
        return None;
    }
    y -= i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + (153 * mp + 2) / 5 + d - 1 - 719468;
    u64::try_from(((days * 24 + h) * 60 + min) * 60_000 + sec * 1000).ok()
}
fn parse_info(
    current: &str,
    input: &str,
    target: &Target,
    now: u64,
    config: &Config,
) -> Result<(Value, Option<Asset>)> {
    let release: Release = serde_json::from_str(input)
        .map_err(|_| AppError::new("unknown").detail("GitHub API returned invalid release JSON"))?;
    let mut info =
        json!({"currentVersion":current,"checkedAt":now,"latest":null,"available":false});
    let version = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    let Ok(version_parsed) = Version::parse(version) else {
        return Ok((info, None));
    };
    if release.draft || release.prerelease {
        return Ok((info, None));
    }
    let asset = select(&release, target, config);
    let url = if release.html_url.starts_with(&config.release_prefix) {
        release.html_url.clone()
    } else {
        format!("{}tag/{}", config.release_prefix, release.tag_name)
    };
    info["available"] =
        json!(Version::parse(current).is_ok_and(|v| version_parsed.cmp_precedence(&v).is_gt()));
    info["latest"] = json!({"version":version,"name":release.name.filter(|s| !s.is_empty()),
        "notes":release.body.filter(|s| !s.is_empty()).map(|s| s.chars().take(20_000).collect::<String>()),
        "publishedAt":release.published_at.as_deref().and_then(timestamp),"url":url,
        "asset":asset.as_ref().map(|a| json!({"name":a.name,"size":a.size,"kind":kind(a,target).unwrap().0,"hasDigest":digest(a).is_some()}))});
    Ok((info, asset))
}
pub fn info_from_release(
    current_version: &str,
    release_json: &str,
    target: &Target,
    now: u64,
) -> Result<Value> {
    parse_info(
        current_version,
        release_json,
        target,
        now,
        &Config::default(),
    )
    .map(|r| r.0)
}
pub(crate) fn client() -> Result<Client> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Revia/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| AppError::new("network_unreachable"))
}
pub(crate) fn network(e: reqwest::Error) -> AppError {
    AppError::new(if e.is_timeout() {
        "timeout"
    } else {
        "network_unreachable"
    })
}
pub(crate) fn io(e: std::io::Error) -> AppError {
    AppError::new(if e.kind() == std::io::ErrorKind::StorageFull {
        "disk_full"
    } else {
        "io_error"
    })
}
fn validate_url(url: &Url, config: &Config) -> Result<()> {
    if (url.scheme() != "https" && !(config.allow_http && url.scheme() == "http"))
        || !url.username().is_empty()
        || url.password().is_some()
        || !config
            .download_hosts
            .iter()
            .any(|h| Some(h.as_str()) == url.host_str())
    {
        return Err(AppError::new("invalid_input"));
    }
    Ok(())
}
async fn regular(path: &Path) -> Result<bool> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(m) if m.is_file() && !m.is_symlink() => Ok(true),
        Ok(_) => Err(AppError::new("invalid_path")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io(e)),
    }
}
async fn hash_file(path: &Path) -> Result<(u64, String)> {
    let mut file = tokio::fs::File::open(path).await.map_err(io)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0; 64 * 1024];
    let mut size = 0;
    loop {
        let n = file.read(&mut buf).await.map_err(io)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > LIMIT {
            return Err(AppError::new("verification_failed"));
        }
        hash.update(&buf[..n]);
    }
    Ok((size, format!("{:x}", hash.finalize())))
}
pub(crate) async fn fetch(
    asset: &Asset,
    config: &Config,
    part: &Path,
    cancel: &mut watch::Receiver<bool>,
    mut progress: impl FnMut(u64),
) -> Result<()> {
    let client = client()?;
    let mut url =
        Url::parse(&asset.browser_download_url).map_err(|_| AppError::new("invalid_input"))?;
    let mut redirects = 0;
    let mut response = loop {
        validate_url(&url, config)?;
        let response = tokio::select! {
            biased;
            _ = cancel.changed() => return Err(AppError::new("cancelled")),
            result = tokio::time::timeout(Duration::from_secs(60), client.get(url.clone()).send()) =>
                result.map_err(|_| AppError::new("timeout"))?.map_err(network)?,
        };
        if response.status().is_redirection() {
            if redirects == 5 {
                return Err(AppError::new("invalid_input"));
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| AppError::new("invalid_input"))?;
            url = url
                .join(location)
                .map_err(|_| AppError::new("invalid_input"))?;
            redirects += 1;
        } else {
            break response;
        }
    };
    if !response.status().is_success() {
        return Err(AppError::new("unknown").detail(&format!(
            "Download returned HTTP {}",
            response.status().as_u16()
        )));
    }
    if regular(part).await? {
        tokio::fs::remove_file(part).await.map_err(io)?;
    }
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(part).await.map_err(io)?;
    let mut received = 0;
    let mut hash = Sha256::new();
    let mut last = Instant::now();
    let mut last_received = Instant::now();
    loop {
        let chunk = tokio::select! {
            biased;
            _ = cancel.changed() => return Err(AppError::new("cancelled")),
            result = tokio::time::timeout(Duration::from_secs(60).saturating_sub(last_received.elapsed()), response.chunk()) =>
                result.map_err(|_| AppError::new("timeout"))?.map_err(network)?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        if chunk.is_empty() {
            continue;
        }
        last_received = Instant::now();
        received += chunk.len() as u64;
        if received > LIMIT {
            return Err(AppError::new("verification_failed"));
        }
        hash.update(&chunk);
        file.write_all(&chunk).await.map_err(io)?;
        if last.elapsed() >= Duration::from_millis(150) {
            progress(received);
            last = Instant::now();
        }
    }
    if received != asset.size
        || digest(asset).is_some_and(|d| !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(d))
    {
        return Err(AppError::new("verification_failed"));
    }
    file.flush().await.map_err(io)?;
    Ok(())
}
impl Core {
    pub async fn check_update(&self) -> Result<Value> {
        self.check_update_with(&Config::default(), &Target::current())
            .await
    }
    pub async fn check_update_with(&self, config: &Config, target: &Target) -> Result<Value> {
        let response = client()?
            .get(format!(
                "{}/repos/Shake1227/Revia/releases/latest",
                config.api_base_url.trim_end_matches('/')
            ))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(network)?;
        let status = response.status().as_u16();
        let (info, asset) = match status {
            404 => (
                json!({"currentVersion":env!("CARGO_PKG_VERSION"),"checkedAt":crate::now(),"latest":null,"available":false}),
                None,
            ),
            403 | 429 => return Err(AppError::new("rate_limited")),
            200 => parse_info(
                env!("CARGO_PKG_VERSION"),
                &response.text().await.map_err(network)?,
                target,
                crate::now(),
                config,
            )?,
            _ => {
                return Err(
                    AppError::new("unknown").detail(&format!("GitHub API returned HTTP {status}"))
                );
            }
        };
        self.updates.lock().unwrap().asset = asset;
        Ok(info)
    }
    pub fn cancel_update_download(&self) {
        if let Some(cancel) = &self.updates.lock().unwrap().cancel {
            let _ = cancel.send(true);
        }
    }
    pub async fn download_update(&self) -> Result<Value> {
        self.download_update_with(&Config::default()).await
    }
    pub async fn download_update_with(&self, config: &Config) -> Result<Value> {
        let (asset, mut cancel) = {
            let mut state = self.updates.lock().unwrap();
            if state.cancel.is_some() {
                return Err(AppError::new("busy"));
            }
            let asset = state
                .asset
                .clone()
                .ok_or_else(|| AppError::new("invalid_input"))?;
            let (tx, rx) = watch::channel(false);
            state.cancel = Some(tx);
            (asset, rx)
        };
        let dir = self.options.cache_dir.join("updates");
        let part = dir.join(format!("{}.part", asset.name));
        let result = self
            .download_inner(&asset, config, &dir, &part, &mut cancel)
            .await;
        if result.is_err()
            && tokio::fs::symlink_metadata(&dir)
                .await
                .is_ok_and(|m| m.is_dir() && !m.is_symlink())
        {
            let _ = tokio::fs::remove_file(&part).await;
        }
        let mut state = self.updates.lock().unwrap();
        if let Ok(value) = &result {
            state.downloaded = value["path"].as_str().map(PathBuf::from);
        }
        state.cancel = None;
        result
    }
    async fn download_inner(
        &self,
        asset: &Asset,
        config: &Config,
        dir: &Path,
        part: &Path,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<Value> {
        if !valid_filename(&asset.name) {
            return Err(AppError::new("invalid_input"));
        }
        if asset.size > LIMIT {
            return Err(AppError::new("verification_failed"));
        }
        crate::settings::private_dir(&self.options.cache_dir)?;
        match tokio::fs::symlink_metadata(dir).await {
            Ok(m) if !m.is_dir() || m.is_symlink() => return Err(AppError::new("invalid_path")),
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                crate::settings::private_dir(dir)?
            }
            Err(e) => return Err(io(e)),
        }
        let dir = tokio::fs::canonicalize(dir).await.map_err(io)?;
        let path = dir.join(&asset.name);
        let exists = regular(&path).await?;
        regular(part).await?;
        if *cancel.borrow() {
            return Err(AppError::new("cancelled"));
        }
        let reuse = if exists
            && tokio::fs::metadata(&path).await.map_err(io)?.len() == asset.size
            && let Some(expected) = digest(asset)
        {
            let (size, actual) = hash_file(&path).await?;
            size == asset.size && actual.eq_ignore_ascii_case(expected)
        } else {
            false
        };
        if !reuse {
            fetch(asset, config, part, cancel, |received| {
                self.options.events.emit(
                    "revia:update-progress",
                    json!({"received":received,"total":asset.size}),
                );
            })
            .await?;
            if exists {
                tokio::fs::remove_file(&path).await.map_err(io)?;
            }
            tokio::fs::rename(part, &path).await.map_err(io)?;
        }
        if *cancel.borrow() {
            return Err(AppError::new("cancelled"));
        }
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(io)?;
        while let Some(entry) = entries.next_entry().await.map_err(io)? {
            if entry.path() != path && !entry.file_type().await.map_err(io)?.is_dir() {
                tokio::fs::remove_file(entry.path()).await.map_err(io)?;
            }
        }
        self.options.events.emit(
            "revia:update-progress",
            json!({"received":asset.size,"total":asset.size}),
        );
        Ok(
            json!({"name":asset.name,"size":asset.size,"path":path.to_string_lossy(),"verified":digest(asset).is_some()}),
        )
    }
    pub async fn install_update(&self, app: &tauri::AppHandle) -> Result<()> {
        let path = self
            .updates
            .lock()
            .unwrap()
            .downloaded
            .clone()
            .ok_or_else(|| AppError::new("invalid_input"))?;
        let parent = path.parent().ok_or_else(|| AppError::new("invalid_path"))?;
        let metadata = tokio::fs::symlink_metadata(parent).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::new("path_not_found")
            } else {
                io(e)
            }
        })?;
        if metadata.is_symlink() || !metadata.is_dir() {
            return Err(AppError::new("invalid_path"));
        }
        if !regular(&path).await? {
            return Err(AppError::new("path_not_found"));
        }
        #[cfg(target_os = "macos")]
        {
            use tauri_plugin_opener::OpenerExt;
            app.opener()
                .open_path(path.to_string_lossy(), None::<&str>)
                .map_err(|_| AppError::new("io_error"))?;
        }
        #[cfg(windows)]
        {
            let _ = app;
            std::process::Command::new(&path).spawn().map_err(io)?;
        }
        #[cfg(target_os = "linux")]
        {
            use tauri_plugin_opener::OpenerExt;
            app.opener()
                .reveal_item_in_dir(&path)
                .map_err(|_| AppError::new("io_error"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    #[derive(Default)]
    struct Events(Mutex<Vec<Value>>);
    impl crate::EventSink for Events {
        fn emit(&self, name: &str, payload: Value) {
            assert_eq!(name, "revia:update-progress");
            self.0.lock().unwrap().push(payload);
        }
    }
    fn fixture() -> (tempfile::TempDir, Arc<Core>, Arc<Events>) {
        let temp = tempfile::tempdir().unwrap();
        let events = Arc::new(Events::default());
        let core = Core::new(crate::core::Options {
            oauth: None,
            trash: Arc::new(crate::trash::MoveTo(temp.path().join("trash"))),
            config_dir: temp.path().join("config"),
            data_dir: temp.path().join("data"),
            cache_dir: temp.path().join("cache"),
            git: crate::git::Git::default(),
            events: events.clone(),
        })
        .unwrap();
        (temp, core, events)
    }
    fn asset() -> Asset {
        Asset {
            name: "Revia_0.2.0_universal.dmg".into(),
            size: 3,
            browser_download_url:
                "https://github.com/Shake1227/Revia/releases/download/v0.2.0/installer".into(),
            digest: Some(format!("sha256:{:x}", Sha256::digest(b"abc"))),
        }
    }
    #[tokio::test]
    async fn reuse_verified_file_without_network() {
        let (_temp, core, events) = fixture();
        assert_eq!(
            core.download_update().await.unwrap_err().code,
            "invalid_input"
        );
        let dir = core.options.cache_dir.join("updates");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let a = asset();
        tokio::fs::write(dir.join(&a.name), b"abc").await.unwrap();
        tokio::fs::write(dir.join("old.part"), b"partial")
            .await
            .unwrap();
        core.updates.lock().unwrap().asset = Some(a);
        let result = core.download_update().await.unwrap();
        assert_eq!(result["verified"], true);
        assert_eq!(result["size"], 3);
        assert!(Path::new(result["path"].as_str().unwrap()).is_absolute());
        assert_eq!(
            tokio::fs::read_dir(&dir)
                .await
                .unwrap()
                .next_entry()
                .await
                .unwrap()
                .unwrap()
                .file_name(),
            "Revia_0.2.0_universal.dmg"
        );
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        assert_eq!(
            *events.0.lock().unwrap(),
            vec![json!({"received":3,"total":3})]
        );
        assert!(core.updates.lock().unwrap().downloaded.is_some());
        core.cancel_update_download();
        assert_eq!(core.download_update().await.unwrap(), result);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn update_directory_and_files_reject_symlinks() {
        let (temp, core, _) = fixture();
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(&core.options.cache_dir).unwrap();
        let a = asset();
        let outside_part = outside.join(format!("{}.part", a.name));
        std::fs::write(&outside_part, b"keep").unwrap();
        let dir = core.options.cache_dir.join("updates");
        std::os::unix::fs::symlink(&outside, &dir).unwrap();
        core.updates.lock().unwrap().asset = Some(a.clone());
        assert_eq!(
            core.download_update().await.unwrap_err().code,
            "invalid_path"
        );
        assert_eq!(std::fs::read(&outside_part).unwrap(), b"keep");
        std::fs::remove_file(&dir).unwrap();
        std::fs::create_dir(&dir).unwrap();
        std::os::unix::fs::symlink(&outside_part, dir.join(&a.name)).unwrap();
        assert_eq!(
            core.download_update().await.unwrap_err().code,
            "invalid_path"
        );
        assert_eq!(std::fs::read(&outside_part).unwrap(), b"keep");
    }
    #[test]
    fn redirect_url_policy_and_timestamps() {
        let config = Config::default();
        for url in [
            "https://github.com/a",
            "https://objects.githubusercontent.com/a",
            "https://release-assets.githubusercontent.com/a",
        ] {
            assert!(validate_url(&Url::parse(url).unwrap(), &config).is_ok());
        }
        for url in [
            "http://github.com/a",
            "https://evil.example/a",
            "https://github.com.evil.example/a",
            "https://user@github.com/a",
        ] {
            assert_eq!(
                validate_url(&Url::parse(url).unwrap(), &config)
                    .unwrap_err()
                    .code,
                "invalid_input"
            );
        }
        assert_eq!(timestamp("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(timestamp("2026-02-29T00:00:00Z"), None);
        assert_eq!(timestamp("2024-02-29T00:00:00Z"), Some(1709164800000));
    }
}
