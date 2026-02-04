//! Error and notification message components
//!
//! Provides consistent styling for error messages and notifications.
//! For empty states, use `empty_state` module.

use ratatui::{
    prelude::*,
    text::{Line, Span},
};

use crate::model::{Notification, NotificationKind};

/// Build an error message line for overlay display
///
/// Returns a styled line suitable for rendering as a banner.
/// Format: `[red bg] Error: [/red bg][red text] message [/red text]`
pub fn build_error_line(error: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(" Error: ", Style::default().fg(Color::White).bg(Color::Red)),
        Span::styled(format!(" {} ", error), Style::default().fg(Color::Red)),
    ])
}

/// Build a notification line for title bar display
///
/// If `max_width` is provided and the notification is too long,
/// it will be truncated with "…" at the end.
pub fn build_notification_title(
    notification: &Notification,
    max_width: Option<usize>,
) -> Line<'static> {
    let (label, label_bg, text_fg) = match notification.kind {
        NotificationKind::Success => ("Success:", Color::Green, Color::Green),
        NotificationKind::Info => ("Info:", Color::Cyan, Color::Cyan),
        NotificationKind::Warning => ("Warning:", Color::Yellow, Color::Yellow),
    };

    let message = &notification.message;

    // Calculate full width: " | " + label + " " + message + " "
    let separator_width = 3; // " | "
    let label_width = label.len() + 1; // label + " "
    let message_display_width = message.chars().count() + 1; // message + " "
    let full_width = separator_width + label_width + message_display_width;

    let truncated_message = if let Some(max) = max_width {
        if full_width > max {
            // Calculate available space for message
            let available = max.saturating_sub(separator_width + label_width + 2); // +2 for "… "
            if available == 0 {
                // Not enough space, return empty
                return Line::from(vec![]);
            }
            let truncated: String = message.chars().take(available).collect();
            format!("{}… ", truncated)
        } else {
            format!("{} ", message)
        }
    } else {
        format!("{} ", message)
    };

    // Return empty line if truncated to nothing useful
    if truncated_message.trim().is_empty() || truncated_message == "… " {
        return Line::from(vec![]);
    }

    Line::from(vec![
        Span::raw(" | "),
        Span::styled(
            format!("{} ", label),
            Style::default().fg(Color::Black).bg(label_bg),
        ),
        Span::styled(truncated_message, Style::default().fg(text_fg)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_error_line() {
        let line = build_error_line("Connection failed");
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[0].content, " Error: ");
        assert_eq!(line.spans[1].content, " Connection failed ");
    }

    #[test]
    fn test_build_error_line_with_special_chars() {
        let line = build_error_line("Can't find file: /path/to/file");
        assert!(!line.spans.is_empty());
    }
}
