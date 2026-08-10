//! Error and notification message components
//!
//! Provides consistent styling for error messages and notifications.
//! For empty states, use `empty_state` module.

use ratatui::{
    prelude::*,
    text::{Line, Span},
};

use crate::model::{Notification, NotificationKind};
use crate::ui::text::display_width;

/// Label that opens the error banner's first row (8 terminal cells).
const ERROR_LABEL: &str = " Error: ";

/// Build the error banner's rows, wrapped to `width` terminal cells.
///
/// jj answers most failures with two or more stderr lines — an `Error:` line
/// plus the actionable `Hint:` line — and a single one routinely overruns an
/// 80-column terminal, so this both honours hard newlines and wraps. The first
/// row carries the `Error:` label; continuation rows are indented underneath
/// it, matching the Command History detail.
///
/// At most `max_lines` rows come back; when more were needed the last row is a
/// `... (N more lines)` marker (same wording as the Command History detail).
/// The returned length is always `min(rows_needed, max_lines)` — the banner's
/// height reservation leans on that, see
/// [`error_banner_height`](crate::ui::widgets::error_banner_height).
///
/// Format: `[red bg] Error: [/red bg][red text] message[/red text]`
pub fn build_error_lines(error: &str, width: usize, max_lines: usize) -> Vec<Line<'static>> {
    if max_lines == 0 || width == 0 {
        return Vec::new();
    }
    let label_width = display_width(ERROR_LABEL);
    // Every row reads `<label or indent><space><text>`.
    let text_width = width.saturating_sub(label_width + 1).max(1);

    let mut chunks: Vec<String> = Vec::new();
    for source_line in error.lines() {
        chunks.extend(wrap_display_width(source_line, text_width));
    }
    if chunks.is_empty() {
        // `Some("")` still means "there is an error" — keep the label visible.
        chunks.push(String::new());
    }

    let total = chunks.len();
    let shown = if total <= max_lines {
        total
    } else if max_lines == 1 {
        // With room for one row only, a row of real text beats a lone marker.
        1
    } else {
        max_lines - 1
    };

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(shown + 1);
    for (i, chunk) in chunks.iter().take(shown).enumerate() {
        let head = if i == 0 {
            Span::styled(
                ERROR_LABEL,
                Style::default().fg(Color::White).bg(Color::Red),
            )
        } else {
            Span::raw(" ".repeat(label_width))
        };
        lines.push(Line::from(vec![
            head,
            Span::styled(format!(" {}", chunk), Style::default().fg(Color::Red)),
        ]));
    }
    if total > shown && max_lines > 1 {
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(label_width)),
            Span::styled(
                format!(" ... ({} more lines)", total - shown),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }
    lines
}

/// Split `s` into pieces at most `width` terminal cells wide.
///
/// Character-level rather than word-level on purpose: error text mixes paths,
/// backticked commands and CJK, where word boundaries either do not exist or
/// do not help, and a per-character walk keeps the row count exact. A single
/// character wider than `width` still gets its own piece, so the walk always
/// makes progress.
fn wrap_display_width(s: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    let mut buf = [0u8; 4];
    for ch in s.chars() {
        let w = display_width(ch.encode_utf8(&mut buf));
        if current_width + w > width && !current.is_empty() {
            out.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push(ch);
        current_width += w;
    }
    out.push(current);
    out
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

    /// jj's real two-line answer when a tag would create an untracked remote
    /// ref: the second line is the actionable half.
    const MULTILINE_STDERR: &str = "Error: Refusing to create new remote tag v1.0@other\n\
                                    Hint: Run `jj tag track v1.0@other` and try again.";

    fn texts(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn test_build_error_line() {
        let lines = build_error_lines("Connection failed", 80, 5);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans.len(), 2);
        assert_eq!(lines[0].spans[0].content, " Error: ");
        assert_eq!(lines[0].spans[1].content, " Connection failed");
    }

    #[test]
    fn test_build_error_line_with_special_chars() {
        let lines = build_error_lines("Can't find file: /path/to/file", 80, 5);
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].spans.is_empty());
    }

    /// The whole point of the multi-line banner: the `Hint:` line is the half
    /// that tells the user what to do, and a one-row banner dropped it.
    #[test]
    fn hint_line_is_rendered_on_its_own_row() {
        let rendered = texts(&build_error_lines(MULTILINE_STDERR, 80, 5));
        assert_eq!(rendered.len(), 2, "{rendered:#?}");
        assert!(rendered[0].contains("Refusing to create new remote tag v1.0@other"));
        assert!(
            rendered[1].contains("Hint: Run `jj tag track v1.0@other` and try again."),
            "{rendered:#?}"
        );
        // Continuation rows are indented under the label, not re-labelled.
        assert!(!rendered[1].contains("Error: "), "{:?}", rendered[1]);
        assert!(rendered[1].starts_with("         "), "{:?}", rendered[1]);
    }

    #[test]
    fn long_line_wraps_at_the_terminal_width() {
        let long = "Error: Refusing to create new remote tag v1.0@other";
        assert_eq!(build_error_lines(long, 80, 5).len(), 1, "fits at 80");

        let rendered = texts(&build_error_lines(long, 30, 5));
        assert!(
            rendered.len() > 1,
            "should wrap at 30 columns: {rendered:#?}"
        );
        for row in &rendered {
            assert!(
                display_width(row) <= 30,
                "row overflows the terminal: {row:?} ({})",
                display_width(row)
            );
        }
        // Wrapping must not drop or duplicate characters: the text spans
        // (everything after the label/indent) rejoin into the original.
        let joined: String = build_error_lines(long, 30, usize::MAX)
            .iter()
            .map(|l| l.spans[1].content[1..].to_string())
            .collect();
        assert_eq!(joined, long);
    }

    /// v0.10.1 introduced `display_width` because CJK is two cells wide;
    /// wrapping by `chars().count()` would overflow every row by up to 2x.
    #[test]
    fn cjk_wraps_by_display_width_not_char_count() {
        let error = "エラー: リモートタグの作成を拒否しました";
        let rendered = texts(&build_error_lines(error, 30, 5));
        assert!(rendered.len() > 1, "should wrap: {rendered:#?}");
        for row in &rendered {
            assert!(
                display_width(row) <= 30,
                "CJK row overflows: {row:?} ({})",
                display_width(row)
            );
        }
    }

    /// A character wider than the text column still gets a row of its own —
    /// otherwise the wrap loop would never advance.
    #[test]
    fn cjk_makes_progress_even_when_wider_than_the_column() {
        let rendered = texts(&build_error_lines("あいうえお", 10, 5));
        assert_eq!(rendered.len(), 5);
    }

    #[test]
    fn caps_at_max_lines_with_a_more_marker() {
        let rendered = texts(&build_error_lines("l1\nl2\nl3\nl4\nl5\nl6\nl7", 80, 5));
        assert_eq!(rendered.len(), 5);
        assert!(rendered[3].contains("l4"));
        assert!(!rendered.iter().any(|r| r.contains("l5")));
        assert!(
            rendered[4].contains("... (3 more lines)"),
            "{:?}",
            rendered[4]
        );
    }

    /// With room for a single row, showing the actual error beats showing only
    /// a "3 more lines" marker.
    #[test]
    fn single_row_budget_shows_text_rather_than_the_marker() {
        let rendered = texts(&build_error_lines(MULTILINE_STDERR, 80, 1));
        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].contains("Refusing to create"), "{rendered:#?}");
        assert!(!rendered[0].contains("more lines"));
    }

    /// `error_banner_height` reserves `min(needed, max_lines)` rows; the builder
    /// must return exactly that many or the view loses/gains rows.
    #[test]
    fn line_count_is_min_of_needed_and_max_lines() {
        for error in [
            "",
            "one line",
            MULTILINE_STDERR,
            "l1\nl2\nl3\nl4\nl5\nl6\nl7",
            "日本語のとても長いエラーメッセージがここに入ります",
        ] {
            let needed = build_error_lines(error, 40, usize::MAX).len();
            for max in 0..=8usize {
                assert_eq!(
                    build_error_lines(error, 40, max).len(),
                    needed.min(max),
                    "error={error:?} max={max}"
                );
            }
        }
    }

    #[test]
    fn empty_error_still_shows_the_label() {
        let rendered = texts(&build_error_lines("", 80, 5));
        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].starts_with(" Error: "));
    }

    #[test]
    fn zero_width_or_zero_budget_yields_nothing() {
        assert!(build_error_lines(MULTILINE_STDERR, 0, 5).is_empty());
        assert!(build_error_lines(MULTILINE_STDERR, 80, 0).is_empty());
    }
}
