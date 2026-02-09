//! Input handling and search for LogView

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::keys;
use crate::model::Change;

use super::{InputMode, LogAction, LogView, RebaseMode};

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
            InputMode::BookmarkInput => self.handle_bookmark_input_key(key),
            InputMode::RebaseModeSelect => self.handle_rebase_mode_select_key(key),
            InputMode::RebaseSelect => self.handle_rebase_select_key(key),
            InputMode::SquashSelect => self.handle_squash_select_key(key),
            InputMode::CompareSelect => self.handle_compare_select_key(key),
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
                if let Some(change) = self.selected_change() {
                    LogAction::StartDescribe(change.change_id.clone())
                } else {
                    LogAction::None
                }
            }
            k if k == keys::EDIT => {
                if let Some(change) = self.selected_change() {
                    LogAction::Edit(change.change_id.clone())
                } else {
                    LogAction::None
                }
            }
            k if k == keys::NEW_CHANGE => LogAction::NewChange,
            k if k == keys::NEW_FROM => {
                if let Some(change) = self.selected_change() {
                    if change.is_working_copy {
                        // @ で C を押した場合は c の使用を案内
                        LogAction::NewChangeFromCurrent
                    } else {
                        // 表示名: 先頭 bookmark があれば優先、なければ short_id
                        let display_name = change.bookmarks.first().cloned().unwrap_or_else(|| {
                            change.change_id[..8.min(change.change_id.len())].to_string()
                        });
                        LogAction::NewChangeFrom {
                            change_id: change.change_id.clone(),
                            display_name,
                        }
                    }
                } else {
                    LogAction::None
                }
            }
            k if k == keys::SQUASH => {
                // Enter SquashSelect mode (validation happens in App layer)
                self.start_squash_select();
                LogAction::None
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
            k if k == keys::BOOKMARK => {
                self.start_bookmark_input();
                LogAction::None
            }
            k if k == keys::BOOKMARK_DELETE => {
                // Let state.rs handle the dialog
                LogAction::StartBookmarkDelete
            }
            k if k == keys::REBASE => {
                self.start_rebase_mode_select();
                LogAction::None
            }
            k if k == keys::ABSORB => LogAction::Absorb,
            k if k == keys::RESOLVE_LIST => {
                if let Some(change) = self.selected_change() {
                    LogAction::OpenResolveList {
                        change_id: change.change_id.clone(),
                        is_working_copy: change.is_working_copy,
                    }
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
            k if k == keys::FETCH => LogAction::Fetch,
            k if k == keys::PUSH => LogAction::StartPush,
            k if k == keys::TRACK => LogAction::StartTrack,
            k if k == keys::BOOKMARK_JUMP => LogAction::StartBookmarkJump,
            k if k == keys::COMPARE => {
                if self.start_compare_select() {
                    let from_id = self.compare_from.as_ref().unwrap().clone();
                    LogAction::StartCompare(from_id)
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
        // Ctrl+S to save
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
        {
            if let (Some(change_id), Some(textarea)) =
                (self.editing_change_id.take(), self.textarea.take())
            {
                let message = textarea.lines().join("\n");
                self.input_mode = InputMode::Normal;

                if message.trim().is_empty() {
                    return LogAction::None; // Empty = cancel
                }
                return LogAction::Describe { change_id, message };
            }
            return LogAction::None;
        }

        // Esc to cancel
        if key.code == KeyCode::Esc {
            self.cancel_describe_input();
            return LogAction::None;
        }

        // All other keys delegate to textarea (Enter = newline, cursor movement, etc.)
        if let Some(ref mut textarea) = self.textarea {
            textarea.input(key);
        }
        LogAction::None
    }

    fn handle_bookmark_input_key(&mut self, key: KeyEvent) -> LogAction {
        self.handle_text_input(key, |view, name| {
            if let Some(change_id) = view.editing_change_id.take() {
                if name.is_empty() {
                    // Empty name = cancel
                    LogAction::None
                } else {
                    LogAction::CreateBookmark { change_id, name }
                }
            } else {
                LogAction::None
            }
        })
    }

    /// Handle key events in rebase mode selection (r/s/A/B)
    ///
    /// Single key press selects the rebase mode, then transitions to RebaseSelect.
    fn handle_rebase_mode_select_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            KeyCode::Char('r') => {
                self.rebase_mode = RebaseMode::Revision;
                self.input_mode = InputMode::RebaseSelect;
                LogAction::None
            }
            KeyCode::Char('s') => {
                self.rebase_mode = RebaseMode::Source;
                self.input_mode = InputMode::RebaseSelect;
                LogAction::None
            }
            KeyCode::Char('A') => {
                self.rebase_mode = RebaseMode::InsertAfter;
                self.input_mode = InputMode::RebaseSelect;
                LogAction::None
            }
            KeyCode::Char('B') => {
                self.rebase_mode = RebaseMode::InsertBefore;
                self.input_mode = InputMode::RebaseSelect;
                LogAction::None
            }
            k if k == keys::ESC => {
                self.cancel_rebase_mode_select();
                LogAction::None
            }
            // Ignore other keys in mode selection
            _ => LogAction::None,
        }
    }

    /// Handle key events in rebase destination selection mode
    ///
    /// In this mode, j/k navigates to select a destination, Enter confirms,
    /// and Esc cancels. Other keys are ignored to prevent accidental actions.
    fn handle_rebase_select_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            // Navigation
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
            // Confirm rebase
            KeyCode::Enter => {
                if let (Some(source), Some(dest_change)) =
                    (self.rebase_source.clone(), self.selected_change())
                {
                    let destination = dest_change.change_id.clone();

                    // Prevent rebasing to self (all modes)
                    if source == destination {
                        return LogAction::None;
                    }

                    let mode = self.rebase_mode;
                    self.rebase_source = None;
                    self.rebase_mode = RebaseMode::default();
                    self.input_mode = InputMode::Normal;
                    LogAction::Rebase {
                        source,
                        destination,
                        mode,
                    }
                } else {
                    LogAction::None
                }
            }
            // Cancel
            k if k == keys::ESC => {
                self.cancel_rebase_select();
                LogAction::None
            }
            // Ignore other keys in rebase select mode
            _ => LogAction::None,
        }
    }

    /// Handle key events in squash destination selection mode
    ///
    /// In this mode, j/k navigates to select a destination, Enter confirms,
    /// and Esc cancels. Other keys are ignored to prevent accidental actions.
    fn handle_squash_select_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            // Navigation
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
            // Confirm squash
            KeyCode::Enter => {
                if let (Some(source), Some(dest_change)) =
                    (self.squash_source.take(), self.selected_change())
                {
                    let destination = dest_change.change_id.clone();

                    // Prevent squashing into self
                    if source == destination {
                        // Restore squash_source and stay in mode
                        self.squash_source = Some(source);
                        return LogAction::None;
                    }

                    self.input_mode = InputMode::Normal;
                    LogAction::SquashInto {
                        source,
                        destination,
                    }
                } else {
                    LogAction::None
                }
            }
            // Cancel
            k if k == keys::ESC => {
                self.cancel_squash_select();
                LogAction::None
            }
            // Ignore other keys in squash select mode
            _ => LogAction::None,
        }
    }

    /// Handle key events in compare revision selection mode
    ///
    /// In this mode, j/k navigates to select the "to" revision, Enter confirms,
    /// and Esc cancels. Selecting the same revision as "from" is guarded.
    fn handle_compare_select_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            // Navigation
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
            // Confirm compare
            KeyCode::Enter => {
                if let (Some(from), Some(to_change)) =
                    (self.compare_from.take(), self.selected_change())
                {
                    let to = to_change.change_id.clone();

                    // Prevent comparing a revision with itself
                    if from == to {
                        // Restore compare_from and stay in mode
                        self.compare_from = Some(from);
                        return LogAction::CompareSameRevision;
                    }

                    self.input_mode = InputMode::Normal;
                    LogAction::Compare { from, to }
                } else {
                    LogAction::None
                }
            }
            // Cancel
            k if k == keys::ESC => {
                self.cancel_compare_select();
                LogAction::None
            }
            // Ignore other keys in compare select mode
            _ => LogAction::None,
        }
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
