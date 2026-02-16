//! Evolog View key handling

use crossterm::event::{KeyCode, KeyEvent};

use super::{EvologAction, EvologView};
use crate::keys;

impl EvologView {
    /// Handle key input
    pub fn handle_key(&mut self, key: KeyEvent) -> EvologAction {
        match key.code {
            // Navigation
            k if keys::is_move_down(k) => {
                self.select_next();
                EvologAction::None
            }
            k if keys::is_move_up(k) => {
                self.select_prev();
                EvologAction::None
            }
            k if k == keys::GO_TOP => {
                self.select_first();
                EvologAction::None
            }
            k if k == keys::GO_BOTTOM => {
                self.select_last();
                EvologAction::None
            }

            // Actions
            KeyCode::Enter => {
                if let Some(entry) = self.selected_entry() {
                    EvologAction::OpenDiff(entry.commit_id.clone())
                } else {
                    EvologAction::None
                }
            }

            // Back/Quit
            k if k == keys::QUIT => EvologAction::Back,
            KeyCode::Esc => EvologAction::Back,

            _ => EvologAction::None,
        }
    }
}
