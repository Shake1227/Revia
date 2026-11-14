use crate::{
    error::{AppError, Result},
    git::Git,
    registry,
    settings::{self, Settings},
    status,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
};
use tokio::sync::Mutex as AsyncMutex;
pub struct Options {
    pub oauth: Option<crate::oauth::Config>,
    pub trash: Arc<dyn crate::trash::Trash>,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub git: Git,
    pub events: Arc<dyn crate::EventSink>,
}
pub(crate) struct State {
    pub settings: Settings,
    pub projects: Vec<Value>,
    pub recovery: Option<Value>,
    pub operations: BTreeMap<String, Value>,
    pub cancellations: BTreeMap<String, Arc<AtomicBool>>,
    pub remote_errors: BTreeMap<String, String>,
    pub fetched: BTreeMap<String, u64>,
}
pub struct Core {
    pub options: Options,
    pub(crate) github: Mutex<crate::github::GithubState>,
    pub(crate) updates: Mutex<crate::update::UpdateState>,
    pub(crate) state: Mutex<State>,
    pub(crate) settings_lock: AsyncMutex<()>,
    pub(crate) scan_lock: AsyncMutex<()>,
    locks: Mutex<BTreeMap<String, Arc<AsyncMutex<()>>>>,
}
impl Core {
    pub fn new(options: Options) -> Result<Arc<Self>> {
        let (settings, recovery) = settings::load(&options.config_dir)?;
        let mut projects: Vec<Value> = std::fs::read(options.cache_dir.join("projects.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Vec<Value>>(&b).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|p| {
                if serde_json::from_value::<crate::dto::Project>(p.clone()).is_err() {
                    return false;
                }
                p["path"]
                    .as_str()
                    .is_some_and(|s| Path::new(s).is_absolute())
                    && p["id"]
                        .as_str()
                        .is_some_and(|id| registry::path_hash(p["path"].as_str().unwrap()) == id)
            })
            .collect();
        for p in &mut projects {
            let path = Path::new(p["path"].as_str().unwrap());
            let path = crate::platform::normalize(path);
            p["path"] = json!(path.to_string_lossy());
            p["id"] = json!(registry::id(p["path"].as_str().unwrap()));
        }
        projects.retain(|p| {
            !settings.settings["excludedPaths"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .any(|e| Path::new(p["path"].as_str().unwrap()).starts_with(e))
        });
        for p in &mut projects {
            let path = p["path"].as_str().unwrap().to_string();
            p["settings"] = settings
                .project_settings
                .get(&path)
                .cloned()
                .unwrap_or_else(settings::project_defaults);
        }
        if projects.is_empty() {
            projects = registry::scan(&settings, &[], &crate::NoEvents);
        }
        for path in settings
            .manual_paths
            .iter()
            .chain(settings.project_settings.keys())
        {
            if !projects.iter().any(|p| p["path"] == *path)
                && !settings.settings["excludedPaths"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|e| Path::new(path).starts_with(e))
            {
                projects.push(registry::project(
                    Path::new(path),
                    json!({"kind":"manual"}),
                    &settings,
                ));
            }
        }
        Ok(Arc::new(Self {
            options,
            github: Mutex::new(crate::github::GithubState::default()),
            updates: Mutex::new(crate::update::UpdateState::default()),
            state: Mutex::new(State {
                settings,
                projects,
                recovery,
                operations: BTreeMap::new(),
                cancellations: BTreeMap::new(),
                remote_errors: BTreeMap::new(),
                fetched: BTreeMap::new(),
            }),
            settings_lock: AsyncMutex::new(()),
            scan_lock: AsyncMutex::new(()),
            locks: Mutex::new(BTreeMap::new()),
        }))
    }
    pub(crate) fn lock(&self, id: &str) -> Arc<AsyncMutex<()>> {
        self.locks
            .lock()
            .unwrap()
            .entry(id.into())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }
    pub fn project(&self, id: &str) -> Result<Value> {
        let p = self
            .state
            .lock()
            .unwrap()
            .projects
            .iter()
            .find(|p| p["id"] == id)
            .cloned()
            .ok_or_else(|| AppError::new("project_not_found"))?;
        let path = Path::new(
            p["path"]
                .as_str()
                .ok_or_else(|| AppError::new("invalid_path"))?,
        );
        if crate::platform::canonicalize(path).is_ok_and(|canonical| canonical != path) {
            return Err(AppError::new("invalid_path"));
        }
        if std::fs::symlink_metadata(path.join(".git")).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(AppError::new("invalid_path"));
        }
        Ok(p)
    }
    pub fn workspace(&self) -> Value {
        let s = self.state.lock().unwrap();
        json!({"settings":s.settings.settings,"projects":s.projects})
    }
    pub async fn environment(&self) -> Value {
        let g = self
            .options
            .git
            .local(&self.options.config_dir, &["--version"])
            .await
            .ok();
        let gh = if self.options.git.gh.is_absolute() && self.options.git.gh.is_file() {
            tokio::time::timeout(
                std::time::Duration::from_secs(60),
                crate::platform::command(&self.options.git.gh)
                    .arg("--version")
                    .envs(&self.options.git.env)
                    .env("GH_PROMPT_DISABLED", "1")
                    .env("NO_COLOR", "1")
                    .env("GH_NO_UPDATE_NOTIFIER", "1")
                    .env("CLICOLOR", "0")
                    .stdin(std::process::Stdio::null())
                    .kill_on_drop(true)
                    .output(),
            )
            .await
            .ok()
            .and_then(|r| r.ok())
            .filter(|o| o.status.success())
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
        } else {
            None
        };
        json!({"platform":if cfg!(target_os="macos") {"macos"} else if cfg!(target_os="windows") {"windows"} else {"linux"},"appVersion":env!("CARGO_PKG_VERSION"),"git":{"installed":g.is_some(),"version":g.map(|s|s.trim().to_string())},"gh":{"installed":gh.is_some(),"version":gh}})
    }
    pub async fn app_state(&self) -> Value {
        let mut w = self.workspace();
        w["environment"] = self.environment().await;
        w["recovery"] = self
            .state
            .lock()
            .unwrap()
            .recovery
            .take()
            .unwrap_or(Value::Null);
        w
    }
    fn persist(&self, s: &Settings) -> Result<()> {
        settings::save(&self.options.config_dir, s)
    }
    pub(crate) fn cache(&self) {
        let s = self.state.lock().unwrap();
        if let Ok(b) = serde_json::to_vec(&s.projects) {
            let path = self.options.cache_dir.join("projects.json");
            if std::fs::read(&path).ok().as_deref() != Some(b.as_slice()) {
                let _ = settings::atomic(&self.options.cache_dir, "projects.json", &b);
            }
        }
    }
    pub async fn scan(&self) -> Result<Value> {
        let _scan_guard = self.scan_lock.lock().await;
        let (mut s, old) = {
            let s = self.state.lock().unwrap();
            (s.settings.clone(), s.projects.clone())
        };
        for root in s.settings["roots"].as_array_mut().unwrap() {
            root["exists"] = json!(Path::new(root["path"].as_str().unwrap()).is_dir());
        }
        {
            self.state.lock().unwrap().settings.settings["roots"] = s.settings["roots"].clone();
        }
        let scan_settings = s.clone();
        let scan_old = old.clone();
        let events = self.options.events.clone();
        let mut projects = tokio::task::spawn_blocking(move || {
            registry::scan(&scan_settings, &scan_old, events.as_ref())
        })
        .await
        .map_err(|_| AppError::new("io_error"))?;
        for p in &mut projects {
            if let Some(previous) = old.iter().find(|o| o["id"] == p["id"]) {
                p["status"] = previous["status"].clone();
            }
        }
        self.state.lock().unwrap().projects = projects;
        self.refresh(None).await?;
        let w = self.workspace();
        if old.iter().map(|p| p["id"].clone()).collect::<Vec<_>>()
            != w["projects"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p["id"].clone())
                .collect::<Vec<_>>()
        {
            self.options
                .events
                .emit("revia:projects-changed", w["projects"].clone());
        }
        self.cache();
        Ok(w)
    }
    pub async fn refresh(&self, ids: Option<Vec<String>>) -> Result<Vec<Value>> {
        let projects = self.state.lock().unwrap().projects.clone();
        let mut out = vec![];
        if let Some(ids) = &ids {
            for id in ids {
                if !projects.iter().any(|p| p["id"] == *id) {
                    return Err(AppError::new("project_not_found"));
                }
            }
        }
        let semaphore = Arc::new(tokio::sync::Semaphore::new(8));
        let mut jobs = tokio::task::JoinSet::new();
        for (index, mut p) in projects.into_iter().enumerate() {
            let id = p["id"].as_str().unwrap().to_string();
            if ids.as_ref().is_some_and(|ids| !ids.contains(&id)) {
                continue;
            }
            let (re, fetched) = {
                let s = self.state.lock().unwrap();
                (
                    s.remote_errors.get(&id).cloned(),
                    s.fetched.get(&id).copied(),
                )
            };
            let git = self.options.git.clone();
            let semaphore = semaphore.clone();
            jobs.spawn(async move {
                let _permit = semaphore.acquire_owned().await.unwrap();
                let old = p.clone();
                match status::read(&git, &p, re.as_deref(), fetched).await {
                    Ok(st) => p["status"] = st,
                    Err(e) => {
                        let mut st = status::empty();
                        st["error"] = json!(e);
                        p["status"] = st;
                    }
                }
                (index, id, old, p)
            });
        }
        let mut results = vec![];
        while let Some(result) = jobs.join_next().await {
            results.push(result.map_err(|_| AppError::new("io_error"))?);
        }
        results.sort_by_key(|r| r.0);
        for (_, id, old, mut p) in results {
            {
                let mut s = self.state.lock().unwrap();
                if let Some(entry) = s.projects.iter_mut().find(|v| v["id"] == id) {
                    entry["status"] = p["status"].clone();
                    p = entry.clone();
                }
            }
            let mut a = old["status"].clone();
            let mut b = p["status"].clone();
            if a.is_object() {
                a["checkedAt"] = Value::Null;
            }
            if b.is_object() {
                b["checkedAt"] = Value::Null;
            }
            if a != b {
                self.options.events.emit("revia:project-updated", p.clone());
            } else {
                p["status"]["checkedAt"] = old["status"]["checkedAt"].clone();
                if let Some(entry) = self
                    .state
                    .lock()
                    .unwrap()
                    .projects
                    .iter_mut()
                    .find(|v| v["id"] == id)
                {
                    entry["status"] = p["status"].clone();
                }
            }
            out.push(p);
        }
        self.cache();
        Ok(out)
    }
    pub async fn update_ui(&self, ui: Value) -> Result<()> {
        if !ui.is_object() {
            return Err(AppError::new("invalid_input"));
        }
        let _guard = self.settings_lock.lock().await;
        let mut s = self.state.lock().unwrap();
        let mut next = s.settings.clone();
        next.settings["ui"] = ui;
        self.persist(&next)?;
        s.settings = next;
        Ok(())
    }
    pub async fn update_general(&self, patch: Value) -> Result<Value> {
        if !patch.is_object() {
            return Err(AppError::new("invalid_input"));
        }
        let _guard = self.settings_lock.lock().await;
        let mut s = self.state.lock().unwrap();
        let mut next = s.settings.clone();
        settings::merge(&mut next.settings["general"], &patch);
        settings::validate(&next.settings)?;
        self.persist(&next)?;
        s.settings = next;
        Ok(s.settings.settings.clone())
    }
    pub async fn registry_change(&self, kind: &str, arg: Value, patch: Value) -> Result<Value> {
        {
            let _guard = self.settings_lock.lock().await;
            let mut s = self.state.lock().unwrap();
            let mut next = s.settings.clone();
            match kind {
                "add_root" => {
                    let p = registry::directory(
                        arg.as_str().ok_or_else(|| AppError::new("invalid_path"))?,
                    )?;
                    let p = p.to_string_lossy().into_owned();
                    let depth = if patch.is_null() {
                        4
                    } else {
                        patch
                            .as_u64()
                            .ok_or_else(|| AppError::new("invalid_input"))?
                    };
                    if !(1..=8).contains(&depth) {
                        return Err(AppError::new("invalid_input"));
                    }
                    let roots = next.settings["roots"].as_array_mut().unwrap();
                    if roots.iter().any(|r| r["path"] == p) {
                        return Err(AppError::new("already_registered"));
                    }
                    roots.push(json!({"id":registry::id(&p),"path":p,"maxDepth":depth,"enabled":true,"exists":true}));
                }
                "update_root" => {
                    let root = next.settings["roots"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|r| r["id"] == arg)
                        .ok_or_else(|| AppError::new("invalid_input"))?;
                    if !patch.is_object() {
                        return Err(AppError::new("invalid_input"));
                    }
                    for key in ["enabled", "maxDepth"] {
                        if let Some(v) = patch.get(key) {
                            root[key] = v.clone();
                        }
                    }
                }
                "remove_root" => {
                    next.settings["roots"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|r| r["id"] != arg);
                }
                "add_project" => {
                    let p = registry::directory(
                        arg.as_str().ok_or_else(|| AppError::new("invalid_path"))?,
                    )?
                    .to_string_lossy()
                    .into_owned();
                    if s.projects.iter().any(|v| v["path"] == p) {
                        return Err(AppError::new("already_registered"));
                    }
                    next.manual_paths.push(p);
                }
                "exclude" => {
                    let p = s
                        .projects
                        .iter()
                        .find(|p| p["id"] == arg)
                        .ok_or_else(|| AppError::new("project_not_found"))?;
                    next.settings["excludedPaths"]
                        .as_array_mut()
                        .unwrap()
                        .push(p["path"].clone());
                }
                "restore" => {
                    let p = registry::directory(
                        arg.as_str().ok_or_else(|| AppError::new("invalid_path"))?,
                    )?
                    .to_string_lossy()
                    .into_owned();
                    next.settings["excludedPaths"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|v| v != &p);
                }
                _ => return Err(AppError::new("invalid_input")),
            }
            settings::validate(&next.settings)?;
            self.persist(&next)?;
            s.settings = next;
        }
        self.scan().await
    }
    pub async fn update_project(&self, id: &str, patch: Value) -> Result<Value> {
        let lock = self.lock(id);
        let _guard = lock.lock().await;
        if !patch.is_object() {
            return Err(AppError::new("invalid_input"));
        }
        let mut filtered = json!({});
        for key in [
            "favorite",
            "includeInBatch",
            "defaultRemote",
            "defaultBranch",
            "pullMode",
        ] {
            if let Some(v) = patch.get(key) {
                filtered[key] = v.clone();
            }
        }
        let project = self.project(id)?;
        for key in ["defaultRemote", "defaultBranch"] {
            if let Some(v) = filtered[key].as_str() {
                self.options
                    .git
                    .reference(Path::new(project["path"].as_str().unwrap()), v)
                    .await?;
            }
        }
        self.update_project_all(id, filtered).await
    }
    pub(crate) async fn update_project_all(&self, id: &str, patch: Value) -> Result<Value> {
        let p = self.project(id)?;
        let _guard = self.settings_lock.lock().await;
        let mut s = self.state.lock().unwrap();
        let mut next = s.settings.clone();
        let path = p["path"].as_str().unwrap();
        let v = next
            .project_settings
            .entry(path.into())
            .or_insert_with(settings::project_defaults);
        settings::merge(v, &patch);
        settings::validate_project(v)?;
        let ps = v.clone();
        self.persist(&next)?;
        s.settings = next;
        let p = s.projects.iter_mut().find(|p| p["id"] == id).unwrap();
        p["settings"] = ps;
        let p = p.clone();
        drop(s);
        self.options.events.emit("revia:project-updated", p.clone());
        Ok(p)
    }
    pub async fn set_target(
        &self,
        id: &str,
        branch: &str,
        kind: &str,
        target: Value,
    ) -> Result<Value> {
        let lock = self.lock(id);
        let _guard = lock.lock().await;
        let p = self.project(id)?;
        let path = Path::new(p["path"].as_str().unwrap());
        self.options.git.reference(path, branch).await?;
        if !matches!(kind, "push" | "pull") {
            return Err(AppError::new("invalid_input"));
        }
        if !target.is_null() {
            for key in ["remote", "branch"] {
                self.options
                    .git
                    .reference(
                        path,
                        target[key]
                            .as_str()
                            .ok_or_else(|| AppError::new("invalid_input"))?,
                    )
                    .await?;
            }
        }
        let mut targets = p["settings"]["branchTargets"].clone();
        if targets.get(branch).is_none() {
            targets[branch] = json!({"push":null,"pull":null});
        }
        targets[branch][kind] = target;
        self.update_project_all(id, json!({"branchTargets":targets}))
            .await?;
        self.refresh(Some(vec![id.into()])).await?;
        self.project(id)
    }
    pub async fn init(&self, id: &str, branch: Option<String>) -> Result<Value> {
        let lock = self.lock(id);
        let _guard = lock.lock().await;
        let p = self.project(id)?;
        let path = Path::new(p["path"].as_str().unwrap());
        let branch = branch.unwrap_or("main".into());
        self.options.git.reference(path, &branch).await?;
        let result = if !path.join(".git").exists() {
            self.options
                .git
                .local(path, &["init", "-b", &branch])
                .await
                .map(|_| ())
        } else {
            Ok(())
        };
        self.refresh(Some(vec![id.into()])).await?;
        self.history_record(
            None,
            id,
            "init",
            if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
            result.as_ref().err(),
        )
        .await?;
        result?;
        self.project(id)
    }
}
