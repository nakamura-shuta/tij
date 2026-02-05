//! Input handling for ResolveView

use crossterm::event::{KeyCode, KeyEvent};

use crate::keys;

use super::{ResolveAction, ResolveView};

impl ResolveView {
    /// Handle key event and return action
    pub fn handle_key(&mut self, key: KeyEvent) -> ResolveAction {
        match key.code {
            // Navigation
            k if keys::is_move_down(k) => {
                self.move_down();
                ResolveAction::None
            }
            k if keys::is_move_up(k) => {
                self.move_up();
                ResolveAction::None
            }
            k if k == keys::GO_TOP => {
                self.move_to_top();
                ResolveAction::None
            }
            k if k == keys::GO_BOTTOM => {
                self.move_to_bottom();
                ResolveAction::None
            }
            // Resolve with external merge tool (@ only)
            KeyCode::Enter => {
                if self.is_working_copy {
                    if let Some(path) = self.selected_file_path() {
                        ResolveAction::ResolveExternal(path.to_string())
                    } else {
                        ResolveAction::None
                    }
                } else {
                    // Non-@ change: Enter is disabled
                    ResolveAction::None
                }
            }
            // Resolve with :ours
            KeyCode::Char('o') => {
                if let Some(path) = self.selected_file_path() {
                    ResolveAction::ResolveOurs(path.to_string())
                } else {
                    ResolveAction::None
                }
            }
            // Resolve with :theirs
            KeyCode::Char('t') => {
                if let Some(path) = self.selected_file_path() {
                    ResolveAction::ResolveTheirs(path.to_string())
                } else {
                    ResolveAction::None
                }
            }
            // Show diff for selected file
            KeyCode::Char('d') => {
                if let Some(path) = self.selected_file_path() {
                    ResolveAction::ShowDiff(path.to_string())
                } else {
                    ResolveAction::None
                }
            }
            // Back
            k if k == keys::QUIT || k == keys::ESC => ResolveAction::Back,
            _ => ResolveAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ConflictFile;
    use crossterm::event::KeyModifiers;

    fn make_test_files() -> Vec<ConflictFile> {
        vec![
            ConflictFile {
                path: "test.txt".to_string(),
                description: "2-sided conflict".to_string(),
            },
            ConflictFile {
                path: "src/main.rs".to_string(),
                description: "2-sided conflict".to_string(),
            },
        ]
    }

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_handle_key_navigation() {
        let mut view = ResolveView::new("abc".to_string(), true, make_test_files());

        let action = view.handle_key(key_event(KeyCode::Char('j')));
        assert_eq!(action, ResolveAction::None);
        assert_eq!(view.selected_file_path(), Some("src/main.rs"));

        let action = view.handle_key(key_event(KeyCode::Char('k')));
        assert_eq!(action, ResolveAction::None);
        assert_eq!(view.selected_file_path(), Some("test.txt"));
    }

    #[test]
    fn test_handle_key_enter_working_copy() {
        let mut view = ResolveView::new("abc".to_string(), true, make_test_files());

        let action = view.handle_key(key_event(KeyCode::Enter));
        assert_eq!(
            action,
            ResolveAction::ResolveExternal("test.txt".to_string())
        );
    }

    #[test]
    fn test_handle_key_enter_non_working_copy() {
        let mut view = ResolveView::new("abc".to_string(), false, make_test_files());

        // Enter is disabled for non-@ changes
        let action = view.handle_key(key_event(KeyCode::Enter));
        assert_eq!(action, ResolveAction::None);
    }

    #[test]
    fn test_handle_key_ours() {
        let mut view = ResolveView::new("abc".to_string(), true, make_test_files());

        let action = view.handle_key(key_event(KeyCode::Char('o')));
        assert_eq!(action, ResolveAction::ResolveOurs("test.txt".to_string()));
    }

    #[test]
    fn test_handle_key_theirs() {
        let mut view = ResolveView::new("abc".to_string(), true, make_test_files());

        let action = view.handle_key(key_event(KeyCode::Char('t')));
        assert_eq!(action, ResolveAction::ResolveTheirs("test.txt".to_string()));
    }

    #[test]
    fn test_handle_key_diff() {
        let mut view = ResolveView::new("abc".to_string(), true, make_test_files());

        let action = view.handle_key(key_event(KeyCode::Char('d')));
        assert_eq!(action, ResolveAction::ShowDiff("test.txt".to_string()));
    }

    #[test]
    fn test_handle_key_back() {
        let mut view = ResolveView::new("abc".to_string(), true, make_test_files());

        let action = view.handle_key(key_event(KeyCode::Char('q')));
        assert_eq!(action, ResolveAction::Back);

        let action = view.handle_key(key_event(KeyCode::Esc));
        assert_eq!(action, ResolveAction::Back);
    }

    #[test]
    fn test_ours_theirs_work_for_non_working_copy() {
        let mut view = ResolveView::new("abc".to_string(), false, make_test_files());

        // :ours and :theirs work for any change
        let action = view.handle_key(key_event(KeyCode::Char('o')));
        assert_eq!(action, ResolveAction::ResolveOurs("test.txt".to_string()));

        let action = view.handle_key(key_event(KeyCode::Char('t')));
        assert_eq!(action, ResolveAction::ResolveTheirs("test.txt".to_string()));
    }
}
