//! Notification banner widget
//!
//! Displays temporary feedback messages for operations like undo/redo.

use ratatui::{prelude::*, text::Line, widgets::Paragraph};

use crate::model::{Notification, NotificationKind};

/// Render a notification banner near the bottom of the screen
///
/// Position: Above the status bar, similar to error_banner but with
/// different colors based on notification kind.
///
/// `status_bar_height` - The height of the status bar (1 or 3 for 2-row mode)
pub fn render_notification_banner(
    frame: &mut Frame,
    notification: &Notification,
    status_bar_height: u16,
) {
    let area = frame.area();
    // Position just above the status bar
    let y_offset = status_bar_height + 1;
    let banner_area = Rect {
        x: area.x + 2,
        y: area.y + area.height.saturating_sub(y_offset),
        width: area.width.saturating_sub(4),
        height: 1,
    };

    let line = build_notification_line(notification);
    frame.render_widget(Paragraph::new(line), banner_area);
}

/// Build a styled line for the notification
fn build_notification_line(notification: &Notification) -> Line<'static> {
    let (label, label_bg, text_fg) = match notification.kind {
        NotificationKind::Success => (" Success: ", Color::Green, Color::Green),
        NotificationKind::Info => (" Info: ", Color::Cyan, Color::Cyan),
        NotificationKind::Warning => (" Warning: ", Color::Yellow, Color::Yellow),
    };

    Line::from(vec![
        Span::styled(label, Style::default().fg(Color::Black).bg(label_bg)),
        Span::styled(
            format!(" {} ", notification.message),
            Style::default().fg(text_fg),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_notification_line_success() {
        let n = Notification::success("Undo complete");
        let line = build_notification_line(&n);
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[0].content, " Success: ");
        assert_eq!(line.spans[1].content, " Undo complete ");
    }

    #[test]
    fn test_build_notification_line_info() {
        let n = Notification::info("Nothing to redo");
        let line = build_notification_line(&n);
        assert_eq!(line.spans[0].content, " Info: ");
    }

    #[test]
    fn test_build_notification_line_warning() {
        let n = Notification::warning("Some operations cannot be undone");
        let line = build_notification_line(&n);
        assert_eq!(line.spans[0].content, " Warning: ");
    }
}
