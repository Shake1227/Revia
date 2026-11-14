use std::{os::unix::fs::PermissionsExt, path::Path, process::Command};

pub fn executable(script: &Path, mode: u32) {
    let staged = script.with_extension("staged");
    std::fs::rename(script, &staged).unwrap();
    let copied = Command::new("/bin/cp")
        .arg(&staged)
        .arg(script)
        .status()
        .unwrap();
    assert!(copied.success());
    std::fs::remove_file(&staged).unwrap();
    std::fs::set_permissions(script, std::fs::Permissions::from_mode(mode)).unwrap();
}
