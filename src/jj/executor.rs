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

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use crate::model::{
    AnnotationContent, Bookmark, BookmarkInfo, Change, ChangeId, CommandKind, CommitId,
    ConflictFile, DiffContent, Operation, RebaseMode, Status, TagInfo, WorkspaceInfo,
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
    ///
    /// jj 0.44 pushes tags alongside bookmarks for all three bulk modes, so the
    /// labels name both.
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "all bookmarks and tags",
            Self::Tracked => "tracked bookmarks and tags",
            Self::Deleted => "deleted bookmarks and tags",
        }
    }
}

/// Result of running a jj command, including the captured arguments
pub struct RunResult {
    /// The command output (stdout)
    pub output: String,
    /// The stderr output (informational messages, warnings)
    pub stderr: String,
    /// The arguments passed to jj (excluding --color=never and --repository)
    pub args: Vec<String>,
    /// Sequence number of the captured [`JjInvocation`] — lets the App label
    /// exactly this invocation with an operation name (command transparency)
    pub seq: u64,
}

/// One captured jj process execution (command transparency P1).
///
/// Every spawn the executor performs is recorded here, then drained by the
/// App into the Command History so the user can see — and copy — exactly
/// what tij ran.
#[derive(Debug, Clone)]
pub struct JjInvocation {
    /// Monotonic id (shared across executor clones)
    pub seq: u64,
    /// The full argv actually passed to jj (`-R`, `--color=never`,
    /// `--no-integrate-operation` included — the honest command line)
    pub argv: Vec<String>,
    /// The args as the caller passed them (no auto-prepended flags). Used to
    /// correlate failed invocations with App-side operation labels — the full
    /// argv never matches the caller's args because of the prepended flags.
    pub bare_args: Vec<String>,
    pub kind: CommandKind,
    pub at: SystemTime,
    pub duration_ms: u128,
    pub success: bool,
    /// **Full** stderr on failure (see [`recorded_error`]) — not just its
    /// first line. jj routinely answers with `Error: …` followed by a
    /// `Hint: …` line, and the Command History detail is the only place that
    /// hint is reachable (the error banner is one row tall).
    pub error: Option<String>,
    /// Operation label set by the App (e.g. "Describe"); None = auto-label
    pub operation: Option<String>,
}

/// The error text recorded for a failed invocation: the **whole** stderr,
/// trailing newline trimmed.
///
/// Truncating to the first line here used to drop jj's second line, e.g.
///
/// ```text
/// Error: Refusing to create new remote tag v1.0@other
/// Hint: Run `jj tag track v1.0@other` and try again.
/// ```
///
/// The hint is the actionable half, so the capture keeps every line and lets
/// the consumer decide how much to show (`CommandHistoryView` renders up to
/// `MAX_ERROR_LINES`). `trim_end` only removes the trailing newline so the
/// detail view does not gain a phantom blank row; an empty stderr still
/// records `Some("")`, exactly as before, and renders as no error line.
fn recorded_error(stderr: &str) -> String {
    stderr.trim_end().to_string()
}

/// Exit code `jj resolve --list` uses for "this revision has no conflicts".
///
/// Measured against jj 0.44: no conflicts (with or without `-r`, with or
/// without paths) exits **2**, while genuine failures such as a nonexistent
/// revision exit 1. The code alone therefore separates the non-error from
/// real errors.
const NO_CONFLICTS_EXIT_CODE: i32 = 2;

/// Substring jj prints on stderr for the same state. Covers both wordings:
/// `Error: No conflicts found at this revision` and, when paths are given,
/// `Error: No conflicts found at the given path(s)`.
const NO_CONFLICTS_MARKER: &str = "No conflicts";

/// Is this failure jj's way of saying "there is nothing to resolve here"?
///
/// "No conflicts" is a normal state, not an error, but jj reports it as one.
/// Both the exit code **and** the message are required: exit-code-only would
/// swallow a real error if jj ever reused code 2 for something else, and
/// message-only would swallow a different (exit 1) failure whose text happens
/// to mention conflicts. Requiring both fails in the safe direction — if jj
/// changes either half, "no conflicts" merely surfaces as an error banner
/// again instead of a real error being silently discarded.
fn is_no_conflicts_error(err: &JjError) -> bool {
    matches!(
        err,
        JjError::CommandFailed { stderr, exit_code }
            if *exit_code == NO_CONFLICTS_EXIT_CODE && stderr.contains(NO_CONFLICTS_MARKER)
    )
}

/// Sequence counter and pending records live in ONE mutex so cloned
/// executors (which share the `Arc`) can never hand out duplicate seqs.
#[derive(Debug, Default)]
struct InvocationLog {
    next_seq: u64,
    records: VecDeque<JjInvocation>,
}

/// Backstop so the pending queue cannot grow unbounded if the App never
/// drains it (e.g. headless test harness). Generously above anything a
/// single event loop iteration can produce.
const MAX_PENDING_INVOCATIONS: usize = 1000;

/// Executor for jj commands
///
/// All methods take `&self` (no mutable state), making `JjExecutor` safe to
/// share across threads via `&JjExecutor`. This is verified by the compile-time
/// assertion below.
#[derive(Debug, Clone)]
pub struct JjExecutor {
    /// Path to the repository (None = current directory)
    repo_path: Option<PathBuf>,
    /// Captured invocations awaiting drain. `Arc<Mutex<…>>` (NOT RefCell):
    /// the `assert_sync` below requires `Sync` because Compare/Interdiff
    /// share `&JjExecutor` across `thread::scope` threads. Clones share the
    /// same log, so seq stays globally monotonic.
    log: Arc<Mutex<InvocationLog>>,
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
        Self {
            repo_path: None,
            log: Arc::default(),
        }
    }

    /// Create a new executor for a specific repository path
    #[allow(dead_code)]
    pub fn with_repo_path(path: PathBuf) -> Self {
        Self {
            repo_path: Some(path),
            log: Arc::default(),
        }
    }

    /// Get the repository path (for use by other impl blocks in sibling modules)
    pub(crate) fn repo_path(&self) -> Option<&PathBuf> {
        self.repo_path.as_ref()
    }

    /// The full argv `run`/`run_stderr` actually pass to jj for `args`.
    fn full_argv(&self, args: &[&str]) -> Vec<String> {
        let mut v = Vec::with_capacity(args.len() + 3);
        if let Some(ref path) = self.repo_path {
            v.push(flags::REPO_PATH.to_string());
            v.push(path.display().to_string());
        }
        v.push(flags::NO_COLOR.to_string());
        v.extend(args.iter().map(|s| s.to_string()));
        v
    }

    /// Record one finished invocation; returns its seq.
    fn capture(
        &self,
        args: &[&str],
        kind: CommandKind,
        started: Instant,
        success: bool,
        error: Option<String>,
    ) -> u64 {
        let mut log = self.log.lock().unwrap_or_else(|p| p.into_inner());
        let seq = log.next_seq;
        log.next_seq += 1;
        if log.records.len() >= MAX_PENDING_INVOCATIONS {
            log.records.pop_front();
        }
        log.records.push_back(JjInvocation {
            seq,
            argv: self.full_argv(args),
            bare_args: args.iter().map(|s| s.to_string()).collect(),
            kind,
            at: SystemTime::now(),
            duration_ms: started.elapsed().as_millis(),
            success,
            error,
            operation: None,
        });
        seq
    }

    /// Drain all pending invocations (oldest first). The App converts these
    /// into Command History records once per event-loop iteration.
    pub fn take_invocations(&self) -> Vec<JjInvocation> {
        let mut log = self.log.lock().unwrap_or_else(|p| p.into_inner());
        log.records.drain(..).collect()
    }

    /// Attach an operation label to the invocation with this seq.
    /// Returns false when it was already drained (caller falls back).
    pub fn label_invocation(&self, seq: u64, operation: &str) -> bool {
        let mut log = self.log.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(inv) = log.records.iter_mut().find(|i| i.seq == seq) {
            inv.operation = Some(operation.to_string());
            true
        } else {
            false
        }
    }

    /// Label the newest unlabeled invocation whose `bare_args` equal `args`
    /// (the Err path has no `RunResult.seq`; matching must use bare args —
    /// the full argv never equals the caller's args because of the
    /// auto-prepended `-R`/`--color=never`).
    pub fn label_newest_unlabeled_matching(&self, args: &[&str], operation: &str) -> bool {
        let mut log = self.log.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(inv) = log
            .records
            .iter_mut()
            .rev()
            .find(|i| i.operation.is_none() && i.bare_args == args)
        {
            inv.operation = Some(operation.to_string());
            true
        } else {
            false
        }
    }

    /// Test-only: inject a synthetic invocation (lib tests must not spawn jj).
    #[cfg(test)]
    pub(crate) fn push_test_invocation(&self, inv: JjInvocation) {
        let mut log = self.log.lock().unwrap_or_else(|p| p.into_inner());
        log.next_seq = log.next_seq.max(inv.seq + 1);
        log.records.push_back(inv);
    }

    /// Run a jj command with the given arguments
    ///
    /// Automatically adds `--color=never` to ensure parseable output.
    /// Returns `RunResult` containing both the output and the captured args.
    pub fn run(&self, args: &[&str]) -> Result<RunResult, JjError> {
        // Public entry = mutating by default; reads go through
        // `run_readonly_str`, which passes CommandKind::Read explicitly.
        self.run_kind(args, CommandKind::Write)
    }

    /// `run()` with an explicit invocation kind for the command-transparency
    /// capture. Every outcome path records exactly one [`JjInvocation`].
    fn run_kind(&self, args: &[&str], kind: CommandKind) -> Result<RunResult, JjError> {
        use std::process::Stdio;

        let args_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let started = Instant::now();

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

        // Defensively suppress any pager. jj usually skips the pager when stdout
        // is not a tty, but JJ_PAGER (jj 0.41+, mirrors JJ_EDITOR) lets us pin
        // it explicitly so terminal misdetection cannot blow up the TUI.
        cmd.env("JJ_PAGER", "cat");

        let output = match cmd.output() {
            Ok(o) => o,
            Err(e) => {
                let err = if e.kind() == std::io::ErrorKind::NotFound {
                    JjError::JjNotFound
                } else {
                    JjError::IoError(e)
                };
                self.capture(args, kind, started, false, Some(err.to_string()));
                return Err(err);
            }
        };

        if output.status.success() {
            let seq = self.capture(args, kind, started, true, None);
            Ok(RunResult {
                output: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                args: args_vec,
                seq,
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);
            let err_text = recorded_error(&stderr);

            // Check for common error patterns
            if stderr.contains(errors::NOT_A_REPO) {
                self.capture(args, kind, started, false, Some(err_text));
                return Err(JjError::NotARepository);
            }

            // jj exits with code 1 when snapshot warnings are present
            // (e.g., large files exceeding snapshot.max-new-file-size)
            // but the command itself may have succeeded.
            //
            // The snapshot is a pre-command operation; its failure does not
            // mean the command failed. Treat "Refused to snapshot" as
            // non-fatal for all commands — the proper fix is configuring
            // .jjignore or snapshot.max-new-file-size in the repository.
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            if stderr.contains("Refused to snapshot") {
                let seq = self.capture(args, kind, started, true, None);
                return Ok(RunResult {
                    output: stdout,
                    stderr,
                    args: args_vec,
                    seq,
                });
            }

            self.capture(args, kind, started, false, Some(err_text));
            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Run a jj command whose useful output is **stderr** (jj writes
    /// `duplicate` results and all `git push --dry-run` previews to stderr,
    /// so `run()` — which returns stdout — can't serve them). Same argv
    /// construction as `run()`, same capture, stderr returned on success.
    fn run_stderr(&self, args: &[&str], kind: CommandKind) -> Result<String, JjError> {
        let started = Instant::now();
        let mut cmd = Command::new(constants::JJ_COMMAND);
        if let Some(ref path) = self.repo_path {
            cmd.arg(flags::REPO_PATH).arg(path);
        }
        cmd.arg(flags::NO_COLOR);
        cmd.args(args);

        let output = match cmd.output() {
            Ok(o) => o,
            Err(e) => {
                let err = if e.kind() == std::io::ErrorKind::NotFound {
                    JjError::JjNotFound
                } else {
                    JjError::IoError(e)
                };
                self.capture(args, kind, started, false, Some(err.to_string()));
                return Err(err);
            }
        };

        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if output.status.success() {
            self.capture(args, kind, started, true, None);
            Ok(stderr)
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            let err_text = recorded_error(&stderr);
            self.capture(args, kind, started, false, Some(err_text));
            Err(JjError::CommandFailed { stderr, exit_code })
        }
    }

    /// Run a jj command and return only the output string.
    ///
    /// Convenience wrapper around `run()` for callers that don't need `RunResult.args`.
    // Used by run_and_record: see execute_*() in app/actions/
    fn run_str(&self, args: &[&str]) -> Result<String, JjError> {
        self.run(args).map(|r| r.output)
    }

    /// Run a read-only jj command, returning only the output string.
    ///
    /// Prepends `--no-integrate-operation` (jj 0.41+) so the invocation does
    /// not write a "snapshot working copy" entry to the operation log. Use
    /// only for invocations that observe state (log/status/diff/op log/...);
    /// never for commands that mutate the repo.
    fn run_readonly_str(&self, args: &[&str]) -> Result<String, JjError> {
        let mut all_args: Vec<&str> = Vec::with_capacity(args.len() + 1);
        all_args.push(flags::NO_INTEGRATE_OPERATION);
        all_args.extend_from_slice(args);
        self.run_kind(&all_args, CommandKind::Read)
            .map(|r| r.output)
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
        }

        // Always apply --limit to avoid slowness on large repos
        args.push(flags::LIMIT);
        args.push(constants::DEFAULT_LOG_LIMIT);

        if reversed {
            args.push(flags::REVERSED);
        }

        self.run_readonly_str(&args)
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
        self.run_readonly_str(&[commands::STATUS])
    }

    /// Run `jj status` and parse the output into Status
    pub fn status(&self) -> Result<Status, JjError> {
        let output = self.status_raw()?;
        Parser::parse_status(&output)
    }

    /// Run `jj config get <key>` and return the value, or None when unset
    ///
    /// tij stores its own settings under the `tij.*` namespace in jj's config
    /// (verified: jj 0.42 tolerates custom tables without warnings). Any
    /// error — including "Value not found" — maps to None so callers fall
    /// back to defaults silently.
    pub fn config_get(&self, key: &str) -> Option<String> {
        self.run_readonly_str(&[commands::CONFIG, commands::CONFIG_GET, key])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Run `jj show` for a specific change
    pub fn show_raw(&self, revision: &str) -> Result<String, JjError> {
        self.run_readonly_str(&[commands::SHOW, flags::REVISION, revision])
    }

    /// Run `jj show` and parse the output into DiffContent
    ///
    /// This is the preferred API for application code, following the same pattern as log_changes().
    pub fn show(&self, revision: &str) -> Result<DiffContent, JjError> {
        let output = self.show_raw(revision)?;
        Parser::parse_show(&output)
    }

    /// Run `jj show -r <revset>` and parse as a multi-revision stack diff
    ///
    /// jj 0.42+ accepts a revset resolving to multiple revisions and prints
    /// one `Commit ID:`-headed block per revision (newest first).
    pub fn show_stack(&self, revset: &str) -> Result<DiffContent, JjError> {
        let output = self.show_raw(revset)?;
        Parser::parse_show_stack(&output)
    }

    /// Count revisions matched by a revset, capped at `cap + 1`
    ///
    /// Returns at most `cap + 1` so callers can cheaply detect "more than cap"
    /// without enumerating a huge revset (e.g. a stack rooted near root()).
    pub fn count_revisions_capped(&self, revset: &str, cap: usize) -> Result<usize, JjError> {
        let limit = (cap + 1).to_string();
        let output = self.run_readonly_str(&[
            commands::LOG,
            flags::REVISION,
            revset,
            flags::NO_GRAPH,
            flags::TEMPLATE,
            r#"".""#,
            flags::LIMIT,
            &limit,
        ])?;
        Ok(output.chars().filter(|&c| c == '.').count())
    }

    /// Run `jj show --stat` for a specific change (histogram overview)
    pub fn show_stat(&self, revision: &str) -> Result<String, JjError> {
        self.run_readonly_str(&[commands::SHOW, flags::STAT, flags::REVISION, revision])
    }

    /// Run `jj show --git` for a specific change (git unified diff)
    pub fn show_git(&self, revision: &str) -> Result<String, JjError> {
        self.run_readonly_str(&[commands::SHOW, flags::GIT_FORMAT, flags::REVISION, revision])
    }

    /// Run `jj describe` to update change description
    ///
    /// Uses positional argument format: `jj describe <change-id> -m <message>`
    /// Note: `-r` is accepted as an alias for compatibility but positional is preferred.
    pub fn describe(&self, revision: &str, message: &str) -> Result<String, JjError> {
        self.run_str(&[commands::DESCRIBE, revision, "-m", message])
    }

    /// Get the full description (multi-line) for a change
    ///
    /// Uses `jj log -r <change-id> -T 'description'` to fetch the complete description.
    /// Unlike the normal log output which uses `description.first_line()`, this returns
    /// the entire description including all lines.
    pub fn get_description(&self, revision: &str) -> Result<String, JjError> {
        let output = self.run_readonly_str(&[
            commands::LOG,
            flags::NO_GRAPH,
            flags::REVISION,
            revision,
            flags::TEMPLATE,
            "description",
        ])?;
        Ok(output)
    }

    /// Check if a revision is immutable
    pub fn is_immutable(&self, revision: &str) -> bool {
        self.run_readonly_str(&[
            commands::LOG,
            flags::NO_GRAPH,
            flags::REVISION,
            revision,
            flags::TEMPLATE,
            r#"if(immutable, "true", "false")"#,
        ])
        .is_ok_and(|output| output.trim() == "true")
    }

    /// Run `jj edit` to set working-copy revision
    pub fn edit(&self, revision: &str) -> Result<String, JjError> {
        self.run_str(&[commands::EDIT, revision])
    }

    /// Run `jj new` to create a new empty change
    pub fn new_change(&self) -> Result<String, JjError> {
        self.run_str(&[commands::NEW])
    }

    /// Run `jj new <revision>` to create a new change with specified parent
    ///
    /// Creates a new empty change as a child of the specified revision.
    /// The working copy (@) moves to the new change.
    pub fn new_change_from(&self, revision: &str) -> Result<String, JjError> {
        self.run_str(&[commands::NEW, revision])
    }

    /// Run `jj commit` to commit current changes with a message
    ///
    /// This is equivalent to `jj describe` + `jj new`, but atomic.
    /// After commit, a new empty change is created on top.
    pub fn commit(&self, message: &str) -> Result<String, JjError> {
        self.run_str(&[commands::COMMIT, "-m", message])
    }

    /// Run `jj squash` to squash @ into @- (non-interactive)
    ///
    /// Moves changes from the current working copy into its parent.
    /// If the current change becomes empty, it is automatically abandoned.
    /// This uses `--use-destination-message` to avoid opening an editor.
    pub fn squash(&self) -> Result<String, JjError> {
        self.run_str(&[commands::SQUASH, "--use-destination-message"])
    }

    /// Run `jj abandon <change-id>` to abandon a revision
    ///
    /// Descendants are automatically rebased onto the parent.
    /// If @ is abandoned, a new empty change is created.
    pub fn abandon(&self, revision: &str) -> Result<String, JjError> {
        self.run_str(&[commands::ABANDON, revision])
    }

    /// Run `jj revert -r <change_id> --onto @` to create a reverse-diff commit
    ///
    /// Creates a new commit on top of @ that undoes the changes from the specified revision.
    pub fn revert(&self, revision: &str) -> Result<String, JjError> {
        self.run_str(&[
            commands::REVERT,
            flags::REVISION,
            revision,
            flags::ONTO,
            "@",
        ])
    }

    /// Run `jj restore <file_path>` to restore a specific file to its parent state
    pub fn restore_file(&self, file_path: &str) -> Result<String, JjError> {
        self.run_str(&[commands::RESTORE, file_path])
    }

    /// Run `jj restore` to restore all files to their parent state
    pub fn restore_all(&self) -> Result<String, JjError> {
        self.run_str(&[commands::RESTORE])
    }

    /// Run `jj evolog -r <change_id>` with template output
    pub fn evolog(&self, revision: &str) -> Result<String, JjError> {
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
        self.run_readonly_str(&[
            commands::EVOLOG,
            flags::REVISION,
            revision,
            flags::TEMPLATE,
            template,
        ])
    }

    /// Run `jj undo` to undo the last operation
    ///
    /// Returns the raw output from the command for notification display.
    pub fn undo(&self) -> Result<String, JjError> {
        self.run_str(&[commands::UNDO])
    }

    /// Run `jj op restore` to restore a previous operation (redo)
    ///
    /// This restores the operation before the most recent undo, effectively redoing.
    /// The operation ID should be obtained from `get_redo_target()`.
    pub fn redo(&self, operation_id: &str) -> Result<String, JjError> {
        self.run_str(&[commands::OP, commands::OP_RESTORE, operation_id])
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
        let output = self.run_readonly_str(&[
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

        let output = self.run_readonly_str(&args)?;
        Parser::parse_op_log(&output)
    }

    /// Run `jj op restore <operation_id>` to restore a previous state
    ///
    /// This restores the repository state to what it was after the specified operation.
    /// Use with caution - this is a powerful operation.
    pub fn op_restore(&self, operation_id: &str) -> Result<String, JjError> {
        self.run_str(&[commands::OP, commands::OP_RESTORE, operation_id])
    }

    /// Run `jj bookmark create <name> -r <change-id>` to create a bookmark
    ///
    /// Creates a new bookmark pointing to the specified change.
    /// Returns an error if a bookmark with the same name already exists.
    pub fn bookmark_create(&self, name: &str, revision: &str) -> Result<String, JjError> {
        self.run_str(&[
            commands::BOOKMARK,
            commands::BOOKMARK_CREATE,
            name,
            "-r",
            revision,
        ])
    }

    /// Run `jj bookmark set <name> -r <change-id> --allow-backwards` to move an existing bookmark
    ///
    /// Moves an existing bookmark to point to the specified change.
    /// Uses `--allow-backwards` to permit moving in any direction.
    pub fn bookmark_set(&self, name: &str, revision: &str) -> Result<String, JjError> {
        self.run_str(&[
            commands::BOOKMARK,
            commands::BOOKMARK_SET,
            name,
            "-r",
            revision,
            "--allow-backwards",
        ])
    }

    /// Run `jj bookmark delete <names>...` to delete bookmarks
    ///
    /// Deletes the specified bookmarks. Deletions propagate to remotes on push.
    pub fn bookmark_delete(&self, names: &[&str]) -> Result<String, JjError> {
        let mut args = vec![commands::BOOKMARK, commands::BOOKMARK_DELETE];
        args.extend(names);
        self.run_str(&args)
    }

    /// Run `jj bookmark list --all-remotes` to get all bookmarks
    ///
    /// Returns both local and remote bookmarks with their tracking status.
    /// Uses a template to output: name, remote, tracked (tab-separated).
    ///
    /// Note: Uses the `tracked` template field (jj 0.37+, guaranteed by startup check).
    pub fn bookmark_list_all(&self) -> Result<Vec<Bookmark>, JjError> {
        const BOOKMARK_LIST_TEMPLATE: &str = r#"separate("\t", name, remote, tracked) ++ "\n""#;

        let output = self.run_readonly_str(&[
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

        let log_output = self.run_readonly_str(&[
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
                    change_id: info.map(|(c, _, _)| ChangeId::new(c.clone())),
                    commit_id: info.map(|(_, c, _)| CommitId::new(c.clone())),
                    description: info.map(|(_, _, d)| d.clone()),
                    bookmark,
                }
            })
            .collect();

        Ok(result)
    }

    /// Compute the local bookmarks that `jj bookmark advance` would move to `@`.
    ///
    /// Evaluates `heads(::@ & bookmarks()) ~ @`: the front-most local bookmark
    /// on each ancestor branch, excluding bookmarks already at `@`
    /// (i.e. only those that would actually move).
    pub fn bookmarks_to_advance(&self) -> Result<Vec<String>, JjError> {
        const TEMPLATE: &str = r#"bookmarks.map(|x| x.name()).join(" ") ++ "\n""#;
        let output = self.run_readonly_str(&[
            commands::LOG,
            flags::NO_GRAPH,
            flags::REVISION,
            "heads(::@ & bookmarks()) ~ @",
            flags::TEMPLATE,
            TEMPLATE,
        ])?;
        Ok(super::parser::parse_advance_bookmarks(&output))
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
        self.run_str(&args)
    }

    /// Run `jj bookmark untrack <names...>` to stop tracking remote bookmarks
    ///
    /// Stops tracking the specified remote bookmarks locally.
    /// Format: `<name>@<remote>` (e.g., "feature-x@origin")
    pub fn bookmark_untrack(&self, names: &[&str]) -> Result<String, JjError> {
        let mut args = vec![commands::BOOKMARK, commands::BOOKMARK_UNTRACK];
        args.extend(names);
        self.run_str(&args)
    }

    /// Unified rebase: run jj rebase with the given mode and optional extra flags
    ///
    /// Supports all five modes:
    /// - `Revision` (`-r`): Move single change, descendants rebased onto parent
    /// - `Source` (`-s`): Move change and all descendants together
    /// - `Branch` (`-b`): Move entire branch relative to destination's ancestors
    /// - `InsertAfter` (`-A`): Insert change after target in history
    /// - `InsertBefore` (`-B`): Insert change before target in history
    ///
    /// `extra_flags` can include e.g. `--skip-emptied`.
    ///
    /// Returns the command output which may contain conflict information.
    pub fn rebase_unified(
        &self,
        mode: RebaseMode,
        source: &str,
        target: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![commands::REBASE];
        match mode {
            RebaseMode::Revision => {
                args.extend_from_slice(&[flags::REVISION, source, "-d", target]);
            }
            RebaseMode::Source => {
                args.extend_from_slice(&[flags::SOURCE, source, "-d", target]);
            }
            RebaseMode::Branch => {
                args.extend_from_slice(&[flags::BRANCH_SHORT, source, "-d", target]);
            }
            RebaseMode::InsertAfter => {
                args.extend_from_slice(&[flags::REVISION, source, flags::INSERT_AFTER, target]);
            }
            RebaseMode::InsertBefore => {
                args.extend_from_slice(&[flags::REVISION, source, flags::INSERT_BEFORE, target]);
            }
        }
        args.extend_from_slice(extra_flags);
        self.run_str(&args)
    }

    /// Check if a specific change has conflicts
    ///
    /// Uses `jj log -r <change_id> -T 'conflict'` to query the conflict status.
    /// Returns true if the change has unresolved conflicts.
    pub fn has_conflict(&self, revision: &str) -> Result<bool, JjError> {
        let output = self.run_readonly_str(&[
            commands::LOG,
            flags::NO_GRAPH,
            flags::REVISION,
            revision,
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
        self.run_str(&[commands::ABSORB])
    }

    /// Run `jj simplify-parents -r <change_id>` to remove redundant parent edges
    ///
    /// Removes parents that are ancestors of other parents, simplifying the DAG
    /// without changing content. Returns the command output.
    pub fn simplify_parents(&self, revision: &str) -> Result<String, JjError> {
        self.run_str(&[commands::SIMPLIFY_PARENTS, flags::REVISION, revision])
    }

    /// Run `jj fix -s <change_id>` (optionally with `--all-lines`)
    ///
    /// Applies configured code formatters to the specified revision
    /// and its descendants. Requires `[fix.tools.*]` in jj config.
    ///
    /// `all_lines = true` adds the `--all-lines` flag introduced in jj 0.41,
    /// which forces tools with `line-range-arg` configured to format the
    /// entire file instead of only modified lines.
    pub fn fix(&self, revision: &str, all_lines: bool) -> Result<String, JjError> {
        let mut args = vec![commands::FIX, flags::SOURCE, revision];
        if all_lines {
            args.push(flags::ALL_LINES);
        }
        self.run_str(&args)
    }

    /// Run `jj parallelize` to convert a linear chain into parallel (sibling) commits
    ///
    /// Uses the revset `from::to | to::from` to handle both directions
    /// (user may select newer→older or older→newer). One side will be empty,
    /// and the union ensures the correct range is always used.
    pub fn parallelize(&self, from: &str, to: &str) -> Result<String, JjError> {
        let revset = format!("{}::{} | {}::{}", from, to, to, from);
        self.run_str(&[commands::PARALLELIZE, &revset])
    }

    /// List conflicted files for a change
    ///
    /// Runs `jj resolve --list [-r <change_id>]` and parses the output.
    /// Returns an empty list if there are no conflicts — jj reports that
    /// normal state as a failure, which `is_no_conflicts_error` normalizes
    /// back into `Ok(vec![])`. Every other failure is propagated.
    pub fn resolve_list(&self, revision: Option<&str>) -> Result<Vec<ConflictFile>, JjError> {
        let mut args = vec![commands::RESOLVE, resolve_flags::LIST];

        if let Some(rev) = revision {
            args.push(flags::REVISION);
            args.push(rev);
        }

        match self.run_readonly_str(&args) {
            Ok(output) => Ok(Parser::parse_resolve_list(&output)),
            Err(e) if is_no_conflicts_error(&e) => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    /// Resolve a conflict using a built-in tool (:ours or :theirs)
    ///
    /// Works for any change (not just @).
    pub fn resolve_with_tool(
        &self,
        file_path: &str,
        tool: &str,
        revision: Option<&str>,
    ) -> Result<String, JjError> {
        let mut args = vec![commands::RESOLVE, resolve_flags::TOOL, tool];

        if let Some(rev) = revision {
            args.push(flags::REVISION);
            args.push(rev);
        }

        args.push(file_path);
        self.run_str(&args)
    }

    /// Rename a local bookmark
    ///
    /// Runs `jj bookmark rename <old> <new>`.
    /// Only works for local bookmarks. Remote bookmarks cannot be renamed.
    pub fn bookmark_rename(&self, old_name: &str, new_name: &str) -> Result<String, JjError> {
        self.run_str(&[
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
        self.run_str(&args)
    }

    /// Run `jj next --edit` to move @ to the next child
    pub fn next(&self) -> Result<String, JjError> {
        self.run_str(&[commands::NEXT, flags::EDIT_FLAG])
    }

    /// Run `jj prev --edit` to move @ to the previous parent
    pub fn prev(&self) -> Result<String, JjError> {
        self.run_str(&[commands::PREV, flags::EDIT_FLAG])
    }

    /// Run `jj duplicate <change_id>` to create a copy of the specified change
    ///
    /// Returns the jj stderr output containing the new change ID.
    /// Note: `jj duplicate` writes its result to stderr, not stdout.
    /// Output format: "Duplicated <commit_id> as <new_change_id> <new_commit_id> <description>"
    pub fn duplicate(&self, revision: &str) -> Result<String, JjError> {
        self.run_stderr(&[commands::DUPLICATE, revision], CommandKind::Write)
    }

    /// Run `jj git fetch` to fetch from default remotes
    ///
    /// Returns the command output describing what was fetched.
    /// Empty output typically means "already up to date".
    pub fn git_fetch(&self) -> Result<String, JjError> {
        self.run_str(&[commands::GIT, commands::GIT_FETCH])
    }

    /// Run `jj git fetch --all-remotes`
    pub fn git_fetch_all_remotes(&self) -> Result<String, JjError> {
        self.run_str(&[commands::GIT, commands::GIT_FETCH, flags::ALL_REMOTES])
    }

    /// Run `jj git fetch --remote <name>`
    pub fn git_fetch_remote(&self, remote: &str) -> Result<String, JjError> {
        self.run_str(&[commands::GIT, commands::GIT_FETCH, flags::REMOTE, remote])
    }

    /// Run `jj git fetch --tracked`
    ///
    /// Fetches only bookmarks that are already tracked from the default remote(s).
    /// `flags::TRACKED` (`"--tracked"`) is the same string for both fetch and push.
    pub fn git_fetch_tracked(&self) -> Result<String, JjError> {
        self.run_str(&[commands::GIT, commands::GIT_FETCH, flags::TRACKED])
    }

    /// Run `jj git fetch --tracked --remote <name>`
    pub fn git_fetch_tracked_remote(&self, remote: &str) -> Result<String, JjError> {
        self.run_str(&[
            commands::GIT,
            commands::GIT_FETCH,
            flags::TRACKED,
            flags::REMOTE,
            remote,
        ])
    }

    /// Run `jj git remote list` to get all remote names
    pub fn git_remote_list(&self) -> Result<Vec<String>, JjError> {
        let output = self.run_readonly_str(&[
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
        self.run_str(&[
            commands::GIT,
            commands::GIT_PUSH,
            flags::BOOKMARK_FLAG,
            bookmark_name,
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
        // `jj git push --dry-run` outputs to stderr, not stdout — and it
        // mutates nothing, so it is a Read for the command history.
        self.run_stderr(
            &[
                commands::GIT,
                commands::GIT_PUSH,
                flags::DRY_RUN,
                flags::BOOKMARK_FLAG,
                bookmark_name,
            ],
            CommandKind::Read,
        )
    }

    /// Run `jj git push --named <bookmark>=<revision>` for new remote bookmarks (jj 0.37+)
    ///
    /// This is the recommended way to push new bookmarks in jj 0.37+.
    /// The --named flag creates the bookmark if it doesn't exist, auto-tracks it,
    /// and pushes it in a single operation.
    pub fn git_push_named(&self, bookmark_name: &str, revision: &str) -> Result<String, JjError> {
        let named_arg = format!("{}={}", bookmark_name, revision);
        self.run_str(&[commands::GIT, commands::GIT_PUSH, flags::NAMED, &named_arg])
    }

    /// Run `jj git push --change <change_id>` to push by change ID
    ///
    /// Automatically creates a bookmark named `push-<change_id_prefix>`
    /// and pushes it to the remote. If the bookmark already exists, it
    /// reuses it.
    pub fn git_push_change(&self, change_id: &str) -> Result<String, JjError> {
        self.run_str(&[commands::GIT, commands::GIT_PUSH, flags::CHANGE, change_id])
    }

    /// Run `jj git push --bookmark <name> --remote <remote>` to push to specific remote
    pub fn git_push_bookmark_to_remote(
        &self,
        bookmark_name: &str,
        remote: &str,
    ) -> Result<String, JjError> {
        self.run_str(&[
            commands::GIT,
            commands::GIT_PUSH,
            flags::BOOKMARK_FLAG,
            bookmark_name,
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
        self.run_stderr(
            &[
                commands::GIT,
                commands::GIT_PUSH,
                flags::DRY_RUN,
                flags::BOOKMARK_FLAG,
                bookmark_name,
                flags::REMOTE,
                remote,
            ],
            CommandKind::Read,
        )
    }

    /// Run `jj git push --change <change_id> --remote <remote>`
    pub fn git_push_change_to_remote(
        &self,
        change_id: &str,
        remote: &str,
    ) -> Result<String, JjError> {
        self.run_str(&[
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
        self.run_stderr(
            &[
                commands::GIT,
                commands::GIT_PUSH,
                flags::DRY_RUN,
                flags::CHANGE,
                change_id,
                flags::REMOTE,
                remote,
            ],
            CommandKind::Read,
        )
    }

    /// Run `jj git push --change <change_id> --dry-run` to preview push
    ///
    /// Returns stderr output describing what would change on the remote
    /// if this change were pushed. Does NOT actually push anything.
    pub fn git_push_change_dry_run(&self, change_id: &str) -> Result<String, JjError> {
        self.run_stderr(
            &[
                commands::GIT,
                commands::GIT_PUSH,
                flags::DRY_RUN,
                flags::CHANGE,
                change_id,
            ],
            CommandKind::Read,
        )
    }

    /// Push with a bulk flag (--all, --tracked, --deleted)
    ///
    /// Returns the full `RunResult` so callers can inspect stderr for
    /// jj 0.41+ skip warnings ("Won't push bookmark X: commit … is private").
    pub fn git_push_bulk(
        &self,
        mode: PushBulkMode,
        remote: Option<&str>,
    ) -> Result<RunResult, JjError> {
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
        let mut args = vec![
            commands::GIT,
            commands::GIT_PUSH,
            flags::DRY_RUN,
            mode.flag(),
        ];
        if let Some(r) = remote {
            args.extend([flags::REMOTE, r]);
        }
        self.run_stderr(&args, CommandKind::Read)
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
        self.run_str(&args)
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
        self.run_str(&args)
    }

    /// Run `jj git push --change <change_id>` with extra flags
    pub fn git_push_change_with_flags(
        &self,
        change_id: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![commands::GIT, commands::GIT_PUSH, flags::CHANGE, change_id];
        args.extend_from_slice(extra_flags);
        self.run_str(&args)
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
        self.run_str(&args)
    }

    /// Run `jj git push --revisions <change_id>` with extra flags
    pub fn git_push_revisions_with_flags(
        &self,
        revision: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![
            commands::GIT,
            commands::GIT_PUSH,
            flags::REVISIONS,
            revision,
        ];
        args.extend_from_slice(extra_flags);
        self.run_str(&args)
    }

    /// Run `jj git push --revisions <change_id> --remote <remote>` with extra flags
    pub fn git_push_revisions_to_remote_with_flags(
        &self,
        revision: &str,
        remote: &str,
        extra_flags: &[&str],
    ) -> Result<String, JjError> {
        let mut args = vec![
            commands::GIT,
            commands::GIT_PUSH,
            flags::REVISIONS,
            revision,
            flags::REMOTE,
            remote,
        ];
        args.extend_from_slice(extra_flags);
        self.run_str(&args)
    }

    /// Run `jj git fetch --branch <name>` to fetch a specific branch
    pub fn git_fetch_branch(&self, branch: &str) -> Result<String, JjError> {
        self.run_str(&[commands::GIT, commands::GIT_FETCH, flags::BRANCH, branch])
    }

    /// Run `jj git push --revisions <change_id>` to push all bookmarks on a revision
    pub fn git_push_revisions(&self, revision: &str) -> Result<String, JjError> {
        self.run_str(&[
            commands::GIT,
            commands::GIT_PUSH,
            flags::REVISIONS,
            revision,
        ])
    }

    /// Run `jj git push --revisions <change_id> --remote <remote>`
    pub fn git_push_revisions_to_remote(
        &self,
        revision: &str,
        remote: &str,
    ) -> Result<String, JjError> {
        self.run_str(&[
            commands::GIT,
            commands::GIT_PUSH,
            flags::REVISIONS,
            revision,
            flags::REMOTE,
            remote,
        ])
    }

    /// Run `jj git push --dry-run --revisions <change_id>` to preview push
    ///
    /// Returns stderr output (jj git push --dry-run outputs to stderr).
    pub fn git_push_revisions_dry_run(&self, revision: &str) -> Result<String, JjError> {
        self.run_stderr(
            &[
                commands::GIT,
                commands::GIT_PUSH,
                flags::DRY_RUN,
                flags::REVISIONS,
                revision,
            ],
            CommandKind::Read,
        )
    }

    /// Run `jj git push --dry-run --revisions <change_id> --remote <remote>`
    pub fn git_push_revisions_dry_run_to_remote(
        &self,
        revision: &str,
        remote: &str,
    ) -> Result<String, JjError> {
        self.run_stderr(
            &[
                commands::GIT,
                commands::GIT_PUSH,
                flags::DRY_RUN,
                flags::REVISIONS,
                revision,
                flags::REMOTE,
                remote,
            ],
            CommandKind::Read,
        )
    }

    /// Move a bookmark to a revision
    ///
    /// Runs `jj bookmark move <name> --to <to>`.
    /// Forward moves succeed; backward/sideways moves require --allow-backwards.
    pub fn bookmark_move(&self, name: &str, to: &str) -> Result<String, JjError> {
        self.run_str(&[
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
        self.run_str(&[
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
    pub fn diff_raw(&self, revision: &str) -> Result<String, JjError> {
        self.run_readonly_str(&[commands::DIFF, flags::REVISION, revision])
    }

    /// Run `jj diff --git -r <change_id>` for git-compatible unified patch output
    ///
    /// Produces output suitable for `git apply`.
    pub fn diff_git_raw(&self, revision: &str) -> Result<String, JjError> {
        self.run_readonly_str(&[commands::DIFF, flags::GIT_FORMAT, flags::REVISION, revision])
    }

    /// Run `jj diff --from <from> --to <to>` to compare two revisions
    ///
    /// Returns the raw diff output between the two revisions.
    pub fn diff_range(&self, from: &str, to: &str) -> Result<String, JjError> {
        self.run_readonly_str(&[commands::DIFF, flags::FROM, from, flags::TO, to])
    }

    /// Run `jj diff --git --from <from> --to <to>` for git-compatible unified patch
    pub fn diff_range_git(&self, from: &str, to: &str) -> Result<String, JjError> {
        self.run_readonly_str(&[
            commands::DIFF,
            flags::GIT_FORMAT,
            flags::FROM,
            from,
            flags::TO,
            to,
        ])
    }

    /// Run `jj diff --stat --from <from> --to <to>` for histogram overview
    pub fn diff_range_stat(&self, from: &str, to: &str) -> Result<String, JjError> {
        self.run_readonly_str(&[
            commands::DIFF,
            flags::STAT,
            flags::FROM,
            from,
            flags::TO,
            to,
        ])
    }

    /// Run `jj interdiff --from <from> --to <to>` for patch comparison
    pub fn interdiff(&self, from: &str, to: &str) -> Result<String, JjError> {
        self.run_readonly_str(&[commands::INTERDIFF, flags::FROM, from, flags::TO, to])
    }

    /// Run `jj interdiff --git --from <from> --to <to>` for git-compatible patch comparison
    pub fn interdiff_git(&self, from: &str, to: &str) -> Result<String, JjError> {
        self.run_readonly_str(&[
            commands::INTERDIFF,
            flags::GIT_FORMAT,
            flags::FROM,
            from,
            flags::TO,
            to,
        ])
    }

    /// Run `jj interdiff --stat --from <from> --to <to>` for histogram overview
    pub fn interdiff_stat(&self, from: &str, to: &str) -> Result<String, JjError> {
        self.run_readonly_str(&[
            commands::INTERDIFF,
            flags::STAT,
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
        let output = self.run_readonly_str(&[
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

        let output = self.run_readonly_str(&args)?;
        Parser::parse_file_annotate(&output, file_path)
    }

    // ── Tag operations ─────────────────────────────────────────────

    /// List all tags (local and remote) with their target commit info
    ///
    /// Uses a single-stage query (unlike bookmarks which need 2 stages)
    /// because `jj tag list -T` can access `normal_target` directly.
    ///
    /// `--all-remotes` is required: without it `jj tag list` returns no
    /// untracked remote tag at all, so there would be nothing to track.
    ///
    /// The template concatenates fields explicitly with `++ "\t" ++` instead of
    /// using `separate()`. `separate()` drops empty fields *together with* their
    /// separator, which makes the column count vary per row; explicit
    /// concatenation keeps empty fields and yields a fixed 8 columns.
    ///
    /// The three `normal_target` fields are wrapped in `try(expr, "")`: rows with
    /// `present == false` (local tag deleted while the remote tag remains) have no
    /// commit, and jj does not fail there — it embeds the literal string
    /// `<Error: No Commit available>` in the output, which would be mis-parsed as
    /// a change_id. `try()` turns it into an empty field.
    pub fn tag_list(&self) -> Result<Vec<TagInfo>, JjError> {
        const TAG_LIST_TEMPLATE: &str = concat!(
            r#"name ++ "\t""#,
            r#" ++ if(remote, remote, "") ++ "\t""#,
            r#" ++ if(present, "true", "false") ++ "\t""#,
            r#" ++ if(tracked, "true", "false") ++ "\t""#,
            r#" ++ if(conflict, "true", "false") ++ "\t""#,
            r#" ++ try(normal_target.change_id().short(8), "") ++ "\t""#,
            r#" ++ try(normal_target.commit_id().short(8), "") ++ "\t""#,
            r#" ++ try(normal_target.description().first_line(), "") ++ "\n""#,
        );

        let output = self.run_readonly_str(&[
            commands::TAG,
            commands::TAG_LIST,
            flags::ALL_REMOTES,
            flags::TEMPLATE,
            TAG_LIST_TEMPLATE,
        ])?;
        Ok(super::parser::parse_tag_list(&output))
    }

    /// Create a tag on a specific revision
    ///
    /// Runs `jj tag set <name> -r <revision>`.
    pub fn tag_set(&self, name: &str, revision: &str) -> Result<String, JjError> {
        self.run_str(&[
            commands::TAG,
            commands::TAG_SET,
            name,
            flags::REVISION,
            revision,
        ])
    }

    /// Delete a tag
    ///
    /// Runs `jj tag delete <name>`.
    pub fn tag_delete(&self, name: &str) -> Result<String, JjError> {
        self.run_str(&[commands::TAG, commands::TAG_DELETE, name])
    }

    // ── Workspace operations ──────────────────────────────────────

    /// Get the workspace root path
    pub fn workspace_root(&self) -> Result<String, JjError> {
        let output = self.run_readonly_str(&[commands::WORKSPACE, "root"])?;
        Ok(output.trim().to_string())
    }

    /// List all workspaces
    pub fn workspace_list(&self) -> Result<Vec<WorkspaceInfo>, JjError> {
        let template = Templates::workspace_list();
        let output = self.run_readonly_str(&[
            commands::WORKSPACE,
            commands::WORKSPACE_LIST,
            flags::TEMPLATE,
            template,
        ])?;
        Ok(super::parser::parse_workspace_list(&output))
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

    /// Regression: the capture must not collapse jj's stderr to its first
    /// line. jj puts the actionable half in the `Hint:` line, and after this
    /// the Command History detail is the only place it can be read.
    #[test]
    fn recorded_error_keeps_every_stderr_line() {
        let stderr = "Error: Refusing to create new remote tag v1.0@other\n\
                      Hint: Run `jj tag track v1.0@other` and try again.\n";
        let recorded = recorded_error(stderr);
        assert_eq!(
            recorded.lines().count(),
            2,
            "both lines survive the capture: {recorded:?}"
        );
        assert!(recorded.starts_with("Error: Refusing to create new remote tag"));
        assert!(
            recorded.contains("Hint: Run `jj tag track v1.0@other` and try again."),
            "the Hint line must not be truncated away: {recorded:?}"
        );
    }

    #[test]
    fn recorded_error_trims_only_the_trailing_newline() {
        // No phantom blank row in the detail view …
        assert_eq!(recorded_error("Error: boom\n"), "Error: boom");
        assert_eq!(recorded_error("Error: boom\n\n"), "Error: boom");
        // … and interior blank lines / indentation are left alone.
        assert_eq!(
            recorded_error("Error: a\n\n  Hint: b"),
            "Error: a\n\n  Hint: b"
        );
        // Empty stderr keeps recording as an empty string (renders as no
        // error line), matching the pre-full-stderr behaviour.
        assert_eq!(recorded_error(""), "");
    }

    // =====================================================================
    // resolve_list: "no conflicts" is a normal state, not an error
    //
    // jj 0.44 exits 2 with `Error: No conflicts found …` when a revision is
    // conflict-free. `resolve_list` turns exactly that into `Ok(vec![])`; the
    // predicate below is what draws the line, so it is pinned from both sides.
    // =====================================================================

    #[test]
    fn no_conflicts_at_this_revision_is_not_a_real_error() {
        let err = JjError::CommandFailed {
            stderr: "Error: No conflicts found at this revision\n".to_string(),
            exit_code: 2,
        };
        assert!(is_no_conflicts_error(&err));
    }

    #[test]
    fn no_conflicts_at_the_given_paths_is_not_a_real_error() {
        // Path-scoped wording (`jj resolve --list <path>`), plus jj's warning line.
        let err = JjError::CommandFailed {
            stderr: "Warning: No matching entries for paths: src/main.rs\n\
                     Error: No conflicts found at the given path(s)\n"
                .to_string(),
            exit_code: 2,
        };
        assert!(is_no_conflicts_error(&err));
    }

    #[test]
    fn exit_2_with_another_message_stays_a_real_error() {
        // Exit code alone must not classify: if jj ever reuses 2, a genuine
        // failure must still reach the error banner.
        let err = JjError::CommandFailed {
            stderr: "Error: Merge tool exited with code 2\n".to_string(),
            exit_code: 2,
        };
        assert!(!is_no_conflicts_error(&err));
    }

    #[test]
    fn no_conflicts_wording_with_exit_1_stays_a_real_error() {
        // Message alone must not classify either — exit 1 is a real failure
        // even when its text mentions conflicts.
        let err = JjError::CommandFailed {
            stderr: "Error: No conflicts something went wrong\n".to_string(),
            exit_code: 1,
        };
        assert!(!is_no_conflicts_error(&err));
    }

    #[test]
    fn nonexistent_revision_stays_a_real_error() {
        // Measured jj 0.44 behaviour for a bad `-r`: exit 1.
        let err = JjError::CommandFailed {
            stderr: "Error: Revision `nosuch` doesn't exist\n".to_string(),
            exit_code: 1,
        };
        assert!(!is_no_conflicts_error(&err));
    }

    #[test]
    fn non_command_failures_stay_real_errors() {
        assert!(!is_no_conflicts_error(&JjError::JjNotFound));
        assert!(!is_no_conflicts_error(&JjError::NotARepository));
        assert!(!is_no_conflicts_error(&JjError::ParseError(
            "No conflicts".to_string()
        )));
    }

    #[test]
    fn test_push_bulk_mode_label() {
        // jj 0.44: every bulk mode pushes tags together with bookmarks.
        assert_eq!(PushBulkMode::All.label(), "all bookmarks and tags");
        assert_eq!(PushBulkMode::Tracked.label(), "tracked bookmarks and tags");
        assert_eq!(PushBulkMode::Deleted.label(), "deleted bookmarks and tags");
    }
}
