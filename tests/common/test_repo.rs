//! TestRepo helper for integration tests.
//!
//! Provides a temporary jj repository for testing Tij operations.

use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// A temporary jj repository for testing.
///
/// The repository is automatically cleaned up when the TestRepo is dropped.
pub struct TestRepo {
    dir: TempDir,
}

impl TestRepo {
    /// Create a new jj repository in a temporary directory.
    pub fn new() -> Self {
        let dir = TempDir::new().expect("Failed to create temp directory");

        let output = Command::new("jj")
            .args(["git", "init"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to execute jj git init");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("jj git init failed: {}", stderr);
        }

        Self { dir }
    }

    /// Get the path to the repository root.
    pub fn path(&self) -> PathBuf {
        self.dir.path().to_path_buf()
    }

    /// Execute a jj command in this repository.
    ///
    /// # Panics
    ///
    /// Panics if the command fails to execute or returns a non-zero exit code.
    pub fn jj(&self, args: &[&str]) -> String {
        let output = Command::new("jj")
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("Failed to execute jj command");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "jj {:?} failed with exit code {:?}:\n{}",
                args,
                output.status.code(),
                stderr
            );
        }

        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Execute a jj command, returning Result instead of panicking.
    ///
    /// Use this when testing error cases or when failure is expected.
    pub fn jj_result(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("jj")
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("Failed to execute jj command");

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    /// Write a file in the repository.
    pub fn write_file(&self, name: &str, content: &str) {
        let path = self.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create parent directories");
        }
        std::fs::write(&path, content).expect("Failed to write file");
    }

    /// Read a file from the repository.
    ///
    /// Returns an empty string if the file does not exist.
    pub fn read_file(&self, name: &str) -> String {
        std::fs::read_to_string(self.path().join(name)).unwrap_or_default()
    }

    /// Get the current change ID (short form, 8 characters).
    pub fn current_change_id(&self) -> String {
        self.jj(&["log", "-r", "@", "--no-graph", "-T", "change_id.short(8)"])
            .trim()
            .to_string()
    }

    /// Get the description of a revision.
    pub fn get_description(&self, rev: &str) -> String {
        self.jj(&["log", "-r", rev, "--no-graph", "-T", "description"])
            .trim()
            .to_string()
    }

    /// Count the number of changes matching a revset.
    pub fn count_changes(&self, revset: &str) -> usize {
        self.jj(&["log", "-r", revset, "--no-graph", "-T", "\"x\""])
            .matches('x')
            .count()
    }

    /// Check if a bookmark exists.
    pub fn bookmark_exists(&self, name: &str) -> bool {
        self.jj(&["bookmark", "list"]).contains(name)
    }

    /// Add a remote to this repository.
    pub fn add_remote(&self, name: &str, url: &str) {
        self.jj(&["git", "remote", "add", name, url]);
    }
}

impl Default for TestRepo {
    fn default() -> Self {
        Self::new()
    }
}
