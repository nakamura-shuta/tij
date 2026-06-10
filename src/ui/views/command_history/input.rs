//! Command History View key handling

use crossterm::event::{KeyCode, KeyEvent};

use super::{CommandHistoryAction, CommandHistoryView};
use crate::keys;
use crate::model::CommandHistory;

impl CommandHistoryView {
    /// Handle key input.
    ///
    /// Takes the history itself (not a raw total): the view re-syncs its
    /// filtered `visible` set first, so selection / Enter / `y` always act on
    /// the rows currently displayed.
    pub fn handle_key(&mut self, key: KeyEvent, history: &CommandHistory) -> CommandHistoryAction {
        self.sync(history);
        let total = self.visible_len();
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
            KeyCode::Char('f') => {
                self.cycle_filter();
                self.sync(history);
                CommandHistoryAction::None
            }
            KeyCode::Char('y') => {
                if let Some(raw) = self.raw_index(self.selected)
                    && let Some(record) = history.records().get(raw)
                {
                    CommandHistoryAction::CopyCommand(record.shell_command_line())
                } else {
                    CommandHistoryAction::None
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => CommandHistoryAction::Back,
            _ => CommandHistoryAction::None,
        }
    }
}
