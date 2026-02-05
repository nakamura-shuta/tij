//! Log View - displays jj log output
//!
//! The main view of Tij, showing the change history.

mod input;
mod render;

use tui_textarea::TextArea;

use crate::model::Change;

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
    /// Rebase destination selection mode
    RebaseSelect,
}

impl InputMode {
    pub fn input_bar_meta(self) -> Option<(&'static str, &'static str)> {
        match self {
            InputMode::SearchInput => Some(("Search: ", " / Search ")),
            InputMode::RevsetInput => Some(("Revset: ", " r Revset ")),
            InputMode::BookmarkInput => Some(("Bookmark: ", " b Bookmark ")),
            // DescribeInput uses TextArea, not input bar
            InputMode::DescribeInput | InputMode::Normal | InputMode::RebaseSelect => None,
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
    /// Edit a specific change (jj edit)
    Edit(String),
    /// Create a new empty change (jj new)
    NewChange,
    /// Squash a change into its parent (jj squash -r)
    Squash(String),
    /// Abandon a change (jj abandon)
    Abandon(String),
    /// Split a change (jj split, opens external editor)
    Split(String),
    /// Create a bookmark on a change
    CreateBookmark { change_id: String, name: String },
    /// Start bookmark deletion (opens selection dialog)
    StartBookmarkDelete,
    /// Rebase source change to destination
    Rebase { source: String, destination: String },
    /// Absorb working copy changes into ancestor commits
    Absorb,
    /// Open resolve list view for a change
    OpenResolveList {
        change_id: String,
        is_working_copy: bool,
    },
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
    /// Text area for multi-line description input
    pub(crate) textarea: Option<TextArea<'static>>,
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
        self.textarea = None;
    }

    /// Set describe input mode with the full description text
    ///
    /// Called by App after fetching the full (multi-line) description.
    /// The description parameter should contain the complete description text.
    pub fn set_describe_input(&mut self, change_id: String, full_description: String) {
        self.editing_change_id = Some(change_id);

        // Initialize TextArea with full description
        let textarea = if full_description.is_empty() {
            TextArea::default()
        } else {
            let lines: Vec<String> = full_description.lines().map(|s| s.to_string()).collect();
            TextArea::new(lines)
        };
        self.textarea = Some(textarea);
        self.input_mode = InputMode::DescribeInput;
    }

    /// Start describe input mode for the selected change (legacy method for tests)
    ///
    /// Note: This uses the first-line description from Change.description.
    /// For full multi-line support, use set_describe_input() instead.
    #[cfg(test)]
    pub fn start_describe_input(&mut self) {
        // Clone values first to avoid borrow conflict
        let change_data = self
            .selected_change()
            .map(|c| (c.change_id.clone(), c.description.clone()));

        if let Some((change_id, description)) = change_data {
            self.set_describe_input(change_id, description);
        }
    }

    /// Cancel describe input mode
    pub fn cancel_describe_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.textarea = None;
        self.editing_change_id = None;
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
        self.input_mode = InputMode::Normal;
    }
}

#[cfg(test)]
mod tests;
