//! Input handling for the application

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::state::{App, View};
use crate::keys;
use crate::model::Notification;
use crate::ui::views::{
    BlameAction, BookmarkAction, DiffAction, EvologAction, InputMode, LogAction, OperationAction,
    RenameState, ResolveAction, StatusAction, StatusInputMode,
};

impl App {
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

        // Handle Ctrl+R for redo (only in Log view, normal mode)
        // Only active in Normal mode to avoid conflicts with input modes
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'))
            && self.current_view == View::Log
            && matches!(self.log_view.input_mode, InputMode::Normal)
        {
            self.notification = None; // Clear any existing notification
            self.execute_redo();
            return;
        }

        // Handle Ctrl+L for refresh (all views, normal mode)
        if keys::is_refresh_key(&key) {
            // Skip if in input mode or special mode (like RebaseSelect)
            let in_special_mode = match self.current_view {
                View::Log => !matches!(self.log_view.input_mode, InputMode::Normal),
                View::Status => self.status_view.input_mode != StatusInputMode::Normal,
                View::Help => self.help_search_input,
                _ => false,
            };
            if !in_special_mode {
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

        if self.handle_global_key(key) {
            return;
        }

        self.handle_view_key(key);
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
            keys::UNDO if matches!(self.current_view, View::Log | View::Bookmark) => {
                self.notification = None; // Clear any existing notification
                self.execute_undo();
                if self.current_view == View::Bookmark {
                    self.refresh_bookmark_view();
                }
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
                        // Clear pending fetch and cache on disable
                        self.preview_pending_id = None;
                        self.preview_cache = None;
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
                                let indices = crate::ui::widgets::matching_line_indices(&query);
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
                            let indices = crate::ui::widgets::matching_line_indices(query);
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
                        let indices = crate::ui::widgets::matching_line_indices(query);
                        if let Some(prev) = indices.iter().rev().find(|&&i| i < self.help_scroll) {
                            self.help_scroll = *prev;
                        } else if let Some(&last) = indices.last() {
                            // Wrap to bottom
                            self.help_scroll = last;
                        }
                    }
                }
            }
        }
    }

    fn handle_log_action(&mut self, action: LogAction) {
        match action {
            LogAction::None => {}
            LogAction::OpenDiff(change_id) => {
                self.open_diff(&change_id);
            }
            LogAction::ExecuteRevset(revset) => {
                self.refresh_log(Some(&revset));
            }
            LogAction::ClearRevset => {
                self.refresh_log(None);
            }
            LogAction::StartDescribe(change_id) => {
                self.start_describe_input(&change_id);
            }
            LogAction::Describe { change_id, message } => {
                self.execute_describe(&change_id, &message);
            }
            LogAction::DescribeExternal(change_id) => {
                self.execute_describe_external(&change_id);
            }
            LogAction::Edit(change_id) => {
                self.execute_edit(&change_id);
            }
            LogAction::NewChange => {
                self.execute_new_change();
            }
            LogAction::NewChangeFrom {
                change_id,
                display_name,
            } => {
                self.execute_new_change_from(&change_id, &display_name);
            }
            LogAction::NewChangeFromCurrent => {
                self.notification =
                    Some(Notification::info("Use 'c' to create from current change"));
            }
            LogAction::SquashInto {
                source,
                destination,
            } => {
                self.execute_squash_into(&source, &destination);
            }
            LogAction::Abandon(change_id) => {
                self.execute_abandon(&change_id);
            }
            LogAction::Split(change_id) => {
                self.execute_split(&change_id);
            }
            LogAction::CreateBookmark { change_id, name } => {
                self.execute_bookmark_create(&change_id, &name);
            }
            LogAction::StartBookmarkDelete => {
                self.start_bookmark_delete();
            }
            LogAction::Rebase {
                source,
                destination,
                mode,
            } => {
                self.execute_rebase(&source, &destination, mode);
            }
            LogAction::Absorb => {
                self.execute_absorb();
            }
            LogAction::OpenResolveList {
                change_id,
                is_working_copy,
            } => {
                self.open_resolve_view(&change_id, is_working_copy);
            }
            LogAction::Fetch => {
                self.start_fetch();
            }
            LogAction::StartPush => {
                self.start_push();
            }
            LogAction::StartTrack => {
                self.start_track();
            }
            LogAction::StartBookmarkJump => {
                self.start_bookmark_jump();
            }
            LogAction::StartCompare(from_id) => {
                self.notification = Some(Notification::info(format!(
                    "From: {}. Select 'To' and press Enter",
                    from_id
                )));
            }
            LogAction::Compare { ref from, ref to } => {
                let msg = format!("Comparing {} -> {}", from, to);
                self.open_compare_diff(from, to);
                // Show notification only if diff opened successfully (no error_message)
                if self.error_message.is_none() {
                    self.notification = Some(Notification::info(&msg));
                }
            }
            LogAction::CompareSameRevision => {
                self.notification = Some(Notification::info("Cannot compare revision with itself"));
            }
            LogAction::OpenBookmarkView => {
                self.open_bookmark_view();
            }
            LogAction::NextChange => {
                self.execute_next();
            }
            LogAction::PrevChange => {
                self.execute_prev();
            }
            LogAction::Duplicate(change_id) => {
                self.duplicate(&change_id);
            }
            LogAction::DiffEdit(change_id) => {
                self.execute_diffedit(&change_id, None);
            }
            LogAction::OpenEvolog(change_id) => {
                self.open_evolog(&change_id);
            }
            LogAction::Revert(change_id) => {
                use crate::ui::components::{Dialog, DialogCallback};
                let short_id = &change_id[..8.min(change_id.len())];
                self.active_dialog = Some(Dialog::confirm(
                    "Revert Change",
                    format!("Revert changes from {}?", short_id),
                    Some(
                        "Creates a new commit that undoes these changes. Undo with 'u' if needed."
                            .to_string(),
                    ),
                    DialogCallback::Revert { change_id },
                ));
            }
            LogAction::ToggleReversed => {
                // Preserve selection by change_id across toggle
                let selected_id = self.log_view.selected_change().map(|c| c.change_id.clone());
                self.log_view.reversed = !self.log_view.reversed;
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                // Try to restore selection; fallback to working copy
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
                self.notification = Some(Notification::info(format!("Log order: {}", label)));
            }
        }
    }

    fn handle_bookmark_action(&mut self, action: BookmarkAction) {
        match action {
            BookmarkAction::None => {}
            BookmarkAction::Jump(change_id) => {
                self.execute_bookmark_jump(&change_id);
                self.go_to_view(View::Log);
            }
            BookmarkAction::Track(full_name) => {
                self.execute_track(&[full_name]);
                self.refresh_bookmark_view();
            }
            BookmarkAction::Untrack(full_name) => {
                self.execute_untrack(&full_name);
                self.refresh_bookmark_view();
            }
            BookmarkAction::Delete(name) => {
                self.execute_bookmark_delete(&[name]);
                self.refresh_bookmark_view();
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
                         Use 'D' for local delete only.\n\
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
                self.notification = Some(Notification::info(
                    "Move is available only for local bookmarks",
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
                let revision = self.diff_view.as_ref().map(|v| v.change_id.clone());
                self.open_blame(&file_path, revision.as_deref());
            }
            DiffAction::ShowNotification(message) => {
                self.notification = Some(Notification::info(&message));
            }
            DiffAction::CopyToClipboard { full } => {
                self.copy_diff_to_clipboard(full);
            }
            DiffAction::ExportToFile => {
                self.export_diff_to_file();
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
                self.go_back();
                // Refresh log to update conflict indicators
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
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
                let change_id = self
                    .resolve_view
                    .as_ref()
                    .map(|v| v.change_id.clone())
                    .unwrap_or_default();
                self.open_diff_at_file(&change_id, &file_path);
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
}
