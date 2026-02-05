//! Select dialog input handling and rendering

use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::{Dialog, DialogKind, DialogResult, SelectItem, centered_rect, keys};

impl Dialog {
    pub(super) fn handle_select_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> Option<DialogResult> {
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

    pub(super) fn render_select(
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
