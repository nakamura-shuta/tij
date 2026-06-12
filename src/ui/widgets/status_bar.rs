//! Status bar widget
//!
//! Provides key hint display at the bottom of the screen.
//! Automatically switches to 2-row layout when terminal is too narrow.

use ratatui::{Frame, prelude::*, text::Line, widgets::Paragraph};

use crate::keys::KeyHint;
use crate::ui::views::{BlameView, DiffView};

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
pub fn status_hints_height(hints: &[KeyHint], width: u16) -> u16 {
    if total_hints_width(hints) > width as usize {
        3 // 2 rows + 1 spacer
    } else {
        1
    }
}

// Rendering
// ─────────────────────────────────────────────────────────────────────────────

/// Calculate status bar area at the bottom of the screen for a known height.
fn status_bar_area_h(frame: &Frame, height: u16) -> Option<Rect> {
    let area = frame.area();
    if area.height < 2 {
        return None;
    }

    // Fallback to single row if not enough space
    let actual_height = if area.height < height + 1 { 1 } else { height };

    Some(Rect {
        x: area.x,
        y: area.y + area.height - actual_height,
        width: area.width,
        height: actual_height,
    })
}

/// Calculate status bar area for plain (no-prefix) hint bars.
fn status_bar_area(frame: &Frame, hints: &[KeyHint]) -> Option<Rect> {
    status_bar_area_h(frame, status_hints_height(hints, frame.area().width))
}

/// Display width of a status-bar prefix in terminal cells (CJK-correct —
/// Blame/Diff prefixes carry file paths that may contain wide characters).
fn prefix_width(prefix: &[Span]) -> usize {
    prefix
        .iter()
        .map(|s| crate::ui::text::display_width(&s.content))
        .sum()
}

/// Width the hints add after a prefix — each hint is preceded by one space
/// (see [`build_status_bar_with_prefix`]).
fn prefixed_hints_width(hints: &[KeyHint]) -> usize {
    hints.iter().map(|h| 1 + hint_width(h)).sum()
}

/// Rows a prefixed bar needs: 1 when prefix + hints fit, else 3 (two rows +
/// spacer), mirroring [`build_content`]. Without this the bar ran off the
/// right edge because the height ignored the prefix.
fn prefixed_height(prefix_w: usize, hints: &[KeyHint], width: u16) -> u16 {
    if prefix_w + prefixed_hints_width(hints) <= width as usize {
        1
    } else {
        3
    }
}

/// Build prefixed status content, wrapping hints onto a second row when the
/// prefix + hints exceed the width so nothing overflows the right edge.
fn build_prefixed_content(
    prefix: Vec<Span<'static>>,
    prefix_w: usize,
    hints: &[KeyHint],
    width: u16,
) -> Vec<Line<'static>> {
    let width = width as usize;
    if prefix_w + prefixed_hints_width(hints) <= width {
        return vec![build_status_bar_with_prefix(prefix, hints)];
    }

    // Fill the first row (after the prefix) with as many hints as fit.
    let mut row_w = prefix_w;
    let mut split = 0;
    for (i, hint) in hints.iter().enumerate() {
        let w = 1 + hint_width(hint);
        if row_w + w > width {
            break;
        }
        row_w += w;
        split = i + 1;
    }
    let (first, second) = hints.split_at(split);
    vec![
        build_status_bar_with_prefix(prefix, first),
        Line::from(""),
        build_line(second),
    ]
}

/// Render status bar hints at the bottom of the screen
pub fn render_status_hints(frame: &mut Frame, hints: &[KeyHint]) {
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

/// Render the status bar for diff view (special: includes context prefix)
pub fn render_diff_status_bar(frame: &mut Frame, diff_view: &DiffView) {
    let hints = crate::keys::DIFF_VIEW_HINTS;
    let prefix = diff_prefix(diff_view);
    let prefix_w = prefix_width(&prefix);
    let height = prefixed_height(prefix_w, hints, frame.area().width);
    let Some(area) = status_bar_area_h(frame, height) else {
        return;
    };

    let content = if area.height >= 3 {
        build_prefixed_content(prefix, prefix_w, hints, area.width)
    } else {
        vec![build_status_bar_with_prefix(prefix, hints)]
    };
    frame.render_widget(Paragraph::new(content), area);
}

/// Render the status bar for blame view (special: includes file path prefix)
pub fn render_blame_status_bar(frame: &mut Frame, blame_view: &BlameView) {
    let hints = crate::keys::BLAME_VIEW_HINTS;
    let prefix = blame_prefix(blame_view);
    let prefix_w = prefix_width(&prefix);
    let height = prefixed_height(prefix_w, hints, frame.area().width);
    let Some(area) = status_bar_area_h(frame, height) else {
        return;
    };

    let content = if area.height >= 3 {
        build_prefixed_content(prefix, prefix_w, hints, area.width)
    } else {
        vec![build_status_bar_with_prefix(prefix, hints)]
    };
    frame.render_widget(Paragraph::new(content), area);
}

/// Status-bar prefix for the Diff view: revision tag + current file/context.
fn diff_prefix(diff_view: &DiffView) -> Vec<Span<'static>> {
    let context = diff_view.current_context();
    vec![
        Span::styled(
            format!(" {} ", diff_view.revision),
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ),
        Span::raw(" "),
        Span::styled(format!(" {} ", context), Style::default().fg(Color::Cyan)),
    ]
}

/// Status-bar prefix for the Blame view: the file path.
fn blame_prefix(blame_view: &BlameView) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(" {} ", blame_view.file_path()),
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ),
        Span::raw(" "),
    ]
}

/// Rows the Diff status bar needs at `width` (prefix-aware). The layout must
/// reserve this so the bar can wrap instead of overflowing.
pub fn diff_status_height(diff_view: &DiffView, width: u16) -> u16 {
    let prefix = diff_prefix(diff_view);
    prefixed_height(prefix_width(&prefix), crate::keys::DIFF_VIEW_HINTS, width)
}

/// Rows the Blame status bar needs at `width` (prefix-aware).
pub fn blame_status_height(blame_view: &BlameView, width: u16) -> u16 {
    let prefix = blame_prefix(blame_view);
    prefixed_height(prefix_width(&prefix), crate::keys::BLAME_VIEW_HINTS, width)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn hints3() -> &'static [KeyHint] {
        &[
            KeyHint {
                key: "j",
                label: "Down",
                color: Color::Cyan,
            },
            KeyHint {
                key: "k",
                label: "Up",
                color: Color::Cyan,
            },
            KeyHint {
                key: "q",
                label: "Back",
                color: Color::Red,
            },
        ]
    }

    #[test]
    fn prefixed_height_one_row_when_it_fits() {
        // Wide terminal: prefix + hints fit on one line.
        assert_eq!(prefixed_height(10, hints3(), 200), 1);
    }

    #[test]
    fn prefixed_height_three_rows_when_prefix_overflows() {
        // Hints alone fit, but a long prefix pushes total past the width →
        // must wrap (3 rows), not overflow the right edge.
        let hints = hints3();
        let hints_w = prefixed_hints_width(hints);
        let width = (hints_w + 5) as u16; // hints fit alone...
        assert_eq!(status_hints_height(hints, width), 1);
        // ...but with a 40-col prefix the bar no longer fits on one row.
        assert_eq!(prefixed_height(40, hints, width), 3);
    }

    #[test]
    fn build_prefixed_content_wraps_to_three_lines() {
        let hints = hints3();
        let prefix = vec![Span::raw("X".repeat(40))];
        let width = (prefixed_hints_width(hints) + 5) as u16;
        let lines = build_prefixed_content(prefix, 40, hints, width);
        assert_eq!(lines.len(), 3, "prefix + hints wrap: row, spacer, row");
        assert_eq!(lines[1].width(), 0, "middle line is the spacer");
    }

    #[test]
    fn build_prefixed_content_single_line_when_fits() {
        let hints = hints3();
        let prefix = vec![Span::raw(" rev ")];
        let lines = build_prefixed_content(prefix, 5, hints, 200);
        assert_eq!(lines.len(), 1);
    }

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
    fn test_status_hints_height_single() {
        let hints = &[KeyHint {
            key: "q",
            label: "Quit",
            color: Color::Red,
        }];
        assert_eq!(status_hints_height(hints, 80), 1);
    }

    #[test]
    fn test_status_hints_height_multi() {
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
        assert_eq!(status_hints_height(hints, 15), 3);
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
