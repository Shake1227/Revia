use revia_lib::{
    core::{Core, Options},
    git::Git,
    tools,
    update::Config,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const BINARY: &str = if cfg!(windows) { "gh.exe" } else { "gh" };

fn archiver() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("tar.exe")
    } else {
        PathBuf::from("tar")
    }
}
fn pack(source: &Path, entry: &str, archive: &Path) {
    let mut command = std::process::Command::new(archiver());
    if archive.extension().is_some_and(|e| e == "zip") {
        command.arg("-a").arg("-cf");
    } else {
        command.arg("-czf");
    }
    let status = command
        .arg(archive)
        .arg("-C")
        .arg(source)
        .arg(entry)
        .status()
        .unwrap();
    assert!(status.success());
}
fn archive_name(base: &str) -> String {
    if cfg!(target_os = "linux") {
        format!("{base}.tar.gz")
    } else {
        format!("{base}.zip")
    }
}
fn core(gh: PathBuf) -> (tempfile::TempDir, Arc<Core>) {
    let temp = tempfile::tempdir().unwrap();
    let core = Core::new(Options {
        oauth: None,
        trash: Arc::new(revia_lib::trash::MoveTo(temp.path().join("trash"))),
        config_dir: temp.path().join("config"),
        data_dir: temp.path().join("data"),
        cache_dir: temp.path().join("cache"),
        git: Git {
            gh,
            ..Git::default()
        },
        events: Arc::new(revia_lib::NoEvents),
    })
    .unwrap();
    (temp, core)
}
struct Server {
    base: String,
    join: Option<std::thread::JoinHandle<()>>,
    requests: Arc<AtomicUsize>,
}
impl Server {
    fn new(bodies: Vec<Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .expect("Local HTTP tests require permission to bind a loopback socket");
        let base = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let accepted = requests.clone();
        let join = std::thread::spawn(move || {
            for body in bodies {
                let deadline = std::time::Instant::now() + Duration::from_secs(3);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                && std::time::Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(_) => return,
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = stream.read(&mut buffer).unwrap();
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..n]);
                }
                let request = String::from_utf8_lossy(&request).to_ascii_lowercase();
                assert!(!request.contains("authorization"));
                accepted.fetch_add(1, Ordering::SeqCst);
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(&body);
            }
        });
        Self {
            base,
            join: Some(join),
            requests,
        }
    }
    fn config(&self, api: &Server) -> Config {
        Config {
            api_base_url: api.base.clone(),
            release_prefix: format!("{}/releases/", self.base),
            download_prefix: format!("{}/", self.base),
            download_hosts: vec!["127.0.0.1".into()],
            allow_http: true,
        }
    }
    fn finish(mut self) -> usize {
        self.join.take().unwrap().join().unwrap();
        self.requests.load(Ordering::SeqCst)
    }
}
fn release(name: &str, bytes: &[u8], url: &str, digest: Option<String>) -> Vec<u8> {
    json!({
        "tag_name": "v9.9.9",
        "assets": [
            {"name": "gh_9.9.9_checksums.txt", "size": 10, "browser_download_url": format!("{url}/checksums"), "digest": null},
            {"name": name, "size": bytes.len(), "browser_download_url": format!("{url}/{name}"), "digest": digest},
        ],
    })
    .to_string()
    .into_bytes()
}
fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[tokio::test]
async fn unpacks_both_archive_layouts() {
    let temp = tempfile::tempdir().unwrap();
    for (index, top) in [Some("gh_9.9.9_test"), None].into_iter().enumerate() {
        let source = temp.path().join(format!("source-{index}"));
        let root = top
            .map(|t| source.join(t))
            .unwrap_or_else(|| source.clone());
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(root.join("bin").join(BINARY), b"binary").unwrap();
        std::fs::write(root.join("LICENSE"), b"license").unwrap();
        let archive = temp.path().join(archive_name(&format!("layout-{index}")));
        match top {
            Some(top) => pack(&source, top, &archive),
            None => pack(&source, "bin", &archive),
        }
        let out = temp.path().join(format!("out-{index}"));
        std::fs::create_dir(&out).unwrap();
        let binary = tools::unpack(&archive, &out).await.unwrap();
        assert!(binary.starts_with(&out));
        assert_eq!(binary.file_name().unwrap(), BINARY);
        assert_eq!(std::fs::read(&binary).unwrap(), b"binary");
    }
    let empty = temp.path().join("empty");
    std::fs::create_dir_all(empty.join("docs")).unwrap();
    std::fs::write(empty.join("docs").join("readme"), b"text").unwrap();
    let archive = temp.path().join(archive_name("empty"));
    pack(&empty, "docs", &archive);
    let out = temp.path().join("out-empty");
    std::fs::create_dir(&out).unwrap();
    assert_eq!(
        tools::unpack(&archive, &out).await.unwrap_err().code,
        "verification_failed"
    );
    assert!(tools::find_binary(&out, 2).is_none());
}
#[tokio::test]
async fn refuses_assets_without_a_digest_before_downloading() {
    let (temp, core) = core(PathBuf::from("gh"));
    let (os, cpu, name) = ("linux", "x86_64", "gh_9.9.9_linux_amd64.tar.gz");
    let download = Server::new(vec![]);
    for digest in [None, Some("sha256:abc".to_owned())] {
        let api = Server::new(vec![release(name, b"data", &download.base, digest)]);
        let error = core
            .install_github_cli_with(&download.config(&api), os, cpu)
            .await
            .unwrap_err();
        assert_eq!(error.code, "verification_failed");
        assert_eq!(api.finish(), 1);
    }
    let api = Server::new(vec![release(
        "gh_9.9.9_linux_arm64.tar.gz",
        b"data",
        &download.base,
        Some(sha256(b"data")),
    )]);
    let error = core
        .install_github_cli_with(&download.config(&api), os, cpu)
        .await
        .unwrap_err();
    assert_eq!(error.code, "unknown");
    api.finish();
    assert_eq!(download.finish(), 0);
    assert!(!tools::managed_gh(&temp.path().join("data")).exists());
}
#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const SCRIPT: &str = "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo \"gh version 9.9.9 (test)\"\n  exit 0\nfi\necho \"You are not logged into any GitHub hosts.\" >&2\nexit 1\n";

    fn leftovers(data: &Path) -> Vec<String> {
        std::fs::read_dir(data.join("tools"))
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
    fn target() -> (&'static str, &'static str, String) {
        if cfg!(target_os = "linux") {
            ("linux", "x86_64", "gh_9.9.9_linux_amd64.tar.gz".into())
        } else {
            ("macos", "aarch64", "gh_9.9.9_macOS_arm64.zip".into())
        }
    }
    fn package(dir: &Path, script: &str, name: &str) -> Vec<u8> {
        let top = "gh_9.9.9_test";
        let bin = dir.join("source").join(top).join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("gh"), script).unwrap();
        std::fs::set_permissions(bin.join("gh"), std::fs::Permissions::from_mode(0o644)).unwrap();
        let archive = dir.join(name);
        pack(&dir.join("source"), top, &archive);
        std::fs::read(archive).unwrap()
    }
    #[tokio::test]
    async fn installs_into_the_data_directory_and_reports_it() {
        let (temp, core) = core(PathBuf::from("gh"));
        let (os, cpu, name) = target();
        let bytes = package(temp.path(), SCRIPT, &name);
        let before = core.github_status(true).await.unwrap();
        assert_eq!(before["cliInstalled"], false);
        let download = Server::new(vec![bytes.clone()]);
        let api = Server::new(vec![release(
            &name,
            &bytes,
            &download.base,
            Some(sha256(&bytes)),
        )]);
        let status = core
            .install_github_cli_with(&download.config(&api), os, cpu)
            .await
            .unwrap();
        assert_eq!(status["cliInstalled"], true);
        assert_eq!(status["authenticated"], false);
        assert_eq!(api.finish(), 1);
        assert_eq!(download.finish(), 1);
        let installed = tools::managed_gh(&temp.path().join("data"));
        let mode = std::fs::metadata(&installed).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
        assert_eq!(leftovers(&temp.path().join("data")), ["gh"]);
        let idle = Server::new(vec![]);
        let again = core
            .install_github_cli_with(&idle.config(&idle), os, cpu)
            .await
            .unwrap();
        assert_eq!(again["cliInstalled"], true);
        assert_eq!(idle.finish(), 0);
    }
    #[tokio::test]
    async fn rejects_a_changed_download_and_a_binary_that_does_not_start() {
        let (temp, core) = core(PathBuf::from("gh"));
        let (os, cpu, name) = target();
        let bytes = package(temp.path(), SCRIPT, &name);
        let mut changed = bytes.clone();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        let download = Server::new(vec![changed]);
        let api = Server::new(vec![release(
            &name,
            &bytes,
            &download.base,
            Some(sha256(&bytes)),
        )]);
        let error = core
            .install_github_cli_with(&download.config(&api), os, cpu)
            .await
            .unwrap_err();
        assert_eq!(error.code, "verification_failed");
        api.finish();
        download.finish();
        std::fs::remove_dir_all(temp.path().join("source")).unwrap();
        let broken = package(temp.path(), "#!/bin/sh\nexit 3\n", &name);
        let download = Server::new(vec![broken.clone()]);
        let api = Server::new(vec![release(
            &name,
            &broken,
            &download.base,
            Some(sha256(&broken)),
        )]);
        let error = core
            .install_github_cli_with(&download.config(&api), os, cpu)
            .await
            .unwrap_err();
        assert_eq!(error.code, "verification_failed");
        api.finish();
        download.finish();
        let data = temp.path().join("data");
        assert!(!tools::managed_gh(&data).exists());
        assert!(leftovers(&data).iter().all(|name| !name.starts_with("gh-")));
        assert_eq!(
            core.github_status(true).await.unwrap()["cliInstalled"],
            false
        );
    }
    #[tokio::test]
    async fn keeps_a_command_line_tool_that_is_already_there() {
        let temp = tempfile::tempdir().unwrap();
        let existing = temp.path().join("gh");
        std::fs::write(&existing, SCRIPT).unwrap();
        std::fs::set_permissions(&existing, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (data, core) = core(existing);
        let idle = Server::new(vec![]);
        let status = core
            .install_github_cli_with(&idle.config(&idle), "linux", "x86_64")
            .await
            .unwrap();
        assert_eq!(status["cliInstalled"], true);
        assert_eq!(idle.finish(), 0);
        assert!(!tools::managed_gh(&data.path().join("data")).exists());
    }
}
