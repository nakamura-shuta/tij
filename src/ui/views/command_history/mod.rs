//! Command History View for displaying executed jj commands

mod input;
mod render;

use crate::ui::navigation;

/// Action returned by the Command History View after handling input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandHistoryAction {
    /// No action needed
    None,
    /// Go back to previous view
    Back,
    /// Toggle detail expansion for the selected record
    ToggleDetail(usize),
}

/// Command History View state
#[derive(Debug)]
pub struct CommandHistoryView {
    /// Selected index
    selected: usize,
    /// Scroll offset
    scroll_offset: usize,
    /// Index of expanded detail (None if no detail open)
    expanded_index: Option<usize>,
}

impl Default for CommandHistoryView {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHistoryView {
    /// Create a new Command History View
    pub fn new() -> Self {
        Self {
            selected: 0,
            scroll_offset: 0,
            expanded_index: None,
        }
    }

    /// Move selection to next record
    pub fn select_next(&mut self, total: usize) {
        let max = total.saturating_sub(1);
        self.selected = navigation::select_next(self.selected, max);
        self.expanded_index = None;
    }

    /// Move selection to previous record
    pub fn select_prev(&mut self) {
        self.selected = navigation::select_prev(self.selected);
        self.expanded_index = None;
    }

    /// Go to first record
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
        self.expanded_index = None;
    }

    /// Go to last record
    pub fn select_last(&mut self, total: usize) {
        if total > 0 {
            self.selected = total - 1;
        }
        self.expanded_index = None;
    }

    /// Toggle detail for the currently selected record
    pub fn toggle_detail(&mut self) {
        if self.expanded_index == Some(self.selected) {
            self.expanded_index = None;
        } else {
            self.expanded_index = Some(self.selected);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent};

    #[test]
    fn test_new_command_history_view() {
        let view = CommandHistoryView::new();
        assert_eq!(view.selected, 0);
        assert_eq!(view.scroll_offset, 0);
        assert!(view.expanded_index.is_none());
    }

    #[test]
    fn test_navigation() {
        let mut view = CommandHistoryView::new();
        assert_eq!(view.selected, 0);

        view.select_next(5);
        assert_eq!(view.selected, 1);

        view.select_next(5);
        assert_eq!(view.selected, 2);

        view.select_prev();
        assert_eq!(view.selected, 1);
    }

    #[test]
    fn test_select_first_last() {
        let mut view = CommandHistoryView::new();
        view.select_next(5);
        view.select_next(5);

        view.select_first();
        assert_eq!(view.selected, 0);

        view.select_last(5);
        assert_eq!(view.selected, 4);
    }

    #[test]
    fn test_toggle_detail() {
        let mut view = CommandHistoryView::new();
        assert!(view.expanded_index.is_none());

        view.toggle_detail();
        assert_eq!(view.expanded_index, Some(0));

        view.toggle_detail();
        assert!(view.expanded_index.is_none());
    }

    #[test]
    fn test_move_closes_detail() {
        let mut view = CommandHistoryView::new();
        view.toggle_detail();
        assert_eq!(view.expanded_index, Some(0));

        view.select_next(5);
        assert!(view.expanded_index.is_none());
    }

    #[test]
    fn test_empty_history_no_panic() {
        let mut view = CommandHistoryView::new();
        // All navigation on empty should not panic
        view.select_next(0);
        view.select_prev();
        view.select_first();
        view.select_last(0);
        view.toggle_detail();
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_handle_key_back_q() {
        let mut view = CommandHistoryView::new();
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('q')), 0);
        assert_eq!(action, CommandHistoryAction::Back);
    }

    #[test]
    fn test_handle_key_back_esc() {
        let mut view = CommandHistoryView::new();
        let action = view.handle_key(KeyEvent::from(KeyCode::Esc), 0);
        assert_eq!(action, CommandHistoryAction::Back);
    }

    #[test]
    fn test_handle_key_enter_toggle() {
        let mut view = CommandHistoryView::new();
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter), 3);
        assert_eq!(action, CommandHistoryAction::ToggleDetail(0));
        assert_eq!(view.expanded_index, Some(0));
    }

    #[test]
    fn test_handle_key_enter_empty() {
        let mut view = CommandHistoryView::new();
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter), 0);
        assert_eq!(action, CommandHistoryAction::None);
    }

    #[test]
    fn test_handle_key_navigation_j_k() {
        let mut view = CommandHistoryView::new();
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('j')), 5);
        assert_eq!(action, CommandHistoryAction::None);
        assert_eq!(view.selected, 1);

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('k')), 5);
        assert_eq!(action, CommandHistoryAction::None);
        assert_eq!(view.selected, 0);
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_handle_key_g_G() {
        let mut view = CommandHistoryView::new();
        view.handle_key(KeyEvent::from(KeyCode::Char('G')), 5);
        assert_eq!(view.selected, 4);

        view.handle_key(KeyEvent::from(KeyCode::Char('g')), 5);
        assert_eq!(view.selected, 0);
    }
}
