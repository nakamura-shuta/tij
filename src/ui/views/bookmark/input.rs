//! Bookmark View key handling

use crossterm::event::{KeyCode, KeyEvent};

use super::{BookmarkAction, BookmarkView};
use crate::keys;

impl BookmarkView {
    /// Handle key input
    pub fn handle_key(&mut self, key: KeyEvent) -> BookmarkAction {
        match key.code {
            k if keys::is_move_down(k) => {
                self.select_next();
                BookmarkAction::None
            }
            k if keys::is_move_up(k) => {
                self.select_prev();
                BookmarkAction::None
            }
            k if k == keys::GO_TOP => {
                self.select_first();
                BookmarkAction::None
            }
            k if k == keys::GO_BOTTOM => {
                self.select_last();
                BookmarkAction::None
            }
            KeyCode::Enter => {
                if let Some(info) = self.selected_bookmark() {
                    if let Some(change_id) = &info.change_id {
                        BookmarkAction::Jump(change_id.clone())
                    } else {
                        BookmarkAction::None
                    }
                } else {
                    BookmarkAction::None
                }
            }
            k if k == keys::TRACK => {
                if let Some(info) = self.selected_bookmark() {
                    if info.bookmark.is_untracked_remote() {
                        BookmarkAction::Track(info.bookmark.full_name())
                    } else {
                        BookmarkAction::None
                    }
                } else {
                    BookmarkAction::None
                }
            }
            k if k == keys::BOOKMARK_UNTRACK => {
                if let Some(info) = self.selected_bookmark() {
                    if info.bookmark.remote.is_some() && info.bookmark.is_tracked {
                        BookmarkAction::Untrack(info.bookmark.full_name())
                    } else {
                        BookmarkAction::None
                    }
                } else {
                    BookmarkAction::None
                }
            }
            k if k == keys::BOOKMARK_DELETE => {
                if let Some(info) = self.selected_bookmark() {
                    if info.bookmark.remote.is_none() {
                        BookmarkAction::Delete(info.bookmark.name.clone())
                    } else {
                        BookmarkAction::None
                    }
                } else {
                    BookmarkAction::None
                }
            }
            _ => BookmarkAction::None,
        }
    }
}
