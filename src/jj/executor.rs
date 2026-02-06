//! jj command executor
//!
//! Handles running jj commands and capturing their output.

use std::io;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

use crate::model::{
    AnnotationContent, Bookmark, Change, ConflictFile, DiffContent, Operation, Status,
};

use super::JjError;
use super::constants::{self, commands, errors, flags, resolve_flags};
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
    #[allow(dead_code)]
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

    /// Get the full description (multi-line) for a change
    ///
    /// Uses `jj log -r <change-id> -T 'description'` to fetch the complete description.
    /// Unlike the normal log output which uses `description.first_line()`, this returns
    /// the entire description including all lines.
    pub fn get_description(&self, change_id: &str) -> Result<String, JjError> {
        let output = self.run(&[
            commands::LOG,
            flags::NO_GRAPH,
            flags::REVISION,
            change_id,
            flags::TEMPLATE,
            "description",
        ])?;
        Ok(output)
    }

    /// Run `jj edit` to set working-copy revision
    pub fn edit(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[commands::EDIT, change_id])
    }

    /// Run `jj new` to create a new empty change
    pub fn new_change(&self) -> Result<String, JjError> {
        self.run(&[commands::NEW])
    }

    /// Run `jj new <revision>` to create a new change with specified parent
    ///
    /// Creates a new empty change as a child of the specified revision.
    /// The working copy (@) moves to the new change.
    pub fn new_change_from(&self, revision: &str) -> Result<String, JjError> {
        self.run(&[commands::NEW, revision])
    }

    /// Run `jj commit` to commit current changes with a message
    ///
    /// This is equivalent to `jj describe` + `jj new`, but atomic.
    /// After commit, a new empty change is created on top.
    pub fn commit(&self, message: &str) -> Result<String, JjError> {
        self.run(&[commands::COMMIT, "-m", message])
    }

    /// Run `jj squash -r <change-id>` interactively
    ///
    /// Moves changes from the specified revision into its parent.
    /// If the source becomes empty, it is automatically abandoned.
    ///
    /// Uses inherited stdio because jj may open an editor when both
    /// source and destination have non-empty descriptions.
    /// The caller must disable raw mode before calling this method.
    pub fn squash_interactive(&self, change_id: &str) -> io::Result<ExitStatus> {
        let mut cmd = Command::new(constants::JJ_COMMAND);

        if let Some(ref repo_path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(repo_path);
        }

        cmd.args([commands::SQUASH, "-r", change_id])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
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
        if let Some(ref repo_path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(repo_path);
        }

        cmd.args([commands::SPLIT, "-r", change_id])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
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
            flags::NO_GRAPH,
            flags::TEMPLATE,
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

    /// Run `jj op log` and parse the output into Operations
    ///
    /// Returns a list of operations, most recent first.
    /// The first operation in the list is the current operation.
    pub fn op_log(&self, limit: Option<usize>) -> Result<Vec<Operation>, JjError> {
        let template = Templates::op_log();
        let mut args = vec![
            commands::OP,
            commands::OP_LOG,
            flags::NO_GRAPH,
            flags::TEMPLATE,
            template,
        ];

        // Convert limit to String and store it
        let limit_str;
        if let Some(n) = limit {
            limit_str = n.to_string();
            args.push("--limit");
            args.push(&limit_str);
        }

        let output = self.run(&args)?;
        Parser::parse_op_log(&output)
    }

    /// Run `jj op restore <operation_id>` to restore a previous state
    ///
    /// This restores the repository state to what it was after the specified operation.
    /// Use with caution - this is a powerful operation.
    pub fn op_restore(&self, operation_id: &str) -> Result<String, JjError> {
        self.run(&[commands::OP, commands::OP_RESTORE, operation_id])
    }

    /// Run `jj bookmark create <name> -r <change-id>` to create a bookmark
    ///
    /// Creates a new bookmark pointing to the specified change.
    /// Returns an error if a bookmark with the same name already exists.
    pub fn bookmark_create(&self, name: &str, change_id: &str) -> Result<String, JjError> {
        self.run(&[
            commands::BOOKMARK,
            commands::BOOKMARK_CREATE,
            name,
            "-r",
            change_id,
        ])
    }

    /// Run `jj bookmark set <name> -r <change-id> --allow-backwards` to move an existing bookmark
    ///
    /// Moves an existing bookmark to point to the specified change.
    /// Uses `--allow-backwards` to permit moving in any direction.
    pub fn bookmark_set(&self, name: &str, change_id: &str) -> Result<String, JjError> {
        self.run(&[
            commands::BOOKMARK,
            commands::BOOKMARK_SET,
            name,
            "-r",
            change_id,
            "--allow-backwards",
        ])
    }

    /// Run `jj bookmark delete <names>...` to delete bookmarks
    ///
    /// Deletes the specified bookmarks. Deletions propagate to remotes on push.
    pub fn bookmark_delete(&self, names: &[&str]) -> Result<String, JjError> {
        let mut args = vec![commands::BOOKMARK, commands::BOOKMARK_DELETE];
        args.extend(names);
        self.run(&args)
    }

    /// Run `jj bookmark list --all-remotes` to get all bookmarks
    ///
    /// Returns both local and remote bookmarks with their tracking status.
    /// Uses a template to output: name, remote, tracked (tab-separated).
    ///
    /// Note: Requires jj 0.37+ which supports the `tracked` template field.
    pub fn bookmark_list_all(&self) -> Result<Vec<Bookmark>, JjError> {
        const BOOKMARK_LIST_TEMPLATE: &str = r#"separate("\t", name, remote, tracked) ++ "\n""#;

        let output = self.run(&[
            commands::BOOKMARK,
            commands::BOOKMARK_LIST,
            flags::ALL_REMOTES,
            flags::TEMPLATE,
            BOOKMARK_LIST_TEMPLATE,
        ])?;
        Ok(super::parser::parse_bookmark_list(&output))
    }

    /// Run `jj bookmark track <names>...` to start tracking remote bookmarks
    ///
    /// Starts tracking the specified remote bookmarks locally.
    /// After tracking, `jj git fetch` will update the local copy.
    ///
    /// Format: `<name>@<remote>` (e.g., "feature-x@origin")
    pub fn bookmark_track(&self, names: &[&str]) -> Result<String, JjError> {
        let mut args = vec![commands::BOOKMARK, commands::BOOKMARK_TRACK];
        args.extend(names);
        self.run(&args)
    }

    /// Run `jj rebase -r <source> -d <destination>` to move a change
    ///
    /// Moves the specified change to be a child of the destination.
    /// Unlike `-s` option, this only moves the single change; descendants
    /// are rebased onto the original parent.
    ///
    /// Returns the command output which may contain conflict information.
    pub fn rebase(&self, source: &str, destination: &str) -> Result<String, JjError> {
        self.run(&[commands::REBASE, flags::REVISION, source, "-d", destination])
    }

    /// Check if a specific change has conflicts
    ///
    /// Uses `jj log -r <change_id> -T 'conflict'` to query the conflict status.
    /// Returns true if the change has unresolved conflicts.
    pub fn has_conflict(&self, change_id: &str) -> Result<bool, JjError> {
        let output = self.run(&[
            commands::LOG,
            flags::NO_GRAPH,
            flags::REVISION,
            change_id,
            flags::TEMPLATE,
            "conflict",
        ])?;
        Ok(output.trim() == "true")
    }

    /// Run `jj absorb` to move changes into ancestor commits
    ///
    /// Each hunk in the working copy (@) is moved to the closest mutable
    /// ancestor where the corresponding lines were modified last.
    /// If the destination cannot be determined unambiguously, the change
    /// remains in the source.
    ///
    /// Returns the command output which describes what was absorbed.
    pub fn absorb(&self) -> Result<String, JjError> {
        self.run(&[commands::ABSORB])
    }

    /// List conflicted files for a change
    ///
    /// Runs `jj resolve --list [-r <change_id>]` and parses the output.
    /// Returns an empty list if there are no conflicts.
    pub fn resolve_list(&self, change_id: Option<&str>) -> Result<Vec<ConflictFile>, JjError> {
        let mut args = vec![commands::RESOLVE, resolve_flags::LIST];

        if let Some(rev) = change_id {
            args.push(flags::REVISION);
            args.push(rev);
        }

        let output = self.run(&args)?;
        Ok(Parser::parse_resolve_list(&output))
    }

    /// Resolve a conflict using a built-in tool (:ours or :theirs)
    ///
    /// Works for any change (not just @).
    pub fn resolve_with_tool(
        &self,
        file_path: &str,
        tool: &str,
        change_id: Option<&str>,
    ) -> Result<String, JjError> {
        let mut args = vec![commands::RESOLVE, resolve_flags::TOOL, tool];

        if let Some(rev) = change_id {
            args.push(flags::REVISION);
            args.push(rev);
        }

        args.push(file_path);
        self.run(&args)
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

        if let Some(ref repo_path) = self.repo_path {
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

    /// Run `jj git fetch` to fetch from all tracked remotes
    ///
    /// Returns the command output describing what was fetched.
    /// Empty output typically means "already up to date".
    pub fn git_fetch(&self) -> Result<String, JjError> {
        self.run(&[commands::GIT, commands::GIT_FETCH])
    }

    /// Run `jj git push --bookmark <name>` to push a bookmark to remote
    ///
    /// Pushes the specified bookmark to the default remote (origin).
    /// jj automatically performs force-with-lease equivalent safety checks.
    pub fn git_push_bookmark(&self, bookmark_name: &str) -> Result<String, JjError> {
        self.run(&[
            commands::GIT,
            commands::GIT_PUSH,
            flags::BOOKMARK_FLAG,
            bookmark_name,
        ])
    }

    /// Run `jj git push --bookmark <name> --allow-new` for new remote bookmarks
    ///
    /// Same as git_push_bookmark but allows creating new remote bookmarks.
    /// Note: --allow-new is deprecated in jj 0.37+ but still functional.
    /// Users should configure `remotes.origin.auto-track-bookmarks` for a permanent fix.
    pub fn git_push_bookmark_allow_new(&self, bookmark_name: &str) -> Result<String, JjError> {
        self.run(&[
            commands::GIT,
            commands::GIT_PUSH,
            flags::BOOKMARK_FLAG,
            bookmark_name,
            flags::ALLOW_NEW,
        ])
    }

    /// Run `jj file annotate` to show blame information for a file
    ///
    /// Shows the change responsible for each line of the specified file.
    /// Optionally annotates at a specific revision.
    ///
    /// Returns AnnotationContent containing line-by-line blame information.
    ///
    /// Note: Uses default output format (no custom template) for jj 0.37.x compatibility.
    /// The AnnotationLine template methods are only available in jj 0.38+.
    pub fn file_annotate(
        &self,
        file_path: &str,
        revision: Option<&str>,
    ) -> Result<AnnotationContent, JjError> {
        let mut args = vec![commands::FILE, commands::FILE_ANNOTATE];

        if let Some(rev) = revision {
            args.push(flags::REVISION);
            args.push(rev);
        }

        args.push(file_path);

        let output = self.run(&args)?;
        Parser::parse_file_annotate(&output, file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
