//! Placeholder view widget

use ratatui::{
    prelude::*,
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

/// Render a simple placeholder view with a title and body text.
///
/// Takes the target `area` (callers pass `App::view_area`) so placeholder
/// routes obey the same screen-real-estate rules as loaded views — with the
/// command echo bar on, the placeholder must not paint the echo row.
pub fn render_placeholder(frame: &mut Frame, area: Rect, title: &str, color: Color, body: &str) {
    let title = Line::from(title).bold().fg(color).centered();
    frame.render_widget(
        Paragraph::new(body).block(Block::default().borders(Borders::ALL).title(title)),
        area,
    );
}
