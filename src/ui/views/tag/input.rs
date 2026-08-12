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
                        TagAction::Jump(change_id.to_string())
                    } else {
                        TagAction::None
                    }
                } else {
                    TagAction::None
                }
            }
            // Track / untrack / push are no-ops on rows they cannot act on
            // (same guard style as the Bookmark View).
            k if k == keys::TRACK => match self.selected_tag() {
                Some(t) if t.is_untracked_remote() => TagAction::Track(t.full_name()),
                _ => TagAction::None,
            },
            k if k == keys::BOOKMARK_UNTRACK => match self.selected_tag() {
                Some(t) if t.is_tracked_remote() => TagAction::Untrack(t.full_name()),
                _ => TagAction::None,
            },
            k if k == keys::PUSH => match self.selected_tag() {
                Some(t) if t.is_local() => TagAction::Push(t.name.clone()),
                _ => TagAction::None,
            },
            k if k == keys::TAG_FILTER => TagAction::CycleFilter,
            k if k == keys::OBJECT_NEW => TagAction::StartCreate,
            k if k == keys::OBJECT_DELETE => {
                // Local rows only. `jj tag delete` takes a bare name, so on a
                // remote row it would delete the *local* tag of the same name —
                // a destructive action on a different object than the one under
                // the cursor. Same guard as Bookmark View (`bookmark/input.rs`).
                match self.selected_tag() {
                    Some(tag) if tag.is_local() => TagAction::Delete(tag.name.clone()),
                    _ => TagAction::None,
                }
            }
            _ => TagAction::None,
        }
    }
}
