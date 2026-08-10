//! Error and notification message components
//!
//! Provides consistent styling for error messages and notifications.
//! For empty states, use `empty_state` module.

use std::borrow::Cow;

use ratatui::{
    prelude::*,
    text::{Line, Span},
};

use crate::model::{Notification, NotificationKind};
use crate::ui::text::display_width;

/// Label that opens the error banner's first row (8 terminal cells).
const ERROR_LABEL: &str = " Error: ";

/// Head of the plumbing prefix `JjError::CommandFailed`'s `Display` puts in
/// front of jj's stderr — `jj command failed (exit code {n}): `.
const CMD_FAILED_HEAD: &str = "jj command failed (exit code ";
/// Tail of that prefix, right after the exit code.
const CMD_FAILED_TAIL: &str = "): ";
/// jj's own stderr opener, which follows the prefix above and duplicates the
/// banner's `Error:` label.
const JJ_ERROR_HEAD: &str = "Error: ";

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
/// [`error_banner_height`](crate::ui::widgets::error_banner_height). A blank
/// error yields zero rows, i.e. no banner and no reservation at all.
///
/// This is the one place the banner text is shaped, so it is also where the
/// display-only trim lives (see [`strip_command_failed_wrapper`]): both the
/// height and the drawing come through here, which is what keeps them equal.
///
/// Format: `[red bg] Error: [/red bg][red text] message[/red text]`
pub fn build_error_lines(error: &str, width: usize, max_lines: usize) -> Vec<Line<'static>> {
    // A blank `error_message` has nothing to say: a lone `Error:` label is
    // noise *and* costs the view a row, so draw no banner at all. Returning
    // zero rows here is what makes `error_banner_height` reserve zero.
    if max_lines == 0 || width == 0 || error.trim().is_empty() {
        return Vec::new();
    }
    let error = strip_command_failed_wrapper(error);
    let label_width = display_width(ERROR_LABEL);
    // Every row reads `<label or indent><space><text>`.
    let text_width = width.saturating_sub(label_width + 1).max(1);

    let mut chunks: Vec<String> = Vec::new();
    for source_line in error.lines() {
        chunks.extend(wrap_display_width(source_line, text_width));
    }
    if chunks.is_empty() {
        return Vec::new();
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

/// Drop the plumbing that stacks up in front of jj's own message.
///
/// A failed `jj tag set` reaches the banner as four layers of preamble:
/// the banner label, tij's operation context, `JjError::CommandFailed`'s
/// `jj command failed (exit code 1): ` and jj's own `Error: ` — ~70 of an
/// 80-column terminal's cells before the first useful word. The middle
/// layer says nothing the red `Error:` label does not, and the exit code is
/// recoverable from the Command History, so both it and the `Error: ` that
/// directly follows it come off here.
///
/// Only the *display* is trimmed: this runs inside [`build_error_lines`], the
/// single place the banner is built, so `JjError`'s `Display` and the raw
/// stderr kept in the Command History are untouched.
///
/// tij's own context (`Tag creation failed: `) stays — it is the only thing
/// saying *which* operation failed. Text without the wrapper is returned
/// borrowed and byte-for-byte unchanged, and a wrapper with nothing after it
/// falls back to the original rather than blanking the banner.
fn strip_command_failed_wrapper(error: &str) -> Cow<'_, str> {
    if !error.contains(CMD_FAILED_HEAD) {
        return Cow::Borrowed(error);
    }
    let mut out = String::with_capacity(error.len());
    let mut rest = error;
    let mut stripped = false;

    while let Some(pos) = rest.find(CMD_FAILED_HEAD) {
        let after_head = &rest[pos + CMD_FAILED_HEAD.len()..];
        // `exit_code` is an i32 and `-1` is a real value (signal / spawn
        // failure), so the sign is part of the number to skip.
        let sign = usize::from(after_head.starts_with('-'));
        let digits = after_head[sign..]
            .bytes()
            .take_while(|b| b.is_ascii_digit())
            .count();
        let after_code = &after_head[sign + digits..];
        if digits == 0 || !after_code.starts_with(CMD_FAILED_TAIL) {
            // Looks like the prefix but is not one — keep it verbatim.
            out.push_str(&rest[..pos + CMD_FAILED_HEAD.len()]);
            rest = after_head;
            continue;
        }
        out.push_str(&rest[..pos]);
        let tail = &after_code[CMD_FAILED_TAIL.len()..];
        rest = tail.strip_prefix(JJ_ERROR_HEAD).unwrap_or(tail);
        stripped = true;
    }
    out.push_str(rest);

    if !stripped || out.trim().is_empty() {
        // Never swallow the whole message: an empty result would mean no
        // banner at all, hiding that anything failed.
        return Cow::Borrowed(error);
    }
    Cow::Owned(out)
}

/// Split `s` into pieces at most `width` terminal cells wide, breaking at
/// spaces where it can.
///
/// Each piece is a run of `word + its trailing spaces`, so the break lands
/// between words and the pieces still rejoin into `s` exactly — nothing is
/// dropped at a break, which is what keeps the row count honest. The trailing
/// space counts towards the width, so a break may leave one cell unused; that
/// is cheaper than a row starting with a stray space.
///
/// Word boundaries are not universal: CJK has no spaces, and a path or a
/// backticked command can be wider than the whole column. Any segment that
/// cannot fit a row on its own falls back to the per-character walk (by
/// [`display_width`], never by byte or char count), so the walk always makes
/// progress and no row ever overflows.
fn wrap_display_width(s: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    let mut buf = [0u8; 4];

    for segment in space_segments(s) {
        let segment_width = display_width(segment);
        if current_width + segment_width > width && !current.is_empty() {
            out.push(std::mem::take(&mut current));
            current_width = 0;
        }
        if segment_width <= width {
            current.push_str(segment);
            current_width += segment_width;
            continue;
        }
        // Wider than a whole row even on its own — walk it per character.
        for ch in segment.chars() {
            let w = display_width(ch.encode_utf8(&mut buf));
            if current_width + w > width && !current.is_empty() {
                out.push(std::mem::take(&mut current));
                current_width = 0;
            }
            current.push(ch);
            current_width += w;
        }
    }
    out.push(current);
    out
}

/// Split `s` into `word + trailing spaces` pieces — the units a wrap may not
/// break apart. Only ASCII spaces separate words; anything else stays inside
/// the word, where the per-character fallback can still split it.
fn space_segments(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut in_spaces = false;
    for (i, ch) in s.char_indices() {
        if ch == ' ' {
            in_spaces = true;
        } else if in_spaces {
            out.push(&s[start..i]);
            start = i;
            in_spaces = false;
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
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

    /// A blank error has nothing to show: a label-only row is noise and costs
    /// the view a row, so the banner disappears entirely.
    #[test]
    fn blank_error_draws_no_banner() {
        for error in ["", " ", "   \t ", "\n", "\n  \n"] {
            assert!(
                build_error_lines(error, 80, 5).is_empty(),
                "blank error must draw nothing: {error:?}"
            );
        }
    }

    #[test]
    fn zero_width_or_zero_budget_yields_nothing() {
        assert!(build_error_lines(MULTILINE_STDERR, 0, 5).is_empty());
        assert!(build_error_lines(MULTILINE_STDERR, 80, 0).is_empty());
    }

    /// The real Tag View failure: four layers of preamble before the message.
    /// The `JjError` wrapper and jj's duplicated `Error: ` come off; tij's
    /// operation context — the only thing naming the failed operation — stays.
    #[test]
    fn command_failed_wrapper_is_stripped_but_context_survives() {
        let raw = "Tag creation failed: jj command failed (exit code 1): \
                   Error: Refusing to move tag: v1.0\n\
                   Hint: Use --allow-move to update existing tags.";
        let rendered = texts(&build_error_lines(raw, 80, 5));
        assert_eq!(rendered.len(), 2, "{rendered:#?}");
        assert_eq!(
            rendered[0], " Error:  Tag creation failed: Refusing to move tag: v1.0",
            "{rendered:#?}"
        );
        assert!(!rendered[0].contains("jj command failed"), "{rendered:#?}");
        assert!(!rendered[0].contains("exit code"), "{rendered:#?}");
        // The actionable half is untouched.
        assert!(
            rendered[1].contains("Hint: Use --allow-move to update existing tags."),
            "{rendered:#?}"
        );
    }

    /// `exit_code` is an `i32`: spawn/signal failures record `-1`, and that
    /// wrapper must come off too.
    #[test]
    fn negative_exit_code_is_stripped() {
        let raw = "Push failed: jj command failed (exit code -1): Error: killed";
        let rendered = texts(&build_error_lines(raw, 80, 5));
        assert_eq!(rendered, vec![" Error:  Push failed: killed"]);
        // Multi-digit codes too.
        let raw = "Push failed: jj command failed (exit code 128): Error: killed";
        let rendered = texts(&build_error_lines(raw, 80, 5));
        assert_eq!(rendered, vec![" Error:  Push failed: killed"]);
    }

    /// Anything that is not the wrapper is passed through byte-for-byte —
    /// including a bare `Error: ` (jj's own, which the label may repeat) and
    /// prefix look-alikes with no exit code.
    #[test]
    fn messages_without_the_wrapper_are_untouched() {
        for error in [
            "Not a jj repository",
            "Error: Refusing to move tag: v1.0",
            "Tag creation failed: something else entirely",
            // Look-alikes: no digits, and no `): ` after the code.
            "jj command failed (exit code ): boom",
            "jj command failed (exit code 1) boom",
        ] {
            assert_eq!(
                strip_command_failed_wrapper(error),
                error,
                "must pass through unchanged"
            );
        }
    }

    /// Stripping must never leave the banner empty — an empty banner would
    /// hide that anything failed at all.
    #[test]
    fn wrapper_with_no_message_falls_back_to_the_raw_text() {
        let raw = "jj command failed (exit code 1): ";
        assert_eq!(strip_command_failed_wrapper(raw), raw);
        assert_eq!(texts(&build_error_lines(raw, 80, 5)).len(), 1);
    }

    /// Words used to be sliced mid-word (`Refusing t` / `o move tag`); breaks
    /// now land between words, and the rows still rejoin into the original.
    #[test]
    fn english_wraps_at_word_boundaries() {
        let long = "Error: Refusing to create new remote tag v1.0@other";
        let rendered = texts(&build_error_lines(long, 30, usize::MAX));
        assert!(rendered.len() > 1, "should wrap: {rendered:#?}");
        for row in &rendered {
            // Drop the label/indent, then the leading space of the text span.
            let text = row[9..].trim_end();
            assert!(
                display_width(row) <= 30,
                "row overflows: {row:?} ({})",
                display_width(row)
            );
            assert!(
                !text.is_empty() && long.split(' ').any(|w| text.starts_with(w)),
                "row starts mid-word: {text:?}"
            );
            assert!(
                long.split(' ').any(|w| text.ends_with(w)),
                "row ends mid-word: {text:?}"
            );
        }
    }

    /// Japanese has no spaces: word wrapping alone would leave one enormous
    /// row, so the per-character fallback must still bound every row — and
    /// terminate.
    #[test]
    fn japanese_without_spaces_still_wraps_by_display_width() {
        let error = "タグの作成に失敗しました: 既存のタグを移動することを拒否しました";
        // From width 12 up: below that the text column is narrower than one
        // 2-cell character, which has always overflowed (and is clipped) —
        // see `cjk_makes_progress_even_when_wider_than_the_column`.
        for width in [12usize, 20, 30, 40] {
            let rendered = texts(&build_error_lines(error, width, usize::MAX));
            let text_width = width - 9;
            for row in &rendered {
                assert!(
                    display_width(row) <= width,
                    "row overflows at width {width}: {row:?}"
                );
            }
            // Bounded row count: every row but the last is filled to within one
            // (2-cell) character of the column.
            let cells = display_width(error);
            assert!(
                rendered.len() <= cells.div_ceil(text_width.saturating_sub(1).max(1)) + 1,
                "too many rows at width {width}: {rendered:#?}"
            );
        }
    }

    /// A single word wider than the column (a long path, a URL) cannot be
    /// broken at a space, so it falls back to characters instead of
    /// overflowing the row.
    #[test]
    fn word_wider_than_the_column_falls_back_to_characters() {
        let long_word = "a".repeat(60);
        let error = format!("Fetch failed: {long_word} missing");
        let rendered = texts(&build_error_lines(&error, 40, usize::MAX));
        assert!(rendered.len() > 2, "{rendered:#?}");
        for row in &rendered {
            assert!(display_width(row) <= 40, "row overflows: {row:?}");
        }
        // Nothing is lost: the rows rejoin into the original text.
        let joined: String = build_error_lines(&error, 40, usize::MAX)
            .iter()
            .map(|l| l.spans[1].content[1..].to_string())
            .collect();
        assert_eq!(joined, error);
    }

    /// The wrap is lossless for CJK too — no character is dropped at a break.
    #[test]
    fn wrapping_never_drops_characters() {
        for error in [
            "Error: Refusing to create new remote tag v1.0@other",
            "エラー: リモートタグの作成を拒否しました",
            "  indented   with    runs   of spaces  ",
            "trailing space ",
        ] {
            for width in [10usize, 12, 17, 30, 80] {
                let joined: String = build_error_lines(error, width, usize::MAX)
                    .iter()
                    .map(|l| l.spans[1].content[1..].to_string())
                    .collect();
                assert_eq!(joined, error, "width={width}");
            }
        }
    }
}
