//! Application state and view management

use std::cell::Cell;

use crate::jj::JjExecutor;
use crate::model::Notification;
use crate::ui::components::Dialog;
use crate::ui::views::{BlameView, DiffView, LogView, OperationView, ResolveView, StatusView};

/// Available views in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Log,
    Diff,
    Status,
    Operation,
    Blame,
    Resolve,
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
    /// Blame view state (created on demand)
    pub blame_view: Option<BlameView>,
    /// Resolve view state (created on demand)
    pub resolve_view: Option<ResolveView>,
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
    /// Bookmark names pending for push (Confirm dialog only; Select dialog uses DialogResult names)
    pub(crate) pending_push_bookmarks: Vec<String>,
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
            blame_view: None,
            resolve_view: None,
            status_view: StatusView::new(),
            operation_view: OperationView::new(),
            jj: JjExecutor::new(),
            error_message: None,
            notification: None,
            last_frame_height: Cell::new(24), // Default terminal height
            active_dialog: None,
            pending_push_bookmarks: Vec::new(),
        };

        // Load initial log
        app.refresh_log(None);

        app
    }

    /// Switch to next view (Tab key)
    pub(crate) fn next_view(&mut self) {
        let next = match self.current_view {
            View::Log => View::Status,
            View::Status => View::Log,
            View::Diff => View::Log,
            View::Operation => View::Log,
            View::Blame => View::Log,
            View::Resolve => View::Log,
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

    /// Clear expired notification
    pub(crate) fn clear_expired_notification(&mut self) {
        if let Some(ref notification) = self.notification {
            if notification.is_expired() {
                self.notification = None;
            }
        }
    }
}
