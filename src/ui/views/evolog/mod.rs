//! Evolution Log View - displays jj evolog output for a change
//!
//! Shows the rewrite history of a change (describe, rebase, squash, etc.).

mod input;
mod render;

use crate::model::EvologEntry;
use crate::ui::navigation;

/// Action returned by the Evolog View after handling input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvologAction {
    /// No action needed
    None,
    /// Go back to previous view
    Back,
    /// Open diff for the selected commit_id
    OpenDiff(String),
}

/// Evolution Log View state
#[derive(Debug)]
pub struct EvologView {
    /// Target change ID (the change whose history we're viewing)
    pub change_id: String,
    /// Evolution entries (newest first)
    pub(super) entries: Vec<EvologEntry>,
    /// Selected entry index
    pub(super) selected: usize,
    /// Scroll offset for long lists
    pub(super) scroll_offset: usize,
}

impl EvologView {
    /// Create a new Evolog View
    pub fn new(change_id: String, entries: Vec<EvologEntry>) -> Self {
        Self {
            change_id,
            entries,
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Get the currently selected entry
    pub fn selected_entry(&self) -> Option<&EvologEntry> {
        self.entries.get(self.selected)
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        self.selected = navigation::select_prev(self.selected);
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        let max = self.entries.len().saturating_sub(1);
        self.selected = navigation::select_next(self.selected, max);
    }

    /// Go to first entry
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Go to last entry
    pub fn select_last(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entries() -> Vec<EvologEntry> {
        vec![
            EvologEntry {
                commit_id: "43a4bc7d".to_string(),
                change_id: "zxsrvopz".to_string(),
                author: "user@example.com".to_string(),
                timestamp: "2025-10-03 18:10:00".to_string(),
                is_empty: false,
                description: "my feature description".to_string(),
            },
            EvologEntry {
                commit_id: "7aa68914".to_string(),
                change_id: "zxsrvopz".to_string(),
                author: "user@example.com".to_string(),
                timestamp: "2025-10-03 18:08:05".to_string(),
                is_empty: false,
                description: "(no description set)".to_string(),
            },
            EvologEntry {
                commit_id: "initial1".to_string(),
                change_id: "zxsrvopz".to_string(),
                author: "user@example.com".to_string(),
                timestamp: "2025-10-03 18:05:00".to_string(),
                is_empty: true,
                description: "(no description set)".to_string(),
            },
        ]
    }

    #[test]
    fn test_new_evolog_view() {
        let entries = create_test_entries();
        let view = EvologView::new("zxsrvopz".to_string(), entries);
        assert_eq!(view.change_id, "zxsrvopz");
        assert_eq!(view.entries.len(), 3);
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_navigation() {
        let mut view = EvologView::new("zxsrvopz".to_string(), create_test_entries());

        assert_eq!(view.selected, 0);
        view.select_next();
        assert_eq!(view.selected, 1);
        view.select_next();
        assert_eq!(view.selected, 2);
        view.select_next();
        assert_eq!(view.selected, 2); // can't go past end

        view.select_prev();
        assert_eq!(view.selected, 1);

        view.select_last();
        assert_eq!(view.selected, 2);

        view.select_first();
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_selected_entry() {
        let view = EvologView::new("zxsrvopz".to_string(), create_test_entries());
        let entry = view.selected_entry().unwrap();
        assert_eq!(entry.commit_id, "43a4bc7d");
    }

    #[test]
    fn test_handle_key_enter() {
        use crossterm::event::{KeyCode, KeyEvent};
        let mut view = EvologView::new("zxsrvopz".to_string(), create_test_entries());
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(action, EvologAction::OpenDiff("43a4bc7d".to_string()));
    }

    #[test]
    fn test_handle_key_back() {
        use crossterm::event::{KeyCode, KeyEvent};
        let mut view = EvologView::new("zxsrvopz".to_string(), create_test_entries());
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('q')));
        assert_eq!(action, EvologAction::Back);
    }

    #[test]
    fn test_empty_entries() {
        let view = EvologView::new("test".to_string(), vec![]);
        assert!(view.selected_entry().is_none());
    }
}
