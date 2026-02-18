//! Application state and view management

use std::cell::Cell;
use std::collections::VecDeque;

use crate::jj::JjExecutor;
use crate::model::{Change, DiffContent, Notification};
use crate::ui::components::Dialog;
use crate::ui::views::{
    BlameView, BookmarkView, DiffView, EvologView, LogView, OperationView, ResolveView, StatusView,
};

/// Tracks which data needs refreshing after a jj operation.
///
/// All write operations set `op_log: true` since they create a new jj operation.
/// Use the convenience constructors to create flags for specific operations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DirtyFlags {
    pub log: bool,
    pub status: bool,
    pub op_log: bool,
    pub bookmarks: bool,
}

impl DirtyFlags {
    /// Log and operation log (metadata-only changes like describe)
    pub fn log() -> Self {
        Self {
            log: true,
            op_log: true,
            ..Default::default()
        }
    }

    /// Log and status (most write operations)
    pub fn log_and_status() -> Self {
        Self {
            log: true,
            status: true,
            op_log: true,
            ..Default::default()
        }
    }

    /// Log and bookmarks (bookmark create/delete/move)
    pub fn log_and_bookmarks() -> Self {
        Self {
            log: true,
            bookmarks: true,
            op_log: true,
            ..Default::default()
        }
    }

    /// All flags dirty (fetch, undo, redo, op_restore)
    pub fn all() -> Self {
        Self {
            log: true,
            status: true,
            op_log: true,
            bookmarks: true,
        }
    }
}

const PREVIEW_CACHE_CAPACITY: usize = 8;

/// Single preview cache entry
#[derive(Debug)]
pub(crate) struct PreviewCacheEntry {
    pub change_id: String,
    pub commit_id: String,
    pub content: DiffContent,
    pub bookmarks: Vec<String>,
}

/// LRU preview cache (VecDeque: front=LRU, back=MRU)
#[derive(Debug)]
pub(crate) struct PreviewCache {
    entries: VecDeque<PreviewCacheEntry>,
    capacity: usize,
}

impl PreviewCache {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: PREVIEW_CACHE_CAPACITY,
        }
    }

    /// Search for an entry by change_id (read-only, no MRU promotion)
    pub fn peek(&self, change_id: &str) -> Option<&PreviewCacheEntry> {
        self.entries.iter().find(|e| e.change_id == change_id)
    }

    /// Promote entry to MRU position (back of deque)
    pub fn touch(&mut self, change_id: &str) {
        if let Some(pos) = self.entries.iter().position(|e| e.change_id == change_id) {
            let entry = self.entries.remove(pos).unwrap();
            self.entries.push_back(entry);
        }
    }

    /// Insert or replace an entry. Evicts LRU if at capacity.
    pub fn insert(&mut self, entry: PreviewCacheEntry) {
        // Remove existing entry with same change_id
        self.entries.retain(|e| e.change_id != entry.change_id);
        // Evict LRU if at capacity
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Remove a specific entry by change_id
    pub fn remove(&mut self, change_id: &str) {
        self.entries.retain(|e| e.change_id != change_id);
    }

    /// Validate cache entries against the current Change list.
    ///
    /// Entries whose commit_id no longer matches (or are absent from the list)
    /// are evicted. Entries that match get their bookmarks updated.
    pub fn validate(&mut self, changes: &[Change]) {
        self.entries.retain_mut(|entry| {
            // Find matching change (skip graph-only lines)
            if let Some(change) = changes
                .iter()
                .filter(|c| !c.is_graph_only)
                .find(|c| c.change_id == entry.change_id)
            {
                if change.commit_id == entry.commit_id {
                    // Content unchanged — update bookmarks
                    entry.bookmarks = change.bookmarks.clone();
                    true
                } else {
                    // commit_id changed — content is stale
                    false
                }
            } else {
                // Not in current log — evict
                false
            }
        });
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of cached entries (for tests)
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
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
    /// LRU preview cache (change_id → DiffContent + commit_id + bookmarks)
    pub(crate) preview_cache: PreviewCache,
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
    /// Dirty flags for lazy refresh
    pub(crate) dirty: DirtyFlags,
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
            preview_cache: PreviewCache::new(),
            preview_pending_id: None,
            push_target_remote: None,
            help_scroll: 0,
            help_search_query: None,
            help_search_input: false,
            help_input_buffer: String::new(),
            dirty: DirtyFlags {
                log: false, // Log is loaded in new()
                status: true,
                op_log: true,
                bookmarks: true,
            },
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
    ///
    /// Refreshes view data only when the corresponding dirty flag is set.
    /// This avoids unnecessary jj subprocess spawns on Tab switching.
    pub(crate) fn go_to_view(&mut self, view: View) {
        if self.current_view != view {
            // Cancel pending preview when leaving Log view
            if self.current_view == View::Log {
                self.preview_pending_id = None;
            }

            self.previous_view = Some(self.current_view);
            self.current_view = view;

            // Refresh data only when dirty, reset state when entering certain views
            match view {
                View::Log if self.dirty.log => {
                    let revset = self.log_view.current_revset.clone();
                    self.refresh_log(revset.as_deref());
                    self.dirty.log = false;
                }
                View::Status if self.dirty.status => {
                    self.refresh_status();
                    self.dirty.status = false;
                }
                View::Operation if self.dirty.op_log => {
                    self.refresh_operation_log();
                    self.dirty.op_log = false;
                }
                View::Bookmark if self.dirty.bookmarks => {
                    self.refresh_bookmark_view();
                    self.dirty.bookmarks = false;
                }
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
    ///
    /// Routes through `go_to_view()` to ensure dirty flags are checked.
    pub(crate) fn go_back(&mut self) {
        let target = self.previous_view.take().unwrap_or(View::Log);
        self.go_to_view(target);
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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // DirtyFlags constructor tests
    // =========================================================================

    #[test]
    fn dirty_flags_log_includes_op_log() {
        let flags = DirtyFlags::log();
        assert!(flags.log);
        assert!(flags.op_log);
        assert!(!flags.status);
        assert!(!flags.bookmarks);
    }

    #[test]
    fn dirty_flags_log_and_status_includes_op_log() {
        let flags = DirtyFlags::log_and_status();
        assert!(flags.log);
        assert!(flags.status);
        assert!(flags.op_log);
        assert!(!flags.bookmarks);
    }

    #[test]
    fn dirty_flags_log_and_bookmarks_includes_op_log() {
        let flags = DirtyFlags::log_and_bookmarks();
        assert!(flags.log);
        assert!(!flags.status);
        assert!(flags.op_log);
        assert!(flags.bookmarks);
    }

    #[test]
    fn dirty_flags_all_sets_everything() {
        let flags = DirtyFlags::all();
        assert!(flags.log);
        assert!(flags.status);
        assert!(flags.op_log);
        assert!(flags.bookmarks);
    }

    #[test]
    fn dirty_flags_default_is_all_false() {
        let flags = DirtyFlags::default();
        assert!(!flags.log);
        assert!(!flags.status);
        assert!(!flags.op_log);
        assert!(!flags.bookmarks);
    }

    // =========================================================================
    // go_to_view dirty flag tests
    // =========================================================================

    #[test]
    fn go_to_view_status_skips_refresh_when_not_dirty() {
        let mut app = App::new_for_test();
        app.dirty.status = false;
        app.go_to_view(View::Status);
        // Should reach Status view without error (no jj command needed)
        assert_eq!(app.current_view, View::Status);
    }

    #[test]
    fn go_to_view_operation_skips_refresh_when_not_dirty() {
        let mut app = App::new_for_test();
        app.dirty.op_log = false;
        app.go_to_view(View::Operation);
        assert_eq!(app.current_view, View::Operation);
    }

    // =========================================================================
    // go_back routes through go_to_view
    // =========================================================================

    #[test]
    fn go_back_sets_previous_view() {
        let mut app = App::new_for_test();
        // Simulate: Log → Help (so previous = Log)
        app.go_to_view(View::Help);
        assert_eq!(app.current_view, View::Help);
        assert_eq!(app.previous_view, Some(View::Log));

        // go_back: Help → Log (via go_to_view, so previous = Help)
        app.go_back();
        assert_eq!(app.current_view, View::Log);
        assert_eq!(app.previous_view, Some(View::Help));
    }

    #[test]
    fn go_back_defaults_to_log_when_no_previous() {
        let mut app = App::new_for_test();
        app.current_view = View::Diff;
        app.previous_view = None;
        app.go_back();
        assert_eq!(app.current_view, View::Log);
    }

    // =========================================================================
    // App::init dirty flag initialization
    // =========================================================================

    #[test]
    fn init_dirty_flags() {
        let app = App::new_for_test();
        // Log is false because new() loads it; status/op_log/bookmarks are true
        assert!(!app.dirty.log);
        assert!(app.dirty.status);
        assert!(app.dirty.op_log);
        assert!(app.dirty.bookmarks);
    }

    // =========================================================================
    // PreviewCache LRU tests
    // =========================================================================

    fn make_entry(change_id: &str, commit_id: &str) -> PreviewCacheEntry {
        PreviewCacheEntry {
            change_id: change_id.to_string(),
            commit_id: commit_id.to_string(),
            content: crate::model::DiffContent::default(),
            bookmarks: vec![],
        }
    }

    #[test]
    fn preview_cache_insert_and_peek() {
        let mut cache = PreviewCache::new();
        assert_eq!(cache.len(), 0);

        cache.insert(make_entry("aaa", "c1"));
        assert_eq!(cache.len(), 1);
        assert!(cache.peek("aaa").is_some());
        assert!(cache.peek("bbb").is_none());
    }

    #[test]
    fn preview_cache_insert_replaces_same_change_id() {
        let mut cache = PreviewCache::new();
        cache.insert(make_entry("aaa", "c1"));
        cache.insert(make_entry("aaa", "c2"));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.peek("aaa").unwrap().commit_id, "c2");
    }

    #[test]
    fn preview_cache_evicts_lru_at_capacity() {
        let mut cache = PreviewCache::new();
        // Insert 8 entries (capacity)
        for i in 0..8 {
            cache.insert(make_entry(&format!("id{}", i), &format!("c{}", i)));
        }
        assert_eq!(cache.len(), 8);

        // Insert 9th → evicts id0 (LRU, front)
        cache.insert(make_entry("id8", "c8"));
        assert_eq!(cache.len(), 8);
        assert!(cache.peek("id0").is_none());
        assert!(cache.peek("id8").is_some());
    }

    #[test]
    fn preview_cache_touch_promotes_to_mru() {
        let mut cache = PreviewCache::new();
        for i in 0..8 {
            cache.insert(make_entry(&format!("id{}", i), &format!("c{}", i)));
        }

        // Touch id0 (currently LRU) → promotes to MRU
        cache.touch("id0");

        // Insert 9th → should evict id1 (new LRU), not id0
        cache.insert(make_entry("id8", "c8"));
        assert_eq!(cache.len(), 8);
        assert!(cache.peek("id0").is_some()); // promoted, not evicted
        assert!(cache.peek("id1").is_none()); // new LRU, evicted
    }

    #[test]
    fn preview_cache_remove() {
        let mut cache = PreviewCache::new();
        cache.insert(make_entry("aaa", "c1"));
        cache.insert(make_entry("bbb", "c2"));
        assert_eq!(cache.len(), 2);

        cache.remove("aaa");
        assert_eq!(cache.len(), 1);
        assert!(cache.peek("aaa").is_none());
        assert!(cache.peek("bbb").is_some());
    }

    #[test]
    fn preview_cache_clear() {
        let mut cache = PreviewCache::new();
        cache.insert(make_entry("aaa", "c1"));
        cache.insert(make_entry("bbb", "c2"));
        cache.clear();
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn preview_cache_validate_keeps_matching() {
        let mut cache = PreviewCache::new();
        cache.insert(PreviewCacheEntry {
            change_id: "aaa".to_string(),
            commit_id: "c1".to_string(),
            content: crate::model::DiffContent::default(),
            bookmarks: vec!["old-bm".to_string()],
        });

        let changes = vec![Change {
            change_id: "aaa".to_string(),
            commit_id: "c1".to_string(),
            bookmarks: vec!["new-bm".to_string()],
            ..Change::default()
        }];

        cache.validate(&changes);
        assert_eq!(cache.len(), 1);
        // Bookmarks should be updated
        assert_eq!(
            cache.peek("aaa").unwrap().bookmarks,
            vec!["new-bm".to_string()]
        );
    }

    #[test]
    fn preview_cache_validate_evicts_stale_commit() {
        let mut cache = PreviewCache::new();
        cache.insert(make_entry("aaa", "c1"));

        let changes = vec![Change {
            change_id: "aaa".to_string(),
            commit_id: "c2".to_string(), // different commit_id
            ..Change::default()
        }];

        cache.validate(&changes);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn preview_cache_validate_evicts_absent() {
        let mut cache = PreviewCache::new();
        cache.insert(make_entry("aaa", "c1"));

        // Empty change list → entry absent → evicted
        cache.validate(&[]);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn preview_cache_validate_skips_graph_only() {
        let mut cache = PreviewCache::new();
        cache.insert(make_entry("aaa", "c1"));

        // Graph-only line with matching change_id should be ignored
        let changes = vec![Change {
            change_id: "aaa".to_string(),
            commit_id: "c1".to_string(),
            is_graph_only: true,
            ..Change::default()
        }];

        cache.validate(&changes);
        // Entry evicted because it only matches a graph-only line
        assert_eq!(cache.len(), 0);
    }
}
