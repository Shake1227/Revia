use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    path::Path,
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub schema_version: u32,
    pub settings: Value,
    pub project_settings: BTreeMap<String, Value>,
    pub manual_paths: Vec<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            settings: json!({"roots":[],"excludedPaths":[],"general":{"autoRefresh":{"onFocus":true,"intervalMinutes":10,"fetch":false},"concurrency":4,"editor":null,"safety":{"allowForcePush":false,"allowHardReset":false}},"ui":{}}),
            project_settings: BTreeMap::new(),
            manual_paths: vec![],
        }
    }
}
pub fn project_defaults() -> Value {
    json!({"favorite":false,"includeInBatch":true,"defaultRemote":null,"defaultBranch":null,"pullMode":"ff_only","branchTargets":{}})
}
pub fn merge(target: &mut Value, patch: &Value) {
    if patch.is_object() && !target.is_object() {
        *target = json!({});
    }
    if let (Some(t), Some(p)) = (target.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            if v.is_object() {
                merge(t.entry(k).or_insert_with(|| json!({})), v);
            } else {
                t.insert(k.clone(), v.clone());
            }
        }
    }
}
fn parse(bytes: &[u8]) -> Result<Settings> {
    let raw: Value = serde_json::from_slice(bytes)?;
    if !raw.is_object() {
        return Err(AppError::new("settings_corrupt"));
    }
    let mut v: Settings = serde_json::from_value(raw)?;
    if !v.settings.is_object() || v.project_settings.values().any(|p| !p.is_object()) {
        return Err(AppError::new("settings_corrupt"));
    }
    let mut defaults = Settings::default().settings;
    merge(&mut defaults, &v.settings);
    v.settings = defaults;
    if let Some(roots) = v.settings["roots"].as_array_mut() {
        for root in roots {
            if !root.is_object() {
                return Err(AppError::new("settings_corrupt"));
            }
            for (k, val) in [
                ("maxDepth", json!(4)),
                ("enabled", json!(true)),
                ("exists", json!(false)),
            ] {
                if root.get(k).is_none() {
                    root[k] = val;
                }
            }
        }
    }
    validate(&v.settings)?;
    for p in v.project_settings.values_mut() {
        let mut d = project_defaults();
        merge(&mut d, p);
        if let Some(targets) = d["branchTargets"].as_object_mut() {
            for target in targets.values_mut() {
                if !target.is_object() {
                    return Err(AppError::new("settings_corrupt"));
                }
                for key in ["push", "pull"] {
                    if target.get(key).is_none() {
                        target[key] = Value::Null;
                    }
                }
            }
        }
        validate_project(&d)?;
        *p = d;
    }
    for path in v
        .manual_paths
        .iter()
        .chain(v.project_settings.keys())
        .chain(
            v.settings["excludedPaths"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .iter(),
        )
    {
        if !Path::new(path).is_absolute()
            || Path::new(path).components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
        {
            return Err(AppError::new("settings_corrupt"));
        }
    }
    fn normalized(s: &str) -> String {
        let p = Path::new(s);
        crate::platform::normalize(p).to_string_lossy().into_owned()
    }
    v.manual_paths = v.manual_paths.iter().map(|p| normalized(p)).collect();
    v.project_settings = v
        .project_settings
        .into_iter()
        .map(|(p, v)| (normalized(&p), v))
        .collect();
    for p in v.settings["excludedPaths"].as_array_mut().unwrap() {
        *p = json!(normalized(p.as_str().unwrap()));
    }
    for root in v.settings["roots"].as_array_mut().unwrap() {
        root["path"] = json!(normalized(root["path"].as_str().unwrap()));
    }
    Ok(v)
}
pub fn validate_project(v: &Value) -> Result<()> {
    for key in ["favorite", "includeInBatch"] {
        if !v[key].is_boolean() {
            return Err(AppError::new("invalid_input"));
        }
    }
    if !matches!(v["pullMode"].as_str(), Some("ff_only" | "merge" | "rebase"))
        || !v["branchTargets"].is_object()
    {
        return Err(AppError::new("invalid_input"));
    }
    for key in ["defaultRemote", "defaultBranch"] {
        if !v[key].is_null() && !v[key].is_string() {
            return Err(AppError::new("invalid_input"));
        }
    }
    for targets in v["branchTargets"].as_object().unwrap().values() {
        for key in ["push", "pull"] {
            if !targets[key].is_null()
                && (!targets[key]["remote"].is_string() || !targets[key]["branch"].is_string())
            {
                return Err(AppError::new("invalid_input"));
            }
        }
    }
    let _: crate::dto::ProjectSettings = serde_json::from_value(v.clone())?;
    Ok(())
}
pub fn validate(v: &Value) -> Result<()> {
    if !v["roots"].is_array()
        || !v["excludedPaths"]
            .as_array()
            .is_some_and(|a| a.iter().all(Value::is_string))
        || !v["ui"].is_object()
    {
        return Err(AppError::new("invalid_input"));
    }
    let _: crate::dto::AppSettings = serde_json::from_value(v.clone())?;
    let g = &v["general"];
    if !g["concurrency"]
        .as_u64()
        .is_some_and(|n| (1..=8).contains(&n))
        || !g["autoRefresh"]["intervalMinutes"].is_u64()
        || !(g["editor"].is_null() || g["editor"].is_string())
    {
        return Err(AppError::new("invalid_input"));
    }
    for b in [
        &g["autoRefresh"]["onFocus"],
        &g["autoRefresh"]["fetch"],
        &g["safety"]["allowForcePush"],
        &g["safety"]["allowHardReset"],
    ] {
        if !b.is_boolean() {
            return Err(AppError::new("invalid_input"));
        }
    }
    for r in v["roots"].as_array().unwrap() {
        if !r["path"]
            .as_str()
            .is_some_and(|p| Path::new(p).is_absolute())
            || !r["id"].is_string()
            || !r["path"].is_string()
            || !r["enabled"].is_boolean()
            || !r["maxDepth"].as_u64().is_some_and(|n| (1..=8).contains(&n))
        {
            return Err(AppError::new("invalid_input"));
        }
    }
    Ok(())
}
pub fn load(dir: &Path) -> Result<(Settings, Option<Value>)> {
    private_dir(dir)?;
    let path = dir.join("settings.json");
    if !path.exists() {
        return Ok((Settings::default(), None));
    }
    if let Ok(s) = fs::read(&path)
        .map_err(AppError::from)
        .and_then(|b| parse(&b))
    {
        return Ok((s, None));
    }
    let corrupt = dir.join(format!(
        "settings.corrupt-{}-{}.json",
        crate::now(),
        uuid::Uuid::new_v4()
    ));
    private_permissions(&path, 0o600)?;
    fs::rename(&path, &corrupt)?;
    let backup = fs::read(dir.join("settings.json.bak"))
        .map_err(AppError::from)
        .and_then(|b| parse(&b));
    let (s, kind) = match backup {
        Ok(s) => (s, "restored_backup"),
        Err(_) => (Settings::default(), "reset"),
    };
    save(dir, &s)?;
    Ok((s, Some(json!({"kind":kind,"corruptCopyPath":corrupt}))))
}
pub fn save(dir: &Path, settings: &Settings) -> Result<()> {
    private_dir(dir)?;
    let target = dir.join("settings.json");
    if let Ok(old) = fs::read(&target)
        && parse(&old).is_ok()
    {
        atomic(dir, "settings.json.bak", &old)?;
    }
    atomic(dir, "settings.json", &serde_json::to_vec_pretty(settings)?)
}
pub fn atomic(dir: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    private_dir(dir)?;
    let temp = dir.join(format!(".{}-{}.tmp", name, uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&temp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&temp, dir.join(name))?;
        #[cfg(unix)]
        File::open(dir)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub(crate) fn private_permissions(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}
pub(crate) fn private_dir(dir: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)?;
    Ok(())
}
