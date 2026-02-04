//! Error banner widget

use ratatui::{Frame, prelude::*, widgets::Paragraph};

use crate::ui::components;

/// Render an error message above the status bar
///
/// Position: One line above the status bar (bottom area).
/// Only rendered when error exists.
pub fn render_error_banner(frame: &mut Frame, error: &str, status_bar_height: u16) {
    let area = frame.area();
    let error_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(status_bar_height + 1),
        width: area.width,
        height: 1,
    };

    let error_line = components::build_error_line(error);
    frame.render_widget(Paragraph::new(error_line), error_area);
}
