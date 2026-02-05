//! Operation History View for displaying jj operation log

mod input;
mod render;

use crate::model::Operation;
use crate::ui::navigation;

/// Action returned by the Operation View after handling input
#[derive(Debug, Clone)]
pub enum OperationAction {
    /// No action needed
    None,
    /// Go back to previous view
    Back,
    /// Restore to selected operation (returns operation ID)
    Restore(String),
}

/// Operation History View state
#[derive(Debug)]
pub struct OperationView {
    /// List of operations from jj op log
    pub(super) operations: Vec<Operation>,
    /// Selected operation index
    pub(super) selected: usize,
    /// Scroll offset for long lists
    pub(super) scroll_offset: usize,
}

impl Default for OperationView {
    fn default() -> Self {
        Self::new()
    }
}

impl OperationView {
    /// Create a new Operation View
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Set the operations to display
    pub fn set_operations(&mut self, operations: Vec<Operation>) {
        self.operations = operations;
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Get the currently selected operation
    pub fn selected_operation(&self) -> Option<&Operation> {
        self.operations.get(self.selected)
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        self.selected = navigation::select_prev(self.selected);
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        let max = self.operations.len().saturating_sub(1);
        self.selected = navigation::select_next(self.selected, max);
    }

    /// Go to first operation
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Go to last operation
    pub fn select_last(&mut self) {
        if !self.operations.is_empty() {
            self.selected = self.operations.len() - 1;
        }
    }

    /// Get operation count for status display (test-only helper)
    #[cfg(test)]
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }

    /// Check if the view is empty (test-only helper)
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent};

    fn create_test_operations() -> Vec<Operation> {
        vec![
            Operation {
                id: "abc123def456".to_string(),
                user: "user@example.com".to_string(),
                timestamp: "5 minutes ago".to_string(),
                description: "snapshot working copy".to_string(),
                is_current: true,
            },
            Operation {
                id: "xyz789uvw012".to_string(),
                user: "user@example.com".to_string(),
                timestamp: "10 minutes ago".to_string(),
                description: "describe commit abc".to_string(),
                is_current: false,
            },
            Operation {
                id: "def456ghi789".to_string(),
                user: "user@example.com".to_string(),
                timestamp: "1 hour ago".to_string(),
                description: "new empty commit".to_string(),
                is_current: false,
            },
        ]
    }

    #[test]
    fn test_new_operation_view() {
        let view = OperationView::new();
        assert!(view.operations.is_empty());
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_set_operations() {
        let mut view = OperationView::new();
        let ops = create_test_operations();
        view.set_operations(ops);

        assert_eq!(view.operation_count(), 3);
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_navigation() {
        let mut view = OperationView::new();
        view.set_operations(create_test_operations());

        // Initially at first
        assert_eq!(view.selected, 0);

        // Move down
        view.select_next();
        assert_eq!(view.selected, 1);

        view.select_next();
        assert_eq!(view.selected, 2);

        // Can't go past end
        view.select_next();
        assert_eq!(view.selected, 2);

        // Move up
        view.select_prev();
        assert_eq!(view.selected, 1);

        // Go to first/last
        view.select_last();
        assert_eq!(view.selected, 2);

        view.select_first();
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_selected_operation() {
        let mut view = OperationView::new();
        view.set_operations(create_test_operations());

        let op = view.selected_operation().unwrap();
        assert_eq!(op.id, "abc123def456");
        assert!(op.is_current);
    }

    #[test]
    fn test_handle_key_enter() {
        let mut view = OperationView::new();
        view.set_operations(create_test_operations());

        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        match action {
            OperationAction::Restore(id) => assert_eq!(id, "abc123def456"),
            _ => panic!("Expected Restore action"),
        }
    }

    #[test]
    fn test_handle_key_back() {
        let mut view = OperationView::new();
        view.set_operations(create_test_operations());

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('q')));
        assert!(matches!(action, OperationAction::Back));
    }
}
