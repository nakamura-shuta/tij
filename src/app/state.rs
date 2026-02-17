//! Application state and view management

use std::cell::Cell;

use crate::jj::JjExecutor;
use crate::model::{DiffContent, Notification};
use crate::ui::components::Dialog;
use crate::ui::views::{
    BlameView, BookmarkView, DiffView, EvologView, LogView, OperationView, ResolveView, StatusView,
};

/// Cached preview to avoid refetching on every render
#[derive(Debug)]
pub(crate) struct PreviewCache {
    pub change_id: String,
    pub content: DiffContent,
    /// Bookmarks captured at fetch time (from Change model, not jj show)
    pub bookmarks: Vec<String>,
}

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
    Bookmark,
    Evolog,
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
    /// Evolog view state (created on demand)
    pub evolog_view: Option<EvologView>,
    /// Bookmark view state
    pub bookmark_view: BookmarkView,
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
    /// Pending bookmark forget name (Confirm dialog)
    pub(crate) pending_forget_bookmark: Option<String>,
    /// Pending jump target from Blame View (for 2-step J: first shows hint, second expands revset)
    pub(crate) pending_jump_change_id: Option<String>,
    /// Preview pane enabled (p key toggle) — represents user intent
    pub preview_enabled: bool,
    /// Preview auto-disabled due to small terminal (render-time flag, does not override user intent)
    pub(crate) preview_auto_disabled: bool,
    /// Cached preview content (change_id → DiffContent + bookmarks)
    pub(crate) preview_cache: Option<PreviewCache>,
    /// Pending preview fetch (deferred to idle tick)
    pub(crate) preview_pending_id: Option<String>,
    /// Selected remote for push (None = default remote)
    ///
    /// Cleared on all exit paths: push success/error (via `take()` at top of
    /// `execute_push`), remote selection cancel, bookmark selection cancel.
    pub(crate) push_target_remote: Option<String>,
    /// Help view scroll offset (line-based)
    pub(crate) help_scroll: u16,
    /// Help view: active search query (for highlighting and n/N navigation)
    pub(crate) help_search_query: Option<String>,
    /// Help view: search input mode active
    pub(crate) help_search_input: bool,
    /// Help view: search input buffer
    pub(crate) help_input_buffer: String,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Pure initialization without any external command execution.
    ///
    /// Used by both `new()` (production) and `new_for_test()` (tests).
    fn init() -> Self {
        Self {
            running: true,
            current_view: View::Log,
            previous_view: None,
            log_view: LogView::new(),
            diff_view: None,
            blame_view: None,
            resolve_view: None,
            evolog_view: None,
            bookmark_view: BookmarkView::new(),
            status_view: StatusView::new(),
            operation_view: OperationView::new(),
            jj: JjExecutor::new(),
            error_message: None,
            notification: None,
            last_frame_height: Cell::new(24), // Default terminal height
            active_dialog: None,
            pending_push_bookmarks: Vec::new(),
            pending_forget_bookmark: None,
            pending_jump_change_id: None,
            preview_enabled: true,
            preview_auto_disabled: false,
            preview_cache: None,
            preview_pending_id: None,
            push_target_remote: None,
            help_scroll: 0,
            help_search_query: None,
            help_search_input: false,
            help_input_buffer: String::new(),
        }
    }

    /// Construct a new instance of [`App`].
    ///
    /// Performs pure initialization via [`init()`] then loads the initial log
    /// from jj. Production entry point.
    pub fn new() -> Self {
        let mut app = Self::init();
        app.refresh_log(None);
        app
    }

    /// Create a new App for unit tests.
    ///
    /// Pure initialization only — no `jj log` or other external commands.
    /// Safe to use in CI environments without a jj repository.
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::init()
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
            View::Bookmark => View::Log,
            View::Evolog => View::Log,
            View::Help => View::Log,
        };
        self.go_to_view(next);
    }

    /// Navigate to a specific view
    pub(crate) fn go_to_view(&mut self, view: View) {
        if self.current_view != view {
            // Cancel pending preview when leaving Log view
            if self.current_view == View::Log {
                self.preview_pending_id = None;
            }

            self.previous_view = Some(self.current_view);
            self.current_view = view;

            // Refresh data / reset state when entering certain views
            match view {
                View::Status => self.refresh_status(),
                View::Operation => self.refresh_operation_log(),
                View::Help => {
                    self.help_scroll = 0;
                    self.help_search_query = None;
                    self.help_search_input = false;
                    self.help_input_buffer.clear();
                }
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
        if let Some(ref notification) = self.notification
            && notification.is_expired()
        {
            self.notification = None;
        }
    }
}
