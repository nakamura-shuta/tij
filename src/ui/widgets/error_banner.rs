//! Error banner widget

use ratatui::{Frame, prelude::*, widgets::Paragraph};

use crate::ui::components;

/// Rows the banner may take at most. Same cap as `MAX_ERROR_LINES` in the
/// Command History detail, so a truncated stderr reads the same in both places.
pub const MAX_BANNER_ROWS: u16 = 5;

/// Rows the view keeps whatever the banner wants: a bordered view needs a top
/// border, one content row and a bottom border before it says anything at all.
pub const MIN_VIEW_ROWS: u16 = 3;

/// How many rows the error banner occupies.
///
/// `available` is what is left of the frame once the status bar and the command
/// echo row are taken off. The banner never eats into the last [`MIN_VIEW_ROWS`]
/// of that, so a short terminal keeps a usable view — and a terminal too short
/// for both simply gets no banner instead of a blanked-out view.
///
/// The caller must reserve exactly this many rows (see `App::view_area`):
/// [`render_error_banner`] draws precisely `height` rows, never more.
pub fn error_banner_height(error: &str, width: u16, available: u16) -> u16 {
    let wanted =
        components::build_error_lines(error, width as usize, MAX_BANNER_ROWS as usize).len() as u16;
    wanted.min(available.saturating_sub(MIN_VIEW_ROWS))
}

/// Render the error message on the `height` rows directly above `bottom_offset`
/// (status bar rows, plus the command echo row when it is on).
///
/// The `Paragraph` deliberately has no `.wrap()`: wrapping happens in
/// [`components::build_error_lines`] so the row count is known *before*
/// rendering. Letting ratatui reflow would make the drawn height differ from
/// the reserved height, and the view underneath would lose rows or gain blanks.
pub fn render_error_banner(frame: &mut Frame, error: &str, bottom_offset: u16, height: u16) {
    let area = frame.area();
    // Defensive: never draw past the bottom of the frame.
    let height = height.min(area.height.saturating_sub(bottom_offset));
    if height == 0 {
        return;
    }
    let error_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(bottom_offset + height),
        width: area.width,
        height,
    };

    let lines = components::build_error_lines(error, area.width as usize, height as usize);
    debug_assert_eq!(
        lines.len(),
        height as usize,
        "banner drew {} rows into a {}-row reservation",
        lines.len(),
        height
    );
    frame.render_widget(Paragraph::new(lines), error_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// jj's real two-line answer when a tag would create an untracked remote ref.
    const MULTILINE_STDERR: &str = "Error: Refusing to create new remote tag v1.0@other\n\
                                    Hint: Run `jj tag track v1.0@other` and try again.";

    /// The reservation and the drawing must never disagree: `error_banner_height`
    /// decides how many rows `view_area` gives up, `build_error_lines` decides
    /// how many rows get painted. A mismatch either clips the view or leaves
    /// stale rows behind.
    #[test]
    fn reserved_height_matches_rendered_line_count() {
        let errors = [
            "",
            "Error: single line",
            MULTILINE_STDERR,
            "l1\nl2\nl3\nl4\nl5\nl6\nl7",
            "Error: リモートタグ v1.0@other を新規作成することを拒否しました",
            &"x".repeat(400),
        ];
        for error in errors {
            for width in [1u16, 9, 10, 20, 40, 80, 200] {
                for available in [0u16, 1, 3, 4, 5, 6, 8, 24] {
                    let height = error_banner_height(error, width, available);
                    let drawn =
                        components::build_error_lines(error, width as usize, height as usize).len();
                    assert_eq!(
                        drawn, height as usize,
                        "error={error:?} width={width} available={available}"
                    );
                    assert!(
                        height <= MAX_BANNER_ROWS,
                        "banner grew past its cap: {height}"
                    );
                }
            }
        }
    }

    #[test]
    fn height_leaves_min_view_rows() {
        // Plenty of room: the banner takes exactly what the error needs.
        assert_eq!(error_banner_height(MULTILINE_STDERR, 80, 24), 2);
        // Tight: available - MIN_VIEW_ROWS is the ceiling.
        assert_eq!(error_banner_height(MULTILINE_STDERR, 80, 5), 2);
        assert_eq!(error_banner_height(MULTILINE_STDERR, 80, 4), 1);
        // Too short for both — the view wins, the banner disappears.
        assert_eq!(error_banner_height(MULTILINE_STDERR, 80, 3), 0);
        assert_eq!(error_banner_height(MULTILINE_STDERR, 80, 0), 0);
    }

    /// A blank error takes no rows at any terminal size — the view keeps them.
    #[test]
    fn blank_error_reserves_no_rows() {
        for error in ["", " ", "   \t ", "\n  \n"] {
            for available in [0u16, 3, 4, 24, 100] {
                assert_eq!(
                    error_banner_height(error, 80, available),
                    0,
                    "error={error:?} available={available}"
                );
            }
        }
    }

    #[test]
    fn height_is_capped_at_max_banner_rows() {
        let many = "a\nb\nc\nd\ne\nf\ng\nh";
        assert_eq!(error_banner_height(many, 80, 100), MAX_BANNER_ROWS);
    }

    #[test]
    fn narrow_terminal_wraps_into_more_rows() {
        // The same message needs one row at 80 columns and several at 24.
        let long = "Error: Refusing to create new remote tag v1.0@other";
        assert_eq!(error_banner_height(long, 80, 40), 1);
        assert!(error_banner_height(long, 24, 40) > 1);
    }

    /// A zero-height banner must not paint anything, and a bottom offset larger
    /// than the frame must not panic.
    #[test]
    fn render_is_a_noop_when_there_is_no_room() {
        let backend = ratatui::backend::TestBackend::new(20, 3);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_error_banner(f, MULTILINE_STDERR, 1, 0);
                render_error_banner(f, MULTILINE_STDERR, 99, 2);
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            !text.contains("Error:"),
            "nothing should be drawn: {text:?}"
        );
    }
}
