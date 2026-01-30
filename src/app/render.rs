//! Rendering logic for the application

use ratatui::{
    Frame,
    prelude::*,
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

use super::state::{App, View};
use crate::keys;
use crate::ui::widgets::{render_error_banner, render_status_bar};

impl App {
    /// Render the UI
    pub fn render(&self, frame: &mut Frame) {
        match self.current_view {
            View::Log => self.render_log_view(frame),
            View::Diff => self.render_diff_view(frame),
            View::Status => self.render_status_view(frame),
            View::Help => self.render_help_view(frame),
        }

        // Render error message if present
        if let Some(ref error) = self.error_message {
            render_error_banner(frame, error);
        }
    }

    fn render_log_view(&self, frame: &mut Frame) {
        let area = frame.area();

        // Reserve space for status bar
        let main_area = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };

        self.log_view.render(frame, main_area);
        render_status_bar(frame);
    }

    fn render_diff_view(&self, frame: &mut Frame) {
        let area = frame.area();

        let title = Line::from(" Tij - Diff View ").bold().yellow().centered();

        frame.render_widget(
            Paragraph::new("Diff view - Press q to go back")
                .block(Block::default().borders(Borders::ALL).title(title)),
            area,
        );
    }

    fn render_status_view(&self, frame: &mut Frame) {
        let area = frame.area();

        let title = Line::from(" Tij - Status View ").bold().green().centered();

        frame.render_widget(
            Paragraph::new("Status view - Press q or Tab to go back")
                .block(Block::default().borders(Borders::ALL).title(title)),
            area,
        );
    }

    fn render_help_view(&self, frame: &mut Frame) {
        let area = frame.area();

        let title = Line::from(" Tij - Help ").bold().white().centered();

        let mut lines = vec![
            Line::from("Key bindings:".bold()),
            Line::from(""),
            Line::from("Global:".underlined()),
        ];

        for entry in keys::GLOBAL_KEYS {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:10}", entry.key),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(entry.description),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from("Navigation:".underlined()));

        for entry in keys::NAV_KEYS {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:10}", entry.key),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(entry.description),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from("Log View:".underlined()));

        for entry in keys::LOG_KEYS {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:10}", entry.key),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(entry.description),
            ]));
        }

        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title)),
            area,
        );
    }
}
