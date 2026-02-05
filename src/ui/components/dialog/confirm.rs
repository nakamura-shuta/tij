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
