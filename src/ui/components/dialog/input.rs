//! Input dialog handling and rendering

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::{Dialog, DialogKind, DialogResult, centered_rect};

impl Dialog {
    pub(super) fn handle_input_key(&mut self, key: KeyEvent) -> Option<DialogResult> {
        match key.code {
            KeyCode::Enter => {
                if let DialogKind::Input { ref buffer, .. } = self.kind {
                    Some(DialogResult::Confirmed(vec![buffer.clone()]))
                } else {
                    None
                }
            }
            KeyCode::Esc => Some(DialogResult::Cancelled),
            KeyCode::Backspace => {
                if let DialogKind::Input { ref mut buffer, .. } = self.kind {
                    buffer.pop();
                }
                None
            }
            KeyCode::Char(c) => {
                if let DialogKind::Input { ref mut buffer, .. } = self.kind {
                    buffer.push(c);
                }
                None
            }
            _ => None,
        }
    }

    pub(super) fn render_input(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        message: &str,
        buffer: &str,
    ) {
        let width = 50.min(area.width.saturating_sub(4));
        let height = 8u16.min(area.height.saturating_sub(4));

        let dialog_area = centered_rect(width, height, area);

        frame.render_widget(Clear, dialog_area);

        let inner_width = width.saturating_sub(4) as usize;
        let display_buffer = if buffer.chars().count() > inner_width && inner_width > 0 {
            let skip = buffer
                .chars()
                .count()
                .saturating_sub(inner_width.saturating_sub(1));
            format!("…{}", buffer.chars().skip(skip).collect::<String>())
        } else {
            buffer.to_string()
        };

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                message,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    display_buffer,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("_", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Enter]", Style::default().fg(Color::Green)),
                Span::raw(" Confirm   "),
                Span::styled("[Esc]", Style::default().fg(Color::Red)),
                Span::raw(" Cancel"),
            ]),
        ];

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
