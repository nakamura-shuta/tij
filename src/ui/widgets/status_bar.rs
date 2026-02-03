//! Status bar widget

use ratatui::{Frame, prelude::*, text::Line, widgets::Paragraph};

use crate::keys::{self, KeyHint};
use crate::ui::views::DiffView;

/// Build a status bar line from key hints
pub fn build_status_bar(hints: &[KeyHint]) -> Line<'static> {
    let mut spans = Vec::new();

    for (i, hint) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(
            format!(" [{}] {} ", hint.key, hint.label),
            Style::default().fg(Color::Black).bg(hint.color),
        ));
    }

    Line::from(spans)
}

/// Build a status bar line with a prefix and key hints
pub fn build_status_bar_with_prefix(
    prefix: Vec<Span<'static>>,
    hints: &[KeyHint],
) -> Line<'static> {
    let mut spans = prefix;

    for hint in hints {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!(" [{}] {} ", hint.key, hint.label),
            Style::default().fg(Color::Black).bg(hint.color),
        ));
    }

    Line::from(spans)
}

/// Calculate status bar area at bottom of screen
fn status_bar_area(frame: &Frame) -> Option<Rect> {
    let area = frame.area();
    if area.height < 2 {
        return None;
    }

    Some(Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    })
}

/// Render the status bar for log view
pub fn render_status_bar(frame: &mut Frame) {
    let Some(status_area) = status_bar_area(frame) else {
        return;
    };

    let status = build_status_bar(keys::LOG_VIEW_HINTS);
    frame.render_widget(Paragraph::new(status), status_area);
}

/// Render the status bar for diff view
pub fn render_diff_status_bar(frame: &mut Frame, diff_view: &DiffView) {
    let Some(status_area) = status_bar_area(frame) else {
        return;
    };

    // Build context info (current file)
    let context = diff_view.current_context();

    let prefix = vec![
        Span::styled(
            format!(" {} ", diff_view.change_id),
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ),
        Span::raw(" "),
        Span::styled(format!(" {} ", context), Style::default().fg(Color::Cyan)),
    ];

    let status = build_status_bar_with_prefix(prefix, keys::DIFF_VIEW_HINTS);
    frame.render_widget(Paragraph::new(status), status_area);
}

/// Render the status bar for status view
pub fn render_status_view_status_bar(frame: &mut Frame) {
    let Some(status_area) = status_bar_area(frame) else {
        return;
    };

    let status = build_status_bar(keys::STATUS_VIEW_HINTS);
    frame.render_widget(Paragraph::new(status), status_area);
}

/// Render the status bar for operation history view
pub fn render_operation_status_bar(frame: &mut Frame) {
    let Some(status_area) = status_bar_area(frame) else {
        return;
    };

    let status = build_status_bar(keys::OPERATION_VIEW_HINTS);
    frame.render_widget(Paragraph::new(status), status_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_status_bar() {
        let hints = &[
            KeyHint {
                key: "q",
                label: "Quit",
                color: Color::Red,
            },
            KeyHint {
                key: "?",
                label: "Help",
                color: Color::Cyan,
            },
        ];

        let line = build_status_bar(hints);
        // Line is created without panic
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_build_status_bar_with_prefix() {
        let prefix = vec![Span::raw("Test: ")];
        let hints = &[KeyHint {
            key: "q",
            label: "Quit",
            color: Color::Red,
        }];

        let line = build_status_bar_with_prefix(prefix, hints);
        assert!(!line.spans.is_empty());
    }
}
