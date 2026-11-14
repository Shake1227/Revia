use crate::{
    core::Core,
    error::{AppError, Result},
};
use serde::Deserialize;
use serde_json::json;
use std::{
    process::Stdio,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::io::AsyncWriteExt;

#[derive(Clone)]
pub struct Config {
    pub client_id: String,
    pub device_code_url: String,
    pub token_url: String,
    pub allow_http: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            client_id: "Ov23lid8N3XXfT9C0QTa".into(),
            device_code_url: "https://github.com/login/device/code".into(),
            token_url: "https://github.com/login/oauth/access_token".into(),
            allow_http: false,
        }
    }
}
#[derive(Default)]
struct Secret(Vec<u8>);
impl Drop for Secret {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(d).map(|s| Self(s.into_bytes()))
    }
}
impl Secret {
    fn text(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("")
    }
    fn redact(&self, text: &str) -> String {
        if self.0.is_empty() {
            text.into()
        } else {
            text.replace(self.text(), "[redacted]")
        }
    }
}
#[derive(Default, Deserialize)]
struct Response {
    #[serde(default)]
    device_code: Secret,
    #[serde(default)]
    user_code: Secret,
    verification_uri: Option<String>,
    expires_in: Option<u64>,
    interval: Option<u64>,
    #[serde(default)]
    access_token: Secret,
    error: Option<String>,
}
fn invalid() -> AppError {
    AppError::new("auth_failed")
}
fn validate(config: &Config) -> Result<()> {
    for value in [&config.device_code_url, &config.token_url] {
        let url = reqwest::Url::parse(value).map_err(|_| AppError::new("invalid_input"))?;
        if (url.scheme() != "https" && !(config.allow_http && url.scheme() == "http"))
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err(AppError::new("invalid_input"));
        }
    }
    Ok(())
}
fn form(fields: &[(&str, &str)]) -> String {
    fn encode(s: &str) -> String {
        let mut out = String::new();
        for b in s.bytes() {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    }
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}
async fn request(
    client: &reqwest::Client,
    url: &str,
    body: String,
    initial: bool,
) -> Result<Option<Response>> {
    let mut response = client
        .post(url)
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| AppError::new("network_unreachable"))?;
    let status = response.status();
    if initial && matches!(status.as_u16(), 401 | 403 | 404 | 422) {
        return Ok(None);
    }
    if status.is_redirection() {
        return Err(invalid());
    }
    let mut bytes = Secret::default();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::new("network_unreachable"))?
    {
        if bytes.0.len() + chunk.len() > 64 * 1024 {
            return Err(invalid());
        }
        bytes.0.extend_from_slice(&chunk);
    }
    let parsed: Response = serde_json::from_slice(&bytes.0).map_err(|_| invalid())?;
    if initial
        && matches!(
            parsed.error.as_deref(),
            Some("device_flow_disabled" | "incorrect_client_credentials" | "unauthorized_client")
        )
    {
        return Ok(None);
    }
    if !status.is_success() && parsed.error.is_none() {
        return Err(invalid());
    }
    Ok(Some(parsed))
}
pub(crate) async fn cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
pub(crate) async fn login(
    core: &Core,
    config: &Config,
    id: &str,
    additional: bool,
) -> Result<bool> {
    login_with_timing(
        core,
        config,
        id,
        additional,
        Duration::from_secs(5),
        Duration::from_secs(600),
    )
    .await
}
async fn login_with_timing(
    core: &Core,
    config: &Config,
    id: &str,
    additional: bool,
    minimum: Duration,
    limit: Duration,
) -> Result<bool> {
    validate(config)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| AppError::new("network_unreachable"))?;
    let started = tokio::time::Instant::now();
    let flow = async {
        let scope = if additional {
            "repo read:org workflow delete_repo"
        } else {
            "repo read:org workflow"
        };
        let Some(device) = request(
            &client,
            &config.device_code_url,
            form(&[("client_id", &config.client_id), ("scope", scope)]),
            true,
        )
        .await?
        else {
            return Ok(true);
        };
        if device.error.is_some() {
            return Err(invalid());
        }
        if device.device_code.0.is_empty()
            || device.user_code.0.is_empty()
            || device.verification_uri.as_deref() != Some("https://github.com/login/device")
        {
            return Err(invalid());
        }
        let expires = device.expires_in.ok_or_else(invalid)?;
        let deadline = started + limit.min(Duration::from_secs(expires));
        let mut interval = Duration::from_secs(device.interval.ok_or_else(invalid)?)
            .max(minimum)
            .min(limit);
        core.options.events.emit("revia:github-login", json!({"loginId":id,"kind":"code","userCode":device.user_code.text(),"verificationUrl":"https://github.com/login/device"}));
        let polling = async {
            loop {
                tokio::time::sleep(interval).await;
                let response = request(
                    &client,
                    &config.token_url,
                    form(&[
                        ("client_id", &config.client_id),
                        ("device_code", device.device_code.text()),
                        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ]),
                    false,
                )
                .await?
                .ok_or_else(invalid)?;
                if let Some(error) = response.error.as_deref() {
                    match error {
                        "authorization_pending" => continue,
                        "slow_down" => {
                            interval = interval.saturating_add(minimum);
                            continue;
                        }
                        "access_denied" => return Err(AppError::new("cancelled")),
                        "expired_token" => return Err(AppError::new("timeout")),
                        _ => {
                            return Err(invalid().detail(
                                &device.user_code.redact(
                                    &device
                                        .device_code
                                        .redact(&response.access_token.redact(error)),
                                ),
                            ));
                        }
                    }
                }
                if response.access_token.0.is_empty() {
                    return Err(invalid());
                }
                store_token(core, response.access_token).await?;
                return Ok(false);
            }
        };
        tokio::time::timeout_at(deadline, polling)
            .await
            .map_err(|_| AppError::new("timeout"))?
    };
    tokio::time::timeout(limit, flow)
        .await
        .map_err(|_| AppError::new("timeout"))?
}
async fn store_token(core: &Core, token: Secret) -> Result<()> {
    let mut cmd = crate::github::command(
        core,
        &[
            "auth",
            "login",
            "--hostname",
            "github.com",
            "--git-protocol",
            "https",
            "--with-token",
        ],
    )?;
    cmd.stdin(Stdio::piped());
    let mut child = cmd.spawn()?;
    let mut stdin = child.stdin.take().ok_or_else(invalid)?;
    let output = async move { child.wait_with_output().await };
    let write = async {
        stdin.write_all(&token.0).await?;
        stdin.shutdown().await?;
        drop(stdin);
        Ok::<_, std::io::Error>(())
    };
    let (written, output) = tokio::join!(write, output);
    let mut output = output?;
    if !output.status.success() {
        let mut diagnostic = Secret(std::mem::take(&mut output.stderr));
        diagnostic.0.push(b'\n');
        diagnostic.0.extend_from_slice(&output.stdout);
        output.stdout.fill(0);
        let text = token.redact(&String::from_utf8_lossy(&diagnostic.0));
        return Err(crate::github::classify(&text));
    }
    output.stdout.fill(0);
    output.stderr.fill(0);
    written?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_forms_and_url_policy() {
        let mut config = Config::default();
        assert_eq!(config.client_id, "Ov23lid8N3XXfT9C0QTa");
        assert!(validate(&config).is_ok());
        assert_eq!(
            form(&[("scope", "repo read:org workflow"), ("x", "a+&=あ")]),
            "scope=repo%20read%3Aorg%20workflow&x=a%2B%26%3D%E3%81%82"
        );
        config.token_url = "http://127.0.0.1/token".into();
        assert_eq!(validate(&config).unwrap_err().code, "invalid_input");
        config.allow_http = true;
        assert!(validate(&config).is_ok());
        for value in [
            "file:///tmp/token",
            "https://user:password@github.com/token",
            "https://github.com/token#fragment",
        ] {
            config.token_url = value.into();
            assert!(validate(&config).is_err());
        }
        let secret = Secret(b"opaque-secret".to_vec());
        assert_eq!(secret.redact("error opaque-secret"), "error [redacted]");
    }
    #[cfg(unix)]
    mod local {
        use super::*;
        use serde_json::Value;
        use std::{
            io::{Read, Write},
            net::TcpListener,
            os::unix::fs::PermissionsExt,
            sync::{Arc, Mutex},
        };
        #[derive(Default)]
        struct Events(Mutex<Vec<Value>>);
        impl crate::EventSink for Events {
            fn emit(&self, name: &str, payload: Value) {
                if name == "revia:github-login" {
                    self.0.lock().unwrap().push(payload);
                }
            }
        }
        struct Server {
            url: String,
            requests: Arc<Mutex<Vec<String>>>,
            stop: Arc<AtomicBool>,
            thread: Option<std::thread::JoinHandle<()>>,
        }
        impl Server {
            fn new(responses: Vec<(u16, String)>) -> Self {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                listener.set_nonblocking(true).unwrap();
                let url = format!("http://{}", listener.local_addr().unwrap());
                let requests = Arc::new(Mutex::new(Vec::new()));
                let stop = Arc::new(AtomicBool::new(false));
                let (record, stopping) = (requests.clone(), stop.clone());
                let thread = std::thread::spawn(move || {
                    let mut index = 0;
                    while !stopping.load(Ordering::SeqCst) {
                        let (mut stream, _) = match listener.accept() {
                            Ok(s) => s,
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                std::thread::sleep(Duration::from_millis(2));
                                continue;
                            }
                            Err(_) => break,
                        };
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut bytes = Vec::new();
                        let mut buf = [0; 1024];
                        while let Ok(n) = stream.read(&mut buf) {
                            if n == 0 {
                                break;
                            }
                            bytes.extend_from_slice(&buf[..n]);
                            if let Some(end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                                let header =
                                    String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                                let length = header
                                    .lines()
                                    .find_map(|l| l.strip_prefix("content-length: "))
                                    .and_then(|s| s.parse::<usize>().ok())
                                    .unwrap_or(0);
                                if bytes.len() >= end + 4 + length {
                                    break;
                                }
                            }
                        }
                        record
                            .lock()
                            .unwrap()
                            .push(String::from_utf8_lossy(&bytes).into_owned());
                        let (status, body) = &responses[index.min(responses.len() - 1)];
                        index += 1;
                        let _ = write!(
                            stream,
                            "HTTP/1.1 {status} Response\r\nContent-Length: {}\r\nLocation: /unexpected\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                    }
                });
                Self {
                    url,
                    requests,
                    stop,
                    thread: Some(thread),
                }
            }
            fn config(&self) -> Config {
                Config {
                    client_id: "test+client&".into(),
                    device_code_url: format!("{}/device", self.url),
                    token_url: format!("{}/token", self.url),
                    allow_http: true,
                }
            }
        }
        impl Drop for Server {
            fn drop(&mut self) {
                self.stop.store(true, Ordering::SeqCst);
                self.thread.take().unwrap().join().unwrap();
            }
        }
        fn device() -> String {
            json!({"device_code":"private-device-code", "user_code":"ABCD-1234", "verification_uri":"https://github.com/login/device", "expires_in":600, "interval":0}).to_string()
        }
        fn fixture(config: Option<Config>) -> (tempfile::TempDir, Arc<Core>, Arc<Events>) {
            let temp = tempfile::tempdir().unwrap();
            let gh = temp.path().join("gh");
            std::fs::write(&gh, r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$MOCK_ROOT/calls"
case "$1 $2" in
'auth login')
 if [ "$6" = https ] && [ "$7" = --with-token ]; then
   token=$(cat)
   [ "$token" = opaque-fixture-secret ]
   case "$*" in *opaque-fixture-secret*) exit 42;; esac
   if env | /usr/bin/grep -q opaque-fixture-secret; then exit 43; fi
   touch "$MOCK_ROOT/received"
   if [ -e "$MOCK_ROOT/wait" ]; then exec /bin/sleep 30; fi
   if [ -e "$MOCK_ROOT/fail" ]; then printf 'HTTP 401 %s\n' "$token" >&2; exit 1; fi
 else
   echo 'First copy your one-time code: ABCD-1234 https://github.com/login/device' >&2
 fi
 ;;
'auth refresh') echo 'First copy your one-time code: ABCD-1234 https://github.com/login/device' >&2 ;;
'auth status') if [ -e "$MOCK_ROOT/logout" ]; then echo 'not logged in' >&2; exit 1; fi ;;
'api user') echo '{"login":"mock","name":null,"avatar_url":null}' ;;
'config get') echo https ;;
*) exit 44 ;;
esac
"#).unwrap();
            std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
            let events = Arc::new(Events::default());
            let mut git = crate::git::Git {
                gh,
                ..Default::default()
            };
            git.env
                .insert("MOCK_ROOT".into(), temp.path().to_string_lossy().into());
            let core = Core::new(crate::core::Options {
                oauth: config,
                trash: Arc::new(crate::trash::MoveTo(temp.path().join("trash"))),
                config_dir: temp.path().join("config"),
                data_dir: temp.path().join("data"),
                cache_dir: temp.path().join("cache"),
                git,
                events: events.clone(),
            })
            .unwrap();
            (temp, core, events)
        }
        async fn fast(core: &Core, config: &Config, additional: bool) -> Result<bool> {
            login_with_timing(
                core,
                config,
                "test-id",
                additional,
                Duration::from_millis(5),
                Duration::from_secs(2),
            )
            .await
        }
        async fn terminal(events: &Events, id: &str) -> Value {
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if let Some(v) = events
                        .0
                        .lock()
                        .unwrap()
                        .iter()
                        .find(|v| v["loginId"] == id && v["kind"] != "code")
                        .cloned()
                    {
                        return v;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap()
        }
        async fn until_code(events: &Events) {
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if events.0.lock().unwrap().iter().any(|v| v["kind"] == "code") {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
        }
        #[tokio::test]
        async fn oauth_terminal_events_success_and_redacted_failure() {
            for fail in [false, true] {
                let server = Server::new(vec![
                    (200, device()),
                    (
                        200,
                        json!({"access_token":"opaque-fixture-secret"}).to_string(),
                    ),
                ]);
                let (temp, core, events) = fixture(Some(server.config()));
                if fail {
                    std::fs::write(temp.path().join("fail"), "").unwrap();
                }
                let id = core.github_login_start(None).await.unwrap()["loginId"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                let event = tokio::time::timeout(Duration::from_secs(8), async {
                    loop {
                        if let Some(event) = events
                            .0
                            .lock()
                            .unwrap()
                            .iter()
                            .find(|v| v["loginId"] == id && v["kind"] != "code")
                            .cloned()
                        {
                            break event;
                        }
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await
                .unwrap();
                assert_eq!(event["kind"], if fail { "failed" } else { "succeeded" });
                if fail {
                    assert_eq!(event["error"]["code"], "auth_failed");
                } else {
                    assert_eq!(event["status"]["authenticated"], true);
                }
                assert!(temp.path().join("received").exists());
                let serialized = serde_json::to_string(&*events.0.lock().unwrap()).unwrap();
                assert!(!serialized.contains("opaque-fixture-secret"));
                assert!(!serialized.contains("private-device-code"));
            }
        }
        #[tokio::test]
        async fn pending_success_forms_stdin_and_scope_addition() {
            for additional in [false, true] {
                let server = Server::new(vec![
                    (200, device()),
                    (200, json!({"error":"authorization_pending"}).to_string()),
                    (
                        200,
                        json!({"access_token":"opaque-fixture-secret"}).to_string(),
                    ),
                ]);
                let config = server.config();
                let (temp, core, events) = fixture(Some(config.clone()));
                assert!(!fast(&core, &config, additional).await.unwrap());
                assert!(temp.path().join("received").exists());
                let calls = std::fs::read_to_string(temp.path().join("calls")).unwrap();
                assert!(calls.contains(
                    "auth login --hostname github.com --git-protocol https --with-token"
                ));
                let requests = server.requests.lock().unwrap();
                assert_eq!(requests.len(), 3);
                assert!(requests[0].contains("client_id=test%2Bclient%26"));
                let scope = if additional {
                    "scope=repo%20read%3Aorg%20workflow%20delete_repo"
                } else {
                    "scope=repo%20read%3Aorg%20workflow"
                };
                assert!(requests[0].ends_with(scope));
                assert!(
                    requests[0]
                        .to_ascii_lowercase()
                        .contains("content-type: application/x-www-form-urlencoded")
                );
                assert!(
                    requests[0]
                        .to_ascii_lowercase()
                        .contains("accept: application/json")
                );
                assert!(requests[1].contains("device_code=private-device-code&grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"));
                let events = events.0.lock().unwrap();
                assert_eq!(events[0]["userCode"], "ABCD-1234");
                assert_eq!(
                    events[0]["verificationUrl"],
                    "https://github.com/login/device"
                );
                assert!(
                    !serde_json::to_string(&*events)
                        .unwrap()
                        .contains("private-device-code")
                );
            }
        }
        #[tokio::test]
        async fn slow_down_and_error_classification() {
            for (error, expected) in [
                ("access_denied", "cancelled"),
                ("expired_token", "timeout"),
                ("unrecognized_error", "auth_failed"),
            ] {
                let server = Server::new(vec![
                    (200, device()),
                    (200, json!({"error":"slow_down"}).to_string()),
                    (
                        200,
                        json!({"error":error, "error_description":"ignored"}).to_string(),
                    ),
                ]);
                let config = server.config();
                let (_temp, core, _) = fixture(None);
                let start = tokio::time::Instant::now();
                let result = fast(&core, &config, false).await.unwrap_err();
                assert_eq!(result.code, expected);
                assert!(start.elapsed() >= Duration::from_millis(15));
                if expected == "auth_failed" {
                    assert_eq!(result.detail.as_deref(), Some(error));
                }
                assert_eq!(server.requests.lock().unwrap().len(), 3);
            }
        }
        #[tokio::test]
        async fn malformed_oversized_redirect_and_verification_responses() {
            let mut wrong: Value = serde_json::from_str(&device()).unwrap();
            wrong["verification_uri"] = json!("https://evil.example/device");
            for (status, body) in [
                (200, wrong.to_string()),
                (200, "x".repeat(65537)),
                (200, "not json".into()),
                (200, "{}".into()),
                (302, device()),
            ] {
                let server = Server::new(vec![(status, body)]);
                let (_temp, core, events) = fixture(None);
                assert_eq!(
                    fast(&core, &server.config(), false).await.unwrap_err().code,
                    "auth_failed"
                );
                assert!(events.0.lock().unwrap().is_empty());
                assert_eq!(server.requests.lock().unwrap().len(), 1);
            }
        }
        #[tokio::test]
        async fn total_timeout_and_expiry_stop_polling() {
            for expires in [0, 600] {
                let mut body: Value = serde_json::from_str(&device()).unwrap();
                body["expires_in"] = json!(expires);
                let server = Server::new(vec![
                    (200, body.to_string()),
                    (200, json!({"error":"authorization_pending"}).to_string()),
                ]);
                let (_temp, core, _) = fixture(None);
                let result = login_with_timing(
                    &core,
                    &server.config(),
                    "id",
                    false,
                    Duration::from_millis(5),
                    Duration::from_millis(40),
                )
                .await
                .unwrap_err();
                assert_eq!(result.code, "timeout");
            }
        }
        #[tokio::test]
        async fn fallback_responses_and_none_use_legacy_and_same_id() {
            for (status, body) in [
                (200, json!({"error":"device_flow_disabled"}).to_string()),
                (404, "not json".into()),
                (
                    200,
                    json!({"error":"incorrect_client_credentials"}).to_string(),
                ),
                (200, json!({"error":"unauthorized_client"}).to_string()),
                (401, "".into()),
                (403, "".into()),
                (422, "".into()),
            ] {
                let server = Server::new(vec![(status, body)]);
                let (temp, core, events) = fixture(Some(server.config()));
                let id = core.github_login_start(None).await.unwrap()["loginId"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                assert_eq!(terminal(&events, &id).await["kind"], "succeeded");
                assert!(
                    events
                        .0
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|v| v["kind"] == "code" && v["loginId"] == id)
                );
                assert!(
                    std::fs::read_to_string(temp.path().join("calls"))
                        .unwrap()
                        .contains("--web --skip-ssh-key")
                );
            }
            let (_temp, core, events) = fixture(None);
            let id = core.github_login_start(None).await.unwrap()["loginId"]
                .as_str()
                .unwrap()
                .to_owned();
            assert_eq!(terminal(&events, &id).await["kind"], "succeeded");
        }
        #[tokio::test]
        async fn cancel_wait_busy_restart_and_no_further_requests() {
            let server = Server::new(vec![(200, device())]);
            let (_temp, core, events) = fixture(Some(server.config()));
            let id = core.github_login_start(None).await.unwrap()["loginId"]
                .as_str()
                .unwrap()
                .to_owned();
            until_code(&events).await;
            assert_eq!(
                core.github_login_start(None).await.unwrap_err().code,
                "busy"
            );
            let start = tokio::time::Instant::now();
            core.github_login_cancel(&id);
            assert_eq!(terminal(&events, &id).await["kind"], "cancelled");
            assert!(start.elapsed() < Duration::from_secs(1));
            tokio::time::sleep(Duration::from_millis(60)).await;
            assert_eq!(server.requests.lock().unwrap().len(), 1);
            assert!(core.github.lock().unwrap().login.is_none());
            let next = core.github_login_start(None).await.unwrap();
            core.github_login_cancel(next["loginId"].as_str().unwrap());
            assert_eq!(
                terminal(&events, next["loginId"].as_str().unwrap()).await["kind"],
                "cancelled"
            );
        }
        #[tokio::test]
        async fn failed_gh_redacts_opaque_token_and_private_codes() {
            let server = Server::new(vec![
                (200, device()),
                (
                    200,
                    json!({"access_token":"opaque-fixture-secret"}).to_string(),
                ),
            ]);
            let (temp, core, _) = fixture(None);
            std::fs::write(temp.path().join("fail"), "").unwrap();
            let error = fast(&core, &server.config(), false).await.unwrap_err();
            assert_eq!(error.code, "auth_failed");
            let serialized = serde_json::to_string(&error).unwrap();
            assert!(!serialized.contains("opaque-fixture-secret"));
            assert!(!serialized.contains("private-device-code"));
            assert!(serialized.contains("[redacted]"));
        }
        #[tokio::test]
        async fn validation_not_authenticated_and_http_rejection() {
            let (temp, core, events) = fixture(Some(Config {
                device_code_url: "http://127.0.0.1/device".into(),
                ..Default::default()
            }));
            for scopes in [vec![], vec!["repo".into()]] {
                assert_eq!(
                    core.github_login_start(Some(scopes))
                        .await
                        .unwrap_err()
                        .code,
                    "invalid_input"
                );
            }
            std::fs::write(temp.path().join("logout"), "").unwrap();
            assert_eq!(
                core.github_login_start(Some(vec!["delete_repo".into()]))
                    .await
                    .unwrap_err()
                    .code,
                "not_authenticated"
            );
            let id = core.github_login_start(None).await.unwrap()["loginId"]
                .as_str()
                .unwrap()
                .to_owned();
            assert_eq!(
                terminal(&events, &id).await["error"]["code"],
                "invalid_input"
            );
        }
        #[tokio::test]
        async fn unreachable_does_not_fallback() {
            let server = Server::new(vec![(200, device())]);
            let config = server.config();
            drop(server);
            let (temp, core, events) = fixture(Some(config));
            let id = core.github_login_start(None).await.unwrap()["loginId"]
                .as_str()
                .unwrap()
                .to_owned();
            let event = terminal(&events, &id).await;
            assert_eq!(event["kind"], "failed");
            assert_eq!(event["error"]["code"], "network_unreachable");
            assert!(!temp.path().join("calls").exists());
        }
        #[tokio::test]
        async fn cancel_gh_stdin_child_within_one_second() {
            let (temp, core, _) = fixture(None);
            std::fs::write(temp.path().join("wait"), "").unwrap();
            let cancel = AtomicBool::new(false);
            let flow = async {
                tokio::select! {
                    _ = cancelled(&cancel) => Err(AppError::new("cancelled")),
                    result = store_token(&core, Secret(b"opaque-fixture-secret".to_vec())) => result,
                }
            };
            let trigger = async {
                tokio::time::timeout(Duration::from_secs(2), async {
                    while !temp.path().join("received").exists() {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await
                .unwrap();
                cancel.store(true, Ordering::SeqCst);
            };
            let (result, ()) = tokio::time::timeout(Duration::from_secs(3), async {
                tokio::join!(flow, trigger)
            })
            .await
            .unwrap();
            assert_eq!(result.unwrap_err().code, "cancelled");
        }
        #[tokio::test]
        async fn token_storage_without_network() {
            let (temp, core, _) = fixture(None);
            store_token(&core, Secret(b"opaque-fixture-secret".to_vec()))
                .await
                .unwrap();
            assert!(temp.path().join("received").exists());
            std::fs::write(temp.path().join("fail"), "").unwrap();
            let error = store_token(&core, Secret(b"opaque-fixture-secret".to_vec()))
                .await
                .unwrap_err();
            assert_eq!(error.code, "auth_failed");
            assert!(
                !serde_json::to_string(&error)
                    .unwrap()
                    .contains("opaque-fixture-secret")
            );
        }
    }
}
