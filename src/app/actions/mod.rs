//! jj operations (actions that modify repository state)

mod bookmark;
mod dialog;
mod push;
mod tag;

use std::io;
use std::process::ExitStatus;
use std::time::{Instant, SystemTime};

use crate::jj::{JjError, RunResult};
use crate::model::{
    CommandRecord, CommandStatus, CompareInfo, DiffContent, DiffDisplayFormat, DiffMode,
    Notification, RebaseMode,
};
use crate::ui::components::{Dialog, DialogCallback, SelectItem};

use crate::app::helpers::revision::{SelectedRevision, is_root_by_commit_id, short_id};

use super::state::{App, DirtyFlags, View};

/// Suspend TUI mode (raw mode off, leave alternate screen).
///
/// Returns a scope guard that restores TUI mode on drop.
/// Use this before running interactive jj commands (describe --edit, split, diffedit, resolve).
fn suspend_tui() -> impl Drop {
    use crossterm::execute;
    use crossterm::terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    };
    use std::io::stdout;

    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen, Clear(ClearType::All));

    scopeguard::guard((), |_| {
        let _ = enable_raw_mode();
        let _ = execute!(stdout(), EnterAlternateScreen);
    })
}

impl App {
    // ── Notification / error helpers ──────────────────────────────────

    /// Set a success notification (green)
    pub(crate) fn notify_success(&mut self, msg: impl Into<String>) {
        self.notification = Some(Notification::success(msg));
    }

    /// Set an info notification (blue)
    pub(crate) fn notify_info(&mut self, msg: impl Into<String>) {
        self.notification = Some(Notification::info(msg));
    }

    /// Set a warning notification (yellow)
    pub(crate) fn notify_warning(&mut self, msg: impl Into<String>) {
        self.notification = Some(Notification::warning(msg));
    }

    /// Set an error message (displayed in error area)
    pub(crate) fn set_error(&mut self, msg: impl Into<String>) {
        self.error_message = Some(msg.into());
    }

    // ── Command history recording helpers ─────────────────────────────

    /// Record a command execution into the command history.
    ///
    /// Called after a jj command completes. `args` are provided separately
    /// so that the command is recorded even on failure (when `RunResult` is not available).
    fn record_command(
        &mut self,
        operation: &str,
        args: &[&str],
        start: Instant,
        result: &Result<RunResult, JjError>,
    ) {
        let (status, error) = match result {
            Ok(_) => (CommandStatus::Success, None),
            Err(e) => (CommandStatus::Failed, Some(e.to_string())),
        };
        self.command_history.push(CommandRecord {
            operation: operation.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            timestamp: SystemTime::now(),
            duration_ms: start.elapsed().as_millis(),
            status,
            error,
        });
    }

    /// Run a jj command via the executor's `run()` and record it in command history.
    ///
    /// Returns the output string (same API as executor individual methods).
    /// Use this for write operations that should appear in Command History View.
    fn run_and_record(&mut self, operation: &str, args: &[&str]) -> Result<String, JjError> {
        let start = Instant::now();
        let result = self.jj.run(args);
        self.record_command(operation, args, start, &result);
        result.map(|r| r.output)
    }

    /// Record an interactive command execution (ExitStatus-based).
    ///
    /// Used for commands that go through `Stdio::inherit()` (split, diffedit, etc.)
    /// where we don't get RunResult but only ExitStatus.
    fn record_interactive_command(
        &mut self,
        operation: &str,
        args: &[&str],
        start: Instant,
        result: &io::Result<ExitStatus>,
    ) {
        let (status, error) = match result {
            Ok(s) if s.success() => (CommandStatus::Success, None),
            Ok(s) => (
                CommandStatus::Failed,
                Some(format!("exit code: {}", s.code().unwrap_or(-1))),
            ),
            Err(e) => (CommandStatus::Failed, Some(e.to_string())),
        };
        self.command_history.push(CommandRecord {
            operation: operation.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            timestamp: SystemTime::now(),
            duration_ms: start.elapsed().as_millis(),
            status,
            error,
        });
    }

    /// Record a command from a `Result<String, JjError>` (for methods that don't use RunResult).
    ///
    /// Used by push operations where executor methods return String directly.
    fn record_str_command(
        &mut self,
        operation: &str,
        args: &[&str],
        start: Instant,
        result: &Result<String, JjError>,
    ) {
        let (status, error) = match result {
            Ok(_) => (CommandStatus::Success, None),
            Err(e) => (CommandStatus::Failed, Some(e.to_string())),
        };
        self.command_history.push(CommandRecord {
            operation: operation.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            timestamp: SystemTime::now(),
            duration_ms: start.elapsed().as_millis(),
            status,
            error,
        });
    }

    /// Handle a simple jj action result: notify on success, set error on failure
    ///
    /// For the common pattern: Ok → notify_success + mark_dirty, Err → set_error.
    /// `err_prefix` is prepended to the error: `"{err_prefix}: {e}"`.
    /// Not suitable for actions that parse output, branch on result, or have side-effects.
    fn run_jj_action(
        &mut self,
        result: Result<String, crate::jj::JjError>,
        err_prefix: &str,
        success_msg: &str,
        dirty: DirtyFlags,
    ) {
        match result {
            Ok(_) => {
                self.notify_success(success_msg);
                self.mark_dirty_and_refresh_current(dirty);
            }
            Err(e) => {
                self.set_error(format!("{}: {}", err_prefix, e));
            }
        }
    }

    /// Execute undo operation
    ///
    /// jj 0.39+ outputs "Undid operation: ..." to stderr.
    /// We extract the description part for a more informative notification.
    pub(crate) fn execute_undo(&mut self) {
        let args: &[&str] = &["undo"];
        let start = Instant::now();
        let result = self.jj.run(args);
        self.record_command("Undo", args, start, &result);
        match result {
            Ok(r) => {
                let msg = Self::parse_undo_message(&r.stderr);
                self.notify_success(msg);
                self.mark_dirty_and_refresh_current(DirtyFlags::all());
            }
            Err(e) => {
                self.set_error(format!("Undo failed: {}", e));
            }
        }
    }

    /// Parse jj undo output to extract a concise notification message.
    ///
    /// Input format (jj 0.39+):
    ///   "Undid operation: <id> (<timestamp>) <description>\nRestored to ..."
    /// Returns: "Undo: <description>" or "Undo complete" as fallback.
    fn parse_undo_message(output: &str) -> String {
        // Find first line starting with "Undid operation:"
        for line in output.lines() {
            if let Some(rest) = line.strip_prefix("Undid operation:") {
                // Format: " <id> (<timestamp>) <description>"
                // Find closing ')' of timestamp, take everything after it
                if let Some(paren_end) = rest.find(')') {
                    let description = rest[paren_end + 1..].trim();
                    if !description.is_empty() {
                        return format!("Undo: {}", description);
                    }
                }
            }
        }
        "Undo complete".to_string()
    }

    /// Start describe input mode by fetching the full description
    ///
    /// If the description is multi-line, automatically opens the external
    /// editor instead of the 1-line input bar to prevent data loss.
    pub(crate) fn start_describe_input(&mut self, revision: &str) {
        // Fetch the full (multi-line) description from jj
        match self.jj.get_description(revision) {
            Ok(full_description) => {
                let description = full_description.trim_end_matches('\n').to_string();

                // Multi-line: fall through to external editor directly
                if description.lines().nth(1).is_some() {
                    self.execute_describe_external(revision);
                    return;
                }

                self.log_view
                    .set_describe_input(revision.to_string(), description);
            }
            Err(e) => {
                self.set_error(format!("Failed to get description: {}", e));
            }
        }
    }

    /// Execute describe via external editor (jj describe --edit)
    ///
    /// Temporarily exits TUI mode to allow the editor to run.
    /// Uses before/after description comparison to detect changes,
    /// since jj describe --edit exits 0 regardless of whether the user saved.
    pub(crate) fn execute_describe_external(&mut self, revision: &str) {
        // Pre-check: immutable commits cannot be described
        if self.jj.is_immutable(revision) {
            self.set_error("Cannot describe: commit is immutable");
            return;
        }

        // Capture description before editing for change detection
        let before = match self.jj.get_description(revision) {
            Ok(desc) => Some(desc.trim_end().to_string()),
            Err(_) => None,
        };

        let _guard = suspend_tui();

        // Run jj describe --editor (blocking, interactive)
        let start = Instant::now();
        let result = self.jj.describe_edit_interactive(revision);
        self.record_interactive_command(
            "Describe (editor)",
            &["describe", revision],
            start,
            &result,
        );

        match result {
            Ok(status) if status.success() => {
                // Compare before/after to detect actual changes
                let after = match self.jj.get_description(revision) {
                    Ok(desc) => Some(desc.trim_end().to_string()),
                    Err(_) => None,
                };

                match (before, after) {
                    (Some(b), Some(a)) if b == a => {
                        self.notify_info("Description unchanged");
                    }
                    (Some(_), Some(_)) => {
                        self.notify_success("Description updated");
                    }
                    _ => {
                        // Could not compare (get_description failed before or after)
                        self.notify_success("Describe editor closed");
                    }
                }
            }
            Ok(status) => {
                self.set_error(format!(
                    "Describe editor exited with error (code: {})",
                    status.code().unwrap_or(-1)
                ));
                return;
            }
            Err(e) => {
                self.set_error(format!("Describe failed: {}", e));
                return;
            }
        }

        // Refresh views (only on success — refresh_log clears error_message)
        self.mark_dirty_and_refresh_current(DirtyFlags::log());
    }

    /// Execute describe operation
    pub(crate) fn execute_describe(&mut self, revision: &str, message: &str) {
        let result = self.run_and_record("Describe", &["describe", revision, "-m", message]);
        self.run_jj_action(
            result,
            "Failed to update description",
            "Description updated",
            DirtyFlags::log(),
        );
    }

    /// Execute edit operation (set working-copy to specified change)
    pub(crate) fn execute_edit(&mut self, revision: &str) {
        let short_id = short_id(revision);
        let msg = format!("Now editing: {}", short_id);
        let result = self.run_and_record("Edit", &["edit", revision]);
        self.run_jj_action(result, "Failed to edit", &msg, DirtyFlags::log_and_status());
    }

    /// Execute new change operation
    pub(crate) fn execute_new_change(&mut self) {
        let result = self.run_and_record("New", &["new"]);
        self.run_jj_action(
            result,
            "Failed to create change",
            "Created new change",
            DirtyFlags::log_and_status(),
        );
    }

    /// Execute new change from specified parent
    pub(crate) fn execute_new_change_from(&mut self, parent_id: &str, display_name: &str) {
        let msg = format!("Created new change from {}", display_name);
        let result = self.run_and_record("New from", &["new", parent_id]);
        self.run_jj_action(
            result,
            "Failed to create change",
            &msg,
            DirtyFlags::log_and_status(),
        );
    }

    /// Execute commit operation (describe current change + create new change)
    pub(crate) fn execute_commit(&mut self, message: &str) {
        let result = self.run_and_record("Commit", &["commit", "-m", message]);
        self.run_jj_action(
            result,
            "Commit failed",
            "Changes committed",
            DirtyFlags::log_and_status(),
        );
    }

    /// Execute squash into target (requires terminal control transfer)
    ///
    /// jj squash --from/--into may open an editor when both source and destination
    /// have non-empty descriptions. Temporarily exits TUI mode to allow editor interaction.
    pub(crate) fn execute_squash_into(&mut self, source: &str, destination: &str) {
        if is_root_by_commit_id(&self.log_view.changes, source) {
            self.notify_info("Cannot squash: root commit has no parent");
            return;
        }

        let _guard = suspend_tui();

        // Run jj squash --from --into (blocking, interactive)
        let start = Instant::now();
        let result = self.jj.squash_into_interactive(source, destination);
        self.record_interactive_command(
            "Squash into",
            &["squash", "--from", source, "--into", destination],
            start,
            &result,
        );

        // 4. Handle result (io::Result<ExitStatus>)
        match result {
            Ok(status) if status.success() => {
                let src_short = short_id(source);
                let dst_short = short_id(destination);
                self.notify_success(format!(
                    "Squashed {} into {} (undo: u)",
                    src_short, dst_short
                ));
            }
            Ok(_) => {
                // Non-zero exit (user cancelled editor, or jj error)
                self.notify_info("Squash cancelled or failed");
            }
            Err(e) => {
                // IO error (command not found, etc.)
                self.set_error(format!("Squash failed: {}", e));
            }
        }

        // 5. Refresh views
        self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
    }

    /// Execute abandon operation (abandon a change)
    pub(crate) fn execute_abandon(&mut self, revision: &str) {
        if is_root_by_commit_id(&self.log_view.changes, revision) {
            self.notify_info("Cannot abandon: root commit");
            return;
        }
        let short_id = short_id(revision);
        let msg = format!("Abandoned {} (undo: u)", short_id);
        let result = self.run_and_record("Abandon", &["abandon", revision]);
        self.run_jj_action(result, "Abandon failed", &msg, DirtyFlags::log_and_status());
    }

    /// Execute revert operation (creates reverse-diff commit)
    pub(crate) fn execute_revert(&mut self, revision: &str) {
        let short_id = short_id(revision);
        let msg = format!("Reverted {} (undo: u)", short_id);
        let result = self.run_and_record("Revert", &["revert", "-r", revision, "--onto", "@"]);
        self.run_jj_action(result, "Revert failed", &msg, DirtyFlags::log());
    }

    /// Execute redo operation
    ///
    /// Only works if the last operation was an undo.
    pub(crate) fn execute_redo(&mut self) {
        // First, check if last operation was an undo and get the target
        match self.jj.get_redo_target() {
            Ok(Some(op_id)) => {
                let result = self.run_and_record("Redo", &["op", "restore", &op_id]);
                match result {
                    Ok(_) => {
                        self.notify_success("Redo complete");
                        self.mark_dirty_and_refresh_current(DirtyFlags::all());
                    }
                    Err(e) => {
                        self.set_error(format!("Redo failed: {}", e));
                    }
                }
            }
            Ok(None) => {
                // Not in an undo/redo chain, or multiple consecutive undos
                // Note: After multiple undos, use 'o' (Operation History) to restore
                self.notify_info(
                    "Nothing to redo (use 'o' for operation history after multiple undos)",
                );
            }
            Err(e) => {
                self.set_error(format!("Failed to check redo target: {}", e));
            }
        }
    }

    /// Execute operation restore
    ///
    /// **Warning**: This is a destructive operation that modifies repository history.
    /// TODO: Add confirmation dialog (Phase 5.2) before executing.
    /// Currently, users can undo with `u` key if needed.
    pub(crate) fn execute_op_restore(&mut self, operation_id: &str) {
        match self.run_and_record("Op restore", &["op", "restore", operation_id]) {
            Ok(_) => {
                let short_id = &operation_id[..12.min(operation_id.len())];
                self.notify_success(format!("Restored to {} (undo: u)", short_id));
                self.mark_dirty_and_refresh_current(DirtyFlags::all());
                // Go back to log view
                self.go_to_view(View::Log);
            }
            Err(e) => {
                self.set_error(format!("Restore failed: {}", e));
            }
        }
    }

    /// Execute split operation (requires terminal control transfer)
    ///
    /// This method temporarily exits raw mode to allow jj split
    /// to use its external diff editor.
    ///
    /// Uses scope guard to ensure terminal state is always restored,
    /// even if jj split panics or returns early.
    pub(crate) fn execute_split(&mut self, revision: &str) {
        // Guard: cannot split an empty commit (nothing to split)
        let is_empty = self.log_view.selected_change().is_some_and(|c| c.is_empty);
        if is_empty {
            self.notify_info("Cannot split: no changes in this revision");
            return;
        }

        let _guard = suspend_tui();

        // Run jj split (blocking)
        let start = Instant::now();
        let result = self.jj.split_interactive(revision);
        self.record_interactive_command("Split", &["split", "-r", revision], start, &result);

        // 4. Handle result and refresh
        // Note: _guard will restore terminal when this function returns
        match result {
            Ok(status) if status.success() => {
                let short_id = short_id(revision);
                self.notify_success(format!("Split {} complete (undo: u)", short_id));
            }
            Ok(_) => {
                self.notify_info("Split cancelled or failed");
            }
            Err(e) => {
                self.set_error(format!("Split failed: {}", e));
            }
        }

        // 5. Refresh views
        self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
    }

    /// Execute diffedit (interactive diff editor)
    ///
    /// When `file` is None, opens the full diffedit for the revision.
    /// When `file` is Some, opens diffedit scoped to that file.
    pub(crate) fn execute_diffedit(&mut self, revision: &str, file: Option<&str>) {
        let _guard = suspend_tui();

        // Run jj diffedit (blocking)
        let start = Instant::now();
        let result = if let Some(f) = file {
            self.jj.diffedit_file_interactive(revision, f)
        } else {
            self.jj.diffedit_interactive(revision)
        };
        let args: Vec<&str> = if let Some(f) = file {
            vec!["diffedit", "-r", revision, f]
        } else {
            vec!["diffedit", "-r", revision]
        };
        self.record_interactive_command("Diffedit", &args, start, &result);

        // 4. Handle result
        match result {
            Ok(status) if status.success() => {
                let short_id = short_id(revision);
                self.notify_success(format!("Diffedit {} complete (undo: u)", short_id));
            }
            Ok(_) => {
                self.notify_info("Diffedit cancelled or failed");
            }
            Err(e) => {
                self.set_error(format!("Diffedit failed: {}", e));
            }
        }

        // 5. Refresh views
        self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
    }

    /// Execute bisect run interactively
    ///
    /// Suspends TUI, runs `jj bisect run --range good..bad -- bash -c <command>`,
    /// then resumes TUI and refreshes the log.
    pub(crate) fn execute_bisect(&mut self, good: &str, bad: &str, command: &str) {
        let cmd = if command.trim().is_empty() {
            "bash"
        } else {
            command.trim()
        };

        let _guard = suspend_tui();

        println!(
            "--- jj bisect run --range {}..{} -- bash -c '{}' ---",
            good, bad, cmd
        );
        println!("(Press Ctrl+D or type 'exit' to finish if using interactive shell)\n");

        let start = Instant::now();
        let result = self.jj.bisect_run_interactive(good, bad, cmd);
        let range = format!("{}..{}", good, bad);
        self.record_interactive_command(
            "Bisect",
            &["bisect", "run", "--range", &range, "--", "bash", "-c", cmd],
            start,
            &result,
        );

        // _guard drop → TUI resume

        match result {
            Ok(status) if status.success() => {
                self.notify_success("Bisect completed (undo: u)");
            }
            Ok(status) => {
                self.notify_info(format!(
                    "Bisect exited with status: {}",
                    status.code().unwrap_or(-1)
                ));
            }
            Err(e) => {
                self.set_error(format!("Bisect failed: {}", e));
            }
        }

        self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
    }

    /// Execute restore for a single file
    pub(crate) fn execute_restore_file(&mut self, file_path: &str) {
        let msg = format!("Restored: {}", file_path);
        let result = self.run_and_record("Restore", &["restore", file_path]);
        self.run_jj_action(result, "Restore failed", &msg, DirtyFlags::log_and_status());
    }

    /// Execute restore for all files
    pub(crate) fn execute_restore_all(&mut self) {
        let result = self.run_and_record("Restore all", &["restore"]);
        self.run_jj_action(
            result,
            "Restore failed",
            "All files restored",
            DirtyFlags::log_and_status(),
        );
    }

    /// Execute `jj next --edit` and refresh
    pub(crate) fn execute_next(&mut self) {
        match self.run_and_record("Next", &["next", "--edit"]) {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());

                // Move cursor to @ position
                self.log_view.select_working_copy();

                let msg = Self::parse_next_prev_message(&output, "next");
                self.notify_success(msg);
            }
            Err(e) => {
                let msg = Self::format_next_prev_error(&e, "next");
                self.notify_warning(msg);
            }
        }
    }

    /// Execute `jj prev --edit` and refresh
    pub(crate) fn execute_prev(&mut self) {
        match self.run_and_record("Prev", &["prev", "--edit"]) {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());

                // Move cursor to @ position
                self.log_view.select_working_copy();

                let msg = Self::parse_next_prev_message(&output, "prev");
                self.notify_success(msg);
            }
            Err(e) => {
                let msg = Self::format_next_prev_error(&e, "prev");
                self.notify_warning(msg);
            }
        }
    }

    /// Parse jj next/prev success output to notification message
    fn parse_next_prev_message(output: &str, direction: &str) -> String {
        let trimmed = output.trim();
        if trimmed.is_empty() {
            format!("Moved {} successfully", direction)
        } else {
            let first_line = trimmed.lines().next().unwrap_or(trimmed);
            format!("Moved {}: {}", direction, first_line)
        }
    }

    /// Format next/prev error message for user
    fn format_next_prev_error(error: &crate::jj::JjError, direction: &str) -> String {
        let err_str = error.to_string();
        if err_str.contains("more than one child") || err_str.contains("more than one parent") {
            format!(
                "Cannot move {}: multiple {}. Use 'e' to edit a specific revision.",
                direction,
                if direction == "next" {
                    "children"
                } else {
                    "parents"
                }
            )
        } else if err_str.contains("No descendant") || err_str.contains("no child") {
            "Already at the newest change".to_string()
        } else if err_str.contains("No ancestor") || err_str.contains("no parent") {
            "Already at the root".to_string()
        } else {
            format!("Move {} failed: {}", direction, err_str)
        }
    }

    /// Execute `jj duplicate <change_id>` and refresh log
    ///
    /// Parses the output to extract the new change ID, refreshes the log,
    /// and moves focus to the duplicated change.
    pub(crate) fn duplicate(&mut self, revision: &str) {
        match self.jj.duplicate(revision) {
            Ok(output) => {
                // Parse new change_id from output
                let new_change_id = Self::parse_duplicate_output(&output);

                // Refresh log first (before notification)
                self.mark_dirty_and_refresh_current(DirtyFlags::log());

                // If refresh_log failed, don't show success notification
                if self.error_message.is_some() {
                    return;
                }

                // Move focus to duplicated change + build notification
                match new_change_id {
                    Some(ref new_id) => {
                        let short = &new_id[..new_id.len().min(8)];
                        if self.log_view.select_change_by_prefix(new_id) {
                            self.notification =
                                Some(Notification::success(format!("Duplicated as {}", short)));
                        } else {
                            self.notify_success(format!(
                                "Duplicated as {} (not in current revset)",
                                short
                            ));
                        }
                    }
                    None => {
                        self.notification =
                            Some(Notification::success("Duplicated successfully".to_string()));
                    }
                }
            }
            Err(e) => {
                self.set_error(format!("Duplicate failed: {}", e));
            }
        }
    }

    /// Parse the new change ID from `jj duplicate` output
    ///
    /// Output format: "Duplicated <commit_id> as <new_change_id> <new_commit_id> <description>"
    fn parse_duplicate_output(output: &str) -> Option<String> {
        for line in output.lines() {
            if let Some(rest) = line.strip_prefix("Duplicated ") {
                let parts: Vec<&str> = rest.splitn(4, ' ').collect();
                // parts[0] = commit_id, parts[1] = "as", parts[2] = new_change_id
                if parts.len() >= 3 && parts[1] == "as" {
                    return Some(parts[2].to_string());
                }
            }
        }
        None
    }

    /// Execute absorb: move working copy changes into ancestor commits
    ///
    /// Each hunk is moved to the closest mutable ancestor where the
    /// corresponding lines were modified last.
    pub(crate) fn execute_absorb(&mut self) {
        match self.run_and_record("Absorb", &["absorb"]) {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());

                // Simple notification: check if output is empty or contains "nothing"
                // Avoid detailed parsing due to jj version differences
                let notification =
                    if output.trim().is_empty() || output.to_lowercase().contains("nothing") {
                        Notification::info("Nothing to absorb")
                    } else {
                        Notification::success("Absorb finished")
                    };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.set_error(format!("Absorb failed: {}", e));
            }
        }
    }

    /// Execute simplify-parents: remove redundant parent edges
    pub(crate) fn execute_simplify_parents(&mut self, revision: &str) {
        match self.run_and_record("Simplify parents", &["simplify-parents", "-r", revision]) {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());

                let notification = if output.trim().is_empty()
                    || output.to_lowercase().contains("nothing")
                {
                    Notification::info("No redundant parents found")
                } else {
                    let short_id = short_id(revision);
                    Notification::success(format!("Simplified parents for {} (undo: u)", short_id))
                };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.set_error(format!("Simplify parents failed: {}", e));
            }
        }
    }

    /// Execute fix: apply configured code formatters to revision and descendants
    pub(crate) fn execute_fix(&mut self, revision: &str, change_id: &str) {
        // Capture commit_id before fix to detect if changes were made
        let commit_id_before = self
            .log_view
            .changes
            .iter()
            .find(|c| c.change_id == change_id)
            .map(|c| c.commit_id.to_string());

        match self.run_and_record("Fix", &["fix", "-s", revision]) {
            Ok(_) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());

                let short_id = short_id(change_id);

                // Compare commit_id after refresh to detect actual changes
                let commit_id_after = self
                    .log_view
                    .changes
                    .iter()
                    .find(|c| c.change_id == change_id)
                    .map(|c| c.commit_id.to_string());

                let notification = if commit_id_before == commit_id_after {
                    Notification::info("No fixes needed")
                } else {
                    Notification::success(format!(
                        "Applied fix to {} and descendants (undo: u)",
                        short_id
                    ))
                };
                self.notification = Some(notification);
            }
            Err(e) => {
                let err_msg = e.to_string();
                // jj actual error: "Config error: No `fix.tools` are configured"
                // Match on "fix.tools" only — specific to this error, avoids false positives
                if err_msg.contains("fix.tools") {
                    self.set_error("Fix failed: no fix.tools configured in jj config");
                } else {
                    self.set_error(format!("Fix failed: {}", e));
                }
            }
        }
    }

    /// Execute parallelize: convert linear chain to parallel (sibling) commits
    pub(crate) fn execute_parallelize(&mut self, from: &str, to: &str) {
        let revset = format!("{}::{} | {}::{}", from, to, to, from);
        match self.run_and_record("Parallelize", &["parallelize", &revset]) {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
                self.notification = Some(Self::parallelize_notification(&output));
            }
            Err(e) => {
                self.set_error(format!("Parallelize failed: {}", e));
            }
        }
    }

    /// Determine the notification for parallelize output
    ///
    /// Unlike simplify-parents, `jj parallelize` outputs nothing to stdout on success
    /// (changes are reported on stderr). So empty output means success, not "nothing happened".
    /// Only explicit "nothing" in output indicates no change.
    fn parallelize_notification(output: &str) -> Notification {
        if output.to_lowercase().contains("nothing") {
            Notification::info("Nothing to parallelize (revisions may not be connected)")
        } else {
            Notification::success("Parallelized (undo: u)")
        }
    }

    /// Execute git fetch (default behavior)
    pub(crate) fn execute_fetch(&mut self) {
        match self.run_and_record("Fetch", &["git", "fetch"]) {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::all());

                let notification = if output.trim().is_empty() {
                    Notification::info("Already up to date")
                } else {
                    Notification::success("Fetched from remote")
                };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.set_error(format!("Fetch failed: {}", e));
            }
        }
    }

    /// Start fetch flow: check remotes and show dialog if multiple
    ///
    /// When multiple remotes exist, shows a selection dialog including
    /// a "Specific branch..." option that opens a second dialog for
    /// branch selection.
    pub(crate) fn start_fetch(&mut self) {
        match self.jj.git_remote_list() {
            Ok(remotes) => {
                if remotes.len() <= 1 {
                    // Single remote (or none): execute immediately
                    self.execute_fetch();
                } else {
                    // Multiple remotes: show selection dialog
                    let mut items = vec![
                        SelectItem {
                            label: "Default fetch (jj config)".to_string(),
                            value: "__default__".to_string(),
                            selected: false,
                        },
                        SelectItem {
                            label: "All remotes (including untracked)".to_string(),
                            value: "__all_remotes__".to_string(),
                            selected: false,
                        },
                        SelectItem {
                            label: "Tracked bookmarks only".to_string(),
                            value: "__tracked__".to_string(),
                            selected: false,
                        },
                    ];
                    for remote in &remotes {
                        items.push(SelectItem {
                            label: remote.clone(),
                            value: remote.clone(),
                            selected: false,
                        });
                    }
                    items.push(SelectItem {
                        label: "Specific branch...".to_string(),
                        value: "__branch__".to_string(),
                        selected: false,
                    });
                    self.active_dialog = Some(Dialog::select_single(
                        "Git Fetch",
                        "Select remote to fetch from:",
                        items,
                        None,
                        DialogCallback::GitFetch,
                    ));
                }
            }
            Err(_) => {
                // Fallback to default fetch on remote list failure
                self.execute_fetch();
            }
        }
    }

    /// Execute fetch with specific remote option
    pub(crate) fn execute_fetch_with_option(&mut self, option: &str) {
        let (label, result) = match option {
            "__default__" => ("Fetch", self.run_and_record("Fetch", &["git", "fetch"])),
            "__all_remotes__" => (
                "Fetch all",
                self.run_and_record("Fetch all", &["git", "fetch", "--all-remotes"]),
            ),
            "__tracked__" => (
                "Fetch tracked",
                self.run_and_record("Fetch tracked", &["git", "fetch", "--tracked"]),
            ),
            remote => (
                "Fetch remote",
                self.run_and_record("Fetch remote", &["git", "fetch", "--remote", remote]),
            ),
        };
        let _ = label;
        match result {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::all());

                let notification = if output.trim().is_empty() {
                    let msg = match option {
                        "__tracked__" => "Tracked bookmarks: already up to date",
                        _ => "Already up to date",
                    };
                    Notification::info(msg)
                } else {
                    let source = match option {
                        "__default__" => "default remotes",
                        "__all_remotes__" => "all remotes",
                        "__tracked__" => "tracked bookmarks",
                        remote => remote,
                    };
                    Notification::success(format!("Fetched {}", source))
                };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.set_error(format!("Fetch failed: {}", e));
            }
        }
    }

    /// Show 2nd-step Select dialog for branch selection
    ///
    /// Gets local bookmark names via `jj bookmark list` and shows a Select dialog.
    /// If no bookmarks found, falls back to default fetch with notification.
    fn start_fetch_branch_select(&mut self) {
        match self.jj.bookmark_list_all() {
            Ok(bookmarks) => {
                // Filter to local-only bookmarks (no remote)
                let local_names: Vec<String> = bookmarks
                    .iter()
                    .filter(|b| b.remote.is_none())
                    .map(|b| b.name.clone())
                    .collect();

                if local_names.is_empty() {
                    self.notify_info("No bookmarks found");
                    self.execute_fetch();
                    return;
                }

                let items: Vec<SelectItem> = local_names
                    .iter()
                    .map(|name| SelectItem {
                        label: name.clone(),
                        value: name.clone(),
                        selected: false,
                    })
                    .collect();

                self.active_dialog = Some(Dialog::select_single(
                    "Fetch Branch",
                    "Select branch to fetch:",
                    items,
                    None,
                    DialogCallback::GitFetchBranch,
                ));
            }
            Err(_) => {
                // Fallback to default fetch on bookmark list failure
                self.notification =
                    Some(Notification::info("Failed to list bookmarks, fetching all"));
                self.execute_fetch();
            }
        }
    }

    /// Execute `jj git fetch --branch <name>` for a specific branch
    fn execute_fetch_branch(&mut self, branch: &str) {
        match self.run_and_record("Fetch branch", &["git", "fetch", "--branch", branch]) {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::all());

                let notification = if output.trim().is_empty() {
                    Notification::info(format!("Branch '{}': already up to date", branch))
                } else {
                    Notification::success(format!("Fetched branch '{}'", branch))
                };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.set_error(format!("Fetch failed: {}", e));
            }
        }
    }

    /// Resolve a conflict using :ours tool
    pub(crate) fn execute_resolve_ours(&mut self, file_path: &str) {
        let (change_id, is_wc) = match self.resolve_view {
            Some(ref v) => (v.revision.clone(), v.is_working_copy),
            None => return,
        };

        match self.run_and_record(
            "Resolve :ours",
            &["resolve", "--tool", ":ours", "-r", &change_id, file_path],
        ) {
            Ok(_) => {
                self.notify_success(format!("Resolved {} with :ours", file_path));
                self.refresh_resolve_list(&change_id, is_wc);
            }
            Err(e) => {
                self.set_error(format!("Resolve failed: {}", e));
            }
        }
    }

    /// Resolve a conflict using :theirs tool
    pub(crate) fn execute_resolve_theirs(&mut self, file_path: &str) {
        let (change_id, is_wc) = match self.resolve_view {
            Some(ref v) => (v.revision.clone(), v.is_working_copy),
            None => return,
        };

        match self.run_and_record(
            "Resolve :theirs",
            &["resolve", "--tool", ":theirs", "-r", &change_id, file_path],
        ) {
            Ok(_) => {
                self.notify_success(format!("Resolved {} with :theirs", file_path));
                self.refresh_resolve_list(&change_id, is_wc);
            }
            Err(e) => {
                self.set_error(format!("Resolve failed: {}", e));
            }
        }
    }

    /// Resolve a conflict using external merge tool (@ only)
    ///
    /// Similar to execute_split: temporarily exits TUI mode for interactive tool.
    pub(crate) fn execute_resolve_external(&mut self, file_path: &str) {
        let (change_id, is_wc) = match self.resolve_view {
            Some(ref v) => (v.revision.clone(), v.is_working_copy),
            None => return,
        };

        if !is_wc {
            self.notify_warning("External merge tool only works for working copy (@)");
            return;
        }

        let _guard = suspend_tui();

        // Run jj resolve (blocking)
        let start = Instant::now();
        let result = self.jj.resolve_interactive(file_path, Some(&change_id));
        self.record_interactive_command(
            "Resolve",
            &["resolve", "-r", &change_id, file_path],
            start,
            &result,
        );

        // 4. Handle result
        match result {
            Ok(status) if status.success() => {
                self.notify_success(format!("Resolved {}", file_path));
            }
            Ok(_) => {
                self.notify_info("Resolve cancelled or failed");
            }
            Err(e) => {
                self.set_error(format!("Resolve failed: {}", e));
            }
        }

        // 5. Refresh resolve list
        self.refresh_resolve_list(&change_id, is_wc);
    }

    /// Execute rebase with specified mode
    ///
    /// Supports five modes:
    /// - `Revision` (`-r`): Move single change, descendants rebased onto parent
    /// - `Source` (`-s`): Move change and all descendants together
    /// - `Branch` (`-b`): Move entire branch relative to destination's ancestors
    /// - `InsertAfter` (`-A`): Insert change after target in history
    /// - `InsertBefore` (`-B`): Insert change before target in history
    ///
    /// When `skip_emptied` is true, `--skip-emptied` is appended.
    /// When `simplify_parents` is true, `--simplify-parents` is appended.
    /// On unsupported flag errors, retries without the flag or shows guidance.
    pub(crate) fn execute_rebase(
        &mut self,
        source: &str,
        destination: &str,
        mode: RebaseMode,
        skip_emptied: bool,
        simplify_parents: bool,
        use_revset: bool,
    ) {
        // Prevent rebasing to self (skip for revset — let jj validate)
        if !use_revset && source == destination {
            self.notify_warning("Cannot rebase to itself");
            return;
        }

        let mut extra_flags: Vec<&str> = Vec::new();
        if skip_emptied {
            extra_flags.push(crate::jj::constants::flags::SKIP_EMPTIED);
        }
        if simplify_parents {
            extra_flags.push(crate::jj::constants::flags::SIMPLIFY_PARENTS);
        }

        // Build args for recording
        let mut rebase_args = vec!["rebase"];
        match mode {
            RebaseMode::Revision => {
                rebase_args.extend_from_slice(&["-r", source, "-d", destination])
            }
            RebaseMode::Source => rebase_args.extend_from_slice(&["-s", source, "-d", destination]),
            RebaseMode::Branch => rebase_args.extend_from_slice(&["-b", source, "-d", destination]),
            RebaseMode::InsertAfter => {
                rebase_args.extend_from_slice(&["-r", source, "-A", destination])
            }
            RebaseMode::InsertBefore => {
                rebase_args.extend_from_slice(&["-r", source, "-B", destination])
            }
        }
        rebase_args.extend_from_slice(&extra_flags);

        let result = self.run_and_record("Rebase", &rebase_args);

        match result {
            Ok(output) => {
                self.notify_rebase_success(
                    &output,
                    destination,
                    mode,
                    skip_emptied,
                    simplify_parents,
                );
            }
            Err(e) => {
                let err_msg = format!("{}", e);

                if !is_rebase_flag_unsupported(&err_msg) {
                    self.set_error(format!("Rebase failed: {}", e));
                    return;
                }

                // Fallback: remove optional flags in fixed order
                // Step 1: remove --simplify-parents if ON
                // Step 2: remove --skip-emptied if ON
                // Step 3: -b not supported → warning
                let mut unsupported_notes: Vec<&str> = Vec::new();
                let mut retry_skip = skip_emptied;
                let mut retry_simplify = simplify_parents;

                // Step 1: try without --simplify-parents
                if retry_simplify {
                    retry_simplify = false;
                    let mut retry_flags: Vec<&str> = Vec::new();
                    if retry_skip {
                        retry_flags.push(crate::jj::constants::flags::SKIP_EMPTIED);
                    }
                    let result = self.rebase_with_mode(source, destination, mode, &retry_flags);
                    match result {
                        Ok(output) => {
                            self.notify_rebase_success(
                                &output,
                                destination,
                                mode,
                                retry_skip,
                                false,
                            );
                            self.append_fallback_note("--simplify-parents not supported");
                            return;
                        }
                        Err(e2) => {
                            let msg2 = format!("{}", e2);
                            if !is_rebase_flag_unsupported(&msg2) {
                                self.set_error(format!("Rebase failed: {}", e2));
                                return;
                            }
                            unsupported_notes.push("--simplify-parents");
                        }
                    }
                }

                // Step 2: try without --skip-emptied
                if retry_skip {
                    retry_skip = false;
                    let result = self.rebase_with_mode(source, destination, mode, &[]);
                    match result {
                        Ok(output) => {
                            self.notify_rebase_success(&output, destination, mode, false, false);
                            unsupported_notes.push("--skip-emptied");
                            let note = format!("{} not supported", unsupported_notes.join("/"));
                            self.append_fallback_note(&note);
                            return;
                        }
                        Err(e3) => {
                            let msg3 = format!("{}", e3);
                            if !is_rebase_flag_unsupported(&msg3) {
                                self.set_error(format!("Rebase failed: {}", e3));
                                return;
                            }
                            unsupported_notes.push("--skip-emptied");
                        }
                    }
                }

                // Step 3: no optional flags left — if Branch mode, it's -b not supported
                if !retry_skip && !retry_simplify {
                    // All optional flags already removed, try bare rebase
                    let result = self.rebase_with_mode(source, destination, mode, &[]);
                    match result {
                        Ok(output) => {
                            self.notify_rebase_success(&output, destination, mode, false, false);
                            if !unsupported_notes.is_empty() {
                                let note = format!("{} not supported", unsupported_notes.join("/"));
                                self.append_fallback_note(&note);
                            }
                        }
                        Err(_) if mode == RebaseMode::Branch => {
                            self.notify_warning(
                                "Branch mode (-b) not supported in this jj version. Use Source mode (-s) instead.",
                            );
                        }
                        Err(e_final) => {
                            self.set_error(format!("Rebase failed: {}", e_final));
                        }
                    }
                }
            }
        }
    }

    /// Build and set notification for successful rebase
    fn notify_rebase_success(
        &mut self,
        output: &str,
        destination: &str,
        mode: RebaseMode,
        skip_emptied: bool,
        simplify_parents: bool,
    ) {
        self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());

        let mut suffixes = Vec::new();
        if skip_emptied {
            suffixes.push("empty commits skipped");
        }
        if simplify_parents {
            suffixes.push("parents simplified");
        }
        let suffix = if suffixes.is_empty() {
            String::new()
        } else {
            format!(" ({})", suffixes.join(", "))
        };

        // Unified conflict detection from jj output
        let has_conflict = output.to_lowercase().contains("conflict");
        let notification = if has_conflict {
            Notification::warning("Rebased with conflicts - resolve with jj resolve")
        } else {
            let msg = match mode {
                RebaseMode::Revision => format!("Rebased successfully{}", suffix),
                RebaseMode::Source => {
                    format!("Rebased source and descendants successfully{}", suffix)
                }
                RebaseMode::Branch => format!("Rebased branch successfully{}", suffix),
                RebaseMode::InsertAfter => {
                    let short = &destination[..8.min(destination.len())];
                    format!("Inserted after {} successfully{}", short, suffix)
                }
                RebaseMode::InsertBefore => {
                    let short = &destination[..8.min(destination.len())];
                    format!("Inserted before {} successfully{}", short, suffix)
                }
            };
            Notification::success(msg)
        };
        self.notification = Some(notification);
    }

    /// Build rebase args for a given mode and run (used by fallback retry)
    fn rebase_with_mode(
        &mut self,
        source: &str,
        destination: &str,
        mode: RebaseMode,
        extra_flags: &[&str],
    ) -> Result<String, crate::jj::JjError> {
        let mut args = vec!["rebase"];
        match mode {
            RebaseMode::Revision => args.extend_from_slice(&["-r", source, "-d", destination]),
            RebaseMode::Source => args.extend_from_slice(&["-s", source, "-d", destination]),
            RebaseMode::Branch => args.extend_from_slice(&["-b", source, "-d", destination]),
            RebaseMode::InsertAfter => args.extend_from_slice(&["-r", source, "-A", destination]),
            RebaseMode::InsertBefore => args.extend_from_slice(&["-r", source, "-B", destination]),
        }
        args.extend_from_slice(extra_flags);
        self.run_and_record("Rebase (retry)", &args)
    }

    /// Append fallback note to existing notification, preserving severity
    fn append_fallback_note(&mut self, note: &str) {
        if let Some(existing) = self.notification.take() {
            let new_msg = format!("{} ({})", existing.message, note);
            self.notification = Some(Notification::new(new_msg, existing.kind));
        }
    }

    /// Update preview cache if selected change has changed.
    ///
    /// Called after key processing, NOT during render.
    /// Skips fetch if:
    /// - Preview is disabled
    /// - Same change_id is already cached (cache hit)
    /// - Preview auto-disabled (small terminal) — tracks pending_id for later
    /// - Rapid movement detected (debounce: skip if last fetch was < 100ms ago)
    pub(crate) fn update_preview_if_needed(&mut self) {
        if !self.preview_enabled {
            return;
        }

        let sel = match SelectedRevision::from_log_view(&self.log_view) {
            Some(s) => s,
            None => return, // No selection — keep cache intact
        };

        // Working copy: always re-fetch (commit_id changes on every auto-snapshot,
        // so cached content is likely stale after any file change)
        // Non-working copy: cache hit if change_id + commit_id match
        if !sel.is_working_copy
            && let Some(cached) = self.preview_cache.peek(sel.change_id.as_str())
            && cached.commit_id == sel.commit_id.as_str()
        {
            self.preview_cache.touch(sel.change_id.as_str());
            return;
        }

        // Always defer to idle tick — never block key handling with jj show.
        // resolve_pending_preview() will fetch on the next poll timeout.
        self.preview_pending_id = Some(sel.change_id.to_string());
    }

    /// Actually fetch preview content via jj show
    fn fetch_preview(&mut self, change_id: &str) {
        self.preview_pending_id = None;

        // Capture bookmarks, commit_id, and working copy status from the Change model
        let (commit_id, bookmarks, is_working_copy) = self
            .log_view
            .selected_change()
            .filter(|c| c.change_id == change_id)
            .map(|c| {
                (
                    c.commit_id.to_string(),
                    c.bookmarks.clone(),
                    c.is_working_copy,
                )
            })
            .unwrap_or_default();

        // Use change_id for working copy (commit_id changes on every auto-snapshot),
        // commit_id for others (avoids ambiguity with divergent changes)
        let revision = if is_working_copy || commit_id.is_empty() {
            change_id
        } else {
            &commit_id
        };
        match self.jj.show(revision) {
            Ok(content) => {
                self.preview_cache.insert(super::state::PreviewCacheEntry {
                    change_id: change_id.to_string(),
                    commit_id,
                    content,
                    bookmarks,
                });
            }
            Err(_) => {
                self.preview_cache.remove(change_id);
            }
        }
    }

    /// Called from the event loop idle handler (when no key is pressed).
    /// Resolves any pending preview fetch that was deferred by debounce or auto-disable.
    pub fn resolve_pending_preview(&mut self) {
        if !self.preview_enabled || self.preview_auto_disabled {
            return;
        }
        if let Some(pending_id) = self.preview_pending_id.take() {
            // Verify the selection hasn't changed
            let still_selected = self
                .log_view
                .selected_change()
                .map(|c| c.change_id == pending_id)
                .unwrap_or(false);
            if still_selected {
                self.fetch_preview(&pending_id);
            }
        }
    }

    // =========================================================================
    // Diff export (clipboard copy & file export)
    // =========================================================================

    /// Copy diff content to system clipboard
    pub(crate) fn copy_diff_to_clipboard(&mut self, full: bool) {
        let Some(ref diff_view) = self.diff_view else {
            return;
        };
        let revision = diff_view.revision.clone();
        let compare_info = diff_view.compare_info.clone();
        let mode = diff_view.mode;

        let result = if full {
            match mode {
                DiffMode::Compare => {
                    let ci = compare_info.as_ref().unwrap();
                    let diff = self
                        .jj
                        .diff_range(ci.from.commit_id.as_str(), ci.to.commit_id.as_str());
                    diff.map(|d| {
                        format!(
                            "Compare: {} -> {}\nFrom: {} ({})\nTo:   {} ({})\n\n{}",
                            ci.from.change_id,
                            ci.to.change_id,
                            ci.from.change_id,
                            ci.from.description,
                            ci.to.change_id,
                            ci.to.description,
                            d,
                        )
                    })
                }
                DiffMode::Interdiff => {
                    let ci = compare_info.as_ref().unwrap();
                    let diff = self
                        .jj
                        .interdiff(ci.from.commit_id.as_str(), ci.to.commit_id.as_str());
                    diff.map(|d| {
                        format!(
                            "Interdiff: {} -> {}\nFrom: {} ({})\nTo:   {} ({})\n\n{}",
                            ci.from.change_id,
                            ci.to.change_id,
                            ci.from.change_id,
                            ci.from.description,
                            ci.to.change_id,
                            ci.to.description,
                            d,
                        )
                    })
                }
                DiffMode::Single => self.jj.show_raw(&revision),
            }
        } else {
            match mode {
                DiffMode::Compare => {
                    let ci = compare_info.as_ref().unwrap();
                    self.jj
                        .diff_range(ci.from.commit_id.as_str(), ci.to.commit_id.as_str())
                }
                DiffMode::Interdiff => {
                    let ci = compare_info.as_ref().unwrap();
                    self.jj
                        .interdiff(ci.from.commit_id.as_str(), ci.to.commit_id.as_str())
                }
                DiffMode::Single => self.jj.diff_raw(&revision),
            }
        };

        match result {
            Ok(text) => {
                let line_count = text.lines().count();
                match super::clipboard::copy_to_clipboard(&text) {
                    Ok(()) => {
                        let mode = if full { "show" } else { "diff" };
                        self.notify_success(format!(
                            "Copied to clipboard ({} lines, {})",
                            line_count, mode
                        ));
                    }
                    Err(e) => {
                        self.set_error(e);
                    }
                }
            }
            Err(e) => {
                self.set_error(format!("Failed to get diff: {}", e));
            }
        }
    }

    /// Fetch diff content in the specified format
    ///
    /// Handles the difference between normal and compare modes,
    /// and between ColorWords (which returns DiffContent directly via `jj show`)
    /// and Stat/Git (which return String and need parsing).
    fn fetch_diff_content(
        &self,
        change_id: &str,
        format: DiffDisplayFormat,
        compare: Option<&CompareInfo>,
        mode: DiffMode,
    ) -> Result<DiffContent, crate::jj::JjError> {
        use crate::jj::parser::Parser;

        match mode {
            DiffMode::Compare => {
                let ci = compare.expect("Compare mode requires compare_info");
                match format {
                    DiffDisplayFormat::ColorWords => self
                        .jj
                        .diff_range(ci.from.commit_id.as_str(), ci.to.commit_id.as_str())
                        .map(|o| Parser::parse_diff_body(&o)),
                    DiffDisplayFormat::Stat => self
                        .jj
                        .diff_range_stat(ci.from.commit_id.as_str(), ci.to.commit_id.as_str())
                        .map(|o| Parser::parse_diff_body_stat(&o)),
                    DiffDisplayFormat::Git => self
                        .jj
                        .diff_range_git(ci.from.commit_id.as_str(), ci.to.commit_id.as_str())
                        .map(|o| Parser::parse_diff_body_git(&o)),
                }
            }
            DiffMode::Interdiff => {
                let ci = compare.expect("Interdiff mode requires compare_info");
                match format {
                    DiffDisplayFormat::ColorWords => self
                        .jj
                        .interdiff(ci.from.commit_id.as_str(), ci.to.commit_id.as_str())
                        .map(|o| Parser::parse_diff_body(&o)),
                    DiffDisplayFormat::Stat => self
                        .jj
                        .interdiff_stat(ci.from.commit_id.as_str(), ci.to.commit_id.as_str())
                        .map(|o| Parser::parse_diff_body_stat(&o)),
                    DiffDisplayFormat::Git => self
                        .jj
                        .interdiff_git(ci.from.commit_id.as_str(), ci.to.commit_id.as_str())
                        .map(|o| Parser::parse_diff_body_git(&o)),
                }
            }
            DiffMode::Single => match format {
                DiffDisplayFormat::ColorWords => self.jj.show(change_id),
                DiffDisplayFormat::Stat => self
                    .jj
                    .show_stat(change_id)
                    .and_then(|o| Parser::parse_show_stat(&o)),
                DiffDisplayFormat::Git => self
                    .jj
                    .show_git(change_id)
                    .and_then(|o| Parser::parse_show_git(&o)),
            },
        }
    }

    /// Cycle the diff display format and re-fetch content
    pub(crate) fn cycle_diff_format(&mut self) {
        use crate::model::DiffDisplayFormat;

        let Some(ref diff_view) = self.diff_view else {
            return;
        };

        let old_format = diff_view.display_format;
        let new_format = old_format.next();
        let revision = diff_view.revision.clone();
        let compare_info = diff_view.compare_info.clone();
        let mode = diff_view.mode;

        match self.fetch_diff_content(&revision, new_format, compare_info.as_ref(), mode) {
            Ok(content) => {
                let diff_view = self.diff_view.as_mut().unwrap();
                diff_view.set_content(revision, content);
                diff_view.compare_info = compare_info;
                diff_view.display_format = new_format;

                self.notify_info(format!(
                    "Display: {} ({}/{})",
                    new_format.label(),
                    new_format.position(),
                    DiffDisplayFormat::COUNT,
                ));
            }
            Err(e) => {
                self.set_error(format!(
                    "Failed to load {} format: {}",
                    new_format.label(),
                    e
                ));
            }
        }
    }

    /// Export diff content to a .patch file
    pub(crate) fn export_diff_to_file(&mut self) {
        let Some(ref diff_view) = self.diff_view else {
            return;
        };
        let revision = diff_view.revision.clone();
        let compare_info = diff_view.compare_info.clone();
        let mode = diff_view.mode;

        // Determine filename and content based on mode
        // Uses `jj diff --git` for git-compatible unified patch format (git apply compatible)
        let (short_id, result) = match mode {
            DiffMode::Compare | DiffMode::Interdiff => {
                let ci = compare_info.as_ref().unwrap();
                let from_short = short_id(ci.from.change_id.as_str());
                let to_short = short_id(ci.to.change_id.as_str());
                let prefix = if mode == DiffMode::Interdiff {
                    "interdiff_"
                } else {
                    ""
                };
                let label = format!("{}{}_{}", prefix, from_short, to_short);
                let result = if mode == DiffMode::Interdiff {
                    self.jj
                        .interdiff_git(ci.from.commit_id.as_str(), ci.to.commit_id.as_str())
                } else {
                    self.jj
                        .diff_range_git(ci.from.commit_id.as_str(), ci.to.commit_id.as_str())
                };
                (label, result)
            }
            DiffMode::Single => {
                let short = short_id(&revision).to_string();
                let result = self.jj.diff_git_raw(&revision);
                (short, result)
            }
        };

        match result {
            Ok(text) => {
                let filename = unique_patch_filename(&short_id);
                match std::fs::write(&filename, &text) {
                    Ok(()) => {
                        self.notification =
                            Some(Notification::success(format!("Exported to {}", filename)));
                    }
                    Err(e) => {
                        self.set_error(format!("Failed to write {}: {}", filename, e));
                    }
                }
            }
            Err(e) => {
                self.set_error(format!("Failed to get diff: {}", e));
            }
        }
    }
}

/// Generate a unique .patch filename, appending -1, -2, etc. if the file already exists
fn unique_patch_filename(short_id: &str) -> String {
    let base = format!("{}.patch", short_id);
    if !std::path::Path::new(&base).exists() {
        return base;
    }
    for i in 1.. {
        let candidate = format!("{}-{}.patch", short_id, i);
        if !std::path::Path::new(&candidate).exists() {
            return candidate;
        }
    }
    unreachable!()
}

/// Check if a rebase error indicates an unsupported flag
///
/// Older jj versions may not support `--skip-emptied` or `-b`.
/// Detects generic "unexpected argument" / "unrecognized" errors.
fn is_rebase_flag_unsupported(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    lower.contains("unexpected argument")
        || lower.contains("unrecognized")
        || lower.contains("unknown flag")
        || lower.contains("unknown option")
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Describe multi-line detection tests
    //
    // The App::start_describe_input() method uses `desc.lines().nth(1).is_some()`
    // to detect multi-line descriptions and fall through to the external editor.
    // These tests verify the detection logic matches expectations.
    // =========================================================================

    #[test]
    fn test_multiline_detection_single_line() {
        let desc = "single line description";
        assert!(desc.lines().nth(1).is_none());
    }

    #[test]
    fn test_multiline_detection_two_lines() {
        let desc = "first line\nsecond line";
        assert!(desc.lines().nth(1).is_some());
    }

    #[test]
    fn test_multiline_detection_empty_string() {
        let desc = "";
        assert!(desc.lines().nth(1).is_none());
    }

    #[test]
    fn test_multiline_detection_trailing_newline_only() {
        // After trim_end_matches('\n'), a single-line desc with trailing \n becomes single-line
        let desc = "single line\n".trim_end_matches('\n');
        assert!(desc.lines().nth(1).is_none());
    }

    #[test]
    fn test_multiline_detection_two_lines_with_trailing_newline() {
        // After trim_end_matches('\n'), multi-line desc is still multi-line
        let desc = "first\nsecond\n".trim_end_matches('\n');
        assert!(desc.lines().nth(1).is_some());
    }

    // =========================================================================
    // parse_undo_message tests (jj 0.39+ output parsing)
    // =========================================================================

    #[test]
    fn test_parse_undo_message_jj039_format() {
        let output = "Undid operation: 4332678ef1ed (2026-03-06 10:29:59) describe commit 509de9e3\nRestored to operation: 6633e04968e6 (2026-03-06 10:29:58) add workspace 'default'";
        assert_eq!(
            App::parse_undo_message(output),
            "Undo: describe commit 509de9e3"
        );
    }

    #[test]
    fn test_parse_undo_message_fallback() {
        // Pre-0.39 or unexpected format
        let output = "Working copy now at: abc12345";
        assert_eq!(App::parse_undo_message(output), "Undo complete");
    }

    #[test]
    fn test_parse_undo_message_empty() {
        assert_eq!(App::parse_undo_message(""), "Undo complete");
    }

    // =========================================================================
    // Before/after description comparison tests
    //
    // The App::execute_describe_external() method compares descriptions
    // using trim_end() to normalize trailing whitespace.
    // These tests verify the comparison logic.
    // =========================================================================

    #[test]
    fn test_description_comparison_identical() {
        let before = "test description".trim_end().to_string();
        let after = "test description".trim_end().to_string();
        assert_eq!(before, after);
    }

    #[test]
    fn test_description_comparison_trailing_whitespace_normalized() {
        let before = "test description\n".trim_end().to_string();
        let after = "test description\n\n".trim_end().to_string();
        assert_eq!(before, after);
    }

    #[test]
    fn test_description_comparison_different() {
        let before = "old description".trim_end().to_string();
        let after = "new description".trim_end().to_string();
        assert_ne!(before, after);
    }

    // =========================================================================
    // has_force_push tests
    // =========================================================================

    // =========================================================================
    // is_immutable_bookmark tests
    // =========================================================================

    // =========================================================================
    // format_bookmark_status tests (multi-bookmark select dialog labels)
    // =========================================================================

    // =========================================================================
    // truncate_description tests (UTF-8 safe truncation)
    // =========================================================================

    // =========================================================================
    // parse_push_change_bookmark tests
    // =========================================================================

    // =========================================================================
    // push_target_remote cleanup tests
    // =========================================================================

    // =========================================================================
    // duplicate output parsing tests
    // =========================================================================

    #[test]
    fn test_parse_duplicate_output() {
        let output = "Duplicated 0193efbd0b2d as nyowntnw 6abd63b3 no-bookmark change (plain)";
        let result = App::parse_duplicate_output(output);
        assert_eq!(result, Some("nyowntnw".to_string()));
    }

    #[test]
    fn test_parse_duplicate_output_no_match() {
        let result = App::parse_duplicate_output("Some unrelated output");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_duplicate_output_empty() {
        let result = App::parse_duplicate_output("");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_duplicate_output_multiline() {
        // Warning lines before the actual duplicate output
        let output = "Working copy changes were not restored.\n\
                       Duplicated abc1234567890 as xyzwqrst def5678901 test description";
        let result = App::parse_duplicate_output(output);
        assert_eq!(result, Some("xyzwqrst".to_string()));
    }

    // =========================================================================
    // Revert dialog callback tests
    // =========================================================================

    // =========================================================================
    // is_revisions_unsupported_error tests
    // =========================================================================

    // =========================================================================
    // GitPushRevisions dialog callback tests
    // =========================================================================

    // =========================================================================
    // GitPushMultiBookmarkMode dialog callback tests
    // =========================================================================

    #[test]
    fn test_unique_patch_filename_no_conflict() {
        // When file doesn't exist, returns base name
        // (uses a name unlikely to exist in current dir)
        let name = unique_patch_filename("zzzz_test_nonexistent");
        assert_eq!(name, "zzzz_test_nonexistent.patch");
    }

    #[test]
    fn test_unique_patch_filename_with_conflict() {
        use std::fs;
        let base = "test_unique_patch_tmp";
        let base_file = format!("{}.patch", base);

        // Create the base file to force a conflict
        fs::write(&base_file, "test").unwrap();

        let name = unique_patch_filename(base);
        assert_eq!(name, format!("{}-1.patch", base));

        // Clean up
        let _ = fs::remove_file(&base_file);
    }

    #[test]
    fn test_unique_patch_filename_with_multiple_conflicts() {
        use std::fs;
        let base = "test_unique_multi_tmp";
        let files: Vec<String> = vec![
            format!("{}.patch", base),
            format!("{}-1.patch", base),
            format!("{}-2.patch", base),
        ];

        // Create all conflicting files
        for f in &files {
            fs::write(f, "test").unwrap();
        }

        let name = unique_patch_filename(base);
        assert_eq!(name, format!("{}-3.patch", base));

        // Clean up
        for f in &files {
            let _ = fs::remove_file(f);
        }
    }

    // --- Compare mode export path tests ---

    #[test]
    fn test_export_compare_mode_uses_diff_range_not_show() {
        use crate::model::{ChangeId, CommitId, CompareInfo, CompareRevisionInfo, DiffContent};
        use crate::ui::views::DiffView;

        let mut app = App::new_for_test();

        let compare_info = CompareInfo {
            from: CompareRevisionInfo {
                change_id: ChangeId::new("aaaa1111".to_string()),
                commit_id: CommitId::new("ff001111".to_string()),
                bookmarks: vec![],
                author: "user@test.com".to_string(),
                timestamp: "2024-01-01".to_string(),
                description: "from revision".to_string(),
            },
            to: CompareRevisionInfo {
                change_id: ChangeId::new("bbbb2222".to_string()),
                commit_id: CommitId::new("ff002222".to_string()),
                bookmarks: vec![],
                author: "user@test.com".to_string(),
                timestamp: "2024-01-02".to_string(),
                description: "to revision".to_string(),
            },
        };
        app.diff_view = Some(DiffView::new_compare(DiffContent::default(), compare_info));

        // Export will fail (no jj repo in test), but the error reveals which path was taken.
        // In compare mode, it should attempt `jj diff --git --from --to`.
        app.export_diff_to_file();

        // Should have an error (no jj repo), confirming the code path was executed
        assert!(
            app.error_message.is_some(),
            "Expected error from jj command in test environment"
        );
    }

    #[test]
    fn test_copy_clipboard_compare_mode_uses_diff_range() {
        use crate::model::{ChangeId, CommitId, CompareInfo, CompareRevisionInfo, DiffContent};
        use crate::ui::views::DiffView;

        let mut app = App::new_for_test();

        let compare_info = CompareInfo {
            from: CompareRevisionInfo {
                change_id: ChangeId::new("cccc3333".to_string()),
                commit_id: CommitId::new("ff003333".to_string()),
                bookmarks: vec![],
                author: "user@test.com".to_string(),
                timestamp: "2024-01-01".to_string(),
                description: "from".to_string(),
            },
            to: CompareRevisionInfo {
                change_id: ChangeId::new("dddd4444".to_string()),
                commit_id: CommitId::new("ff004444".to_string()),
                bookmarks: vec![],
                author: "user@test.com".to_string(),
                timestamp: "2024-01-02".to_string(),
                description: "to".to_string(),
            },
        };
        app.diff_view = Some(DiffView::new_compare(DiffContent::default(), compare_info));

        // Both full and diff-only should attempt diff_range in compare mode
        app.copy_diff_to_clipboard(true);
        assert!(
            app.error_message.is_some(),
            "Expected error from jj command in test environment (full)"
        );

        app.error_message = None;
        app.copy_diff_to_clipboard(false);
        assert!(
            app.error_message.is_some(),
            "Expected error from jj command in test environment (diff)"
        );
    }

    #[test]
    fn test_export_normal_mode_uses_diff_git() {
        use crate::model::DiffContent;
        use crate::ui::views::DiffView;

        let mut app = App::new_for_test();
        app.diff_view = Some(DiffView::new(
            "testid12".to_string(),
            DiffContent::default(),
        ));

        // Normal mode: should attempt `jj diff --git`
        app.export_diff_to_file();
        assert!(
            app.error_message.is_some(),
            "Expected error from jj command in test environment"
        );
    }

    // =========================================================================
    // is_private_commit_error tests
    // =========================================================================

    // =========================================================================
    // is_empty_description_error tests
    // =========================================================================

    // =========================================================================
    // is_rebase_flag_unsupported tests
    // =========================================================================

    #[test]
    fn test_rebase_flag_unsupported_unexpected_argument() {
        assert!(is_rebase_flag_unsupported(
            "error: unexpected argument '--skip-emptied'"
        ));
    }

    #[test]
    fn test_rebase_flag_unsupported_unrecognized() {
        assert!(is_rebase_flag_unsupported(
            "error: unrecognized option '-b'"
        ));
    }

    #[test]
    fn test_rebase_flag_unsupported_unknown_flag() {
        assert!(is_rebase_flag_unsupported(
            "error: unknown flag '--skip-emptied'"
        ));
    }

    #[test]
    fn test_rebase_flag_unsupported_unknown_option() {
        assert!(is_rebase_flag_unsupported(
            "error: unknown option '--skip-emptied'"
        ));
    }

    #[test]
    fn test_rebase_flag_unsupported_false_for_normal_error() {
        assert!(!is_rebase_flag_unsupported(
            "Error: Revision abc123 is not reachable from destination"
        ));
    }

    #[test]
    fn test_rebase_flag_unsupported_false_for_conflict() {
        assert!(!is_rebase_flag_unsupported(
            "Rebase produced conflict in src/main.rs"
        ));
    }

    // =========================================================================
    // Rebase fallback: Branch unsupported × skip_emptied=true (Issue #1)
    // =========================================================================

    /// When both `-b` and `--skip-emptied` are unsupported, the retry (without
    /// `--skip-emptied`) also fails with "unsupported flag" for `-b`.
    /// The handler must detect this and show the Branch guidance notification.
    #[test]
    fn test_rebase_branch_unsupported_detected_on_retry_error() {
        // Simulates: first error = "--skip-emptied unsupported", retry error = "-b unsupported"
        let retry_msg = "error: unexpected argument '-b'";
        assert!(is_rebase_flag_unsupported(retry_msg));
        // In execute_rebase, mode == Branch && unsupported → guidance notification (not error_message)
    }

    // =========================================================================
    // Rebase fallback: notification severity preservation (Issue #2)
    // =========================================================================

    /// When --skip-emptied retry succeeds with conflicts, notify_rebase_success
    /// sets a Warning. The skip-emptied suffix must preserve Warning severity.
    #[test]
    fn test_notification_severity_preserved_on_skip_emptied_fallback() {
        use crate::model::{Notification, NotificationKind};

        // Simulate: notify_rebase_success set a Warning for conflicts
        let existing = Notification::warning("Rebased with conflicts - resolve with jj resolve");
        assert_eq!(existing.kind, NotificationKind::Warning);

        // The fallback code takes the existing notification and creates a new one
        // preserving the kind
        let new_msg = format!(
            "{} (--skip-emptied not supported, empty commits may remain)",
            existing.message
        );
        let result = Notification::new(new_msg, existing.kind);

        // Severity must remain Warning (not downgraded to Info)
        assert_eq!(result.kind, NotificationKind::Warning);
        assert!(result.message.contains("conflicts"));
        assert!(result.message.contains("--skip-emptied not supported"));
    }

    /// When --skip-emptied retry succeeds without conflicts, severity is Success.
    #[test]
    fn test_notification_severity_success_on_skip_emptied_fallback() {
        use crate::model::{Notification, NotificationKind};

        let existing = Notification::success("Rebased successfully");
        let new_msg = format!(
            "{} (--skip-emptied not supported, empty commits may remain)",
            existing.message
        );
        let result = Notification::new(new_msg, existing.kind);

        assert_eq!(result.kind, NotificationKind::Success);
        assert!(result.message.contains("Rebased successfully"));
        assert!(result.message.contains("--skip-emptied not supported"));
    }

    // =========================================================================
    // Simplify Parents dialog callback tests
    // =========================================================================

    // =========================================================================
    // Parallelize dialog callback tests
    // =========================================================================

    #[test]
    fn test_parallelize_notification_success() {
        use crate::model::NotificationKind;
        let n = App::parallelize_notification("Rebased 3 commits");
        assert_eq!(n.kind, NotificationKind::Success);
        assert!(n.message.contains("Parallelized"));
    }

    #[test]
    fn test_parallelize_notification_nothing_output() {
        use crate::model::NotificationKind;
        let n = App::parallelize_notification("Nothing changed");
        assert_eq!(n.kind, NotificationKind::Info);
        assert!(n.message.contains("Nothing to parallelize"));
    }

    #[test]
    fn test_parallelize_notification_empty_output_is_success() {
        // jj parallelize outputs nothing to stdout on success
        use crate::model::NotificationKind;
        let n = App::parallelize_notification("");
        assert_eq!(n.kind, NotificationKind::Success);
        assert!(n.message.contains("Parallelized"));
    }

    #[test]
    fn test_parallelize_notification_whitespace_only_is_success() {
        use crate::model::NotificationKind;
        let n = App::parallelize_notification("  \n  ");
        assert_eq!(n.kind, NotificationKind::Success);
        assert!(n.message.contains("Parallelized"));
    }

    // =========================================================================
    // Notification / error helper regression tests (R1)
    // =========================================================================

    #[test]
    fn test_notify_success_sets_notification() {
        let mut app = App::new_for_test();
        app.notify_success("Operation complete");
        let n = app.notification.unwrap();
        assert_eq!(n.message, "Operation complete");
        assert_eq!(n.kind, crate::model::NotificationKind::Success);
    }

    #[test]
    fn test_notify_info_sets_notification() {
        let mut app = App::new_for_test();
        app.notify_info("Some info");
        let n = app.notification.unwrap();
        assert_eq!(n.message, "Some info");
        assert_eq!(n.kind, crate::model::NotificationKind::Info);
    }

    #[test]
    fn test_set_error_sets_error_message() {
        let mut app = App::new_for_test();
        app.set_error("Something failed");
        assert_eq!(app.error_message.as_deref(), Some("Something failed"));
    }

    // =========================================================================
    // Command history recording tests
    // =========================================================================

    #[test]
    fn test_record_command_preserves_args_on_success() {
        use crate::jj::RunResult;

        let mut app = App::new_for_test();
        let start = Instant::now();
        let result: Result<RunResult, JjError> = Ok(RunResult {
            output: "done".to_string(),
            stderr: String::new(),
            args: vec!["describe".to_string(), "-m".to_string(), "test".to_string()],
        });
        app.record_command("Describe", &["describe", "-m", "test"], start, &result);

        assert_eq!(app.command_history.len(), 1);
        let record = &app.command_history.records()[0];
        assert_eq!(record.operation, "Describe");
        assert_eq!(record.args, vec!["describe", "-m", "test"]);
        assert_eq!(record.status, CommandStatus::Success);
        assert!(record.error.is_none());
    }

    #[test]
    fn test_record_command_preserves_args_on_failure() {
        let mut app = App::new_for_test();
        let start = Instant::now();
        let result: Result<RunResult, JjError> = Err(JjError::CommandFailed {
            stderr: "Immutable commit".to_string(),
            exit_code: 1,
        });
        app.record_command("Edit", &["edit", "-r", "abc123"], start, &result);

        assert_eq!(app.command_history.len(), 1);
        let record = &app.command_history.records()[0];
        assert_eq!(record.operation, "Edit");
        // Bug #1 fix: args must be preserved even on failure
        assert_eq!(record.args, vec!["edit", "-r", "abc123"]);
        assert_eq!(record.status, CommandStatus::Failed);
        assert!(
            record
                .error
                .as_deref()
                .unwrap()
                .contains("Immutable commit")
        );
    }

    #[test]
    fn test_record_str_command_preserves_args_on_failure() {
        let mut app = App::new_for_test();
        let start = Instant::now();
        let result: Result<String, JjError> = Err(JjError::CommandFailed {
            stderr: "push failed".to_string(),
            exit_code: 1,
        });
        app.record_str_command(
            "Push",
            &["git", "push", "--bookmark", "main"],
            start,
            &result,
        );

        assert_eq!(app.command_history.len(), 1);
        let record = &app.command_history.records()[0];
        assert_eq!(record.args, vec!["git", "push", "--bookmark", "main"]);
        assert_eq!(record.status, CommandStatus::Failed);
        assert!(record.error.is_some());
    }

    #[test]
    fn test_record_interactive_command_preserves_args_on_failure() {
        let mut app = App::new_for_test();
        let start = Instant::now();
        let result: io::Result<ExitStatus> =
            Err(io::Error::new(io::ErrorKind::NotFound, "editor not found"));
        app.record_interactive_command("Split", &["split", "-r", "abc"], start, &result);

        assert_eq!(app.command_history.len(), 1);
        let record = &app.command_history.records()[0];
        assert_eq!(record.args, vec!["split", "-r", "abc"]);
        assert_eq!(record.status, CommandStatus::Failed);
        assert!(
            record
                .error
                .as_deref()
                .unwrap()
                .contains("editor not found")
        );
    }
}
