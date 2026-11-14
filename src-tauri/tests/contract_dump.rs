use revia_lib::{
    NoEvents,
    core::{Core, Options},
    git::Git,
    update::{self, Target},
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc, time::Duration};

fn write(dir: &Path, name: &str, value: &Value) {
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_vec_pretty(value).unwrap(),
    )
    .unwrap();
}

async fn finish(core: &Arc<Core>, snapshot: Value) -> Value {
    let id = snapshot["operationId"].as_str().unwrap().to_string();
    for _ in 0..600 {
        let current = core.operation(&id).unwrap();
        if current["state"] == "finished" {
            return current;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("operation did not finish");
}

#[tokio::test]
async fn dump_contract_samples() {
    let (Ok(out), Ok(workspace)) = (
        std::env::var("REVIA_CONTRACT_DUMP"),
        std::env::var("REVIA_TEST_WORKSPACE"),
    ) else {
        return;
    };
    let out = Path::new(&out);
    std::fs::create_dir_all(out).unwrap();
    let home = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(Path::new(&workspace).join("projects")).unwrap();
    let root = root.to_string_lossy().into_owned();
    let config = home.path().join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("settings.json"),
        serde_json::to_vec(&json!({
            "schemaVersion": 1,
            "settings": {"roots": [{"id": revia_lib::registry::id(&root), "path": root, "maxDepth": 4, "enabled": true, "exists": true}]},
        }))
        .unwrap(),
    )
    .unwrap();

    let core = Core::new(Options {
        oauth: None,
        trash: Arc::new(revia_lib::trash::MoveTo(home.path().join("trash"))),
        config_dir: config,
        data_dir: home.path().join("data"),
        cache_dir: home.path().join("cache"),
        git: Git::default(),
        events: Arc::new(NoEvents),
    })
    .unwrap();

    let scanned = core.scan().await.unwrap();
    write(out, "workspace", &scanned);
    write(out, "app_state", &core.app_state().await);

    let projects = scanned["projects"].as_array().unwrap().clone();
    let id_of = |name: &str| -> String {
        projects
            .iter()
            .find(|p| p["name"] == name)
            .unwrap_or_else(|| panic!("project {name} not found"))["id"]
            .as_str()
            .unwrap()
            .to_string()
    };

    let mut details = vec![];
    let mut changes = vec![];
    let mut diffs = vec![];
    let mut logs = vec![];
    let mut commit_details = vec![];
    let mut conflicts = vec![];
    for project in &projects {
        let id = project["id"].as_str().unwrap();
        details.push(core.detail(id).await.unwrap());
        let files = core.changes(id).await.unwrap();
        for file in &files {
            let path = file["path"].as_str().unwrap().to_string();
            diffs.push(
                core.diff(id, path, json!({"kind": "worktree"}))
                    .await
                    .unwrap(),
            );
        }
        changes.push(json!(files));
        if project["status"]["state"] != "not_initialized" {
            let log = core.commit_log(id, None, 20, 0).await.unwrap();
            if let Some(first) = log.first() {
                let sha = first["sha"].as_str().unwrap().to_string();
                let detail = core.commit_detail(id, sha.clone()).await.unwrap();
                if let Some(file) = detail["files"].as_array().and_then(|f| f.first()) {
                    let path = file["path"].as_str().unwrap().to_string();
                    diffs.push(
                        core.diff(id, path, json!({"kind": "commit", "sha": sha}))
                            .await
                            .unwrap(),
                    );
                }
                commit_details.push(detail);
            }
            logs.push(json!(log));
            conflicts.push(core.conflicts(id).await.unwrap());
        }
    }
    write(out, "details", &json!(details));
    write(out, "changes", &json!(changes));
    write(out, "diffs", &json!(diffs));
    write(out, "logs", &json!(logs));
    write(out, "commit_details", &json!(commit_details));
    write(out, "conflicts", &json!(conflicts));

    let all = |extra: Value| -> Vec<Value> {
        projects
            .iter()
            .map(|p| {
                let mut item = json!({"itemId": p["id"], "projectId": p["id"]});
                for (key, value) in extra.as_object().unwrap() {
                    item[key] = value.clone();
                }
                item
            })
            .collect()
    };
    let site = Path::new(&root).join("group/plain-site");
    std::fs::write(site.join(".env"), "KEY=value\n").unwrap();
    std::fs::write(site.join(".DS_Store"), "").unwrap();
    std::fs::create_dir_all(site.join("node_modules/pkg")).unwrap();
    std::fs::write(site.join("node_modules/pkg/index.js"), "").unwrap();
    std::fs::write(site.join("debug.log"), "").unwrap();
    let scan = core
        .scan_ignore_candidates(&id_of("plain-site"))
        .await
        .unwrap();
    assert!(scan["suggestions"].as_array().unwrap().len() >= 4, "{scan}");
    assert!(!site.join(".git").exists());
    write(out, "ignore_scan", &scan);

    let mut plans = vec![];
    for request in [
        json!({"kind": "sync", "items": all(json!({}))}),
        json!({"kind": "fetch", "items": all(json!({}))}),
        json!({"kind": "pull", "items": all(json!({"mode": "ff_only"}))}),
        json!({"kind": "pull", "items": all(json!({"mode": "merge"}))}),
        json!({"kind": "push", "items": all(json!({"force": false, "setUpstream": false}))}),
        json!({"kind": "push", "items": all(json!({"force": true, "setUpstream": false}))}),
        json!({"kind": "commit", "items": all(json!({"message": "確認", "paths": null}))}),
        json!({"kind": "merge", "items": [{"itemId": id_of("feature-work"), "projectId": id_of("feature-work"), "source": "feature/login", "target": "feature/new-ui", "mode": "default"}]}),
    ] {
        plans.push(core.plan(request).await.unwrap());
    }
    for request in [
        json!({"kind": "publish", "items": [
            {"itemId": id_of("local-only"), "projectId": id_of("local-only"), "owner": null, "name": "local-only", "visibility": "private", "description": null, "initialCommitMessage": null, "ignore": null},
            {"itemId": id_of("plain-site"), "projectId": id_of("plain-site"), "owner": null, "name": "plain-site", "visibility": "public", "description": "確認", "initialCommitMessage": "最初のコミット", "ignore": ["/.env", ".DS_Store", "node_modules/"]},
        ]}),
        json!({"kind": "clone", "items": [{"itemId": "clone:1", "url": format!("{workspace}/remotes/synced-app.git"), "parentDir": home.path(), "folderName": "cloned"}]}),
    ] {
        plans.push(core.plan(request).await.unwrap());
    }
    write(out, "plans", &json!(plans));

    let started = core
        .start(json!({"kind": "clone", "items": [{"itemId": "clone:1", "url": format!("{workspace}/remotes/synced-app.git"), "parentDir": home.path(), "folderName": "cloned"}]}))
        .await
        .unwrap();
    write(out, "clone_operation", &finish(&core, started).await);

    if std::env::var("REVIA_CONTRACT_GITHUB").is_ok() {
        let status = core.github_status(true).await.unwrap();
        if status["authenticated"] == true {
            write(
                out,
                "github_owners",
                &json!(core.github_list_owners().await.unwrap()),
            );
            write(
                out,
                "github_repos",
                &json!(core.github_list_repos(true).await.unwrap()),
            );
        }
        write(out, "github_status", &status);
    }

    write(
        out,
        "merge_preview",
        &core
            .preview_merge(&id_of("feature-work"), "feature/login", "feature/new-ui")
            .await
            .unwrap(),
    );
    write(
        out,
        "rebase_preview",
        &core
            .preview_rebase(&id_of("feature-work"), "feature/login")
            .await
            .unwrap(),
    );
    write(
        out,
        "reset_preview",
        &core
            .preview_reset(&id_of("ahead-api"), "HEAD~1", "mixed")
            .await
            .unwrap(),
    );

    let mut snapshots = vec![];
    let started = core
        .start(json!({"kind": "sync", "items": all(json!({}))}))
        .await
        .unwrap();
    snapshots.push(started.clone());
    snapshots.push(finish(&core, started).await);
    let started = core
        .start(json!({"kind": "commit", "items": [{"itemId": id_of("dirty-notes"), "projectId": id_of("dirty-notes"), "message": "契約確認用のコミット", "paths": null}]}))
        .await
        .unwrap();
    snapshots.push(finish(&core, started).await);
    write(out, "operations", &json!(snapshots));

    write(
        out,
        "history",
        &json!(core.history(100, None, None).await.unwrap()),
    );
    write(out, "workspace_after", &core.workspace());

    let mut previews = vec![];
    for name in ["ahead-api", "diverged-cli", "plain-site", "local-only"] {
        previews.push(core.preview_removal(&id_of(name)).await.unwrap());
    }
    write(out, "removal_previews", &json!(previews));
    let preview = core.preview_removal(&id_of("feature-work")).await.unwrap();
    write(
        out,
        "removed_git",
        &core
            .remove_git(
                &id_of("feature-work"),
                preview["fingerprint"].as_str().unwrap(),
                "feature-work",
            )
            .await
            .unwrap(),
    );
    let preview = core.preview_removal(&id_of("local-only")).await.unwrap();
    let removed = core
        .delete_project(
            &id_of("local-only"),
            false,
            preview["fingerprint"].as_str().unwrap(),
            "local-only",
        )
        .await
        .unwrap();
    assert_eq!(removed["local"], "trashed");
    assert!(!Path::new(preview["path"].as_str().unwrap()).exists());
    write(out, "removal_result", &removed);

    let release = json!({
        "tag_name": "v9.0.0",
        "draft": false,
        "prerelease": false,
        "name": "Revia 9.0.0",
        "body": "## What's new\n\n- Contract sample",
        "published_at": "2026-11-14T00:00:00Z",
        "html_url": "https://github.com/Shake1227/Revia/releases/tag/v9.0.0",
        "assets": [
            {"name": "Revia_9.0.0_universal.dmg", "size": 1024, "browser_download_url": "https://github.com/Shake1227/Revia/releases/download/v9.0.0/Revia_9.0.0_universal.dmg", "digest": format!("sha256:{}", "0".repeat(64))},
            {"name": "Revia_9.0.0_x64-setup.exe", "size": 2048, "browser_download_url": "https://github.com/Shake1227/Revia/releases/download/v9.0.0/Revia_9.0.0_x64-setup.exe", "digest": null},
            {"name": "Revia_9.0.0_amd64.deb", "size": 4096, "browser_download_url": "https://github.com/Shake1227/Revia/releases/download/v9.0.0/Revia_9.0.0_amd64.deb"},
        ],
    })
    .to_string();
    let updates: Vec<Value> = [
        Target::new("macos", "aarch64", ""),
        Target::new("windows", "x86_64", ""),
        Target::new("linux", "x86_64", "deb"),
        Target::new("linux", "aarch64", "rpm"),
    ]
    .iter()
    .map(|target| update::info_from_release("0.1.0", &release, target, revia_lib::now()).unwrap())
    .collect();
    assert_eq!(updates[0]["latest"]["asset"]["kind"], "dmg");
    assert_eq!(updates[1]["latest"]["asset"]["kind"], "nsis");
    assert_eq!(updates[2]["latest"]["asset"]["kind"], "deb");
    assert!(updates[3]["latest"]["asset"].is_null());
    write(out, "update_infos", &json!(updates));
    write(
        out,
        "update_info_none",
        &update::info_from_release(
            "0.1.0",
            &json!({"tag_name": "v9.0.0", "draft": true, "html_url": "https://github.com/Shake1227/Revia/releases/tag/v9.0.0"}).to_string(),
            &Target::new("macos", "aarch64", ""),
            revia_lib::now(),
        )
        .unwrap(),
    );
}
