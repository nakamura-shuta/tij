//! Error banner widget

use ratatui::{Frame, prelude::*, widgets::Paragraph};

use crate::ui::components;

/// Render an error message near the bottom of the screen
pub fn render_error_banner(frame: &mut Frame, error: &str) {
    let area = frame.area();
    let error_area = Rect {
        x: area.x + 2,
        y: area.y + area.height.saturating_sub(3),
        width: area.width.saturating_sub(4),
        height: 1,
    };

    let error_line = components::build_error_line(error);
    frame.render_widget(Paragraph::new(error_line), error_area);
}
