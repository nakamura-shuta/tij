//! Interactive jj command methods
//!
//! These methods spawn jj as a child process with inherited stdio,
//! requiring the caller to disable raw mode before invocation.
//! Separated from executor.rs because they have fundamentally different
//! I/O patterns (Stdio::inherit vs captured output).

use std::io;
use std::process::{Command, ExitStatus, Stdio};

use super::constants::{self, commands, flags};
use super::executor::JjExecutor;

impl JjExecutor {
    /// Run `jj squash --from <source> --into <destination>` interactively
    ///
    /// Moves changes from the source revision into the destination.
    /// If the source becomes empty, it is automatically abandoned.
    ///
    /// Uses inherited stdio because jj may open an editor when both
    /// source and destination have non-empty descriptions.
    /// The caller must disable raw mode before calling this method.
    pub fn squash_into_interactive(
        &self,
        source: &str,
        destination: &str,
    ) -> io::Result<ExitStatus> {
        let mut cmd = Command::new(constants::JJ_COMMAND);

        if let Some(repo_path) = self.repo_path() {
            cmd.arg(flags::REPO_PATH).arg(repo_path);
        }

        cmd.args([commands::SQUASH, "--from", source, "--into", destination])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
    }

    /// Run `jj describe -r <change-id> --edit` interactively
    ///
    /// This spawns jj as a child process with inherited stdio,
    /// allowing the user to interact with their configured editor.
    /// The caller must disable raw mode before calling this method.
    ///
    /// Note: Unlike `run()`, this method does NOT use `--color=never`
    /// because interactive mode benefits from the editor's native behavior.
    pub fn describe_edit_interactive(&self, change_id: &str) -> io::Result<ExitStatus> {
        let mut cmd = Command::new(constants::JJ_COMMAND);

        if let Some(repo_path) = self.repo_path() {
            cmd.arg(flags::REPO_PATH).arg(repo_path);
        }

        cmd.args([commands::DESCRIBE, "-r", change_id, flags::EDIT_FLAG])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
    }

    /// Run `jj split -r <change-id>` interactively
    ///
    /// This spawns jj as a child process with inherited stdio,
    /// allowing the user to interact with their configured diff editor.
    /// The caller must disable raw mode before calling this method.
    ///
    /// Note: Unlike `run()`, this method does NOT use `--color=never`
    /// because interactive mode benefits from color output in the diff editor.
    pub fn split_interactive(&self, change_id: &str) -> io::Result<ExitStatus> {
        let mut cmd = Command::new(constants::JJ_COMMAND);

        // repo_path がある場合は -R を付与（tij /path/to/repo 対応）
        if let Some(repo_path) = self.repo_path() {
            cmd.arg(flags::REPO_PATH).arg(repo_path);
        }

        cmd.args([commands::SPLIT, "-r", change_id])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
    }

    /// Resolve a conflict interactively using an external merge tool
    ///
    /// Spawns jj resolve as a child process with inherited stdio.
    /// The caller must disable raw mode before calling this method.
    /// Only works for @ (working copy).
    pub fn resolve_interactive(
        &self,
        file_path: &str,
        change_id: Option<&str>,
    ) -> io::Result<ExitStatus> {
        let mut cmd = Command::new(constants::JJ_COMMAND);

        if let Some(repo_path) = self.repo_path() {
            cmd.arg(flags::REPO_PATH).arg(repo_path);
        }

        let mut args = vec![commands::RESOLVE];
        if let Some(rev) = change_id {
            args.push(flags::REVISION);
            args.push(rev);
        }
        args.push(file_path);

        cmd.args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
    }
}
