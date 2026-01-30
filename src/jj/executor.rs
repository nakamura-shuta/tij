//! jj command executor
//!
//! Handles running jj commands and capturing their output.

use std::path::PathBuf;
use std::process::Command;

use super::JjError;
use super::constants::{self, commands, errors, flags, special};
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

    /// Run `jj log` with optional revset filter
    pub fn log_raw(&self, revset: Option<&str>) -> Result<String, JjError> {
        let template = Templates::log();
        let mut args = vec![commands::LOG, flags::NO_GRAPH, flags::TEMPLATE, template];

        if let Some(rev) = revset {
            args.push(flags::REVISION);
            args.push(rev);
        }

        self.run(&args)
    }

    /// Run `jj status`
    pub fn status_raw(&self) -> Result<String, JjError> {
        self.run(&[commands::STATUS])
    }

    /// Run `jj diff` for a specific change
    pub fn diff_raw(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[commands::DIFF, flags::REVISION, change_id])
    }

    /// Run `jj show` for a specific change
    pub fn show_raw(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[commands::SHOW, flags::REVISION, change_id])
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
