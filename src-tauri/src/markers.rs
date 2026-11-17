use crate::{
    error::{AppError, Result},
    git::{self, Git},
};
use std::{collections::BTreeSet, io::Read, path::Path};

#[derive(Default)]
pub struct Scanner {
    prefix: Vec<u8>,
    state: u8,
    found: bool,
}

impl Scanner {
    pub fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.found {
                return;
            }
            if byte == b'\n' {
                self.line();
            } else if self.prefix.len() < 8 {
                self.prefix.push(byte);
            }
        }
    }

    pub fn finish(mut self) -> bool {
        if !self.prefix.is_empty() {
            self.line();
        }
        self.found
    }

    fn line(&mut self) {
        let prefix = std::mem::take(&mut self.prefix);
        let marker = |c: u8, labelled: bool| {
            prefix.len() >= 7
                && prefix[..7].iter().all(|b| *b == c)
                && match prefix.get(7) {
                    None | Some(b'\r') => true,
                    Some(b' ') => labelled,
                    _ => false,
                }
        };
        self.state = if marker(b'<', true) {
            1
        } else if marker(b'|', true) && matches!(self.state, 1 | 2) {
            2
        } else if marker(b'=', false) && matches!(self.state, 1..=3) {
            3
        } else if marker(b'>', true) && self.state == 3 {
            self.found = true;
            0
        } else if marker(b'>', true) {
            0
        } else {
            self.state
        };
    }
}

pub fn bytes(content: &[u8]) -> bool {
    let mut scanner = Scanner::default();
    scanner.feed(content);
    scanner.finish()
}

pub fn file(path: &Path) -> std::io::Result<bool> {
    let mut reader = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    if !std::fs::metadata(path)?.is_file() {
        return Ok(false);
    }
    let mut scanner = Scanner::default();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        scanner.feed(&buffer[..n]);
    }
    Ok(scanner.finish())
}

async fn blob(g: &Git, root: &Path, spec: &str) -> Option<bool> {
    let output = g.raw(root, &["cat-file", "blob", spec]).await.ok()?;
    output.status.success().then(|| bytes(&output.stdout))
}

#[derive(Clone, Copy)]
pub enum Source<'a> {
    Index,
    Tree(&'a str),
}

async fn changed(
    g: &Git,
    root: &Path,
    source: &Source<'_>,
    base: Option<&str>,
) -> Option<BTreeSet<String>> {
    let base = base?;
    let mut args = vec![
        "diff",
        "--name-only",
        "-z",
        "--no-renames",
        "--no-ext-diff",
        "--no-textconv",
    ];
    match source {
        Source::Index => args.extend(["--cached", base]),
        Source::Tree(rev) => args.extend([base, rev]),
    }
    let output = g.raw(root, &args).await.ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    })
}

pub async fn introduced(
    g: &Git,
    root: &Path,
    source: Source<'_>,
    base: Option<&str>,
) -> Result<Vec<String>> {
    let scope = changed(g, root, &source, base).await;
    if scope.as_ref().is_some_and(BTreeSet::is_empty) {
        return Ok(vec![]);
    }
    let mut args = vec!["grep", "-I", "-l", "-z", "--full-name", "--no-color"];
    let prefix = match source {
        Source::Index => {
            args.push("--cached");
            String::new()
        }
        Source::Tree(rev) => format!("{rev}:"),
    };
    args.extend(["-e", "^<<<<<<<"]);
    if let Source::Tree(rev) = source {
        args.push(rev);
    }
    let narrow = scope
        .as_ref()
        .is_some_and(|s| s.iter().map(|p| p.len() + 3).sum::<usize>() < 16 * 1024);
    if narrow {
        args.push("--");
        args.extend(scope.iter().flatten().map(String::as_str));
    }
    let output = g.raw(root, &args).await?;
    match output.status.code() {
        Some(0) => {}
        Some(1) if output.stderr.is_empty() => return Ok(vec![]),
        _ => return Err(git::classify(&String::from_utf8_lossy(&output.stderr))),
    }
    let listed = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut found = vec![];
    for entry in listed.split('\0').filter(|s| !s.is_empty()) {
        let path = entry.strip_prefix(&prefix).unwrap_or(entry);
        if scope.as_ref().is_some_and(|s| !s.contains(path)) {
            continue;
        }
        let spec = match source {
            Source::Index => format!(":0:{path}"),
            Source::Tree(rev) => format!("{rev}:{path}"),
        };
        if blob(g, root, &spec).await != Some(true) {
            continue;
        }
        if let Some(base) = base
            && blob(g, root, &format!("{base}:{path}")).await == Some(true)
        {
            continue;
        }
        found.push(path.to_string());
    }
    Ok(found)
}

pub async fn worktree(
    g: &Git,
    root: &Path,
    paths: &[&str],
    base: Option<&str>,
) -> Result<Vec<String>> {
    let mut found = vec![];
    for path in paths {
        let full = git::git_path(root, path)?;
        if !file(&full)? {
            continue;
        }
        if let Some(base) = base
            && blob(g, root, &format!("{base}:{path}")).await == Some(true)
        {
            continue;
        }
        found.push(path.to_string());
    }
    Ok(found)
}

pub fn error(paths: &[String]) -> AppError {
    let mut shown = paths.iter().take(20).cloned().collect::<Vec<_>>();
    if paths.len() > shown.len() {
        shown.push(format!("+{}", paths.len() - shown.len()));
    }
    AppError::new("conflict_markers").detail(&shown.join("\n"))
}

pub fn check(paths: Vec<String>) -> Result<()> {
    if paths.is_empty() {
        Ok(())
    } else {
        Err(error(&paths))
    }
}

#[cfg(test)]
mod tests {
    use super::bytes;

    #[test]
    fn finds_complete_conflict_blocks() {
        assert!(bytes(
            b"a\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> refs/remotes/origin/main\nb\n"
        ));
        assert!(bytes(
            b"<<<<<<< HEAD\r\nours\r\n=======\r\ntheirs\r\n>>>>>>> other\r\n"
        ));
        assert!(bytes(
            b"<<<<<<< ours\nx\n||||||| base\ny\n=======\nz\n>>>>>>> theirs"
        ));
        assert!(bytes(b"<<<<<<<\n=======\n>>>>>>>\n"));
    }

    #[test]
    fn ignores_text_that_only_looks_similar() {
        assert!(!bytes(b"Title\n=======\n\ntext\n"));
        assert!(!bytes(b"let s = \"<<<<<<< HEAD\";\n=======\n>>>>>>> x\n"));
        assert!(!bytes(b"<<<<<<< HEAD\nonly the start\n"));
        assert!(!bytes(b"<<<<<<<< eight\n=======\n>>>>>>> x\n"));
        assert!(!bytes(b"<<<<<<< a\n>>>>>>> b\n=======\n"));
        assert!(!bytes(b"<<<<<<< a\n=======x\n>>>>>>> b\n"));
        assert!(!bytes(b""));
    }
}
