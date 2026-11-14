pub use crate::dto::{OperationKind, OperationRequest};
use crate::{
    core::Core,
    error::{AppError, Result},
    git, status,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| AppError::new("invalid_input"))
}
impl Core {
    async fn validate_request(&self, request: &Value, start: bool) -> Result<()> {
        let _: OperationRequest = serde_json::from_value(request.clone())?;
        let kind = text(request, "kind")?;
        if !matches!(
            kind,
            "fetch" | "pull" | "push" | "commit" | "sync" | "merge" | "publish" | "clone"
        ) {
            return Err(AppError::new("invalid_input"));
        }
        let items = request["items"]
            .as_array()
            .ok_or_else(|| AppError::new("invalid_input"))?;
        if items.is_empty() {
            return Err(AppError::new("invalid_input"));
        }
        let mut seen = BTreeSet::new();
        for item in items {
            if kind == "clone" {
                if !seen.insert(text(item, "itemId")?) {
                    return Err(AppError::new("invalid_input"));
                }
                crate::publish::clone_destination(item)?;
                continue;
            }
            if kind == "publish" && start && item["fingerprint"].as_str().is_none() {
                return Err(AppError::new("invalid_input"));
            }
            let id = text(item, "projectId")?;
            if text(item, "itemId")? != id || !seen.insert(id) {
                return Err(AppError::new("invalid_input"));
            }
            let p = self.project(id)?;
            let root = Path::new(p["path"].as_str().unwrap());
            if let Some(f) = item.get("fingerprint")
                && !f.is_null()
                && !f.is_string()
            {
                return Err(AppError::new("invalid_input"));
            }
            match kind {
                "merge" => {
                    for key in ["source", "target"] {
                        text(item, key)?;
                    }
                    if !matches!(text(item, "mode")?, "default" | "no_ff" | "ff_only")
                        || (start && item["fingerprint"].as_str().is_none())
                    {
                        return Err(AppError::new("invalid_input"));
                    }
                }
                "pull" => {
                    let mode = text(item, "mode")?;
                    if !matches!(mode, "ff_only" | "merge" | "rebase")
                        || (start && mode != "ff_only" && item["fingerprint"].as_str().is_none())
                    {
                        return Err(AppError::new("invalid_input"));
                    }
                }
                "push" => {
                    if !item["force"].is_boolean()
                        || !item["setUpstream"].is_boolean()
                        || (start
                            && item["force"] == true
                            && item["fingerprint"].as_str().is_none())
                    {
                        return Err(AppError::new("invalid_input"));
                    }
                }
                "commit" => {
                    if text(item, "message")?.trim().is_empty() {
                        return Err(AppError::new("invalid_input"));
                    }
                    if !item["paths"].is_null() {
                        for p in item["paths"]
                            .as_array()
                            .ok_or_else(|| AppError::new("invalid_input"))?
                        {
                            git::git_path(
                                root,
                                p.as_str().ok_or_else(|| AppError::new("invalid_path"))?,
                            )?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    async fn fingerprint(&self, p: &Value) -> Result<String> {
        Ok(self.fingerprint_state(p).await?.0)
    }
    async fn fingerprint_state(&self, p: &Value) -> Result<(String, Option<String>)> {
        let root = Path::new(p["path"].as_str().unwrap());
        let g = &self.options.git;
        let mut data = json!({"branch":p["status"]["branch"],"targets":p["status"]["targets"],"changes":p["status"]["changes"],"progress":p["status"]["inProgress"]});
        data["head"] = json!(g.local(root, &["rev-parse", "--verify", "HEAD"]).await.ok());
        data["porcelain"] = json!(
            g.local(root, &["status", "--porcelain=v1", "-z"])
                .await
                .ok()
        );
        for kind in ["push", "pull"] {
            let t = &p["status"]["targets"][kind]["target"];
            data[kind] = json!(
                g.local(root, &["rev-parse", "--verify", &status::tracking(t)])
                    .await
                    .ok()
            );
        }
        let push_sha = data["push"].as_str().map(|s| s.trim().to_string());
        Ok((
            format!("{:x}", Sha256::digest(serde_json::to_vec(&data)?)),
            push_sha,
        ))
    }
    async fn local_guard(&self, root: &Path) -> Result<Value> {
        let g = &self.options.git;
        Ok(json!({
            "branch": g.local(root, &["symbolic-ref", "-q", "HEAD"]).await.ok(),
            "head": g.local(root, &["rev-parse", "--verify", "HEAD"]).await?,
            "status": g.local(root, &["status", "--porcelain=v1", "-z", "--untracked-files=all"]).await?,
        }))
    }
    pub async fn plan(self: &Arc<Self>, request: Value) -> Result<Value> {
        self.validate_request(&request, false).await?;
        let kind = text(&request, "kind")?.to_string();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(8));
        let mut jobs = tokio::task::JoinSet::new();
        for (index, item) in request["items"]
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .enumerate()
        {
            let core = self.clone();
            let kind = kind.clone();
            let semaphore = semaphore.clone();
            jobs.spawn(async move {
                let _permit = semaphore.acquire_owned().await.unwrap();
                if kind == "clone" {
                    return Ok((index, core.clone_plan(&item)?));
                }
                let id = text(&item, "projectId")?;
                core.refresh(Some(vec![id.into()])).await?;
                let p = core.project(id)?;
                Ok::<_, AppError>((index, core.plan_item(&kind, &item, &p).await?))
            });
        }
        let mut plans = vec![];
        while let Some(r) = jobs.join_next().await {
            plans.push(r.map_err(|_| AppError::new("io_error"))??);
        }
        plans.sort_by_key(|p| p.0);
        Ok(json!({"kind":kind,"items":plans.into_iter().map(|p|p.1).collect::<Vec<_>>()}))
    }
    async fn plan_item(&self, kind: &str, item: &Value, p: &Value) -> Result<Value> {
        if kind == "publish" {
            return self.publish_plan(item, p).await;
        }
        if kind == "merge" {
            let preview = self
                .preview_merge(
                    text(item, "projectId")?,
                    text(item, "source")?,
                    text(item, "target")?,
                )
                .await?;
            let mut blocker = preview["blocker"].clone();
            if blocker.is_null() && preview["upToDate"] == true {
                blocker = json!("nothing_to_do");
            }
            if blocker.is_null() && item["mode"] == "ff_only" && preview["fastForward"] == false {
                blocker = json!("diverged");
            }
            let mut steps = vec![];
            if preview["needsCheckout"] == true {
                steps.push(json!({"code":"checkout","branch":item["target"]}));
            }
            steps.push(merge_step(item, &preview));
            let warnings = if preview["conflicts"]["status"] == "conflicts" {
                vec![json!({"code":"conflicts_likely","paths":preview["conflicts"]["paths"]})]
            } else {
                vec![]
            };
            return Ok(
                json!({"itemId":item["itemId"],"projectId":item["projectId"],"executable":blocker.is_null(),"blocker":blocker,"steps":steps,"risk":"caution","warnings":warnings,"fingerprint":preview["fingerprint"]}),
            );
        }
        let force = kind == "push" && item["force"] == true;
        let s = &p["status"];
        let mut blocker = None;
        let mut steps = vec![];
        let mut warnings = vec![];
        let state = s["state"].as_str().unwrap_or("unknown");
        if matches!(state, "missing" | "not_initialized")
            || (kind != "fetch" && matches!(state, "conflicted" | "in_progress"))
        {
            blocker = Some(state);
        }
        if state == "unknown" {
            blocker = Some("invalid_input");
        }
        if blocker.is_none() && kind != "fetch" && kind != "commit" && s["detached"] == true {
            blocker = Some("detached");
        }
        if blocker.is_none()
            && kind != "commit"
            && s["remotes"].as_array().is_none_or(Vec::is_empty)
        {
            blocker = Some("no_remote");
        }
        if blocker.is_none() && kind != "commit" && kind != "fetch" && s["unborn"] == true {
            blocker = Some("unborn");
        }
        let push = &s["targets"]["push"];
        let pull = &s["targets"]["pull"];
        for t in if kind == "sync" {
            vec![push, pull]
        } else if kind == "pull" {
            vec![pull]
        } else if kind == "push" {
            vec![push]
        } else {
            vec![]
        } {
            if blocker.is_some() {
                break;
            }
            if !t["savedInvalid"].is_null() {
                blocker = Some("target_invalid");
            } else if t["target"].is_null() && !(kind == "sync" && std::ptr::eq(t, pull)) {
                blocker = Some("target_unresolved");
            }
        }
        let changes = s["changes"]["total"].as_u64().unwrap_or(0);
        if blocker.is_none() && kind == "commit" && changes == 0 {
            blocker = Some("nothing_to_do");
        }
        if blocker.is_none()
            && kind == "push"
            && force
            && !self.state.lock().unwrap().settings.settings["general"]["safety"]["allowForcePush"]
                .as_bool()
                .unwrap_or(false)
        {
            blocker = Some("feature_disabled");
        }
        if blocker.is_none() && kind == "pull" && item["mode"] != "ff_only" && changes > 0 {
            blocker = Some("dirty_worktree");
        }
        if blocker.is_none()
            && matches!(kind, "pull" | "sync")
            && (kind == "sync" || item["mode"] == "ff_only")
            && s["behind"].as_u64().unwrap_or(0) > 0
        {
            let root = Path::new(p["path"].as_str().unwrap());
            if self
                .options
                .git
                .local(
                    root,
                    &[
                        "merge-base",
                        "--is-ancestor",
                        "HEAD",
                        &status::tracking(&pull["target"]),
                    ],
                )
                .await
                .is_err()
            {
                blocker = Some("diverged");
            }
        }
        if kind == "sync" && changes > 0 {
            warnings.push(json!({"code":"uncommitted_changes","files":changes}));
        }
        if matches!(kind, "fetch" | "pull" | "sync") {
            steps.push(json!({"code":"fetch"}));
        }
        if matches!(kind, "pull" | "sync")
            && !pull["target"].is_null()
            && s["behind"].as_u64().unwrap_or(0) > 0
        {
            steps.push(pull_step(
                &pull["target"],
                if kind == "sync" {
                    "ff_only"
                } else {
                    item["mode"].as_str().unwrap()
                },
                s["behind"].as_u64().unwrap_or(0),
            ));
        }
        if matches!(kind, "push" | "sync")
            && !push["target"].is_null()
            && (s["ahead"].as_u64().unwrap_or(0) > 0 || push["target"]["exists"] == false || force)
        {
            let step = push_step(
                &push["target"],
                s["ahead"].clone(),
                force,
                item["setUpstream"] == true,
            );
            if push["target"]["exists"] == false {
                warnings
                    .push(json!({"code":"creates_remote_branch","to":remote_ref(&push["target"])}));
            }
            steps.push(step);
        }
        if kind == "push" && steps.is_empty() && blocker.is_none() {
            blocker = Some("nothing_to_do");
        }
        if kind == "commit" {
            steps.push(json!({"code":"commit","files":changes}));
        }
        if blocker == Some("diverged") {
            steps.retain(|s| s["code"] == "fetch");
        }
        if kind == "commit"
            && let Some(paths) = item["paths"].as_array()
        {
            let files =
                status::change_names(&self.options.git, Path::new(p["path"].as_str().unwrap()))
                    .await?;
            let selected = files
                .iter()
                .filter(|f| paths.iter().any(|p| p == &f["path"] || p == &f["oldPath"]))
                .count();
            steps = vec![json!({"code":"commit","files":selected})];
            if selected == 0 && blocker.is_none() {
                blocker = Some("nothing_to_do");
            }
        }
        let root = Path::new(p["path"].as_str().unwrap());
        let mut rewrites = 0;
        if kind == "push" && force && push["target"]["exists"] == true {
            rewrites = status::count(
                &self.options.git,
                root,
                &format!("HEAD..{}", status::tracking(&push["target"])),
            )
            .await
            .unwrap_or(0);
        }
        if kind == "pull"
            && s["behind"].as_u64().unwrap_or(0) > 0
            && item["mode"] == "rebase"
            && push["target"]["exists"] == true
            && pull["target"]["exists"] == true
        {
            let local = self
                .options
                .git
                .local(
                    root,
                    &[
                        "rev-list",
                        &format!("{}..HEAD", status::tracking(&pull["target"])),
                    ],
                )
                .await
                .unwrap_or_default();
            let published = self
                .options
                .git
                .local(root, &["rev-list", &status::tracking(&push["target"])])
                .await
                .unwrap_or_default();
            let set = published.lines().collect::<BTreeSet<_>>();
            rewrites = local.lines().filter(|s| set.contains(s)).count() as u64;
        }
        if rewrites > 0 || (kind == "push" && force) {
            warnings.push(json!({"code":"rewrites_published_history","commits":rewrites}));
        }
        if kind == "pull" && item["mode"] == "merge" && pull["target"]["exists"] == true {
            let conflicts = self
                .options
                .git
                .merge_conflicts(root, "HEAD", &status::tracking(&pull["target"]))
                .await;
            if conflicts["status"] == "conflicts" {
                warnings.push(json!({"code":"conflicts_likely","paths":conflicts["paths"]}));
            }
        }
        if kind == "sync"
            && s["fetchedAt"]
                .as_u64()
                .is_none_or(|t| crate::now().saturating_sub(t) >= 24 * 60 * 60 * 1000)
        {
            warnings.push(json!({"code":"stale_remote","fetchedAt":s["fetchedAt"]}));
        }
        let risk = if force || rewrites > 0 {
            "danger"
        } else if kind == "pull" && item["mode"] != "ff_only" {
            "caution"
        } else {
            "safe"
        };
        Ok(
            json!({"itemId":item["itemId"],"projectId":item["projectId"],"executable":blocker.is_none(),"blocker":blocker,"steps":steps,"risk":risk,"warnings":warnings,"fingerprint":self.fingerprint(p).await?}),
        )
    }
    pub async fn start(self: &Arc<Self>, request: Value) -> Result<Value> {
        self.validate_request(&request, true).await?;
        let id = uuid::Uuid::new_v4().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        let snapshot = json!({"operationId":id,"kind":request["kind"],"state":"running","startedAt":crate::now(),"finishedAt":null,"items":request["items"].as_array().unwrap().iter().map(|i|json!({"itemId":i["itemId"],"projectId":i["projectId"],"state":"queued","step":null,"progress":null,"completed":[],"reason":null,"error":null})).collect::<Vec<_>>()});
        {
            let mut s = self.state.lock().unwrap();
            s.operations.insert(id.clone(), snapshot.clone());
            s.cancellations.insert(id.clone(), cancel.clone());
        }
        self.options.events.emit(
            "revia:operation",
            json!({"operationId":id,"kind":"state","snapshot":snapshot}),
        );
        let core = self.clone();
        tokio::spawn(async move {
            core.execute(id, request, cancel).await;
        });
        Ok(snapshot)
    }
    pub fn operation(&self, id: &str) -> Option<Value> {
        self.state.lock().unwrap().operations.get(id).cloned()
    }
    pub(crate) fn item_event(&self, id: &str, index: usize, item: Value) {
        {
            let mut s = self.state.lock().unwrap();
            s.operations.get_mut(id).unwrap()["items"][index] = item.clone();
        }
        self.options.events.emit(
            "revia:operation",
            json!({"operationId":id,"kind":"item","item":item}),
        );
    }
    pub fn cancel(&self, id: &str) -> Result<()> {
        let mut events = vec![];
        let snapshot = {
            let mut s = self.state.lock().unwrap();
            let Some(snapshot) = s.operations.get(id) else {
                return Err(AppError::new("invalid_input"));
            };
            if snapshot["state"] == "finished" {
                return Ok(());
            }
            s.cancellations[id].store(true, Ordering::SeqCst);
            let v = s.operations.get_mut(id).unwrap();
            v["state"] = json!("cancelling");
            for i in v["items"].as_array_mut().unwrap() {
                if i["state"] == "queued" {
                    i["state"] = json!("cancelled");
                    events.push(i.clone());
                }
            }
            v.clone()
        };
        for item in events {
            self.options.events.emit(
                "revia:operation",
                json!({"operationId":id,"kind":"item","item":item}),
            );
        }
        self.options.events.emit(
            "revia:operation",
            json!({"operationId":id,"kind":"state","snapshot":snapshot}),
        );
        Ok(())
    }
    async fn execute(self: Arc<Self>, id: String, request: Value, cancel: Arc<AtomicBool>) {
        let concurrency = self.state.lock().unwrap().settings.settings["general"]["concurrency"]
            .as_u64()
            .unwrap_or(4) as usize;
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
        let mut handles = vec![];
        for (index, input) in request["items"]
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .enumerate()
        {
            let core = self.clone();
            let id = id.clone();
            let kind = request["kind"].as_str().unwrap().to_string();
            let cancel = cancel.clone();
            let semaphore = semaphore.clone();
            handles.push(tokio::spawn(async move {
                let _permit = semaphore.acquire_owned().await.unwrap();
                let project_id = input["projectId"].as_str().unwrap_or("");
                let destination_lock = if kind == "clone" {
                    crate::publish::clone_destination(&input)
                        .ok()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    project_id.to_string()
                };
                let lock = core.lock(&destination_lock);
                let _guard = lock.lock().await;
                let mut item = {
                    let mut state = core.state.lock().unwrap();
                    let current = &mut state.operations.get_mut(&id).unwrap()["items"][index];
                    current["state"] = json!(if cancel.load(Ordering::SeqCst) {
                        "cancelled"
                    } else {
                        "running"
                    });
                    current.clone()
                };
                let mut operation_branch = Value::Null;
                core.item_event(&id, index, item.clone());
                if item["state"] != "cancelled" {
                    let result = core
                        .execute_item(
                            &kind,
                            &input,
                            &mut item,
                            cancel.clone(),
                            &id,
                            index,
                            &mut operation_branch,
                        )
                        .await;
                    if let Err(e) = result {
                        item["state"] = json!(if e.code == "cancelled" {
                            "cancelled"
                        } else if e.code == "merge_conflict" {
                            "conflict"
                        } else {
                            "failed"
                        });
                        item["error"] = if e.code == "cancelled" {
                            Value::Null
                        } else {
                            json!(e)
                        };
                    }
                    if item["progress"].is_null() {
                        item["progress"] =
                            core.operation(&id).unwrap()["items"][index]["progress"].clone();
                    }
                    item["step"] = Value::Null;
                    core.item_event(&id, index, item.clone());
                }
                let _ = core.refresh(Some(vec![project_id.into()])).await;
                if let Ok(p) = core.project(project_id) {
                    core.options.events.emit("revia:project-updated", p);
                }
                let error = serde_json::from_value::<AppError>(item["error"].clone()).ok();
                let mut params = operation_params(&input, &item);
                params["branch"] = operation_branch;
                if let Ok(p) = core.project(project_id)
                    && kind == "fetch"
                    && p["status"]["remotes"]
                        .as_array()
                        .is_some_and(|r| r.len() == 1)
                {
                    params["remote"] = p["status"]["remotes"][0]["name"].clone();
                }
                let history_id = item["projectId"].as_str().unwrap_or(project_id);
                if let Err(e) = core
                    .history_record_params(
                        Some(&id),
                        history_id,
                        &kind,
                        item["state"].as_str().unwrap(),
                        error.as_ref(),
                        params,
                    )
                    .await
                {
                    item["state"] = json!("failed");
                    item["error"] = json!(e);
                    core.item_event(&id, index, item);
                }
            }));
        }
        for h in handles {
            let _ = h.await;
        }
        let snapshot = {
            let mut s = self.state.lock().unwrap();
            let v = s.operations.get_mut(&id).unwrap();
            v["state"] = json!("finished");
            v["finishedAt"] = json!(crate::now());
            let snapshot = v.clone();
            s.cancellations.remove(&id);
            let mut finished = s
                .operations
                .iter()
                .filter(|(_, v)| v["state"] == "finished")
                .map(|(k, v)| (v["startedAt"].as_u64().unwrap_or(0), k.clone()))
                .collect::<Vec<_>>();
            finished.sort();
            let excess = finished.len().saturating_sub(20);
            for (_, id) in finished.into_iter().take(excess) {
                s.operations.remove(&id);
            }
            snapshot
        };
        self.options.events.emit(
            "revia:operation",
            json!({"operationId":id,"kind":"state","snapshot":snapshot}),
        );
    }
    #[allow(clippy::too_many_arguments)]
    async fn execute_item(
        self: &Arc<Self>,
        kind: &str,
        input: &Value,
        item: &mut Value,
        cancel: Arc<AtomicBool>,
        op: &str,
        index: usize,
        operation_branch: &mut Value,
    ) -> Result<()> {
        if kind == "clone" || kind == "publish" {
            let result = if kind == "clone" {
                self.execute_clone(input, item, cancel, op, index).await
            } else {
                self.execute_publish(input, item, cancel, op, index).await
            };
            if let Some(id) = item["projectId"].as_str() {
                let _ = self.refresh(Some(vec![id.into()])).await;
                if let Ok(p) = self.project(id) {
                    *operation_branch = p["status"]["branch"].clone();
                }
            }
            return result.map_err(|mut e| {
                if kind == "publish"
                    && !item["completed"].as_array().unwrap().is_empty()
                    && e.code != "cancelled"
                {
                    let steps = item["completed"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|s| s["code"].as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    e = e.clone().detail(&format!(
                        "Completed steps: {steps}. Created repositories and remotes were kept.\n{}",
                        e.detail.as_deref().unwrap_or("")
                    ));
                }
                e
            });
        }
        let id = text(input, "projectId")?;
        self.refresh(Some(vec![id.into()])).await?;
        let mut p = self.project(id)?;
        *operation_branch = p["status"]["branch"].clone();
        if kind == "merge" {
            let preview = self
                .preview_merge(id, text(input, "source")?, text(input, "target")?)
                .await?;
            if input["fingerprint"] != preview["fingerprint"] {
                confirmation(item, "state_changed");
                return Ok(());
            }
            if let Some(b) = preview["blocker"].as_str() {
                confirmation(item, b);
                return Ok(());
            }
            if preview["upToDate"] == true {
                item["state"] = json!("skipped");
                item["reason"] = json!("nothing_to_do");
                return Ok(());
            }
            if input["mode"] == "ff_only" && preview["fastForward"] == false {
                confirmation(item, "diverged");
                return Ok(());
            }
            if cancel.load(Ordering::SeqCst) {
                item["state"] = json!("cancelled");
                return Ok(());
            }
            let root = Path::new(p["path"].as_str().unwrap());
            let g = &self.options.git;
            if preview["needsCheckout"] == true {
                self.step_event(op, index, item, "checkout");
                g.local_write(root, &["checkout", text(input, "target")?])
                    .await?;
                completed(item, json!({"code":"checkout","branch":input["target"]}));
            }
            self.step_event(op, index, item, "merge");
            let mode = match text(input, "mode")? {
                "no_ff" => "--no-ff",
                "ff_only" => "--ff-only",
                _ => "--ff",
            };
            let result = g
                .local_write(
                    root,
                    &[
                        "merge",
                        "--no-edit",
                        mode,
                        preview["source"]["sha"].as_str().unwrap(),
                    ],
                )
                .await;
            if result
                .as_ref()
                .is_err_and(|e| !matches!(e.code.as_str(), "repo_locked" | "timeout"))
                && !self.conflicts(id).await?["files"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            {
                return Err(AppError::new("merge_conflict"));
            }
            result?;
            completed(item, merge_step(input, &preview));
            item["state"] = json!("succeeded");
            return Ok(());
        }
        let mut lease_sha = None;
        if let Some(f) = input["fingerprint"].as_str() {
            let (current, sha) = self.fingerprint_state(&p).await?;
            lease_sha = sha;
            if f != current {
                confirmation(item, "state_changed");
                return Ok(());
            }
        }
        let plan = self.plan_item(kind, input, &p).await?;
        if let Some(b) = plan["blocker"].as_str().filter(|b| *b != "diverged") {
            if b == "nothing_to_do" {
                item["state"] = json!("skipped");
                item["reason"] = json!(b);
            } else {
                confirmation(item, b);
            }
            return Ok(());
        }
        let root = Path::new(p["path"].as_str().unwrap()).to_path_buf();
        let g = &self.options.git;
        let progress_core = self.clone();
        let progress_op = op.to_string();
        let progress: Arc<dyn Fn(f64) + Send + Sync> = Arc::new(move |value| {
            let mut current =
                progress_core.operation(&progress_op).unwrap()["items"][index].clone();
            current["progress"] = json!(value);
            progress_core.item_event(&progress_op, index, current);
        });
        if cancel.load(Ordering::SeqCst) {
            item["state"] = json!("cancelled");
            return Ok(());
        }
        if kind == "commit" {
            for k in ["user.name", "user.email"] {
                if g.local(&root, &["config", "--get", k])
                    .await
                    .ok()
                    .is_none_or(|v| v.trim().is_empty())
                {
                    return Err(AppError::new("identity_missing"));
                }
            }
            self.step_event(op, index, item, "commit");
            let paths = input["paths"].as_array();
            if let Some(paths) = paths {
                if paths.is_empty() {
                    item["state"] = json!("skipped");
                    item["reason"] = json!("nothing_to_do");
                    return Ok(());
                }
                let changes = status::change_names(g, &root).await?;
                if !paths.iter().any(|p| {
                    changes
                        .iter()
                        .any(|c| c["path"] == *p || c["oldPath"] == *p)
                }) {
                    item["state"] = json!("skipped");
                    item["reason"] = json!("nothing_to_do");
                    return Ok(());
                }
                let mut add = vec!["add".into(), "--intent-to-add".into(), "--".into()];
                for path in paths {
                    let path = path.as_str().unwrap();
                    if std::fs::symlink_metadata(root.join(path)).is_ok() {
                        add.push(path.into());
                    }
                }
                if add.len() > 3 {
                    g.write(&root, &add).await?;
                }
                let head = if p["status"]["unborn"] == true {
                    "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
                } else {
                    "HEAD"
                };
                let mut diff = vec![
                    "diff".into(),
                    "--no-ext-diff".into(),
                    "--no-textconv".into(),
                    "--quiet".into(),
                    head.into(),
                    "--".into(),
                ];
                diff.extend(paths.iter().map(|p| p.as_str().unwrap().to_string()));
                if g.run(&root, &diff, false, None).await.is_ok() {
                    item["state"] = json!("skipped");
                    item["reason"] = json!("nothing_to_do");
                    return Ok(());
                }
                let mut args = vec![
                    "commit".into(),
                    "--only".into(),
                    "-m".into(),
                    text(input, "message")?.into(),
                    "--".into(),
                ];
                args.extend(paths.iter().map(|p| p.as_str().unwrap().to_string()));
                g.write(&root, &args).await?;
            } else {
                g.local_write(&root, &["add", "-A", "--", "."]).await?;
                if g.local(
                    &root,
                    &[
                        "diff",
                        "--cached",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--quiet",
                    ],
                )
                .await
                .is_ok()
                {
                    item["state"] = json!("skipped");
                    item["reason"] = json!("nothing_to_do");
                    return Ok(());
                }
                g.local_write(&root, &["commit", "-m", text(input, "message")?])
                    .await?;
            }
            let changed = g
                .local(
                    &root,
                    &[
                        "diff-tree",
                        "--root",
                        "--no-commit-id",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--name-status",
                        "-r",
                        "-M",
                        "-z",
                        "HEAD",
                        "--",
                    ],
                )
                .await?;
            let mut records = changed.split('\0');
            let mut files = 0;
            while let Some(code) = records.next() {
                if code.is_empty() {
                    continue;
                }
                records.next();
                if code.starts_with(['R', 'C']) {
                    records.next();
                }
                files += 1;
            }
            completed(item, json!({"code":"commit","files":files}));
            item["state"] = json!("succeeded");
            return Ok(());
        }
        let local_before = if matches!(kind, "pull" | "sync") {
            Some(self.local_guard(&root).await?)
        } else {
            None
        };
        if matches!(kind, "fetch" | "pull" | "sync") {
            self.step_event(op, index, item, "fetch");
            let remotes = if kind == "sync" {
                ["pull", "push"]
                    .iter()
                    .filter_map(|k| {
                        p["status"]["targets"][k]["target"]["remote"]
                            .as_str()
                            .map(str::to_owned)
                    })
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            } else if kind == "pull" {
                vec![
                    p["status"]["targets"]["pull"]["target"]["remote"]
                        .as_str()
                        .unwrap()
                        .to_string(),
                ]
            } else {
                p["status"]["remotes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r["name"].as_str().unwrap().to_string())
                    .collect()
            };
            let mut first_error = None;
            for remote in remotes {
                g.reference(&root, &remote).await?;
                let result = g
                    .run_progress(
                        &root,
                        &["fetch".into(), "--progress".into(), remote],
                        true,
                        Some(cancel.clone()),
                        Some(progress.clone()),
                    )
                    .await;
                self.network_result(id, &result);
                if let Err(e) = result {
                    if e.code == "cancelled" {
                        return Err(e);
                    }
                    if first_error.is_none() {
                        first_error = Some(e);
                    }
                }
            }
            if let Some(e) = first_error {
                self.network_result::<()>(id, &Err(e.clone()));
                return Err(e);
            }
            self.state
                .lock()
                .unwrap()
                .fetched
                .insert(id.into(), crate::now());
            item["progress"] = self.operation(op).unwrap()["items"][index]["progress"].clone();
            completed(item, json!({"code":"fetch"}));
            self.item_event(op, index, item.clone());
            self.refresh(Some(vec![id.into()])).await?;
            p = self.project(id)?;
            if let Some(before) = &local_before
                && self.local_guard(&root).await.ok().as_ref() != Some(before)
            {
                confirmation(item, "state_changed");
                return Ok(());
            }
        }
        if cancel.load(Ordering::SeqCst) {
            item["state"] = json!("cancelled");
            return Ok(());
        }
        if matches!(kind, "pull" | "sync")
            && !(kind == "sync" && p["status"]["targets"]["pull"]["target"].is_null())
        {
            let target = &p["status"]["targets"]["pull"]["target"];
            if target.is_null() {
                confirmation(item, "target_invalid");
                return Ok(());
            }
            let behind = p["status"]["behind"].as_u64().unwrap_or(0);
            let reference = status::tracking(target);
            if behind > 0 {
                let ff = g
                    .local(&root, &["merge-base", "--is-ancestor", "HEAD", &reference])
                    .await
                    .is_ok();
                let mode = if kind == "sync" {
                    "ff_only"
                } else {
                    text(input, "mode")?
                };
                if !ff && mode == "ff_only" {
                    confirmation(item, "diverged");
                    return Ok(());
                }
                self.step_event(op, index, item, "pull");
                let args = match mode {
                    "ff_only" => vec!["merge", "--ff-only", &reference],
                    "merge" => vec!["merge", "--no-edit", &reference],
                    _ => vec!["rebase", &reference],
                };
                if local_before.as_ref() != self.local_guard(&root).await.ok().as_ref() {
                    confirmation(item, "state_changed");
                    return Ok(());
                }
                let result = g.local_write(&root, &args).await;
                match result {
                    Ok(_) => completed(item, pull_step(target, mode, behind)),
                    Err(e) => {
                        if matches!(e.code.as_str(), "repo_locked" | "timeout") {
                            return Err(e);
                        }
                        if status::change_names(g, &root)
                            .await?
                            .iter()
                            .any(|c| c["kind"] == "conflicted")
                        {
                            return Err(AppError::new("merge_conflict"));
                        }
                        if e.code == "dirty_worktree" {
                            confirmation(item, "dirty_worktree");
                            return Ok(());
                        }
                        return Err(e);
                    }
                }
                self.refresh(Some(vec![id.into()])).await?;
                p = self.project(id)?;
            } else if kind == "pull" {
                item["state"] = json!("skipped");
                item["reason"] = json!("nothing_to_do");
                return Ok(());
            }
        }
        if matches!(kind, "push" | "sync") {
            let t = &p["status"]["targets"]["push"]["target"];
            if t.is_null() {
                confirmation(item, "target_invalid");
                return Ok(());
            }
            let force = kind == "push" && input["force"] == true;
            if force
                && self.state.lock().unwrap().settings.settings["general"]["safety"]["allowForcePush"]
                    != true
            {
                confirmation(item, "feature_disabled");
                return Ok(());
            }
            if t["exists"] == false || p["status"]["ahead"].as_u64().unwrap_or(0) > 0 || force {
                if cancel.load(Ordering::SeqCst) {
                    item["state"] = json!("cancelled");
                    return Ok(());
                }
                let remote = text(t, "remote")?;
                let branch = text(t, "branch")?;
                let local = text(&p["status"], "branch")?;
                for r in [remote, branch, local] {
                    g.reference(&root, r).await?;
                }
                let mut args = vec!["push".into(), "--progress".into()];
                let set_upstream = kind == "push" && input["setUpstream"] == true;
                if set_upstream {
                    args.push("-u".into());
                }
                if force {
                    let sha = lease_sha.as_deref().unwrap_or("");
                    args.push(format!(
                        "--force-with-lease=refs/heads/{branch}:{}",
                        sha.trim()
                    ));
                }
                args.push(remote.into());
                args.push(format!("refs/heads/{local}:refs/heads/{branch}"));
                self.step_event(op, index, item, "push");
                let result = g
                    .run_progress(
                        &root,
                        &args,
                        true,
                        Some(cancel.clone()),
                        Some(progress.clone()),
                    )
                    .await;
                self.network_result(id, &result);
                if result.as_ref().is_err_and(|e| e.code == "push_rejected") {
                    let _ = g
                        .run(&root, &["fetch".into(), remote.into()], true, None)
                        .await;
                    let _ = self.refresh(Some(vec![id.into()])).await;
                }
                result?;
                item["progress"] = self.operation(op).unwrap()["items"][index]["progress"].clone();
                completed(
                    item,
                    push_step(
                        t,
                        if t["exists"] == false {
                            json!(status::count(g, &root, "HEAD").await)
                        } else {
                            p["status"]["ahead"].clone()
                        },
                        force,
                        set_upstream,
                    ),
                );
            } else if kind == "push" {
                item["state"] = json!("skipped");
                item["reason"] = json!("nothing_to_do");
                return Ok(());
            }
        }
        item["state"] = json!("succeeded");
        Ok(())
    }
    pub(crate) fn step_event(&self, op: &str, index: usize, item: &mut Value, step: &str) {
        item["step"] = json!(step);
        item["progress"] = Value::Null;
        self.item_event(op, index, item.clone());
    }
    pub(crate) fn network_result<T>(&self, id: &str, result: &Result<T>) {
        let mut s = self.state.lock().unwrap();
        match result {
            Ok(_) => {
                s.remote_errors.remove(id);
            }
            Err(e) => {
                let kind = match e.code.as_str() {
                    "auth_failed" | "permission_denied" => Some("auth"),
                    "remote_not_found" => Some("not_found"),
                    "network_unreachable" | "timeout" => Some("unreachable"),
                    _ => None,
                };
                if let Some(k) = kind {
                    s.remote_errors.insert(id.into(), k.into());
                }
            }
        }
    }
}
fn confirmation(item: &mut Value, reason: &str) {
    item["state"] = json!("needs_confirmation");
    item["reason"] = json!(reason);
}
fn completed(item: &mut Value, step: Value) {
    item["completed"].as_array_mut().unwrap().push(step);
}
fn remote_ref(t: &Value) -> Value {
    json!({"remote":t["remote"],"branch":t["branch"]})
}
fn pull_step(t: &Value, mode: &str, commits: u64) -> Value {
    json!({"code":"pull","mode":mode,"from":remote_ref(t),"commits":commits})
}
fn push_step(t: &Value, commits: Value, force: bool, set_upstream: bool) -> Value {
    json!({"code":"push","to":remote_ref(t),"commits":commits,"createsRemoteBranch":t["exists"]==false,"force":force,"setUpstream":set_upstream})
}

fn operation_params(input: &Value, item: &Value) -> Value {
    let mut params = json!({});
    if let Some(url) = input["url"].as_str() {
        params["repo"] = json!(crate::error::sanitize(url));
    }
    if !input["name"].is_null() {
        params["repo"] = json!(format!(
            "{}/{}",
            input["owner"].as_str().unwrap_or(""),
            input["name"].as_str().unwrap_or("")
        ));
    }
    for key in ["source", "target", "mode", "remote", "sha", "name"] {
        if !input[key].is_null() {
            params[key] = input[key].clone();
        }
    }
    for step in item["completed"].as_array().into_iter().flatten() {
        match step["code"].as_str() {
            Some("pull") => {
                params["pulled"] = step["commits"].clone();
                params["target"] = json!(format!(
                    "{}/{}",
                    step["from"]["remote"].as_str().unwrap_or(""),
                    step["from"]["branch"].as_str().unwrap_or("")
                ));
            }
            Some("push") => {
                params["pushed"] = step["commits"].clone();
                params["target"] = json!(format!(
                    "{}/{}",
                    step["to"]["remote"].as_str().unwrap_or(""),
                    step["to"]["branch"].as_str().unwrap_or("")
                ));
            }
            Some("create_repo") => {
                params["repo"] = step["nameWithOwner"].clone();
            }
            Some("clone") => {
                params["repo"] = step["url"].clone();
            }
            Some("commit" | "initial_commit") => {
                params["files"] = step["files"].clone();
                params["commits"] = json!(1);
            }
            _ => {}
        }
    }
    params
}

fn merge_step(item: &Value, preview: &Value) -> Value {
    json!({"code":"merge","source":item["source"],"target":item["target"],"mode":item["mode"],"commits":preview["commitCount"],"fastForward":preview["fastForward"]==true&&item["mode"]!="no_ff"})
}
