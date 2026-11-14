use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
}
pub type Result<T> = std::result::Result<T, AppError>;
impl AppError {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.into(),
            message: match code {
                "verification_failed" => "Downloaded file did not match the published release",
                "repo_locked" => "Repository is locked by another Git process",
                "push_rejected" => "Remote has new commits",
                "invalid_input" => "Invalid input",
                "invalid_path" => "Invalid path",
                "invalid_ref_name" => "Invalid branch or remote name",
                "project_not_found" => "Project not found",
                "git_not_installed" => "Git not found",
                "gh_not_installed" => "GitHub CLI not found",
                "scope_missing" => "Required GitHub scope is missing",
                "not_authenticated" => "Not authenticated with GitHub",
                "rate_limited" => "GitHub rate limit reached",
                "busy" => "Operation already in progress",
                "repo_exists" => "Repository already exists",
                "destination_exists" => "Destination already exists",
                "auth_failed" => "Authentication failed",
                "permission_denied" => "Permission denied",
                "network_unreachable" => "Remote is unreachable",
                "remote_not_found" => "Remote not found",
                "disk_full" => "Disk is full",
                "merge_conflict" => "Merge conflict",
                "dirty_worktree" => "Uncommitted changes exist",
                "identity_missing" => "Commit identity is missing",
                "not_fast_forward" => "Cannot fast-forward",
                "timeout" => "Operation timed out",
                "cancelled" => "Operation cancelled",
                "io_error" => "File I/O failed",
                "state_changed" => "State changed",
                "feature_disabled" => "Operation disabled in settings",
                "branch_not_found" => "Branch not found",
                "unborn" => "No commits yet",
                "detached" => "HEAD is detached",
                "conflicted" => "Unresolved conflicts exist",
                "in_progress" => "An operation is in progress",
                "unknown" => "Git command failed",
                "branch_exists" => "Branch already exists",
                "branch_not_merged" => "Unmerged commits exist",
                "remote_exists" => "Remote already exists",
                "operation_in_progress" => "An operation must be completed",
                "not_a_repository" => "Not a Git repository",
                "settings_corrupt" => "Cannot read settings file",
                "already_registered" => "Already registered",
                "path_not_found" => "Path not found",
                _ => "Operation failed",
            }
            .into(),
            detail: None,
        }
    }
    pub fn detail(mut self, detail: &str) -> Self {
        self.detail = Some(sanitize(detail));
        self
    }
}
impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::new(if error.kind() == std::io::ErrorKind::InvalidInput {
            "invalid_input"
        } else {
            "io_error"
        })
    }
}
impl From<serde_json::Error> for AppError {
    fn from(_: serde_json::Error) -> Self {
        Self::new("invalid_input")
    }
}
pub fn strip_userinfo(input: &str) -> String {
    let mut text = input.to_string();
    let mut offset = 0;
    while let Some(i) = text[offset..].find("://") {
        let start = offset + i + 3;
        let end = text[start..]
            .find(['/', ' ', '\n', '\r', '\t', '\'', '"'])
            .map_or(text.len(), |n| start + n);
        if let Some(at) = text[start..end].rfind('@') {
            text.replace_range(start..start + at + 1, "");
        }
        offset = start;
    }
    text
}
pub fn sanitize(input: &str) -> String {
    strip_userinfo(input)
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.contains("authorization:")
                || lower.contains("access_token=")
                || lower.contains("token=")
                || lower.contains("token:")
                || lower.contains("password=")
                || lower.contains("ghp_")
                || lower.contains("github_pat_")
                || lower.contains("gho_")
                || lower.contains("ghu_")
                || lower.contains("ghs_")
                || lower.contains("ghr_")
            {
                "[redacted]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_error_messages_are_ascii() {
        for code in [
            "scope_missing",
            "verification_failed",
            "repo_locked",
            "push_rejected",
            "invalid_input",
            "invalid_path",
            "invalid_ref_name",
            "project_not_found",
            "git_not_installed",
            "gh_not_installed",
            "not_authenticated",
            "rate_limited",
            "busy",
            "repo_exists",
            "destination_exists",
            "auth_failed",
            "permission_denied",
            "network_unreachable",
            "remote_not_found",
            "disk_full",
            "merge_conflict",
            "dirty_worktree",
            "identity_missing",
            "not_fast_forward",
            "timeout",
            "cancelled",
            "io_error",
            "state_changed",
            "feature_disabled",
            "branch_not_found",
            "unborn",
            "detached",
            "conflicted",
            "in_progress",
            "unknown",
            "branch_exists",
            "branch_not_merged",
            "remote_exists",
            "operation_in_progress",
            "not_a_repository",
            "settings_corrupt",
            "already_registered",
            "path_not_found",
            "unrecognized_code",
        ] {
            let error = AppError::new(code);
            assert_eq!(error.code, code);
            assert!(!error.message.is_empty());
            assert!(error.message.is_ascii(), "{code}: {}", error.message);
        }
    }

    #[test]
    fn secret_lines_use_english_redaction_marker() {
        assert_eq!(
            sanitize("diagnostic\nAuthorization: secret\ntoken=secret"),
            "diagnostic\n[redacted]\n[redacted]"
        );
    }
}
