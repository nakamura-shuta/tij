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
    /// List of changes to display
    pub changes: Vec<Change>,
    /// Currently selected index
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
}

impl LogView {
    /// Create a new LogView
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the changes to display
    pub fn set_changes(&mut self, changes: Vec<Change>) {
        self.changes = changes;
        // Reset selection if out of bounds
        if self.selected_index >= self.changes.len() && !self.changes.is_empty() {
            self.selected_index = self.changes.len() - 1;
        }
        if self.changes.is_empty() {
            self.selected_index = 0;
        }
    }

    /// Get the currently selected change
    pub fn selected_change(&self) -> Option<&Change> {
        self.changes.get(self.selected_index)
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if !self.changes.is_empty() && self.selected_index < self.changes.len() - 1 {
            self.selected_index += 1;
        }
    }

    /// Move to top
    pub fn move_to_top(&mut self) {
        self.selected_index = 0;
    }

    /// Move to bottom
    pub fn move_to_bottom(&mut self) {
        if !self.changes.is_empty() {
            self.selected_index = self.changes.len() - 1;
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
    }
}

#[cfg(test)]
mod tests;
