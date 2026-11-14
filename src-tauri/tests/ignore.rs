#[cfg(unix)]
mod common;
use revia_lib::{
    git::Git,
    ignore::{classify_content, classify_path, path_pattern, scan},
};
use std::path::Path;

fn git() -> Git {
    let mut g = Git::default();
    g.env.insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
    g.env.insert(
        "GIT_CONFIG_GLOBAL".into(),
        if cfg!(windows) { "NUL" } else { "/dev/null" }.into(),
    );
    g
}
fn write(root: &Path, path: &str, bytes: impl AsRef<[u8]>) {
    let p = root.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}
#[test]
fn all_name_rules_and_negative_cases() {
    let rows = [
        (".env", false, "env_file"),
        ("a/.ENV.local", false, "env_file"),
        ("id_dsa_backup", false, "key_file"),
        ("a/identity.PFX", false, "key_file"),
        ("service-account-prod.json", false, "credential_file"),
        ("app-firebase-adminsdk-a.json", false, "credential_file"),
        ("顧客名簿.csv", false, "personal_data_file"),
        ("a/CONTACTS.xlsx", false, "personal_data_file"),
        ("a.vcf", false, "personal_data_file"),
        ("a.sqlite3", false, "database_file"),
        ("dump.sql", false, "database_file"),
        ("a/.DS_Store", false, "os_file"),
        ("._a", false, "os_file"),
        ("a.swp", false, "editor_file"),
        (".idea", true, "editor_file"),
        ("a/node_modules", true, "dependency_dir"),
        ("Pods", true, "dependency_dir"),
        ("a/target", true, "build_output"),
        ("a.pyc", false, "build_output"),
        (".cache", true, "cache_dir"),
        (".eslintcache", false, "cache_dir"),
        ("debug.log", false, "log_file"),
        ("a.tmp", false, "temp_file"),
        ("tmp", true, "temp_file"),
        ("test.md", false, "scratch_file"),
        ("新規テキスト.txt", false, "scratch_file"),
        ("untitled.rs", false, "scratch_file"),
        (".env.key", false, "env_file"),
        ("secrets.db", false, "credential_file"),
    ];
    for (path, dir, reason) in rows {
        assert_eq!(classify_path(path, dir).unwrap().reason, reason, "{path}");
    }
    for path in [
        ".env.example",
        ".ENV.SAMPLE",
        ".env.template",
        ".env.dist",
        "id_rsa.pub",
        "id_ed25519.pub",
        "tests/test_api.py",
        "tests/customers.csv",
        "tests/app.log",
        "tests/data.sqlite",
        "src/app.test.ts",
        "test_api.py",
        "test.py",
        "README.md",
        "customers.rs",
        "node_modules",
        "builder",
        "a.pem.pub",
    ] {
        assert!(classify_path(path, false).is_none(), "{path}");
    }
    for (path, reason) in [
        ("tests/.env", "env_file"),
        ("tests/fixtures/id_rsa", "key_file"),
        ("__tests__/credentials.json", "credential_file"),
    ] {
        assert_eq!(classify_path(path, false).unwrap().reason, reason, "{path}");
    }
    assert_eq!(
        classify_path("tests/node_modules", true).unwrap().reason,
        "dependency_dir"
    );
    for ext in ["pem", "key", "p12", "pfx", "jks", "keystore", "ppk"] {
        assert_eq!(
            classify_path(&format!("a.{ext}"), false).unwrap().reason,
            "key_file"
        );
    }
    for path in [
        ".npmrc",
        ".netrc",
        ".pypirc",
        ".htpasswd",
        "credentials.json",
        "client_secret.json",
        "secrets.txt",
        "a.tfvars",
        "terraform.tfstate",
        "terraform.tfstate.backup",
        "a.kdbx",
    ] {
        assert_eq!(
            classify_path(path, false).unwrap().reason,
            "credential_file"
        );
    }
}
#[test]
fn content_shapes_boundaries_and_test_exclusion() {
    let positives = [
        "-----BEGIN PRIVATE KEY-----".into(),
        "-----BEGIN RSA PRIVATE KEY-----".into(),
        format!("AKIA{}", "A".repeat(16)),
        format!("github_pat_{}_{}", "a".repeat(22), "B".repeat(59)),
        [
            "xoxb",
            "1234567890",
            "1234567890",
            "abcdefghijklmnopqrstuvwx",
        ]
        .join("-"),
        format!("AIza{}", "a".repeat(35)),
        format!("sk_live_{}", "a".repeat(24)),
    ];
    for secret in positives
        .into_iter()
        .chain(["ghp_", "gho_", "ghu_", "ghs_", "ghr_"].map(|s| format!("{s}{}", "A".repeat(36))))
    {
        assert_eq!(
            classify_content("src/main.rs", secret.as_bytes())
                .unwrap()
                .reason,
            "secret_in_content"
        );
        assert!(classify_content("tests/data.txt", secret.as_bytes()).is_none());
    }
    for bad in [
        format!("AKIA{}", "a".repeat(16)),
        format!("AKIA{}", "A".repeat(15)),
        format!("ghp_{}", "a".repeat(35)),
        format!("AIza{}", "a".repeat(34)),
        format!("sk_live_{}", "a".repeat(23)),
        "xoxb-short".into(),
        "-----BEGIN PUBLIC KEY-----".into(),
    ] {
        assert!(classify_content("a.txt", bad.as_bytes()).is_none());
    }
    let emails = |n| {
        (0..n)
            .map(|i| format!("person{i}@example.invalid\n"))
            .collect::<String>()
    };
    assert!(classify_content("a.csv", emails(19).as_bytes()).is_none());
    assert_eq!(
        classify_content("a.csv", emails(20).as_bytes())
            .unwrap()
            .reason,
        "personal_in_content"
    );
    assert!(classify_content("a.rs", emails(20).as_bytes()).is_none());
    assert!(classify_content("a.csv", "same@example.invalid\n".repeat(20).as_bytes()).is_none());
    let phones = "090-1234-5678\n".repeat(20);
    assert_eq!(
        classify_content("a.yml", phones.as_bytes()).unwrap().reason,
        "personal_in_content"
    );
    assert!(classify_content("a.txt", b"-----BEGIN PRIVATE KEY-----\0").is_none());
    let mut big = b"-----BEGIN PRIVATE KEY-----".to_vec();
    big.resize(256 * 1024 + 1, b' ');
    assert!(classify_content("a.txt", &big).is_none());
}
#[tokio::test]
async fn escaped_patterns_ignore_only_literal_files() {
    let t = tempfile::tempdir().unwrap();
    let g = git();
    g.local(t.path(), &["init"]).await.unwrap();
    for path in [
        "#file",
        "!file",
        "a[b].txt",
        "a*b.txt",
        "a?b.txt",
        "space file ",
        "日本語/顧客.csv",
    ] {
        if cfg!(windows) && (path.contains(['*', '?']) || path.ends_with(' ')) {
            continue;
        }
        write(t.path(), path, "");
        write(t.path(), "control.txt", "");
        write(
            t.path(),
            ".gitignore",
            format!("{}\n", path_pattern(path, false)),
        );
        let out = g
            .local(
                t.path(),
                &["ls-files", "--others", "--exclude-standard", "-z"],
            )
            .await
            .unwrap();
        assert!(!out.split('\0').any(|p| p == path), "{path}: {out}");
        assert!(out.split('\0').any(|p| p == "control.txt"));
        std::fs::remove_file(t.path().join(path)).unwrap();
    }
    assert_eq!(path_pattern("a\\b", false), "/a\\\\b");
}
#[tokio::test]
async fn scans_uninitialized_and_initialized_without_mutation() {
    let t = tempfile::tempdir().unwrap();
    let g = git();
    write(t.path(), ".gitignore", "ignored.pem\n");
    write(t.path(), "ignored.pem", "");
    write(t.path(), "node_modules/.env", "");
    write(t.path(), "a/.DS_Store", "");
    write(t.path(), "b/.DS_Store", "");
    write(t.path(), "a/.env", "must-not-appear-in-response");
    write(t.path(), "test.py", "");
    write(t.path(), "tests/.env", "");
    let before = std::fs::read_dir(t.path()).unwrap().count();
    let result = scan(&g, t.path(), "p").await.unwrap();
    assert_eq!(std::fs::read_dir(t.path()).unwrap().count(), before);
    assert!(!t.path().join(".git").exists());
    assert_eq!(result["truncated"], false, "{result}");
    let s = result["suggestions"].as_array().unwrap();
    assert_eq!(s.len(), 4, "{result}");
    assert_eq!(s[0]["pattern"], "/a/.env");
    assert_eq!(s[1]["pattern"], "/tests/.env");
    assert_eq!(s[2]["pattern"], ".DS_Store");
    assert_eq!(s[2]["matches"], 2);
    assert_eq!(s[2]["examples"].as_array().unwrap().len(), 2);
    assert_eq!(s[3]["pattern"], "node_modules/");
    assert!(!result.to_string().contains("must-not-appear"));
    g.local(t.path(), &["init"]).await.unwrap();
    g.local(t.path(), &["add", "--", "a/.env"]).await.unwrap();
    let result = scan(&g, t.path(), "p").await.unwrap();
    assert!(!result.to_string().contains("/a/.env"));
}
#[tokio::test]
async fn scan_depth_and_content_budget_are_reported() {
    let t = tempfile::tempdir().unwrap();
    let g = git();
    write(
        t.path(),
        &format!("{}/.env", vec!["deep"; 13].join("/")),
        "",
    );
    assert_eq!(scan(&g, t.path(), "p").await.unwrap()["truncated"], true);
    let t = tempfile::tempdir().unwrap();
    for i in 0..2001 {
        write(t.path(), &format!("{i}.txt"), "plain");
    }
    assert_eq!(scan(&g, t.path(), "p").await.unwrap()["truncated"], true);
}

#[tokio::test]
async fn scan_entry_and_byte_limits_and_example_cap() {
    let g = git();
    let t = tempfile::tempdir().unwrap();
    for i in 0..7 {
        write(t.path(), &format!("dir{i}/.DS_Store"), "");
    }
    let result = scan(&g, t.path(), "p").await.unwrap();
    assert_eq!(result["suggestions"][0]["matches"], 7);
    assert_eq!(
        result["suggestions"][0]["examples"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    let t = tempfile::tempdir().unwrap();
    let body = vec![b' '; 256 * 1024];
    for i in 0..129 {
        write(t.path(), &format!("{i}.txt"), &body);
    }
    assert_eq!(scan(&g, t.path(), "p").await.unwrap()["truncated"], true);
    let t = tempfile::tempdir().unwrap();
    for i in 0..20_001 {
        write(t.path(), &format!("{i}.pem"), "");
    }
    let result = scan(&g, t.path(), "p").await.unwrap();
    assert_eq!(result["truncated"], true);
    assert!(result["suggestions"].as_array().unwrap().len() <= 20_000);
}
#[cfg(unix)]
#[tokio::test]
async fn scan_deadline_and_symlinks() {
    use std::os::unix::fs::symlink;
    let g = git();
    let t = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), ".env", "private");
    symlink(outside.path(), t.path().join("linked")).unwrap();
    symlink(outside.path().join(".env"), t.path().join(".env")).unwrap();
    assert!(
        scan(&g, t.path(), "p").await.unwrap()["suggestions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let script = outside.path().join("slow-git");
    std::fs::write(&script, "#!/bin/sh\nexec /bin/sleep 10\n").unwrap();
    common::executable(&script, 0o755);
    let mut slow = g;
    slow.executable = script;
    let started = std::time::Instant::now();
    assert_eq!(scan(&slow, t.path(), "p").await.unwrap()["truncated"], true);
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    assert!(!t.path().join(".git").exists());
}
#[test]
fn remaining_name_aliases_and_precedence() {
    for (names, reason) in [
        (
            vec![
                "bower_components",
                ".venv",
                "venv",
                ".gradle",
                "__pypackages__",
            ],
            "dependency_dir",
        ),
        (
            vec![
                "dist",
                "build",
                "out",
                ".next",
                ".nuxt",
                ".output",
                ".svelte-kit",
                ".turbo",
                "DerivedData",
                "coverage",
                "__pycache__",
            ],
            "build_output",
        ),
        (
            vec![
                ".parcel-cache",
                ".pytest_cache",
                ".mypy_cache",
                ".ruff_cache",
                ".sass-cache",
            ],
            "cache_dir",
        ),
        (vec!["temp"], "temp_file"),
    ] {
        for name in names {
            assert_eq!(classify_path(name, true).unwrap().reason, reason);
            assert!(classify_path(name, false).is_none() || name == "temp");
        }
    }
    for (path, reason) in [
        ("test", "scratch_file"),
        ("a.swo", "editor_file"),
        ("a.iml", "editor_file"),
        ("a~", "editor_file"),
        ("Thumbs.db", "os_file"),
        ("desktop.ini", "os_file"),
        ("a.temp", "temp_file"),
        ("a.bak", "temp_file"),
        ("a.old", "temp_file"),
        ("a.orig", "temp_file"),
        ("a.sql.gz", "database_file"),
        ("address_book.txt", "personal_data_file"),
    ] {
        assert_eq!(classify_path(path, false).unwrap().reason, reason, "{path}");
    }
}

#[tokio::test]
async fn scan_global_excludes_content_precedence_and_partially_tracked_dependency() {
    let mut g = git();
    let t = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let excludes = outside.path().join("global-ignore");
    std::fs::write(&excludes, "global.pem\n").unwrap();
    g.env.extend([
        ("GIT_CONFIG_COUNT".into(), "1".into()),
        ("GIT_CONFIG_KEY_0".into(), "core.excludesfile".into()),
        (
            "GIT_CONFIG_VALUE_0".into(),
            excludes.to_string_lossy().into_owned(),
        ),
    ]);
    let token = format!("ghp_{}", "a".repeat(36));
    write(t.path(), "customers.csv", &token);
    write(t.path(), "global.pem", &token);
    write(t.path(), ".cache/.env", &token);
    write(t.path(), "node_modules/.env", &token);
    write(t.path(), "node_modules/tracked", "");
    write(t.path(), "src/main.rs", &token);
    write(t.path(), "src/app.test.ts", &token);
    write(t.path(), "test.py", &token);
    write(t.path(), "Thumbs.db", "");
    g.local(t.path(), &["init"]).await.unwrap();
    g.local(t.path(), &["add", "--", "node_modules/tracked"])
        .await
        .unwrap();
    let result = scan(&g, t.path(), "p").await.unwrap();
    let s = result["suggestions"].as_array().unwrap();
    assert_eq!(s.len(), 5, "{result}");
    assert_eq!(
        s.iter().find(|s| s["pattern"] == "/customers.csv").unwrap()["reason"],
        "secret_in_content"
    );
    assert_eq!(
        s.iter().find(|s| s["pattern"] == "Thumbs.db").unwrap()["reason"],
        "os_file"
    );
    for path in [
        "global.pem",
        "node_modules/.env",
        ".cache/.env",
        "app.test.ts",
        "test.py",
        &token,
    ] {
        assert!(!result.to_string().contains(path), "{path}");
    }
}
