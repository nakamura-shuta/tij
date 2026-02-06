//! RemoteRepo helper for Git remote testing.
//!
//! Provides a bare Git repository to simulate a remote server.

use std::process::Command;
use tempfile::TempDir;

use super::TestRepo;

/// A bare Git repository for use as a remote in tests.
///
/// The repository is automatically cleaned up when the RemoteRepo is dropped.
pub struct RemoteRepo {
    dir: TempDir,
}

impl RemoteRepo {
    /// Create a new bare Git repository.
    pub fn new_bare() -> Self {
        let dir = TempDir::new().expect("Failed to create temp directory");

        let output = Command::new("git")
            .args(["init", "--bare"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to execute git init --bare");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("git init --bare failed: {}", stderr);
        }

        Self { dir }
    }

    /// Get the URL (path) of this remote repository.
    pub fn url(&self) -> String {
        self.dir.path().to_string_lossy().into_owned()
    }
}

impl TestRepo {
    /// Create a new TestRepo with an origin remote already configured.
    pub fn with_remote(remote: &RemoteRepo) -> Self {
        let repo = Self::new();
        repo.add_remote("origin", &remote.url());
        repo
    }
}
