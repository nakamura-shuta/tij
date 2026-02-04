//! Operation History View for displaying jj operation log

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::keys;
use crate::model::{Notification, Operation};
use crate::ui::components;

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
    operations: Vec<Operation>,
    /// Selected operation index
    selected: usize,
    /// Scroll offset for long lists
    scroll_offset: usize,
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
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.selected < self.operations.len().saturating_sub(1) {
            self.selected += 1;
        }
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

    /// Handle key input
    pub fn handle_key(&mut self, key: KeyEvent) -> OperationAction {
        match key.code {
            // Navigation
            k if keys::is_move_down(k) => {
                self.select_next();
                OperationAction::None
            }
            k if keys::is_move_up(k) => {
                self.select_prev();
                OperationAction::None
            }
            k if k == keys::GO_TOP => {
                self.select_first();
                OperationAction::None
            }
            k if k == keys::GO_BOTTOM => {
                self.select_last();
                OperationAction::None
            }

            // Actions
            KeyCode::Enter => {
                if let Some(op) = self.selected_operation() {
                    OperationAction::Restore(op.id.clone())
                } else {
                    OperationAction::None
                }
            }

            // Back/Quit
            k if k == keys::QUIT => OperationAction::Back,
            KeyCode::Esc => OperationAction::Back,

            _ => OperationAction::None,
        }
    }

    /// Render the operation view with optional notification in title bar
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let title = Line::from(" Operation History ").bold().cyan().centered();

        // Build notification line for title bar
        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let block = components::bordered_block_with_notification(title, notif_line);

        if self.operations.is_empty() {
            let paragraph = Paragraph::new("No operations found").block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize; // borders
        if inner_height == 0 {
            return;
        }

        // Calculate scroll offset to keep selection visible
        let scroll_offset = self.calculate_scroll_offset(inner_height);

        // Build lines
        let mut lines: Vec<Line> = Vec::new();
        for (idx, op) in self.operations.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }

            let is_selected = idx == self.selected;
            let line = self.build_operation_line(op, is_selected);
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }

    /// Calculate scroll offset to keep selection visible
    fn calculate_scroll_offset(&self, visible_height: usize) -> usize {
        if visible_height == 0 {
            return 0;
        }

        let mut offset = self.scroll_offset;

        // Ensure selected item is visible
        if self.selected < offset {
            offset = self.selected;
        } else if self.selected >= offset + visible_height {
            offset = self.selected - visible_height + 1;
        }

        offset
    }

    /// Build a line for an operation
    fn build_operation_line(&self, op: &Operation, is_selected: bool) -> Line<'static> {
        let is_current = op.is_current;

        // Build the line with styled spans
        let marker = if is_current { "@" } else { " " };
        let marker_style = if is_current {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let id_style = Style::default().fg(Color::Magenta);
        let time_style = Style::default().fg(Color::Yellow);
        let desc_style = Style::default().fg(Color::White);

        let mut line = Line::from(vec![
            Span::styled(marker.to_string(), marker_style),
            Span::raw("  "),
            Span::styled(op.short_id().to_string(), id_style),
            Span::raw("  "),
            Span::styled(op.timestamp.clone(), time_style),
            Span::raw("  "),
            Span::styled(op.description.clone(), desc_style),
        ]);

        if is_selected {
            line = line.style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );
        }

        line
    }

    /// Get operation count for status display
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }

    /// Check if the view is empty
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
