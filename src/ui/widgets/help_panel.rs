//! Help panel widget

use ratatui::{
    prelude::*,
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

use crate::keys;

/// Render help content showing key bindings.
pub fn render_help_panel(frame: &mut Frame, area: Rect) {
    let title = Line::from(" Tij - Help ").bold().white().centered();

    let mut lines = vec![Line::from("Key bindings:".bold()), Line::from("")];
    push_key_section(&mut lines, "Global", keys::GLOBAL_KEYS);
    push_key_section(&mut lines, "Navigation", keys::NAV_KEYS);
    push_key_section(&mut lines, "Log View", keys::LOG_KEYS);

    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title)),
        area,
    );
}

fn push_key_section(lines: &mut Vec<Line<'static>>, title: &str, entries: &[keys::KeyBindEntry]) {
    lines.push(Line::from(format!("{title}:")).underlined());
    for entry in entries {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:10}", entry.key),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(entry.description),
        ]));
    }
    lines.push(Line::from(""));
}
