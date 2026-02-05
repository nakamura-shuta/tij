//! Status View key handling

use crossterm::event::{KeyCode, KeyEvent};

use super::{StatusAction, StatusInputMode, StatusView};
use crate::keys;

impl StatusView {
    /// Handle key event
    pub fn handle_key(&mut self, key: KeyEvent) -> StatusAction {
        self.handle_key_with_height(key, Self::DEFAULT_VISIBLE_COUNT)
    }

    /// Handle key event with explicit visible height
    pub fn handle_key_with_height(&mut self, key: KeyEvent, visible_count: usize) -> StatusAction {
        match self.input_mode {
            StatusInputMode::Normal => self.handle_normal_key(key, visible_count),
            StatusInputMode::CommitInput => self.handle_commit_input_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent, visible_count: usize) -> StatusAction {
        match key.code {
            code if keys::is_move_down(code) => {
                self.move_down(visible_count);
                StatusAction::None
            }
            code if keys::is_move_up(code) => {
                self.move_up(visible_count);
                StatusAction::None
            }
            code if code == keys::GO_TOP => {
                self.jump_to_top();
                StatusAction::None
            }
            code if code == keys::GO_BOTTOM => {
                self.jump_to_bottom(visible_count);
                StatusAction::None
            }
            code if code == keys::OPEN_DIFF => {
                if let (Some(change_id), Some(file_path)) =
                    (self.working_copy_id(), self.selected_file_path())
                {
                    StatusAction::ShowFileDiff {
                        change_id: change_id.to_string(),
                        file_path: file_path.to_string(),
                    }
                } else {
                    StatusAction::None
                }
            }
            code if code == keys::COMMIT => {
                // Only allow commit if there are changes
                if self.status.as_ref().is_some_and(|s| !s.is_clean()) {
                    self.start_commit_input();
                }
                StatusAction::None
            }
            code if code == keys::ANNOTATE => {
                if let Some(file_path) = self.selected_file_path() {
                    StatusAction::OpenBlame {
                        file_path: file_path.to_string(),
                    }
                } else {
                    StatusAction::None
                }
            }
            code if code == keys::JUMP_CONFLICT => {
                if self.jump_to_first_conflict() {
                    StatusAction::JumpToConflict
                } else {
                    StatusAction::None
                }
            }
            // Note: QUIT, TAB, ESC are handled by global key handler in input.rs
            _ => StatusAction::None,
        }
    }

    fn handle_commit_input_key(&mut self, key: KeyEvent) -> StatusAction {
        match key.code {
            KeyCode::Esc => {
                self.cancel_input();
                StatusAction::None
            }
            KeyCode::Enter => {
                let message = std::mem::take(&mut self.input_buffer);
                self.input_mode = StatusInputMode::Normal;
                if message.is_empty() {
                    // Empty message = cancel
                    StatusAction::None
                } else {
                    StatusAction::Commit { message }
                }
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                StatusAction::None
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                StatusAction::None
            }
            _ => StatusAction::None,
        }
    }
}
