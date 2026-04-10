//! Workspace View key handling

use crossterm::event::{KeyCode, KeyEvent};

use super::{WorkspaceAction, WorkspaceView};
use crate::keys;

impl WorkspaceView {
    /// Handle key input
    pub fn handle_key(&mut self, key: KeyEvent) -> WorkspaceAction {
        match key.code {
            k if keys::is_move_down(k) => {
                self.select_next();
                WorkspaceAction::None
            }
            k if keys::is_move_up(k) => {
                self.select_prev();
                WorkspaceAction::None
            }
            k if k == keys::GO_TOP => {
                self.select_first();
                WorkspaceAction::None
            }
            k if k == keys::GO_BOTTOM => {
                self.select_last();
                WorkspaceAction::None
            }
            KeyCode::Char('a') => WorkspaceAction::StartAdd,
            k if k == keys::BOOKMARK_DELETE => {
                // D key: forget workspace (blocked for current)
                if let Some(ws) = self.selected_workspace() {
                    if self.is_current(ws) {
                        WorkspaceAction::ForgetCurrentBlocked
                    } else {
                        WorkspaceAction::Forget(ws.name.clone())
                    }
                } else {
                    WorkspaceAction::None
                }
            }
            KeyCode::Char('r') => {
                // Rename: only allowed for current workspace
                if let Some(ws) = self.selected_workspace() {
                    if self.is_current(ws) {
                        WorkspaceAction::StartRename(ws.name.clone())
                    } else {
                        WorkspaceAction::RenameNonCurrentBlocked
                    }
                } else {
                    WorkspaceAction::None
                }
            }
            _ => WorkspaceAction::None,
        }
    }
}
