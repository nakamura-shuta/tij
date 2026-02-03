//! Application state and view management

use std::cell::Cell;

use crate::jj::JjExecutor;
use crate::model::Notification;
use crate::ui::views::{DiffView, LogView, StatusView};

/// Available views in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Log,
    Diff,
    Status,
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
    /// jj executor
    pub jj: JjExecutor,
    /// Error message to display
    pub error_message: Option<String>,
    /// Notification to display (success/info/warning messages)
    pub notification: Option<Notification>,
    /// Last known frame height (updated during render, uses Cell for interior mutability)
    pub(crate) last_frame_height: Cell<u16>,
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
            jj: JjExecutor::new(),
            error_message: None,
            notification: None,
            last_frame_height: Cell::new(24), // Default terminal height
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
            if view == View::Status {
                self.refresh_status();
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
                    "Squashed {} into parent",
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
}
