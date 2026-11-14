use crate::{
    core::Core,
    error::{AppError, Result},
    settings::private_dir,
    update::{self, Asset, Config},
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::sync::watch;

const LIMIT: u64 = 200_000_000;
const DOWNLOAD_PREFIX: &str = "https://github.com/cli/cli/releases/download/";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

fn binary_name() -> &'static str {
    if cfg!(windows) { "gh.exe" } else { "gh" }
}
pub fn managed_gh(data_dir: &Path) -> PathBuf {
    data_dir
        .join("tools")
        .join("gh")
        .join("bin")
        .join(binary_name())
}
pub fn asset_name(tag: &str, os: &str, cpu: &str) -> Option<String> {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    if version.is_empty()
        || version.len() > 32
        || !version.bytes().all(|b| b.is_ascii_digit() || b == b'.')
    {
        return None;
    }
    let cpu = match cpu {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "x86" => "386",
        "arm" => "armv6",
        _ => return None,
    };
    match (os, cpu) {
        ("macos", "amd64" | "arm64") => Some(format!("gh_{version}_macOS_{cpu}.zip")),
        ("windows", "amd64" | "arm64" | "386") => Some(format!("gh_{version}_windows_{cpu}.zip")),
        ("linux", _) => Some(format!("gh_{version}_linux_{cpu}.tar.gz")),
        _ => None,
    }
}
fn unavailable() -> AppError {
    AppError::new("unknown").detail("No GitHub CLI build is published for this system")
}
fn archiver() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let path = PathBuf::from(std::env::var_os("SystemRoot")?)
            .join("System32")
            .join("tar.exe");
        path.is_file().then_some(path)
    }
    #[cfg(not(windows))]
    {
        ["/usr/bin/tar", "/bin/tar"]
            .into_iter()
            .map(PathBuf::from)
            .find(|p| crate::git::executable(p))
            .or_else(|| {
                let path = crate::git::find_tool("tar");
                (path.is_absolute() && crate::git::executable(&path)).then_some(path)
            })
    }
}
pub fn find_binary(root: &Path, depth: u8) -> Option<PathBuf> {
    let direct = root.join("bin").join(binary_name());
    if std::fs::symlink_metadata(&direct).is_ok_and(|m| m.is_file()) {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }
    std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .find_map(|e| find_binary(&e.path(), depth - 1))
}
pub async fn unpack(archive: &Path, out: &Path) -> Result<PathBuf> {
    let archiver =
        archiver().ok_or_else(|| AppError::new("unknown").detail("tar was not found"))?;
    let output = tokio::time::timeout(
        Duration::from_secs(120),
        crate::platform::command(&archiver)
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(out)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| AppError::new("timeout"))?
    .map_err(update::io)?;
    if !output.status.success() {
        return Err(AppError::new("unknown").detail(&format!(
            "Could not unpack the GitHub CLI archive\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    find_binary(out, 2).ok_or_else(|| AppError::new("verification_failed"))
}
async fn starts(binary: &Path) -> bool {
    tokio::time::timeout(
        Duration::from_secs(20),
        crate::platform::command(binary)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()
    .and_then(|r| r.ok())
    .is_some_and(|o| o.status.success() && o.stdout.starts_with(b"gh version "))
}
async fn latest(config: &Config, os: &str, cpu: &str) -> Result<Asset> {
    let response = update::client()?
        .get(format!(
            "{}/repos/cli/cli/releases/latest",
            config.api_base_url.trim_end_matches('/')
        ))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(update::network)?;
    match response.status().as_u16() {
        200 => (),
        403 | 429 => return Err(AppError::new("rate_limited")),
        status => {
            return Err(
                AppError::new("unknown").detail(&format!("GitHub API returned HTTP {status}"))
            );
        }
    }
    let release: Release = serde_json::from_str(&response.text().await.map_err(update::network)?)
        .map_err(|_| {
        AppError::new("unknown").detail("GitHub API returned an unexpected body")
    })?;
    let name = asset_name(&release.tag_name, os, cpu).ok_or_else(unavailable)?;
    let asset = release
        .assets
        .into_iter()
        .find(|a| a.name == name)
        .ok_or_else(unavailable)?;
    if asset.size == 0
        || asset.size > LIMIT
        || update::digest(&asset).is_none()
        || (!config.allow_http && !asset.browser_download_url.starts_with(DOWNLOAD_PREFIX))
    {
        return Err(AppError::new("verification_failed"));
    }
    Ok(asset)
}
async fn install(asset: &Asset, config: &Config, work: &Path, target: &Path) -> Result<()> {
    let archive = work.join(&asset.name);
    let (_keep, mut cancel) = watch::channel(false);
    update::fetch(asset, config, &archive, &mut cancel, |_| ()).await?;
    let out = work.join("out");
    private_dir(&out)?;
    let binary = unpack(&archive, &out).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
            .await
            .map_err(update::io)?;
    }
    if !starts(&binary).await {
        return Err(
            AppError::new("verification_failed").detail("The downloaded GitHub CLI did not start")
        );
    }
    let bin = target
        .parent()
        .ok_or_else(|| AppError::new("invalid_path"))?;
    private_dir(bin)?;
    match tokio::fs::symlink_metadata(target).await {
        Ok(m) if m.is_dir() => return Err(AppError::new("invalid_path")),
        Ok(_) => tokio::fs::remove_file(target).await.map_err(update::io)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(update::io(e)),
    }
    tokio::fs::rename(&binary, target).await.map_err(update::io)
}
impl Core {
    pub async fn install_github_cli(&self) -> Result<Value> {
        self.install_github_cli_with(
            &Config::default(),
            std::env::consts::OS,
            std::env::consts::ARCH,
        )
        .await
    }
    pub async fn install_github_cli_with(
        &self,
        config: &Config,
        os: &str,
        cpu: &str,
    ) -> Result<Value> {
        let target = self.options.git.gh.clone();
        if target.is_absolute() && target.is_file() {
            return self.github_status(true).await;
        }
        if target != managed_gh(&self.options.data_dir) {
            return Err(AppError::new("gh_not_installed")
                .detail("Restart Revia, then install the GitHub CLI again"));
        }
        {
            let mut state = self.github.lock().unwrap();
            if state.installing {
                return Err(AppError::new("busy"));
            }
            state.installing = true;
        }
        let result = async {
            let asset = latest(config, os, cpu).await?;
            if !update::valid_filename(&asset.name) {
                return Err(AppError::new("verification_failed"));
            }
            let tools = self.options.data_dir.join("tools");
            private_dir(&tools)?;
            let work = tools.join(format!("gh-{}", uuid::Uuid::new_v4()));
            private_dir(&work)?;
            let result = install(&asset, config, &work, &target).await;
            let _ = tokio::fs::remove_dir_all(&work).await;
            result
        }
        .await;
        self.github.lock().unwrap().installing = false;
        result?;
        self.invalidate_github();
        self.github_status(true).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asset_names() {
        for (tag, os, cpu, name) in [
            ("v2.104.0", "macos", "aarch64", "gh_2.104.0_macOS_arm64.zip"),
            ("v2.104.0", "macos", "x86_64", "gh_2.104.0_macOS_amd64.zip"),
            (
                "v2.104.0",
                "windows",
                "x86_64",
                "gh_2.104.0_windows_amd64.zip",
            ),
            (
                "v2.104.0",
                "windows",
                "aarch64",
                "gh_2.104.0_windows_arm64.zip",
            ),
            ("v2.104.0", "windows", "x86", "gh_2.104.0_windows_386.zip"),
            (
                "2.104.0",
                "linux",
                "x86_64",
                "gh_2.104.0_linux_amd64.tar.gz",
            ),
            (
                "v2.104.0",
                "linux",
                "aarch64",
                "gh_2.104.0_linux_arm64.tar.gz",
            ),
            ("v2.104.0", "linux", "arm", "gh_2.104.0_linux_armv6.tar.gz"),
        ] {
            assert_eq!(asset_name(tag, os, cpu).as_deref(), Some(name));
        }
        for (tag, os, cpu) in [
            ("v2.104.0", "macos", "x86"),
            ("v2.104.0", "windows", "arm"),
            ("v2.104.0", "freebsd", "x86_64"),
            ("v2.104.0", "linux", "riscv64"),
            ("v2/../x", "linux", "x86_64"),
            ("", "linux", "x86_64"),
            ("v", "linux", "x86_64"),
        ] {
            assert_eq!(asset_name(tag, os, cpu), None);
        }
    }
    #[test]
    fn managed_location_is_inside_the_data_directory() {
        let data = Path::new(if cfg!(windows) { r"C:\data" } else { "/data" });
        let path = managed_gh(data);
        assert!(path.starts_with(data));
        assert_eq!(path.file_name().unwrap(), binary_name());
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "bin");
    }
}
