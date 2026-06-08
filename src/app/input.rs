//! Input handling for the application

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::helpers::revision::short_id;

use super::state::{App, View};
use crate::keys;
use crate::ui::views::{
    BlameAction, BookmarkAction, CommandHistoryAction, DiffAction, EvologAction, InputMode,
    LogAction, OperationAction, RenameState, ResolveAction, StatusAction, StatusInputMode,
    TagAction,
};

impl App {
    /// Views where jj undo/redo is meaningful (mutating views). Read-only views
    /// (Diff/Blame/Evolog/CommandHistory/TraceDetail/Help) are excluded
    /// (Phase 48-D, G4).
    pub(crate) fn view_allows_undo(view: View) -> bool {
        matches!(
            view,
            View::Log
                | View::Status
                | View::Bookmark
                | View::Tag
                | View::Workspace
                | View::Operation
                | View::Resolve
        )
    }

    /// True when the current view is in an input/rename/select special mode where
    /// global single-key shortcuts (undo `u`, redo `^R`, refresh `^L`) must NOT
    /// fire — keys belong to the active input/select widget instead.
    pub(crate) fn in_special_input_mode(&self) -> bool {
        match self.current_view {
            View::Log => !matches!(self.log_view.input_mode, InputMode::Normal),
            View::Status => self.status_view.input_mode != StatusInputMode::Normal,
            View::Bookmark => self.bookmark_view.rename_state.is_some(),
            View::Help => self.help_search_input,
            _ => false,
        }
    }

    /// Handle key events
    pub fn on_key_event(&mut self, key: KeyEvent) {
        // Handle active dialog first (blocks other input)
        if let Some(ref mut dialog) = self.active_dialog {
            if let Some(result) = dialog.handle_key(key) {
                // handle_dialog_result() clears active_dialog internally.
                // Do NOT clear again here — dialog chains (e.g. remote select → push confirm)
                // set a new active_dialog inside handle_dialog_result().
                self.handle_dialog_result(result);
            }
            return;
        }

        // Command palette active: capture all keys (Phase 46-C). Like the dialog
        // block above, this intentionally traps every key (including Ctrl+C) while
        // open — close with Esc first. Kept consistent with dialog behavior.
        if self.palette_active {
            self.handle_palette_key(key);
            return;
        }

        // Clear error message and expired notification on any key press
        self.error_message = None;
        self.clear_expired_notification();

        // Handle Ctrl+C globally
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            self.quit();
            return;
        }

        // Handle Ctrl+R for redo (all mutating views, normal/non-special mode).
        // Excluded in read-only views and suppressed while any input/rename/select
        // mode is active so the key reaches the active widget (Phase 48-D).
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'))
            && Self::view_allows_undo(self.current_view)
            && !self.in_special_input_mode()
        {
            self.notification = None; // Clear any existing notification
            self.execute_redo();
            return;
        }

        // Handle Ctrl+L for refresh (all views, normal mode)
        if keys::is_refresh_key(&key) {
            // Skip if in input mode or special mode (like RebaseSelect)
            if !self.in_special_input_mode() {
                self.execute_refresh();
                return;
            }
        }

        // If in input mode or rebase select mode, delegate all keys to the view (skip global handling)
        if self.current_view == View::Log && !matches!(self.log_view.input_mode, InputMode::Normal)
        {
            let action = self.log_view.handle_key(key);
            self.handle_log_action(action);
            return;
        }

        // Handle Status View input mode
        if self.current_view == View::Status
            && self.status_view.input_mode != StatusInputMode::Normal
        {
            let action = self.status_view.handle_key(key);
            self.handle_status_action(action);
            return;
        }

        // Handle Help search input mode (skip global keys so Esc/q/Tab stay in search)
        if self.current_view == View::Help && self.help_search_input {
            self.handle_view_key(key);
            return;
        }

        // Bookmark rename input: delegate all keys to the view so global keys
        // (q/?/:/Tab) and palette don't misfire while typing a new name.
        if self.current_view == View::Bookmark && self.bookmark_view.rename_state.is_some() {
            self.handle_view_key(key);
            return;
        }

        // Command palette trigger (Phase 46-C): Log View Normal mode.
        if self.current_view == View::Log
            && matches!(self.log_view.input_mode, InputMode::Normal)
            && key.code == keys::COMMAND_PALETTE
            && key.modifiers.is_empty()
        {
            self.palette_active = true;
            self.palette_input.clear();
            self.palette_selected = 0;
            return;
        }

        if self.handle_global_key(key) {
            return;
        }

        self.handle_view_key(key);
    }

    /// Handle a key while the command palette is active (Phase 46-C).
    fn handle_palette_key(&mut self, key: KeyEvent) {
        use crate::ui::components::{PaletteDispatch, filter_commands, palette_commands};
        match key.code {
            KeyCode::Esc => self.close_palette(),
            KeyCode::Enter => {
                // Resolve current selection from the filtered list, then dispatch
                // (Phase 48-C): low-frequency commands by action-id, view-openers
                // by their (still-bound) single key.
                let filtered = filter_commands(palette_commands(), &self.palette_input);
                let chosen = filtered.get(self.palette_selected).map(|c| c.dispatch);
                self.close_palette();
                if let Some(dispatch) = chosen {
                    let action = match dispatch {
                        PaletteDispatch::Command(cmd) => self.log_view.command_action(cmd),
                        PaletteDispatch::Key(code) => {
                            self.log_view.handle_key(KeyEvent::from(code))
                        }
                    };
                    self.handle_log_action(action);
                }
            }
            // Movement: arrows + Ctrl+n/Ctrl+p (NOT j/k — those are text input).
            KeyCode::Down => self.palette_move(1),
            KeyCode::Up => self.palette_move(-1),
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.palette_move(1)
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.palette_move(-1)
            }
            KeyCode::Backspace => {
                self.palette_input.pop();
                self.palette_selected = 0;
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.palette_input.push(c);
                self.palette_selected = 0;
            }
            _ => {}
        }
    }

    /// Close the command palette and reset its state.
    fn close_palette(&mut self) {
        self.palette_active = false;
        self.palette_input.clear();
        self.palette_selected = 0;
    }

    /// Move palette selection within the current filtered list bounds (wraps).
    fn palette_move(&mut self, delta: isize) {
        let len = crate::ui::components::filter_commands(
            crate::ui::components::palette_commands(),
            &self.palette_input,
        )
        .len();
        if len == 0 {
            self.palette_selected = 0;
            return;
        }
        let next = (self.palette_selected as isize + delta).rem_euclid(len as isize);
        self.palette_selected = next as usize;
    }

    fn handle_global_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            keys::QUIT => {
                self.handle_quit();
                true
            }
            keys::ESC => {
                // Don't intercept Esc when bookmark rename is active
                if self.current_view == View::Bookmark && self.bookmark_view.rename_state.is_some()
                {
                    return false;
                }
                self.handle_back();
                true
            }
            keys::HELP => {
                self.go_to_view(View::Help);
                true
            }
            keys::TAB => {
                self.next_view();
                true
            }
            keys::STATUS_VIEW if self.current_view == View::Log => {
                self.go_to_view(View::Status);
                true
            }
            keys::UNDO if Self::view_allows_undo(self.current_view) => {
                self.notification = None; // Clear any existing notification
                self.execute_undo();
                true
            }
            keys::OPERATION_HISTORY if self.current_view == View::Log => {
                self.open_operation_history();
                true
            }
            _ => false,
        }
    }

    fn handle_quit(&mut self) {
        if self.current_view == View::Log {
            self.quit();
        } else {
            self.go_back();
        }
    }

    fn handle_back(&mut self) {
        if self.current_view != View::Log {
            self.go_back();
        }
    }

    fn handle_view_key(&mut self, key: KeyEvent) {
        match self.current_view {
            View::Log => {
                // Preview toggle (handled at App level since it's App state)
                if key.code == keys::PREVIEW
                    && matches!(self.log_view.input_mode, InputMode::Normal)
                {
                    self.preview_enabled = !self.preview_enabled;
                    if self.preview_enabled {
                        // Immediate fetch on toggle-ON (no 200ms wait)
                        self.update_preview_if_needed();
                        self.resolve_pending_preview();
                    } else {
                        // Clear pending fetch on disable (keep cache for toggle back ON)
                        self.preview_pending_id = None;
                    }
                    return;
                }

                let action = self.log_view.handle_key(key);
                self.handle_log_action(action);

                // Update preview after key processing (debounced)
                // Guard: only if still on Log view (Enter → Diff would have transitioned away)
                if self.preview_enabled && self.current_view == View::Log {
                    self.update_preview_if_needed();
                }
            }
            View::Diff => {
                if let Some(ref mut diff_view) = self.diff_view {
                    let visible_height = self.last_frame_height.get() as usize;
                    let action = diff_view.handle_key_with_height(key, visible_height);
                    self.handle_diff_action(action);
                }
            }
            View::Status => {
                let visible_height = self.last_frame_height.get() as usize;
                let action = self.status_view.handle_key_with_height(key, visible_height);
                self.handle_status_action(action);
            }
            View::Operation => {
                let action = self.operation_view.handle_key(key);
                self.handle_operation_action(action);
            }
            View::Blame => {
                if let Some(ref mut blame_view) = self.blame_view {
                    let action = blame_view.handle_key(key);
                    self.handle_blame_action(action);
                }
            }
            View::Bookmark => {
                let action = self.bookmark_view.handle_key(key);
                self.handle_bookmark_action(action);
            }
            View::Tag => {
                let action = self.tag_view.handle_key(key);
                self.handle_tag_action(action);
            }
            View::Workspace => {
                let action = self.workspace_view.handle_key(key);
                self.handle_workspace_action(action);
            }
            View::Resolve => {
                if let Some(ref mut resolve_view) = self.resolve_view {
                    let action = resolve_view.handle_key(key);
                    self.handle_resolve_action(action);
                }
            }
            View::Evolog => {
                if let Some(ref mut evolog_view) = self.evolog_view {
                    let action = evolog_view.handle_key(key);
                    self.handle_evolog_action(action);
                }
            }
            View::CommandHistory => {
                let total = self.command_history.len();
                let action = self.command_history_view.handle_key(key, total);
                self.handle_command_history_action(action);
            }
            View::TraceDetail => {
                if let Some(ref mut view) = self.trace_detail_view {
                    let action = view.handle_key(key);
                    self.handle_trace_detail_action(action);
                }
            }
            View::Help => {
                if self.help_search_input {
                    // Search input mode: capture text
                    match key.code {
                        KeyCode::Esc => {
                            self.help_search_input = false;
                            self.help_input_buffer.clear();
                        }
                        KeyCode::Enter => {
                            let query = std::mem::take(&mut self.help_input_buffer);
                            self.help_search_input = false;
                            if query.is_empty() {
                                self.help_search_query = None;
                            } else {
                                self.help_search_query = Some(query.clone());
                                // Jump to first match
                                let indices = crate::ui::widgets::matching_line_indices(
                                    &query,
                                    self.previous_view,
                                    self.help_show_all,
                                );
                                if let Some(&first) = indices.first() {
                                    self.help_scroll = first;
                                }
                            }
                        }
                        KeyCode::Char(c)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            self.help_input_buffer.push(c);
                        }
                        KeyCode::Backspace => {
                            self.help_input_buffer.pop();
                        }
                        _ => {}
                    }
                } else {
                    // Normal mode: scrolling + search start + n/N navigation
                    if keys::is_move_down(key.code) {
                        self.help_scroll = self.help_scroll.saturating_add(1);
                    } else if keys::is_move_up(key.code) {
                        self.help_scroll = self.help_scroll.saturating_sub(1);
                    } else if key.code == keys::GO_BOTTOM {
                        self.help_scroll = u16::MAX; // clamped during render
                    } else if key.code == keys::GO_TOP {
                        self.help_scroll = 0;
                    } else if key.code == keys::SEARCH_INPUT {
                        self.help_search_input = true;
                        self.help_input_buffer.clear();
                    } else if key.code == keys::SEARCH_NEXT {
                        if let Some(ref query) = self.help_search_query {
                            let indices = crate::ui::widgets::matching_line_indices(
                                query,
                                self.previous_view,
                                self.help_show_all,
                            );
                            if let Some(next) = indices.iter().find(|&&i| i > self.help_scroll) {
                                self.help_scroll = *next;
                            } else if let Some(&first) = indices.first() {
                                // Wrap to top
                                self.help_scroll = first;
                            }
                        }
                    } else if key.code == keys::SEARCH_PREV
                        && let Some(ref query) = self.help_search_query
                    {
                        let indices = crate::ui::widgets::matching_line_indices(
                            query,
                            self.previous_view,
                            self.help_show_all,
                        );
                        if let Some(prev) = indices.iter().rev().find(|&&i| i < self.help_scroll) {
                            self.help_scroll = *prev;
                        } else if let Some(&last) = indices.last() {
                            // Wrap to bottom
                            self.help_scroll = last;
                        }
                    } else if key.code == keys::HELP_TOGGLE_ALL {
                        self.help_show_all = !self.help_show_all;
                        self.help_scroll = 0;
                    }
                }
            }
        }
    }

    fn handle_log_action(&mut self, action: LogAction) {
        match action {
            LogAction::None => {}

            // Navigation
            LogAction::OpenDiff(_)
            | LogAction::ShowStackDiff(_)
            | LogAction::ShowTraces { .. }
            | LogAction::ExecuteRevset(_)
            | LogAction::ClearRevset
            | LogAction::OpenBookmarkView
            | LogAction::OpenTagView
            | LogAction::OpenWorkspaceView
            | LogAction::OpenCommandHistory
            | LogAction::OpenEvolog(_)
            | LogAction::OpenResolveList { .. } => {
                self.handle_log_navigation(action);
            }

            // Editing
            LogAction::StartDescribe(_)
            | LogAction::Describe { .. }
            | LogAction::DescribeExternal(_)
            | LogAction::Edit(_)
            | LogAction::NewChange
            | LogAction::NewChangeFrom { .. }
            | LogAction::NewChangeFromCurrent
            | LogAction::SquashInto { .. }
            | LogAction::Abandon(_)
            | LogAction::Split(_)
            | LogAction::Duplicate(_)
            | LogAction::DiffEdit(_)
            | LogAction::Revert(_)
            | LogAction::SimplifyParents(_)
            | LogAction::Fix { .. }
            | LogAction::Metaedit { .. } => {
                self.handle_log_editing(action);
            }

            // Bookmark
            LogAction::CreateBookmark { .. }
            | LogAction::StartBookmarkDelete
            | LogAction::StartBookmarkJump
            | LogAction::AdvanceBookmark => {
                self.handle_log_bookmark(action);
            }

            // Git
            LogAction::Fetch | LogAction::StartPush | LogAction::StartTrack => {
                self.handle_log_git(action);
            }

            // Rebase / Parallelize
            LogAction::Rebase { .. }
            | LogAction::Absorb
            | LogAction::StartParallelize(_)
            | LogAction::Parallelize { .. }
            | LogAction::ParallelizeSameRevision => {
                self.handle_log_rebase(action);
            }

            // Compare / Interdiff
            LogAction::StartCompare(_)
            | LogAction::Compare { .. }
            | LogAction::CompareSameRevision
            | LogAction::StartInterdiff(_)
            | LogAction::Interdiff { .. }
            | LogAction::InterdiffSameRevision => {
                self.handle_log_compare(action);
            }

            // Bisect
            LogAction::StartBisect(_)
            | LogAction::Bisect { .. }
            | LogAction::BisectSameRevision => {
                self.handle_log_bisect(action);
            }

            // Arrange (interactive TUI)
            LogAction::Arrange => {
                self.execute_arrange();
            }

            // Misc
            LogAction::NextChange | LogAction::PrevChange | LogAction::ToggleReversed => {
                self.handle_log_misc(action);
            }
        }
    }

    fn handle_log_navigation(&mut self, action: LogAction) {
        match action {
            LogAction::OpenDiff(change_id) => self.open_diff(&change_id),
            LogAction::ShowStackDiff(change_id) => self.open_stack_diff(&change_id),
            LogAction::ShowTraces {
                change_id,
                commit_id,
            } => self.open_trace_detail(&change_id, &commit_id),
            LogAction::ExecuteRevset(revset) => self.refresh_log(Some(&revset)),
            LogAction::ClearRevset => self.refresh_log(None),
            LogAction::OpenBookmarkView => self.open_bookmark_view(),
            LogAction::OpenTagView => self.open_tag_view(),
            LogAction::OpenWorkspaceView => self.open_workspace_view(),
            LogAction::OpenCommandHistory => self.go_to_view(View::CommandHistory),
            LogAction::OpenEvolog(change_id) => self.open_evolog(&change_id),
            LogAction::OpenResolveList {
                revision,
                is_working_copy,
            } => self.open_resolve_view(&revision, is_working_copy),
            _ => {}
        }
    }

    fn handle_log_editing(&mut self, action: LogAction) {
        use crate::ui::components::{Dialog, DialogCallback, SelectItem};

        match action {
            LogAction::StartDescribe(revision) => self.start_describe_input(&revision),
            LogAction::Describe { revision, message } => {
                self.execute_describe(&revision, &message);
            }
            LogAction::DescribeExternal(revision) => self.execute_describe_external(&revision),
            LogAction::Edit(revision) => self.execute_edit(&revision),
            LogAction::NewChange => self.execute_new_change(),
            LogAction::NewChangeFrom {
                revision,
                display_name,
            } => self.execute_new_change_from(&revision, &display_name),
            LogAction::NewChangeFromCurrent => {
                self.notify_info("Use 'c' to create from current change");
            }
            LogAction::SquashInto {
                source,
                destination,
            } => self.execute_squash_into(&source, &destination),
            LogAction::Abandon(revision) => self.execute_abandon(&revision),
            LogAction::Split(revision) => self.execute_split(&revision),
            LogAction::Duplicate(revision) => self.duplicate(&revision),
            LogAction::DiffEdit(revision) => self.execute_diffedit(&revision, None),
            LogAction::Revert(revision) => {
                let short_id = short_id(&revision);
                self.active_dialog = Some(Dialog::confirm(
                    "Revert Change",
                    format!("Revert changes from {}?", short_id),
                    Some(
                        "Creates a new commit that undoes these changes. Undo with 'u' if needed."
                            .to_string(),
                    ),
                    DialogCallback::Revert { revision },
                ));
            }
            LogAction::SimplifyParents(revision) => {
                let short_id = short_id(&revision);
                self.active_dialog = Some(Dialog::confirm(
                    "Simplify Parents",
                    format!("Simplify parents for {}?", short_id),
                    None,
                    DialogCallback::SimplifyParents { revision },
                ));
            }
            LogAction::Fix {
                revision,
                change_id,
            } => {
                if self.jj.is_immutable(&revision) {
                    self.set_error("Cannot fix: commit is immutable");
                    return;
                }
                let short_id = short_id(&revision);
                let items = vec![
                    SelectItem {
                        label: "Default (respect fix.tools line-range-arg)".to_string(),
                        value: "default".to_string(),
                        selected: false,
                    },
                    SelectItem {
                        label: "All lines (--all-lines, format entire files)".to_string(),
                        value: "all-lines".to_string(),
                        selected: false,
                    },
                ];
                self.active_dialog = Some(Dialog::select_single(
                    "Fix",
                    format!("Apply code formatters to {} and descendants", short_id),
                    items,
                    None,
                    DialogCallback::Fix {
                        revision,
                        change_id,
                    },
                ));
            }
            LogAction::Metaedit {
                change_id,
                commit_id,
            } => {
                if self.jj.is_immutable(&commit_id) {
                    self.set_error("Cannot metaedit: commit is immutable");
                    return;
                }
                let short = short_id(&change_id);
                let items = vec![
                    SelectItem {
                        label: "Update author".to_string(),
                        value: "update-author".to_string(),
                        selected: false,
                    },
                    SelectItem {
                        label: "Set author...".to_string(),
                        value: "set-author".to_string(),
                        selected: false,
                    },
                    SelectItem {
                        label: "Update author timestamp".to_string(),
                        value: "update-timestamp".to_string(),
                        selected: false,
                    },
                    SelectItem {
                        label: "Generate new change-id".to_string(),
                        value: "new-change-id".to_string(),
                        selected: false,
                    },
                    SelectItem {
                        label: "Force rewrite".to_string(),
                        value: "force-rewrite".to_string(),
                        selected: false,
                    },
                ];
                self.active_dialog = Some(Dialog::select_single(
                    "Metaedit",
                    format!("Edit metadata for {}", short),
                    items,
                    None,
                    DialogCallback::MetaeditSelect {
                        commit_id,
                        change_id,
                    },
                ));
            }
            _ => {}
        }
    }

    fn handle_log_bookmark(&mut self, action: LogAction) {
        match action {
            LogAction::CreateBookmark { revision, name } => {
                self.execute_bookmark_create(&revision, &name);
            }
            LogAction::StartBookmarkDelete => self.start_bookmark_delete(),
            LogAction::StartBookmarkJump => self.start_bookmark_jump(),
            LogAction::AdvanceBookmark => {
                self.start_bookmark_advance();
            }
            _ => {}
        }
    }

    fn handle_log_git(&mut self, action: LogAction) {
        match action {
            LogAction::Fetch => self.start_fetch(),
            LogAction::StartPush => self.start_push(),
            LogAction::StartTrack => self.start_track(),
            _ => {}
        }
    }

    fn handle_log_rebase(&mut self, action: LogAction) {
        use crate::ui::components::{Dialog, DialogCallback};

        match action {
            LogAction::Rebase {
                source,
                destination,
                mode,
                skip_emptied,
                use_revset,
                simplify_parents,
            } => self.execute_rebase(
                &source,
                &destination,
                mode,
                skip_emptied,
                simplify_parents,
                use_revset,
            ),
            LogAction::Absorb => self.execute_absorb(),
            LogAction::StartParallelize(from_id) => {
                self.notify_info(format!("From: {}. Select end and press Enter", from_id));
            }
            LogAction::Parallelize { from, to } => {
                let from_short = &from[..8.min(from.len())];
                let to_short = &to[..8.min(to.len())];
                self.active_dialog = Some(Dialog::confirm(
                    "Parallelize",
                    format!("Parallelize {}::{}?", from_short, to_short),
                    None,
                    DialogCallback::Parallelize { from, to },
                ));
            }
            LogAction::ParallelizeSameRevision => {
                self.notify_info("Cannot parallelize single revision");
            }
            _ => {}
        }
    }

    fn handle_log_compare(&mut self, action: LogAction) {
        match action {
            LogAction::StartCompare(from_id) => {
                self.notify_info(format!("From: {}. Select 'To' and press Enter", from_id));
            }
            LogAction::Compare { ref from, ref to } => {
                let msg = format!("Comparing {} -> {}", from, to);
                self.open_compare_diff(from, to);
                if self.error_message.is_none() {
                    self.notify_info(&msg);
                }
            }
            LogAction::CompareSameRevision => {
                self.notify_info("Cannot compare revision with itself");
            }
            LogAction::StartInterdiff(from_id) => {
                self.notify_info(format!(
                    "Interdiff From: {}. Select 'To' and press Enter",
                    from_id
                ));
            }
            LogAction::Interdiff { ref from, ref to } => {
                let msg = format!("Interdiff {} -> {}", from, to);
                self.open_interdiff(from, to);
                if self.error_message.is_none() {
                    self.notify_info(&msg);
                }
            }
            LogAction::InterdiffSameRevision => {
                self.notify_info("Cannot interdiff revision with itself");
            }
            _ => {}
        }
    }

    fn handle_log_bisect(&mut self, action: LogAction) {
        use crate::ui::components::{Dialog, DialogCallback};

        match action {
            LogAction::StartBisect(bad_id) => {
                self.notify_info(format!(
                    "Bisect: Bad={}. Select Good and press Enter",
                    bad_id
                ));
            }
            LogAction::Bisect { good, bad } => {
                self.active_dialog = Some(Dialog::input(
                    "Bisect Command (empty = bash for manual test)",
                    "bash",
                    DialogCallback::BisectRun { good, bad },
                ));
            }
            LogAction::BisectSameRevision => {
                self.notify_info("Cannot bisect: same revision selected for good and bad");
            }
            _ => {}
        }
    }

    fn handle_log_misc(&mut self, action: LogAction) {
        match action {
            LogAction::NextChange => self.execute_next(),
            LogAction::PrevChange => self.execute_prev(),
            LogAction::ToggleReversed => {
                let selected_id = self
                    .log_view
                    .selected_change()
                    .map(|c| c.change_id.to_string());
                self.log_view.reversed = !self.log_view.reversed;
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                if let Some(ref id) = selected_id
                    && !self.log_view.select_change_by_id(id)
                {
                    self.log_view.select_working_copy();
                }
                let label = if self.log_view.reversed {
                    "oldest first"
                } else {
                    "newest first"
                };
                self.notify_info(format!("Log order: {}", label));
            }
            _ => {}
        }
    }

    fn handle_bookmark_action(&mut self, action: BookmarkAction) {
        match action {
            BookmarkAction::None => {}
            // Open the create dialog with the captured target revision (default @).
            BookmarkAction::StartCreate => {
                use crate::ui::components::{Dialog, DialogCallback};
                let target = self.create_target_revset();
                let display = short_id(&target);
                self.active_dialog = Some(Dialog::input(
                    "Create Bookmark",
                    format!("New bookmark on {}", display),
                    DialogCallback::BookmarkCreate { revision: target },
                ));
            }
            BookmarkAction::Jump(change_id) => {
                self.execute_bookmark_jump(&change_id);
                self.go_to_view(View::Log);
            }
            BookmarkAction::Track(full_name) => {
                self.execute_track(&[full_name]);
            }
            BookmarkAction::Untrack(full_name) => {
                self.execute_untrack(&full_name);
            }
            BookmarkAction::Delete(name) => {
                self.execute_bookmark_delete(&[name]);
            }
            BookmarkAction::StartRename(old_name) => {
                self.bookmark_view.rename_state = Some(RenameState::new(old_name));
            }
            BookmarkAction::ConfirmRename { old_name, new_name } => {
                self.execute_bookmark_rename(&old_name, &new_name);
            }
            BookmarkAction::CancelRename => {
                // rename_state already cleared by BookmarkView
            }
            BookmarkAction::Forget(name) => {
                use crate::ui::components::{Dialog, DialogCallback};
                self.active_dialog = Some(Dialog::confirm(
                    "Forget Bookmark",
                    format!(
                        "Forget bookmark '{}'?\n\n\
                         This removes remote tracking.\n\
                         Use 'd' for local delete only.\n\
                         Undo with 'u' if needed.",
                        name
                    ),
                    None,
                    DialogCallback::BookmarkForget,
                ));
                self.pending_forget_bookmark = Some(name);
            }
            BookmarkAction::Move(name) => {
                self.start_bookmark_move(&name);
            }
            BookmarkAction::MoveUnavailable => {
                self.notify_info("Move is available only for local bookmarks");
            }
        }
    }

    fn handle_tag_action(&mut self, action: TagAction) {
        match action {
            TagAction::None => {}
            TagAction::Jump(change_id) => {
                self.jump_to_log(&change_id);
            }
            TagAction::StartCreate => {
                use crate::ui::components::{Dialog, DialogCallback};
                let target = self.create_target_revset();
                let display = short_id(&target);
                self.active_dialog = Some(Dialog::input(
                    "Create Tag",
                    format!("New tag on {}", display),
                    DialogCallback::TagCreate { revision: target },
                ));
            }
            TagAction::Delete(name) => {
                use crate::ui::components::{Dialog, DialogCallback};
                self.active_dialog = Some(Dialog::confirm(
                    "Delete Tag",
                    format!("Delete tag '{}'?", name),
                    None,
                    DialogCallback::TagDelete { name: name.clone() },
                ));
            }
        }
    }

    fn handle_diff_action(&mut self, action: DiffAction) {
        match action {
            DiffAction::None => {}
            DiffAction::Back => {
                self.go_back();
            }
            DiffAction::OpenBlame { file_path } => {
                // Get the current change_id from diff_view for proper revision
                let revision = self.diff_view.as_ref().map(|v| v.revision.clone());
                self.open_blame(&file_path, revision.as_deref());
            }
            DiffAction::ShowNotification(message) => {
                self.notify_info(&message);
            }
            DiffAction::CopyToClipboard { full } => {
                self.copy_diff_to_clipboard(full);
            }
            DiffAction::ExportToFile => {
                self.export_diff_to_file();
            }
            DiffAction::CycleFormat => {
                self.cycle_diff_format();
            }
        }
    }

    fn handle_status_action(&mut self, action: StatusAction) {
        match action {
            StatusAction::None => {}
            StatusAction::ShowFileDiff {
                change_id,
                file_path,
            } => {
                self.open_diff_at_file(&change_id, &file_path);
            }
            StatusAction::OpenBlame { file_path } => {
                self.open_blame(&file_path, None);
            }
            StatusAction::Commit { message } => {
                self.execute_commit(&message);
            }
            StatusAction::JumpToConflict => {
                // Selection already moved by StatusView; no further action needed
            }
            StatusAction::RestoreFile { file_path } => {
                // Show confirm dialog before restoring
                use crate::ui::components::{Dialog, DialogCallback};
                self.active_dialog = Some(Dialog::confirm(
                    "Restore File",
                    format!(
                        "Restore '{}'?\nThis discards your changes to this file.",
                        file_path
                    ),
                    Some("Undo with 'u' if needed.".to_string()),
                    DialogCallback::RestoreFile {
                        file_path: file_path.clone(),
                    },
                ));
            }
            StatusAction::RestoreAll => {
                use crate::ui::components::{Dialog, DialogCallback};
                self.active_dialog = Some(Dialog::confirm(
                    "Restore All Files",
                    "Restore all files?\nThis discards ALL your changes in the working copy.",
                    Some("Undo with 'u' if needed.".to_string()),
                    DialogCallback::RestoreAll,
                ));
            }
            StatusAction::DiffEdit { file_path } => {
                self.execute_diffedit("@", Some(&file_path));
            }
        }
    }

    fn handle_operation_action(&mut self, action: OperationAction) {
        match action {
            OperationAction::None => {}
            OperationAction::Back => {
                self.go_back();
            }
            OperationAction::Restore(operation_id) => {
                self.execute_op_restore(&operation_id);
            }
        }
    }

    fn handle_resolve_action(&mut self, action: ResolveAction) {
        match action {
            ResolveAction::None => {}
            ResolveAction::Back => {
                self.resolve_view = None;
                // Mark log dirty so conflict indicators update on return
                self.dirty.log = true;
                self.dirty.op_log = true;
                self.go_back();
            }
            ResolveAction::ResolveExternal(file_path) => {
                self.execute_resolve_external(&file_path);
            }
            ResolveAction::ResolveOurs(file_path) => {
                self.execute_resolve_ours(&file_path);
            }
            ResolveAction::ResolveTheirs(file_path) => {
                self.execute_resolve_theirs(&file_path);
            }
            ResolveAction::ShowDiff(file_path) => {
                // Open diff for the change, jumping to the file
                let revision = self
                    .resolve_view
                    .as_ref()
                    .map(|v| v.revision.clone())
                    .unwrap_or_default();
                self.open_diff_at_file(&revision, &file_path);
            }
        }
    }

    fn handle_evolog_action(&mut self, action: EvologAction) {
        match action {
            EvologAction::None => {}
            EvologAction::Back => {
                self.go_back();
            }
            EvologAction::OpenDiff(change_id) => {
                self.open_diff(&change_id);
            }
        }
    }

    fn handle_command_history_action(&mut self, action: CommandHistoryAction) {
        match action {
            CommandHistoryAction::None => {}
            CommandHistoryAction::Back => {
                self.go_back();
            }
            CommandHistoryAction::ToggleDetail(_) => {
                // Detail toggle is handled internally by CommandHistoryView
            }
        }
    }

    fn handle_blame_action(&mut self, action: BlameAction) {
        match action {
            BlameAction::None => {}
            BlameAction::Back => {
                self.go_back();
            }
            BlameAction::OpenDiff(change_id) => {
                self.open_diff(&change_id);
            }
            BlameAction::JumpToLog(change_id) => {
                self.jump_to_log(&change_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    /// Simulate a key press on the App (through on_key_event, which runs
    /// handle_global_key before handle_view_key).
    fn press(app: &mut App, code: KeyCode) {
        app.on_key_event(KeyEvent::from(code));
    }

    /// Put App into Help view with search input active.
    fn enter_help_search(app: &mut App) {
        app.current_view = View::Help;
        app.help_search_input = true;
        app.help_input_buffer.clear();
    }

    // =========================================================================
    // Help search input: global key conflict tests
    // =========================================================================

    #[test]
    fn help_search_esc_cancels_search_not_back() {
        let mut app = App::new_for_test();
        enter_help_search(&mut app);

        press(&mut app, KeyCode::Esc);

        // Search input should be cancelled
        assert!(!app.help_search_input);
        // Should still be on Help view (not navigated back)
        assert_eq!(app.current_view, View::Help);
    }

    #[test]
    fn help_search_q_types_character_not_quit() {
        let mut app = App::new_for_test();
        enter_help_search(&mut app);

        press(&mut app, KeyCode::Char('q'));

        // 'q' should be added to input buffer
        assert_eq!(app.help_input_buffer, "q");
        // Should still be in search input mode
        assert!(app.help_search_input);
        // Should still be on Help view and running
        assert_eq!(app.current_view, View::Help);
        assert!(app.running);
    }

    #[test]
    fn help_search_tab_stays_in_help_not_switch() {
        let mut app = App::new_for_test();
        enter_help_search(&mut app);

        press(&mut app, KeyCode::Tab);

        // Should still be on Help view (not switched to next view)
        assert_eq!(app.current_view, View::Help);
        // Should still be in search input mode
        assert!(app.help_search_input);
    }

    #[test]
    fn help_search_question_mark_stays_in_search() {
        let mut app = App::new_for_test();
        enter_help_search(&mut app);

        press(&mut app, KeyCode::Char('?'));

        // '?' should be added to input buffer (it's a Char)
        assert_eq!(app.help_input_buffer, "?");
        // Should still be in search input mode on Help view
        assert!(app.help_search_input);
        assert_eq!(app.current_view, View::Help);
    }

    #[test]
    fn help_search_enter_confirms_and_exits_input() {
        let mut app = App::new_for_test();
        enter_help_search(&mut app);
        app.help_input_buffer = "quit".to_string();

        press(&mut app, KeyCode::Enter);

        // Search input should be deactivated
        assert!(!app.help_search_input);
        // Query should be stored
        assert_eq!(app.help_search_query, Some("quit".to_string()));
        // Should still be on Help view
        assert_eq!(app.current_view, View::Help);
    }

    #[test]
    fn help_search_typing_multiple_chars() {
        let mut app = App::new_for_test();
        enter_help_search(&mut app);

        press(&mut app, KeyCode::Char('h'));
        press(&mut app, KeyCode::Char('e'));
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Char('p'));

        assert_eq!(app.help_input_buffer, "help");
        assert!(app.help_search_input);
    }

    #[test]
    fn help_search_backspace_removes_char() {
        let mut app = App::new_for_test();
        enter_help_search(&mut app);
        app.help_input_buffer = "test".to_string();

        press(&mut app, KeyCode::Backspace);

        assert_eq!(app.help_input_buffer, "tes");
        assert!(app.help_search_input);
    }

    #[test]
    fn help_search_ctrl_l_suppressed() {
        let mut app = App::new_for_test();
        enter_help_search(&mut app);

        // Ctrl+L should NOT trigger refresh while in search input mode
        app.on_key_event(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));

        // Should still be in search input mode on Help view
        assert!(app.help_search_input);
        assert_eq!(app.current_view, View::Help);
    }

    #[test]
    fn help_normal_mode_esc_goes_back() {
        let mut app = App::new_for_test();
        app.go_to_view(View::Help); // sets previous_view to Log
        assert!(!app.help_search_input);

        press(&mut app, KeyCode::Esc);

        // Should navigate back (not Help anymore)
        assert_ne!(app.current_view, View::Help);
    }

    #[test]
    fn help_normal_mode_q_goes_back() {
        let mut app = App::new_for_test();
        app.go_to_view(View::Help);
        assert!(!app.help_search_input);

        press(&mut app, KeyCode::Char('q'));

        // Should go back from Help view
        assert_ne!(app.current_view, View::Help);
    }

    // =========================================================================
    // Bookmark view opener (Task 48-A)
    // =========================================================================

    #[test]
    fn b_opens_bookmark_view_directly() {
        let mut app = App::new_for_test();
        app.current_view = View::Log;
        app.on_key_event(KeyEvent::from(KeyCode::Char('b')));
        assert_eq!(
            app.current_view,
            View::Bookmark,
            "b opens Bookmark View (no chord)"
        );
    }

    // =========================================================================
    // Command palette (Phase 46-C)
    // =========================================================================

    #[test]
    fn palette_colon_opens_in_log_normal() {
        let mut app = App::new_for_test();
        app.current_view = View::Log;
        press(&mut app, KeyCode::Char(':'));
        assert!(app.palette_active);
    }

    #[test]
    fn palette_esc_closes() {
        let mut app = App::new_for_test();
        app.current_view = View::Log;
        press(&mut app, KeyCode::Char(':'));
        press(&mut app, KeyCode::Esc);
        assert!(!app.palette_active);
    }

    #[test]
    fn palette_typing_filters_and_resets_selection() {
        let mut app = App::new_for_test();
        app.current_view = View::Log;
        press(&mut app, KeyCode::Char(':'));
        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Char('i'));
        press(&mut app, KeyCode::Char('x'));
        assert_eq!(app.palette_input, "fix");
        assert_eq!(app.palette_selected, 0);
    }

    #[test]
    fn palette_not_opened_outside_log() {
        let mut app = App::new_for_test();
        app.current_view = View::Status;
        press(&mut app, KeyCode::Char(':'));
        assert!(!app.palette_active);
    }

    #[test]
    fn palette_cleared_on_view_change() {
        let mut app = App::new_for_test();
        app.current_view = View::Log;
        press(&mut app, KeyCode::Char(':'));
        assert!(app.palette_active);
        app.go_to_view(View::Status);
        assert!(!app.palette_active);
    }

    #[test]
    fn bookmark_rename_captures_global_keys_as_text() {
        // Rename mode must intercept global keys (q/?/Tab/:) as text input,
        // not let them quit / open help / switch view.
        let mut app = App::new_for_test();
        app.current_view = View::Bookmark;
        app.bookmark_view.rename_state = Some(RenameState::new("old".to_string()));

        press(&mut app, KeyCode::Char('q'));
        assert!(
            app.bookmark_view.rename_state.is_some(),
            "q must be typed into rename, not quit"
        );
        assert!(app.running, "q must not quit the app during rename");

        press(&mut app, KeyCode::Char('?'));
        assert_eq!(
            app.current_view,
            View::Bookmark,
            "? must not open Help during rename"
        );
    }

    #[test]
    fn palette_enter_dispatches_command_history() {
        // Core dispatch path: filter to a jj-independent command and Enter it.
        // `command-history` opens View::CommandHistory via the delegated key.
        let mut app = App::new_for_test();
        app.current_view = View::Log;
        press(&mut app, KeyCode::Char(':'));
        for c in "command-history".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);
        assert!(!app.palette_active, "palette closes after Enter");
        assert_eq!(
            app.current_view,
            View::CommandHistory,
            "Enter dispatched the command (via log_view.handle_key delegation)"
        );
    }

    // =========================================================================
    // Phase 48-B2: create_target capture and create dialogs
    // =========================================================================

    #[test]
    fn opening_bookmark_view_from_log_captures_selected_change() {
        use crate::model::{Change, ChangeId, CommitId};

        let mut app = App::new_for_test();

        // Pre-load a real change so selected_change() returns Some(...)
        let test_change_id = "abc12345";
        app.log_view.set_changes(vec![Change {
            change_id: ChangeId::new(test_change_id.to_string()),
            commit_id: CommitId::new("def67890".to_string()),
            author: "user@example.com".to_string(),
            timestamp: "2024-01-29".to_string(),
            description: "Test commit".to_string(),
            is_working_copy: true,
            is_empty: false,
            bookmarks: vec![],
            graph_prefix: "@  ".to_string(),
            is_graph_only: false,
            has_conflict: false,
            working_copy_names: vec![],
        }]);

        // Confirm the change is selected before navigation
        let expected = app
            .log_view
            .selected_change()
            .map(|c| c.change_id.to_string());
        assert!(
            expected.is_some(),
            "pre-condition: log must have a selected change before navigating"
        );

        app.on_key_event(KeyEvent::from(KeyCode::Char('b'))); // Log -> Bookmark

        assert_eq!(
            app.create_target, expected,
            "create_target = Log-selected change"
        );
        assert!(
            app.create_target.is_some(),
            "must capture a real change id, not None"
        );
        assert_eq!(
            app.create_target.as_deref(),
            Some(test_change_id),
            "captured change id must match the loaded change"
        );
    }

    #[test]
    fn create_target_revset_defaults_to_at() {
        let mut app = App::new_for_test();
        app.create_target = None;
        assert_eq!(app.create_target_revset(), "@");
    }

    #[test]
    fn n_in_bookmark_view_opens_create_dialog_with_target() {
        let mut app = App::new_for_test();
        app.go_to_view(View::Bookmark); // capture happens here from real Log selection
        // Press n -> create dialog opens
        app.on_key_event(KeyEvent::from(KeyCode::Char('n')));
        // Assert a create input dialog is active
        let dialog = app.active_dialog.as_ref().expect("n opens a create dialog");
        assert!(
            matches!(dialog.kind, crate::ui::components::DialogKind::Input { .. }),
            "Expected Input dialog, got: {:?}",
            dialog.kind
        );
        // Assert dialog carries the BookmarkCreate callback with the captured target
        let target = app.create_target_revset();
        assert_eq!(
            dialog.callback_id,
            crate::ui::components::DialogCallback::BookmarkCreate {
                revision: target.clone()
            },
            "Dialog callback should carry the captured target revision"
        );
        // Assert prompt text mentions the target revset
        if let crate::ui::components::DialogKind::Input { message, .. } = &dialog.kind {
            assert!(
                message.contains(&target),
                "Dialog prompt '{}' should mention target '{}'",
                message,
                target
            );
        }
    }

    // =========================================================================
    // Phase 48-C: palette-only commands must be inert as single keys
    // =========================================================================

    #[test]
    fn log_does_not_bind_palette_only_keys() {
        use crate::model::{Change, ChangeId, CommitId};

        let mut app = App::new_for_test();
        // Load a real, non-empty change so that — if any of these keys were still
        // bound — they would actually fire (enter a select mode, open a dialog,
        // or change view). This makes the inert assertions meaningful.
        app.log_view.set_changes(vec![Change {
            change_id: ChangeId::new("abc12345".to_string()),
            commit_id: CommitId::new("def67890".to_string()),
            author: "user@example.com".to_string(),
            timestamp: "2024-01-29".to_string(),
            description: "Test commit".to_string(),
            is_working_copy: false,
            is_empty: false,
            bookmarks: vec![],
            graph_prefix: "○  ".to_string(),
            is_graph_only: false,
            has_conflict: false,
            working_copy_names: vec![],
        }]);

        // The 11 low-frequency single keys removed in Phase 48-C.
        for c in ['I', '=', 'Y', 'E', 'Z', 'i', '|', 'v', 'O', 'W', 'f'] {
            let before = app.current_view;
            press(&mut app, KeyCode::Char(c));
            assert_eq!(
                app.current_view, before,
                "'{c}' must be inert as a single key (no view change)"
            );
            assert!(app.active_dialog.is_none(), "'{c}' must not open a dialog");
            assert_eq!(
                app.log_view.input_mode,
                InputMode::Normal,
                "'{c}' must not enter any select/input mode"
            );
        }
    }

    #[test]
    fn palette_enter_dispatches_compare_command() {
        use crate::model::{Change, ChangeId, CommitId};
        use crate::ui::components::{filter_commands, palette_commands};

        let mut app = App::new_for_test();
        app.log_view.set_changes(vec![Change {
            change_id: ChangeId::new("abc12345".to_string()),
            commit_id: CommitId::new("def67890".to_string()),
            author: "user@example.com".to_string(),
            timestamp: "2024-01-29".to_string(),
            description: "Test commit".to_string(),
            is_working_copy: false,
            is_empty: false,
            bookmarks: vec![],
            graph_prefix: "○  ".to_string(),
            is_graph_only: false,
            has_conflict: false,
            working_copy_names: vec![],
        }]);

        // Open the palette and type "compare" to select that entry.
        app.palette_active = true;
        app.palette_input = "compare".to_string();
        let filtered = filter_commands(palette_commands(), &app.palette_input);
        assert_eq!(filtered.len(), 1, "exactly one 'compare' match expected");
        app.palette_selected = 0;

        // Enter dispatches the compare command via command_action.
        app.on_key_event(KeyEvent::from(KeyCode::Enter));

        assert!(!app.palette_active, "palette closes after Enter");
        assert_eq!(
            app.log_view.input_mode,
            InputMode::CompareSelect,
            "palette 'compare' must enter compare-select mode"
        );
    }

    // =========================================================================
    // Phase 48-D: undo/redo gating across views
    // =========================================================================

    #[test]
    fn undo_redo_active_in_mutating_views_only() {
        for v in [
            View::Log,
            View::Status,
            View::Bookmark,
            View::Tag,
            View::Workspace,
            View::Operation,
            View::Resolve,
        ] {
            assert!(App::view_allows_undo(v), "{v:?} must allow undo/redo");
        }
        for v in [
            View::Diff,
            View::Blame,
            View::Evolog,
            View::CommandHistory,
            View::TraceDetail,
            View::Help,
        ] {
            assert!(!App::view_allows_undo(v), "{v:?} must NOT allow undo/redo");
        }
    }

    #[test]
    fn redo_ctrl_r_gated_by_view_and_special_mode() {
        let mut app = App::new_for_test();

        // Mutating view (Bookmark), normal mode: redo is allowed.
        app.current_view = View::Bookmark;
        assert!(App::view_allows_undo(app.current_view));
        assert!(!app.in_special_input_mode(), "bookmark normal: not special");

        // Bookmark rename mode suppresses undo/redo (^R must reach the input).
        app.bookmark_view.rename_state =
            Some(crate::ui::views::RenameState::new("feature-x".to_string()));
        assert!(
            app.in_special_input_mode(),
            "bookmark rename must be a special mode"
        );
        app.bookmark_view.rename_state = None;

        // Status input mode suppresses undo/redo.
        app.current_view = View::Status;
        app.status_view.input_mode = StatusInputMode::CommitInput;
        assert!(
            app.in_special_input_mode(),
            "status commit input must be a special mode"
        );
        app.status_view.input_mode = StatusInputMode::Normal;
        assert!(!app.in_special_input_mode());

        // Read-only view (Diff): redo is not allowed regardless of mode.
        app.current_view = View::Diff;
        assert!(
            !App::view_allows_undo(app.current_view),
            "Diff must not allow redo"
        );
    }

    #[test]
    fn log_input_mode_suppresses_undo_redo() {
        let mut app = App::new_for_test();
        app.current_view = View::Log;
        assert!(!app.in_special_input_mode(), "log normal: not special");

        app.log_view.input_mode = InputMode::SearchInput;
        assert!(
            app.in_special_input_mode(),
            "log search input must suppress undo/redo"
        );
    }
}
