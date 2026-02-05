//! jj operations (actions that modify repository state)

use crate::model::Notification;
use crate::ui::components::{Dialog, DialogCallback, DialogResult, SelectItem};

use super::state::{App, View};

impl App {
    /// Execute undo operation
    pub(crate) fn execute_undo(&mut self) {
        match self.jj.undo() {
            Ok(_) => {
                self.notification = Some(Notification::success("Undo complete"));
                // Refresh log to show updated state
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
            }
            Err(e) => {
                self.error_message = Some(format!("Undo failed: {}", e));
            }
        }
    }

    /// Start describe input mode by fetching the full description
    pub(crate) fn start_describe_input(&mut self, change_id: &str) {
        // Fetch the full (multi-line) description from jj
        match self.jj.get_description(change_id) {
            Ok(full_description) => {
                // Remove only trailing newline that jj adds, preserve intentional blank lines
                let description = full_description.trim_end_matches('\n').to_string();
                self.log_view
                    .set_describe_input(change_id.to_string(), description);
            }
            Err(e) => {
                // If we can't fetch the description, show error
                self.error_message = Some(format!("Failed to get description: {}", e));
            }
        }
    }

    /// Execute describe operation
    pub(crate) fn execute_describe(&mut self, change_id: &str, message: &str) {
        match self.jj.describe(change_id, message) {
            Ok(_) => {
                self.notification = Some(Notification::success("Description updated"));
                // Refresh log and status to show updated description
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to update description: {}", e));
            }
        }
    }

    /// Execute edit operation (set working-copy to specified change)
    pub(crate) fn execute_edit(&mut self, change_id: &str) {
        match self.jj.edit(change_id) {
            Ok(_) => {
                let short_id = &change_id[..8.min(change_id.len())];
                self.notification =
                    Some(Notification::success(format!("Now editing: {}", short_id)));
                // Refresh log to show @ marker moved
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to edit: {}", e));
            }
        }
    }

    /// Execute new change operation
    pub(crate) fn execute_new_change(&mut self) {
        match self.jj.new_change() {
            Ok(_) => {
                self.notification = Some(Notification::success("Created new change"));
                // Refresh log to show new change
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to create change: {}", e));
            }
        }
    }

    /// Execute commit operation (describe current change + create new change)
    pub(crate) fn execute_commit(&mut self, message: &str) {
        match self.jj.commit(message) {
            Ok(_) => {
                self.notification = Some(Notification::success("Changes committed"));
                // Refresh status view to show clean state
                self.refresh_status();
                // Also refresh log view
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
            }
            Err(e) => {
                self.error_message = Some(format!("Commit failed: {}", e));
            }
        }
    }

    /// Execute squash operation (requires terminal control transfer)
    ///
    /// jj squash may open an editor when both source and destination
    /// have non-empty descriptions. Uses the same interactive pattern
    /// as execute_split to avoid freezing.
    pub(crate) fn execute_squash(&mut self, change_id: &str) {
        use crate::jj::constants::ROOT_CHANGE_ID;
        use crossterm::execute;
        use crossterm::terminal::{
            Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        };
        use std::io::stdout;

        // Guard: cannot squash root commit (has no parent)
        if change_id == ROOT_CHANGE_ID {
            self.notification = Some(Notification::info(
                "Cannot squash: root commit has no parent",
            ));
            return;
        }

        // 1. Exit TUI mode
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Clear(ClearType::All));

        // 2. Scope guard to ensure terminal restoration on any exit path
        let _guard = scopeguard::guard((), |_| {
            let _ = enable_raw_mode();
            let _ = execute!(stdout(), EnterAlternateScreen);
        });

        // 3. Run jj squash (blocking, interactive)
        let result = self.jj.squash_interactive(change_id);

        // 4. Handle result
        match result {
            Ok(status) if status.success() => {
                let short_id = &change_id[..8.min(change_id.len())];
                self.notification = Some(Notification::success(format!(
                    "Squashed {} into parent (undo: u)",
                    short_id
                )));
            }
            Ok(_) => {
                self.notification = Some(Notification::info("Squash cancelled or failed"));
            }
            Err(e) => {
                self.error_message = Some(format!("Squash failed: {}", e));
            }
        }

        // 5. Refresh views
        let revset = self.log_view.current_revset.clone();
        self.refresh_log(revset.as_deref());
        self.refresh_status();
    }

    /// Execute abandon operation (abandon a change)
    pub(crate) fn execute_abandon(&mut self, change_id: &str) {
        use crate::jj::constants::ROOT_CHANGE_ID;

        // Guard: cannot abandon root commit
        if change_id == ROOT_CHANGE_ID {
            self.notification = Some(Notification::info("Cannot abandon: root commit"));
            return;
        }

        match self.jj.abandon(change_id) {
            Ok(_) => {
                let short_id = &change_id[..8.min(change_id.len())];
                self.notification = Some(Notification::success(format!(
                    "Abandoned {} (undo: u)",
                    short_id
                )));
                // Refresh log to show updated state
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                // Also refresh status view (abandon may affect working copy)
                self.refresh_status();
            }
            Err(e) => {
                self.error_message = Some(format!("Abandon failed: {}", e));
            }
        }
    }

    /// Execute redo operation
    ///
    /// Only works if the last operation was an undo.
    pub(crate) fn execute_redo(&mut self) {
        // First, check if last operation was an undo and get the target
        match self.jj.get_redo_target() {
            Ok(Some(op_id)) => {
                match self.jj.redo(&op_id) {
                    Ok(_) => {
                        self.notification = Some(Notification::success("Redo complete"));
                        // Refresh log to show updated state
                        let revset = self.log_view.current_revset.clone();
                        self.refresh_log(revset.as_deref());
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Redo failed: {}", e));
                    }
                }
            }
            Ok(None) => {
                // Not in an undo/redo chain, or multiple consecutive undos
                // Note: After multiple undos, use 'o' (Operation History) to restore
                self.notification = Some(Notification::info(
                    "Nothing to redo (use 'o' for operation history after multiple undos)",
                ));
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to check redo target: {}", e));
            }
        }
    }

    /// Execute operation restore
    ///
    /// **Warning**: This is a destructive operation that modifies repository history.
    /// TODO: Add confirmation dialog (Phase 5.2) before executing.
    /// Currently, users can undo with `u` key if needed.
    pub(crate) fn execute_op_restore(&mut self, operation_id: &str) {
        match self.jj.op_restore(operation_id) {
            Ok(_) => {
                let short_id = &operation_id[..12.min(operation_id.len())];
                self.notification = Some(Notification::success(format!(
                    "Restored to {} (undo: u)",
                    short_id
                )));
                // Refresh log and status after restore
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();
                // Go back to log view
                self.go_to_view(View::Log);
            }
            Err(e) => {
                self.error_message = Some(format!("Restore failed: {}", e));
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
    pub(crate) fn execute_split(&mut self, change_id: &str) {
        use crossterm::execute;
        use crossterm::terminal::{
            Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        };
        use std::io::stdout;

        // 1. Exit TUI mode
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Clear(ClearType::All));

        // 2. Scope guard to ensure terminal restoration on any exit path
        //    (panic, early return, normal completion)
        let _guard = scopeguard::guard((), |_| {
            let _ = enable_raw_mode();
            let _ = execute!(stdout(), EnterAlternateScreen);
        });

        // 3. Run jj split (blocking)
        let result = self.jj.split_interactive(change_id);

        // 4. Handle result and refresh
        // Note: _guard will restore terminal when this function returns
        match result {
            Ok(status) if status.success() => {
                let short_id = &change_id[..8.min(change_id.len())];
                self.notification = Some(Notification::success(format!(
                    "Split {} complete (undo: u)",
                    short_id
                )));
            }
            Ok(_) => {
                self.notification = Some(Notification::info("Split cancelled or failed"));
            }
            Err(e) => {
                self.error_message = Some(format!("Split failed: {}", e));
            }
        }

        // 5. Refresh views
        let revset = self.log_view.current_revset.clone();
        self.refresh_log(revset.as_deref());
        self.refresh_status();
    }

    /// Execute bookmark creation or show move confirmation dialog
    ///
    /// First tries `jj bookmark create`. If the bookmark already exists,
    /// shows a confirmation dialog before moving it.
    pub(crate) fn execute_bookmark_create(&mut self, change_id: &str, name: &str) {
        match self.jj.bookmark_create(name, change_id) {
            Ok(_) => {
                self.notification =
                    Some(Notification::success(format!("Created bookmark: {}", name)));
                // Refresh log to show bookmark
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
            }
            Err(e) => {
                // Check if bookmark already exists - show confirmation dialog
                if is_bookmark_exists_error(&e) {
                    // Show confirmation dialog for moving bookmark
                    self.active_dialog = Some(Dialog::confirm(
                        "Move Bookmark",
                        format!("Move bookmark \"{}\" to this change?", name),
                        Some("Bookmark will be updated.".to_string()),
                        DialogCallback::MoveBookmark {
                            name: name.to_string(),
                            change_id: change_id.to_string(),
                        },
                    ));
                } else {
                    self.error_message = Some(format!("Failed to create bookmark: {}", e));
                }
            }
        }
    }

    /// Execute bookmark move (called after confirmation)
    fn execute_bookmark_move(&mut self, name: &str, change_id: &str) {
        match self.jj.bookmark_set(name, change_id) {
            Ok(_) => {
                self.notification =
                    Some(Notification::success(format!("Moved bookmark: {}", name)));
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to move bookmark: {}", e));
            }
        }
    }

    /// Start bookmark deletion flow (opens dialog)
    ///
    /// Gets bookmarks from the currently selected change in LogView.
    pub(crate) fn start_bookmark_delete(&mut self) {
        // Get bookmarks from currently selected change
        let (change_id, bookmarks) = match self.log_view.selected_change() {
            Some(change) => (change.change_id.clone(), change.bookmarks.clone()),
            None => return,
        };

        if bookmarks.is_empty() {
            self.notification = Some(Notification::info("No bookmarks to delete"));
            return;
        }

        // Create selection dialog
        let items: Vec<SelectItem> = bookmarks
            .iter()
            .map(|name| SelectItem {
                label: name.clone(),
                value: name.clone(),
                selected: false,
            })
            .collect();

        self.active_dialog = Some(Dialog::select(
            "Delete Bookmarks",
            format!(
                "Select bookmarks to delete from {}:",
                &change_id[..8.min(change_id.len())]
            ),
            items,
            Some("Deletions will propagate to remotes on push.".to_string()),
            DialogCallback::DeleteBookmarks,
        ));
    }

    /// Execute bookmark deletion
    pub(crate) fn execute_bookmark_delete(&mut self, names: &[String]) {
        if names.is_empty() {
            return;
        }

        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        match self.jj.bookmark_delete(&name_refs) {
            Ok(_) => {
                let names_str = names.join(", ");
                self.notification = Some(Notification::success(format!(
                    "Deleted bookmarks: {}",
                    names_str
                )));
                // Refresh log
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to delete bookmarks: {}", e));
            }
        }
    }

    /// Execute absorb: move working copy changes into ancestor commits
    ///
    /// Each hunk is moved to the closest mutable ancestor where the
    /// corresponding lines were modified last.
    pub(crate) fn execute_absorb(&mut self) {
        match self.jj.absorb() {
            Ok(output) => {
                // Refresh both log and status
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();

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
                self.error_message = Some(format!("Absorb failed: {}", e));
            }
        }
    }

    /// Resolve a conflict using :ours tool
    pub(crate) fn execute_resolve_ours(&mut self, file_path: &str) {
        let (change_id, is_wc) = match self.resolve_view {
            Some(ref v) => (v.change_id.clone(), v.is_working_copy),
            None => return,
        };

        match self
            .jj
            .resolve_with_tool(file_path, ":ours", Some(&change_id))
        {
            Ok(_) => {
                self.notification = Some(Notification::success(format!(
                    "Resolved {} with :ours",
                    file_path
                )));
                self.refresh_resolve_list(&change_id, is_wc);
            }
            Err(e) => {
                self.error_message = Some(format!("Resolve failed: {}", e));
            }
        }
    }

    /// Resolve a conflict using :theirs tool
    pub(crate) fn execute_resolve_theirs(&mut self, file_path: &str) {
        let (change_id, is_wc) = match self.resolve_view {
            Some(ref v) => (v.change_id.clone(), v.is_working_copy),
            None => return,
        };

        match self
            .jj
            .resolve_with_tool(file_path, ":theirs", Some(&change_id))
        {
            Ok(_) => {
                self.notification = Some(Notification::success(format!(
                    "Resolved {} with :theirs",
                    file_path
                )));
                self.refresh_resolve_list(&change_id, is_wc);
            }
            Err(e) => {
                self.error_message = Some(format!("Resolve failed: {}", e));
            }
        }
    }

    /// Resolve a conflict using external merge tool (@ only)
    ///
    /// Similar to execute_split: temporarily exits TUI mode for interactive tool.
    pub(crate) fn execute_resolve_external(&mut self, file_path: &str) {
        use crossterm::execute;
        use crossterm::terminal::{
            Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        };
        use std::io::stdout;

        let (change_id, is_wc) = match self.resolve_view {
            Some(ref v) => (v.change_id.clone(), v.is_working_copy),
            None => return,
        };

        if !is_wc {
            self.notification = Some(Notification::warning(
                "External merge tool only works for working copy (@)",
            ));
            return;
        }

        // 1. Exit TUI mode
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Clear(ClearType::All));

        // 2. Scope guard to ensure terminal restoration
        let _guard = scopeguard::guard((), |_| {
            let _ = enable_raw_mode();
            let _ = execute!(stdout(), EnterAlternateScreen);
        });

        // 3. Run jj resolve (blocking)
        let result = self.jj.resolve_interactive(file_path, Some(&change_id));

        // 4. Handle result
        match result {
            Ok(status) if status.success() => {
                self.notification = Some(Notification::success(format!("Resolved {}", file_path)));
            }
            Ok(_) => {
                self.notification = Some(Notification::info("Resolve cancelled or failed"));
            }
            Err(e) => {
                self.error_message = Some(format!("Resolve failed: {}", e));
            }
        }

        // 5. Refresh resolve list
        self.refresh_resolve_list(&change_id, is_wc);
    }

    /// Execute rebase: move source change to be a child of destination
    ///
    /// Uses `jj rebase -r <source> -d <destination>` which moves only the
    /// single change. Descendants are rebased onto the original parent.
    pub(crate) fn execute_rebase(&mut self, source: &str, destination: &str) {
        // Prevent rebasing to self
        if source == destination {
            self.notification = Some(Notification::warning("Cannot rebase to itself"));
            return;
        }

        match self.jj.rebase(source, destination) {
            Ok(_) => {
                // Refresh both log and status
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();

                // Check for conflicts in the rebased change (not just working copy)
                let notification = match self.jj.has_conflict(source) {
                    Ok(true) => {
                        Notification::warning("Rebased with conflicts - resolve with jj resolve")
                    }
                    Ok(false) => Notification::success("Rebased successfully"),
                    Err(_) => Notification::warning("Rebase finished, conflict status unknown"),
                };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.error_message = Some(format!("Rebase failed: {}", e));
            }
        }
    }

    /// Handle dialog result
    ///
    /// Called when a dialog is closed.
    ///
    /// Implementation order (important):
    /// 1. Clone callback_id from active_dialog
    /// 2. Set active_dialog to None
    /// 3. Match on callback and result
    pub(crate) fn handle_dialog_result(&mut self, result: DialogResult) {
        // Step 1: Clone callback_id (String clone cost is acceptable)
        let callback = self.active_dialog.as_ref().map(|d| d.callback_id.clone());

        // Step 2: Clear active_dialog (callback is already cloned)
        self.active_dialog = None;

        // Step 3: Match on callback and result
        // Note: For Confirm dialogs, Confirmed(vec) always contains an empty vec.
        //       Only Select dialogs return selected values.
        match (callback, result) {
            (Some(DialogCallback::DeleteBookmarks), DialogResult::Confirmed(names)) => {
                // Select dialog - names contains selected bookmark names
                self.execute_bookmark_delete(&names);
            }
            (
                Some(DialogCallback::MoveBookmark { name, change_id }),
                DialogResult::Confirmed(_),
            ) => {
                // Confirm dialog - execute bookmark move
                self.execute_bookmark_move(&name, &change_id);
            }
            (Some(DialogCallback::OpRestore), DialogResult::Confirmed(_)) => {
                // TODO: Implement op restore with dialog
            }
            (_, DialogResult::Cancelled) => {
                // Cancelled - do nothing
            }
            _ => {}
        }
    }
}

/// Check if a JjError indicates that a bookmark already exists
///
/// This is used to determine whether to fallback from `bookmark create` to `bookmark set`.
fn is_bookmark_exists_error(error: &crate::jj::JjError) -> bool {
    if let crate::jj::JjError::CommandFailed { stderr, .. } = error {
        let stderr_lower = stderr.to_lowercase();
        stderr_lower.contains("already exists") || stderr_lower.contains("bookmark already")
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jj::JjError;

    #[test]
    fn test_is_bookmark_exists_error_with_already_exists() {
        let error = JjError::CommandFailed {
            stderr: "Error: Bookmark 'main' already exists".to_string(),
            exit_code: 1,
        };
        assert!(is_bookmark_exists_error(&error));
    }

    #[test]
    fn test_is_bookmark_exists_error_with_bookmark_already() {
        let error = JjError::CommandFailed {
            stderr: "Error: bookmark already exists: feature".to_string(),
            exit_code: 1,
        };
        assert!(is_bookmark_exists_error(&error));
    }

    #[test]
    fn test_is_bookmark_exists_error_case_insensitive() {
        let error = JjError::CommandFailed {
            stderr: "Error: BOOKMARK ALREADY EXISTS".to_string(),
            exit_code: 1,
        };
        assert!(is_bookmark_exists_error(&error));
    }

    #[test]
    fn test_is_bookmark_exists_error_different_error() {
        let error = JjError::CommandFailed {
            stderr: "Error: Invalid revision".to_string(),
            exit_code: 1,
        };
        assert!(!is_bookmark_exists_error(&error));
    }

    #[test]
    fn test_is_bookmark_exists_error_not_command_failed() {
        let error = JjError::NotARepository;
        assert!(!is_bookmark_exists_error(&error));
    }
}
