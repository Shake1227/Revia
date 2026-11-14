use crate::{
    core::Core,
    error::{AppError, Result},
};
use serde_json::{Value, json};
use std::{fs, io::Read, path::Path};
const LIMIT: u64 = 30 * 1024 * 1024;
fn mime(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}
fn valid_id(id: &str) -> bool {
    id.split_once('.').is_some_and(|(uuid, ext)| {
        uuid.len() == 36
            && uuid
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-')
            && mime(ext).is_some()
    })
}
impl Core {
    pub async fn copy_background_image(&self, source: &Path) -> Result<Value> {
        let _guard = self.settings_lock.lock().await;
        let source = source.to_path_buf();
        let dir = self.options.data_dir.join("backgrounds");
        tokio::task::spawn_blocking(move || copy_image(&source, &dir))
            .await
            .map_err(|_| AppError::new("io_error"))?
    }
    pub async fn read_background_image(&self, id: &str) -> Result<Vec<u8>> {
        if !valid_id(id) {
            return Err(AppError::new("invalid_input"));
        }
        let _guard = self.settings_lock.lock().await;
        let dir = self.options.data_dir.join("backgrounds");
        let path = dir.join(id);
        tokio::task::spawn_blocking(move || {
            ensure_directory(&dir)?;
            let metadata = fs::symlink_metadata(&path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AppError::new("path_not_found")
                } else {
                    e.into()
                }
            })?;
            if !metadata.is_file() || metadata.len() > LIMIT {
                return Err(AppError::new("invalid_input"));
            }
            Ok(fs::read(path)?)
        })
        .await
        .map_err(|_| AppError::new("io_error"))?
    }
    pub async fn clear_background_image(&self) -> Result<()> {
        let _guard = self.settings_lock.lock().await;
        let dir = self.options.data_dir.join("backgrounds");
        tokio::task::spawn_blocking(move || {
            ensure_directory(&dir)?;
            remove_others(&dir, None)
        })
        .await
        .map_err(|_| AppError::new("io_error"))?
    }
}
fn remove_others(dir: &Path, keep: Option<&str>) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    for entry in entries {
        let entry = entry?;
        if keep.is_some_and(|k| entry.file_name() == k) {
            continue;
        }
        if !entry.file_type()?.is_dir() {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn ensure_directory(dir: &Path) -> Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(dir)
        && (!metadata.is_dir() || metadata.file_type().is_symlink())
    {
        return Err(AppError::new("invalid_input"));
    }
    Ok(())
}

fn copy_image(source: &Path, dir: &Path) -> Result<Value> {
    ensure_directory(dir)?;
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = mime(&ext).ok_or_else(|| AppError::new("invalid_input"))?;
    let metadata = fs::metadata(source).map_err(|_| AppError::new("invalid_input"))?;
    if !metadata.is_file() || metadata.len() > LIMIT {
        return Err(AppError::new("invalid_input"));
    }
    let file = fs::File::open(source)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > LIMIT {
        return Err(AppError::new("invalid_input"));
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(AppError::new("invalid_input"));
    }
    let id = format!("{}.{}", uuid::Uuid::new_v4(), ext);
    crate::settings::atomic(dir, &id, &bytes)?;
    remove_others(dir, Some(&id))?;
    Ok(
        json!({"id":id,"name":source.file_name().unwrap_or_default().to_string_lossy(),"mime":mime,"size":bytes.len()}),
    )
}

pub(crate) fn normalize_filter_label(label: Option<&str>) -> String {
    let cleaned: String = label
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    let label: String = cleaned.trim().chars().take(40).collect();
    if label.is_empty() {
        "Images".into()
    } else {
        label
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_filter_label;

    #[test]
    fn filter_label_defaults() {
        for label in [None, Some(""), Some(" \t\n "), Some("\0\u{7f}")] {
            assert_eq!(normalize_filter_label(label), "Images");
        }
    }

    #[test]
    fn filter_label_removes_controls_and_trims() {
        assert_eq!(normalize_filter_label(Some(" \0 Im\nages\t ")), "Images");
    }

    #[test]
    fn filter_label_limits_unicode_characters() {
        assert_eq!(
            normalize_filter_label(Some(&"a".repeat(41))),
            "a".repeat(40)
        );
        assert_eq!(
            normalize_filter_label(Some(&"画".repeat(41))),
            "画".repeat(40)
        );
    }

    #[test]
    fn filter_label_preserves_normal_and_japanese_labels() {
        assert_eq!(normalize_filter_label(Some(" Photos ")), "Photos");
        assert_eq!(normalize_filter_label(Some(" 画像 ")), "画像");
    }
}
