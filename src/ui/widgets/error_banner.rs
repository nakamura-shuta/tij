//! Error banner widget

use ratatui::{Frame, prelude::*, widgets::Paragraph};

use crate::ui::components;

/// Render an error message near the bottom of the screen
///
/// `status_bar_height` - The height of the status bar (1 or 3 for 2-row mode)
pub fn render_error_banner(frame: &mut Frame, error: &str, status_bar_height: u16) {
    let area = frame.area();
    // Position just above the status bar
    let y_offset = status_bar_height + 1;
    let error_area = Rect {
        x: area.x + 2,
        y: area.y + area.height.saturating_sub(y_offset),
        width: area.width.saturating_sub(4),
        height: 1,
    };

    let error_line = components::build_error_line(error);
    frame.render_widget(Paragraph::new(error_line), error_area);
}
