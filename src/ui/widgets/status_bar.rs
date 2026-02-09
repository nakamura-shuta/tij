//! Status bar widget
//!
//! Provides key hint display at the bottom of the screen.
//! Automatically switches to 2-row layout when terminal is too narrow.

use ratatui::{Frame, prelude::*, text::Line, widgets::Paragraph};

use crate::keys::{self, KeyHint};
use crate::ui::views::{BlameView, DiffView, InputMode};

// ─────────────────────────────────────────────────────────────────────────────
// Hint formatting
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a single KeyHint to a styled Span
fn hint_to_span(hint: &KeyHint) -> Span<'static> {
    Span::styled(
        format!(" [{}] {} ", hint.key, hint.label),
        Style::default().fg(Color::Black).bg(hint.color),
    )
}

/// Calculate the display width of a hint (including brackets and spaces)
fn hint_width(hint: &KeyHint) -> usize {
    // Format: " [key] label " with space separator
    hint.key.len() + hint.label.len() + 5
}

/// Calculate the total width needed for hints
fn total_hints_width(hints: &[KeyHint]) -> usize {
    hints.iter().enumerate().fold(0, |acc, (i, hint)| {
        acc + hint_width(hint) + if i > 0 { 1 } else { 0 }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Line building
// ─────────────────────────────────────────────────────────────────────────────

/// Build a status bar line from key hints
fn build_line(hints: &[KeyHint]) -> Line<'static> {
    let mut spans = Vec::with_capacity(hints.len() * 2);

    for (i, hint) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(hint_to_span(hint));
    }

    Line::from(spans)
}

/// Build status bar content, splitting into multiple lines if needed
fn build_content(hints: &[KeyHint], width: u16) -> Vec<Line<'static>> {
    let width = width as usize;

    if total_hints_width(hints) <= width {
        // Single line
        return vec![build_line(hints)];
    }

    // Find split point: fill first row as much as possible
    let mut first_row_width = 0;
    let mut split_index = hints.len();

    for (i, hint) in hints.iter().enumerate() {
        let w = hint_width(hint) + if i > 0 { 1 } else { 0 };
        if first_row_width + w > width {
            split_index = i;
            break;
        }
        first_row_width += w;
    }

    // Ensure at least 1 hint on first line (avoid empty first row)
    let split_index = split_index.max(1);
    let (first_hints, second_hints) = hints.split_at(split_index);

    // Two lines with empty line separator for readability
    vec![
        build_line(first_hints),
        Line::from(""), // Spacer line
        build_line(second_hints),
    ]
}

/// Build a status bar line with a prefix and key hints
pub fn build_status_bar_with_prefix(
    prefix: Vec<Span<'static>>,
    hints: &[KeyHint],
) -> Line<'static> {
    let mut spans = prefix;

    for hint in hints {
        spans.push(Span::raw(" "));
        spans.push(hint_to_span(hint));
    }

    Line::from(spans)
}

// ─────────────────────────────────────────────────────────────────────────────
// Height calculation (for layout)
// ─────────────────────────────────────────────────────────────────────────────

/// Calculate status bar height for given hints and width
fn calc_height(hints: &[KeyHint], width: u16) -> u16 {
    if total_hints_width(hints) > width as usize {
        3 // 2 rows + 1 spacer
    } else {
        1
    }
}

/// Get the hints to use for the current log view mode
fn log_view_hints(input_mode: InputMode) -> &'static [KeyHint] {
    match input_mode {
        InputMode::CompareSelect => keys::COMPARE_SELECT_HINTS,
        _ => keys::LOG_VIEW_HINTS,
    }
}

/// Get the status bar height for log view
pub fn log_view_status_bar_height(width: u16, input_mode: InputMode) -> u16 {
    calc_height(log_view_hints(input_mode), width)
}

/// Get the status bar height for status view
pub fn status_view_status_bar_height(width: u16) -> u16 {
    calc_height(keys::STATUS_VIEW_HINTS, width)
}

/// Get the status bar height for operation view
pub fn operation_view_status_bar_height(width: u16) -> u16 {
    calc_height(keys::OPERATION_VIEW_HINTS, width)
}

/// Get the status bar height for blame view
pub fn blame_view_status_bar_height(width: u16) -> u16 {
    calc_height(keys::BLAME_VIEW_HINTS, width)
}

// ─────────────────────────────────────────────────────────────────────────────
// Rendering
// ─────────────────────────────────────────────────────────────────────────────

/// Calculate status bar area at bottom of screen
fn status_bar_area(frame: &Frame, hints: &[KeyHint]) -> Option<Rect> {
    let area = frame.area();
    if area.height < 2 {
        return None;
    }

    let height = calc_height(hints, area.width);

    // Fallback to single row if not enough space
    let actual_height = if area.height < height + 1 { 1 } else { height };

    Some(Rect {
        x: area.x,
        y: area.y + area.height - actual_height,
        width: area.width,
        height: actual_height,
    })
}

/// Generic status bar renderer
fn render_hints(frame: &mut Frame, hints: &[KeyHint]) {
    let Some(status_area) = status_bar_area(frame, hints) else {
        return;
    };

    let content = if status_area.height >= 3 {
        build_content(hints, status_area.width)
    } else {
        vec![build_line(hints)]
    };

    frame.render_widget(Paragraph::new(content), status_area);
}

/// Render the status bar for log view
pub fn render_status_bar(frame: &mut Frame, input_mode: InputMode) {
    render_hints(frame, log_view_hints(input_mode));
}

/// Render the status bar for status view
pub fn render_status_view_status_bar(frame: &mut Frame) {
    render_hints(frame, keys::STATUS_VIEW_HINTS);
}

/// Render the status bar for operation history view
pub fn render_operation_status_bar(frame: &mut Frame) {
    render_hints(frame, keys::OPERATION_VIEW_HINTS);
}

/// Render the status bar for diff view (special: includes context prefix)
pub fn render_diff_status_bar(frame: &mut Frame, diff_view: &DiffView) {
    let Some(status_area) = status_bar_area(frame, keys::DIFF_VIEW_HINTS) else {
        return;
    };

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

/// Render the status bar for blame view (special: includes file path prefix)
pub fn render_blame_status_bar(frame: &mut Frame, blame_view: &BlameView) {
    let Some(status_area) = status_bar_area(frame, keys::BLAME_VIEW_HINTS) else {
        return;
    };

    let file_path = blame_view.file_path();
    let prefix = vec![
        Span::styled(
            format!(" {} ", file_path),
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ),
        Span::raw(" "),
    ];

    let status = build_status_bar_with_prefix(prefix, keys::BLAME_VIEW_HINTS);
    frame.render_widget(Paragraph::new(status), status_area);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hint_to_span() {
        let hint = KeyHint {
            key: "q",
            label: "Quit",
            color: Color::Red,
        };
        let span = hint_to_span(&hint);
        assert!(span.content.contains("[q]"));
        assert!(span.content.contains("Quit"));
    }

    #[test]
    fn test_hint_width() {
        let hint = KeyHint {
            key: "q",
            label: "Quit",
            color: Color::Red,
        };
        // " [q] Quit " = 10 chars
        assert_eq!(hint_width(&hint), 10);
    }

    #[test]
    fn test_build_line() {
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

        let line = build_line(hints);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_build_content_single_line() {
        let hints = &[KeyHint {
            key: "q",
            label: "Quit",
            color: Color::Red,
        }];

        let content = build_content(hints, 80);
        assert_eq!(content.len(), 1);
    }

    #[test]
    fn test_build_content_two_lines() {
        let hints = &[
            KeyHint {
                key: "a",
                label: "AAAA",
                color: Color::Red,
            },
            KeyHint {
                key: "b",
                label: "BBBB",
                color: Color::Red,
            },
        ];

        // Width too small for both hints on one line
        let content = build_content(hints, 15);
        assert_eq!(content.len(), 3); // 2 lines + 1 spacer
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

    #[test]
    fn test_calc_height_single() {
        let hints = &[KeyHint {
            key: "q",
            label: "Quit",
            color: Color::Red,
        }];
        assert_eq!(calc_height(hints, 80), 1);
    }

    #[test]
    fn test_calc_height_multi() {
        let hints = &[
            KeyHint {
                key: "a",
                label: "AAAA",
                color: Color::Red,
            },
            KeyHint {
                key: "b",
                label: "BBBB",
                color: Color::Red,
            },
        ];
        assert_eq!(calc_height(hints, 15), 3);
    }

    #[test]
    fn test_build_content_extremely_narrow() {
        // Edge case: width so narrow that even first hint doesn't fit
        let hints = &[
            KeyHint {
                key: "a",
                label: "AAAA",
                color: Color::Red,
            },
            KeyHint {
                key: "b",
                label: "BBBB",
                color: Color::Red,
            },
        ];

        // Width = 5, way too narrow for any hint
        let content = build_content(hints, 5);

        // Should still have 3 lines (first row should have at least 1 hint)
        assert_eq!(content.len(), 3);
        // First line should NOT be empty
        assert!(!content[0].spans.is_empty());
    }
}
