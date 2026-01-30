//! Input handling for the application

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::state::{App, View};
use crate::keys;
use crate::ui::views::{DiffAction, InputMode, LogAction};

impl App {
    /// Handle key events
    pub fn on_key_event(&mut self, key: KeyEvent) {
        // Clear error message on any key press
        self.error_message = None;

        // Handle Ctrl+C globally
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            self.quit();
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
                    let action = diff_view.handle_key(key);
                    self.handle_diff_action(action);
                }
            }
            View::Status => {
                // TODO: Status view key handling
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
}
