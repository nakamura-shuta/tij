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
        // Get items and single_select flag from kind
        let (items, single_select) = match &mut self.kind {
            DialogKind::Select {
                items,
                single_select,
                ..
            } => (items, *single_select),
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
            // Toggle selection (multi-select only)
            KeyCode::Char(' ') => {
                if !single_select {
                    if let Some(item) = items.get_mut(self.cursor) {
                        item.selected = !item.selected;
                    }
                }
                None
            }
            // Confirm
            KeyCode::Enter => {
                if single_select {
                    // Single select: return current cursor item
                    if let Some(item) = items.get(self.cursor) {
                        Some(DialogResult::Confirmed(vec![item.value.clone()]))
                    } else {
                        Some(DialogResult::Cancelled)
                    }
                } else {
                    // Multi select: return all checked items
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
            }
            // Cancel
            KeyCode::Esc | KeyCode::Char('q') => Some(DialogResult::Cancelled),
            _ => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_select(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        message: &str,
        items: &[SelectItem],
        detail: Option<&str>,
        single_select: bool,
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

        // Add items (with or without checkboxes)
        for (i, item) in items.iter().enumerate() {
            let cursor = if i == self.cursor { "> " } else { "  " };

            let style = if i == self.cursor {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let item_text = if single_select {
                // Single select: no checkbox
                format!("{}{}", cursor, item.label)
            } else {
                // Multi select: with checkbox
                let checkbox = if item.selected { "[x]" } else { "[ ]" };
                format!("{}{} {}", cursor, checkbox, item.label)
            };

            lines.push(Line::from(Span::styled(item_text, style)));
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

        // Hint line: different for single_select vs multi_select
        let hints = if single_select {
            vec![
                Span::styled("[j/k]", Style::default().fg(Color::Cyan)),
                Span::raw(" Move "),
                Span::styled("[Enter]", Style::default().fg(Color::Green)),
                Span::raw(" Select "),
                Span::styled("[Esc]", Style::default().fg(Color::Red)),
                Span::raw(" Cancel"),
            ]
        } else {
            vec![
                Span::styled("[j/k]", Style::default().fg(Color::Cyan)),
                Span::raw(" Move "),
                Span::styled("[Space]", Style::default().fg(Color::Green)),
                Span::raw(" Toggle "),
                Span::styled("[Enter]", Style::default().fg(Color::Green)),
                Span::raw(" OK "),
                Span::styled("[Esc]", Style::default().fg(Color::Red)),
                Span::raw(" Cancel"),
            ]
        };

        lines.push(Line::from(hints));

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(paragraph, dialog_area);
    }
}
