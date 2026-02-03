//! jj command executor
//!
//! Handles running jj commands and capturing their output.

use std::path::PathBuf;
use std::process::Command;

use crate::model::{Change, DiffContent, Status};

use super::JjError;
use super::constants::{self, commands, errors, flags, special};
use super::parser::Parser;
use super::template::Templates;

/// Executor for jj commands
#[derive(Debug, Clone)]
pub struct JjExecutor {
    /// Path to the repository (None = current directory)
    repo_path: Option<PathBuf>,
}

impl Default for JjExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl JjExecutor {
    /// Create a new executor for the current directory
    pub fn new() -> Self {
        Self { repo_path: None }
    }

    /// Create a new executor for a specific repository path
    pub fn with_repo_path(path: PathBuf) -> Self {
        Self {
            repo_path: Some(path),
        }
    }

    /// Run a jj command with the given arguments
    ///
    /// Automatically adds `--color=never` to ensure parseable output.
    pub fn run(&self, args: &[&str]) -> Result<String, JjError> {
        let mut cmd = Command::new(constants::JJ_COMMAND);

        // Add repository path if specified
        if let Some(ref path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(path);
        }

        // Always disable color for parsing
        cmd.arg(flags::NO_COLOR);

        // Add user-specified arguments
        cmd.args(args);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                JjError::JjNotFound
            } else {
                JjError::IoError(e)
            }
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);

            // Check for common error patterns
            if stderr.contains(errors::NOT_A_REPO) {
                return Err(JjError::NotARepository);
            }

            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Get the jj version
    pub fn version(&self) -> Result<String, JjError> {
        let output = self.run(&[flags::VERSION])?;
        // Output format: "jj 0.37.0"
        let trimmed = output.trim();
        Ok(trimmed
            .strip_prefix(special::VERSION_PREFIX)
            .unwrap_or(trimmed)
            .to_string())
    }

    /// Check if jj version is supported
    pub fn check_version(&self) -> Result<(), JjError> {
        let version = self.version()?;
        if !is_version_supported(&version, constants::MIN_JJ_VERSION) {
            return Err(JjError::UnsupportedVersion {
                version,
                minimum: constants::MIN_JJ_VERSION.to_string(),
            });
        }
        Ok(())
    }

    /// Run `jj log` with optional revset filter (raw output)
    ///
    /// Note: Graph output is enabled to show DAG structure.
    /// The parser handles graph prefixes in the output.
    pub fn log_raw(&self, revset: Option<&str>) -> Result<String, JjError> {
        let template = Templates::log();
        let mut args = vec![commands::LOG, flags::TEMPLATE, template];

        if let Some(rev) = revset {
            args.push(flags::REVISION);
            args.push(rev);
        }

        self.run(&args)
    }

    /// Run `jj log` and parse the output into Changes
    pub fn log(&self, revset: Option<&str>) -> Result<Vec<Change>, JjError> {
        let output = self.log_raw(revset)?;
        Parser::parse_log(&output).map_err(|e| JjError::ParseError(e.to_string()))
    }

    /// Run `jj log` and parse output into Changes for current view.
    /// This is the preferred API for application code.
    pub fn log_changes(&self, revset: Option<&str>) -> Result<Vec<Change>, JjError> {
        self.log(revset)
    }

    /// Run `jj status`
    pub fn status_raw(&self) -> Result<String, JjError> {
        self.run(&[commands::STATUS])
    }

    /// Run `jj status` and parse the output into Status
    pub fn status(&self) -> Result<Status, JjError> {
        let output = self.status_raw()?;
        Parser::parse_status(&output)
    }

    /// Run `jj diff` for a specific change
    pub fn diff_raw(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[commands::DIFF, flags::REVISION, change_id])
    }

    /// Run `jj show` for a specific change
    pub fn show_raw(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[commands::SHOW, flags::REVISION, change_id])
    }

    /// Run `jj show` and parse the output into DiffContent
    ///
    /// This is the preferred API for application code, following the same pattern as log_changes().
    pub fn show(&self, change_id: &str) -> Result<DiffContent, JjError> {
        let output = self.show_raw(change_id)?;
        Parser::parse_show(&output)
    }

    /// Run `jj describe` to update change description
    ///
    /// Uses positional argument format: `jj describe <change-id> -m <message>`
    /// Note: `-r` is accepted as an alias for compatibility but positional is preferred.
    pub fn describe(&self, change_id: &str, message: &str) -> Result<String, JjError> {
        self.run(&[commands::DESCRIBE, change_id, "-m", message])
    }

    /// Run `jj edit` to set working-copy revision
    pub fn edit(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[commands::EDIT, change_id])
    }

    /// Run `jj new` to create a new empty change
    pub fn new_change(&self) -> Result<String, JjError> {
        self.run(&[commands::NEW])
    }

    /// Run `jj commit` to commit current changes with a message
    ///
    /// This is equivalent to `jj describe` + `jj new`, but atomic.
    /// After commit, a new empty change is created on top.
    pub fn commit(&self, message: &str) -> Result<String, JjError> {
        self.run(&[commands::COMMIT, "-m", message])
    }

    /// Run `jj squash -r <change-id>` to squash into parent
    ///
    /// Moves changes from the specified revision into its parent.
    /// If the source becomes empty, it is automatically abandoned.
    pub fn squash(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[commands::SQUASH, "-r", change_id])
    }

    /// Run `jj abandon <change-id>` to abandon a revision
    ///
    /// Descendants are automatically rebased onto the parent.
    /// If @ is abandoned, a new empty change is created.
    pub fn abandon(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[commands::ABANDON, change_id])
    }

    /// Run `jj undo` to undo the last operation
    ///
    /// Returns the raw output from the command for notification display.
    pub fn undo(&self) -> Result<String, JjError> {
        self.run(&[commands::UNDO])
    }

    /// Run `jj op restore` to restore a previous operation (redo)
    ///
    /// This restores the operation before the most recent undo, effectively redoing.
    /// The operation ID should be obtained from `get_redo_target()`.
    pub fn redo(&self, operation_id: &str) -> Result<String, JjError> {
        self.run(&[commands::OP, commands::OP_RESTORE, operation_id])
    }

    /// Get the redo target operation ID, if we're in an undo/redo chain.
    ///
    /// Returns `Some(operation_id)` if the most recent operation is an undo or restore
    /// (i.e., we're in an undo/redo chain).
    /// Returns `None` if there's nothing to redo.
    ///
    /// # Limitations
    ///
    /// **Single redo only**: This implementation only supports redoing after a single undo.
    /// After multiple consecutive undos, this returns `None` because the second line
    /// in the op log is also an undo operation.
    ///
    /// For multiple undo recovery, users should use Operation History View ('o' key)
    /// to restore to any arbitrary point in history.
    ///
    /// # Implementation Note
    ///
    /// This uses string matching on `description.first_line()` to detect undo/restore.
    /// The detection checks if the description starts with "undo" or "restore" (case-insensitive).
    ///
    /// **Known limitation**: If jj changes the operation description format,
    /// this detection may break. As of jj 0.37+:
    /// - Undo: "undo operation <id>"
    /// - Restore: "restore operation <id>"
    ///
    /// If jj adds a native `jj redo` command in the future, this implementation
    /// should be updated to use it instead.
    pub fn get_redo_target(&self) -> Result<Option<String>, JjError> {
        // Template: id<TAB>description.first_line()
        let output = self.run(&[
            commands::OP,
            commands::OP_LOG,
            "--no-graph",
            "-T",
            r#"id.short() ++ "\t" ++ description.first_line() ++ "\n""#,
            "--limit",
            "2",
        ])?;

        let lines: Vec<&str> = output.lines().collect();

        // We need at least 2 operations to redo
        if lines.len() < 2 {
            return Ok(None);
        }

        // Parse first line: check if it's an undo or restore operation
        let first_line = lines[0];
        let parts: Vec<&str> = first_line.split('\t').collect();
        if parts.len() < 2 {
            return Ok(None);
        }

        let first_desc = parts[1].to_lowercase();

        // Allow redo if the latest operation is an undo OR restore (in redo chain)
        if !first_desc.starts_with("undo") && !first_desc.starts_with("restore") {
            return Ok(None);
        }

        // Parse second line to get the operation to restore
        let second_line = lines[1];
        let second_parts: Vec<&str> = second_line.split('\t').collect();
        if second_parts.len() < 2 {
            return Ok(None);
        }

        let second_desc = second_parts[1].to_lowercase();

        // If second line is also an undo/restore, we can't redo properly
        // (multiple consecutive undos - need more complex logic)
        if second_desc.starts_with("undo") || second_desc.starts_with("restore") {
            return Ok(None);
        }

        Ok(Some(second_parts[0].trim().to_string()))
    }
}

/// Compare version strings (simple semver comparison)
///
/// Handles prerelease suffixes like "0.37.0-rc1" by stripping the suffix.
fn is_version_supported(version: &str, minimum: &str) -> bool {
    let parse_version = |v: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() >= 2 {
            let major = parts[0].parse().ok()?;
            let minor = parts[1].parse().ok()?;
            // Strip prerelease suffix (e.g., "0-rc1" -> "0")
            let patch = parts
                .get(2)
                .and_then(|p| {
                    // Split on '-' to handle prerelease versions
                    p.split('-').next().and_then(|n| n.parse().ok())
                })
                .unwrap_or(0);
            Some((major, minor, patch))
        } else {
            None
        }
    };

    match (parse_version(version), parse_version(minimum)) {
        (Some(v), Some(m)) => v >= m,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_version_supported("0.37.0", "0.20.0"));
        assert!(is_version_supported("0.20.0", "0.20.0"));
        assert!(is_version_supported("1.0.0", "0.20.0"));
        assert!(!is_version_supported("0.19.0", "0.20.0"));
        assert!(!is_version_supported("0.10.0", "0.20.0"));
    }

    #[test]
    fn test_version_comparison_prerelease() {
        // Prerelease versions should be supported if base version meets minimum
        assert!(is_version_supported("0.37.0-rc1", "0.20.0"));
        assert!(is_version_supported("0.20.0-beta", "0.20.0"));
        assert!(is_version_supported("1.0.0-alpha.1", "0.20.0"));
        assert!(!is_version_supported("0.19.0-rc1", "0.20.0"));
    }

    #[test]
    fn test_executor_default() {
        let executor = JjExecutor::default();
        assert!(executor.repo_path.is_none());
    }

    #[test]
    fn test_executor_with_path() {
        let executor = JjExecutor::with_repo_path(PathBuf::from("/tmp/test"));
        assert_eq!(executor.repo_path, Some(PathBuf::from("/tmp/test")));
    }
}
