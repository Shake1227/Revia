use crate::{
    core::Core,
    error::{AppError, Result},
    git,
};
use serde_json::{Value, json};
use std::path::Path;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
pub async fn pick(app: tauri::AppHandle, title: Option<String>) -> Result<Option<String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut dialog = app.dialog().file();
    if let Some(title) = title {
        dialog = dialog.set_title(title);
    }
    dialog.pick_folder(move |p| {
        let _ = tx.send(
            p.and_then(|p| p.into_path().ok())
                .and_then(|p| crate::platform::canonicalize(p).ok())
                .map(|p| p.to_string_lossy().into_owned()),
        );
    });
    rx.await.map_err(|_| AppError::new("io_error"))
}
pub fn open_url(app: &tauri::AppHandle, url: &str) -> Result<()> {
    if !allowed_url(url) {
        return Err(AppError::new("invalid_input"));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| AppError::new("io_error"))
}
pub fn editors() -> Vec<Value> {
    [
        ("code", "VS Code"),
        ("cursor", "Cursor"),
        ("zed", "Zed"),
        ("subl", "Sublime Text"),
        #[cfg(target_os = "macos")]
        ("xed", "Xcode"),
    ]
    .into_iter()
    .filter(|(id, _)| editor_tool(id).is_file() || editor_bundle(id).is_some())
    .map(|(id, name)| json!({"id":id,"name":name}))
    .collect()
}
fn editor_tool(id: &str) -> std::path::PathBuf {
    #[cfg(target_os = "linux")]
    {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|p| p.join(id))
            .find(|p| git::executable(p))
            .map(|p| crate::platform::canonicalize(&p).unwrap_or(p))
            .unwrap_or_else(|| id.into())
    }
    #[cfg(not(target_os = "linux"))]
    {
        git::find_tool(id)
    }
}
#[cfg(target_os = "macos")]
fn editor_bundle(id: &str) -> Option<std::path::PathBuf> {
    let name = match id {
        "code" => "Visual Studio Code.app",
        "cursor" => "Cursor.app",
        "zed" => "Zed.app",
        "subl" => "Sublime Text.app",
        "xed" => "Xcode.app",
        _ => return None,
    };
    let mut roots = vec![std::path::PathBuf::from("/Applications")];
    if let Some(home) = crate::platform::home() {
        roots.push(home.join("Applications"));
    }
    roots.into_iter().map(|p| p.join(name)).find(|p| p.is_dir())
}
#[cfg(not(target_os = "macos"))]
fn editor_bundle(_: &str) -> Option<std::path::PathBuf> {
    None
}

pub fn editor_args(path: &Path) -> Vec<std::ffi::OsString> {
    vec![path.as_os_str().to_owned()]
}

pub fn terminal_args(name: &str, path: &Path) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = match name {
        "open" => vec!["-a".into(), "Terminal".into()],
        "wt.exe" => vec!["-d".into()],
        "cmd.exe" => vec!["/C".into(), "start".into(), "".into(), "cmd.exe".into()],
        "x-terminal-emulator" | "xterm" => vec![],
        "konsole" => vec!["--workdir".into()],
        "kitty" => vec!["--directory".into()],
        "alacritty" => vec!["--working-directory".into()],
        "wezterm" => vec!["start".into(), "--cwd".into()],
        "foot" => vec!["--working-directory".into()],
        _ => vec!["--working-directory".into()],
    };
    if !matches!(name, "cmd.exe" | "x-terminal-emulator" | "xterm") {
        args.push(path.as_os_str().to_owned());
    }
    args
}
fn terminal() -> Result<(&'static str, std::path::PathBuf)> {
    #[cfg(target_os = "macos")]
    let names = ["open"];
    #[cfg(windows)]
    let names = ["wt.exe", "cmd.exe"];
    #[cfg(target_os = "linux")]
    let names = [
        "x-terminal-emulator",
        "gnome-terminal",
        "konsole",
        "xfce4-terminal",
        "kitty",
        "alacritty",
        "wezterm",
        "foot",
        "xterm",
    ];
    names
        .into_iter()
        .find_map(|name| {
            let path = git::find_tool(name);
            path.is_file().then_some((name, path))
        })
        .ok_or_else(|| AppError::new("invalid_input").detail("No available terminal found"))
}
pub async fn open(
    core: &Core,
    app: &tauri::AppHandle,
    id: &str,
    target: &str,
    relative: Option<String>,
) -> Result<()> {
    let project = core.project(id)?;
    let root = Path::new(project["path"].as_str().unwrap());
    let path = if let Some(p) = relative {
        git::relative(root, &p)?
    } else {
        root.into()
    };
    let path = crate::platform::canonicalize(path).map_err(|_| AppError::new("path_not_found"))?;
    if !path.starts_with(root) {
        return Err(AppError::new("invalid_path"));
    }
    match target {
        "github" => open_url(
            app,
            project["status"]["github"]["url"]
                .as_str()
                .ok_or_else(|| AppError::new("invalid_input"))?,
        ),
        "file_manager" => {
            if path.is_file() {
                app.opener()
                    .reveal_item_in_dir(&path)
                    .map_err(|_| AppError::new("io_error"))
            } else {
                app.opener()
                    .open_path(path.to_string_lossy(), None::<&str>)
                    .map_err(|_| AppError::new("io_error"))
            }
        }
        "editor" => {
            let chosen = core.workspace()["settings"]["general"]["editor"]
                .as_str()
                .map(str::to_string)
                .or_else(|| {
                    editors()
                        .first()
                        .and_then(|v| v["id"].as_str())
                        .map(str::to_string)
                });
            if let Some(editor) = chosen {
                if !editors().iter().any(|v| v["id"] == editor) {
                    return Err(AppError::new("invalid_input"));
                }
                #[cfg(target_os = "macos")]
                if let Some(bundle) = editor_bundle(&editor) {
                    crate::platform::command("/usr/bin/open")
                        .arg("-a")
                        .arg(bundle)
                        .args(editor_args(&path))
                        .spawn()?;
                    return Ok(());
                }
                crate::platform::command(editor_tool(&editor))
                    .args(editor_args(&path))
                    .spawn()?;
                Ok(())
            } else {
                app.opener()
                    .open_path(path.to_string_lossy(), None::<&str>)
                    .map_err(|_| AppError::new("io_error"))
            }
        }
        "terminal" => {
            let directory = if path.is_file() {
                path.parent().unwrap_or(root)
            } else {
                &path
            };
            let (name, executable) = terminal()?;
            crate::platform::command(executable)
                .args(terminal_args(name, directory))
                .current_dir(directory)
                .spawn()?;
            Ok(())
        }
        _ => Err(AppError::new("invalid_input")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn application_arguments_preserve_paths() {
        let path = Path::new("directory with spaces/日本語");
        for (name, flags) in [
            ("open", vec!["-a", "Terminal"]),
            ("wt.exe", vec!["-d"]),
            ("gnome-terminal", vec!["--working-directory"]),
            ("konsole", vec!["--workdir"]),
            ("xfce4-terminal", vec!["--working-directory"]),
            ("kitty", vec!["--directory"]),
            ("alacritty", vec!["--working-directory"]),
            ("wezterm", vec!["start", "--cwd"]),
            ("foot", vec!["--working-directory"]),
        ] {
            let mut expected = flags
                .into_iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>();
            expected.push(path.as_os_str().into());
            assert_eq!(terminal_args(name, path), expected, "{name}");
        }
        for name in ["xterm", "x-terminal-emulator"] {
            assert!(terminal_args(name, path).is_empty());
        }
        assert_eq!(
            terminal_args("cmd.exe", path),
            vec![
                std::ffi::OsString::from("/C"),
                "start".into(),
                "".into(),
                "cmd.exe".into()
            ]
        );
        assert_eq!(editor_args(path), vec![path.as_os_str().to_owned()]);
    }
}

fn allowed_url(url: &str) -> bool {
    [
        "https://github.com/",
        "https://x.com/",
        "https://creativecommons.org/",
    ]
    .iter()
    .any(|prefix| url.starts_with(prefix))
        && !url.chars().any(char::is_control)
        && !url.contains(['\\', '@'])
}
#[cfg(test)]
mod url_tests {
    #[test]
    fn allowed_urls() {
        for url in [
            "https://github.com/a",
            "https://x.com/a",
            "https://creativecommons.org/licenses/",
        ] {
            assert!(super::allowed_url(url));
        }
        for url in [
            "https://x.com.evil.example/",
            "https://github.com@evil.example/",
            "https://github.com/a\\b",
            "https://github.com/a\n",
            "http://github.com/",
            "https://github.com",
        ] {
            assert!(!super::allowed_url(url));
        }
    }
}
