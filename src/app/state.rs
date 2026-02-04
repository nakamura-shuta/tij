//! Application state and view management

use std::cell::Cell;

use crate::jj::JjExecutor;
use crate::model::Notification;
use crate::ui::components::{Dialog, DialogCallback, DialogResult, SelectItem};
use crate::ui::views::{DiffView, LogView, OperationView, StatusView};

/// Available views in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Log,
    Diff,
    Status,
    Operation,
    Help,
}

/// The main application state
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// Current view
    pub current_view: View,
    /// Previous view (for back navigation)
    pub(crate) previous_view: Option<View>,
    /// Log view state
    pub log_view: LogView,
    /// Diff view state (created on demand)
    pub diff_view: Option<DiffView>,
    /// Status view state
    pub status_view: StatusView,
    /// Operation history view state
    pub operation_view: OperationView,
    /// jj executor
    pub jj: JjExecutor,
    /// Error message to display
    pub error_message: Option<String>,
    /// Notification to display (success/info/warning messages)
    pub notification: Option<Notification>,
    /// Last known frame height (updated during render, uses Cell for interior mutability)
    pub(crate) last_frame_height: Cell<u16>,
    /// Active dialog (blocks other input when Some)
    pub active_dialog: Option<Dialog>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new() -> Self {
        let mut app = Self {
            running: true,
            current_view: View::Log,
            previous_view: None,
            log_view: LogView::new(),
            diff_view: None,
            status_view: StatusView::new(),
            operation_view: OperationView::new(),
            jj: JjExecutor::new(),
            error_message: None,
            notification: None,
            last_frame_height: Cell::new(24), // Default terminal height
            active_dialog: None,
        };

        // Load initial log
        app.refresh_log(None);

        app
    }

    /// Refresh the log view with optional revset
    pub fn refresh_log(&mut self, revset: Option<&str>) {
        match self.jj.log_changes(revset) {
            Ok(changes) => {
                self.log_view.set_changes(changes);
                self.log_view.current_revset = revset.map(|s| s.to_string());
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("jj error: {}", e));
            }
        }
    }

    /// Switch to next view (Tab key)
    pub(crate) fn next_view(&mut self) {
        let next = match self.current_view {
            View::Log => View::Status,
            View::Status => View::Log,
            View::Diff => View::Log,
            View::Operation => View::Log,
            View::Help => View::Log,
        };
        self.go_to_view(next);
    }

    /// Navigate to a specific view
    pub(crate) fn go_to_view(&mut self, view: View) {
        if self.current_view != view {
            self.previous_view = Some(self.current_view);
            self.current_view = view;

            // Refresh data when entering certain views
            match view {
                View::Status => self.refresh_status(),
                View::Operation => self.refresh_operation_log(),
                _ => {}
            }
        }
    }

    /// Refresh the status view
    pub fn refresh_status(&mut self) {
        match self.jj.status() {
            Ok(status) => {
                self.status_view.set_status(status);
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("jj status error: {}", e));
            }
        }
    }

    /// Go back to previous view
    pub(crate) fn go_back(&mut self) {
        if let Some(prev) = self.previous_view.take() {
            self.current_view = prev;
        } else {
            self.current_view = View::Log;
        }
    }

    /// Set running to false to quit the application.
    pub(crate) fn quit(&mut self) {
        self.running = false;
    }

    /// Open diff view for a specific change
    pub(crate) fn open_diff(&mut self, change_id: &str) {
        match self.jj.show(change_id) {
            Ok(content) => {
                self.diff_view = Some(DiffView::new(change_id.to_string(), content));
                self.go_to_view(View::Diff);
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to load diff: {}", e));
            }
        }
    }

    /// Open diff view for a specific change and jump to a file
    pub(crate) fn open_diff_at_file(&mut self, change_id: &str, file_path: &str) {
        match self.jj.show(change_id) {
            Ok(content) => {
                let mut diff_view = DiffView::new(change_id.to_string(), content);
                // Jump to the specified file
                diff_view.jump_to_file(file_path);
                self.diff_view = Some(diff_view);
                self.go_to_view(View::Diff);
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to load diff: {}", e));
            }
        }
    }

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

    /// Execute describe operation
    pub(crate) fn execute_describe(&mut self, change_id: &str, message: &str) {
        match self.jj.describe(change_id, message) {
            Ok(_) => {
                self.notification = Some(Notification::success("Description updated"));
                // Refresh log to show updated description
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
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

    /// Execute squash operation (squash change into its parent)
    pub(crate) fn execute_squash(&mut self, change_id: &str) {
        use crate::jj::constants::ROOT_CHANGE_ID;

        // Guard: cannot squash root commit (has no parent)
        if change_id == ROOT_CHANGE_ID {
            self.notification = Some(Notification::info(
                "Cannot squash: root commit has no parent",
            ));
            return;
        }

        match self.jj.squash(change_id) {
            Ok(_) => {
                let short_id = &change_id[..8.min(change_id.len())];
                self.notification = Some(Notification::success(format!(
                    "Squashed {} into parent (undo: u)",
                    short_id
                )));
                // Refresh log to show updated state
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                // Also refresh status view (squash may affect working copy)
                self.refresh_status();
            }
            Err(e) => {
                self.error_message = Some(format!("Squash failed: {}", e));
            }
        }
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

    /// Clear expired notification
    pub(crate) fn clear_expired_notification(&mut self) {
        if let Some(ref notification) = self.notification {
            if notification.is_expired() {
                self.notification = None;
            }
        }
    }

    /// Execute refresh for current view (Ctrl+L)
    ///
    /// Refreshes the data for the current view:
    /// - Log View: reloads commit log (preserves revset filter)
    /// - Status View: reloads file status
    /// - Operation View: reloads operation history
    /// - Diff View: reloads diff for current change (if loaded)
    /// - Help View: no-op (static content)
    ///
    /// Note: Selection position is NOT preserved after refresh.
    pub(crate) fn execute_refresh(&mut self) {
        match self.current_view {
            View::Log => {
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.notification = Some(Notification::info("Refreshed"));
            }
            View::Status => {
                self.refresh_status();
                self.notification = Some(Notification::info("Refreshed"));
            }
            View::Operation => {
                self.refresh_operation_log();
                self.notification = Some(Notification::info("Refreshed"));
            }
            View::Diff => {
                // Only refresh if diff_view is loaded
                if let Some(ref diff_view) = self.diff_view {
                    let change_id = diff_view.change_id.clone();
                    self.open_diff(&change_id);
                    self.notification = Some(Notification::info("Refreshed"));
                }
                // If diff_view is None, do nothing (no notification)
            }
            View::Help => {
                // Help is static content, no refresh needed, no notification
            }
        }
    }

    /// Refresh the operation history view
    pub fn refresh_operation_log(&mut self) {
        match self.jj.op_log(Some(50)) {
            Ok(operations) => {
                self.operation_view.set_operations(operations);
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("jj op log error: {}", e));
            }
        }
    }

    /// Open operation history view
    pub(crate) fn open_operation_history(&mut self) {
        self.go_to_view(View::Operation);
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
                let has_conflict = self.jj.has_conflict(source).unwrap_or(false);
                if has_conflict {
                    self.notification = Some(Notification::warning(
                        "Rebased with conflicts - resolve with jj resolve",
                    ));
                } else {
                    self.notification = Some(Notification::success("Rebased successfully"));
                }
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
