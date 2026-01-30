//! Error banner widget

use ratatui::{Frame, prelude::*, text::Line, widgets::Paragraph};

/// Render an error message near the bottom of the screen
pub fn render_error_banner(frame: &mut Frame, error: &str) {
    let area = frame.area();
    let error_area = Rect {
        x: area.x + 2,
        y: area.y + area.height.saturating_sub(3),
        width: area.width.saturating_sub(4),
        height: 1,
    };

    let error_line = Line::from(vec![
        Span::styled(" Error: ", Style::default().fg(Color::White).bg(Color::Red)),
        Span::styled(format!(" {} ", error), Style::default().fg(Color::Red)),
    ]);

    frame.render_widget(Paragraph::new(error_line), error_area);
}
