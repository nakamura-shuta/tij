//! Log View - displays jj log output
//!
//! The main view of Tij, showing the change history.

mod input;
mod render;

use crate::model::Change;

/// Rebase operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RebaseMode {
    /// `-r`: Move single revision (descendants rebased onto parent)
    #[default]
    Revision,
    /// `-s`: Move revision and all descendants together
    Source,
    /// `-A`: Insert revision after target in history
    InsertAfter,
    /// `-B`: Insert revision before target in history
    InsertBefore,
}

/// Input mode for Log View
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Normal navigation mode
    #[default]
    Normal,
    /// Text search input mode (for n/N navigation)
    SearchInput,
    /// Revset input mode (for jj filtering)
    RevsetInput,
    /// Describe input mode (editing change description)
    DescribeInput,
    /// Bookmark input mode (creating bookmark)
    BookmarkInput,
    /// Rebase mode selection (r/s/A/B single key)
    RebaseModeSelect,
    /// Rebase destination selection mode
    RebaseSelect,
    /// Squash destination selection mode
    SquashSelect,
    /// Compare revision selection mode (select second revision)
    CompareSelect,
}

impl InputMode {
    pub fn input_bar_meta(self) -> Option<(&'static str, &'static str)> {
        match self {
            InputMode::SearchInput => Some(("Search: ", " / Search ")),
            InputMode::RevsetInput => Some(("Revset: ", " r Revset ")),
            InputMode::DescribeInput => Some(("Describe: ", " d Describe ")),
            InputMode::BookmarkInput => Some(("Bookmark: ", " b Bookmark ")),
            // RebaseModeSelect/RebaseSelect/SquashSelect/CompareSelect use status bar hints, not input bar
            InputMode::Normal
            | InputMode::RebaseModeSelect
            | InputMode::RebaseSelect
            | InputMode::SquashSelect
            | InputMode::CompareSelect => None,
        }
    }
}

/// Actions that LogView can request from App
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogAction {
    /// No action needed
    None,
    /// Open diff view for the given change ID
    OpenDiff(String),
    /// Execute revset filter
    ExecuteRevset(String),
    /// Clear revset filter (reset to default)
    ClearRevset,
    /// Start describe input mode (App should fetch full description and call set_describe_input)
    StartDescribe(String),
    /// Update change description
    Describe { change_id: String, message: String },
    /// Open external editor for describe (jj describe --edit)
    DescribeExternal(String),
    /// Edit a specific change (jj edit)
    Edit(String),
    /// Create a new empty change (jj new)
    NewChange,
    /// Create a new change with selected revision as parent (jj new <revision>)
    NewChangeFrom {
        change_id: String,
        display_name: String,
    },
    /// User pressed C on @ - show info notification suggesting 'c'
    NewChangeFromCurrent,
    /// Squash source change into destination (jj squash --from --into)
    SquashInto { source: String, destination: String },
    /// Abandon a change (jj abandon)
    Abandon(String),
    /// Split a change (jj split, opens external editor)
    Split(String),
    /// Create a bookmark on a change
    CreateBookmark { change_id: String, name: String },
    /// Start bookmark deletion (opens selection dialog)
    StartBookmarkDelete,
    /// Rebase source change to destination with specified mode
    Rebase {
        source: String,
        destination: String,
        mode: RebaseMode,
    },
    /// Absorb working copy changes into ancestor commits
    Absorb,
    /// Open resolve list view for a change
    OpenResolveList {
        change_id: String,
        is_working_copy: bool,
    },
    /// Fetch from remote
    Fetch,
    /// Start push flow (opens dialog if bookmarks exist)
    StartPush,
    /// Start track flow (opens dialog if untracked remotes exist)
    StartTrack,
    /// Start bookmark jump flow (opens selection dialog)
    StartBookmarkJump,
    /// Compare two revisions (open diff --from --to)
    Compare { from: String, to: String },
    /// Entered compare mode (notification with from_id)
    StartCompare(String),
    /// Compare blocked: same revision selected
    CompareSameRevision,
    /// Open Bookmark View
    OpenBookmarkView,
    /// Move @ to next child (jj next --edit)
    NextChange,
    /// Move @ to previous parent (jj prev --edit)
    PrevChange,
    /// Toggle reversed display order
    ToggleReversed,
    /// Duplicate a change (jj duplicate)
    Duplicate(String),
}

/// Log View state
#[derive(Debug, Default)]
pub struct LogView {
    /// List of changes to display (includes graph-only lines)
    pub changes: Vec<Change>,
    /// Currently selected index in `changes`
    pub selected_index: usize,
    /// Scroll offset for display
    pub scroll_offset: usize,
    /// Current input mode
    pub input_mode: InputMode,
    /// Input buffer for revset/search/bookmark (NOT used for describe anymore)
    pub input_buffer: String,
    /// Revset input history
    pub revset_history: Vec<String>,
    /// Current revset filter (None = default)
    pub current_revset: Option<String>,
    /// Last search query for n/N navigation
    pub(crate) last_search_query: Option<String>,
    /// Change ID being edited (for DescribeInput mode)
    pub editing_change_id: Option<String>,
    /// Indices of selectable changes (not graph-only)
    selectable_indices: Vec<usize>,
    /// Current position in selectable_indices
    selection_cursor: usize,
    /// Source change ID for rebase (set when entering RebaseSelect mode)
    pub(crate) rebase_source: Option<String>,
    /// Current rebase mode (set during RebaseModeSelect)
    pub(crate) rebase_mode: RebaseMode,
    /// Source change ID for squash (set when entering SquashSelect mode)
    pub(crate) squash_source: Option<String>,
    /// "From" change ID for compare (set when entering CompareSelect mode)
    pub(crate) compare_from: Option<String>,
    /// Whether to display log in reversed order (oldest first)
    pub(crate) reversed: bool,
}

pub mod empty_text {
    pub const TITLE: &str = "No changes found.";
    pub const HINT: &str = "Hint: Try '/' with revset all()";
}

impl LogView {
    /// Create a new LogView
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the changes to display
    ///
    /// Builds the selectable indices list (excluding graph-only lines)
    /// and resets selection to the first selectable change.
    pub fn set_changes(&mut self, changes: Vec<Change>) {
        // Build selectable indices (non graph-only lines)
        self.selectable_indices = changes
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.is_graph_only)
            .map(|(i, _)| i)
            .collect();

        self.changes = changes;
        self.selection_cursor = 0;
        self.selected_index = self.selectable_indices.first().copied().unwrap_or(0);
    }

    /// Get the currently selected change
    pub fn selected_change(&self) -> Option<&Change> {
        self.changes.get(self.selected_index)
    }

    /// Move selection up (skips graph-only lines)
    pub fn move_up(&mut self) {
        if self.selection_cursor > 0 {
            self.selection_cursor -= 1;
            self.selected_index = self.selectable_indices[self.selection_cursor];
        }
    }

    /// Move selection down (skips graph-only lines)
    pub fn move_down(&mut self) {
        if self.selection_cursor < self.selectable_indices.len().saturating_sub(1) {
            self.selection_cursor += 1;
            self.selected_index = self.selectable_indices[self.selection_cursor];
        }
    }

    /// Move to top (first selectable change)
    pub fn move_to_top(&mut self) {
        self.selection_cursor = 0;
        self.selected_index = self.selectable_indices.first().copied().unwrap_or(0);
    }

    /// Move to bottom (last selectable change)
    pub fn move_to_bottom(&mut self) {
        if let Some(&last) = self.selectable_indices.last() {
            self.selection_cursor = self.selectable_indices.len().saturating_sub(1);
            self.selected_index = last;
        }
    }

    /// Start text search input mode
    pub fn start_search_input(&mut self) {
        self.input_mode = InputMode::SearchInput;
        self.input_buffer.clear();
    }

    /// Start revset input mode
    pub fn start_revset_input(&mut self) {
        self.input_mode = InputMode::RevsetInput;
        self.input_buffer.clear();
    }

    /// Cancel input mode
    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
        self.editing_change_id = None;
    }

    /// Set describe input mode with the description text (single-line only)
    ///
    /// Called by App after verifying the description is single-line.
    /// Multi-line descriptions are blocked at the App layer (directed to Ctrl+E).
    pub fn set_describe_input(&mut self, change_id: String, description: String) {
        self.editing_change_id = Some(change_id);
        self.input_buffer = description;
        self.input_mode = InputMode::DescribeInput;
    }

    /// Start bookmark input mode for the selected change
    pub fn start_bookmark_input(&mut self) {
        // Clone change_id first to avoid borrow conflict
        let change_id = self.selected_change().map(|c| c.change_id.clone());

        if let Some(change_id) = change_id {
            self.editing_change_id = Some(change_id);
            self.input_buffer.clear();
            self.input_mode = InputMode::BookmarkInput;
        }
    }

    /// Start rebase mode selection (r/s/A/B single key)
    ///
    /// Saves the source change and enters RebaseModeSelect mode.
    /// Returns true if mode was entered, false if no change is selected.
    pub fn start_rebase_mode_select(&mut self) -> bool {
        let change_id = self.selected_change().map(|c| c.change_id.clone());

        if let Some(change_id) = change_id {
            self.rebase_source = Some(change_id);
            self.input_mode = InputMode::RebaseModeSelect;
            true
        } else {
            false
        }
    }

    /// Cancel rebase mode selection
    pub fn cancel_rebase_mode_select(&mut self) {
        self.rebase_source = None;
        self.rebase_mode = RebaseMode::default();
        self.input_mode = InputMode::Normal;
    }

    /// Start rebase destination selection mode
    ///
    /// Returns true if mode was entered, false if no change is selected.
    pub fn start_rebase_select(&mut self) -> bool {
        // Clone change_id first to avoid borrow conflict
        let change_id = self.selected_change().map(|c| c.change_id.clone());

        if let Some(change_id) = change_id {
            self.rebase_source = Some(change_id);
            self.input_mode = InputMode::RebaseSelect;
            true
        } else {
            false
        }
    }

    /// Cancel rebase selection mode
    pub fn cancel_rebase_select(&mut self) {
        self.rebase_source = None;
        self.rebase_mode = RebaseMode::default();
        self.input_mode = InputMode::Normal;
    }

    /// Start squash destination selection mode
    ///
    /// Returns true if mode was entered, false if no change is selected.
    pub fn start_squash_select(&mut self) -> bool {
        // Clone change_id first to avoid borrow conflict
        let change_id = self.selected_change().map(|c| c.change_id.clone());

        if let Some(change_id) = change_id {
            self.squash_source = Some(change_id);
            self.input_mode = InputMode::SquashSelect;
            true
        } else {
            false
        }
    }

    /// Cancel squash selection mode
    pub fn cancel_squash_select(&mut self) {
        self.squash_source = None;
        self.input_mode = InputMode::Normal;
    }

    /// Start compare revision selection mode
    ///
    /// The currently selected change becomes the "from" revision.
    /// The user then selects the "to" revision.
    /// Returns true if mode was entered, false if no change is selected.
    pub fn start_compare_select(&mut self) -> bool {
        let change_id = self.selected_change().map(|c| c.change_id.clone());

        if let Some(change_id) = change_id {
            self.compare_from = Some(change_id);
            self.input_mode = InputMode::CompareSelect;
            true
        } else {
            false
        }
    }

    /// Cancel compare selection mode
    pub fn cancel_compare_select(&mut self) {
        self.compare_from = None;
        self.input_mode = InputMode::Normal;
    }

    /// Select a change by its change_id (exact match)
    ///
    /// Returns true if the change was found and selected, false otherwise.
    /// The scroll_offset will be updated during next render via calculate_scroll_offset().
    pub fn select_change_by_id(&mut self, change_id: &str) -> bool {
        // Find the change in the selectable indices
        for (cursor, &idx) in self.selectable_indices.iter().enumerate() {
            if let Some(change) = self.changes.get(idx)
                && change.change_id == change_id
            {
                self.selection_cursor = cursor;
                self.selected_index = idx;
                return true;
            }
        }
        false
    }

    /// Get the current selection cursor position (index into selectable_indices)
    pub fn selected_selectable_index(&self) -> usize {
        self.selection_cursor
    }

    /// Select the working copy (@) change
    ///
    /// Searches for the change with `is_working_copy == true` and moves
    /// the cursor to it. Used after `jj next`/`jj prev` to follow @.
    /// Returns true if working copy was found and selected.
    pub fn select_working_copy(&mut self) -> bool {
        for (cursor, &idx) in self.selectable_indices.iter().enumerate() {
            if let Some(change) = self.changes.get(idx)
                && change.is_working_copy
            {
                self.selection_cursor = cursor;
                self.selected_index = idx;
                return true;
            }
        }
        false
    }

    /// Select a change by prefix match on change_id
    ///
    /// Used when the caller has a potentially shorter change_id (e.g., from
    /// `jj file annotate` which uses `shortest()` format instead of `short(8)`).
    pub fn select_change_by_prefix(&mut self, prefix: &str) -> bool {
        for (cursor, &idx) in self.selectable_indices.iter().enumerate() {
            if let Some(change) = self.changes.get(idx)
                && change.change_id.starts_with(prefix)
            {
                self.selection_cursor = cursor;
                self.selected_index = idx;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
