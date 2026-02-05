//! Operation View key handling

use crossterm::event::{KeyCode, KeyEvent};

use super::{OperationAction, OperationView};
use crate::keys;

impl OperationView {
    /// Handle key input
    pub fn handle_key(&mut self, key: KeyEvent) -> OperationAction {
        match key.code {
            // Navigation
            k if keys::is_move_down(k) => {
                self.select_next();
                OperationAction::None
            }
            k if keys::is_move_up(k) => {
                self.select_prev();
                OperationAction::None
            }
            k if k == keys::GO_TOP => {
                self.select_first();
                OperationAction::None
            }
            k if k == keys::GO_BOTTOM => {
                self.select_last();
                OperationAction::None
            }

            // Actions
            KeyCode::Enter => {
                if let Some(op) = self.selected_operation() {
                    OperationAction::Restore(op.id.clone())
                } else {
                    OperationAction::None
                }
            }

            // Back/Quit
            k if k == keys::QUIT => OperationAction::Back,
            KeyCode::Esc => OperationAction::Back,

            _ => OperationAction::None,
        }
    }
}
