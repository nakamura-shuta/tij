//! Application state and view management

use std::cell::Cell;
use std::collections::VecDeque;

use crate::jj::JjExecutor;
use crate::model::{Change, CommandHistory, DiffContent, Notification};
use crate::ui::components::Dialog;
use crate::ui::views::{
    BlameView, BookmarkView, CommandHistoryView, DiffView, EvologView, LogView, OperationView,
    ResolveView, StatusView, TagView, TraceDetailView, WorkspaceView,
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
    Tag,
    Workspace,
    Evolog,
    CommandHistory,
    TraceDetail,
    Help,
}

/// The main application state
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// Current view
    pub current_view: View,
    /// Breadcrumb of ancestor views (root-first) used for back navigation.
    /// `current_view` is NOT included. `Esc`/back pops this; opening a child
    /// view pushes the current one. A single pointer can't represent nested
    /// paths (Log→Status→Diff→Blame) — it looped on `Esc` — so this is a stack.
    pub(crate) view_stack: Vec<View>,
    /// Log view state
    pub log_view: LogView,
    /// Diff view state (created on demand)
    pub diff_view: Option<DiffView>,
    /// Blame view state (created on demand)
    pub blame_view: Option<BlameView>,
    /// Trace Detail View state (Agent Trace A6+A3; None when not open)
    pub trace_detail_view: Option<TraceDetailView>,
    /// Resolve view state (created on demand)
    pub resolve_view: Option<ResolveView>,
    /// Evolog view state (created on demand)
    pub evolog_view: Option<EvologView>,
    /// Bookmark view state
    pub bookmark_view: BookmarkView,
    /// Tag view state
    pub tag_view: TagView,
    /// Workspace view state
    pub workspace_view: WorkspaceView,
    /// Command history view state
    pub command_history_view: CommandHistoryView,
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
    /// Command echo bar enabled (palette `toggle-command-echo`): shows the
    /// last executed jj command on a line above the status bar. Default off —
    /// it costs one screen row (command transparency P2).
    pub command_echo_enabled: bool,
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
    /// Help view: show all views (toggle with 'a')
    pub(crate) help_show_all: bool,
    /// Help view: search input buffer
    pub(crate) help_input_buffer: String,
    /// Command palette active (Log View). When true, keys feed the palette.
    pub(crate) palette_active: bool,
    /// Current palette filter input.
    pub(crate) palette_input: String,
    /// Index of the highlighted command among the filtered list.
    pub(crate) palette_selected: usize,
    /// Dirty flags for lazy refresh
    pub(crate) dirty: DirtyFlags,
    /// Command execution history (for Command History View)
    pub(crate) command_history: CommandHistory,
    /// Change id captured when opening Bookmark/Tag view from Log; the default
    /// `-r` target for in-view `n` (create). `None` means `@`. (Phase 48-B2)
    pub create_target: Option<String>,
    /// Agent Trace index (None = no trace file / unreadable — feature silent)
    pub(crate) trace_index: Option<crate::trace::TraceIndex>,
    /// Whether the "trace file truncated" info was already shown (once per run)
    pub(crate) trace_truncation_notified: bool,
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
            view_stack: Vec::new(),
            log_view: LogView::new(),
            diff_view: None,
            blame_view: None,
            trace_detail_view: None,
            resolve_view: None,
            evolog_view: None,
            bookmark_view: BookmarkView::new(),
            tag_view: TagView::new(),
            workspace_view: WorkspaceView::new(),
            command_history_view: CommandHistoryView::new(),
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
            command_echo_enabled: false,
            preview_auto_disabled: false,
            preview_cache: PreviewCache::new(),
            preview_pending_id: None,
            push_target_remote: None,
            help_scroll: 0,
            help_search_query: None,
            help_search_input: false,
            help_show_all: false,
            palette_active: false,
            palette_input: String::new(),
            palette_selected: 0,
            help_input_buffer: String::new(),
            dirty: DirtyFlags {
                log: false, // Log is loaded in new()
                status: true,
                op_log: true,
                bookmarks: true,
            },
            command_history: CommandHistory::new(),
            create_target: None,
            trace_index: None,
            trace_truncation_notified: false,
        }
    }

    /// Construct a new instance of [`App`].
    ///
    /// Performs pure initialization via [`init()`] then loads the initial log
    /// from jj. Production entry point.
    pub fn new() -> Self {
        let mut app = Self::init();
        app.refresh_log(None);
        // Agent Trace sidecar (silent no-op when absent)
        app.reload_traces();
        // Load preview for the initially selected revision (avoid "No preview available" flash)
        app.update_preview_if_needed();
        app.resolve_pending_preview();
        app
    }

    /// Reload the Agent Trace sidecar file and refresh Log View badges.
    ///
    /// Called at startup and on explicit Log refresh (`Ctrl+L`). Every
    /// failure path (no jj root, unreadable file, missing config) silently
    /// disables the feature — trace problems must never affect tij itself.
    ///
    /// Public for integration tests (which drive App from outside the crate).
    pub fn reload_traces(&mut self) {
        use std::path::PathBuf;

        let Ok(root) = self.jj.workspace_root() else {
            self.trace_index = None;
            self.apply_trace_badges();
            return;
        };

        // Path override via jj config (`tij.*` namespace); relative paths
        // resolve against the workspace root.
        let path = match self.jj.config_get("tij.agent-trace.path") {
            Some(p) => {
                let p = PathBuf::from(p);
                if p.is_absolute() {
                    p
                } else {
                    PathBuf::from(&root).join(p)
                }
            }
            None => PathBuf::from(&root).join(crate::trace::DEFAULT_TRACE_PATH),
        };

        match crate::trace::load(&path) {
            Some(result) => {
                if result.truncated && !self.trace_truncation_notified {
                    self.trace_truncation_notified = true;
                    self.notify_info("Agent trace file exceeds 5 MB; older records skipped");
                }
                self.trace_index = Some(crate::trace::TraceIndex::build(&result.records));
            }
            None => {
                self.trace_index = None;
            }
        }
        self.apply_trace_badges();
    }

    /// Recompute Log View AI badges from the current trace index and changes.
    ///
    /// Cheap (no I/O) — also called after every `refresh_log` so badges track
    /// the latest change list.
    pub(crate) fn apply_trace_badges(&mut self) {
        let badges = self
            .trace_index
            .as_ref()
            .filter(|index| !index.is_empty())
            .map(|index| index.match_commits(&self.log_view.changes))
            .unwrap_or_default();
        self.log_view.set_ai_badges(badges);
    }

    /// Create a new App without running any external commands.
    ///
    /// Pure initialization only — no `jj log` or other subprocess calls.
    /// Use this in unit tests (within the crate) and in integration tests that
    /// need to override `app.jj` before issuing their own refresh calls.
    /// Never use in production code — use [`App::new()`] instead.
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
            View::Tag => View::Log,
            View::Workspace => View::Log,
            View::CommandHistory => View::Log,
            View::TraceDetail => View::Log,
            View::Help => View::Log,
        };
        self.go_to_view(next);
    }

    /// Navigate to a specific view
    ///
    /// Refreshes view data only when the corresponding dirty flag is set.
    /// This avoids unnecessary jj subprocess spawns on Tab switching.
    pub(crate) fn go_to_view(&mut self, view: View) {
        if self.current_view == view {
            return;
        }
        // Maintain the breadcrumb. If `view` is already an ancestor on the
        // stack (e.g. Tab→Log, or a sideways hop back to a view we came
        // through), unwind to it instead of pushing a duplicate — this keeps
        // the stack a true path and prevents `Esc` loops / unbounded growth
        // from Tab cycling. Otherwise we're drilling into a child: push the
        // view we're leaving.
        if let Some(pos) = self.view_stack.iter().position(|&v| v == view) {
            self.view_stack.truncate(pos);
        } else {
            self.view_stack.push(self.current_view);
        }
        self.enter_view(view);
    }

    /// Apply a view transition (palette close, create-target invariant, dirty
    /// refresh) WITHOUT touching the breadcrumb. Shared by `go_to_view`
    /// (forward) and `go_back` (pop) so both run the same enter-side logic.
    fn enter_view(&mut self, view: View) {
        // Close the command palette on view transition (Phase 46-C)
        self.palette_active = false;
        self.palette_input.clear();
        self.palette_selected = 0;
        // Cancel pending preview when leaving Log view
        if self.current_view == View::Log {
            self.preview_pending_id = None;
        }

        // Invariant: `create_target` is captured only on Log→{Bookmark,Tag}.
        // It is cleared on every other transition — including Bookmark→Tag,
        // Tag→Bookmark, or any exit from those views — so a stale change-id
        // from a prior Log selection can never survive unexpected view hops.
        // (Phase 48-B2; defensive rewrite Phase 48-M2)
        if matches!(view, View::Bookmark | View::Tag) {
            // Entering a create-capable view: capture only when coming from Log.
            self.create_target = if self.current_view == View::Log {
                self.log_view
                    .selected_change()
                    .map(|c| c.change_id.to_string())
            } else {
                None
            };
        } else {
            // Leaving to a non-create view: clear any stale target.
            self.create_target = None;
        }

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
                self.help_show_all = false;
                self.help_input_buffer.clear();
            }
            _ => {}
        }
    }

    /// The view `Esc`/back would return to (top of the breadcrumb), if any.
    /// Also the origin view the Help screen documents.
    pub(crate) fn previous_view(&self) -> Option<View> {
        self.view_stack.last().copied()
    }

    /// Go back to the previous view by popping the breadcrumb (defaults to
    /// Log when empty). Runs the same enter-side logic as `go_to_view` but
    /// does NOT push — so nested paths unwind one level at a time instead of
    /// ping-ponging between the last two views.
    pub(crate) fn go_back(&mut self) {
        let target = self.view_stack.pop().unwrap_or(View::Log);
        self.enter_view(target);
    }

    /// Set running to false to quit the application.
    pub(crate) fn quit(&mut self) {
        self.running = false;
    }

    /// Revset for the default create target: captured change id, or `@`. (Phase 48-B2)
    pub(crate) fn create_target_revset(&self) -> String {
        self.create_target
            .clone()
            .unwrap_or_else(|| "@".to_string())
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
    // view-stack back navigation
    // =========================================================================

    #[test]
    fn go_back_pops_the_breadcrumb() {
        let mut app = App::new_for_test();
        // Log → Help: Log is now the breadcrumb top.
        app.go_to_view(View::Help);
        assert_eq!(app.current_view, View::Help);
        assert_eq!(app.previous_view(), Some(View::Log));

        // go_back: Help → Log, breadcrumb now empty (back from Log quits).
        app.go_back();
        assert_eq!(app.current_view, View::Log);
        assert_eq!(app.previous_view(), None);
    }

    #[test]
    fn go_back_defaults_to_log_when_stack_empty() {
        let mut app = App::new_for_test();
        app.current_view = View::Diff;
        app.view_stack.clear();
        app.go_back();
        assert_eq!(app.current_view, View::Log);
    }

    #[test]
    fn nested_back_unwinds_one_level_no_loop() {
        // Regression: Log → Diff → Blame. Esc must go Blame→Diff→Log, NOT
        // ping-pong between Diff and Blame (the single-pointer bug).
        let mut app = App::new_for_test();
        app.go_to_view(View::Diff);
        app.go_to_view(View::Blame);
        assert_eq!(app.view_stack, vec![View::Log, View::Diff]);

        app.go_back();
        assert_eq!(app.current_view, View::Diff, "Blame → Diff");
        app.go_back();
        assert_eq!(
            app.current_view,
            View::Log,
            "Diff → Log (not back to Blame)"
        );
    }

    #[test]
    fn nested_back_through_status_reaches_true_parent() {
        // Log → Status → Diff → Blame unwinds through every real parent.
        let mut app = App::new_for_test();
        app.go_to_view(View::Status);
        app.go_to_view(View::Diff);
        app.go_to_view(View::Blame);

        app.go_back();
        assert_eq!(app.current_view, View::Diff);
        app.go_back();
        assert_eq!(app.current_view, View::Status, "Diff → Status, not Log");
        app.go_back();
        assert_eq!(app.current_view, View::Log);
    }

    #[test]
    fn revisiting_ancestor_unwinds_instead_of_growing() {
        // Tab back to an ancestor (Diff→Log) must truncate the breadcrumb,
        // not push a duplicate that would make Esc bounce.
        let mut app = App::new_for_test();
        app.go_to_view(View::Diff); // stack [Log]
        app.go_to_view(View::Log); // Log is an ancestor → unwind
        assert_eq!(app.current_view, View::Log);
        assert!(app.view_stack.is_empty(), "breadcrumb unwound to root");
    }

    // =========================================================================
    // go_to_view create_target invariant tests (Phase 48-M2)
    // =========================================================================

    #[test]
    fn go_to_view_bookmark_to_tag_clears_stale_create_target() {
        // Bookmark→Tag is currently unreachable via UI keys, but exercising it
        // here ensures the destination-based logic holds for future view additions.
        let mut app = App::new_for_test();
        // Pre-set a stale target as if an earlier Log→Bookmark transition captured one.
        app.create_target = Some("STALE".to_string());
        // Simulate being in Bookmark view (go_to_view guards on current != target).
        app.current_view = View::Bookmark;
        // Transition to Tag: not coming from Log, so target must be cleared.
        app.go_to_view(View::Tag);
        assert_eq!(
            app.create_target, None,
            "Bookmark→Tag must clear create_target; stale id survived the transition"
        );
    }

    #[test]
    fn go_to_view_tag_to_bookmark_clears_stale_create_target() {
        let mut app = App::new_for_test();
        app.create_target = Some("STALE".to_string());
        app.current_view = View::Tag;
        app.go_to_view(View::Bookmark);
        assert_eq!(
            app.create_target, None,
            "Tag→Bookmark must clear create_target"
        );
    }

    #[test]
    fn go_to_view_non_log_to_bookmark_clears_create_target() {
        let mut app = App::new_for_test();
        app.create_target = Some("STALE".to_string());
        app.current_view = View::Status;
        app.go_to_view(View::Bookmark);
        assert_eq!(
            app.create_target, None,
            "non-Log→Bookmark must not carry stale target"
        );
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
            change_id: crate::model::ChangeId::new("aaa".to_string()),
            commit_id: crate::model::CommitId::new("c1".to_string()),
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
            change_id: crate::model::ChangeId::new("aaa".to_string()),
            commit_id: crate::model::CommitId::new("c2".to_string()), // different commit_id
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
            change_id: crate::model::ChangeId::new("aaa".to_string()),
            commit_id: crate::model::CommitId::new("c1".to_string()),
            is_graph_only: true,
            ..Change::default()
        }];

        cache.validate(&changes);
        // Entry evicted because it only matches a graph-only line
        assert_eq!(cache.len(), 0);
    }

    // =========================================================================
    // update_preview_if_needed scheduling tests
    // =========================================================================

    #[test]
    fn update_preview_schedules_pending_on_cache_miss() {
        let mut app = App::new_for_test();
        app.log_view.set_changes(vec![Change {
            change_id: crate::model::ChangeId::new("aaa".to_string()),
            commit_id: crate::model::CommitId::new("c1".to_string()),
            ..Change::default()
        }]);
        app.preview_enabled = true;

        assert!(app.preview_pending_id.is_none());
        app.update_preview_if_needed();
        assert_eq!(app.preview_pending_id.as_deref(), Some("aaa"));
    }

    #[test]
    fn update_preview_skips_when_cache_hit() {
        let mut app = App::new_for_test();
        app.log_view.set_changes(vec![Change {
            change_id: crate::model::ChangeId::new("aaa".to_string()),
            commit_id: crate::model::CommitId::new("c1".to_string()),
            ..Change::default()
        }]);
        app.preview_enabled = true;
        app.preview_cache.insert(make_entry("aaa", "c1"));

        app.update_preview_if_needed();
        assert!(app.preview_pending_id.is_none());
    }

    #[test]
    fn update_preview_schedules_on_stale_commit_id() {
        let mut app = App::new_for_test();
        app.log_view.set_changes(vec![Change {
            change_id: crate::model::ChangeId::new("aaa".to_string()),
            commit_id: crate::model::CommitId::new("c2".to_string()), // new commit_id
            ..Change::default()
        }]);
        app.preview_enabled = true;
        app.preview_cache.insert(make_entry("aaa", "c1")); // old commit_id

        app.update_preview_if_needed();
        assert_eq!(app.preview_pending_id.as_deref(), Some("aaa"));
    }

    #[test]
    fn update_preview_noop_when_disabled() {
        let mut app = App::new_for_test();
        app.log_view.set_changes(vec![Change {
            change_id: crate::model::ChangeId::new("aaa".to_string()),
            commit_id: crate::model::CommitId::new("c1".to_string()),
            ..Change::default()
        }]);
        app.preview_enabled = false;

        app.update_preview_if_needed();
        assert!(app.preview_pending_id.is_none());
    }
}
