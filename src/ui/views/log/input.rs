//! Input handling and search for LogView

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::keys;
use crate::model::Change;

use super::{InputMode, LogAction, LogView, RebaseMode, RebaseSource};

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
            InputMode::InterdiffSelect => self.handle_interdiff_select_key(key),
            InputMode::ParallelizeSelect => self.handle_parallelize_select_key(key),
            InputMode::RebaseRevsetInput => self.handle_rebase_revset_input_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> LogAction {
        // Ctrl+E: external editor describe (must be checked before 'e' match)
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('e') | KeyCode::Char('E'))
        {
            return if let Some(change) = self.selected_change() {
                LogAction::DescribeExternal(change.commit_id.to_string())
            } else {
                LogAction::None
            };
        }

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
                    LogAction::StartDescribe(change.commit_id.to_string())
                } else {
                    LogAction::None
                }
            }
            k if k == keys::EDIT => {
                if let Some(change) = self.selected_change() {
                    LogAction::Edit(change.commit_id.to_string())
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
                        let display_name = change
                            .bookmarks
                            .first()
                            .cloned()
                            .unwrap_or_else(|| change.change_id.short().to_string());
                        LogAction::NewChangeFrom {
                            revision: change.commit_id.to_string(),
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
                    LogAction::Abandon(change.commit_id.to_string())
                } else {
                    LogAction::None
                }
            }
            k if k == keys::SPLIT => {
                if let Some(change) = self.selected_change() {
                    LogAction::Split(change.commit_id.to_string())
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
                        revision: change.commit_id.to_string(),
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
                    // Use change_id for working copy (commit_id changes on auto-snapshot)
                    let revision = if change.is_working_copy {
                        change.change_id.to_string()
                    } else {
                        change.commit_id.to_string()
                    };
                    LogAction::OpenDiff(revision)
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
                    let from_id = self.compare_from.as_ref().unwrap().0.to_string();
                    LogAction::StartCompare(from_id)
                } else {
                    LogAction::None
                }
            }
            k if k == keys::INTERDIFF => {
                if self.start_interdiff_select() {
                    let from_id = self.interdiff_from.as_ref().unwrap().0.to_string();
                    LogAction::StartInterdiff(from_id)
                } else {
                    LogAction::None
                }
            }
            k if k == keys::BOOKMARK_VIEW => LogAction::OpenBookmarkView,
            k if k == keys::TAG_VIEW => LogAction::OpenTagView,
            k if k == keys::COMMAND_HISTORY => LogAction::OpenCommandHistory,
            k if k == keys::NEXT_CHANGE => LogAction::NextChange,
            k if k == keys::PREV_CHANGE => LogAction::PrevChange,
            k if k == keys::LOG_REVERSE => LogAction::ToggleReversed,
            k if k == keys::DUPLICATE => {
                if let Some(change) = self.selected_change() {
                    LogAction::Duplicate(change.commit_id.to_string())
                } else {
                    LogAction::None
                }
            }
            k if k == keys::DIFFEDIT => {
                if let Some(change) = self.selected_change() {
                    LogAction::DiffEdit(change.commit_id.to_string())
                } else {
                    LogAction::None
                }
            }
            k if k == keys::EVOLOG => {
                if let Some(change) = self.selected_change() {
                    LogAction::OpenEvolog(change.commit_id.to_string())
                } else {
                    LogAction::None
                }
            }
            k if k == keys::REVERT => {
                if let Some(change) = self.selected_change() {
                    if change.is_empty {
                        // Empty commit has nothing to revert
                        LogAction::None
                    } else {
                        LogAction::Revert(change.commit_id.to_string())
                    }
                } else {
                    LogAction::None
                }
            }
            k if k == keys::SIMPLIFY_PARENTS => {
                if let Some(change) = self.selected_change() {
                    LogAction::SimplifyParents(change.commit_id.to_string())
                } else {
                    LogAction::None
                }
            }
            k if k == keys::FIX => {
                if let Some(change) = self.selected_change() {
                    LogAction::Fix {
                        revision: change.commit_id.to_string(),
                        change_id: change.change_id.to_string(),
                    }
                } else {
                    LogAction::None
                }
            }
            k if k == keys::PARALLELIZE => {
                if self.start_parallelize_select() {
                    let from_id = self.parallelize_from.as_ref().unwrap().0.clone();
                    LogAction::StartParallelize(from_id)
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
            if let Some(revision) = view.editing_revision.take() {
                if message.trim().is_empty() {
                    LogAction::None // Empty = cancel
                } else {
                    LogAction::Describe { revision, message }
                }
            } else {
                LogAction::None
            }
        })
    }

    fn handle_bookmark_input_key(&mut self, key: KeyEvent) -> LogAction {
        self.handle_text_input(key, |view, name| {
            if let Some(revision) = view.editing_revision.take() {
                if name.is_empty() {
                    // Empty name = cancel
                    LogAction::None
                } else {
                    LogAction::CreateBookmark { revision, name }
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
            KeyCode::Char('b') => {
                self.rebase_mode = RebaseMode::Branch;
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
            // Enter revset input (only for Revision/Source/Branch modes)
            KeyCode::Char(':') => {
                if matches!(
                    self.rebase_mode,
                    RebaseMode::Revision | RebaseMode::Source | RebaseMode::Branch
                ) {
                    self.input_buffer.clear();
                    self.input_mode = InputMode::RebaseRevsetInput;
                }
                LogAction::None
            }
            // Toggle --skip-emptied
            KeyCode::Char('S') => {
                self.skip_emptied = !self.skip_emptied;
                LogAction::None
            }
            // Toggle --simplify-parents
            KeyCode::Char('P') => {
                self.simplify_parents = !self.simplify_parents;
                LogAction::None
            }
            // Confirm rebase
            KeyCode::Enter => {
                if let (Some(rebase_src), Some(dest_change)) =
                    (self.rebase_source.clone(), self.selected_change())
                {
                    let destination = dest_change.commit_id.to_string();

                    // Extract source string and use_revset flag from RebaseSource
                    let (source, use_revset) = match &rebase_src {
                        RebaseSource::Selected {
                            commit_id,
                            change_id: _,
                        } => {
                            // Prevent rebasing to self (compare by commit_id for divergent support)
                            if *commit_id == dest_change.commit_id {
                                return LogAction::None;
                            }
                            (commit_id.to_string(), false)
                        }
                        RebaseSource::Revset(revset) => (revset.clone(), true),
                    };

                    let mode = self.rebase_mode;
                    let skip_emptied = self.skip_emptied;
                    let simplify_parents = self.simplify_parents;
                    self.rebase_source = None;
                    self.rebase_mode = RebaseMode::default();
                    self.skip_emptied = false;
                    self.simplify_parents = false;
                    self.input_mode = InputMode::Normal;
                    LogAction::Rebase {
                        source,
                        destination,
                        mode,
                        skip_emptied,
                        use_revset,
                        simplify_parents,
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
                if let (Some(source_pair), Some(dest_change)) =
                    (self.squash_source.take(), self.selected_change())
                {
                    let destination = dest_change.commit_id.to_string();

                    // Prevent squashing into self (compare by commit_id for divergent support)
                    if source_pair.1 == destination {
                        // Restore squash_source and stay in mode
                        self.squash_source = Some(source_pair);
                        return LogAction::None;
                    }

                    self.input_mode = InputMode::Normal;
                    LogAction::SquashInto {
                        source: source_pair.1,
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
                if let (Some(from_pair), Some(to_change)) =
                    (self.compare_from.take(), self.selected_change())
                {
                    let to = to_change.commit_id.to_string();

                    // Prevent comparing a revision with itself (compare by commit_id for divergent support)
                    if from_pair.1 == to {
                        // Restore compare_from and stay in mode
                        self.compare_from = Some(from_pair);
                        return LogAction::CompareSameRevision;
                    }
                    self.input_mode = InputMode::Normal;
                    LogAction::Compare {
                        from: from_pair.1,
                        to,
                    }
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

    /// Handle key events in interdiff revision selection mode
    ///
    /// In this mode, j/k navigates to select the "to" revision, Enter confirms,
    /// and Esc cancels. Selecting the same revision as "from" is guarded.
    fn handle_interdiff_select_key(&mut self, key: KeyEvent) -> LogAction {
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
            // Confirm interdiff
            KeyCode::Enter => {
                if let (Some(from_pair), Some(to_change)) =
                    (self.interdiff_from.take(), self.selected_change())
                {
                    let to = to_change.commit_id.to_string();

                    // Prevent interdiff of a revision with itself
                    if from_pair.1 == to {
                        self.interdiff_from = Some(from_pair);
                        return LogAction::InterdiffSameRevision;
                    }
                    self.input_mode = InputMode::Normal;
                    LogAction::Interdiff {
                        from: from_pair.1,
                        to,
                    }
                } else {
                    LogAction::None
                }
            }
            // Cancel
            k if k == keys::ESC => {
                self.cancel_interdiff_select();
                LogAction::None
            }
            // Ignore other keys in interdiff select mode
            _ => LogAction::None,
        }
    }

    /// Handle key events in parallelize selection mode
    ///
    /// In this mode, j/k navigates to select the end of the range, Enter confirms,
    /// and Esc cancels. Selecting the same revision as "from" is guarded.
    fn handle_parallelize_select_key(&mut self, key: KeyEvent) -> LogAction {
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
            // Confirm parallelize
            KeyCode::Enter => {
                if let (Some(from_pair), Some(to_change)) =
                    (self.parallelize_from.take(), self.selected_change())
                {
                    let to = to_change.commit_id.to_string();

                    // Prevent parallelizing a single revision (compare by commit_id for divergent support)
                    if from_pair.1 == to {
                        // Restore parallelize_from and stay in mode
                        self.parallelize_from = Some(from_pair);
                        return LogAction::ParallelizeSameRevision;
                    }
                    self.input_mode = InputMode::Normal;
                    LogAction::Parallelize {
                        from: from_pair.1,
                        to,
                    }
                } else {
                    LogAction::None
                }
            }
            // Cancel
            k if k == keys::ESC => {
                self.cancel_parallelize_select();
                LogAction::None
            }
            // Ignore other keys in parallelize select mode
            _ => LogAction::None,
        }
    }

    /// Handle key events in rebase revset text input mode
    ///
    /// Esc cancels and clears revset mode entirely.
    /// Enter with text sets the revset as source.
    /// Enter with empty input restores the original single change_id source.
    fn handle_rebase_revset_input_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            k if k == keys::ESC => {
                // Esc: discard input + restore single change source
                self.input_buffer.clear();
                if let Some(change) = self.selected_change() {
                    self.rebase_source = Some(RebaseSource::Selected {
                        change_id: change.change_id.to_string(),
                        commit_id: change.commit_id.to_string(),
                    });
                }
                self.input_mode = InputMode::RebaseSelect;
                LogAction::None
            }
            k if k == keys::SUBMIT => {
                let revset = std::mem::take(&mut self.input_buffer);
                if revset.is_empty() {
                    // Empty Enter: restore single change source
                    if let Some(change) = self.selected_change() {
                        self.rebase_source = Some(RebaseSource::Selected {
                            change_id: change.change_id.to_string(),
                            commit_id: change.commit_id.to_string(),
                        });
                    }
                } else {
                    self.rebase_source = Some(RebaseSource::Revset(revset));
                }
                self.input_mode = InputMode::RebaseSelect;
                LogAction::None
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
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
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
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
            // Also update selection_cursor so card mode stays in sync
            if let Some(cursor) = self.selectable_indices.iter().position(|&i| i == index) {
                self.selection_cursor = cursor;
            }
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
