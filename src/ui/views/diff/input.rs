//! Key handling for DiffView

use crossterm::event::KeyEvent;

use crate::keys;

use super::{DiffAction, DiffView};

impl DiffView {
    /// Handle key input
    #[cfg(test)]
    pub fn handle_key(&mut self, key: KeyEvent) -> DiffAction {
        self.handle_key_with_height(key, Self::DEFAULT_VISIBLE_HEIGHT)
    }

    /// Handle key input with explicit visible height
    pub fn handle_key_with_height(&mut self, key: KeyEvent, visible_height: usize) -> DiffAction {
        // Always update visible_height to ensure accurate scroll bounds
        self.visible_height = visible_height;

        match key.code {
            code if keys::is_move_down(code) => {
                self.scroll_down();
                DiffAction::None
            }
            code if keys::is_move_up(code) => {
                self.scroll_up();
                DiffAction::None
            }
            keys::HALF_PAGE_DOWN => {
                self.scroll_half_page_down(visible_height);
                DiffAction::None
            }
            keys::HALF_PAGE_UP => {
                self.scroll_half_page_up(visible_height);
                DiffAction::None
            }
            keys::GO_TOP => {
                self.jump_to_top();
                DiffAction::None
            }
            keys::GO_BOTTOM => {
                self.jump_to_bottom(visible_height);
                DiffAction::None
            }
            keys::NEXT_FILE => {
                self.next_file();
                DiffAction::None
            }
            keys::PREV_FILE => {
                self.prev_file();
                DiffAction::None
            }
            keys::ANNOTATE => {
                // Blame is not available in compare mode (no single revision context)
                if self.compare_info.is_some() {
                    DiffAction::ShowNotification(
                        "Blame is not available in compare mode".to_string(),
                    )
                } else if let Some(file_name) = self.current_file_name() {
                    DiffAction::OpenBlame {
                        file_path: file_name.to_string(),
                    }
                } else {
                    DiffAction::None
                }
            }
            keys::DIFF_FORMAT_CYCLE => DiffAction::CycleFormat,
            keys::YANK => DiffAction::CopyToClipboard { full: true },
            keys::YANK_DIFF => DiffAction::CopyToClipboard { full: false },
            keys::WRITE_FILE => DiffAction::ExportToFile,
            keys::QUIT | keys::ESC => DiffAction::Back,
            _ => DiffAction::None,
        }
    }
}
