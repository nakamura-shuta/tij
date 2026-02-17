//! Help panel widget
//!
//! Provides key binding display with optional search highlighting.
//! `build_help_lines()` is the Single Source of Truth for both rendering and search navigation.

use ratatui::{
    layout::{Constraint, Layout},
    prelude::*,
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

use crate::keys;

/// A single line in the help panel (used for both rendering and search)
#[allow(dead_code)]
pub struct HelpLine {
    /// The styled line for display
    pub line: Line<'static>,
    /// Whether this line is a key binding entry (vs section title / blank)
    pub is_entry: bool,
    /// Whether this line matches the current search query
    pub matched: bool,
}

/// Build all help panel lines (Single Source of Truth for rendering and search).
///
/// When `search_query` is `Some`, matching entries get `matched = true` and
/// are rendered with a highlight style.
pub fn build_help_lines(search_query: Option<&str>) -> Vec<HelpLine> {
    let query_lower = search_query.map(|q| q.to_lowercase());

    let mut lines = Vec::new();

    // Header
    lines.push(HelpLine {
        line: Line::from("Key bindings:".bold()),
        is_entry: false,
        matched: false,
    });
    lines.push(HelpLine {
        line: Line::from(""),
        is_entry: false,
        matched: false,
    });

    push_section(
        &mut lines,
        "Global",
        keys::GLOBAL_KEYS,
        query_lower.as_deref(),
    );
    push_section(
        &mut lines,
        "Navigation",
        keys::NAV_KEYS,
        query_lower.as_deref(),
    );
    push_section(
        &mut lines,
        "Log View",
        keys::LOG_KEYS,
        query_lower.as_deref(),
    );
    push_section(
        &mut lines,
        "Input Mode",
        keys::INPUT_KEYS,
        query_lower.as_deref(),
    );
    push_section(
        &mut lines,
        "Diff View",
        keys::DIFF_KEYS,
        query_lower.as_deref(),
    );
    push_section(
        &mut lines,
        "Status View",
        keys::STATUS_KEYS,
        query_lower.as_deref(),
    );
    push_section(
        &mut lines,
        "Bookmark View",
        keys::BOOKMARK_KEYS,
        query_lower.as_deref(),
    );
    push_section(
        &mut lines,
        "Operation View",
        keys::OPERATION_KEYS,
        query_lower.as_deref(),
    );

    lines
}

fn push_section(
    lines: &mut Vec<HelpLine>,
    title: &str,
    entries: &[keys::KeyBindEntry],
    query_lower: Option<&str>,
) {
    // Section title line
    lines.push(HelpLine {
        line: Line::from(format!("{title}:")).underlined(),
        is_entry: false,
        matched: false,
    });

    for entry in entries {
        let matched = query_lower.is_some_and(|q| {
            entry.key.to_lowercase().contains(q) || entry.description.to_lowercase().contains(q)
        });

        let style = if matched {
            Style::default().bg(Color::Yellow).fg(Color::Black)
        } else {
            Style::default()
        };

        let key_style = if matched {
            Style::default().bg(Color::Yellow).fg(Color::Black).bold()
        } else {
            Style::default().fg(Color::Yellow)
        };

        lines.push(HelpLine {
            line: Line::from(vec![
                Span::styled(format!("  {:10}", entry.key), key_style),
                Span::styled(entry.description.to_string(), style),
            ]),
            is_entry: true,
            matched,
        });
    }

    // Blank separator
    lines.push(HelpLine {
        line: Line::from(""),
        is_entry: false,
        matched: false,
    });
}

/// Collect indices of matching lines (for n/N navigation)
pub fn matching_line_indices(query: &str) -> Vec<u16> {
    build_help_lines(Some(query))
        .iter()
        .enumerate()
        .filter(|(_, l)| l.matched)
        .map(|(i, _)| i as u16)
        .collect()
}

/// Render help content showing key bindings.
///
/// `scroll` is the vertical scroll offset (0 = top). Values beyond the
/// content length are clamped by ratatui's Paragraph.
///
/// `search_query` highlights matching entries when `Some`.
/// `search_input` shows a search input bar at the bottom when `Some`.
pub fn render_help_panel(
    frame: &mut Frame,
    area: Rect,
    scroll: u16,
    search_query: Option<&str>,
    search_input: Option<&str>,
) {
    let title = Line::from(" Tij - Help ").bold().white().centered();

    // Split area for input bar if searching
    let (help_area, input_area) = if search_input.is_some() {
        let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let help_lines = build_help_lines(search_query);
    let display_lines: Vec<Line<'static>> = help_lines.into_iter().map(|hl| hl.line).collect();

    frame.render_widget(
        Paragraph::new(display_lines)
            .block(Block::default().borders(Borders::ALL).title(title))
            .scroll((scroll, 0)),
        help_area,
    );

    // Render search input bar
    if let Some(buffer) = search_input {
        let input_text = format!("Search: {buffer}");
        let available_width = input_area.unwrap().width.saturating_sub(2) as usize;
        let char_count = input_text.chars().count();
        let display_text = if char_count > available_width && available_width > 0 {
            let skip = char_count.saturating_sub(available_width.saturating_sub(1));
            format!("…{}", input_text.chars().skip(skip).collect::<String>())
        } else {
            input_text.clone()
        };

        let input_bar = Paragraph::new(display_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(Line::from(" / Search ")),
        );
        let ia = input_area.unwrap();
        frame.render_widget(input_bar, ia);

        // Cursor position
        let cursor_pos = char_count.min(available_width);
        frame.set_cursor_position((ia.x + cursor_pos as u16 + 1, ia.y + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_help_lines_no_query_has_no_matches() {
        let lines = build_help_lines(None);
        assert!(lines.iter().all(|l| !l.matched));
        assert!(!lines.is_empty());
    }

    #[test]
    fn build_help_lines_quit_matches() {
        let lines = build_help_lines(Some("quit"));
        let matched: Vec<_> = lines.iter().filter(|l| l.matched).collect();
        assert!(!matched.is_empty(), "Should match at least one Quit entry");
    }

    #[test]
    fn build_help_lines_bookmark_matches_multiple_sections() {
        let lines = build_help_lines(Some("bookmark"));
        let matched: Vec<_> = lines.iter().filter(|l| l.matched).collect();
        assert!(
            matched.len() >= 2,
            "bookmark should match in multiple sections"
        );
    }

    #[test]
    fn build_help_lines_no_match_returns_all_false() {
        let lines = build_help_lines(Some("zzzzzznonexistent"));
        assert!(lines.iter().all(|l| !l.matched));
    }

    #[test]
    fn build_help_lines_case_insensitive() {
        let upper = build_help_lines(Some("QUIT"));
        let lower = build_help_lines(Some("quit"));
        let upper_count = upper.iter().filter(|l| l.matched).count();
        let lower_count = lower.iter().filter(|l| l.matched).count();
        assert_eq!(
            upper_count, lower_count,
            "Search should be case-insensitive"
        );
        assert!(upper_count > 0);
    }

    #[test]
    fn matching_line_indices_returns_correct_indices() {
        let indices = matching_line_indices("quit");
        assert!(!indices.is_empty());
        // Verify indices are valid
        let lines = build_help_lines(Some("quit"));
        for &idx in &indices {
            assert!(lines[idx as usize].matched);
        }
    }

    #[test]
    fn matching_line_indices_empty_for_nonexistent() {
        let indices = matching_line_indices("zzzzz");
        assert!(indices.is_empty());
    }

    #[test]
    fn build_help_lines_entries_have_is_entry_true() {
        let lines = build_help_lines(None);
        let entries: Vec<_> = lines.iter().filter(|l| l.is_entry).collect();
        assert!(entries.len() > 20, "Should have many key binding entries");
    }
}
