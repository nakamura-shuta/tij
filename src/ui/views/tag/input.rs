//! Tag View key handling

use crossterm::event::{KeyCode, KeyEvent};

use super::{TagAction, TagView};
use crate::keys;

impl TagView {
    /// Handle key input
    pub fn handle_key(&mut self, key: KeyEvent) -> TagAction {
        match key.code {
            k if keys::is_move_down(k) => {
                self.select_next();
                TagAction::None
            }
            k if keys::is_move_up(k) => {
                self.select_prev();
                TagAction::None
            }
            k if k == keys::GO_TOP => {
                self.select_first();
                TagAction::None
            }
            k if k == keys::GO_BOTTOM => {
                self.select_last();
                TagAction::None
            }
            KeyCode::Enter => {
                if let Some(tag) = self.selected_tag() {
                    if let Some(change_id) = &tag.change_id {
                        TagAction::Jump(change_id.clone())
                    } else {
                        TagAction::None
                    }
                } else {
                    TagAction::None
                }
            }
            KeyCode::Char('c') => TagAction::StartCreate,
            k if k == keys::BOOKMARK_DELETE => {
                // Reuse 'D' key for tag deletion
                if let Some(tag) = self.selected_tag() {
                    TagAction::Delete(tag.name.clone())
                } else {
                    TagAction::None
                }
            }
            _ => TagAction::None,
        }
    }
}
