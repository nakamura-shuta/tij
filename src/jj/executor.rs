//! jj command executor
//!
//! Handles running jj commands and capturing their output.
//!
//! ## Concurrency rules for jj command execution
//!
//! - **Read-Read**: Safe to parallelize (`jj log` + `jj status` + `jj op log`, etc.)
//! - **Write → Read**: Must be sequential (action must complete before refresh reads its result)
//! - **Write-Write**: Must be sequential (never parallelize two write operations)
//! - **Result consistency**: When parallel reads complete, apply all results to App state
//!   atomically to avoid partial/inconsistent UI state.

use std::path::PathBuf;
use std::process::Command;

use crate::model::{
    AnnotationContent, Bookmark, BookmarkInfo, Change, ConflictFile, DiffContent, Operation, Status,
};

use super::JjError;
use super::constants::{self, commands, errors, flags, resolve_flags};
use super::parser::Parser;
use super::template::Templates;

/// Bulk push mode (repository-wide push operations)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushBulkMode {
    /// Push all bookmarks (including new) — `--all`
    All,
    /// Push tracked bookmarks only — `--tracked`
    Tracked,
    /// Push deleted bookmarks — `--deleted`
    Deleted,
}

impl PushBulkMode {
    /// Return the jj CLI flag for this mode
    pub fn flag(&self) -> &'static str {
        match self {
            Self::All => flags::ALL,
            Self::Tracked => flags::TRACKED,
            Self::Deleted => flags::DELETED,
        }
    }

    /// Human-readable label for UI
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "all bookmarks",
            Self::Tracked => "tracked bookmarks",
            Self::Deleted => "deleted bookmarks",
        }
    }
}

/// Executor for jj commands
///
/// All methods take `&self` (no mutable state), making `JjExecutor` safe to
/// share across threads via `&JjExecutor`. This is verified by the compile-time
/// assertion below.
#[derive(Debug, Clone)]
pub struct JjExecutor {
    /// Path to the repository (None = current directory)
    repo_path: Option<PathBuf>,
}

// Compile-time assertion: JjExecutor must be Sync for thread::scope sharing.
// If this fails, consider wrapping in Arc or removing interior mutability.
const _: () = {
    const fn assert_sync<T: Sync>() {}
    assert_sync::<JjExecutor>();
};

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

    /// Get the repository path (for use by other impl blocks in sibling modules)
    pub(crate) fn repo_path(&self) -> Option<&PathBuf> {
        self.repo_path.as_ref()
    }

    /// Run a jj command with the given arguments
    ///
    /// Automatically adds `--color=never` to ensure parseable output.
    pub fn run(&self, args: &[&str]) -> Result<String, JjError> {
        use std::process::Stdio;

        let mut cmd = Command::new(constants::JJ_COMMAND);

        // Add repository path if specified
        if let Some(ref path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(path);
        }

        // Always disable color for parsing
        cmd.arg(flags::NO_COLOR);

        // Add user-specified arguments
        cmd.args(args);

        // Explicitly close stdin to prevent jj from waiting for input
        // (e.g., during snapshot warnings or interactive prompts).
        // Without this, Command::output() creates a pipe for stdin,
        // which may not signal EOF properly under raw-mode terminals.
        cmd.stdin(Stdio::null());

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

            // jj exits with code 1 when snapshot warnings are present
            // (e.g., large files exceeding snapshot.max-new-file-size)
            // but still may produce valid stdout.
            //
            // Most commands require non-empty stdout to treat this as success.
            // `jj show` is a special case: empty commits legitimately produce
            // empty stdout (no diff lines), so allow empty stdout only for show.
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let is_show = args.first().is_some_and(|a| *a == commands::SHOW);
            if stderr.contains("Refused to snapshot") && (!stdout.is_empty() || is_show) {
                return Ok(stdout);
            }

            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Run `jj log` with optional revset filter (raw output)
    ///
    /// Note: Graph output is enabled to show DAG structure.
    /// The parser handles graph prefixes in the output.
    pub fn log_raw(&self, revset: Option<&str>, reversed: bool) -> Result<String, JjError> {
        let template = Templates::log();
        let mut args = vec![commands::LOG, flags::TEMPLATE, template];

        if let Some(rev) = revset {
            args.push(flags::REVISION);
            args.push(rev);
            // No --limit for revset queries (preserve exploration)
        } else {
            // Default view: limit to avoid slowness on large repos
            args.push(flags::LIMIT);
            args.push(constants::DEFAULT_LOG_LIMIT);
        }

        if reversed {
            args.push(flags::REVERSED);
        }

        self.run(&args)
    }

    /// Run `jj log` and parse the output into Changes
    pub fn log(&self, revset: Option<&str>, reversed: bool) -> Result<Vec<Change>, JjError> {
        let output = self.log_raw(revset, reversed)?;
        Parser::parse_log(&output).map_err(|e| JjError::ParseError(e.to_string()))
    }

    /// Run `jj log` and parse output into Changes for current view.
    /// This is the preferred API for application code.
    pub fn log_changes(
        &self,
        revset: Option<&str>,
        reversed: bool,
    ) -> Result<Vec<Change>, JjError> {
        self.log(revset, reversed)
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

    /// Run `jj squash` to squash @ into @- (non-interactive)
    ///
    /// Moves changes from the current working copy into its parent.
    /// If the current change becomes empty, it is automatically abandoned.
    /// This uses `--use-destination-message` to avoid opening an editor.
    pub fn squash(&self) -> Result<String, JjError> {
        self.run(&[commands::SQUASH, "--use-destination-message"])
    }

    /// Run `jj abandon <change-id>` to abandon a revision
    ///
    /// Descendants are automatically rebased onto the parent.
    /// If @ is abandoned, a new empty change is created.
    pub fn abandon(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[commands::ABANDON, change_id])
    }

    /// Run `jj revert -r <change_id> --onto @` to create a reverse-diff commit
    ///
    /// Creates a new commit on top of @ that undoes the changes from the specified revision.
    pub fn revert(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[
            commands::REVERT,
            flags::REVISION,
            change_id,
            flags::ONTO,
            "@",
        ])
    }

    /// Run `jj restore <file_path>` to restore a specific file to its parent state
    pub fn restore_file(&self, file_path: &str) -> Result<String, JjError> {
        self.run(&[commands::RESTORE, file_path])
    }

    /// Run `jj restore` to restore all files to their parent state
    pub fn restore_all(&self) -> Result<String, JjError> {
        self.run(&[commands::RESTORE])
    }

    /// Run `jj evolog -r <change_id>` with template output
    pub fn evolog(&self, change_id: &str) -> Result<String, JjError> {
        // evolog template context is EvolutionEntry, not Commit.
        // Fields must be accessed via `commit.` prefix (e.g. commit.commit_id()).
        // Uses committer timestamp (when each version was created), not author
        // timestamp (which stays the same across all versions).
        let template = concat!(
            "separate(\"\\t\",",
            "  commit.commit_id().short(),",
            "  commit.change_id().short(),",
            "  commit.author().email(),",
            "  commit.committer().timestamp().format(\"%Y-%m-%d %H:%M:%S\"),",
            "  if(commit.empty(), \"[empty]\", \"\"),",
            "  if(commit.description(), commit.description().first_line(), \"(no description set)\")",
            ") ++ \"\\n\""
        );
        self.run(&[
            commands::EVOLOG,
            flags::REVISION,
            change_id,
            flags::TEMPLATE,
            template,
        ])
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

    /// Get extended bookmark information for Bookmark Jump/View
    ///
    /// Two-stage approach:
    /// 1. `jj bookmark list --all-remotes` - get all bookmarks with tracking status
    /// 2. `jj log -r 'bookmarks()'` - get change_id and description for local bookmarks
    ///
    /// Remote-only bookmarks will have `change_id = None` and cannot be jumped to.
    /// Remote tracked bookmarks (e.g., main@origin) also have `change_id = None`
    /// to ensure only local bookmarks appear in Jump dialog.
    pub fn bookmark_list_with_info(&self) -> Result<Vec<BookmarkInfo>, JjError> {
        use std::collections::HashMap;

        // Step 1: Get all bookmarks
        let bookmarks = self.bookmark_list_all()?;

        // Step 2: Get revision info for local bookmarks
        // Template: explicitly format bookmarks as space-separated names
        // Using bookmarks.map(|x| x.name()).join(" ") for stable parsing
        // Use short(8) to match LogView's change_id length for exact matching
        const BOOKMARK_INFO_TEMPLATE: &str = r#"bookmarks.map(|x| x.name()).join(" ") ++ "\t" ++ change_id.short(8) ++ "\t" ++ commit_id.short(8) ++ "\t" ++ description.first_line() ++ "\n""#;

        let log_output = self.run(&[
            commands::LOG,
            flags::NO_GRAPH,
            flags::REVISION,
            "bookmarks()",
            flags::TEMPLATE,
            BOOKMARK_INFO_TEMPLATE,
        ])?;

        // Parse log output into a map: bookmark_name -> (change_id, commit_id, description)
        // Note: This only includes LOCAL bookmarks (from `jj log -r 'bookmarks()'`)
        let mut info_map: HashMap<String, (String, String, String)> = HashMap::new();
        for line in log_output.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                let bookmark_names = parts[0]; // Space-separated bookmark names
                let change_id = parts[1].to_string();
                let commit_id = parts[2].to_string();
                let description = parts[3].to_string();

                // Multiple bookmarks may point to the same commit
                for name in bookmark_names.split_whitespace() {
                    info_map.insert(
                        name.to_string(),
                        (change_id.clone(), commit_id.clone(), description.clone()),
                    );
                }
            }
        }

        // Step 3: Merge bookmark list with revision info
        // Only local bookmarks (remote.is_none()) get change_id from info_map
        // Remote bookmarks (including tracked ones like main@origin) get change_id = None
        // This ensures only local bookmarks appear in Jump dialog
        let result: Vec<BookmarkInfo> = bookmarks
            .into_iter()
            .map(|bookmark| {
                // Only apply info_map to local bookmarks
                let info = if bookmark.remote.is_none() {
                    info_map.get(&bookmark.name)
                } else {
                    None
                };
                BookmarkInfo {
                    change_id: info.map(|(c, _, _)| c.clone()),
                    commit_id: info.map(|(_, c, _)| c.clone()),
                    description: info.map(|(_, _, d)| d.clone()),
                    bookmark,
                }
            })
            .collect();

        Ok(result)
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

    /// Run `jj bookmark untrack <names...>` to stop tracking remote bookmarks
    ///
    /// Stops tracking the specified remote bookmarks locally.
    /// Format: `<name>@<remote>` (e.g., "feature-x@origin")
    pub fn bookmark_untrack(&self, names: &[&str]) -> Result<String, JjError> {
        let mut args = vec![commands::BOOKMARK, commands::BOOKMARK_UNTRACK];
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

    /// Run `jj rebase -s <source> -d <destination>` to move a change and its descendants
    ///
    /// Moves the specified change and all its descendants to be children of the destination.
    /// Unlike `-r`, this moves the entire subtree.
    ///
    /// Returns the command output which may contain conflict information.
    pub fn rebase_source(&self, source: &str, destination: &str) -> Result<String, JjError> {
        self.run(&[commands::REBASE, flags::SOURCE, source, "-d", destination])
    }

    /// Run `jj rebase -r <source> -A <target>` to insert a change after target
    ///
    /// Inserts the source change into the history after the target revision.
    /// The target's children become children of the source instead.
    ///
    /// Returns the command output which may contain conflict information.
    pub fn rebase_insert_after(&self, source: &str, target: &str) -> Result<String, JjError> {
        self.run(&[
            commands::REBASE,
            flags::REVISION,
            source,
            flags::INSERT_AFTER,
            target,
        ])
    }

    /// Run `jj rebase -r <source> -B <target>` to insert a change before target
    ///
    /// Inserts the source change into the history before the target revision.
    /// The source becomes a new parent of the target.
    ///
    /// Returns the command output which may contain conflict information.
    pub fn rebase_insert_before(&self, source: &str, target: &str) -> Result<String, JjError> {
        self.run(&[
            commands::REBASE,
            flags::REVISION,
            source,
            flags::INSERT_BEFORE,
            target,
        ])
    }

    /// Run `jj rebase -b <source> -d <destination>` to rebase a branch
    ///
    /// Moves all commits on the branch (relative to the destination's ancestors)
    /// to be children of the destination.
    pub fn rebase_branch(&self, source: &str, destination: &str) -> Result<String, JjError> {
        self.run(&[
            commands::REBASE,
            flags::BRANCH_SHORT,
            source,
            "-d",
            destination,
        ])
    }

    /// Run `jj rebase -r` with extra flags (e.g. --skip-emptied)
    pub fn rebase_with_flags(
        &self,
        source: &str,
        destination: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![commands::REBASE, flags::REVISION, source, "-d", destination];
        args.extend_from_slice(extra_flags);
        self.run(&args)
    }

    /// Run `jj rebase -s` with extra flags
    pub fn rebase_source_with_flags(
        &self,
        source: &str,
        destination: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![commands::REBASE, flags::SOURCE, source, "-d", destination];
        args.extend_from_slice(extra_flags);
        self.run(&args)
    }

    /// Run `jj rebase -b` with extra flags
    pub fn rebase_branch_with_flags(
        &self,
        source: &str,
        destination: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![
            commands::REBASE,
            flags::BRANCH_SHORT,
            source,
            "-d",
            destination,
        ];
        args.extend_from_slice(extra_flags);
        self.run(&args)
    }

    /// Run `jj rebase -r -A` with extra flags
    pub fn rebase_insert_after_with_flags(
        &self,
        source: &str,
        target: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![
            commands::REBASE,
            flags::REVISION,
            source,
            flags::INSERT_AFTER,
            target,
        ];
        args.extend_from_slice(extra_flags);
        self.run(&args)
    }

    /// Run `jj rebase -r -B` with extra flags
    pub fn rebase_insert_before_with_flags(
        &self,
        source: &str,
        target: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![
            commands::REBASE,
            flags::REVISION,
            source,
            flags::INSERT_BEFORE,
            target,
        ];
        args.extend_from_slice(extra_flags);
        self.run(&args)
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

    /// Rename a local bookmark
    ///
    /// Runs `jj bookmark rename <old> <new>`.
    /// Only works for local bookmarks. Remote bookmarks cannot be renamed.
    pub fn bookmark_rename(&self, old_name: &str, new_name: &str) -> Result<String, JjError> {
        self.run(&[
            commands::BOOKMARK,
            commands::BOOKMARK_RENAME,
            old_name,
            new_name,
        ])
    }

    /// Forget bookmarks (removes local and remote tracking info)
    ///
    /// Unlike `bookmark delete`, forget removes remote tracking information.
    /// The bookmark will NOT be re-created on the next `jj git fetch`.
    pub fn bookmark_forget(&self, names: &[&str]) -> Result<String, JjError> {
        let mut args = vec![commands::BOOKMARK, commands::BOOKMARK_FORGET];
        args.extend(names);
        self.run(&args)
    }

    /// Run `jj next --edit` to move @ to the next child
    pub fn next(&self) -> Result<String, JjError> {
        self.run(&[commands::NEXT, flags::EDIT_FLAG])
    }

    /// Run `jj prev --edit` to move @ to the previous parent
    pub fn prev(&self) -> Result<String, JjError> {
        self.run(&[commands::PREV, flags::EDIT_FLAG])
    }

    /// Run `jj duplicate <change_id>` to create a copy of the specified change
    ///
    /// Returns the jj stderr output containing the new change ID.
    /// Note: `jj duplicate` writes its result to stderr, not stdout.
    /// Output format: "Duplicated <commit_id> as <new_change_id> <new_commit_id> <description>"
    pub fn duplicate(&self, change_id: &str) -> Result<String, JjError> {
        let mut cmd = Command::new(constants::JJ_COMMAND);
        if let Some(ref path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(path);
        }
        cmd.arg(flags::NO_COLOR);
        cmd.args([commands::DUPLICATE, change_id]);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                JjError::JjNotFound
            } else {
                JjError::IoError(e)
            }
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stderr).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);
            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Run `jj git fetch` to fetch from default remotes
    ///
    /// Returns the command output describing what was fetched.
    /// Empty output typically means "already up to date".
    pub fn git_fetch(&self) -> Result<String, JjError> {
        self.run(&[commands::GIT, commands::GIT_FETCH])
    }

    /// Run `jj git fetch --all-remotes`
    pub fn git_fetch_all_remotes(&self) -> Result<String, JjError> {
        self.run(&[commands::GIT, commands::GIT_FETCH, flags::ALL_REMOTES])
    }

    /// Run `jj git fetch --remote <name>`
    pub fn git_fetch_remote(&self, remote: &str) -> Result<String, JjError> {
        self.run(&[commands::GIT, commands::GIT_FETCH, flags::REMOTE, remote])
    }

    /// Run `jj git remote list` to get all remote names
    pub fn git_remote_list(&self) -> Result<Vec<String>, JjError> {
        let output = self.run(&[
            commands::GIT,
            commands::GIT_REMOTE,
            commands::GIT_REMOTE_LIST,
        ])?;
        Ok(output
            .lines()
            .filter_map(|line| line.split_whitespace().next().map(|s| s.to_string()))
            .collect())
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

    /// Run `jj git push --dry-run --bookmark <name>` to preview push
    ///
    /// Returns the dry-run output describing what would change on the remote.
    /// Does NOT actually push anything.
    ///
    /// On success (exit 0), returns stderr which can be parsed with `parse_push_dry_run()`.
    /// On failure (exit != 0), returns `Err(JjError)` — e.g., untracked bookmark or
    /// empty description validation errors.
    pub fn git_push_dry_run(&self, bookmark_name: &str) -> Result<String, JjError> {
        // Note: `jj git push --dry-run` outputs to stderr, not stdout,
        // so we can't use the generic `run()` method here.
        let mut cmd = Command::new(constants::JJ_COMMAND);
        if let Some(ref path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(path);
        }
        cmd.arg(flags::NO_COLOR);
        cmd.args([
            commands::GIT,
            commands::GIT_PUSH,
            flags::DRY_RUN,
            flags::BOOKMARK_FLAG,
            bookmark_name,
        ]);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                JjError::JjNotFound
            } else {
                JjError::IoError(e)
            }
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stderr).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);
            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Run `jj git push --named <bookmark>=<revision>` for new remote bookmarks (jj 0.37+)
    ///
    /// This is the recommended way to push new bookmarks in jj 0.37+.
    /// The --named flag creates the bookmark if it doesn't exist, auto-tracks it,
    /// and pushes it in a single operation.
    pub fn git_push_named(&self, bookmark_name: &str, revision: &str) -> Result<String, JjError> {
        let named_arg = format!("{}={}", bookmark_name, revision);
        self.run(&[commands::GIT, commands::GIT_PUSH, flags::NAMED, &named_arg])
    }

    /// Run `jj git push --change <change_id>` to push by change ID
    ///
    /// Automatically creates a bookmark named `push-<change_id_prefix>`
    /// and pushes it to the remote. If the bookmark already exists, it
    /// reuses it.
    pub fn git_push_change(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[commands::GIT, commands::GIT_PUSH, flags::CHANGE, change_id])
    }

    /// Run `jj git push --bookmark <name> --remote <remote>` to push to specific remote
    pub fn git_push_bookmark_to_remote(
        &self,
        bookmark_name: &str,
        remote: &str,
    ) -> Result<String, JjError> {
        self.run(&[
            commands::GIT,
            commands::GIT_PUSH,
            flags::BOOKMARK_FLAG,
            bookmark_name,
            flags::REMOTE,
            remote,
        ])
    }

    /// Run `jj git push --bookmark <name> --allow-new --remote <remote>` for new remote bookmarks
    pub fn git_push_bookmark_allow_new_to_remote(
        &self,
        bookmark_name: &str,
        remote: &str,
    ) -> Result<String, JjError> {
        self.run(&[
            commands::GIT,
            commands::GIT_PUSH,
            flags::BOOKMARK_FLAG,
            bookmark_name,
            flags::ALLOW_NEW,
            flags::REMOTE,
            remote,
        ])
    }

    /// Run `jj git push --dry-run --bookmark <name> --remote <remote>` to preview push to specific remote
    pub fn git_push_dry_run_to_remote(
        &self,
        bookmark_name: &str,
        remote: &str,
    ) -> Result<String, JjError> {
        let mut cmd = Command::new(constants::JJ_COMMAND);
        if let Some(ref path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(path);
        }
        cmd.arg(flags::NO_COLOR);
        cmd.args([
            commands::GIT,
            commands::GIT_PUSH,
            flags::DRY_RUN,
            flags::BOOKMARK_FLAG,
            bookmark_name,
            flags::REMOTE,
            remote,
        ]);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                JjError::JjNotFound
            } else {
                JjError::IoError(e)
            }
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stderr).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);
            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Run `jj git push --change <change_id> --remote <remote>`
    pub fn git_push_change_to_remote(
        &self,
        change_id: &str,
        remote: &str,
    ) -> Result<String, JjError> {
        self.run(&[
            commands::GIT,
            commands::GIT_PUSH,
            flags::CHANGE,
            change_id,
            flags::REMOTE,
            remote,
        ])
    }

    /// Run `jj git push --change <change_id> --dry-run --remote <remote>` to preview push to specific remote
    pub fn git_push_change_dry_run_to_remote(
        &self,
        change_id: &str,
        remote: &str,
    ) -> Result<String, JjError> {
        let mut cmd = Command::new(constants::JJ_COMMAND);
        if let Some(ref path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(path);
        }
        cmd.arg(flags::NO_COLOR);
        cmd.args([
            commands::GIT,
            commands::GIT_PUSH,
            flags::DRY_RUN,
            flags::CHANGE,
            change_id,
            flags::REMOTE,
            remote,
        ]);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                JjError::JjNotFound
            } else {
                JjError::IoError(e)
            }
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stderr).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);
            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Run `jj git push --change <change_id> --dry-run` to preview push
    ///
    /// Returns stderr output describing what would change on the remote
    /// if this change were pushed. Does NOT actually push anything.
    pub fn git_push_change_dry_run(&self, change_id: &str) -> Result<String, JjError> {
        let mut cmd = Command::new(constants::JJ_COMMAND);
        if let Some(ref path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(path);
        }
        cmd.arg(flags::NO_COLOR);
        cmd.args([
            commands::GIT,
            commands::GIT_PUSH,
            flags::DRY_RUN,
            flags::CHANGE,
            change_id,
        ]);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                JjError::JjNotFound
            } else {
                JjError::IoError(e)
            }
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stderr).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);
            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Push with a bulk flag (--all, --tracked, --deleted)
    pub fn git_push_bulk(
        &self,
        mode: PushBulkMode,
        remote: Option<&str>,
    ) -> Result<String, JjError> {
        let mut args = vec![commands::GIT, commands::GIT_PUSH, mode.flag()];
        if let Some(r) = remote {
            args.extend([flags::REMOTE, r]);
        }
        self.run(&args)
    }

    /// Dry-run push with a bulk flag
    ///
    /// Returns stderr output (jj git push --dry-run outputs to stderr).
    pub fn git_push_bulk_dry_run(
        &self,
        mode: PushBulkMode,
        remote: Option<&str>,
    ) -> Result<String, JjError> {
        let mut cmd = Command::new(constants::JJ_COMMAND);
        if let Some(ref path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(path);
        }
        cmd.arg(flags::NO_COLOR);
        let mut args = vec![
            commands::GIT,
            commands::GIT_PUSH,
            flags::DRY_RUN,
            mode.flag(),
        ];
        if let Some(r) = remote {
            args.extend([flags::REMOTE, r]);
        }
        cmd.args(&args);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                JjError::JjNotFound
            } else {
                JjError::IoError(e)
            }
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stderr).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);
            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Run `jj git push --bookmark <name>` with extra flags (e.g. --allow-private)
    ///
    /// Used for retry after error detection.
    pub fn git_push_bookmark_with_flags(
        &self,
        bookmark_name: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![
            commands::GIT,
            commands::GIT_PUSH,
            flags::BOOKMARK_FLAG,
            bookmark_name,
        ];
        args.extend_from_slice(extra_flags);
        self.run(&args)
    }

    /// Run `jj git push --bookmark <name> --remote <remote>` with extra flags
    pub fn git_push_bookmark_to_remote_with_flags(
        &self,
        bookmark_name: &str,
        remote: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![
            commands::GIT,
            commands::GIT_PUSH,
            flags::BOOKMARK_FLAG,
            bookmark_name,
            flags::REMOTE,
            remote,
        ];
        args.extend_from_slice(extra_flags);
        self.run(&args)
    }

    /// Run `jj git push --change <change_id>` with extra flags
    pub fn git_push_change_with_flags(
        &self,
        change_id: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![commands::GIT, commands::GIT_PUSH, flags::CHANGE, change_id];
        args.extend_from_slice(extra_flags);
        self.run(&args)
    }

    /// Run `jj git push --change <change_id> --remote <remote>` with extra flags
    pub fn git_push_change_to_remote_with_flags(
        &self,
        change_id: &str,
        remote: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![
            commands::GIT,
            commands::GIT_PUSH,
            flags::CHANGE,
            change_id,
            flags::REMOTE,
            remote,
        ];
        args.extend_from_slice(extra_flags);
        self.run(&args)
    }

    /// Run `jj git push --revisions <change_id>` with extra flags
    pub fn git_push_revisions_with_flags(
        &self,
        change_id: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![
            commands::GIT,
            commands::GIT_PUSH,
            flags::REVISIONS,
            change_id,
        ];
        args.extend_from_slice(extra_flags);
        self.run(&args)
    }

    /// Run `jj git push --revisions <change_id> --remote <remote>` with extra flags
    pub fn git_push_revisions_to_remote_with_flags(
        &self,
        change_id: &str,
        remote: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![
            commands::GIT,
            commands::GIT_PUSH,
            flags::REVISIONS,
            change_id,
            flags::REMOTE,
            remote,
        ];
        args.extend_from_slice(extra_flags);
        self.run(&args)
    }

    /// Run `jj git fetch --branch <name>` to fetch a specific branch
    pub fn git_fetch_branch(&self, branch: &str) -> Result<String, JjError> {
        self.run(&[commands::GIT, commands::GIT_FETCH, flags::BRANCH, branch])
    }

    /// Run `jj git push --revisions <change_id>` to push all bookmarks on a revision
    pub fn git_push_revisions(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[
            commands::GIT,
            commands::GIT_PUSH,
            flags::REVISIONS,
            change_id,
        ])
    }

    /// Run `jj git push --revisions <change_id> --remote <remote>`
    pub fn git_push_revisions_to_remote(
        &self,
        change_id: &str,
        remote: &str,
    ) -> Result<String, JjError> {
        self.run(&[
            commands::GIT,
            commands::GIT_PUSH,
            flags::REVISIONS,
            change_id,
            flags::REMOTE,
            remote,
        ])
    }

    /// Run `jj git push --dry-run --revisions <change_id>` to preview push
    ///
    /// Returns stderr output (jj git push --dry-run outputs to stderr).
    pub fn git_push_revisions_dry_run(&self, change_id: &str) -> Result<String, JjError> {
        let mut cmd = Command::new(constants::JJ_COMMAND);
        if let Some(ref path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(path);
        }
        cmd.arg(flags::NO_COLOR);
        cmd.args([
            commands::GIT,
            commands::GIT_PUSH,
            flags::DRY_RUN,
            flags::REVISIONS,
            change_id,
        ]);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                JjError::JjNotFound
            } else {
                JjError::IoError(e)
            }
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stderr).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);
            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Run `jj git push --dry-run --revisions <change_id> --remote <remote>`
    pub fn git_push_revisions_dry_run_to_remote(
        &self,
        change_id: &str,
        remote: &str,
    ) -> Result<String, JjError> {
        let mut cmd = Command::new(constants::JJ_COMMAND);
        if let Some(ref path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(path);
        }
        cmd.arg(flags::NO_COLOR);
        cmd.args([
            commands::GIT,
            commands::GIT_PUSH,
            flags::DRY_RUN,
            flags::REVISIONS,
            change_id,
            flags::REMOTE,
            remote,
        ]);

        let output = cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                JjError::JjNotFound
            } else {
                JjError::IoError(e)
            }
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stderr).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);
            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Move a bookmark to a revision
    ///
    /// Runs `jj bookmark move <name> --to <to>`.
    /// Forward moves succeed; backward/sideways moves require --allow-backwards.
    pub fn bookmark_move(&self, name: &str, to: &str) -> Result<String, JjError> {
        self.run(&[
            commands::BOOKMARK,
            commands::BOOKMARK_MOVE,
            name,
            flags::TO,
            to,
        ])
    }

    /// Move a bookmark with --allow-backwards
    ///
    /// Runs `jj bookmark move <name> --to <to> --allow-backwards`.
    pub fn bookmark_move_allow_backwards(&self, name: &str, to: &str) -> Result<String, JjError> {
        self.run(&[
            commands::BOOKMARK,
            commands::BOOKMARK_MOVE,
            name,
            flags::TO,
            to,
            flags::ALLOW_BACKWARDS,
        ])
    }

    /// Run `jj diff -r <change_id>` for a specific change (raw output, no parse)
    ///
    /// Returns diff-only output without the commit header (unlike `jj show`).
    pub fn diff_raw(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[commands::DIFF, flags::REVISION, change_id])
    }

    /// Run `jj diff --git -r <change_id>` for git-compatible unified patch output
    ///
    /// Produces output suitable for `git apply`.
    pub fn diff_git_raw(&self, change_id: &str) -> Result<String, JjError> {
        self.run(&[
            commands::DIFF,
            flags::GIT_FORMAT,
            flags::REVISION,
            change_id,
        ])
    }

    /// Run `jj diff --from <from> --to <to>` to compare two revisions
    ///
    /// Returns the raw diff output between the two revisions.
    pub fn diff_range(&self, from: &str, to: &str) -> Result<String, JjError> {
        self.run(&[commands::DIFF, flags::FROM, from, flags::TO, to])
    }

    /// Run `jj diff --git --from <from> --to <to>` for git-compatible unified patch
    pub fn diff_range_git(&self, from: &str, to: &str) -> Result<String, JjError> {
        self.run(&[
            commands::DIFF,
            flags::GIT_FORMAT,
            flags::FROM,
            from,
            flags::TO,
            to,
        ])
    }

    /// Get metadata for a specific change (for compare info)
    ///
    /// Returns (change_id, bookmarks, author, timestamp, description).
    pub fn get_change_info(
        &self,
        change_id: &str,
    ) -> Result<(String, Vec<String>, String, String, String), JjError> {
        let template = Templates::change_info();
        let output = self.run(&[
            commands::LOG,
            flags::NO_GRAPH,
            flags::REVISION,
            change_id,
            flags::TEMPLATE,
            template,
        ])?;

        let line = output.lines().next().unwrap_or("");
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        if parts.len() == 5 {
            let bookmarks: Vec<String> = if parts[1].is_empty() {
                Vec::new()
            } else {
                parts[1].split(',').map(|s| s.to_string()).collect()
            };
            Ok((
                parts[0].to_string(),
                bookmarks,
                parts[2].to_string(),
                parts[3].to_string(),
                parts[4].to_string(),
            ))
        } else {
            Err(JjError::ParseError(format!(
                "Failed to parse change info: {}",
                line
            )))
        }
    }

    /// Run `jj file annotate` to show blame information for a file
    ///
    /// Shows the change responsible for each line of the specified file.
    /// Optionally annotates at a specific revision.
    ///
    /// Returns AnnotationContent containing line-by-line blame information.
    ///
    /// Uses a custom template with `change_id.short(8)` to ensure change_id
    /// length matches the log template, enabling reliable cross-view ID matching.
    pub fn file_annotate(
        &self,
        file_path: &str,
        revision: Option<&str>,
    ) -> Result<AnnotationContent, JjError> {
        let template = Templates::file_annotate();
        let mut args = vec![commands::FILE, commands::FILE_ANNOTATE];

        if let Some(rev) = revision {
            args.push(flags::REVISION);
            args.push(rev);
        }

        args.push(flags::TEMPLATE);
        args.push(template);
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
        assert!(executor.repo_path().is_none());
    }

    #[test]
    fn test_executor_with_path() {
        let executor = JjExecutor::with_repo_path(PathBuf::from("/tmp/test"));
        assert_eq!(executor.repo_path(), Some(&PathBuf::from("/tmp/test")));
    }

    #[test]
    fn test_push_bulk_mode_flag() {
        assert_eq!(PushBulkMode::All.flag(), "--all");
        assert_eq!(PushBulkMode::Tracked.flag(), "--tracked");
        assert_eq!(PushBulkMode::Deleted.flag(), "--deleted");
    }

    #[test]
    fn test_push_bulk_mode_label() {
        assert_eq!(PushBulkMode::All.label(), "all bookmarks");
        assert_eq!(PushBulkMode::Tracked.label(), "tracked bookmarks");
        assert_eq!(PushBulkMode::Deleted.label(), "deleted bookmarks");
    }
}
