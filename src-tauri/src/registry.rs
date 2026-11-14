use crate::{
    error::{AppError, Result},
    settings,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
pub fn id(path: &str) -> String {
    let path = crate::platform::normalize(Path::new(path));
    path_hash(&path.to_string_lossy())
}
pub(crate) fn path_hash(path: &str) -> String {
    format!("{:x}", Sha256::digest(path.as_bytes()))[..16].into()
}
pub fn directory(path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    if !p.is_absolute() || !p.is_dir() {
        return Err(AppError::new("invalid_path"));
    }
    Ok(crate::platform::resolve(p))
}
pub fn markers(path: &Path) -> Vec<String> {
    if path.join(".git").exists() {
        return vec![".git".into()];
    }
    let exact = [
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "requirements.txt",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "Package.swift",
        "Gemfile",
        "composer.json",
        "pubspec.yaml",
        "CMakeLists.txt",
        "Makefile",
        "deno.json",
        "mix.exs",
    ];
    let mut out = fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if exact.contains(&name.as_str())
                || [".xcodeproj", ".xcworkspace", ".sln", ".csproj"]
                    .iter()
                    .any(|suffix| name.ends_with(suffix))
            {
                Some(name)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    out.sort();
    out
}
pub fn project(path: &Path, origin: Value, s: &settings::Settings) -> Value {
    let path = crate::platform::normalize(path)
        .to_string_lossy()
        .into_owned();
    json!({"id":id(&path),"name":Path::new(&path).file_name().unwrap_or_default().to_string_lossy(),"path":path,"origin":origin,"markers":markers(Path::new(&path)),"settings":s.project_settings.get(&path).cloned().unwrap_or_else(settings::project_defaults),"status":null})
}
pub fn scan(
    s: &settings::Settings,
    previous: &[Value],
    events: &dyn crate::EventSink,
) -> Vec<Value> {
    let mut found = BTreeMap::new();
    for root in s.settings["roots"].as_array().unwrap() {
        if !root["enabled"].as_bool().unwrap_or(false) {
            continue;
        }
        let mut directories = 0;
        let start = found.len();
        let mut last_progress = std::time::Instant::now();
        walk(
            Path::new(root["path"].as_str().unwrap()),
            0,
            root["maxDepth"].as_u64().unwrap_or(4),
            root,
            s,
            &mut found,
            &mut directories,
            events,
            &mut last_progress,
            start,
        );
        events.emit("revia:scan-progress",json!({"rootId":root["id"],"directories":directories,"found":found.len()-start,"done":true}));
    }
    for p in &s.manual_paths {
        if !excluded(Path::new(p), s) {
            found.insert(
                p.clone(),
                project(Path::new(p), json!({"kind":"manual"}), s),
            );
        }
    }
    for p in s.project_settings.keys() {
        if !found.contains_key(p) && !excluded(Path::new(p), s) {
            let origin = previous
                .iter()
                .find(|v| v["path"] == *p)
                .map(|v| v["origin"].clone())
                .unwrap_or(json!({"kind":"manual"}));
            found.insert(p.clone(), project(Path::new(p), origin, s));
        }
    }
    found.into_values().collect()
}
fn excluded(path: &Path, s: &settings::Settings) -> bool {
    s.settings["excludedPaths"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .any(|p| path.starts_with(p))
}
#[allow(clippy::too_many_arguments)]
fn walk(
    path: &Path,
    depth: u64,
    max: u64,
    root: &Value,
    s: &settings::Settings,
    out: &mut BTreeMap<String, Value>,
    directories: &mut usize,
    events: &dyn crate::EventSink,
    last_progress: &mut std::time::Instant,
    found_start: usize,
) {
    if fs::symlink_metadata(path).map_or(true, |m| m.file_type().is_symlink() || !m.is_dir())
        || (depth > 0 && hidden(path))
        || excluded(path, s)
    {
        return;
    }
    *directories += 1;
    if last_progress.elapsed() >= std::time::Duration::from_millis(200) {
        events.emit("revia:scan-progress",json!({"rootId":root["id"],"directories":directories,"found":out.len()-found_start,"done":false}));
        *last_progress = std::time::Instant::now();
    }
    let marks = markers(path);
    if !marks.is_empty() {
        let p = project(path, json!({"kind":"scan","rootId":root["id"]}), s);
        out.insert(path.to_string_lossy().into_owned(), p);
        return;
    }
    if depth >= max {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if os_excluded(&name)
            || name.starts_with('.')
            || [
                "node_modules",
                "target",
                "dist",
                "build",
                "out",
                "vendor",
                "Pods",
                "venv",
                "__pycache__",
                "Library",
                "Applications",
            ]
            .contains(&name.as_str())
        {
            continue;
        }
        walk(
            &entry.path(),
            depth + 1,
            max,
            root,
            s,
            out,
            directories,
            events,
            last_progress,
            found_start,
        );
    }
}

fn os_excluded(name: &str) -> bool {
    #[cfg(windows)]
    {
        [
            "AppData",
            "$RECYCLE.BIN",
            "System Volume Information",
            "Windows",
            "Program Files",
            "Program Files (x86)",
            "ProgramData",
        ]
        .iter()
        .any(|n| name.eq_ignore_ascii_case(n))
    }
    #[cfg(target_os = "linux")]
    {
        ["snap", "proc", "sys", "dev", "lost+found"].contains(&name)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = name;
        false
    }
}
fn hidden(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        path.symlink_metadata()
            .is_ok_and(|m| m.file_attributes() & 2 != 0)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}
