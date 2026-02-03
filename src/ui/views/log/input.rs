//! Input handling and search for LogView

use crossterm::event::{KeyCode, KeyEvent};

use crate::keys;
use crate::model::Change;

use super::{InputMode, LogAction, LogView};

/// Search direction/mode
#[derive(Clone, Copy)]
enum SearchKind {
    First,
    Next,
    Prev,
}

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
            InputMode::DescribeInput => self.handle_describe_input_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            k if keys::is_move_down(k) => {
                self.move_down();
                LogAction::None
            }
            k if keys::is_move_up(k) => {
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
            k if k == keys::DESCRIBE => {
                self.start_describe_input();
                LogAction::None
            }
            k if k == keys::EDIT => {
                if let Some(change) = self.selected_change() {
                    LogAction::Edit(change.change_id.clone())
                } else {
                    LogAction::None
                }
            }
            k if k == keys::NEW_CHANGE => LogAction::NewChange,
            k if k == keys::SQUASH => {
                if let Some(change) = self.selected_change() {
                    // Let state.rs handle validation and show appropriate notification
                    LogAction::Squash(change.change_id.clone())
                } else {
                    LogAction::None
                }
            }
            k if k == keys::ABANDON => {
                if let Some(change) = self.selected_change() {
                    // Let state.rs handle validation and show appropriate notification
                    LogAction::Abandon(change.change_id.clone())
                } else {
                    LogAction::None
                }
            }
            k if k == keys::SPLIT => {
                if let Some(change) = self.selected_change() {
                    // Let state.rs handle the interactive split
                    LogAction::Split(change.change_id.clone())
                } else {
                    LogAction::None
                }
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
        self.handle_text_input(key, |view, query| {
            if query.is_empty() {
                // Clear search query
                view.last_search_query = None;
            } else {
                view.last_search_query = Some(query);
                // Jump to first match from beginning
                view.search_first();
            }
            LogAction::None
        })
    }

    fn handle_revset_input_key(&mut self, key: KeyEvent) -> LogAction {
        self.handle_text_input(key, |view, revset| {
            if revset.is_empty() {
                // Clear revset (reset to default)
                LogAction::ClearRevset
            } else {
                view.revset_history.push(revset.clone());
                LogAction::ExecuteRevset(revset)
            }
        })
    }

    fn handle_describe_input_key(&mut self, key: KeyEvent) -> LogAction {
        self.handle_text_input(key, |view, message| {
            if let Some(change_id) = view.editing_change_id.take() {
                if message.is_empty() {
                    // Empty message = cancel
                    LogAction::None
                } else {
                    LogAction::Describe { change_id, message }
                }
            } else {
                LogAction::None
            }
        })
    }

    fn handle_text_input<F>(&mut self, key: KeyEvent, on_submit: F) -> LogAction
    where
        F: FnOnce(&mut Self, String) -> LogAction,
    {
        match key.code {
            k if k == keys::ESC => {
                self.cancel_input();
                LogAction::None
            }
            k if k == keys::SUBMIT => {
                let input = std::mem::take(&mut self.input_buffer);
                self.input_mode = InputMode::Normal;
                on_submit(self, input)
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
    pub(crate) fn change_matches(&self, change: &Change, query_lower: &str) -> bool {
        change.change_id.to_lowercase().contains(query_lower)
            || change.description.to_lowercase().contains(query_lower)
            || change.author.to_lowercase().contains(query_lower)
            || change
                .bookmarks
                .iter()
                .any(|b| b.to_lowercase().contains(query_lower))
    }

    /// Search for first match from beginning (used when search is confirmed)
    pub fn search_first(&mut self) -> bool {
        self.search(SearchKind::First)
    }

    /// Search for next match (n key)
    pub fn search_next(&mut self) -> bool {
        self.search(SearchKind::Next)
    }

    /// Search for previous match (N key)
    pub fn search_prev(&mut self) -> bool {
        self.search(SearchKind::Prev)
    }

    fn search(&mut self, kind: SearchKind) -> bool {
        let Some(ref query) = self.last_search_query else {
            return false;
        };
        if self.changes.is_empty() {
            return false;
        }

        let query_lower = query.to_lowercase();

        let found = match kind {
            SearchKind::First => self.find_match_in(0..self.changes.len(), &query_lower),
            SearchKind::Next => {
                let start = self.selected_index + 1;
                let forward = start..self.changes.len();
                let wrap = 0..self.selected_index;
                self.find_match_in(forward, &query_lower)
                    .or_else(|| self.find_match_in(wrap, &query_lower))
            }
            SearchKind::Prev => {
                let backward = (0..self.selected_index).rev();
                let wrap = (self.selected_index + 1..self.changes.len()).rev();
                self.find_match_in(backward, &query_lower)
                    .or_else(|| self.find_match_in(wrap, &query_lower))
            }
        };

        if let Some(index) = found {
            self.selected_index = index;
            return true;
        }

        false
    }

    fn find_match_in<I>(&self, indices: I, query_lower: &str) -> Option<usize>
    where
        I: IntoIterator<Item = usize>,
    {
        indices
            .into_iter()
            .find(|&i| self.change_matches(&self.changes[i], query_lower))
    }
}
