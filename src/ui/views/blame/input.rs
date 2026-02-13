//! Input handling for BlameView

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::keys;

use super::{BlameAction, BlameView};

/// Check if key is Shift+J (Jump to Log)
/// Some terminals send Char('J'), others send Char('j') + SHIFT modifier
fn is_jump_to_log_key(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('J'))
        || (matches!(key.code, KeyCode::Char('j')) && key.modifiers.contains(KeyModifiers::SHIFT))
}

impl BlameView {
    /// Handle key event and return action
    pub fn handle_key(&mut self, key: KeyEvent) -> BlameAction {
        // Check Shift+J first (before key.code match, since it needs full KeyEvent)
        if is_jump_to_log_key(&key) {
            return if let Some(change_id) = self.selected_change_id() {
                BlameAction::JumpToLog(change_id.to_string())
            } else {
                BlameAction::None
            };
        }

        match key.code {
            // Navigation
            k if keys::is_move_down(k) => {
                self.move_down();
                BlameAction::None
            }
            k if keys::is_move_up(k) => {
                self.move_up();
                BlameAction::None
            }
            k if k == keys::GO_TOP => {
                self.move_to_top();
                BlameAction::None
            }
            k if k == keys::GO_BOTTOM => {
                self.move_to_bottom();
                BlameAction::None
            }
            // Open diff for selected change
            KeyCode::Enter => {
                if let Some(change_id) = self.selected_change_id() {
                    BlameAction::OpenDiff(change_id.to_string())
                } else {
                    BlameAction::None
                }
            }
            // Back
            k if k == keys::QUIT || k == keys::ESC => BlameAction::Back,
            _ => BlameAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AnnotationContent, AnnotationLine};
    use crossterm::event::KeyModifiers;

    fn make_test_content() -> AnnotationContent {
        let mut content = AnnotationContent::new("test.rs".to_string());
        for i in 1..=3 {
            content.lines.push(AnnotationLine {
                change_id: format!("change{:02}", i),
                author: "test".to_string(),
                timestamp: "2026-01-30 10:00".to_string(),
                line_number: i,
                content: format!("line {}", i),
                first_in_hunk: true,
            });
        }
        content
    }

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_handle_key_navigation() {
        let mut view = BlameView::new();
        view.set_content(make_test_content(), None);

        // Move down
        let action = view.handle_key(key_event(KeyCode::Char('j')));
        assert_eq!(action, BlameAction::None);
        assert_eq!(view.selected_index, 1);

        // Move up
        let action = view.handle_key(key_event(KeyCode::Char('k')));
        assert_eq!(action, BlameAction::None);
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_handle_key_enter() {
        let mut view = BlameView::new();
        view.set_content(make_test_content(), None);

        let action = view.handle_key(key_event(KeyCode::Enter));
        assert_eq!(action, BlameAction::OpenDiff("change01".to_string()));
    }

    #[test]
    fn test_handle_key_back() {
        let mut view = BlameView::new();

        let action = view.handle_key(key_event(KeyCode::Char('q')));
        assert_eq!(action, BlameAction::Back);

        let action = view.handle_key(key_event(KeyCode::Esc));
        assert_eq!(action, BlameAction::Back);
    }

    #[test]
    fn test_handle_key_jump_to_log() {
        let mut view = BlameView::new();
        view.set_content(make_test_content(), None);

        // Char('J') — standard uppercase
        let action = view.handle_key(key_event(KeyCode::Char('J')));
        assert_eq!(action, BlameAction::JumpToLog("change01".to_string()));

        // Move to second line and jump
        view.move_down();
        let action = view.handle_key(key_event(KeyCode::Char('J')));
        assert_eq!(action, BlameAction::JumpToLog("change02".to_string()));
    }

    #[test]
    fn test_handle_key_jump_to_log_shift_j() {
        // Some terminals send Char('j') + SHIFT instead of Char('J')
        let mut view = BlameView::new();
        view.set_content(make_test_content(), None);

        let shift_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::SHIFT);
        let action = view.handle_key(shift_j);
        assert_eq!(action, BlameAction::JumpToLog("change01".to_string()));
    }

    #[test]
    fn test_handle_key_jump_to_log_empty() {
        let mut view = BlameView::new();

        let action = view.handle_key(key_event(KeyCode::Char('J')));
        assert_eq!(action, BlameAction::None);
    }
}
