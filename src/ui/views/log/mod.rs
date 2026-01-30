//! Log View - displays jj log output
//!
//! The main view of Tij, showing the change history.

mod input;
mod render;

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
}

impl InputMode {
    pub fn input_bar_meta(self) -> Option<(&'static str, &'static str)> {
        match self {
            InputMode::SearchInput => Some(("Search: ", " / Search ")),
            InputMode::RevsetInput => Some(("Revset: ", " r Revset ")),
            InputMode::Normal => None,
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
    /// Input buffer for revset
    pub input_buffer: String,
    /// Revset input history
    pub revset_history: Vec<String>,
    /// Current revset filter (None = default)
    pub current_revset: Option<String>,
    /// Last search query for n/N navigation
    pub(crate) last_search_query: Option<String>,
    /// Indices of selectable changes (not graph-only)
    selectable_indices: Vec<usize>,
    /// Current position in selectable_indices
    selection_cursor: usize,
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

    /// Check if the given index is selectable (not graph-only)
    pub fn is_selectable(&self, index: usize) -> bool {
        self.changes
            .get(index)
            .map(|c| !c.is_graph_only)
            .unwrap_or(false)
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

    /// Get the number of selectable changes
    pub fn selectable_count(&self) -> usize {
        self.selectable_indices.len()
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
    }
}

#[cfg(test)]
mod tests;
