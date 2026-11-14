use revia_lib::{
    EventSink,
    core::{Core, Options},
    git::Git,
    update::{self, Config, Target},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
struct Events(Mutex<Vec<Value>>);
impl EventSink for Events {
    fn emit(&self, name: &str, value: Value) {
        if name == "revia:update-progress" {
            self.0.lock().unwrap().push(value);
        }
    }
}
fn core() -> (tempfile::TempDir, Arc<Core>, Arc<Events>) {
    let temp = tempfile::tempdir().unwrap();
    let events = Arc::new(Events::default());
    let core = Core::new(Options {
        oauth: None,
        trash: Arc::new(revia_lib::trash::MoveTo(temp.path().join("trash"))),
        config_dir: temp.path().join("config"),
        data_dir: temp.path().join("data"),
        cache_dir: temp.path().join("cache"),
        git: Git::default(),
        events: events.clone(),
    })
    .unwrap();
    (temp, core, events)
}
fn release(base: &str, body: &[u8]) -> Value {
    json!({"tag_name":"v0.2.0", "draft":false, "prerelease":false, "name":"Version 0.2", "body":"Notes", "published_at":"2026-11-14T00:00:00Z", "html_url":"https://github.com/Shake1227/Revia/releases/tag/v0.2.0", "assets":[{"name":"Revia_0.2.0_universal.dmg", "size":body.len(), "browser_download_url":format!("{base}/installer"), "digest":format!("sha256:{:x}", Sha256::digest(body))}]})
}
fn info(r: &Value, current: &str) -> Value {
    update::info_from_release(
        current,
        &r.to_string(),
        &Target::new("macos", "aarch64", ""),
        42,
    )
    .unwrap()
}
#[test]
fn release_versions_and_metadata() {
    let mut r = release(
        "https://github.com/Shake1227/Revia/releases/download",
        b"abc",
    );
    for (current, available) in [("0.1.0", true), ("0.2.0", false), ("0.3.0", false)] {
        assert_eq!(info(&r, current)["available"], available);
    }
    assert_eq!(info(&r, "0.1.0")["checkedAt"], 42);
    assert_eq!(
        info(&r, "0.1.0")["latest"]["publishedAt"],
        1794614400000_u64
    );
    r["tag_name"] = json!("0.2.0");
    assert_eq!(info(&r, "0.1.0")["latest"]["version"], "0.2.0");
    r["tag_name"] = json!("v0.2.0-beta.10");
    assert_eq!(info(&r, "0.2.0-beta.2")["available"], true);
    assert_eq!(info(&r, "0.2.0")["available"], false);
    for field in ["draft", "prerelease"] {
        r[field] = json!(true);
        assert!(info(&r, "0.1.0")["latest"].is_null());
        r[field] = json!(false);
    }
    r["tag_name"] = json!("not-semver");
    assert!(info(&r, "0.1.0")["latest"].is_null());
    r["tag_name"] = json!("v0.2.0");
    r["name"] = json!("");
    r["body"] = json!("");
    let i = info(&r, "0.1.0");
    assert!(i["latest"]["name"].is_null());
    assert!(i["latest"]["notes"].is_null());
    r["body"] = json!("あ".repeat(20_001));
    r["html_url"] = json!("https://evil.example/");
    let i = info(&r, "0.1.0");
    assert_eq!(
        i["latest"]["notes"].as_str().unwrap().chars().count(),
        20_000
    );
    assert_eq!(
        i["latest"]["url"],
        "https://github.com/Shake1227/Revia/releases/tag/v0.2.0"
    );
    r["tag_name"] = json!("v0.2.0+build.2");
    assert_eq!(info(&r, "0.2.0+build.1")["available"], false);
}
#[test]
fn installer_selection_and_validation() {
    let mut r = release(
        "https://github.com/Shake1227/Revia/releases/download",
        b"abc",
    );
    let names = [
        "Revia_0.2.0_universal.dmg",
        "Revia_0.2.0_aarch64.dmg",
        "Revia_0.2.0_x64-setup.exe",
        "Revia_0.2.0_x64_en-US.msi",
        "Revia_0.2.0_amd64.AppImage",
        "Revia_0.2.0_amd64.deb",
        "Revia-0.2.0-1.x86_64.rpm",
    ];
    let template = r["assets"][0].clone();
    r["assets"] = json!(
        names
            .iter()
            .map(|n| {
                let mut a = template.clone();
                a["name"] = json!(n);
                a
            })
            .collect::<Vec<_>>()
    );
    for (os, cpu, format, name) in [
        ("macos", "aarch64", "", names[0]),
        ("macos", "x86_64", "", names[0]),
        ("windows", "x86_64", "", names[2]),
        ("linux", "x86_64", "appimage", names[4]),
        ("linux", "x86_64", "deb", names[5]),
        ("linux", "x86_64", "rpm", names[6]),
    ] {
        let i =
            update::info_from_release("0.1.0", &r.to_string(), &Target::new(os, cpu, format), 0)
                .unwrap();
        assert_eq!(i["latest"]["asset"]["name"], name);
    }
    for (os, cpu, format, name) in [
        ("macos", "aarch64", "", "Revia_0.2.0_aarch64.dmg"),
        ("macos", "arm64", "", "Revia_0.2.0_ARM64.DMG"),
        ("macos", "x86_64", "", "Revia_0.2.0_x86_64.dmg"),
        ("macos", "x64", "", "Revia_0.2.0_x64.dmg"),
        ("windows", "arm64", "", "Revia_0.2.0_arm64-setup.exe"),
        ("linux", "aarch64", "deb", "Revia_0.2.0_aarch64.deb"),
        ("linux", "arm64", "rpm", "Revia_0.2.0_arm64.rpm"),
        ("linux", "aarch64", "appimage", "Revia_0.2.0_arm64.AppImage"),
    ] {
        let mut one = r.clone();
        let mut asset = template.clone();
        asset["name"] = json!(name);
        one["assets"] = json!([asset]);
        let i =
            update::info_from_release("0.1.0", &one.to_string(), &Target::new(os, cpu, format), 0)
                .unwrap();
        assert_eq!(i["latest"]["asset"]["name"], name);
    }
    for target in [
        Target::new("windows", "arm64", ""),
        Target::new("linux", "aarch64", "deb"),
        Target::new("macos", "unknown", ""),
    ] {
        let mut no_universal = r.clone();
        no_universal["assets"].as_array_mut().unwrap().remove(0);
        assert!(update::info_from_release("0.1.0",&no_universal.to_string(),&target,0).unwrap()["latest"]["asset"].is_null());
    }
    r["assets"] = json!([template]);
    for name in [
        "../bad.dmg",
        ".hidden.dmg",
        "bad/name.dmg",
        "bad\\name.dmg",
        "bad name.dmg",
        "",
    ] {
        r["assets"][0]["name"] = json!(name);
        assert!(info(&r, "0.1.0")["latest"]["asset"].is_null());
    }
    assert!(!update::valid_filename(&"a".repeat(121)));
    assert!(update::valid_filename(&"a".repeat(120)));
    r["assets"][0]["name"] = json!(names[0]);
    r["assets"][0]["browser_download_url"] = json!("https://evil.example/a");
    assert!(info(&r, "0.1.0")["latest"]["asset"].is_null());
    for d in [
        "sha256:abc",
        "SHA256:abc",
        "",
        &format!("sha256:{}", "g".repeat(64)),
    ] {
        assert!(!update::valid_digest(d));
    }
    assert!(update::valid_digest(&format!("sha256:{}", "A".repeat(64))));
    r["assets"][0]["browser_download_url"] =
        json!("https://github.com/Shake1227/Revia/releases/download/a");
    r["assets"][0]["digest"] = json!("sha256:abc");
    assert_eq!(info(&r, "0.1.0")["latest"]["asset"]["hasDigest"], false);
}
struct Server {
    base: String,
    join: Option<std::thread::JoinHandle<()>>,
    requests: Arc<AtomicUsize>,
}
impl Server {
    fn new(responses: Vec<(u16, Vec<u8>, Option<String>, bool)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .expect("Local HTTP tests require permission to bind a loopback socket");
        let base = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let accepted = requests.clone();
        let join = std::thread::spawn(move || {
            for (status, body, location, stall) in responses {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(connection) => break connection,
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                && std::time::Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(e) => panic!("Mock HTTP server did not receive a request: {e}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut b = [0; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = stream.read(&mut b).unwrap();
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&b[..n]);
                }
                let request = String::from_utf8_lossy(&request).to_ascii_lowercase();
                assert!(request.contains(concat!("user-agent: revia/", env!("CARGO_PKG_VERSION"))));
                assert!(!request.contains("authorization"));
                if request.contains("/releases/latest ") {
                    assert!(request.contains("accept: application/vnd.github+json"));
                    assert!(request.contains("x-github-api-version: 2022-11-28"));
                }
                let location = location
                    .map(|s| format!("Location: {s}\r\n"))
                    .unwrap_or_default();
                if write!(stream,"HTTP/1.1 {status} Test\r\nContent-Length: {}\r\n{location}Connection: close\r\n\r\n",body.len()).is_err() {continue;}
                accepted.fetch_add(1, Ordering::SeqCst);
                if stall {
                    std::thread::sleep(Duration::from_millis(500));
                } else {
                    let _ = stream.write_all(&body);
                }
            }
        });
        Self {
            base,
            join: Some(join),
            requests,
        }
    }
    fn config(&self) -> Config {
        Config {
            api_base_url: self.base.clone(),
            release_prefix: format!("{}/releases/", self.base),
            download_prefix: format!("{}/", self.base),
            download_hosts: vec!["127.0.0.1".into()],
            allow_http: true,
        }
    }
    fn finish(mut self) {
        self.join.take().unwrap().join().unwrap();
    }
}
async fn checked(core: &Core, download: &Server, mut r: Value) {
    r["assets"][0]["browser_download_url"] = json!(format!("{}/installer", download.base));
    let api = Server::new(vec![(200, r.to_string().into_bytes(), None, false)]);
    let mut config = download.config();
    config.api_base_url = api.base.clone();
    core.check_update_with(&config, &Target::new("macos", "aarch64", ""))
        .await
        .unwrap();
    api.finish();
}
#[tokio::test]
async fn check_http_statuses() {
    let (_temp, core, _) = core();
    for (status, body, code) in [
        (
            200,
            release(
                "https://github.com/Shake1227/Revia/releases/download",
                b"abc",
            )
            .to_string(),
            None,
        ),
        (404, String::new(), None),
        (403, String::new(), Some("rate_limited")),
        (429, String::new(), Some("rate_limited")),
        (500, String::new(), Some("unknown")),
        (200, "broken".into(), Some("unknown")),
    ] {
        let server = Server::new(vec![(status, body.into_bytes(), None, false)]);
        let result = core
            .check_update_with(&server.config(), &Target::new("macos", "aarch64", ""))
            .await;
        match code {
            Some(code) => assert_eq!(result.unwrap_err().code, code),
            None => {
                let i = result.unwrap();
                assert_eq!(i["latest"].is_null(), status == 404);
            }
        }
        server.finish();
    }
}
#[tokio::test]
async fn download_success_reuse_cleanup() {
    let (_temp, core, events) = core();
    assert_eq!(
        core.download_update().await.unwrap_err().code,
        "invalid_input"
    );
    let server = Server::new(vec![(200, b"abc".to_vec(), None, false)]);
    checked(&core, &server, release("", b"abc")).await;
    let dir = core.options.cache_dir.join("updates");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("old"), b"old").unwrap();
    let result = core.download_update_with(&server.config()).await.unwrap();
    server.finish();
    assert_eq!(
        std::fs::read(result["path"].as_str().unwrap()).unwrap(),
        b"abc"
    );
    assert_eq!(result["verified"], true);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    assert_eq!(
        events.0.lock().unwrap().last().unwrap(),
        &json!({"received":3,"total":3})
    );
    let count = events.0.lock().unwrap().len();
    let result2 = core.download_update_with(&Config::default()).await.unwrap();
    assert_eq!(result, result2);
    assert_eq!(events.0.lock().unwrap().len(), count + 1);
}
#[tokio::test]
async fn verification_redirect_and_cancel() {
    for case in ["size", "digest", "redirect", "cancel", "no-digest"] {
        let (_temp, core, _) = core();
        let (status, location, stall) = if case == "redirect" {
            (302, Some("http://evil.example/file".into()), false)
        } else {
            (200, None, case == "cancel")
        };
        let server = Server::new(vec![(status, b"abc".to_vec(), location, stall)]);
        let mut r = release("", b"abc");
        if case == "size" {
            r["assets"][0]["size"] = json!(4);
        }
        if case == "digest" {
            r["assets"][0]["digest"] = json!(format!("sha256:{}", "0".repeat(64)));
        }
        if case == "no-digest" {
            r["assets"][0]["digest"] = Value::Null;
        }
        checked(&core, &server, r).await;
        let result = if case == "cancel" {
            let c = core.clone();
            let config = server.config();
            let job = tokio::spawn(async move { c.download_update_with(&config).await });
            tokio::time::timeout(Duration::from_secs(5), async {
                while server.requests.load(Ordering::SeqCst) == 0 {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(
                core.download_update_with(&server.config())
                    .await
                    .unwrap_err()
                    .code,
                "busy"
            );
            core.cancel_update_download();
            job.await.unwrap()
        } else {
            core.download_update_with(&server.config()).await
        };
        if case == "no-digest" {
            assert_eq!(result.unwrap()["verified"], false);
        } else {
            assert_eq!(
                result.unwrap_err().code,
                match case {
                    "cancel" => "cancelled",
                    "redirect" => "invalid_input",
                    _ => "verification_failed",
                }
            );
        }
        let dir = core.options.cache_dir.join("updates");
        assert!(!dir.join("Revia_0.2.0_universal.dmg.part").exists());
        core.cancel_update_download();
        server.finish();
    }
}
