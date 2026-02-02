//! Input handling for the application

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::state::{App, View};
use crate::keys;
use crate::ui::views::{DiffAction, InputMode, LogAction, StatusAction};

impl App {
    /// Handle key events
    pub fn on_key_event(&mut self, key: KeyEvent) {
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
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'))
            && self.current_view == View::Log
            && self.log_view.input_mode == InputMode::Normal
        {
            self.notification = None; // Clear any existing notification
            self.execute_redo();
            return;
        }

        // If in input mode, delegate all keys to LogView (skip global handling)
        if self.current_view == View::Log && self.log_view.input_mode != InputMode::Normal {
            let action = self.log_view.handle_key(key);
            self.handle_log_action(action);
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
            keys::UNDO if self.current_view == View::Log => {
                self.notification = None; // Clear any existing notification
                self.execute_undo();
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
                let action = self.log_view.handle_key(key);
                self.handle_log_action(action);
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
            View::Help => {
                // Help view only uses global keys
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
            LogAction::Describe { change_id, message } => {
                self.execute_describe(&change_id, &message);
            }
            LogAction::Edit(change_id) => {
                self.execute_edit(&change_id);
            }
            LogAction::NewChange => {
                self.execute_new_change();
            }
        }
    }

    fn handle_diff_action(&mut self, action: DiffAction) {
        match action {
            DiffAction::None => {}
            DiffAction::Back => {
                self.go_back();
            }
        }
    }

    fn handle_status_action(&mut self, action: StatusAction) {
        match action {
            StatusAction::None => {}
            StatusAction::Back => {
                self.go_back();
            }
            StatusAction::SwitchToLog => {
                self.go_to_view(View::Log);
            }
            StatusAction::ShowFileDiff {
                change_id,
                file_path,
            } => {
                self.open_diff_at_file(&change_id, &file_path);
            }
        }
    }
}
