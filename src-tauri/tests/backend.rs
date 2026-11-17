#[cfg(unix)]
mod common;
use revia_lib::{
    NoEvents,
    core::{Core, Options},
    git::Git,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use tempfile::TempDir;
struct Fixture {
    temp: TempDir,
    core: Arc<Core>,
    git: Git,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let empty = temp.path().join("empty-config");
        std::fs::write(&empty, "").unwrap();
        let mut git = Git::default();
        git.env.extend([
            ("GIT_CONFIG_GLOBAL".into(), empty.to_string_lossy().into()),
            ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ("GIT_AUTHOR_NAME".into(), "Test".into()),
            ("GIT_AUTHOR_EMAIL".into(), "test@example.invalid".into()),
            ("GIT_COMMITTER_NAME".into(), "Test".into()),
            ("GIT_COMMITTER_EMAIL".into(), "test@example.invalid".into()),
            ("GIT_CONFIG_COUNT".into(), "3".into()),
            ("GIT_CONFIG_KEY_0".into(), "commit.gpgsign".into()),
            ("GIT_CONFIG_VALUE_0".into(), "false".into()),
            ("GIT_CONFIG_KEY_1".into(), "user.name".into()),
            ("GIT_CONFIG_VALUE_1".into(), "Test".into()),
            ("GIT_CONFIG_KEY_2".into(), "user.email".into()),
            ("GIT_CONFIG_VALUE_2".into(), "test@example.invalid".into()),
        ]);
        let core = Self::core(temp.path(), git.clone());
        Self { temp, core, git }
    }
    fn core(root: &Path, git: Git) -> Arc<Core> {
        Core::new(Options {
            oauth: None,
            trash: Arc::new(revia_lib::trash::MoveTo(root.join("trash"))),
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            git,
            events: Arc::new(NoEvents),
        })
        .unwrap()
    }
    fn dir(&self, name: &str) -> PathBuf {
        let p = self.temp.path().join(name);
        std::fs::create_dir_all(&p).unwrap();
        revia_lib::platform::canonicalize(p).unwrap()
    }
    fn run(&self, path: &Path, args: &[&str]) -> String {
        let output = Command::new(&self.git.executable)
            .current_dir(path)
            .envs(&self.git.env)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().into()
    }
    fn repo(&self, name: &str) -> PathBuf {
        let p = self.dir(name);
        self.run(&p, &["init", "-b", "main"]);
        self.write_commit(&p, "base.txt", "base\n");
        p
    }
    fn write_commit(&self, p: &Path, name: &str, content: &str) {
        std::fs::write(p.join(name), content).unwrap();
        self.run(p, &["add", "--", name]);
        self.run(p, &["commit", "-m", name]);
    }
    async fn add(&self, p: &Path) -> String {
        let w = self
            .core
            .registry_change("add_project", json!(p), Value::Null)
            .await
            .unwrap();
        w["projects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["path"] == p.to_string_lossy().as_ref())
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .into()
    }
    async fn state(&self, id: &str) -> Value {
        self.core.refresh(Some(vec![id.into()])).await.unwrap()[0]["status"].clone()
    }
    async fn op(&self, kind: &str, id: &str, extra: Value) -> Value {
        let mut input = json!({"itemId":id,"projectId":id});
        for (k, v) in extra.as_object().unwrap() {
            input[k] = v.clone();
        }
        let snapshot = self
            .core
            .start(json!({"kind":kind,"items":[input]}))
            .await
            .unwrap();
        self.wait(&snapshot).await["items"][0].clone()
    }
    async fn wait(&self, snapshot: &Value) -> Value {
        let id = snapshot["operationId"].as_str().unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                let s = self.core.operation(id).unwrap();
                if s["state"] == "finished" {
                    return s;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    fn remote(&self, p: &Path) -> PathBuf {
        let bare = self.dir("remote.git");
        self.run(&bare, &["init", "--bare"]);
        self.run(p, &["remote", "add", "origin", bare.to_str().unwrap()]);
        self.run(p, &["push", "-u", "origin", "main"]);
        bare
    }
    fn clone_repo(&self, bare: &Path, name: &str) -> PathBuf {
        let parent = self.temp.path();
        let p = parent.join(name);
        self.run(
            parent,
            &[
                "clone",
                "-b",
                "main",
                bare.to_str().unwrap(),
                p.to_str().unwrap(),
            ],
        );
        p
    }
}
#[tokio::test]
async fn discovery_boundaries_exclusions_and_missing() {
    let f = Fixture::new();
    let root = f.dir("root");
    for name in [
        "repo",
        "marker",
        "node_modules/hidden",
        "deep/level/project",
        "nested/child",
        ".hidden/project",
    ] {
        std::fs::create_dir_all(root.join(name)).unwrap();
        std::fs::write(root.join(name).join("Cargo.toml"), "").unwrap();
    }
    std::fs::create_dir_all(root.join("repo/.git")).unwrap();
    std::fs::write(root.join("nested/package.json"), "{}").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("marker"), root.join("linked")).unwrap();
    let w = f
        .core
        .registry_change("add_root", json!(root), json!(2))
        .await
        .unwrap();
    let names = w["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"marker"));
    assert!(names.contains(&"nested"));
    let marker = w["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "marker")
        .unwrap();
    let id = marker["id"].as_str().unwrap();
    assert_eq!(marker["status"]["state"], "not_initialized");
    f.core
        .registry_change("exclude", json!(id), Value::Null)
        .await
        .unwrap();
    assert_eq!(f.core.workspace()["projects"].as_array().unwrap().len(), 2);
    f.core
        .registry_change("restore", json!(root.join("marker")), Value::Null)
        .await
        .unwrap();
    assert_eq!(f.core.workspace()["projects"].as_array().unwrap().len(), 3);
    let manual = f.dir("manual");
    let id = f.add(&manual).await;
    std::fs::remove_dir(&manual).unwrap();
    assert_eq!(f.state(&id).await["state"], "missing");
    assert!(
        f.core.scan().await.unwrap()["projects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == id)
    );
    let own = f.dir("own");
    std::fs::write(own.join("package.json"), "{}").unwrap();
    let w = f
        .core
        .registry_change("add_root", json!(own), Value::Null)
        .await
        .unwrap();
    assert!(
        w["projects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "own")
    );
}
#[tokio::test]
async fn status_targets_and_priority() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    assert_eq!(f.state(&id).await["state"], "no_remote");
    let bare = f.remote(&p);
    assert_eq!(f.state(&id).await["state"], "synced");
    assert_eq!(
        f.state(&id).await["targets"]["push"]["target"]["source"],
        "upstream"
    );
    let other = f.clone_repo(&bare, "other");
    f.write_commit(&other, "remote.txt", "remote\n");
    f.run(&other, &["push"]);
    f.run(&p, &["fetch"]);
    assert_eq!(f.state(&id).await["state"], "behind");
    std::fs::write(p.join("dirty.txt"), "dirty").unwrap();
    let status = f.state(&id).await;
    assert_eq!(status["state"], "dirty");
    assert_eq!(status["behind"], 1);
    std::fs::remove_file(p.join("dirty.txt")).unwrap();
    f.write_commit(&p, "local.txt", "local");
    assert_eq!(f.state(&id).await["state"], "diverged");
    f.run(&p, &["checkout", "--detach"]);
    assert_eq!(f.state(&id).await["state"], "detached");
    f.run(&p, &["checkout", "main"]);
    f.core
        .set_target(
            &id,
            "main",
            "push",
            json!({"remote":"missing","branch":"main"}),
        )
        .await
        .unwrap();
    let s = f.state(&id).await;
    assert!(s["targets"]["push"]["target"].is_null());
    assert_eq!(s["targets"]["push"]["savedInvalid"], "remote_missing");
    f.core
        .set_target(
            &id,
            "main",
            "pull",
            json!({"remote":"origin","branch":"absent"}),
        )
        .await
        .unwrap();
    assert_eq!(
        f.state(&id).await["targets"]["pull"]["savedInvalid"],
        "branch_missing"
    );
    f.core
        .set_target(
            &id,
            "main",
            "push",
            json!({"remote":"origin","branch":"new"}),
        )
        .await
        .unwrap();
    assert_eq!(
        f.state(&id).await["targets"]["push"]["target"]["source"],
        "saved"
    );
    assert_eq!(
        f.state(&id).await["targets"]["push"]["target"]["exists"],
        false
    );
}
#[tokio::test]
async fn inferred_remote_default_and_ambiguous() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    let bare = f.remote(&p);
    f.run(&p, &["branch", "--unset-upstream"]);
    assert_eq!(
        f.state(&id).await["targets"]["push"]["target"]["source"],
        "remote_default"
    );
    f.run(&p, &["checkout", "-b", "feature"]);
    let s = f.state(&id).await;
    assert_eq!(s["state"], "unpublished");
    assert_eq!(s["targets"]["push"]["target"]["source"], "inferred");
    f.run(&p, &["remote", "rename", "origin", "one"]);
    f.run(&p, &["remote", "add", "two", bare.to_str().unwrap()]);
    assert!(f.state(&id).await["targets"]["push"]["target"].is_null());
}
#[tokio::test]
async fn settings_persistence_recovery_and_unknown_fields() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    f.core
        .update_ui(json!({"future":{"theme":"night"}}))
        .await
        .unwrap();
    f.core
        .set_target(
            &id,
            "main",
            "push",
            json!({"remote":"origin","branch":"destination"}),
        )
        .await
        .unwrap();
    let reload = Fixture::core(f.temp.path(), f.git.clone());
    assert_eq!(
        reload.workspace()["settings"]["ui"]["future"]["theme"],
        "night"
    );
    reload.scan().await.unwrap();
    assert_eq!(
        reload.project(&id).unwrap()["settings"]["branchTargets"]["main"]["push"]["branch"],
        "destination"
    );
    f.core
        .update_general(json!({"concurrency":2}))
        .await
        .unwrap();
    let path = f.temp.path().join("config/settings.json");
    std::fs::write(&path, "broken").unwrap();
    let recovered = Fixture::core(f.temp.path(), f.git.clone());
    let app = recovered.app_state().await;
    assert_eq!(app["recovery"]["kind"], "restored_backup");
    assert!(Path::new(app["recovery"]["corruptCopyPath"].as_str().unwrap()).exists());
    assert_eq!(app["settings"]["ui"]["future"]["theme"], "night");
    std::fs::remove_file(f.temp.path().join("config/settings.json.bak")).unwrap();
    std::fs::write(&path, "[]").unwrap();
    let recovered = Fixture::core(f.temp.path(), f.git.clone());
    assert_eq!(recovered.app_state().await["recovery"]["kind"], "reset");
    std::fs::write(&path, r#"{"unknown":true,"settings":{"ui":{"future":42}}}"#).unwrap();
    let recovered = Fixture::core(f.temp.path(), f.git.clone());
    let app = recovered.app_state().await;
    assert!(app["recovery"].is_null());
    assert_eq!(app["settings"]["general"]["concurrency"], 4);
    assert_eq!(app["settings"]["ui"]["future"], 42);
}
#[tokio::test]
async fn sync_fast_forward_push_dirty_and_new_branch() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let bare = f.remote(&p);
    let other = f.clone_repo(&bare, "other");
    let id = f.add(&p).await;
    f.core
        .update_general(json!({"sync":{"commit":false}}))
        .await
        .unwrap();
    f.write_commit(&other, "remote.txt", "remote");
    f.run(&other, &["push"]);
    std::fs::write(p.join("dirty.txt"), "keep").unwrap();
    let item = f.op("sync", &id, json!({})).await;
    assert_eq!(item["state"], "succeeded", "{item}");
    assert_eq!(
        std::fs::read_to_string(p.join("dirty.txt")).unwrap(),
        "keep"
    );
    assert!(p.join("remote.txt").exists());
    f.write_commit(&p, "local.txt", "local");
    assert_eq!(f.state(&id).await["state"], "dirty");
    assert_eq!(f.state(&id).await["ahead"], 1);
    assert_eq!(f.op("sync", &id, json!({})).await["state"], "succeeded");
    assert_eq!(f.state(&id).await["ahead"], 0);
    f.core
        .set_target(
            &id,
            "main",
            "push",
            json!({"remote":"origin","branch":"new"}),
        )
        .await
        .unwrap();
    let upstream = f.run(&p, &["rev-parse", "--abbrev-ref", "@{upstream}"]);
    assert_eq!(f.op("sync", &id, json!({})).await["state"], "succeeded");
    assert_eq!(
        f.run(&p, &["rev-parse", "--abbrev-ref", "@{upstream}"]),
        upstream
    );
    assert!(!f.run(&bare, &["rev-parse", "refs/heads/new"]).is_empty());
}
#[tokio::test]
async fn sync_commits_pulls_and_pushes_in_one_go() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let bare = f.remote(&p);
    let other = f.clone_repo(&bare, "other");
    let id = f.add(&p).await;
    f.write_commit(&other, "remote.txt", "remote");
    f.run(&other, &["push"]);
    f.run(&p, &["fetch"]);
    std::fs::write(p.join("base.txt"), "changed\n").unwrap();
    std::fs::write(p.join("local.txt"), "local\n").unwrap();
    std::fs::write(p.join(".env"), "TOKEN=secret\n").unwrap();
    let request = json!({"kind":"sync","items":[{"itemId":id,"projectId":id}]});
    let plan = f.core.plan(request).await.unwrap();
    let _: revia_lib::dto::OperationPlan = serde_json::from_value(plan.clone()).unwrap();
    let planned = &plan["items"][0];
    assert_eq!(planned["risk"], "safe", "{planned}");
    let codes = planned["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(codes, ["fetch", "pull", "commit", "push"]);
    assert_eq!(planned["steps"][2]["files"], 2);
    assert!(
        planned["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["code"] == "skips_sensitive_files" && w["paths"] == json!([".env"]))
    );
    let item = f.op("sync", &id, json!({})).await;
    assert_eq!(item["state"], "succeeded", "{item}");
    let done = item["completed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(done, ["fetch", "pull", "commit", "push"]);
    assert_eq!(
        f.run(&p, &["log", "-1", "--format=%s"]),
        "Update base.txt and local.txt"
    );
    assert_eq!(
        f.run(&bare, &["rev-parse", "main"]),
        f.run(&p, &["rev-parse", "HEAD"])
    );
    assert!(p.join("remote.txt").exists());
    assert!(p.join(".env").exists());
    assert!(
        f.run(&p, &["ls-files", "--others", "--exclude-standard"])
            .contains(".env")
    );
    f.core
        .update_general(json!({"sync":{"commitMessage":"Save work"}}))
        .await
        .unwrap();
    std::fs::write(p.join("local.txt"), "again\n").unwrap();
    assert_eq!(f.op("sync", &id, json!({})).await["state"], "succeeded");
    assert_eq!(f.run(&p, &["log", "-1", "--format=%s"]), "Save work");
    for patch in [
        json!({"sync":{"diverged":"force"}}),
        json!({"sync":{"commit":"yes"}}),
        json!({"sync":{"commitMessage":"x".repeat(201)}}),
        json!({"sync":{"commitMessage":"line\nbreak"}}),
    ] {
        assert_eq!(
            f.core.update_general(patch).await.unwrap_err().code,
            "invalid_input"
        );
    }
}
#[tokio::test]
async fn sync_diverged_policies() {
    for policy in ["stop", "merge", "prefer_local", "prefer_remote"] {
        let f = Fixture::new();
        let p = f.repo("repo");
        let bare = f.remote(&p);
        let other = f.clone_repo(&bare, "other");
        let id = f.add(&p).await;
        f.core
            .update_general(json!({"sync":{"diverged":policy}}))
            .await
            .unwrap();
        f.write_commit(&other, "base.txt", "remote\n");
        f.run(&other, &["push"]);
        f.write_commit(&p, "base.txt", "local\n");
        f.run(&p, &["fetch"]);
        let before = f.run(&p, &["rev-parse", "HEAD"]);
        let plan = f
            .core
            .plan(json!({"kind":"sync","items":[{"itemId":id,"projectId":id}]}))
            .await
            .unwrap();
        let _: revia_lib::dto::OperationPlan = serde_json::from_value(plan.clone()).unwrap();
        let planned = &plan["items"][0];
        let item = f.op("sync", &id, json!({})).await;
        match policy {
            "stop" => {
                assert_eq!(planned["blocker"], "diverged");
                assert_eq!(item["reason"], "diverged", "{item}");
                assert_eq!(f.run(&p, &["rev-parse", "HEAD"]), before);
            }
            "merge" => {
                assert_eq!(planned["risk"], "caution");
                assert!(
                    planned["warnings"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|w| w["code"] == "conflicts_likely")
                );
                assert_eq!(item["state"], "conflict", "{item}");
                assert_eq!(f.state(&id).await["state"], "conflicted");
                assert_ne!(
                    f.run(&bare, &["rev-parse", "main"]),
                    f.run(&p, &["rev-parse", "HEAD"])
                );
            }
            _ => {
                let side = if policy == "prefer_local" {
                    "local"
                } else {
                    "remote"
                };
                assert_eq!(planned["risk"], "caution");
                assert!(
                    planned["warnings"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|w| w["code"] == "conflicts_resolved" && w["prefer"] == side)
                );
                let merge = planned["steps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|s| s["code"] == "pull")
                    .unwrap();
                assert_eq!(merge["mode"], "merge");
                assert_eq!(merge["prefer"], side);
                assert_eq!(item["state"], "succeeded", "{item}");
                assert_eq!(
                    std::fs::read_to_string(p.join("base.txt")).unwrap(),
                    format!("{side}\n")
                );
                assert_eq!(
                    f.run(&bare, &["rev-parse", "main"]),
                    f.run(&p, &["rev-parse", "HEAD"])
                );
                assert_eq!(
                    f.run(&p, &["rev-list", "--parents", "-n", "1", "HEAD"])
                        .split(' ')
                        .count(),
                    3
                );
            }
        }
    }
}
#[tokio::test]
async fn sync_commits_overlapping_changes_before_taking_remote_work() {
    for policy in ["stop", "prefer_local"] {
        let f = Fixture::new();
        let p = f.repo("repo");
        let bare = f.remote(&p);
        let other = f.clone_repo(&bare, "other");
        let id = f.add(&p).await;
        f.core
            .update_general(json!({"sync":{"diverged":policy}}))
            .await
            .unwrap();
        f.write_commit(&other, "base.txt", "remote\n");
        f.run(&other, &["push"]);
        f.run(&p, &["fetch"]);
        std::fs::write(p.join("base.txt"), "local\n").unwrap();
        let before = f.run(&p, &["rev-parse", "HEAD"]);
        let item = f.op("sync", &id, json!({})).await;
        if policy == "stop" {
            assert_eq!(item["state"], "needs_confirmation", "{item}");
            assert_eq!(item["reason"], "dirty_worktree");
            assert_eq!(f.run(&p, &["rev-parse", "HEAD"]), before);
            assert_eq!(
                std::fs::read_to_string(p.join("base.txt")).unwrap(),
                "local\n"
            );
        } else {
            assert_eq!(item["state"], "succeeded", "{item}");
            let done = item["completed"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["code"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(done, ["fetch", "commit", "pull", "push"]);
            assert_eq!(
                std::fs::read_to_string(p.join("base.txt")).unwrap(),
                "local\n"
            );
            assert_eq!(
                f.run(&bare, &["rev-parse", "main"]),
                f.run(&p, &["rev-parse", "HEAD"])
            );
        }
    }
}
#[tokio::test]
async fn divergence_times_and_pull_with_a_preferred_side() {
    for side in ["local", "remote"] {
        let f = Fixture::new();
        let p = f.repo("repo");
        let bare = f.remote(&p);
        let other = f.clone_repo(&bare, "other");
        let id = f.add(&p).await;
        assert!(f.state(&id).await["divergence"].is_null());
        std::fs::write(other.join("base.txt"), "remote\n").unwrap();
        f.run(&other, &["add", "--", "base.txt"]);
        let old = Command::new(&f.git.executable)
            .current_dir(&other)
            .envs(&f.git.env)
            .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00Z")
            .args(["commit", "-m", "old remote change"])
            .output()
            .unwrap();
        assert!(old.status.success());
        f.run(&other, &["push"]);
        f.run(&p, &["fetch"]);
        std::fs::write(p.join("base.txt"), "local\n").unwrap();
        let dirty = f.state(&id).await["divergence"].clone();
        assert_eq!(dirty["remoteAt"], 1_577_836_800_000u64);
        assert!(dirty["localAt"].as_u64().unwrap() > 1_577_836_800_000);
        f.run(&p, &["commit", "-am", "local change"]);
        let state = f.state(&id).await;
        assert_eq!(state["state"], "diverged");
        let divergence = &state["divergence"];
        assert!(divergence["localAt"].as_u64().unwrap() > divergence["remoteAt"].as_u64().unwrap());
        for bad in [
            json!({"mode":"ff_only","prefer":side}),
            json!({"mode":"rebase","prefer":side}),
            json!({"mode":"merge","prefer":"both"}),
        ] {
            let mut item = json!({"itemId":id,"projectId":id,"fingerprint":"x"});
            for (k, v) in bad.as_object().unwrap() {
                item[k] = v.clone();
            }
            assert_eq!(
                f.core
                    .start(json!({"kind":"pull","items":[item]}))
                    .await
                    .unwrap_err()
                    .code,
                "invalid_input"
            );
        }
        let request = json!({"kind":"pull","items":[{"itemId":id,"projectId":id,"mode":"merge","prefer":side}]});
        let plan = f.core.plan(request).await.unwrap();
        let _: revia_lib::dto::OperationPlan = serde_json::from_value(plan.clone()).unwrap();
        let planned = &plan["items"][0];
        assert_eq!(planned["steps"][1]["prefer"], side);
        assert!(
            planned["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["code"] == "conflicts_resolved" && w["prefer"] == side)
        );
        let item = f
            .op(
                "pull",
                &id,
                json!({"mode":"merge","prefer":side,"fingerprint":planned["fingerprint"]}),
            )
            .await;
        assert_eq!(item["state"], "succeeded", "{item}");
        assert_eq!(item["completed"][1]["prefer"], side);
        assert_eq!(
            std::fs::read_to_string(p.join("base.txt")).unwrap(),
            format!("{side}\n")
        );
    }
}
#[tokio::test]
async fn sync_diverged_push_rejected_and_fingerprint() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let bare = f.remote(&p);
    let other = f.clone_repo(&bare, "other");
    let id = f.add(&p).await;
    let request = json!({"kind":"sync","items":[{"itemId":id,"projectId":id}]});
    let plan = f.core.plan(request.clone()).await.unwrap();
    let fingerprint = plan["items"][0]["fingerprint"].clone();
    f.write_commit(&p, "local.txt", "local");
    let item = f.op("sync", &id, json!({"fingerprint":fingerprint})).await;
    assert_eq!(item["reason"], "state_changed");
    assert_eq!(item["state"], "needs_confirmation");
    f.write_commit(&other, "remote.txt", "remote");
    f.run(&other, &["push"]);
    let item = f
        .op("push", &id, json!({"force":false,"setUpstream":false}))
        .await;
    assert_eq!(item["state"], "failed");
    assert_eq!(item["error"]["code"], "push_rejected");
    let state = f.state(&id).await;
    assert!(state["behind"].as_u64().unwrap() > 0);
    assert!(state["ahead"].as_u64().unwrap() > 0);
    let before = f.run(&p, &["rev-parse", "HEAD"]);
    let item = f.op("sync", &id, json!({})).await;
    assert_eq!(item["reason"], "diverged");
    assert_eq!(f.run(&p, &["rev-parse", "HEAD"]), before);
    assert_eq!(item["completed"][0]["code"], "fetch");
}
#[tokio::test]
async fn commit_all_selected_japanese_and_skip() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    std::fs::write(p.join("日本語.txt"), "こんにちは\n").unwrap();
    std::fs::write(p.join("staged.txt"), "stay staged\n").unwrap();
    f.run(&p, &["add", "staged.txt"]);
    let item = f
        .op(
            "commit",
            &id,
            json!({"message":"選択","paths":["日本語.txt"]}),
        )
        .await;
    assert_eq!(item["state"], "succeeded", "{item}");
    assert_eq!(
        f.run(&p, &["diff", "--cached", "--name-only"]),
        "staged.txt"
    );
    assert_eq!(f.run(&p, &["show", "HEAD:日本語.txt"]), "こんにちは");
    assert_eq!(
        f.op("commit", &id, json!({"message":"all","paths":null}))
            .await["state"],
        "succeeded"
    );
    assert_eq!(
        f.op("commit", &id, json!({"message":"empty","paths":null}))
            .await["state"],
        "skipped"
    );
    assert!(f.core.start(json!({"kind":"commit","items":[{"itemId":id,"projectId":id,"message":"","paths":null}]})).await.is_err());
}
#[tokio::test]
async fn pull_modes_and_conflicts() {
    for mode in ["ff_only", "merge", "rebase"] {
        let f = Fixture::new();
        let p = f.repo("repo");
        let bare = f.remote(&p);
        let other = f.clone_repo(&bare, "other");
        let id = f.add(&p).await;
        f.write_commit(&other, "next.txt", "next");
        f.run(&other, &["push"]);
        let request = json!({"kind":"pull","items":[{"itemId":id,"projectId":id,"mode":mode}]});
        let plan = f.core.plan(request).await.unwrap();
        let item = f
            .op(
                "pull",
                &id,
                json!({"mode":mode,"fingerprint":plan["items"][0]["fingerprint"]}),
            )
            .await;
        assert_eq!(item["state"], "succeeded", "{mode}: {item}");
        assert!(p.join("next.txt").exists());
    }
    for mode in ["merge", "rebase"] {
        let f = Fixture::new();
        let p = f.repo("repo");
        let bare = f.remote(&p);
        let other = f.clone_repo(&bare, "other");
        let id = f.add(&p).await;
        f.write_commit(&p, "base.txt", "local\n");
        f.write_commit(&other, "base.txt", "remote\n");
        f.run(&other, &["push"]);
        let plan = f
            .core
            .plan(json!({"kind":"pull","items":[{"itemId":id,"projectId":id,"mode":mode}]}))
            .await
            .unwrap();
        let item = f
            .op(
                "pull",
                &id,
                json!({"mode":mode,"fingerprint":plan["items"][0]["fingerprint"]}),
            )
            .await;
        assert_eq!(item["state"], "conflict", "{item}");
        assert_eq!(f.state(&id).await["state"], "conflicted");
        assert_eq!(
            f.core.conflicts(&id).await.unwrap()["files"][0]["hasMarkers"],
            true
        );
    }
}
#[tokio::test]
async fn preview_is_read_only_batch_independence_cancel() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    let p2 = f.repo("second");
    let id2 = f.add(&p2).await;
    std::fs::write(p2.join("new.txt"), "new").unwrap();
    let before = f.run(&p2, &["status", "--porcelain=v1"]);
    let refs = f.run(&p2, &["show-ref"]);
    let request = json!({"kind":"commit","items":[{"itemId":id,"projectId":id,"message":"one","paths":null},{"itemId":id2,"projectId":id2,"message":"two","paths":null}]});
    f.core.plan(request.clone()).await.unwrap();
    assert_eq!(f.run(&p2, &["status", "--porcelain=v1"]), before);
    assert_eq!(f.run(&p2, &["show-ref"]), refs);
    let snap = f.core.start(request.clone()).await.unwrap();
    let result = f.wait(&snap).await;
    assert_eq!(result["items"][0]["state"], "skipped");
    assert_eq!(result["items"][1]["state"], "succeeded");
    let snap = f.core.start(request).await.unwrap();
    f.core
        .cancel(snap["operationId"].as_str().unwrap())
        .unwrap();
    let result = f.wait(&snap).await;
    assert!(
        result["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["state"] == "cancelled")
    );
    let snap=f.core.start(json!({"kind":"fetch","items":[{"itemId":id,"projectId":id},{"itemId":id2,"projectId":id2}]})).await.unwrap();
    let result = f.wait(&snap).await;
    assert!(
        result["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["reason"] == "no_remote")
    );
}
#[tokio::test]
async fn diff_variants_binary_truncation_and_details() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    std::fs::write(p.join("base.txt"), "modified\n").unwrap();
    std::fs::write(p.join("追加.txt"), "new\n").unwrap();
    std::fs::write(p.join("binary"), [0, 1, 2]).unwrap();
    std::fs::write(p.join("large"), "line\n".repeat(6000)).unwrap();
    let files = f.core.changes(&id).await.unwrap();
    assert_eq!(
        files.iter().find(|f| f["path"] == "追加.txt").unwrap()["kind"],
        "untracked"
    );
    let diff = f
        .core
        .diff(&id, "base.txt".into(), json!({"kind":"worktree"}))
        .await
        .unwrap();
    assert!(!diff["hunks"].as_array().unwrap().is_empty());
    assert_eq!(
        f.core
            .diff(&id, "binary".into(), json!({"kind":"worktree"}))
            .await
            .unwrap()["binary"],
        true
    );
    assert_eq!(
        f.core
            .diff(&id, "large".into(), json!({"kind":"worktree"}))
            .await
            .unwrap()["truncated"],
        true
    );
    assert!(
        f.core
            .diff(&id, "../outside".into(), json!({"kind":"worktree"}))
            .await
            .is_err()
    );
    f.run(&p, &["add", "-A"]);
    f.run(&p, &["commit", "-m", "files"]);
    f.run(&p, &["mv", "追加.txt", "renamed.txt"]);
    std::fs::remove_file(p.join("base.txt")).unwrap();
    let files = f.core.changes(&id).await.unwrap();
    assert!(files.iter().any(|f| f["kind"] == "renamed"));
    assert_eq!(f.core.conflicts(&id).await.unwrap()["files"], json!([]));
    assert!(files.iter().any(|f| f["kind"] == "deleted"));
    let log = f.core.commit_log(&id, None, 10, 0).await.unwrap();
    assert_eq!(log.len(), 2);
    assert!(
        !f.core
            .commit_detail(&id, log[0]["sha"].as_str().unwrap().into())
            .await
            .unwrap()["files"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        f.core.detail(&id).await.unwrap()["branches"][0]["name"],
        "main"
    );
}
#[tokio::test]
async fn url_redaction_and_invalid_transport() {
    let f = Fixture::new();
    let p = f.repo("repo");
    f.run(
        &p,
        &[
            "remote",
            "add",
            "origin",
            "https://user:token@host.invalid/repo",
        ],
    );
    let id = f.add(&p).await;
    let status = f.state(&id).await;
    let raw = serde_json::to_string(&status).unwrap();
    assert!(!raw.contains("user:token"));
    assert!(raw.contains("https://host.invalid/repo"));
    let clean =
        revia_lib::error::AppError::new("unknown").detail("fatal: https://user:token@host/repo");
    assert!(!clean.detail.unwrap().contains("token"));
    f.run(&p, &["remote", "set-url", "origin", "ext::danger"]);
    let item = f.op("fetch", &id, json!({})).await;
    assert_eq!(item["state"], "failed");
    let history = f.core.history(10, None, None).await.unwrap();
    assert!(!serde_json::to_string(&history).unwrap().contains("token"));
}

#[tokio::test]
async fn dto_compatibility_unborn_init_and_in_progress() {
    let f = Fixture::new();
    let p = f.dir("empty");
    let id = f.add(&p).await;
    let _: revia_lib::dto::Workspace = serde_json::from_value(f.core.workspace()).unwrap();
    assert_eq!(
        f.core.commit_log(&id, None, 10, 0).await.unwrap(),
        Vec::<Value>::new()
    );
    f.core.init(&id, None).await.unwrap();
    assert_eq!(f.state(&id).await["unborn"], true);
    std::fs::write(p.join("first.txt"), "first\n").unwrap();
    let item = f
        .op("commit", &id, json!({"message":"initial","paths":null}))
        .await;
    assert_eq!(item["state"], "succeeded");
    let _: revia_lib::dto::ProjectStatus = serde_json::from_value(f.state(&id).await).unwrap();
    let _: revia_lib::dto::ProjectDetail =
        serde_json::from_value(f.core.detail(&id).await.unwrap()).unwrap();
    let _: revia_lib::dto::FileDiff = serde_json::from_value(
        f.core
            .diff(&id, "first.txt".into(), json!({"kind":"worktree"}))
            .await
            .unwrap(),
    )
    .unwrap();
    let plan = f
        .core
        .plan(json!({"kind":"sync","items":[{"itemId":id,"projectId":id}]}))
        .await
        .unwrap();
    let _: revia_lib::dto::OperationPlan = serde_json::from_value(plan).unwrap();
    let head = f.run(&p, &["rev-parse", "HEAD"]);
    std::fs::write(p.join(".git/MERGE_HEAD"), head).unwrap();
    assert_eq!(f.state(&id).await["state"], "in_progress");
    assert_eq!(f.core.conflicts(&id).await.unwrap()["canContinue"], true);
}

#[tokio::test]
async fn batch_failure_does_not_stop_other_item() {
    let f = Fixture::new();
    let p = f.repo("bad");
    let good = f.repo("good");
    let bare = f.remote(&good);
    let id = f.add(&p).await;
    let good_id = f.add(&good).await;
    f.run(&p, &["remote", "add", "origin", "ext::forbidden"]);
    let snapshot=f.core.start(json!({"kind":"fetch","items":[{"itemId":id,"projectId":id},{"itemId":good_id,"projectId":good_id}]})).await.unwrap();
    let result = f.wait(&snapshot).await;
    assert_eq!(result["items"][0]["state"], "failed");
    assert_eq!(result["items"][1]["state"], "succeeded");
    let _: revia_lib::dto::OperationSnapshot = serde_json::from_value(result).unwrap();
    assert!(bare.is_dir());
    assert_eq!(f.core.history(10, None, None).await.unwrap().len(), 2);
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_replacement_and_relative_symlink_rejected() {
    let f = Fixture::new();
    let p = f.repo("registered");
    let other = f.repo("outside");
    let id = f.add(&p).await;
    std::os::unix::fs::symlink(other.join("base.txt"), p.join("link.txt")).unwrap();
    let diff = f
        .core
        .diff(&id, "link.txt".into(), json!({"kind":"worktree"}))
        .await
        .unwrap();
    assert!(!diff["hunks"].as_array().unwrap().is_empty());
    std::os::unix::fs::symlink(&other, p.join("directory-link")).unwrap();
    assert!(
        f.core
            .diff(
                &id,
                "directory-link/base.txt".into(),
                json!({"kind":"worktree"})
            )
            .await
            .is_err()
    );
    let moved = f.temp.path().join("moved");
    std::fs::rename(&p, &moved).unwrap();
    std::os::unix::fs::symlink(&other, &p).unwrap();
    assert_eq!(f.core.project(&id).unwrap_err().code, "invalid_path");
    let state = f.state(&id).await;
    assert_eq!(state["state"], "unknown");
    assert_eq!(state["error"]["code"], "invalid_path");
}

#[tokio::test]
async fn partial_commit_keeps_other_index_and_worktree_versions() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    std::fs::write(p.join("base.txt"), "staged version\n").unwrap();
    f.run(&p, &["add", "base.txt"]);
    std::fs::write(p.join("base.txt"), "worktree version\n").unwrap();
    std::fs::write(p.join("selected.txt"), "selected\n").unwrap();
    let before = f.run(&p, &["show", ":base.txt"]);
    assert_eq!(
        f.op(
            "commit",
            &id,
            json!({"message":"only","paths":["selected.txt"]})
        )
        .await["state"],
        "succeeded"
    );
    assert_eq!(f.run(&p, &["show", ":base.txt"]), before);
    assert_eq!(
        std::fs::read_to_string(p.join("base.txt")).unwrap(),
        "worktree version\n"
    );
    assert_eq!(f.run(&p, &["show", "HEAD:base.txt"]), "base");
}

#[cfg(unix)]
fn mocked_git(f: &Fixture, body: &str) -> Git {
    let script = f.temp.path().join("mock-git");
    let real = f.git.executable.to_string_lossy().replace('\'', "'\\''");
    std::fs::write(&script,format!("#!/bin/sh\nfor arg do\nif [ \"$arg\" = fetch ]; then\n{body}\nfi\ndone\nexec '{real}' \"$@\"\n")).unwrap();
    common::executable(&script, 0o700);
    let mut git = f.git.clone();
    git.executable = script;
    git
}

#[cfg(unix)]
#[tokio::test]
async fn network_error_is_redacted_in_result_and_history() {
    let f = Fixture::new();
    let p = f.repo("repo");
    f.run(
        &p,
        &["remote", "add", "origin", "https://host.invalid/repo"],
    );
    let id = f.add(&p).await;
    let git = mocked_git(
        &f,
        "printf '%s\\n' 'fatal: Authentication failed for https://user:token@host.invalid/repo' >&2\nexit 1",
    );
    let core = Fixture::core(f.temp.path(), git);
    let snapshot = core
        .start(json!({"kind":"fetch","items":[{"itemId":id,"projectId":id}]}))
        .await
        .unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let s = core
                .operation(snapshot["operationId"].as_str().unwrap())
                .unwrap();
            if s["state"] == "finished" {
                break s;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(result["items"][0]["error"]["code"], "auth_failed");
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("user:token")
    );
    let status = &core.workspace()["projects"][0]["status"];
    assert_eq!(status["remoteError"], "auth");
    assert_eq!(status["state"], "auth_error");
    let history = core.history(10, None, None).await.unwrap();
    assert!(
        !serde_json::to_string(&history)
            .unwrap()
            .contains("user:token")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn running_network_process_is_cancelled_without_completed_fetch() {
    let f = Fixture::new();
    let p = f.repo("repo");
    f.remote(&p);
    let id = f.add(&p).await;
    let marker = f.temp.path().join("fetch-started");
    let marker_text = marker.to_string_lossy().replace('\'', "'\\''");
    let git = mocked_git(
        &f,
        &format!("printf started > '{marker_text}'\nexec /bin/sleep 30"),
    );
    let core = Fixture::core(f.temp.path(), git);
    let snap = core
        .start(json!({"kind":"fetch","items":[{"itemId":id,"projectId":id}]}))
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !marker.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let operation = snap["operationId"].as_str().unwrap();
    core.cancel(operation).unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let s = core.operation(operation).unwrap();
            if s["state"] == "finished" {
                break s;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(result["items"][0]["state"], "cancelled");
    assert_eq!(result["items"][0]["completed"], json!([]));
}

#[tokio::test]
async fn safety_validation_and_clean_ahead() {
    let f = Fixture::new();
    let p = f.repo("repo");
    f.remote(&p);
    let id = f.add(&p).await;
    f.write_commit(&p, "ahead.txt", "ahead");
    assert_eq!(f.state(&id).await["state"], "ahead");
    for mode in ["merge", "rebase"] {
        assert_eq!(
            f.core
                .start(json!({"kind":"pull","items":[{"itemId":id,"projectId":id,"mode":mode}]}))
                .await
                .unwrap_err()
                .code,
            "invalid_input"
        );
    }
    assert_eq!(
        f.core
            .start(json!({"kind":"merge","items":[]}))
            .await
            .unwrap_err()
            .code,
        "invalid_input"
    );
    assert_eq!(
        f.op(
            "push",
            &id,
            json!({"force":true,"setUpstream":false,"fingerprint":"stale"})
        )
        .await["reason"],
        "state_changed"
    );
    let request = json!({"kind":"push","items":[{"itemId":id,"projectId":id,"force":true,"setUpstream":false}]});
    let plan = f.core.plan(request).await.unwrap();
    assert_eq!(plan["items"][0]["blocker"], "feature_disabled");
    assert_eq!(
        f.op(
            "push",
            &id,
            json!({"force":true,"setUpstream":false,"fingerprint":plan["items"][0]["fingerprint"]})
        )
        .await["reason"],
        "feature_disabled"
    );
    f.core
        .set_target(
            &id,
            "main",
            "push",
            json!({"remote":"gone","branch":"main"}),
        )
        .await
        .unwrap();
    assert_eq!(
        f.op("push", &id, json!({"force":false,"setUpstream":false}))
            .await["reason"],
        "target_invalid"
    );
    let missing = f.dir("missing");
    let mid = f.add(&missing).await;
    std::fs::remove_dir(&missing).unwrap();
    assert_eq!(f.op("sync", &mid, json!({})).await["reason"], "missing");
}

#[tokio::test]
async fn committed_rename_full_counts_and_reverted_worktree_skip() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    f.run(&p, &["mv", "base.txt", "renamed.txt"]);
    f.run(&p, &["commit", "-m", "rename"]);
    let diff = f
        .core
        .diff(
            &id,
            "renamed.txt".into(),
            json!({"kind":"commit","sha":"HEAD"}),
        )
        .await
        .unwrap();
    assert_eq!(diff["oldPath"], "base.txt");
    let detail = f.core.commit_detail(&id, "HEAD".into()).await.unwrap();
    assert_eq!(detail["files"][0]["kind"], "renamed");
    assert_eq!(detail["files"][0]["additions"], 0);
    std::fs::write(p.join("renamed.txt"), "staged\n").unwrap();
    f.run(&p, &["add", "renamed.txt"]);
    std::fs::write(p.join("renamed.txt"), "base\n").unwrap();
    let head = f.run(&p, &["rev-parse", "HEAD"]);
    assert_eq!(
        f.op(
            "commit",
            &id,
            json!({"message":"same","paths":["renamed.txt"]})
        )
        .await["state"],
        "skipped"
    );
    assert_eq!(f.run(&p, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        f.op("commit", &id, json!({"message":"same all","paths":null}))
            .await["state"],
        "skipped"
    );
    std::fs::write(p.join("large.txt"), "line\n".repeat(6000)).unwrap();
    f.run(&p, &["add", "large.txt"]);
    f.run(&p, &["commit", "-m", "large"]);
    let detail = f.core.commit_detail(&id, "HEAD".into()).await.unwrap();
    assert_eq!(detail["files"][0]["additions"], 6000);
}

#[tokio::test]
async fn review_sync_fetch_redaction_and_history() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let bare = f.remote(&p);
    let id = f.add(&p).await;
    f.run(&p, &["checkout", "-b", "new"]);
    f.write_commit(&p, "new.txt", "token=visible\n");
    f.run(
        &p,
        &["remote", "add", "broken", "/nonexistent/revia-remote"],
    );
    let item = f.op("sync", &id, json!({})).await;
    assert_eq!(item["state"], "succeeded", "{item}");
    assert!(!f.run(&bare, &["rev-parse", "refs/heads/new"]).is_empty());
    let history = f.core.history(10, None, None).await.unwrap();
    assert_eq!(history[0]["params"]["branch"], "new");
    assert_eq!(history[0]["params"]["target"], "origin/new");
    let fetch = f.op("fetch", &id, json!({})).await;
    assert_eq!(fetch["state"], "failed");
    assert_eq!(fetch["completed"], json!([]));
    std::fs::write(p.join("new.txt"), "token=changed\n").unwrap();
    let diff = f
        .core
        .diff(&id, "new.txt".into(), json!({"kind":"worktree"}))
        .await
        .unwrap();
    assert!(diff.to_string().contains("token=changed"));
    f.run(&p, &["add", "new.txt"]);
    f.run(&p, &["commit", "-m", "token=message"]);
    assert_eq!(
        f.core.commit_log(&id, None, 1, 0).await.unwrap()[0]["subject"],
        "token=message"
    );
}
#[test]
fn review_urls_and_progress() {
    assert!(
        revia_lib::git::validate_url(&std::env::temp_dir().join("repo").to_string_lossy()).is_ok()
    );
    for u in [
        "http://host/r",
        "git://host/r",
        "host:path",
        "user@host:path",
        "file:///tmp/r",
        "ssh://git@host/r",
    ] {
        assert!(revia_lib::git::validate_url(u).is_ok(), "{u}");
    }
    for u in [
        "ext::command",
        "fd::1",
        "-host:path",
        "https://host/\nsecret",
        "relative/path",
        "ftp://host/repo",
        "custom://host/repo",
        "",
    ] {
        assert!(revia_lib::git::validate_url(u).is_err(), "{u}");
    }
    assert_eq!(
        revia_lib::git::parse_progress("Receiving objects:  37% (1/3)"),
        Some(0.37)
    );
    assert_eq!(
        revia_lib::git::parse_progress("Writing objects: 100%"),
        Some(1.0)
    );
}
#[tokio::test]
async fn review_refresh_order_and_large_untracked() {
    let f = Fixture::new();
    let mut ids = vec![];
    for i in 0..10 {
        let p = f.repo(&format!("repo{i}"));
        ids.push(f.add(&p).await);
    }
    let projects = f.core.refresh(None).await.unwrap();
    assert_eq!(
        projects
            .iter()
            .map(|p| p["id"].as_str().unwrap().to_string())
            .collect::<Vec<_>>(),
        ids
    );
    let p = Path::new(projects[0]["path"].as_str().unwrap());
    std::fs::write(p.join("large"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
    let changes = f.core.changes(&ids[0]).await.unwrap();
    assert!(changes.iter().find(|c| c["path"] == "large").unwrap()["additions"].is_null());
    assert_eq!(
        f.core
            .diff(&ids[0], "large".into(), json!({"kind":"worktree"}))
            .await
            .unwrap()["truncated"],
        true
    );
}
#[cfg(unix)]
#[tokio::test]
async fn review_commit_symlink_leaf() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    std::os::unix::fs::symlink("base.txt", p.join("link")).unwrap();
    assert_eq!(
        f.op("commit", &id, json!({"message":"link","paths":["link"]}))
            .await["state"],
        "succeeded"
    );
    assert_eq!(f.run(&p, &["show", "HEAD:link"]), "base.txt");
    std::os::unix::fs::symlink("missing-target", p.join("dangling")).unwrap();
    assert_eq!(
        f.op(
            "commit",
            &id,
            json!({"message":"dangling","paths":["dangling"]})
        )
        .await["state"],
        "succeeded"
    );
    assert_eq!(f.run(&p, &["show", "HEAD:dangling"]), "missing-target");
}

#[cfg(unix)]
#[tokio::test]
async fn review_helper_scope_and_optional_gh() {
    let f = Fixture::new();
    let p = f.repo("repo");
    f.run(&p, &["remote", "add", "origin", "https://github.com/o/r"]);
    let args = f.temp.path().join("args");
    let body = format!("printf '%s\\n' \"$@\" > '{}'\nexit 0", args.display());
    let mut git = mocked_git(&f, &body);
    git.gh = f.temp.path().join("no-gh");
    git.run(&p, &["fetch".into(), "origin".into()], true, None)
        .await
        .unwrap();
    assert!(
        !std::fs::read_to_string(&args)
            .unwrap()
            .contains("credential.")
    );
    git.gh = f.git.executable.clone();
    git.run(&p, &["fetch".into(), "origin".into()], true, None)
        .await
        .unwrap();
    let output = std::fs::read_to_string(&args).unwrap();
    assert!(output.contains("credential.https://github.com.helper="));
    assert!(!output.contains("credential.helper="));
}

#[tokio::test]
async fn branches_remotes_settings_and_upstream() {
    let f = Fixture::new();
    let p = f.repo("repo");
    f.remote(&p);
    let id = f.add(&p).await;
    for name in ["-bad", "bad..name"] {
        assert_eq!(
            f.core
                .mutate(&id, "create_branch", json!({"name":name,"checkout":false}))
                .await
                .unwrap_err()
                .code,
            "invalid_ref_name"
        );
    }
    f.core
        .mutate(
            &id,
            "create_branch",
            json!({"name":"feature","checkout":false}),
        )
        .await
        .unwrap();
    assert_eq!(
        f.core
            .mutate(
                &id,
                "create_branch",
                json!({"name":"feature","checkout":false})
            )
            .await
            .unwrap_err()
            .code,
        "branch_exists"
    );
    f.core
        .set_target(
            &id,
            "feature",
            "push",
            json!({"remote":"origin","branch":"feature"}),
        )
        .await
        .unwrap();
    f.core
        .mutate(
            &id,
            "rename_branch",
            json!({"from":"feature","to":"renamed"}),
        )
        .await
        .unwrap();
    let settings = f.core.project(&id).unwrap()["settings"].clone();
    assert!(settings["branchTargets"]["feature"].is_null());
    assert_eq!(
        settings["branchTargets"]["renamed"]["push"]["remote"],
        "origin"
    );
    f.core
        .update_project(&id, json!({"defaultRemote":"origin"}))
        .await
        .unwrap();
    f.core
        .mutate(
            &id,
            "rename_remote",
            json!({"from":"origin","to":"upstream"}),
        )
        .await
        .unwrap();
    let settings = f.core.project(&id).unwrap()["settings"].clone();
    assert_eq!(settings["defaultRemote"], "upstream");
    assert_eq!(
        settings["branchTargets"]["renamed"]["push"]["remote"],
        "upstream"
    );
    f.core
        .mutate(
            &id,
            "set_upstream",
            json!({"branch":"renamed","upstream":{"remote":"upstream","branch":"main"}}),
        )
        .await
        .unwrap();
    assert_eq!(
        f.run(&p, &["rev-parse", "--abbrev-ref", "renamed@{upstream}"]),
        "upstream/main"
    );
    f.core
        .mutate(
            &id,
            "set_upstream",
            json!({"branch":"renamed","upstream":null}),
        )
        .await
        .unwrap();
    assert_eq!(
        f.core
            .mutate(
                &id,
                "add_remote",
                json!({"name":"upstream","url":f.temp.path().join("x").to_string_lossy()})
            )
            .await
            .unwrap_err()
            .code,
        "remote_exists"
    );
    f.core
        .mutate(
            &id,
            "add_remote",
            json!({"name":"second","url":"http://host/repo"}),
        )
        .await
        .unwrap();
    f.core
        .mutate(
            &id,
            "set_remote_url",
            json!({"name":"second","url":"host:repo"}),
        )
        .await
        .unwrap();
    f.core
        .mutate(&id, "remove_remote", json!({"name":"upstream"}))
        .await
        .unwrap();
    assert_eq!(
        f.core.project(&id).unwrap()["settings"]["defaultRemote"],
        "upstream"
    );
    assert_eq!(
        f.core
            .mutate(&id, "delete_branch", json!({"name":"main","force":true}))
            .await
            .unwrap_err()
            .code,
        "invalid_input"
    );
    f.core
        .mutate(
            &id,
            "checkout_branch",
            json!({"branch":"renamed","ifDirty":"carry"}),
        )
        .await
        .unwrap();
    f.write_commit(&p, "feature.txt", "feature");
    f.core
        .mutate(
            &id,
            "checkout_branch",
            json!({"branch":"main","ifDirty":"abort"}),
        )
        .await
        .unwrap();
    assert_eq!(
        f.core
            .mutate(
                &id,
                "delete_branch",
                json!({"name":"renamed","force":false})
            )
            .await
            .unwrap_err()
            .code,
        "branch_not_merged"
    );
    f.core
        .mutate(&id, "delete_branch", json!({"name":"renamed","force":true}))
        .await
        .unwrap();
}
#[tokio::test]
async fn checkout_dirty_strategies_remote_and_stash_rollback() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let bare = f.remote(&p);
    let id = f.add(&p).await;
    f.core
        .mutate(
            &id,
            "create_branch",
            json!({"name":"other","checkout":false}),
        )
        .await
        .unwrap();
    std::fs::write(p.join("base.txt"), "dirty").unwrap();
    assert_eq!(
        f.core
            .mutate(
                &id,
                "checkout_branch",
                json!({"branch":"other","ifDirty":"abort"})
            )
            .await
            .unwrap_err()
            .code,
        "dirty_worktree"
    );
    f.core
        .mutate(
            &id,
            "checkout_branch",
            json!({"branch":"other","ifDirty":"carry"}),
        )
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(p.join("base.txt")).unwrap(),
        "dirty"
    );
    f.core
        .mutate(
            &id,
            "checkout_branch",
            json!({"branch":"main","ifDirty":"stash"}),
        )
        .await
        .unwrap();
    assert_eq!(f.run(&p, &["status", "--porcelain"]), "");
    assert!(!f.run(&p, &["stash", "list"]).is_empty());
    let clone = f.clone_repo(&bare, "other-clone");
    f.run(&clone, &["checkout", "-b", "remote-feature"]);
    f.write_commit(&clone, "remote.txt", "remote");
    f.run(&clone, &["push", "-u", "origin", "remote-feature"]);
    f.run(&p, &["fetch", "origin"]);
    f.core
        .mutate(
            &id,
            "checkout_branch",
            json!({"branch":"origin/remote-feature","ifDirty":"abort"}),
        )
        .await
        .unwrap();
    assert_eq!(f.run(&p, &["branch", "--show-current"]), "remote-feature");
    f.core
        .mutate(
            &id,
            "delete_remote_branch",
            json!({"remote":"origin","branch":"remote-feature"}),
        )
        .await
        .unwrap();
    assert!(
        f.run(&bare, &["for-each-ref", "refs/heads/remote-feature"])
            .is_empty()
    );
}
#[tokio::test]
async fn merge_preview_modes_checkout_fingerprint_and_conflicts() {
    for mode in ["default", "no_ff", "ff_only"] {
        let f = Fixture::new();
        let p = f.repo("repo");
        let id = f.add(&p).await;
        f.run(&p, &["checkout", "-b", "feature"]);
        f.write_commit(&p, "feature.txt", "feature");
        let refs = f.run(&p, &["show-ref"]);
        let status = f.run(&p, &["status", "--porcelain"]);
        let index = std::fs::read(p.join(".git/index")).unwrap();
        let preview = f.core.preview_merge(&id, "feature", "main").await.unwrap();
        assert_eq!(preview["fastForward"], true);
        assert_eq!(preview["needsCheckout"], true);
        assert_eq!(preview["conflicts"]["status"], "none");
        assert_eq!(f.run(&p, &["show-ref"]), refs);
        assert_eq!(f.run(&p, &["status", "--porcelain"]), status);
        assert_eq!(std::fs::read(p.join(".git/index")).unwrap(), index);
        let request = json!({"kind":"merge","items":[{"itemId":id,"projectId":id,"source":"feature","target":"main","mode":mode}]});
        let plan = f.core.plan(request).await.unwrap();
        assert_eq!(plan["items"][0]["fingerprint"], preview["fingerprint"]);
        let item=f.op("merge",&id,json!({"source":"feature","target":"main","mode":mode,"fingerprint":preview["fingerprint"]})).await;
        assert_eq!(item["state"], "succeeded", "{item}");
        assert_eq!(f.run(&p, &["branch", "--show-current"]), "main");
        let parents = f.run(&p, &["show", "-s", "--format=%P", "HEAD"]);
        assert_eq!(
            parents.split_whitespace().count(),
            if mode == "no_ff" { 2 } else { 1 }
        );
        assert_eq!(
            f.core.preview_merge(&id, "feature", "main").await.unwrap()["upToDate"],
            true
        );
    }
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    f.run(&p, &["checkout", "-b", "feature"]);
    f.write_commit(&p, "base.txt", "theirs\n");
    f.run(&p, &["checkout", "main"]);
    f.write_commit(&p, "base.txt", "ours\n");
    let preview = f.core.preview_merge(&id, "feature", "main").await.unwrap();
    assert_eq!(preview["fastForward"], false);
    assert_eq!(preview["conflicts"]["status"], "conflicts");
    let stale = f
        .op(
            "merge",
            &id,
            json!({"source":"feature","target":"main","mode":"default","fingerprint":"stale"}),
        )
        .await;
    assert_eq!(stale["reason"], "state_changed");
    let item=f.op("merge",&id,json!({"source":"feature","target":"main","mode":"default","fingerprint":preview["fingerprint"]})).await;
    assert_eq!(item["state"], "conflict", "{item}");
    let conflict = f.core.conflicts(&id).await.unwrap();
    assert_eq!(conflict["incoming"], "feature");
    assert_eq!(
        f.core
            .mutate(&id, "continue_in_progress", json!({}))
            .await
            .unwrap_err()
            .code,
        "merge_conflict"
    );
    std::fs::write(p.join("base.txt"), "resolved\n").unwrap();
    f.core
        .mutate(&id, "mark_resolved", json!({"paths":["base.txt"]}))
        .await
        .unwrap();
    f.core
        .mutate(&id, "continue_in_progress", json!({}))
        .await
        .unwrap();
    assert!(f.state(&id).await["inProgress"].is_null());
}
#[tokio::test]
async fn reset_rebase_cherry_pick_and_fingerprints() {
    for mode in ["soft", "mixed", "hard"] {
        let f = Fixture::new();
        let p = f.repo("repo");
        let id = f.add(&p).await;
        let base = f.run(&p, &["rev-parse", "HEAD"]);
        f.write_commit(&p, "next.txt", "next");
        let mut preview = f.core.preview_reset(&id, &base, mode).await.unwrap();
        assert_eq!(preview["commitCount"], 1);
        assert_eq!(preview["unpushedCount"], 1);
        if mode == "hard" {
            assert_eq!(
                f.core
                    .mutate(
                        &id,
                        "reset",
                        json!({"target":base,"mode":mode,"fingerprint":preview["fingerprint"]})
                    )
                    .await
                    .unwrap_err()
                    .code,
                "feature_disabled"
            );
            f.core
                .update_general(json!({"safety":{"allowHardReset":true}}))
                .await
                .unwrap();
            preview = f.core.preview_reset(&id, &base, mode).await.unwrap();
        }
        assert_eq!(
            f.core
                .mutate(
                    &id,
                    "reset",
                    json!({"target":base,"mode":mode,"fingerprint":"stale"})
                )
                .await
                .unwrap_err()
                .code,
            "state_changed"
        );
        f.core
            .mutate(
                &id,
                "reset",
                json!({"target":base,"mode":mode,"fingerprint":preview["fingerprint"]}),
            )
            .await
            .unwrap();
        assert_eq!(f.run(&p, &["rev-parse", "HEAD"]), base);
        if mode == "hard" {
            assert!(!p.join("next.txt").exists());
        } else if mode == "soft" {
            assert_eq!(f.run(&p, &["diff", "--cached", "--name-only"]), "next.txt");
        } else {
            assert!(p.join("next.txt").exists());
            assert_eq!(f.run(&p, &["diff", "--cached", "--name-only"]), "");
        }
    }
    let f = Fixture::new();
    let p = f.repo("repo");
    f.remote(&p);
    let id = f.add(&p).await;
    f.run(&p, &["checkout", "-b", "feature"]);
    f.write_commit(&p, "feature.txt", "feature");
    let sha = f.run(&p, &["rev-parse", "HEAD"]);
    f.run(&p, &["push", "origin", "feature"]);
    f.run(&p, &["checkout", "main"]);
    f.write_commit(&p, "main.txt", "main");
    f.run(&p, &["checkout", "feature"]);
    let preview = f.core.preview_rebase(&id, "main").await.unwrap();
    assert_eq!(preview["publishedCount"], 1);
    assert_eq!(preview["commitCount"], 1);
    assert_eq!(
        f.core
            .mutate(&id, "rebase", json!({"onto":"main","fingerprint":"stale"}))
            .await
            .unwrap_err()
            .code,
        "state_changed"
    );
    f.core
        .mutate(
            &id,
            "rebase",
            json!({"onto":"main","fingerprint":preview["fingerprint"]}),
        )
        .await
        .unwrap();
    assert!(p.join("main.txt").exists());
    f.run(&p, &["checkout", "main"]);
    f.core
        .mutate(&id, "cherry_pick", json!({"shas":[sha]}))
        .await
        .unwrap();
    assert!(p.join("feature.txt").exists());
}
#[tokio::test]
async fn discard_selected_untracked_ignored_and_stashes() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    std::fs::write(p.join(".gitignore"), "ignored\n").unwrap();
    f.run(&p, &["add", ".gitignore"]);
    f.run(&p, &["commit", "-m", "ignore"]);
    std::fs::write(p.join("base.txt"), "dirty").unwrap();
    std::fs::write(p.join("untracked"), "new").unwrap();
    std::fs::write(p.join("ignored"), "keep").unwrap();
    f.core
        .mutate(
            &id,
            "discard_changes",
            json!({"paths":["base.txt"],"includeUntracked":true}),
        )
        .await
        .unwrap();
    assert!(p.join("untracked").exists());
    assert_eq!(
        std::fs::read_to_string(p.join("base.txt")).unwrap(),
        "base\n"
    );
    f.core
        .mutate(
            &id,
            "discard_changes",
            json!({"paths":null,"includeUntracked":false}),
        )
        .await
        .unwrap();
    assert!(p.join("untracked").exists());
    f.core
        .mutate(
            &id,
            "discard_changes",
            json!({"paths":null,"includeUntracked":true}),
        )
        .await
        .unwrap();
    assert!(!p.join("untracked").exists());
    assert!(p.join("ignored").exists());
    std::fs::write(p.join("base.txt"), "stash\n").unwrap();
    f.core
        .mutate(
            &id,
            "stash_save",
            json!({"message":"save","includeUntracked":true}),
        )
        .await
        .unwrap();
    let sha = f.run(&p, &["rev-parse", "refs/stash"]);
    assert_eq!(
        f.core
            .mutate(
                &id,
                "stash_apply",
                json!({"stashSha":"0000000000000000000000000000000000000000","drop":true})
            )
            .await
            .unwrap_err()
            .code,
        "state_changed"
    );
    f.core
        .mutate(&id, "stash_apply", json!({"stashSha":sha,"drop":false}))
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(p.join("base.txt")).unwrap(),
        "stash\n"
    );
    f.core
        .mutate(&id, "stash_drop", json!({"stashSha":sha}))
        .await
        .unwrap();
    assert!(f.run(&p, &["stash", "list"]).is_empty());
}

#[tokio::test]
async fn rebase_and_cherry_pick_conflict_continue_and_abort() {
    for kind in ["rebase", "cherry_pick"] {
        for abort in [false, true] {
            let f = Fixture::new();
            let p = f.repo("repo");
            let id = f.add(&p).await;
            f.run(&p, &["checkout", "-b", "feature"]);
            f.write_commit(&p, "base.txt", "feature\n");
            let feature = f.run(&p, &["rev-parse", "HEAD"]);
            f.run(&p, &["checkout", "main"]);
            f.write_commit(&p, "base.txt", "main\n");
            if kind == "rebase" {
                f.run(&p, &["checkout", "feature"]);
            }
            let before = f.run(&p, &["rev-parse", "HEAD"]);
            let args = if kind == "rebase" {
                let preview = f.core.preview_rebase(&id, "main").await.unwrap();
                json!({"onto":"main","fingerprint":preview["fingerprint"]})
            } else {
                json!({"shas":[feature]})
            };
            assert_eq!(
                f.core.mutate(&id, kind, args).await.unwrap_err().code,
                "merge_conflict"
            );
            let conflicts = f.core.conflicts(&id).await.unwrap();
            assert_eq!(conflicts["operation"], kind);
            assert!(conflicts["incoming"].is_string());
            if abort {
                f.core
                    .mutate(&id, "abort_in_progress", json!({}))
                    .await
                    .unwrap();
                assert_eq!(f.run(&p, &["rev-parse", "HEAD"]), before);
            } else {
                std::fs::write(p.join("base.txt"), "resolved\n").unwrap();
                f.core
                    .mutate(&id, "mark_resolved", json!({"paths":["base.txt"]}))
                    .await
                    .unwrap();
                f.core
                    .mutate(&id, "continue_in_progress", json!({}))
                    .await
                    .unwrap();
            }
            assert!(f.state(&id).await["inProgress"].is_null());
        }
    }
}
#[tokio::test]
async fn merge_abort_dirty_rejection_and_non_ff_modes() {
    for mode in ["default", "no_ff", "ff_only"] {
        let f = Fixture::new();
        let p = f.repo("repo");
        let id = f.add(&p).await;
        f.run(&p, &["checkout", "-b", "feature"]);
        f.write_commit(&p, "feature.txt", "feature");
        f.run(&p, &["checkout", "main"]);
        f.write_commit(&p, "main.txt", "main");
        let preview = f.core.preview_merge(&id, "feature", "main").await.unwrap();
        assert_eq!(preview["conflicts"]["status"], "none");
        let item=f.op("merge",&id,json!({"source":"feature","target":"main","mode":mode,"fingerprint":preview["fingerprint"]})).await;
        if mode == "ff_only" {
            assert_eq!(item["reason"], "diverged");
        } else {
            assert_eq!(item["state"], "succeeded");
            assert_eq!(
                f.run(&p, &["show", "-s", "--format=%P"])
                    .split_whitespace()
                    .count(),
                2
            );
        }
        std::fs::write(p.join("dirty"), "dirty").unwrap();
        let preview = f.core.preview_merge(&id, "feature", "main").await.unwrap();
        assert_eq!(preview["blocker"], "dirty_worktree");
        let preview = f.core.preview_rebase(&id, "feature").await.unwrap();
        assert_eq!(
            f.core
                .mutate(
                    &id,
                    "rebase",
                    json!({"onto":"feature","fingerprint":preview["fingerprint"]})
                )
                .await
                .unwrap_err()
                .code,
            "dirty_worktree"
        );
    }
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    f.run(&p, &["checkout", "-b", "feature"]);
    f.write_commit(&p, "base.txt", "theirs");
    f.run(&p, &["checkout", "main"]);
    f.write_commit(&p, "base.txt", "ours");
    let before = f.run(&p, &["rev-parse", "HEAD"]);
    let preview = f.core.preview_merge(&id, "feature", "main").await.unwrap();
    f.op("merge",&id,json!({"source":"feature","target":"main","mode":"default","fingerprint":preview["fingerprint"]})).await;
    assert_eq!(
        f.core
            .mutate(
                &id,
                "discard_changes",
                json!({"paths":null,"includeUntracked":true})
            )
            .await
            .unwrap_err()
            .code,
        "operation_in_progress"
    );
    f.core
        .mutate(&id, "abort_in_progress", json!({}))
        .await
        .unwrap();
    assert_eq!(f.run(&p, &["rev-parse", "HEAD"]), before);
    assert_eq!(std::fs::read_to_string(p.join("base.txt")).unwrap(), "ours");
    assert_eq!(
        f.core
            .mutate(&id, "abort_in_progress", json!({}))
            .await
            .unwrap_err()
            .code,
        "invalid_input"
    );
}
#[tokio::test]
async fn stash_conflict_keeps_stash_and_discard_handles_index_add_delete() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    std::fs::write(p.join("base.txt"), "stashed").unwrap();
    f.core
        .mutate(
            &id,
            "stash_save",
            json!({"message":null,"includeUntracked":true}),
        )
        .await
        .unwrap();
    let sha = f.run(&p, &["rev-parse", "refs/stash"]);
    f.write_commit(&p, "base.txt", "committed");
    assert_eq!(
        f.core
            .mutate(&id, "stash_apply", json!({"stashSha":sha,"drop":true}))
            .await
            .unwrap_err()
            .code,
        "merge_conflict"
    );
    assert_eq!(f.run(&p, &["rev-parse", "refs/stash"]), sha);
    f.core
        .mutate(
            &id,
            "discard_changes",
            json!({"paths":null,"includeUntracked":false}),
        )
        .await
        .unwrap();
    f.run(&p, &["rm", "base.txt"]);
    std::fs::write(p.join("added.txt"), "added").unwrap();
    f.run(&p, &["add", "added.txt"]);
    f.core
        .mutate(
            &id,
            "discard_changes",
            json!({"paths":null,"includeUntracked":false}),
        )
        .await
        .unwrap();
    assert!(p.join("base.txt").exists());
    assert!(!p.join("added.txt").exists());
    assert!(f.run(&p, &["status", "--porcelain"]).is_empty());
}
#[tokio::test]
async fn checkout_failure_restores_stash_and_carry_changes() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    f.run(&p, &["branch", "busy"]);
    let worktree = f.temp.path().join("worktree");
    f.run(&p, &["worktree", "add", worktree.to_str().unwrap(), "busy"]);
    std::fs::write(p.join("base.txt"), "dirty\n").unwrap();
    std::fs::write(p.join("untracked"), "new").unwrap();
    for strategy in ["carry", "stash"] {
        let error = f
            .core
            .mutate(
                &id,
                "checkout_branch",
                json!({"branch":"busy","ifDirty":strategy}),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, "unknown");
        assert!(error.detail.as_deref().unwrap_or("").contains("busy"));
        assert_eq!(f.run(&p, &["branch", "--show-current"]), "main");
        assert_eq!(
            std::fs::read_to_string(p.join("base.txt")).unwrap(),
            "dirty\n"
        );
        assert!(p.join("untracked").exists());
        assert!(f.run(&p, &["stash", "list"]).is_empty(), "{error:?}");
    }
}
#[cfg(unix)]
#[tokio::test]
async fn refresh_and_plan_are_bounded_parallel_and_status_is_light() {
    let f = Fixture::new();
    let mut ids = vec![];
    for i in 0..12 {
        let p = f.repo(&format!("repo{i}"));
        ids.push(f.add(&p).await);
    }
    let trace = f.temp.path().join("trace");
    let script = f.temp.path().join("parallel-git");
    std::fs::write(&script,format!("#!/bin/sh\nprintf 'call\\n' >> '{}'\nfor arg do\nif [ \"$arg\" = status ]; then\nprintf 'start\\n' >> '{}'\nsleep 0.2\nprintf 'end\\n' >> '{}'\nfi\ndone\nexec '{}' \"$@\"\n",trace.display(),trace.display(),trace.display(),f.git.executable.display())).unwrap();
    common::executable(&script, 0o700);
    let mut git = f.git.clone();
    git.executable = script;
    let core = Fixture::core(f.temp.path(), git);
    core.refresh(None).await.unwrap();
    let log = std::fs::read_to_string(&trace).unwrap();
    let mut active = 0;
    let mut peak = 0;
    for line in log.lines() {
        if line == "start" {
            active += 1;
            peak = peak.max(active);
        } else if line == "end" {
            active -= 1;
        }
    }
    assert!((2..=8).contains(&peak), "peak {peak}");
    assert!(log.lines().filter(|s| *s == "call").count() <= 12 * 10);
    std::fs::write(&trace, "").unwrap();
    core.plan(json!({"kind":"fetch","items":ids.iter().map(|id|json!({"itemId":id,"projectId":id})).collect::<Vec<_>>()})).await.unwrap();
    let log = std::fs::read_to_string(&trace).unwrap();
    let mut active = 0;
    let mut peak = 0;
    for line in log.lines() {
        if line == "start" {
            active += 1;
            peak = peak.max(active);
        } else if line == "end" {
            active -= 1;
        }
    }
    assert!((2..=8).contains(&peak), "peak {peak}");
}

#[cfg(unix)]
#[tokio::test]
async fn network_progress_events_are_measured_and_throttled() {
    use std::sync::Mutex;
    #[derive(Default)]
    struct Events(Mutex<Vec<(std::time::Instant, Value)>>);
    impl revia_lib::EventSink for Events {
        fn emit(&self, name: &str, v: Value) {
            if name == "revia:operation" {
                self.0.lock().unwrap().push((std::time::Instant::now(), v));
            }
        }
    }
    let f = Fixture::new();
    let p = f.repo("repo");
    f.remote(&p);
    let id = f.add(&p).await;
    let git = mocked_git(
        &f,
        "printf 'Receiving objects: 10%%\\rReceiving objects: 20%%\\r' >&2\nsleep 0.2\nprintf 'Receiving objects: 80%%\\rReceiving objects: 90%%\\r' >&2\nexit 0",
    );
    let events = Arc::new(Events::default());
    let core = Core::new(Options {
        oauth: None,
        trash: Arc::new(revia_lib::trash::MoveTo(f.temp.path().join("trash"))),
        config_dir: f.temp.path().join("config"),
        data_dir: f.temp.path().join("data"),
        cache_dir: f.temp.path().join("cache"),
        git,
        events: events.clone(),
    })
    .unwrap();
    let snapshot = core
        .start(json!({"kind":"fetch","items":[{"itemId":id,"projectId":id}]}))
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while core
            .operation(snapshot["operationId"].as_str().unwrap())
            .unwrap()["state"]
            != "finished"
        {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let mut distinct = vec![];
    for (time, event) in events.0.lock().unwrap().iter() {
        if let Some(value) = event["item"]["progress"].as_f64()
            && distinct.last().is_none_or(|(_, last)| *last != value)
        {
            distinct.push((*time, value));
        }
    }
    assert_eq!(
        distinct.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
        vec![0.1, 0.8]
    );
    assert!(distinct[1].0.duration_since(distinct[0].0) >= std::time::Duration::from_millis(150));
}
#[cfg(unix)]
#[tokio::test]
async fn network_failure_classification_and_fetch_attempts_remaining_remotes() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let bare = f.remote(&p);
    let id = f.add(&p).await;
    f.run(&p, &["remote", "add", "aaa-failing", "/nonexistent/revia"]);
    f.write_commit(&p, "next.txt", "next");
    f.run(&p, &["push", "origin", "main"]);
    f.run(&p, &["update-ref", "-d", "refs/remotes/origin/main"]);
    assert_eq!(f.op("fetch", &id, json!({})).await["state"], "failed");
    assert_eq!(
        f.run(&p, &["rev-parse", "refs/remotes/origin/main"]),
        f.run(&bare, &["rev-parse", "main"])
    );
    for (message, code) in [
        ("HTTP 403 Forbidden", "permission_denied"),
        ("Connection timed out", "network_unreachable"),
        ("Network is unreachable", "network_unreachable"),
        ("Failed to connect to host", "network_unreachable"),
    ] {
        let git = mocked_git(&f, &format!("printf '%s\\n' '{message}' >&2\nexit 1"));
        let error = git
            .run(&p, &["fetch".into(), "origin".into()], true, None)
            .await
            .unwrap_err();
        assert_eq!(error.code, code);
    }
}
#[tokio::test]
async fn preview_warnings_for_published_rebase_force_push_and_conflicting_pull() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let bare = f.remote(&p);
    let other = f.clone_repo(&bare, "other");
    let id = f.add(&p).await;
    f.write_commit(&p, "base.txt", "local");
    f.run(&p, &["push", "origin", "HEAD:published"]);
    f.write_commit(&other, "base.txt", "remote");
    f.run(&other, &["push"]);
    f.run(&p, &["fetch", "origin"]);
    f.core
        .set_target(
            &id,
            "main",
            "push",
            json!({"remote":"origin","branch":"published"}),
        )
        .await
        .unwrap();
    let rebase = f
        .core
        .plan(json!({"kind":"pull","items":[{"itemId":id,"projectId":id,"mode":"rebase"}]}))
        .await
        .unwrap();
    assert_eq!(rebase["items"][0]["risk"], "danger");
    assert!(
        rebase["items"][0]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["code"] == "rewrites_published_history" && w["commits"] == 1)
    );
    let merge = f
        .core
        .plan(json!({"kind":"pull","items":[{"itemId":id,"projectId":id,"mode":"merge"}]}))
        .await
        .unwrap();
    assert!(
        merge["items"][0]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["code"] == "conflicts_likely")
    );
    f.core
        .set_target(
            &id,
            "main",
            "push",
            json!({"remote":"origin","branch":"main"}),
        )
        .await
        .unwrap();
    let push=f.core.plan(json!({"kind":"push","items":[{"itemId":id,"projectId":id,"force":true,"setUpstream":false}]})).await.unwrap();
    assert!(
        push["items"][0]["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["code"] == "rewrites_published_history" && w["commits"] == 1)
    );
}
#[cfg(unix)]
#[tokio::test]
async fn unavailable_merge_tree_is_unknown_and_large_conflict_is_scanned_fully() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    f.run(&p, &["checkout", "-b", "feature"]);
    f.write_commit(&p, "base.txt", "feature");
    f.run(&p, &["checkout", "main"]);
    f.write_commit(&p, "base.txt", "main");
    let script = f.temp.path().join("old-git");
    std::fs::write(&script,format!("#!/bin/sh\nfor arg do\nif [ \"$arg\" = merge-tree ]; then exit 129; fi\ndone\nexec '{}' \"$@\"\n",f.git.executable.display())).unwrap();
    common::executable(&script, 0o700);
    let mut git = f.git.clone();
    git.executable = script;
    let core = Fixture::core(f.temp.path(), git);
    assert_eq!(
        core.preview_merge(&id, "feature", "main").await.unwrap()["conflicts"]["status"],
        "unknown"
    );
    let preview = f.core.preview_merge(&id, "feature", "main").await.unwrap();
    f.op("merge",&id,json!({"source":"feature","target":"main","mode":"default","fingerprint":preview["fingerprint"]})).await;
    std::fs::write(p.join("base.txt"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
    assert_eq!(
        f.core.conflicts(&id).await.unwrap()["files"][0]["hasMarkers"],
        false
    );
    let mut large = vec![b'x'; 2 * 1024 * 1024];
    large.extend_from_slice(b"\n<<<<<<< HEAD\nmain\n=======\nfeature\n>>>>>>> feature\n");
    std::fs::write(p.join("base.txt"), large).unwrap();
    assert_eq!(
        f.core.conflicts(&id).await.unwrap()["files"][0]["hasMarkers"],
        true
    );
}

#[tokio::test]
async fn history_counts_only_actual_committed_files() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    std::fs::write(p.join("new.txt"), "new").unwrap();
    assert_eq!(
        f.op(
            "commit",
            &id,
            json!({"message":"selected","paths":["base.txt","new.txt"]})
        )
        .await["state"],
        "succeeded"
    );
    let history = f.core.history(1, None, Some(id)).await.unwrap();
    assert_eq!(history[0]["params"]["files"], 1);
    assert_eq!(history[0]["params"]["commits"], 1);
}

#[cfg(unix)]
mod github_publish_clone {
    use super::*;
    use std::{sync::Mutex, time::Duration};
    #[derive(Default)]
    struct Events(Mutex<Vec<(String, Value)>>);
    impl revia_lib::EventSink for Events {
        fn emit(&self, name: &str, payload: Value) {
            self.0.lock().unwrap().push((name.into(), payload));
        }
    }
    struct GhFixture {
        f: Fixture,
        events: Arc<Events>,
    }
    impl GhFixture {
        fn new() -> Self {
            let mut f = Fixture::new();
            let script = f.temp.path().join("gh-mock");
            std::fs::write(&script,r#"#!/bin/sh
set -eu
[ "$GH_PROMPT_DISABLED" = 1 ] && [ "$NO_COLOR" = 1 ] && [ "$GH_NO_UPDATE_NOTIFIER" = 1 ] && [ "$CLICOLOR" = 0 ]
printf '%s\n' "$*" >> "$MOCK_ROOT/calls"
case "$1 $2" in
'auth status')
  if [ -e "$MOCK_ROOT/logged-out" ]; then echo 'not logged into github.com' >&2; exit 1; fi
  if [ -e "$MOCK_ROOT/scopes-missing" ]; then echo "Token scopes: 'read:org'" >&2; else echo "Token scopes: 'repo', 'read:org'" >&2; fi
  ;;
'config get') if [ -e "$MOCK_ROOT/ssh" ]; then echo ssh; else echo https; fi ;;
'auth login')
  [ -x "$GH_BROWSER" ]
  echo 'First copy your one-time code: ABCD-1234' >&2
  echo 'https://github.com/login/device' >&2
  if [ -e "$MOCK_ROOT/login-wait" ]; then exec /bin/sleep 30; fi
  if [ -e "$MOCK_ROOT/login-fail" ]; then echo 'HTTP 401 ghp_secret123' >&2; exit 1; fi
  rm -f "$MOCK_ROOT/logged-out"
  ;;
'auth logout') touch "$MOCK_ROOT/logged-out" ;;
'repo create')
  if [ -e "$MOCK_ROOT/repo-exists" ]; then echo 'repository already exists ghp_hidden' >&2; exit 1; fi
  touch "$MOCK_ROOT/created"
  ;;
'api user') echo '{"login":"mock","name":"Mock User","avatar_url":"https://github.com/avatar"}' ;;
'api --paginate')
  if [ -e "$MOCK_ROOT/rate-limit" ]; then echo 'HTTP 403 secondary rate limit ghp_hidden' >&2; exit 1; fi
  case "$3" in
    user/orgs) echo '[{"login":"team","avatar_url":null}]' ;;
    *) cat "$MOCK_ROOT/repos" ;;
  esac ;;
*) echo unexpected >&2; exit 1 ;;
esac
"#).unwrap();
            common::executable(&script, 0o755);
            f.git.gh = script;
            f.git
                .env
                .insert("MOCK_ROOT".into(), f.temp.path().to_string_lossy().into());
            std::fs::write(f.temp.path().join("repos"), "[]").unwrap();
            let events = Arc::new(Events::default());
            f.core = Core::new(Options {
                oauth: None,
                trash: Arc::new(revia_lib::trash::MoveTo(f.temp.path().join("trash"))),
                config_dir: f.temp.path().join("config"),
                data_dir: f.temp.path().join("data"),
                cache_dir: f.temp.path().join("cache"),
                git: f.git.clone(),
                events: events.clone(),
            })
            .unwrap();
            Self { f, events }
        }
        fn flag(&self, name: &str) {
            std::fs::write(self.f.temp.path().join(name), "").unwrap();
        }
        async fn login_terminal(&self, id: &str) -> Value {
            for _ in 0..500 {
                if let Some(e) = self
                    .events
                    .0
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|(n, v)| {
                        n == "revia:github-login" && v["loginId"] == id && v["kind"] != "code"
                    })
                    .map(|(_, v)| v.clone())
                {
                    return e;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("login did not finish");
        }
        fn publish(id: &str) -> Value {
            json!({"kind":"publish","items":[{"itemId":id,"projectId":id,"owner":"mock","name":"demo","visibility":"private","description":null,"initialCommitMessage":"initial"}]})
        }
        async fn wait(&self, request: Value) -> Value {
            let s = self.f.core.start(request).await.unwrap();
            let id = s["operationId"].as_str().unwrap();
            for _ in 0..1000 {
                let s = self.f.core.operation(id).unwrap();
                if s["state"] == "finished" {
                    return s;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("operation did not finish");
        }
        async fn confirm(&self, mut request: Value) -> Value {
            let plan = self.f.core.plan(request.clone()).await.unwrap();
            request["items"][0]["fingerprint"] = plan["items"][0]["fingerprint"].clone();
            request
        }
    }
    #[tokio::test]
    async fn github_status_absent_signed_out_scopes_and_cache() {
        let g = GhFixture::new();
        g.flag("logged-out");
        assert_eq!(
            g.f.core.github_status(true).await.unwrap()["authenticated"],
            false
        );
        std::fs::remove_file(g.f.temp.path().join("logged-out")).unwrap();
        assert_eq!(
            g.f.core.github_status(false).await.unwrap()["authenticated"],
            false
        );
        g.flag("ssh");
        g.flag("scopes-missing");
        let s = g.f.core.github_status(true).await.unwrap();
        assert_eq!(s["account"]["login"], "mock");
        assert_eq!(s["gitProtocol"], "ssh");
        assert_eq!(s["missingScopes"], json!(["repo"]));
        assert_eq!(
            g.f.core.github_logout().await.unwrap()["authenticated"],
            false
        );
        let mut git = g.f.git.clone();
        git.gh = g.f.temp.path().join("absent");
        let core = Fixture::core(g.f.temp.path(), git);
        assert_eq!(
            core.github_status(true).await.unwrap()["cliInstalled"],
            false
        );
        assert_eq!(
            core.github_login_start(None).await.unwrap_err().code,
            "gh_not_installed"
        );
        assert_eq!(
            core.github_list_repos(false).await.unwrap_err().code,
            "gh_not_installed"
        );
    }
    #[tokio::test]
    async fn github_login_events_success_failure_cancel_and_busy() {
        let g = GhFixture::new();
        let id = g.f.core.github_login_start(None).await.unwrap()["loginId"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(g.login_terminal(&id).await["kind"], "succeeded");
        assert!(
            g.events
                .0
                .lock()
                .unwrap()
                .iter()
                .any(|(_, v)| v["kind"] == "code"
                    && v["userCode"] == "ABCD-1234"
                    && v["verificationUrl"] == "https://github.com/login/device")
        );
        g.flag("login-fail");
        let id = g.f.core.github_login_start(None).await.unwrap()["loginId"]
            .as_str()
            .unwrap()
            .to_string();
        let e = g.login_terminal(&id).await;
        assert_eq!(e["kind"], "failed");
        assert!(!e.to_string().contains("ghp_"));
        g.flag("login-wait");
        let id = g.f.core.github_login_start(None).await.unwrap()["loginId"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(
            g.f.core.github_login_start(None).await.unwrap_err().code,
            "busy"
        );
        g.f.core.github_login_cancel("unrelated");
        g.f.core.github_login_cancel(&id);
        assert_eq!(g.login_terminal(&id).await["kind"], "cancelled");
        g.f.core.github_login_cancel(&id);
    }
    #[tokio::test]
    async fn github_repos_pages_mapping_cache_refresh_and_rate_limit() {
        let g = GhFixture::new();
        let p = g.f.repo("local");
        g.f.run(
            &p,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/MOCK/demo.git",
            ],
        );
        let id = g.f.add(&p).await;
        let q = g.f.repo("ssh-local");
        g.f.run(
            &q,
            &[
                "remote",
                "add",
                "origin",
                "ssh://git@github.com/mock/DEMO.git",
            ],
        );
        let id2 = g.f.add(&q).await;
        let repo = json!({"id":12,"full_name":"mock/demo","owner":{"login":"mock"},"name":"demo","html_url":"https://github.com/mock/demo","clone_url":"https://github.com/mock/demo.git","ssh_url":"git@github.com:mock/demo.git","updated_at":"2026-01-01T00:00:00Z","default_branch":"main"});
        std::fs::write(
            g.f.temp.path().join("repos"),
            format!("[{}]\n[{}]", repo, repo),
        )
        .unwrap();
        let r = g.f.core.github_list_repos(false).await.unwrap();
        assert_eq!(r.len(), 2);
        assert!(
            r[0]["localProjectIds"]
                .as_array()
                .unwrap()
                .contains(&json!(id))
        );
        assert!(
            r[0]["localProjectIds"]
                .as_array()
                .unwrap()
                .contains(&json!(id2))
        );
        assert_eq!(r[0]["updatedAt"], 1767225600000u64);
        std::fs::write(g.f.temp.path().join("repos"), "[]").unwrap();
        assert_eq!(g.f.core.github_list_repos(false).await.unwrap().len(), 2);
        assert!(g.f.core.github_list_repos(true).await.unwrap().is_empty());
        assert_eq!(g.f.core.github_list_owners().await.unwrap().len(), 2);
        g.flag("rate-limit");
        let e = g.f.core.github_list_repos(true).await.unwrap_err();
        assert_eq!(e.code, "rate_limited");
        assert!(!json!(e).to_string().contains("ghp_"));
        g.f.core.github_logout().await.unwrap();
        assert_eq!(
            g.f.core.github_list_repos(false).await.unwrap_err().code,
            "not_authenticated"
        );
    }
    #[tokio::test]
    async fn publish_full_flow_local_push_and_origin_guard() {
        let mut g = GhFixture::new();
        let bare = g.f.dir("bare.git");
        g.f.run(&bare, &["init", "--bare"]);
        g.f.git.env.insert("GIT_CONFIG_COUNT".into(), "4".into());
        g.f.git.env.insert(
            "GIT_CONFIG_KEY_3".into(),
            format!("url.{}.insteadOf", bare.display()),
        );
        g.f.git.env.insert(
            "GIT_CONFIG_VALUE_3".into(),
            "https://github.com/mock/demo.git".into(),
        );
        g.f.core = Core::new(Options {
            oauth: None,
            trash: Arc::new(revia_lib::trash::MoveTo(g.f.temp.path().join("trash"))),
            config_dir: g.f.temp.path().join("config"),
            data_dir: g.f.temp.path().join("data"),
            cache_dir: g.f.temp.path().join("cache"),
            git: g.f.git.clone(),
            events: g.events.clone(),
        })
        .unwrap();
        g.f.core.github_status(true).await.unwrap();
        let p = g.f.dir("new-project");
        std::fs::write(p.join("hello.txt"), "hello").unwrap();
        let id = g.f.add(&p).await;
        let request = g.confirm(GhFixture::publish(&id)).await;
        assert_eq!(
            g.f.core
                .start(GhFixture::publish(&id))
                .await
                .unwrap_err()
                .code,
            "invalid_input"
        );
        let s = g.wait(request).await;
        assert_eq!(s["items"][0]["state"], "succeeded", "{s}");
        assert_eq!(
            s["items"][0]["completed"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["code"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "init",
                "initial_commit",
                "create_repo",
                "add_remote",
                "push"
            ]
        );
        assert!(!g.f.run(&bare, &["rev-parse", "refs/heads/main"]).is_empty());
        std::fs::remove_file(g.f.temp.path().join("created")).unwrap();
        let req = g.confirm(GhFixture::publish(&id)).await;
        let s = g.wait(req).await;
        assert_eq!(s["items"][0]["reason"], "remote_exists");
        assert!(!g.f.temp.path().join("created").exists());
    }
    #[tokio::test]
    async fn publish_ignore_preview_and_initial_commit() {
        for existing in [None, Some("# keep\n*.log"), Some("/.env\n")] {
            let g = GhFixture::new();
            g.f.core.github_status(true).await.unwrap();
            g.flag("repo-exists");
            let p = g.f.dir("ignore-project");
            std::fs::write(p.join(".env"), "private fixture").unwrap();
            std::fs::write(p.join("hello.txt"), "hello").unwrap();
            if let Some(content) = existing {
                std::fs::write(p.join(".gitignore"), content).unwrap();
            }
            let id = g.f.add(&p).await;
            let mut request = GhFixture::publish(&id);
            request["items"][0]["ignore"] = json!(["/.env", "/.env"]);
            let before = std::fs::read(p.join(".gitignore")).ok();
            let plan = g.f.core.plan(request.clone()).await.unwrap();
            let steps = plan["items"][0]["steps"].as_array().unwrap();
            assert_eq!(steps[0]["code"], "init");
            if existing == Some("/.env\n") {
                assert!(!steps.iter().any(|s| s["code"] == "ignore"));
            } else {
                assert_eq!(steps[1], json!({"code":"ignore","patterns":1}));
            }
            assert_eq!(
                steps
                    .iter()
                    .find(|s| s["code"] == "initial_commit")
                    .unwrap()["files"],
                2
            );
            assert!(
                !plan["items"][0]["warnings"]
                    .to_string()
                    .contains("sensitive_files")
            );
            assert!(
                !plan["items"][0]["warnings"]
                    .to_string()
                    .contains("no_gitignore")
            );
            assert_eq!(std::fs::read(p.join(".gitignore")).ok(), before);
            assert!(!p.join(".git").exists());
            let state = g.wait(g.confirm(request).await).await;
            assert_eq!(state["items"][0]["state"], "failed", "{state}");
            assert_eq!(state["items"][0]["error"]["code"], "repo_exists");
            let committed = g.f.run(&p, &["ls-tree", "--name-only", "HEAD"]);
            assert!(!committed.lines().any(|p| p == ".env"));
            assert!(committed.lines().any(|p| p == ".gitignore"));
            let content = std::fs::read_to_string(p.join(".gitignore")).unwrap();
            let expected = match existing {
                None => "# Added by Revia\n/.env\n",
                Some("# keep\n*.log") => "# keep\n*.log\n# Added by Revia\n/.env\n",
                _ => "/.env\n",
            };
            assert_eq!(content, expected);
            assert_eq!(
                state["items"][0]["completed"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|s| s["code"] == "ignore"),
                existing != Some("/.env\n")
            );
        }
    }
    #[tokio::test]
    async fn publish_ignore_validation_and_inactive_behavior() {
        let g = GhFixture::new();
        g.f.core.github_status(true).await.unwrap();
        let p = g.f.dir("ignore-invalid");
        std::fs::write(p.join("hello.txt"), "hello").unwrap();
        let id = g.f.add(&p).await;
        let mut request = GhFixture::publish(&id);
        for invalid in [
            json!([""]),
            json!(["a\nb"]),
            json!(["a\tb"]),
            json!(["!foo"]),
            json!(["#foo"]),
            json!(["a/../b"]),
            json!([" leading"]),
            json!(["trailing "]),
            json!(["a".repeat(301)]),
            json!(vec!["a"; 501]),
        ] {
            request["items"][0]["ignore"] = invalid;
            assert_eq!(
                g.f.core.plan(request.clone()).await.unwrap_err().code,
                "invalid_input"
            );
        }
        for ignore in [Value::Null, json!([])] {
            request["items"][0]["ignore"] = ignore;
            g.flag("repo-exists");
            let state = g.wait(g.confirm(request.clone()).await).await;
            assert_eq!(state["items"][0]["error"]["code"], "repo_exists");
            assert!(!p.join(".gitignore").exists());
        }
        request["items"][0]["ignore"] = json!(["!invalid"]);
        let plan = g.f.core.plan(request.clone()).await.unwrap();
        assert!(!plan["items"][0]["steps"].to_string().contains("\"ignore\""));
        let p = g.f.dir("no-initial-commit");
        let id = g.f.add(&p).await;
        request = GhFixture::publish(&id);
        request["items"][0]["initialCommitMessage"] = Value::Null;
        request["items"][0]["ignore"] = json!(["!invalid"]);
        assert!(g.f.core.plan(request).await.is_ok());
    }
    #[tokio::test]
    async fn publish_ignore_rejects_link_and_non_file() {
        let g = GhFixture::new();
        g.f.core.github_status(true).await.unwrap();
        let p = g.f.dir("ignore-link");
        let target = g.f.temp.path().join("outside-ignore");
        std::fs::write(&target, "unchanged").unwrap();
        std::os::unix::fs::symlink(&target, p.join(".gitignore")).unwrap();
        let id = g.f.add(&p).await;
        let mut request = GhFixture::publish(&id);
        request["items"][0]["ignore"] = json!(["/.env"]);
        assert_eq!(
            g.f.core.plan(request.clone()).await.unwrap_err().code,
            "invalid_path"
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "unchanged");
        std::fs::remove_file(p.join(".gitignore")).unwrap();
        std::fs::create_dir(p.join(".gitignore")).unwrap();
        assert_eq!(
            g.f.core.plan(request).await.unwrap_err().code,
            "invalid_path"
        );
    }
    #[tokio::test]
    async fn publish_warning_ignore_risk_fingerprint_repo_exists_partial() {
        let g = GhFixture::new();
        g.f.core.github_status(true).await.unwrap();
        let p = g.f.dir("warning-project");
        std::fs::write(p.join(".env"), "secret").unwrap();
        std::fs::write(p.join(".gitignore"), "ignored.pem\n").unwrap();
        std::fs::write(p.join("ignored.pem"), "secret").unwrap();
        let id = g.f.add(&p).await;
        let request = GhFixture::publish(&id);
        let plan = g.f.core.plan(request.clone()).await.unwrap();
        assert_eq!(plan["items"][0]["risk"], "danger");
        let warnings = plan["items"][0]["warnings"].to_string();
        assert!(warnings.contains(".env"));
        assert!(!warnings.contains("ignored.pem"));
        let confirmed = g.confirm(request.clone()).await;
        std::fs::write(p.join("new.txt"), "changed").unwrap();
        assert_eq!(
            g.wait(confirmed).await["items"][0]["reason"],
            "state_changed"
        );
        g.flag("repo-exists");
        let s = g.wait(g.confirm(request).await).await;
        assert_eq!(s["items"][0]["error"]["code"], "repo_exists", "{s}");
        assert_eq!(s["items"][0]["completed"].as_array().unwrap().len(), 2);
        assert!(!s.to_string().contains("ghp_"));
        assert!(
            !g.f.core
                .history(100, None, None)
                .await
                .unwrap()
                .iter()
                .any(|v| v.to_string().contains("ghp_"))
        );
    }
    #[tokio::test]
    async fn clone_local_registration_destination_and_validation() {
        let g = GhFixture::new();
        let seed = g.f.repo("seed");
        let source = g.f.dir("source.git");
        g.f.run(&source, &["init", "--bare", "-b", "main"]);
        g.f.run(&seed, &["push", source.to_str().unwrap(), "main"]);
        let parent = g.f.dir("clones");
        let request = |name: &str| json!({"kind":"clone","items":[{"itemId":"clone-1","url":source,"parentDir":parent,"folderName":name}]});
        let s = g.wait(request("manual")).await;
        assert_eq!(s["items"][0]["state"], "succeeded", "{s}");
        let id = s["items"][0]["projectId"].as_str().unwrap();
        assert_eq!(g.f.core.project(id).unwrap()["origin"]["kind"], "manual");
        assert_eq!(
            g.f.core.history(10, None, None).await.unwrap()[0]["params"]["repo"],
            json!(source)
        );
        let plan = g.f.core.plan(request("manual")).await.unwrap();
        assert_eq!(plan["items"][0]["blocker"], "destination_exists");
        g.f.core
            .registry_change("add_root", json!(parent), json!(4))
            .await
            .unwrap();
        let s = g.wait(request("scanned")).await;
        assert_eq!(s["items"][0]["state"], "succeeded", "{s}");
        assert_eq!(
            g.f.core
                .project(s["items"][0]["projectId"].as_str().unwrap())
                .unwrap()["origin"]["kind"],
            "scan"
        );
        for name in ["..", "a/b", "a\\b", "a..b", ""] {
            assert!(g.f.core.plan(request(name)).await.is_err());
        }
        let mut invalid = request("bad");
        invalid["items"][0]["url"] = json!("ext::evil");
        assert_eq!(
            g.f.core.start(invalid).await.unwrap_err().code,
            "invalid_input"
        );
    }
    #[tokio::test]
    async fn clone_cancel_removes_owned_destination() {
        let mut g = GhFixture::new();
        let parent = g.f.dir("clones");
        let script = g.f.temp.path().join("git-mock");
        std::fs::write(
            &script,
            "#!/bin/sh\nprintf 'Receiving objects: 25%% (1/4)\\r' >&2\nexec /bin/sleep 30\n",
        )
        .unwrap();
        common::executable(&script, 0o755);
        g.f.git.executable = script;
        g.f.core = Fixture::core(g.f.temp.path(), g.f.git.clone());
        let s=g.f.core.start(json!({"kind":"clone","items":[{"itemId":"clone","url":"https://github.com/mock/demo","parentDir":parent,"folderName":"cancelled"}]})).await.unwrap();
        let id = s["operationId"].as_str().unwrap();
        for _ in 0..100 {
            if parent.join("cancelled").exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
        g.f.core.cancel(id).unwrap();
        for _ in 0..200 {
            if g.f.core.operation(id).unwrap()["state"] == "finished" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            g.f.core.operation(id).unwrap()["items"][0]["state"],
            "cancelled"
        );
        assert!(!parent.join("cancelled").exists());
    }
    #[tokio::test]
    async fn startup_discovers_configured_roots_without_cache() {
        let f = Fixture::new();
        let root = f.dir("root");
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        std::fs::write(repo.join("Cargo.toml"), "").unwrap();
        f.core
            .registry_change("add_root", json!(root), json!(4))
            .await
            .unwrap();
        std::fs::remove_file(f.temp.path().join("cache/projects.json")).unwrap();
        let core = Fixture::core(f.temp.path(), f.git.clone());
        let state = core.workspace();
        assert!(
            state["projects"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["path"] == json!(repo))
        );
    }
    #[tokio::test]
    async fn publish_preview_is_offline_validates_names_and_all_warning_limits() {
        let g = GhFixture::new();
        g.f.core.github_status(true).await.unwrap();
        let p = g.f.dir("many-files");
        for i in 0..20002 {
            std::fs::write(p.join(format!("f{i:05}")), "").unwrap();
        }
        let large = std::fs::File::create(p.join("a-large")).unwrap();
        large.set_len(50 * 1024 * 1024 + 1).unwrap();
        std::fs::write(p.join(".env"), "private").unwrap();
        let id = g.f.add(&p).await;
        let mut req = GhFixture::publish(&id);
        req["items"][0]["visibility"] = json!("public");
        let calls = std::fs::read_to_string(g.f.temp.path().join("calls")).unwrap();
        let plan = g.f.core.plan(req.clone()).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(g.f.temp.path().join("calls")).unwrap(),
            calls
        );
        let warnings = plan["items"][0]["warnings"].as_array().unwrap();
        for code in [
            "sensitive_files",
            "large_files",
            "no_gitignore",
            "many_files",
            "public_repo",
        ] {
            assert!(warnings.iter().any(|w| w["code"] == code), "{plan}");
        }
        assert_eq!(
            warnings.iter().find(|w| w["code"] == "many_files").unwrap()["files"],
            20000
        );
        let mut excluded = req.clone();
        excluded["items"][0]["ignore"] = json!(["/.env", "/a-large", "/f*"]);
        let filtered = g.f.core.plan(excluded).await.unwrap();
        assert_eq!(
            filtered["items"][0]["warnings"],
            json!([{"code":"public_repo"}])
        );
        assert_eq!(
            filtered["items"][0]["steps"][1],
            json!({"code":"ignore","patterns":3})
        );
        assert_eq!(
            filtered["items"][0]["steps"][2],
            json!({"code":"initial_commit","files":1})
        );
        assert!(!p.join(".gitignore").exists());
        for name in [".", "..", "bad/name", "bad name", ""] {
            req["items"][0]["name"] = json!(name);
            let plan = g.f.core.plan(req.clone()).await.unwrap();
            assert_eq!(plan["items"][0]["blocker"], "invalid_input");
        }
    }
    #[tokio::test]
    async fn publish_unborn_without_message_creates_remote_without_push() {
        let g = GhFixture::new();
        g.f.core.github_status(true).await.unwrap();
        let p = g.f.dir("unborn");
        let id = g.f.add(&p).await;
        let mut req = GhFixture::publish(&id);
        req["items"][0]["initialCommitMessage"] = Value::Null;
        let plan = g.f.core.plan(req.clone()).await.unwrap();
        assert_eq!(plan["items"][0]["risk"], "caution");
        assert_eq!(plan["items"][0]["blocker"], Value::Null);
        assert!(
            !plan["items"][0]["steps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["code"] == "push")
        );
        let s = g.wait(g.confirm(req).await).await;
        assert_eq!(s["items"][0]["state"], "succeeded", "{s}");
        assert_eq!(s["items"][0]["completed"].as_array().unwrap().len(), 3);
    }
    #[tokio::test]
    async fn publish_missing_identity_does_not_create_repo() {
        let mut g = GhFixture::new();
        g.f.git.env.insert("GIT_CONFIG_COUNT".into(), "1".into());
        g.f.core = Fixture::core(g.f.temp.path(), g.f.git.clone());
        g.f.core.github_status(true).await.unwrap();
        let p = g.f.dir("identity");
        std::fs::write(p.join("file"), "hello").unwrap();
        let id = g.f.add(&p).await;
        let s = g.wait(g.confirm(GhFixture::publish(&id)).await).await;
        assert_eq!(s["items"][0]["error"]["code"], "identity_missing", "{s}");
        assert_eq!(s["items"][0]["completed"].as_array().unwrap().len(), 1);
        assert!(!g.f.temp.path().join("created").exists());
    }
    #[tokio::test]
    async fn publish_push_failure_retains_created_repo_and_remote() {
        let mut g = GhFixture::new();
        let missing = g.f.temp.path().join("missing.git");
        g.f.git.env.insert("GIT_CONFIG_COUNT".into(), "4".into());
        g.f.git.env.insert(
            "GIT_CONFIG_KEY_3".into(),
            format!("url.{}.insteadOf", missing.display()),
        );
        g.f.git.env.insert(
            "GIT_CONFIG_VALUE_3".into(),
            "https://github.com/mock/demo.git".into(),
        );
        g.f.core = Fixture::core(g.f.temp.path(), g.f.git.clone());
        g.f.core.github_status(true).await.unwrap();
        let p = g.f.repo("already-committed");
        std::fs::write(p.join("untracked.key"), "private").unwrap();
        let id = g.f.add(&p).await;
        let req = GhFixture::publish(&id);
        let plan = g.f.core.plan(req.clone()).await.unwrap();
        assert!(
            !plan["items"][0]["warnings"]
                .to_string()
                .contains("untracked.key")
        );
        let s = g.wait(g.confirm(req).await).await;
        assert_eq!(s["items"][0]["state"], "failed", "{s}");
        assert_eq!(s["items"][0]["completed"].as_array().unwrap().len(), 2);
        assert!(g.f.temp.path().join("created").exists());
        assert!(g.f.run(&p, &["remote"]).contains("origin"));
        assert!(
            s["items"][0]["error"]["detail"]
                .as_str()
                .unwrap()
                .contains("create_repo")
        );
    }
    #[tokio::test]
    async fn github_diagnostic_classification_and_no_secret_events() {
        let g = GhFixture::new();
        let p = g.f.repo("error-repo");
        g.f.remote(&p);
        for (message, code) in [
            ("fatal: object 403abcdef missing", "unknown"),
            ("fatal: returned error: 403", "permission_denied"),
            ("error: 403 forbidden", "permission_denied"),
        ] {
            let git = mocked_git(&g.f, &format!("printf '%s\\n' '{message}' >&2\nexit 1"));
            let error = git
                .run(&p, &["fetch".into(), "origin".into()], true, None)
                .await
                .unwrap_err();
            assert_eq!(error.code, code);
        }
        g.flag("login-fail");
        let login = g.f.core.github_login_start(None).await.unwrap();
        g.login_terminal(login["loginId"].as_str().unwrap()).await;
        assert!(
            !serde_json::to_string(&*g.events.0.lock().unwrap())
                .unwrap()
                .contains("ghp_")
        );
    }
}

#[tokio::test]
async fn background_copy_replace_read_clear() {
    let f = Fixture::new();
    let source = f.temp.path().join("背景.PNG");
    std::fs::write(&source, b"first").unwrap();
    let first = f.core.copy_background_image(&source).await.unwrap();
    assert_eq!(first["name"], "背景.PNG");
    assert_eq!(first["mime"], "image/png");
    assert_eq!(first["size"], 5);
    let id = first["id"].as_str().unwrap();
    assert_eq!(f.core.read_background_image(id).await.unwrap(), b"first");
    std::fs::write(&source, b"second").unwrap();
    let second = f.core.copy_background_image(&source).await.unwrap();
    assert_ne!(first["id"], second["id"]);
    assert_eq!(
        f.core.read_background_image(id).await.unwrap_err().code,
        "path_not_found"
    );
    assert_eq!(std::fs::read(&source).unwrap(), b"second");
    assert_eq!(
        std::fs::read_dir(f.core.options.data_dir.join("backgrounds"))
            .unwrap()
            .count(),
        1
    );
    f.core.clear_background_image().await.unwrap();
    assert_eq!(
        f.core
            .read_background_image(second["id"].as_str().unwrap())
            .await
            .unwrap_err()
            .code,
        "path_not_found"
    );
    f.core.clear_background_image().await.unwrap();
}

#[tokio::test]
async fn background_rejects_invalid_inputs_and_keeps_previous() {
    let f = Fixture::new();
    let source = f.temp.path().join("image.jpg");
    std::fs::write(&source, b"valid").unwrap();
    let kept = f.core.copy_background_image(&source).await.unwrap();
    let large = f.temp.path().join("large.webp");
    std::fs::File::create(&large)
        .unwrap()
        .set_len(30 * 1024 * 1024 + 1)
        .unwrap();
    let wrong = f.temp.path().join("image.svg");
    std::fs::write(&wrong, b"svg").unwrap();
    let directory = f.dir("directory.png");
    for path in [&large, &wrong, &directory] {
        assert_eq!(
            f.core.copy_background_image(path).await.unwrap_err().code,
            "invalid_input"
        );
    }
    for id in [
        "../image.png",
        "..\\image.png",
        "/image.png",
        "a.png",
        "00000000-0000-0000-0000-000000000000.svg",
        "00000000-0000-0000-0000-00000000000g.png",
    ] {
        assert_eq!(
            f.core.read_background_image(id).await.unwrap_err().code,
            "invalid_input"
        );
    }
    assert_eq!(
        f.core
            .read_background_image(kept["id"].as_str().unwrap())
            .await
            .unwrap(),
        b"valid"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn background_source_symlink_and_stored_symlink() {
    let f = Fixture::new();
    let original = f.temp.path().join("original");
    std::fs::write(&original, b"image").unwrap();
    let source = f.temp.path().join("linked.gif");
    std::os::unix::fs::symlink(&original, &source).unwrap();
    let image = f.core.copy_background_image(&source).await.unwrap();
    let id = image["id"].as_str().unwrap();
    assert_eq!(f.core.read_background_image(id).await.unwrap(), b"image");
    let stored = f.core.options.data_dir.join("backgrounds").join(id);
    std::fs::remove_file(&stored).unwrap();
    std::os::unix::fs::symlink(&original, stored).unwrap();
    assert_eq!(
        f.core.read_background_image(id).await.unwrap_err().code,
        "invalid_input"
    );
}

#[tokio::test]
async fn history_omits_null_params() {
    let f = Fixture::new();
    let path = f.dir("unborn");
    let id = f.add(&path).await;
    f.core
        .history_record_params(
            None,
            &id,
            "init",
            "succeeded",
            None,
            json!({"target": null, "files": 0, "name": "test"}),
        )
        .await
        .unwrap();
    let rows = f.core.history(1, None, None).await.unwrap();
    let params = rows[0]["params"].as_object().unwrap();
    assert!(!params.contains_key("target"));
    assert!(!params.contains_key("branch"));
    assert_eq!(params["files"], 0);
    assert_eq!(params["name"], "test");
}

#[test]
fn atomic_replaces_existing_file_and_cleans_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    revia_lib::settings::atomic(dir.path(), "settings.json", b"first").unwrap();
    revia_lib::settings::atomic(dir.path(), "settings.json", b"second").unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("settings.json")).unwrap(),
        b"second"
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(windows)]
#[test]
fn saved_verbatim_paths_remain_compatible() {
    let f = Fixture::new();
    let path = f.dir("project");
    let regular = path.to_string_lossy().into_owned();
    let verbatim = format!(r"\\?\{regular}");
    let mut settings = revia_lib::settings::Settings::default();
    settings.manual_paths.push(verbatim.clone());
    settings
        .project_settings
        .insert(verbatim.clone(), json!({"favorite":true}));
    settings.settings["roots"] =
        json!([{"id":"root", "path":verbatim,"enabled":true,"maxDepth":4,"exists":true}]);
    settings.settings["excludedPaths"] = json!([verbatim]);
    revia_lib::settings::save(&f.core.options.config_dir, &settings).unwrap();
    let (loaded, recovery) = revia_lib::settings::load(&f.core.options.config_dir).unwrap();
    assert!(recovery.is_none());
    assert_eq!(loaded.manual_paths, vec![regular.clone()]);
    assert!(loaded.project_settings.contains_key(&regular));
    assert_eq!(loaded.settings["roots"][0]["path"], regular);
    assert_eq!(loaded.settings["excludedPaths"][0], regular);
}

#[cfg(windows)]
#[tokio::test]
async fn hidden_scan_root_is_searched_and_hidden_children_are_skipped() {
    let f = Fixture::new();
    let root = f.dir("hidden-root");
    let concealed = root.join("concealed");
    for path in [root.join("shown"), concealed.clone()] {
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("package.json"), "{}").unwrap();
    }
    for path in [&root, &concealed] {
        let status = Command::new("attrib")
            .args(["+h", "+s"])
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success());
    }
    let w = f
        .core
        .registry_change("add_root", json!(root), Value::Null)
        .await
        .unwrap();
    let names = w["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names, ["shown"]);
}

#[cfg(windows)]
#[tokio::test]
async fn windows_batch_command_preserves_spaced_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("mock tool.cmd");
    std::fs::write(&script, "@echo off\r\necho %~1\r\n").unwrap();
    let output = revia_lib::platform::command(&script)
        .arg("argument with spaces")
        .output()
        .await
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "argument with spaces"
    );
}

mod removal {
    use super::*;
    struct FailingTrash;
    impl revia_lib::trash::Trash for FailingTrash {
        fn move_to_trash(&self, _: &Path) -> std::io::Result<()> {
            Err(std::io::Error::other("Test trash failure"))
        }
    }
    fn fail_trash(f: &mut Fixture) {
        f.core = Core::new(Options {
            oauth: None,
            config_dir: f.temp.path().join("config"),
            data_dir: f.temp.path().join("data"),
            cache_dir: f.temp.path().join("cache"),
            git: f.git.clone(),
            events: Arc::new(NoEvents),
            trash: Arc::new(FailingTrash),
        })
        .unwrap();
    }
    #[tokio::test]
    async fn preview_local_refs_stashes_changes_and_no_writes() {
        let f = Fixture::new();
        let path = f.repo("project");
        let id = f.add(&path).await;
        let preview = f.core.preview_removal(&id).await.unwrap();
        assert_eq!(preview["unpushedCommits"], 1);
        assert_eq!(preview["localOnlyBranches"], json!(["main"]));
        assert_eq!(preview["github"], Value::Null);
        let index = std::fs::read(path.join(".git/index")).unwrap();
        let head = std::fs::read(path.join(".git/HEAD")).unwrap();
        let status = f.run(&path, &["status", "--porcelain"]);
        assert_eq!(preview, f.core.preview_removal(&id).await.unwrap());
        assert_eq!(index, std::fs::read(path.join(".git/index")).unwrap());
        assert_eq!(head, std::fs::read(path.join(".git/HEAD")).unwrap());
        assert_eq!(status, f.run(&path, &["status", "--porcelain"]));
        f.run(
            &path,
            &[
                "remote",
                "add",
                "upstream",
                "https://github.com/team/project.git",
            ],
        );
        f.run(&path, &["update-ref", "refs/remotes/upstream/main", "HEAD"]);
        let p = f.core.preview_removal(&id).await.unwrap();
        assert_eq!(p["unpushedCommits"], 0);
        assert_eq!(p["localOnlyBranches"], json!([]));
        assert_eq!(p["github"]["owner"], "team");
        f.run(
            &path,
            &[
                "remote",
                "add",
                "origin",
                "git@github.com:owner/primary.git",
            ],
        );
        assert_eq!(
            f.core.preview_removal(&id).await.unwrap()["github"]["repo"],
            "primary"
        );
        f.run(&path, &["branch", "local-only"]);
        f.write_commit(&path, "other", "new");
        std::fs::write(path.join("base.txt"), "dirty").unwrap();
        f.run(&path, &["stash", "push"]);
        std::fs::write(path.join("base.txt"), "dirty again").unwrap();
        let p = f.core.preview_removal(&id).await.unwrap();
        assert_eq!(p["unpushedCommits"], 1);
        assert_eq!(p["stashes"], 1);
        assert_eq!(p["uncommittedFiles"], 1);
        assert_eq!(p["localOnlyBranches"], json!(["local-only"]));
        let child = path.join("child");
        std::fs::create_dir(&child).unwrap();
        f.add(&child).await;
        assert_eq!(
            f.core.preview_removal(&id).await.unwrap()["nestedProjects"],
            json!([child])
        );
    }
    #[tokio::test]
    async fn uninitialized_and_unborn_previews() {
        let f = Fixture::new();
        let path = f.dir("plain");
        let id = f.add(&path).await;
        let p = f.core.preview_removal(&id).await.unwrap();
        assert_eq!(p["repository"], false);
        assert_eq!(p["unpushedCommits"], 0);
        assert_eq!(p["remotes"], json!([]));
        f.run(&path, &["init", "-b", "main"]);
        let p = f.core.preview_removal(&id).await.unwrap();
        assert_eq!(p["repository"], true);
        assert_eq!(p["unpushedCommits"], 0);
    }
    #[tokio::test]
    async fn remove_git_validations_and_success_clear_targets() {
        let f = Fixture::new();
        let path = f.repo("project");
        let id = f.add(&path).await;
        f.core
            .update_project(
                &id,
                json!({"defaultRemote":"origin","defaultBranch":"main"}),
            )
            .await
            .unwrap();
        f.core
            .set_target(
                &id,
                "main",
                "push",
                json!({"remote":"origin","branch":"main"}),
            )
            .await
            .unwrap();
        let p = f.core.preview_removal(&id).await.unwrap();
        let fp = p["fingerprint"].as_str().unwrap();
        assert_eq!(
            f.core.remove_git(&id, fp, "wrong").await.unwrap_err().code,
            "invalid_input"
        );
        std::fs::write(path.join("new"), "new").unwrap();
        assert_eq!(
            f.core
                .remove_git(&id, fp, "project")
                .await
                .unwrap_err()
                .code,
            "state_changed"
        );
        let p = f.core.preview_removal(&id).await.unwrap();
        let project = f
            .core
            .remove_git(&id, p["fingerprint"].as_str().unwrap(), "project")
            .await
            .unwrap();
        assert_eq!(project["status"]["state"], "not_initialized");
        assert_eq!(project["settings"]["branchTargets"], json!({}));
        assert_eq!(project["settings"]["defaultRemote"], Value::Null);
        assert_eq!(project["settings"]["defaultBranch"], Value::Null);
        assert!(!path.join(".git").exists());
        assert!(path.join("base.txt").exists());
        assert!(
            std::fs::read_dir(f.temp.path().join("trash"))
                .unwrap()
                .any(|p| p.unwrap().path().join("HEAD").exists())
        );
        let p = f.core.preview_removal(&id).await.unwrap();
        assert_eq!(
            f.core
                .remove_git(&id, p["fingerprint"].as_str().unwrap(), "project")
                .await
                .unwrap_err()
                .code,
            "not_a_repository"
        );
        assert_eq!(
            f.core.history(10, None, Some(id)).await.unwrap()[0]["action"],
            "remove_git"
        );
    }
    #[tokio::test]
    async fn linked_worktree_git_file_is_rejected() {
        let f = Fixture::new();
        let repo = f.repo("repo");
        let path = f.temp.path().join("worktree");
        f.run(
            &repo,
            &["worktree", "add", "-b", "linked", path.to_str().unwrap()],
        );
        let path = revia_lib::platform::canonicalize(&path).unwrap();
        let id = f.add(&path).await;
        let p = f.core.preview_removal(&id).await.unwrap();
        assert_eq!(
            f.core
                .remove_git(&id, p["fingerprint"].as_str().unwrap(), "worktree")
                .await
                .unwrap_err()
                .code,
            "invalid_input"
        );
        assert!(path.join(".git").is_file());
    }
    #[tokio::test]
    async fn delete_local_validation_and_success_persist_unregister() {
        let f = Fixture::new();
        let path = f.repo("project");
        let id = f.add(&path).await;
        f.core
            .update_project(&id, json!({"favorite":true}))
            .await
            .unwrap();
        let p = f.core.preview_removal(&id).await.unwrap();
        let fp = p["fingerprint"].as_str().unwrap();
        assert_eq!(
            f.core
                .delete_project(&id, false, fp, "wrong")
                .await
                .unwrap_err()
                .code,
            "invalid_input"
        );
        std::fs::write(path.join("file"), "change").unwrap();
        assert_eq!(
            f.core
                .delete_project(&id, false, fp, "project")
                .await
                .unwrap_err()
                .code,
            "state_changed"
        );
        let p = f.core.preview_removal(&id).await.unwrap();
        let r = f
            .core
            .delete_project(&id, false, p["fingerprint"].as_str().unwrap(), "project")
            .await
            .unwrap();
        assert_eq!(r["repository"], "skipped");
        assert_eq!(r["local"], "trashed");
        assert!(!path.exists());
        assert_eq!(r["workspace"]["projects"], json!([]));
        let (settings, _) = revia_lib::settings::load(&f.temp.path().join("config")).unwrap();
        assert!(settings.manual_paths.is_empty());
        assert!(settings.project_settings.is_empty());
        assert_eq!(settings.settings["excludedPaths"], json!([]));
        assert_eq!(
            f.core.history(10, None, None).await.unwrap()[0]["action"],
            "delete_local"
        );
        assert!(
            Fixture::core(f.temp.path(), f.git.clone()).workspace()["projects"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    #[tokio::test]
    async fn deleting_scan_root_removes_root() {
        let f = Fixture::new();
        let path = f.repo("root");
        let w = f
            .core
            .registry_change("add_root", json!(path), Value::Null)
            .await
            .unwrap();
        let id = w["projects"][0]["id"].as_str().unwrap();
        let p = f.core.preview_removal(id).await.unwrap();
        let r = f
            .core
            .delete_project(id, false, p["fingerprint"].as_str().unwrap(), "root")
            .await
            .unwrap();
        assert_eq!(r["workspace"]["settings"]["roots"], json!([]));
    }
    #[tokio::test]
    async fn nested_missing_and_failed_trash_preserve_registration() {
        let mut f = Fixture::new();
        let path = f.repo("project");
        let id = f.add(&path).await;
        fail_trash(&mut f);
        let p = f.core.preview_removal(&id).await.unwrap();
        assert_eq!(
            f.core
                .remove_git(&id, p["fingerprint"].as_str().unwrap(), "project")
                .await
                .unwrap_err()
                .code,
            "io_error"
        );
        assert!(path.join(".git").is_dir());

        assert_eq!(
            f.core
                .delete_project(&id, false, p["fingerprint"].as_str().unwrap(), "project")
                .await
                .unwrap_err()
                .code,
            "io_error"
        );
        assert!(path.exists());
        assert!(f.core.project(&id).is_ok());
        let child = path.join("child");
        std::fs::create_dir(&child).unwrap();
        f.add(&child).await;
        let p = f.core.preview_removal(&id).await.unwrap();
        assert_eq!(
            f.core
                .delete_project(&id, false, p["fingerprint"].as_str().unwrap(), "project")
                .await
                .unwrap_err()
                .code,
            "invalid_input"
        );
        std::fs::rename(&path, f.temp.path().join("moved")).unwrap();
        assert_eq!(
            f.core.preview_removal(&id).await.unwrap_err().code,
            "path_not_found"
        );
        assert_eq!(
            f.core
                .delete_project(&id, false, "x", "project")
                .await
                .unwrap_err()
                .code,
            "path_not_found"
        );
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn symlink_substitution_is_rejected() {
        let f = Fixture::new();
        let path = f.repo("project");
        let id = f.add(&path).await;
        let p = f.core.preview_removal(&id).await.unwrap();
        let moved = f.temp.path().join("moved");
        std::fs::rename(&path, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &path).unwrap();
        assert_eq!(
            f.core
                .delete_project(&id, false, p["fingerprint"].as_str().unwrap(), "project")
                .await
                .unwrap_err()
                .code,
            "invalid_path"
        );
        assert!(moved.join("base.txt").exists());
    }
}

#[cfg(unix)]
mod removal_gh {
    use super::*;
    use revia_lib::EventSink;
    use std::sync::Mutex;
    #[derive(Default)]
    struct Events(Mutex<Vec<(String, Value)>>);
    impl revia_lib::EventSink for Events {
        fn emit(&self, n: &str, v: Value) {
            self.0.lock().unwrap().push((n.into(), v));
        }
    }
    struct Trash {
        root: PathBuf,
        events: Arc<Events>,
        fail: bool,
    }
    impl revia_lib::trash::Trash for Trash {
        fn move_to_trash(&self, path: &Path) -> std::io::Result<()> {
            assert!(
                self.root.join("remote-deleted").exists(),
                "Local trash ran before repository deletion"
            );
            self.events.emit("test:trash", json!(true));
            if self.fail {
                return Err(std::io::Error::other("Test failure"));
            }
            revia_lib::trash::Trash::move_to_trash(
                &revia_lib::trash::MoveTo(self.root.join("trash")),
                path,
            )
        }
    }
    fn fixture(scenario: &str, fail: bool) -> (Fixture, Arc<Events>) {
        let mut f = Fixture::new();
        let script = f.temp.path().join("gh-removal-mock");
        std::fs::write(&script,r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$MOCK_ROOT/calls"
scenario=$(cat "$MOCK_ROOT/scenario")
case "$1 $2" in
'auth status')
  if [ "$scenario" = logged-out ]; then echo 'not logged into github.com' >&2; exit 1; fi
  case "$scenario" in
  no-scope) echo "Token scopes: 'repo'" >&2 ;;
  unknown-scope) echo 'Logged in with environment token' >&2 ;;
  *) echo "Token scopes: 'repo', 'delete_repo'" >&2 ;;
  esac ;;
'repo view')
  case "$scenario" in
  forbidden) echo 'HTTP 403' >&2; exit 1 ;;
  absent) echo 'HTTP 404' >&2; exit 1 ;;
  network) echo 'connection refused' >&2; exit 1 ;;
  no-admin) echo '{"nameWithOwner":"team/project","visibility":"PRIVATE","isFork":true,"viewerCanAdminister":false}' ;;
  *) echo '{"nameWithOwner":"team/project","visibility":"PRIVATE","isFork":false,"viewerCanAdminister":true}' ;;
  esac ;;
'repo delete')
  case "$scenario" in
  no-scope) echo 'requires delete_repo scope' >&2; exit 1 ;;
  forbidden) echo 'HTTP 403' >&2; exit 1 ;;
  absent) echo 'HTTP 404' >&2; exit 1 ;;
  logged-out) echo 'not logged into github.com' >&2; exit 1 ;;
  esac
  touch "$MOCK_ROOT/remote-deleted" ;;
'auth refresh') echo 'First copy your one-time code: ABCD-1234 https://github.com/login/device' >&2 ;;
'api user') echo '{"login":"test","name":null,"avatar_url":null}' ;;
'config get') echo https ;;
*) echo 'unexpected mock command' >&2; exit 1 ;;
esac
"#).unwrap();
        common::executable(&script, 0o755);
        std::fs::write(f.temp.path().join("scenario"), scenario).unwrap();
        f.git.gh = if scenario == "no-gh" {
            f.temp.path().join("absent-gh")
        } else {
            script
        };
        f.git
            .env
            .insert("MOCK_ROOT".into(), f.temp.path().to_string_lossy().into());
        let events = Arc::new(Events::default());
        f.core = Core::new(Options {
            oauth: None,
            config_dir: f.temp.path().join("config"),
            data_dir: f.temp.path().join("data"),
            cache_dir: f.temp.path().join("cache"),
            git: f.git.clone(),
            events: events.clone(),
            trash: Arc::new(Trash {
                root: f.temp.path().to_path_buf(),
                events: events.clone(),
                fail,
            }),
        })
        .unwrap();
        (f, events)
    }
    async fn project(f: &Fixture) -> (PathBuf, String, Value) {
        let path = f.repo("project");
        f.run(
            &path,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/team/project.git",
            ],
        );
        let id = f.add(&path).await;
        let p = f.core.preview_removal(&id).await.unwrap();
        (path, id, p)
    }
    #[tokio::test]
    async fn check_repository_deletion_blockers_and_unknown_scopes() {
        for (scenario, blocker) in [
            ("no-gh", Some("gh_not_installed")),
            ("logged-out", Some("not_authenticated")),
            ("no-scope", Some("scope_missing")),
            ("no-admin", Some("no_permission")),
            ("forbidden", Some("no_permission")),
            ("absent", Some("not_found")),
            ("ok", None),
            ("unknown-scope", None),
        ] {
            let (f, _) = fixture(scenario, false);
            let (_, id, _) = project(&f).await;
            let check = f.core.check_repository_deletion(&id).await.unwrap();
            assert_eq!(check["blocker"], json!(blocker), "{scenario}");
            assert_eq!(check["nameWithOwner"], "team/project");
            if matches!(scenario, "no-gh" | "logged-out") {
                assert!(check["visibility"].is_null());
                assert!(check["isFork"].is_null());
            }
            if scenario == "ok" {
                assert_eq!(check["visibility"], "private");
                assert_eq!(check["isFork"], false);
            }
        }
        let (f, _) = fixture("network", false);
        let (_, id, _) = project(&f).await;
        assert_eq!(
            f.core
                .check_repository_deletion(&id)
                .await
                .unwrap_err()
                .code,
            "network_unreachable"
        );
    }
    #[tokio::test]
    async fn repository_delete_failure_never_touches_local_folder() {
        for (scenario, code) in [
            ("no-gh", "gh_not_installed"),
            ("no-scope", "scope_missing"),
            ("forbidden", "permission_denied"),
            ("absent", "remote_not_found"),
            ("logged-out", "not_authenticated"),
        ] {
            let (f, events) = fixture(scenario, false);
            let (path, id, p) = project(&f).await;
            assert_eq!(
                f.core
                    .delete_project(
                        &id,
                        true,
                        p["fingerprint"].as_str().unwrap(),
                        "team/project"
                    )
                    .await
                    .unwrap_err()
                    .code,
                code
            );
            assert!(path.join(".git").is_dir());
            assert!(f.core.project(&id).is_ok());
            assert!(
                !events
                    .0
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|(n, _)| n == "test:trash")
            );
        }
    }
    #[tokio::test]
    async fn repository_delete_precedes_trash_and_partial_success_retains_registration() {
        for fail in [false, true] {
            let (f, events) = fixture("ok", fail);
            let (path, id, p) = project(&f).await;
            let fp = p["fingerprint"].as_str().unwrap();
            assert_eq!(
                f.core
                    .delete_project(&id, true, fp, "project")
                    .await
                    .unwrap_err()
                    .code,
                "invalid_input"
            );
            let r = f
                .core
                .delete_project(&id, true, fp, "team/project")
                .await
                .unwrap();
            assert_eq!(r["repository"], "deleted");
            assert_eq!(r["local"], if fail { "failed" } else { "trashed" });
            assert_eq!(path.exists(), fail);
            if fail {
                assert_eq!(r["error"]["code"], "io_error");
                assert!(f.core.project(&id).is_ok());
            } else {
                assert!(r["workspace"]["projects"].as_array().unwrap().is_empty());
                assert!(
                    events
                        .0
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|(n, _)| n == "revia:projects-changed")
                );
            }
            let calls = std::fs::read_to_string(f.temp.path().join("calls")).unwrap();
            assert!(calls.lines().any(|s| s == "repo delete team/project --yes"));
            let history = f.core.history(20, None, None).await.unwrap();
            assert!(history.iter().any(|r| r["action"] == "delete_repository"
                && r["outcome"] == "succeeded"
                && r["params"]["repo"] == "team/project"));
            assert!(history.iter().any(|r| r["action"] == "delete_local"));
        }
    }
    #[tokio::test]
    async fn preview_uses_five_local_git_processes() {
        let (mut f, _) = fixture("ok", false);
        let (_, id, _) = project(&f).await;
        let wrapper = f.temp.path().join("git-count");
        std::fs::write(
            &wrapper,
            r#"#!/bin/sh
printf 'call\n' >> "$MOCK_GIT_CALLS"
exec "$REAL_GIT" "$@"
"#,
        )
        .unwrap();
        common::executable(&wrapper, 0o755);
        let calls = f.temp.path().join("git-calls");
        f.git
            .env
            .insert("REAL_GIT".into(), f.git.executable.to_string_lossy().into());
        f.git
            .env
            .insert("MOCK_GIT_CALLS".into(), calls.to_string_lossy().into());
        f.git.executable = wrapper;
        f.core = Fixture::core(f.temp.path(), f.git.clone());
        f.core.preview_removal(&id).await.unwrap();
        assert_eq!(std::fs::read_to_string(calls).unwrap().lines().count(), 5);
    }
    #[tokio::test]
    async fn delete_repository_without_github_remote_is_rejected() {
        let (f, _) = fixture("ok", false);
        let path = f.repo("project");
        let id = f.add(&path).await;
        let p = f.core.preview_removal(&id).await.unwrap();
        assert_eq!(
            f.core
                .delete_project(&id, true, p["fingerprint"].as_str().unwrap(), "project")
                .await
                .unwrap_err()
                .code,
            "invalid_input"
        );
        assert_eq!(
            f.core
                .check_repository_deletion(&id)
                .await
                .unwrap_err()
                .code,
            "invalid_input"
        );
        assert!(path.exists());
    }
    #[tokio::test]
    async fn refresh_login_validates_scopes_authentication_and_arguments() {
        let (f, events) = fixture("ok", false);
        for scopes in [
            vec![],
            vec!["repo".into()],
            vec!["delete_repo".into(), "repo".into()],
        ] {
            assert_eq!(
                f.core
                    .github_login_start(Some(scopes))
                    .await
                    .unwrap_err()
                    .code,
                "invalid_input"
            );
        }
        let login = f
            .core
            .github_login_start(Some(vec!["delete_repo".into()]))
            .await
            .unwrap();
        for _ in 0..200 {
            if events
                .0
                .lock()
                .unwrap()
                .iter()
                .any(|(_, v)| v["loginId"] == login["loginId"] && v["kind"] == "succeeded")
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(
            events
                .0
                .lock()
                .unwrap()
                .iter()
                .any(|(_, v)| v["loginId"] == login["loginId"] && v["kind"] == "succeeded")
        );
        assert!(
            events
                .0
                .lock()
                .unwrap()
                .iter()
                .any(|(_, v)| v["kind"] == "code")
        );
        let calls = std::fs::read_to_string(f.temp.path().join("calls")).unwrap();
        assert!(
            calls
                .lines()
                .any(|s| s == "auth refresh --hostname github.com --scopes delete_repo")
        );
        let (f, _) = fixture("logged-out", false);
        assert_eq!(
            f.core
                .github_login_start(Some(vec!["delete_repo".into()]))
                .await
                .unwrap_err()
                .code,
            "not_authenticated"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn fsmonitor_is_disabled_for_repository_reads() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let hook = f.temp.path().join("monitor");
    let marker = f.temp.path().join("monitor-ran");
    std::fs::write(&hook, "#!/bin/sh\ntouch \"$MONITOR_MARKER\"\n").unwrap();
    common::executable(&hook, 0o700);
    f.run(&p, &["config", "core.fsmonitor", hook.to_str().unwrap()]);
    let mut git = f.git.clone();
    git.env.insert(
        "MONITOR_MARKER".into(),
        marker.to_string_lossy().into_owned(),
    );
    git.local(&p, &["status", "--porcelain"]).await.unwrap();
    git.run(&p, &["status".into(), "--porcelain".into()], false, None)
        .await
        .unwrap();
    assert!(!marker.exists());
}

#[tokio::test]
async fn remote_credentials_rejected_at_entry_points() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let id = f.add(&p).await;
    f.core
        .mutate(
            &id,
            "add_remote",
            json!({"name":"origin","url":"https://github.com/o/r.git"}),
        )
        .await
        .unwrap();
    for url in [
        "https://user@host/x.git",
        "https://user:pass@host/x.git",
        "https://token@github.com/o/r.git",
        "ssh://git:pass@host/x.git",
    ] {
        for action in ["add_remote", "set_remote_url"] {
            let error = f
                .core
                .mutate(
                    &id,
                    action,
                    json!({"name":if action == "add_remote" {"new"} else {"origin"},"url":url}),
                )
                .await
                .unwrap_err();
            assert_eq!(error.code, "invalid_input");
            assert!(!json!(error).to_string().contains(url));
        }
        let error = f.core.plan(json!({"kind":"clone","items":[{"itemId":"clone","url":url,"parentDir":f.temp.path(),"folderName":"cloned"}]})).await.unwrap_err();
        assert_eq!(error.code, "invalid_input");
        assert!(!json!(error).to_string().contains(url));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn private_saved_files_and_corrupt_backup() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let config = f.temp.path().join("config");
    let settings = revia_lib::settings::Settings::default();
    revia_lib::settings::save(&config, &settings).unwrap();
    std::fs::set_permissions(
        config.join("settings.json"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    revia_lib::settings::save(&config, &settings).unwrap();
    std::fs::write(config.join("settings.json"), "broken").unwrap();
    std::fs::set_permissions(
        config.join("settings.json"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    revia_lib::settings::load(&config).unwrap();
    f.core.clear_history().await.unwrap();
    let p = f.repo("repo");
    f.add(&p).await;
    let image = f.temp.path().join("image.png");
    std::fs::write(&image, b"image").unwrap();
    let result = f.core.copy_background_image(&image).await.unwrap();
    for dir in [
        config.clone(),
        f.temp.path().join("data"),
        f.temp.path().join("cache"),
        f.temp.path().join("data/backgrounds"),
    ] {
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700,
            "{dir:?}"
        );
    }
    let mut files = std::fs::read_dir(&config)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    files.extend([
        f.temp.path().join("data/history.jsonl"),
        f.temp.path().join("cache/projects.json"),
        f.temp
            .path()
            .join("data/backgrounds")
            .join(result["id"].as_str().unwrap()),
    ]);
    for file in files {
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600,
            "{file:?}"
        );
    }
}

#[tokio::test]
async fn clean_checkout_failure_preserves_existing_stash() {
    let f = Fixture::new();
    let p = f.repo("repo");
    std::fs::write(p.join("base.txt"), "stashed").unwrap();
    f.run(&p, &["stash", "push"]);
    let sha = f.run(&p, &["rev-parse", "refs/stash"]);
    f.run(&p, &["branch", "busy"]);
    f.run(
        &p,
        &[
            "worktree",
            "add",
            f.temp.path().join("worktree").to_str().unwrap(),
            "busy",
        ],
    );
    let id = f.add(&p).await;
    assert!(
        f.core
            .mutate(
                &id,
                "checkout_branch",
                json!({"branch":"busy","ifDirty":"stash"})
            )
            .await
            .is_err()
    );
    assert_eq!(f.run(&p, &["rev-parse", "refs/stash"]), sha);
    assert_eq!(
        std::fs::read_to_string(p.join("base.txt")).unwrap(),
        "base\n"
    );
}

#[cfg(unix)]
fn fetch_wrapper(f: &Fixture, mode: &str) -> Git {
    let wrapper = f.temp.path().join("wrapped-git");
    std::fs::write(
        &wrapper,
        r#"#!/bin/sh
fetch=false
for arg do
  if [ "$arg" = fetch ]; then fetch=true; fi
done
if $fetch && [ "$CHANGE_MODE" = fail ]; then exit 1; fi
"$REAL_GIT" "$@"
result=$?
if $fetch && [ "$result" = 0 ]; then
  case "$CHANGE_MODE" in
    branch) "$REAL_GIT" checkout -b switched >/dev/null 2>&1 ;;
    head) "$REAL_GIT" commit --allow-empty -m moved >/dev/null 2>&1 ;;
    tree) echo changed > base.txt ;;
    untracked) echo changed > untracked.txt ;;
  esac
fi
exit "$result"
"#,
    )
    .unwrap();
    common::executable(&wrapper, 0o700);
    let mut git = f.git.clone();
    git.env.insert(
        "REAL_GIT".into(),
        git.executable.to_string_lossy().into_owned(),
    );
    git.env.insert("CHANGE_MODE".into(), mode.into());
    git.executable = wrapper;
    git
}

#[cfg(unix)]
#[tokio::test]
async fn fetch_changes_block_pull_and_sync_before_merge() {
    for kind in ["pull", "sync"] {
        for mode in ["branch", "head", "tree", "untracked"] {
            let mut f = Fixture::new();
            let p = f.repo("repo");
            let bare = f.remote(&p);
            let other = f.clone_repo(&bare, "other");
            f.write_commit(&other, "remote.txt", "remote");
            f.run(&other, &["push"]);
            let original_head = f.run(&p, &["rev-parse", "HEAD"]);
            f.core = Fixture::core(f.temp.path(), fetch_wrapper(&f, mode));
            let id = f.add(&p).await;
            let result = f.op(kind, &id, json!({"mode":"ff_only"})).await;
            assert_eq!(
                result["state"], "needs_confirmation",
                "{kind}/{mode}: {result}"
            );
            assert_eq!(result["reason"], "state_changed");
            assert!(!p.join("remote.txt").exists());
            if mode != "head" {
                assert_eq!(f.run(&p, &["rev-parse", "HEAD"]), original_head);
            }
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn rejected_push_fetch_failure_keeps_original_error() {
    let mut f = Fixture::new();
    let p = f.repo("repo");
    let bare = f.remote(&p);
    let other = f.clone_repo(&bare, "other");
    f.write_commit(&other, "remote.txt", "remote");
    f.run(&other, &["push"]);
    f.write_commit(&p, "local.txt", "local");
    f.core = Fixture::core(f.temp.path(), fetch_wrapper(&f, "fail"));
    let id = f.add(&p).await;
    let result = f
        .op("push", &id, json!({"force":false,"setUpstream":false}))
        .await;
    assert_eq!(result["state"], "failed");
    assert_eq!(result["error"]["code"], "push_rejected");
}

#[cfg(unix)]
#[tokio::test]
async fn checkout_noop_stash_does_not_restore_previous_stash() {
    let mut f = Fixture::new();
    let p = f.repo("repo");
    std::fs::write(p.join("base.txt"), "old stash").unwrap();
    f.run(&p, &["stash", "push"]);
    let sha = f.run(&p, &["rev-parse", "refs/stash"]);
    f.run(&p, &["branch", "busy"]);
    f.run(
        &p,
        &[
            "worktree",
            "add",
            f.temp.path().join("worktree").to_str().unwrap(),
            "busy",
        ],
    );
    std::fs::write(p.join("base.txt"), "new changes").unwrap();
    let mut git = f.git.clone();
    let script = f.temp.path().join("noop-stash-git");
    std::fs::write(
        &script,
        r#"#!/bin/sh
previous=
for arg do
    if [ "$previous" = stash ] && [ "$arg" = push ]; then exit 0; fi
    previous=$arg
done
exec "$REAL_GIT" "$@"
"#,
    )
    .unwrap();
    common::executable(&script, 0o700);
    git.env.insert(
        "REAL_GIT".into(),
        git.executable.to_string_lossy().into_owned(),
    );
    git.executable = script;
    f.core = Fixture::core(f.temp.path(), git);
    let id = f.add(&p).await;
    let error = f
        .core
        .mutate(
            &id,
            "checkout_branch",
            json!({"branch":"busy","ifDirty":"stash"}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "unknown");
    assert_eq!(f.run(&p, &["rev-parse", "refs/stash"]), sha);
    assert_eq!(
        std::fs::read_to_string(p.join("base.txt")).unwrap(),
        "new changes"
    );
}

#[tokio::test]
async fn repository_lock_is_reported_and_retained() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let lock = p.join(".git/index.lock");
    std::fs::write(&lock, b"other process").unwrap();
    let error = f.git.local_write(&p, &["add", "-A"]).await.unwrap_err();
    assert_eq!(error.code, "repo_locked");
    assert_eq!(std::fs::read(&lock).unwrap(), b"other process");
}
const CONFLICT_BLOCK: &str =
    "start\n<<<<<<< HEAD\nlocal\n=======\nremote\n>>>>>>> refs/remotes/origin/main\nend\n";
#[tokio::test]
async fn sync_conflict_cannot_be_finished_or_pushed_with_markers_left() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let bare = f.remote(&p);
    let other = f.clone_repo(&bare, "other");
    let id = f.add(&p).await;
    f.core
        .update_general(json!({"sync":{"diverged":"merge","commit":true}}))
        .await
        .unwrap();
    f.write_commit(&other, "base.txt", "remote\n");
    f.run(&other, &["push"]);
    let remote_before = f.run(&bare, &["rev-parse", "main"]);
    std::fs::write(p.join("base.txt"), "local\n").unwrap();
    let item = f.op("sync", &id, json!({})).await;
    assert_eq!(item["state"], "conflict", "{item}");
    assert_eq!(
        f.core.conflicts(&id).await.unwrap()["files"][0]["hasMarkers"],
        true
    );
    let error = f
        .core
        .mutate(&id, "mark_resolved", json!({"paths":["base.txt"]}))
        .await
        .unwrap_err();
    assert_eq!(error.code, "conflict_markers");
    assert_eq!(error.detail.as_deref(), Some("base.txt"));
    assert_eq!(
        f.core.conflicts(&id).await.unwrap()["files"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    f.run(&p, &["add", "--", "base.txt"]);
    assert_eq!(
        f.core
            .mutate(&id, "continue_in_progress", json!({}))
            .await
            .unwrap_err()
            .code,
        "conflict_markers"
    );
    assert_eq!(f.state(&id).await["inProgress"], "merge");
    assert_eq!(f.run(&bare, &["rev-parse", "main"]), remote_before);
    std::fs::write(p.join("base.txt"), "local\nremote\n").unwrap();
    f.core
        .mutate(&id, "mark_resolved", json!({"paths":["base.txt"]}))
        .await
        .unwrap();
    f.core
        .mutate(&id, "continue_in_progress", json!({}))
        .await
        .unwrap();
    assert!(f.state(&id).await["inProgress"].is_null());
    let item = f.op("sync", &id, json!({})).await;
    assert_eq!(item["state"], "succeeded", "{item}");
    assert_eq!(f.run(&bare, &["show", "main:base.txt"]), "local\nremote");
}
#[tokio::test]
async fn commits_and_pushes_stop_only_for_new_conflict_markers() {
    let f = Fixture::new();
    let p = f.repo("repo");
    let bare = f.remote(&p);
    let id = f.add(&p).await;
    f.write_commit(
        &p,
        "guide.md",
        &format!("Example\n=======\n\n{CONFLICT_BLOCK}"),
    );
    f.run(&p, &["push"]);
    std::fs::write(
        p.join("guide.md"),
        format!("Example v2\n=======\n\n{CONFLICT_BLOCK}"),
    )
    .unwrap();
    std::fs::write(
        p.join("notes.md"),
        "Title\n=======\n<<<<<<< not a conflict\n",
    )
    .unwrap();
    let item = f.op("commit", &id, json!({"message":"docs"})).await;
    assert_eq!(item["state"], "succeeded", "{item}");
    let push = json!({"force":false,"setUpstream":false});
    let item = f.op("push", &id, push.clone()).await;
    assert_eq!(item["state"], "succeeded", "{item}");
    std::fs::write(p.join("base.txt"), CONFLICT_BLOCK).unwrap();
    let item = f.op("commit", &id, json!({"message":"broken"})).await;
    assert_eq!(item["state"], "failed", "{item}");
    assert_eq!(item["error"]["code"], "conflict_markers");
    assert_eq!(item["error"]["detail"], "base.txt");
    let item = f
        .op(
            "commit",
            &id,
            json!({"message":"broken","paths":["base.txt"]}),
        )
        .await;
    assert_eq!(item["error"]["code"], "conflict_markers", "{item}");
    f.core
        .update_general(json!({"sync":{"commit":true}}))
        .await
        .unwrap();
    let item = f.op("sync", &id, json!({})).await;
    assert_eq!(item["error"]["code"], "conflict_markers", "{item}");
    f.run(&p, &["commit", "-am", "outside"]);
    let remote_before = f.run(&bare, &["rev-parse", "main"]);
    let item = f.op("push", &id, push.clone()).await;
    assert_eq!(item["error"]["code"], "conflict_markers", "{item}");
    let item = f.op("sync", &id, json!({})).await;
    assert_eq!(item["error"]["code"], "conflict_markers", "{item}");
    assert_eq!(f.run(&bare, &["rev-parse", "main"]), remote_before);
    std::fs::write(p.join("base.txt"), "fixed\n").unwrap();
    f.run(&p, &["commit", "-am", "fix"]);
    let item = f.op("push", &id, push).await;
    assert_eq!(item["state"], "succeeded", "{item}");
    assert_eq!(f.run(&bare, &["show", "main:base.txt"]), "fixed");
}
