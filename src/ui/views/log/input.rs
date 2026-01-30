//! Input handling and search for LogView

use crossterm::event::{KeyCode, KeyEvent};

use crate::keys;
use crate::model::Change;

use super::{InputMode, LogAction, LogView};

impl LogView {
    // ─────────────────────────────────────────────────────────────────────────
    // Input handling
    // ─────────────────────────────────────────────────────────────────────────

    /// Handle key event and return action
    pub fn handle_key(&mut self, key: KeyEvent) -> LogAction {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::SearchInput => self.handle_search_input_key(key),
            InputMode::RevsetInput => self.handle_revset_input_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            k if k == keys::MOVE_DOWN || k == KeyCode::Down => {
                self.move_down();
                LogAction::None
            }
            k if k == keys::MOVE_UP || k == KeyCode::Up => {
                self.move_up();
                LogAction::None
            }
            k if k == keys::GO_TOP => {
                self.move_to_top();
                LogAction::None
            }
            k if k == keys::GO_BOTTOM => {
                self.move_to_bottom();
                LogAction::None
            }
            k if k == keys::SEARCH_INPUT => {
                self.start_search_input();
                LogAction::None
            }
            k if k == keys::REVSET_INPUT => {
                self.start_revset_input();
                LogAction::None
            }
            k if k == keys::SEARCH_NEXT => {
                self.search_next();
                LogAction::None
            }
            k if k == keys::SEARCH_PREV => {
                self.search_prev();
                LogAction::None
            }
            k if k == keys::OPEN_DIFF => {
                if let Some(change) = self.selected_change() {
                    LogAction::OpenDiff(change.change_id.clone())
                } else {
                    LogAction::None
                }
            }
            _ => LogAction::None,
        }
    }

    fn handle_search_input_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            k if k == keys::ESC => {
                self.cancel_input();
                LogAction::None
            }
            KeyCode::Enter => {
                let query = self.input_buffer.clone();
                if query.is_empty() {
                    // Clear search query
                    self.last_search_query = None;
                } else {
                    self.last_search_query = Some(query);
                    // Jump to first match from beginning
                    self.search_first();
                }
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
                LogAction::None
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                LogAction::None
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                LogAction::None
            }
            _ => LogAction::None,
        }
    }

    fn handle_revset_input_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            k if k == keys::ESC => {
                self.cancel_input();
                LogAction::None
            }
            KeyCode::Enter => {
                let revset = self.input_buffer.clone();
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
                if revset.is_empty() {
                    // Clear revset (reset to default)
                    LogAction::ClearRevset
                } else {
                    self.revset_history.push(revset.clone());
                    LogAction::ExecuteRevset(revset)
                }
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                LogAction::None
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                LogAction::None
            }
            _ => LogAction::None,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Search
    // ─────────────────────────────────────────────────────────────────────────

    /// Check if a change matches the search query
    pub(crate) fn change_matches(&self, change: &Change, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        change.change_id.to_lowercase().contains(&query_lower)
            || change.description.to_lowercase().contains(&query_lower)
            || change.author.to_lowercase().contains(&query_lower)
            || change
                .bookmarks
                .iter()
                .any(|b| b.to_lowercase().contains(&query_lower))
    }

    /// Search for first match from beginning (used when search is confirmed)
    pub fn search_first(&mut self) -> bool {
        let Some(ref query) = self.last_search_query else {
            return false;
        };
        if self.changes.is_empty() {
            return false;
        }

        let query = query.clone();

        // Search from beginning
        for i in 0..self.changes.len() {
            if self.change_matches(&self.changes[i], &query) {
                self.selected_index = i;
                return true;
            }
        }

        false
    }

    /// Search for next match (n key)
    pub fn search_next(&mut self) -> bool {
        let Some(ref query) = self.last_search_query else {
            return false;
        };
        if self.changes.is_empty() {
            return false;
        }

        let query = query.clone();
        let start = self.selected_index + 1;

        // Search from current position to end
        for i in start..self.changes.len() {
            if self.change_matches(&self.changes[i], &query) {
                self.selected_index = i;
                return true;
            }
        }

        // Wrap around: search from beginning to current position
        for i in 0..self.selected_index {
            if self.change_matches(&self.changes[i], &query) {
                self.selected_index = i;
                return true;
            }
        }

        false
    }

    /// Search for previous match (N key)
    pub fn search_prev(&mut self) -> bool {
        let Some(ref query) = self.last_search_query else {
            return false;
        };
        if self.changes.is_empty() {
            return false;
        }

        let query = query.clone();

        // Search from current position to beginning
        for i in (0..self.selected_index).rev() {
            if self.change_matches(&self.changes[i], &query) {
                self.selected_index = i;
                return true;
            }
        }

        // Wrap around: search from end to current position
        for i in (self.selected_index + 1..self.changes.len()).rev() {
            if self.change_matches(&self.changes[i], &query) {
                self.selected_index = i;
                return true;
            }
        }

        false
    }
}
