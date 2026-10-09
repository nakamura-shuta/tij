//! Confirm dialog input handling and rendering

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::super::message::wrap_display_width;
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
        let width = 50.min(area.width.saturating_sub(4));

        // Wrap to the inner width so long messages and jj's own error text
        // (dialog details) stay readable instead of being cut at the border.
        // Same wrapping as the error banner: word boundaries, with a
        // per-character fallback for CJK and over-long words.
        let inner_width = width.saturating_sub(4).max(1) as usize;
        let wrap = |text: &str| -> Vec<String> {
            text.split('\n')
                .flat_map(|line| wrap_display_width(line, inner_width))
                .collect()
        };
        let message_lines = wrap(message);
        let detail_lines: Vec<String> = detail.map(wrap).unwrap_or_default();

        // blank + message + blank + [detail + blank] + [Y]/[N], plus borders
        // and the one spare row the original layout kept.
        let content_rows = 1
            + message_lines.len()
            + 1
            + if detail_lines.is_empty() {
                0
            } else {
                detail_lines.len() + 1
            }
            + 1;
        let height = (content_rows as u16 + 3).min(area.height.saturating_sub(4));

        let dialog_area = centered_rect(width, height, area);

        // Clear the area behind the dialog
        frame.render_widget(Clear, dialog_area);

        // Build content
        let mut lines = vec![Line::from("")];

        // First line: bold (question text)
        if let Some(first) = message_lines.first() {
            lines.push(Line::from(Span::styled(
                first.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            )));
        }
        // Subsequent lines: cyan (preview info)
        for line_text in message_lines.iter().skip(1) {
            lines.push(Line::from(Span::styled(
                line_text.clone(),
                Style::default().fg(Color::Cyan),
            )));
        }

        lines.push(Line::from(""));

        if !detail_lines.is_empty() {
            for line_text in &detail_lines {
                lines.push(Line::from(Span::styled(
                    line_text.clone(),
                    Style::default().fg(Color::Yellow),
                )));
            }
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
