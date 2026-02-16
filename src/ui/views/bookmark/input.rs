//! Bookmark View key handling

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{BookmarkAction, BookmarkView};
use crate::keys;

impl BookmarkView {
    /// Handle key input
    pub fn handle_key(&mut self, key: KeyEvent) -> BookmarkAction {
        // Rename input mode: intercept all keys
        if let Some(ref mut state) = self.rename_state {
            match key.code {
                KeyCode::Enter => {
                    let old = state.old_name.clone();
                    let new = state.input_buffer.clone();
                    self.rename_state = None;
                    return BookmarkAction::ConfirmRename {
                        old_name: old,
                        new_name: new,
                    };
                }
                KeyCode::Esc => {
                    self.rename_state = None;
                    return BookmarkAction::CancelRename;
                }
                KeyCode::Backspace => {
                    state.backspace();
                    return BookmarkAction::None;
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    state.insert_char(c);
                    return BookmarkAction::None;
                }
                _ => return BookmarkAction::None,
            }
        }

        // Normal mode
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
            k if k == keys::BOOKMARK_RENAME => {
                if self.rename_state.is_some() {
                    BookmarkAction::None
                } else if let Some(info) = self.selected_bookmark() {
                    if info.bookmark.remote.is_none() {
                        BookmarkAction::StartRename(info.bookmark.name.clone())
                    } else {
                        BookmarkAction::None
                    }
                } else {
                    BookmarkAction::None
                }
            }
            k if k == keys::BOOKMARK_FORGET => {
                if let Some(info) = self.selected_bookmark() {
                    if info.bookmark.remote.is_none() {
                        BookmarkAction::Forget(info.bookmark.name.clone())
                    } else {
                        BookmarkAction::None
                    }
                } else {
                    BookmarkAction::None
                }
            }
            k if k == keys::BOOKMARK_MOVE => {
                if let Some(info) = self.selected_bookmark() {
                    if info.bookmark.remote.is_none() {
                        BookmarkAction::Move(info.bookmark.name.clone())
                    } else {
                        BookmarkAction::MoveUnavailable
                    }
                } else {
                    BookmarkAction::None
                }
            }
            _ => BookmarkAction::None,
        }
    }
}
