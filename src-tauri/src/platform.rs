use std::path::{Path, PathBuf};

pub fn strip_verbatim(value: &str) -> String {
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        let mut parts = rest.split('\\');
        if parts.next().is_some_and(|s| !s.is_empty())
            && parts.next().is_some_and(|s| !s.is_empty())
        {
            format!(r"\\{rest}")
        } else {
            value.into()
        }
    } else if let Some(rest) = value.strip_prefix(r"\\?\") {
        if rest.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && rest.as_bytes().get(1) == Some(&b':')
            && rest.as_bytes().get(2) == Some(&b'\\')
        {
            rest.into()
        } else {
            value.into()
        }
    } else {
        value.into()
    }
}
pub fn normalize(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        path.to_str()
            .map(|p| PathBuf::from(strip_verbatim(p)))
            .unwrap_or_else(|| path.to_path_buf())
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}
pub fn canonicalize(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    std::fs::canonicalize(path).map(|p| normalize(&p))
}
pub fn resolve(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    canonicalize(path)
        .or_else(|_| std::path::absolute(path).map(|p| normalize(&p)))
        .unwrap_or_else(|_| normalize(path))
}
pub fn home() -> Option<PathBuf> {
    #[cfg(windows)]
    let key = "USERPROFILE";
    #[cfg(not(windows))]
    let key = "HOME";
    std::env::var_os(key).map(PathBuf::from)
}
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    let program = program.as_ref();
    #[cfg(windows)]
    {
        let mut cmd = tokio::process::Command::new(program);
        cmd.creation_flags(0x08000000);
        cmd
    }
    #[cfg(not(windows))]
    {
        tokio::process::Command::new(program)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_program_is_passed_directly_to_rust() {
        for program in ["tool.cmd", "tool.BAT", "tool.exe"] {
            let cmd = command(program);
            assert_eq!(cmd.as_std().get_program(), program);
        }
        let error = std::io::Error::from(std::io::ErrorKind::InvalidInput);
        assert_eq!(crate::error::AppError::from(error).code, "invalid_input");
    }
    #[test]
    fn resolve_keeps_working_where_canonicalize_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(resolve(dir.path()), canonicalize(dir.path()).unwrap());
        let missing = dir.path().join("missing").join("child");
        assert!(canonicalize(&missing).is_err());
        let resolved = resolve(&missing);
        assert!(resolved.is_absolute());
        assert!(resolved.ends_with(Path::new("missing").join("child")));
    }
    #[test]
    fn verbatim_paths() {
        assert_eq!(strip_verbatim(r"\\?\C:\repo"), r"C:\repo");
        assert_eq!(strip_verbatim(r"\\?\UNC\server\share"), r"\\server\share");
        assert_eq!(strip_verbatim(r"\\?\Volume{a}\"), r"\\?\Volume{a}\");
        assert_eq!(strip_verbatim("/tmp/a"), "/tmp/a");
        assert_eq!(strip_verbatim(r"\\?\UNC\server"), r"\\?\UNC\server");
        assert_eq!(strip_verbatim(r"\\?\1:\repo"), r"\\?\1:\repo");
    }
}
