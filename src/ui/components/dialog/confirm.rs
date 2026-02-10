//! Confirm dialog input handling and rendering

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::{Dialog, DialogResult, centered_rect};

impl Dialog {
    pub(super) fn handle_confirm_key(&self, key: KeyEvent) -> Option<DialogResult> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                Some(DialogResult::Confirmed(vec![]))
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(DialogResult::Cancelled),
            _ => None,
        }
    }

    pub(super) fn render_confirm(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        message: &str,
        detail: Option<&str>,
    ) {
        // Split message by newlines for multi-line support (e.g., push dry-run preview)
        let message_lines: Vec<&str> = message.split('\n').collect();
        let extra_lines = message_lines.len().saturating_sub(1) as u16;

        // Calculate dialog size (dynamic height based on message lines)
        let width = 50.min(area.width.saturating_sub(4));
        let base_height: u16 = if detail.is_some() { 9 } else { 7 };
        let height = (base_height + extra_lines).min(area.height.saturating_sub(4));

        let dialog_area = centered_rect(width, height, area);

        // Clear the area behind the dialog
        frame.render_widget(Clear, dialog_area);

        // Build content
        let mut lines = vec![Line::from("")];

        // First line: bold (question text)
        if let Some(first) = message_lines.first() {
            lines.push(Line::from(Span::styled(
                *first,
                Style::default().add_modifier(Modifier::BOLD),
            )));
        }
        // Subsequent lines: cyan (preview info)
        for line_text in message_lines.iter().skip(1) {
            lines.push(Line::from(Span::styled(
                *line_text,
                Style::default().fg(Color::Cyan),
            )));
        }

        lines.push(Line::from(""));

        if let Some(detail_text) = detail {
            lines.push(Line::from(Span::styled(
                detail_text,
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::styled("[Y]", Style::default().fg(Color::Green)),
            Span::raw("es       "),
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
}
