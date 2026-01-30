//! Placeholder view widget

use ratatui::{
    prelude::*,
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

/// Render a simple placeholder view with a title and body text.
pub fn render_placeholder(frame: &mut Frame, title: &str, color: Color, body: &str) {
    let area = frame.area();
    let title = Line::from(title).bold().fg(color).centered();
    frame.render_widget(
        Paragraph::new(body).block(Block::default().borders(Borders::ALL).title(title)),
        area,
    );
}
