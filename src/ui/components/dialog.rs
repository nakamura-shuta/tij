//! Dialog components for confirmation and selection
//!
//! Provides reusable dialog components:
//! - Confirm dialog: Yes/No confirmation
//! - Select dialog: Checkbox selection for multiple items

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::keys;

/// Callback identifier for dialog results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogCallback {
    /// Bookmark deletion
    DeleteBookmarks,
    /// Operation restore (future use)
    OpRestore,
}

/// Selection item for Select dialog
#[derive(Debug, Clone)]
pub struct SelectItem {
    /// Display label
    pub label: String,
    /// Internal value (returned on confirm)
    pub value: String,
    /// Whether this item is selected
    pub selected: bool,
}

/// Dialog kind and content
#[derive(Debug, Clone)]
pub enum DialogKind {
    /// Simple Yes/No confirmation
    Confirm {
        title: String,
        message: String,
        /// Optional detail text (warning, etc.)
        detail: Option<String>,
    },
    /// Checkbox selection (multiple items)
    Select {
        title: String,
        message: String,
        items: Vec<SelectItem>,
        /// Optional detail text (warning, etc.)
        detail: Option<String>,
    },
}

/// Dialog result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogResult {
    /// Confirmed with selected values (empty for Confirm dialog)
    Confirmed(Vec<String>),
    /// Cancelled
    Cancelled,
}

/// Dialog state
#[derive(Debug, Clone)]
pub struct Dialog {
    /// Dialog kind and content
    pub kind: DialogKind,
    /// Cursor position (for Select dialog)
    pub cursor: usize,
    /// Callback identifier
    pub callback_id: DialogCallback,
}

impl Dialog {
    /// Create a new Confirm dialog
    pub fn confirm(
        title: impl Into<String>,
        message: impl Into<String>,
        detail: Option<String>,
        callback_id: DialogCallback,
    ) -> Self {
        Self {
            kind: DialogKind::Confirm {
                title: title.into(),
                message: message.into(),
                detail,
            },
            cursor: 0,
            callback_id,
        }
    }

    /// Create a new Select dialog
    pub fn select(
        title: impl Into<String>,
        message: impl Into<String>,
        items: Vec<SelectItem>,
        detail: Option<String>,
        callback_id: DialogCallback,
    ) -> Self {
        Self {
            kind: DialogKind::Select {
                title: title.into(),
                message: message.into(),
                items,
                detail,
            },
            cursor: 0,
            callback_id,
        }
    }

    /// Handle key input, returns Some(result) when dialog should close
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<DialogResult> {
        match &self.kind {
            DialogKind::Confirm { .. } => self.handle_confirm_key(key),
            DialogKind::Select { .. } => self.handle_select_key(key),
        }
    }

    fn handle_confirm_key(&self, key: KeyEvent) -> Option<DialogResult> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                Some(DialogResult::Confirmed(vec![]))
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(DialogResult::Cancelled),
            _ => None,
        }
    }

    fn handle_select_key(&mut self, key: KeyEvent) -> Option<DialogResult> {
        // Get items mutably from kind
        let items = match &mut self.kind {
            DialogKind::Select { items, .. } => items,
            _ => return None,
        };

        match key.code {
            // Navigation
            k if keys::is_move_down(k) => {
                if self.cursor < items.len().saturating_sub(1) {
                    self.cursor += 1;
                }
                None
            }
            k if keys::is_move_up(k) => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                None
            }
            // Toggle selection
            KeyCode::Char(' ') => {
                if let Some(item) = items.get_mut(self.cursor) {
                    item.selected = !item.selected;
                }
                None
            }
            // Confirm
            KeyCode::Enter => {
                let selected: Vec<String> = items
                    .iter()
                    .filter(|item| item.selected)
                    .map(|item| item.value.clone())
                    .collect();

                // If nothing selected, treat as cancel
                if selected.is_empty() {
                    Some(DialogResult::Cancelled)
                } else {
                    Some(DialogResult::Confirmed(selected))
                }
            }
            // Cancel
            KeyCode::Esc | KeyCode::Char('q') => Some(DialogResult::Cancelled),
            _ => None,
        }
    }

    /// Render the dialog centered on screen
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        match &self.kind {
            DialogKind::Confirm {
                title,
                message,
                detail,
            } => self.render_confirm(frame, area, title, message, detail.as_deref()),
            DialogKind::Select {
                title,
                message,
                items,
                detail,
            } => self.render_select(frame, area, title, message, items, detail.as_deref()),
        }
    }

    fn render_confirm(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        message: &str,
        detail: Option<&str>,
    ) {
        // Calculate dialog size
        let width = 50.min(area.width.saturating_sub(4));
        let height = if detail.is_some() { 9 } else { 7 };

        let dialog_area = centered_rect(width, height, area);

        // Clear the area behind the dialog
        frame.render_widget(Clear, dialog_area);

        // Build content
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                message,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if let Some(detail_text) = detail {
            lines.push(Line::from(Span::styled(
                detail_text,
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::raw("        "),
            Span::styled("[Y]", Style::default().fg(Color::Green)),
            Span::raw("es         "),
            Span::styled("[N]", Style::default().fg(Color::Red)),
            Span::raw("o"),
        ]));

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(format!(" {} ", title))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, dialog_area);
    }

    fn render_select(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        message: &str,
        items: &[SelectItem],
        detail: Option<&str>,
    ) {
        // Calculate dialog size (add 2 lines if detail is present)
        let width = 50.min(area.width.saturating_sub(4));
        let detail_lines = if detail.is_some() { 2 } else { 0 };
        let height = (items.len() as u16 + 6 + detail_lines).min(area.height.saturating_sub(4));

        let dialog_area = centered_rect(width, height, area);

        // Clear the area behind the dialog
        frame.render_widget(Clear, dialog_area);

        // Build content
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(message, Style::default())),
            Line::from(""),
        ];

        // Add items with checkboxes
        for (i, item) in items.iter().enumerate() {
            let checkbox = if item.selected { "[x]" } else { "[ ]" };
            let cursor = if i == self.cursor { "> " } else { "  " };

            let style = if i == self.cursor {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            lines.push(Line::from(Span::styled(
                format!("{}{} {}", cursor, checkbox, item.label),
                style,
            )));
        }

        lines.push(Line::from(""));

        // Add optional detail text
        if let Some(detail_text) = detail {
            lines.push(Line::from(Span::styled(
                detail_text,
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::styled("[j/k]", Style::default().fg(Color::Cyan)),
            Span::raw(" Move "),
            Span::styled("[Space]", Style::default().fg(Color::Green)),
            Span::raw(" Toggle "),
            Span::styled("[Enter]", Style::default().fg(Color::Green)),
            Span::raw(" OK "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" Cancel"),
        ]));

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(paragraph, dialog_area);
    }
}

/// Calculate a centered rectangle within the given area
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical_margin = area.height.saturating_sub(height) / 2;
    let horizontal_margin = area.width.saturating_sub(width) / 2;

    let vertical_layout = Layout::vertical([
        Constraint::Length(vertical_margin),
        Constraint::Length(height),
        Constraint::Length(vertical_margin),
    ])
    .split(area);

    let horizontal_layout = Layout::horizontal([
        Constraint::Length(horizontal_margin),
        Constraint::Length(width),
        Constraint::Length(horizontal_margin),
    ])
    .split(vertical_layout[1]);

    horizontal_layout[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_confirm_dialog_yes() {
        let dialog = Dialog::confirm(
            "Test",
            "Are you sure?",
            None,
            DialogCallback::DeleteBookmarks,
        );

        let mut d = dialog.clone();
        assert_eq!(
            d.handle_key(key(KeyCode::Char('y'))),
            Some(DialogResult::Confirmed(vec![]))
        );

        let mut d = dialog.clone();
        assert_eq!(
            d.handle_key(key(KeyCode::Char('Y'))),
            Some(DialogResult::Confirmed(vec![]))
        );

        let mut d = dialog.clone();
        assert_eq!(
            d.handle_key(key(KeyCode::Enter)),
            Some(DialogResult::Confirmed(vec![]))
        );
    }

    #[test]
    fn test_confirm_dialog_no() {
        let dialog = Dialog::confirm(
            "Test",
            "Are you sure?",
            None,
            DialogCallback::DeleteBookmarks,
        );

        let mut d = dialog.clone();
        assert_eq!(
            d.handle_key(key(KeyCode::Char('n'))),
            Some(DialogResult::Cancelled)
        );

        let mut d = dialog.clone();
        assert_eq!(
            d.handle_key(key(KeyCode::Char('N'))),
            Some(DialogResult::Cancelled)
        );

        let mut d = dialog.clone();
        assert_eq!(
            d.handle_key(key(KeyCode::Esc)),
            Some(DialogResult::Cancelled)
        );
    }

    #[test]
    fn test_select_dialog_toggle() {
        let items = vec![
            SelectItem {
                label: "Item 1".to_string(),
                value: "1".to_string(),
                selected: false,
            },
            SelectItem {
                label: "Item 2".to_string(),
                value: "2".to_string(),
                selected: false,
            },
        ];

        let mut dialog = Dialog::select(
            "Test",
            "Select items",
            items,
            None,
            DialogCallback::DeleteBookmarks,
        );

        // Toggle first item
        assert!(dialog.handle_key(key(KeyCode::Char(' '))).is_none());
        if let DialogKind::Select { items, .. } = &dialog.kind {
            assert!(items[0].selected);
            assert!(!items[1].selected);
        }

        // Move down and toggle
        dialog.handle_key(key(KeyCode::Char('j')));
        dialog.handle_key(key(KeyCode::Char(' ')));
        if let DialogKind::Select { items, .. } = &dialog.kind {
            assert!(items[0].selected);
            assert!(items[1].selected);
        }
    }

    #[test]
    fn test_select_dialog_confirm() {
        let items = vec![
            SelectItem {
                label: "Item 1".to_string(),
                value: "value1".to_string(),
                selected: true,
            },
            SelectItem {
                label: "Item 2".to_string(),
                value: "value2".to_string(),
                selected: false,
            },
            SelectItem {
                label: "Item 3".to_string(),
                value: "value3".to_string(),
                selected: true,
            },
        ];

        let mut dialog = Dialog::select(
            "Test",
            "Select items",
            items,
            None,
            DialogCallback::DeleteBookmarks,
        );

        let result = dialog.handle_key(key(KeyCode::Enter));
        assert_eq!(
            result,
            Some(DialogResult::Confirmed(vec![
                "value1".to_string(),
                "value3".to_string()
            ]))
        );
    }

    #[test]
    fn test_select_dialog_empty_confirm_is_cancelled() {
        let items = vec![
            SelectItem {
                label: "Item 1".to_string(),
                value: "1".to_string(),
                selected: false,
            },
            SelectItem {
                label: "Item 2".to_string(),
                value: "2".to_string(),
                selected: false,
            },
        ];

        let mut dialog = Dialog::select(
            "Test",
            "Select items",
            items,
            None,
            DialogCallback::DeleteBookmarks,
        );

        // Confirm with nothing selected should cancel
        let result = dialog.handle_key(key(KeyCode::Enter));
        assert_eq!(result, Some(DialogResult::Cancelled));
    }

    #[test]
    fn test_select_dialog_cancel() {
        let items = vec![SelectItem {
            label: "Item 1".to_string(),
            value: "1".to_string(),
            selected: true,
        }];

        let mut dialog = Dialog::select(
            "Test",
            "Select items",
            items,
            None,
            DialogCallback::DeleteBookmarks,
        );

        assert_eq!(
            dialog.handle_key(key(KeyCode::Esc)),
            Some(DialogResult::Cancelled)
        );

        let mut dialog2 = Dialog::select(
            "Test",
            "Select items",
            vec![],
            None,
            DialogCallback::DeleteBookmarks,
        );
        assert_eq!(
            dialog2.handle_key(key(KeyCode::Char('q'))),
            Some(DialogResult::Cancelled)
        );
    }
}
