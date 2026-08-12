//! Interactive jj command methods
//!
//! These methods spawn jj as a child process with inherited stdio,
//! requiring the caller to disable raw mode before invocation.
//! Separated from executor.rs because they have fundamentally different
//! I/O patterns (Stdio::inherit vs captured output).
//!
//! Each command has an `*_argv()` builder used by BOTH the spawn and the
//! App's history record (command transparency P1), so the recorded command
//! line can never drift from what actually ran — pasting a `y`-copied line
//! into a terminal re-runs exactly this invocation.
//!
//! Note: unlike `run()`, interactive commands do NOT pass `--color=never` —
//! editors and diff editors benefit from their native color behavior.

use std::io;
use std::process::{Command, ExitStatus, Stdio};

use super::constants::{self, commands, flags};
use super::executor::JjExecutor;

impl JjExecutor {
    /// Shared prefix for every interactive argv: `-R <path>` when set.
    fn interactive_argv_base(&self) -> Vec<String> {
        let mut v = Vec::new();
        if let Some(repo_path) = self.repo_path() {
            v.push(flags::REPO_PATH.to_string());
            v.push(repo_path.display().to_string());
        }
        v
    }

    /// Spawn jj with inherited stdio for the given argv.
    /// The caller must disable raw mode (suspend the TUI) first.
    fn spawn_interactive(&self, argv: &[String]) -> io::Result<ExitStatus> {
        Command::new(constants::JJ_COMMAND)
            .args(argv)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
    }

    /// argv for [`Self::squash_into_interactive`]
    pub fn squash_into_argv(&self, source: &str, destination: &str) -> Vec<String> {
        let mut v = self.interactive_argv_base();
        v.extend(
            [commands::SQUASH, "--from", source, "--into", destination]
                .iter()
                .map(|s| s.to_string()),
        );
        v
    }

    /// Run `jj squash --from <source> --into <destination>` interactively
    ///
    /// Moves changes from the source revision into the destination.
    /// If the source becomes empty, it is automatically abandoned.
    ///
    /// Uses inherited stdio because jj may open an editor when both
    /// source and destination have non-empty descriptions.
    pub fn squash_into_interactive(
        &self,
        source: &str,
        destination: &str,
    ) -> io::Result<ExitStatus> {
        self.spawn_interactive(&self.squash_into_argv(source, destination))
    }

    /// argv for [`Self::describe_edit_interactive`]
    pub fn describe_edit_argv(&self, revision: &str) -> Vec<String> {
        let mut v = self.interactive_argv_base();
        v.extend(
            [commands::DESCRIBE, "-r", revision, flags::EDITOR_FLAG]
                .iter()
                .map(|s| s.to_string()),
        );
        v
    }

    /// Run `jj describe -r <change-id> --edit` interactively
    ///
    /// This spawns jj as a child process with inherited stdio,
    /// allowing the user to interact with their configured editor.
    pub fn describe_edit_interactive(&self, revision: &str) -> io::Result<ExitStatus> {
        self.spawn_interactive(&self.describe_edit_argv(revision))
    }

    /// argv for [`Self::split_interactive`]
    pub fn split_argv(&self, revision: &str) -> Vec<String> {
        let mut v = self.interactive_argv_base();
        v.extend(
            [commands::SPLIT, "-r", revision]
                .iter()
                .map(|s| s.to_string()),
        );
        v
    }

    /// Run `jj split -r <change-id>` interactively
    ///
    /// This spawns jj as a child process with inherited stdio,
    /// allowing the user to interact with their configured diff editor.
    pub fn split_interactive(&self, revision: &str) -> io::Result<ExitStatus> {
        self.spawn_interactive(&self.split_argv(revision))
    }

    /// argv for [`Self::diffedit_interactive`]
    pub fn diffedit_argv(&self, revision: &str) -> Vec<String> {
        let mut v = self.interactive_argv_base();
        v.extend(
            [commands::DIFFEDIT, flags::REVISION, revision]
                .iter()
                .map(|s| s.to_string()),
        );
        v
    }

    /// Run `jj diffedit -r <revision>` interactively
    ///
    /// Opens the configured diff editor to edit the changes in a revision.
    pub fn diffedit_interactive(&self, revision: &str) -> io::Result<ExitStatus> {
        self.spawn_interactive(&self.diffedit_argv(revision))
    }

    /// argv for [`Self::diffedit_file_interactive`]
    pub fn diffedit_file_argv(&self, revision: &str, file: &str) -> Vec<String> {
        let mut v = self.interactive_argv_base();
        v.extend(
            [commands::DIFFEDIT, flags::REVISION, revision, file]
                .iter()
                .map(|s| s.to_string()),
        );
        v
    }

    /// Run `jj diffedit -r <revision> <file>` interactively
    ///
    /// Opens the configured diff editor for a specific file in a revision.
    pub fn diffedit_file_interactive(&self, revision: &str, file: &str) -> io::Result<ExitStatus> {
        self.spawn_interactive(&self.diffedit_file_argv(revision, file))
    }

    /// argv for [`Self::resolve_interactive`]
    pub fn resolve_argv(&self, file_path: &str, revision: Option<&str>) -> Vec<String> {
        let mut v = self.interactive_argv_base();
        v.push(commands::RESOLVE.to_string());
        if let Some(rev) = revision {
            v.push(flags::REVISION.to_string());
            v.push(rev.to_string());
        }
        v.push(file_path.to_string());
        v
    }

    /// Resolve a conflict interactively using an external merge tool
    ///
    /// Spawns jj resolve as a child process with inherited stdio.
    /// Only works for @ (working copy).
    pub fn resolve_interactive(
        &self,
        file_path: &str,
        revision: Option<&str>,
    ) -> io::Result<ExitStatus> {
        self.spawn_interactive(&self.resolve_argv(file_path, revision))
    }

    /// argv for [`Self::arrange_interactive`]
    pub fn arrange_argv(&self, revset: Option<&str>) -> Vec<String> {
        let mut v = self.interactive_argv_base();
        v.push(commands::ARRANGE.to_string());
        if let Some(rev) = revset {
            v.push(flags::REVISION.to_string());
            v.push(rev.to_string());
        }
        v
    }

    /// Run `jj arrange` interactively
    ///
    /// Opens the built-in TUI for interactively rearranging the commit graph.
    /// Available since jj 0.40.0.
    pub fn arrange_interactive(&self, revset: Option<&str>) -> io::Result<ExitStatus> {
        self.spawn_interactive(&self.arrange_argv(revset))
    }

    /// argv for [`Self::bisect_run_interactive`]
    pub fn bisect_run_argv(&self, good: &str, bad: &str, command: &str) -> Vec<String> {
        let mut v = self.interactive_argv_base();
        let range = format!("{}..{}", good, bad);
        v.extend(
            [
                commands::BISECT,
                commands::BISECT_RUN,
                "--range",
                range.as_str(),
                "--",
                "bash",
                "-c",
                command,
            ]
            .iter()
            .map(|s| s.to_string()),
        );
        v
    }

    /// Run `jj bisect run --range <good>..<bad> -- bash -c <command>` interactively
    ///
    /// Spawns jj bisect as a child process with inherited stdio.
    pub fn bisect_run_interactive(
        &self,
        good: &str,
        bad: &str,
        command: &str,
    ) -> io::Result<ExitStatus> {
        self.spawn_interactive(&self.bisect_run_argv(good, bad, command))
    }

    /// argv for [`Self::run_interactive`]
    pub fn run_argv(&self, revset: &str, command: &str) -> Vec<String> {
        let mut v = self.interactive_argv_base();
        v.extend(
            [
                commands::RUN,
                flags::REVISION,
                revset,
                "--",
                "bash",
                "-c",
                command,
            ]
            .iter()
            .map(|s| s.to_string()),
        );
        v
    }

    /// Run `jj run -r <revset> -- bash -c <command>` interactively
    ///
    /// Spawns jj run as a child process with inherited stdio. Used to apply a
    /// shell command across the given revset (e.g. reformat, regenerate files).
    pub fn run_interactive(&self, revset: &str, command: &str) -> io::Result<ExitStatus> {
        self.spawn_interactive(&self.run_argv(revset, command))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The builders are the single source for interactive argv — these pin
    /// the exact command lines so the history record (and `y` copy) can be
    /// trusted to match the real spawn.
    #[test]
    fn interactive_argv_builders_produce_real_command_lines() {
        let jj = JjExecutor::new();
        assert_eq!(
            jj.describe_edit_argv("abc123"),
            ["describe", "-r", "abc123", "--editor"]
        );
        assert_eq!(jj.split_argv("abc123"), ["split", "-r", "abc123"]);
        assert_eq!(
            jj.squash_into_argv("src1", "dst2"),
            ["squash", "--from", "src1", "--into", "dst2"]
        );
        assert_eq!(jj.diffedit_argv("abc"), ["diffedit", "-r", "abc"]);
        assert_eq!(
            jj.diffedit_file_argv("abc", "src/main.rs"),
            ["diffedit", "-r", "abc", "src/main.rs"]
        );
        assert_eq!(
            jj.resolve_argv("file.txt", Some("abc")),
            ["resolve", "-r", "abc", "file.txt"]
        );
        assert_eq!(jj.resolve_argv("file.txt", None), ["resolve", "file.txt"]);
        assert_eq!(
            jj.arrange_argv(Some("main..@")),
            ["arrange", "-r", "main..@"]
        );
        assert_eq!(
            jj.bisect_run_argv("good1", "bad2", "cargo test"),
            [
                "bisect",
                "run",
                "--range",
                "good1..bad2",
                "--",
                "bash",
                "-c",
                "cargo test"
            ]
        );
        assert_eq!(
            jj.run_argv("mutable()", "cargo test"),
            ["run", "-r", "mutable()", "--", "bash", "-c", "cargo test"]
        );
    }

    #[test]
    fn interactive_argv_includes_repo_path_when_set() {
        let jj = JjExecutor::with_repo_path(PathBuf::from("/tmp/repo"));
        assert_eq!(
            jj.split_argv("abc"),
            ["-R", "/tmp/repo", "split", "-r", "abc"]
        );
        assert_eq!(
            jj.run_argv("main", "true"),
            [
                "-R",
                "/tmp/repo",
                "run",
                "-r",
                "main",
                "--",
                "bash",
                "-c",
                "true"
            ]
        );
    }
}
