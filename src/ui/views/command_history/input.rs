//! Command History View key handling

use crossterm::event::{KeyCode, KeyEvent};

use super::{CommandHistoryAction, CommandHistoryView};
use crate::keys;

impl CommandHistoryView {
    /// Handle key input
    ///
    /// `total` is the number of records in the command history.
    pub fn handle_key(&mut self, key: KeyEvent, total: usize) -> CommandHistoryAction {
        match key.code {
            k if keys::is_move_down(k) => {
                self.select_next(total);
                CommandHistoryAction::None
            }
            k if keys::is_move_up(k) => {
                self.select_prev();
                CommandHistoryAction::None
            }
            k if k == keys::GO_TOP => {
                self.select_first();
                CommandHistoryAction::None
            }
            k if k == keys::GO_BOTTOM => {
                self.select_last(total);
                CommandHistoryAction::None
            }
            KeyCode::Enter => {
                if total > 0 {
                    self.toggle_detail();
                    CommandHistoryAction::ToggleDetail(self.selected)
                } else {
                    CommandHistoryAction::None
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => CommandHistoryAction::Back,
            _ => CommandHistoryAction::None,
        }
    }
}
