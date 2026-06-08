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

/// Synonym map for keyword-linked highlighting.
/// When a search query matches a trigger keyword (prefix match supported),
/// the expansion terms are also used as additional search queries.
const SYNONYM_MAP: &[(&str, &[&str])] = &[
    ("commit", &["describe", "message", "new", "squash"]),
    ("rebase", &["move", "insert", "source", "destination"]),
    (
        "bookmark",
        &["branch", "track", "untrack", "forget", "rename"],
    ),
    ("tag", &["release", "version", "label"]),
    ("undo", &["redo", "restore", "operation"]),
    ("diff", &["show", "compare", "blame", "export"]),
    ("search", &["filter", "revset"]),
    ("conflict", &["resolve", "merge"]),
    ("push", &["remote", "git"]),
    ("copy", &["clipboard", "export", "patch"]),
    ("navigate", &["next", "prev", "jump"]),
    ("edit", &["describe", "diffedit", "split", "fix", "editor"]),
    ("history", &["command", "execute", "log"]),
];

/// Expand a query into additional search terms via the synonym map.
/// Supports prefix matching: "reb" matches "rebase" entry.
fn expand_synonyms(query_lower: &str) -> Vec<&'static str> {
    if query_lower.is_empty() {
        return Vec::new();
    }
    let mut expansions = Vec::new();
    for &(trigger, terms) in SYNONYM_MAP {
        if trigger.starts_with(query_lower) || query_lower.starts_with(trigger) {
            expansions.extend_from_slice(terms);
        }
    }
    expansions
}

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

/// All help sections in canonical order (includes Blame/Resolve/Evolog, fixing previous omission).
fn all_sections() -> [(&'static str, &'static [keys::KeyBindEntry]); 14] {
    [
        ("Global", keys::GLOBAL_KEYS),
        ("Navigation", keys::NAV_KEYS),
        ("Log View", keys::LOG_KEYS),
        ("Input Mode", keys::INPUT_KEYS),
        ("Diff View", keys::DIFF_KEYS),
        ("Status View", keys::STATUS_KEYS),
        ("Bookmark View", keys::BOOKMARK_KEYS),
        ("Tag View", keys::TAG_KEYS),
        ("Workspace View", keys::WORKSPACE_KEYS),
        ("Command History View", keys::COMMAND_HISTORY_KEYS),
        ("Operation View", keys::OPERATION_KEYS),
        ("Blame View", keys::BLAME_KEYS),
        ("Resolve View", keys::RESOLVE_KEYS),
        ("Evolog View", keys::EVOLOG_KEYS),
    ]
}

/// Map a `View` to its section title string.
pub(crate) fn section_title_for(view: crate::app::View) -> &'static str {
    use crate::app::View;
    match view {
        View::Log => "Log View",
        View::Status => "Status View",
        View::Diff => "Diff View",
        View::Operation => "Operation View",
        View::Bookmark => "Bookmark View",
        View::Tag => "Tag View",
        View::Workspace => "Workspace View",
        View::CommandHistory => "Command History View",
        View::Blame => "Blame View",
        View::Resolve => "Resolve View",
        View::Evolog => "Evolog View",
        View::TraceDetail => "Agent Traces View",
        View::Help => "Help",
    }
}

/// Build all help panel lines (Single Source of Truth for rendering and search).
///
/// When `search_query` is `Some`, matching entries get `matched = true` and
/// are rendered with a highlight style.
///
/// When `current_view` is `Some` and `show_all` is `false`, only the sections
/// for that view (plus Global and Navigation, deduped) are shown.
/// When `show_all` is `true` or `current_view` is `None`, all sections are shown.
pub fn build_help_lines(
    search_query: Option<&str>,
    current_view: Option<crate::app::View>,
    show_all: bool,
) -> Vec<HelpLine> {
    let query_lower = search_query.map(|q| q.to_lowercase());
    let synonyms = query_lower
        .as_deref()
        .map(expand_synonyms)
        .unwrap_or_default();

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

    // The Help panel's own controls (search / nav / toggle / close) — shown in
    // both current-view and all-views modes so they're discoverable here.
    push_section(
        &mut lines,
        "Help panel",
        keys::HELP_KEYS,
        query_lower.as_deref(),
        &synonyms,
    );

    match (current_view, show_all) {
        (Some(view), false) => {
            let mut seen_keys: std::collections::HashSet<&'static str> =
                std::collections::HashSet::new();
            let view_title = section_title_for(view);
            push_section_dedup(
                &mut lines,
                view_title,
                keys::keys_for_view(view),
                query_lower.as_deref(),
                &synonyms,
                &mut seen_keys,
            );
            push_section_dedup(
                &mut lines,
                "Global",
                keys::GLOBAL_KEYS,
                query_lower.as_deref(),
                &synonyms,
                &mut seen_keys,
            );
            push_section_dedup(
                &mut lines,
                "Navigation",
                keys::NAV_KEYS,
                query_lower.as_deref(),
                &synonyms,
                &mut seen_keys,
            );
        }
        _ => {
            for (title, entries) in all_sections() {
                push_section(
                    &mut lines,
                    title,
                    entries,
                    query_lower.as_deref(),
                    &synonyms,
                );
            }
        }
    }

    lines
}

fn push_section_dedup(
    lines: &mut Vec<HelpLine>,
    title: &str,
    entries: &[keys::KeyBindEntry],
    query_lower: Option<&str>,
    synonyms: &[&str],
    seen_keys: &mut std::collections::HashSet<&'static str>,
) {
    let fresh: Vec<&keys::KeyBindEntry> = entries
        .iter()
        .filter(|e| !seen_keys.contains(e.key))
        .collect();
    if fresh.is_empty() {
        return;
    }
    for e in &fresh {
        seen_keys.insert(e.key);
    }
    lines.push(HelpLine {
        line: Line::from(format!("{title}:")).underlined(),
        is_entry: false,
        matched: false,
    });
    for entry in fresh {
        let matched = query_lower.is_some_and(|q| {
            let key_lc = entry.key.to_lowercase();
            let desc_lc = entry.description.to_lowercase();
            key_lc.contains(q)
                || desc_lc.contains(q)
                || synonyms
                    .iter()
                    .any(|s| key_lc.contains(s) || desc_lc.contains(s))
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
    lines.push(HelpLine {
        line: Line::from(""),
        is_entry: false,
        matched: false,
    });
}

fn push_section(
    lines: &mut Vec<HelpLine>,
    title: &str,
    entries: &[keys::KeyBindEntry],
    query_lower: Option<&str>,
    synonyms: &[&str],
) {
    // Section title line
    lines.push(HelpLine {
        line: Line::from(format!("{title}:")).underlined(),
        is_entry: false,
        matched: false,
    });

    for entry in entries {
        let matched = query_lower.is_some_and(|q| {
            let key_lc = entry.key.to_lowercase();
            let desc_lc = entry.description.to_lowercase();
            key_lc.contains(q)
                || desc_lc.contains(q)
                || synonyms
                    .iter()
                    .any(|s| key_lc.contains(s) || desc_lc.contains(s))
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

/// Collect indices of matching lines (for n/N navigation).
///
/// `current_view` and `show_all` must match what is passed to `build_help_lines`
/// so that search indices are consistent with the displayed line set.
pub fn matching_line_indices(
    query: &str,
    current_view: Option<crate::app::View>,
    show_all: bool,
) -> Vec<u16> {
    build_help_lines(Some(query), current_view, show_all)
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
/// `current_view` is the view from which Help was opened (for filtered display).
/// `show_all` toggles between current-view mode and all-views mode.
pub fn render_help_panel(
    frame: &mut Frame,
    area: Rect,
    scroll: u16,
    search_query: Option<&str>,
    search_input: Option<&str>,
    current_view: Option<crate::app::View>,
    show_all: bool,
) {
    let title_text = match (current_view, show_all) {
        (Some(v), false) => format!(" Tij - Help [{}]  (a: all views) ", section_title_for(v)),
        _ => " Tij - Help [All Views]  (a: current view) ".to_string(),
    };
    let title = Line::from(title_text).bold().white().centered();

    // Split area for input bar if searching
    let (help_area, input_area) = if search_input.is_some() {
        let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let help_lines = build_help_lines(search_query, current_view, show_all);
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
        let lines = build_help_lines(None, None, true);
        assert!(lines.iter().all(|l| !l.matched));
        assert!(!lines.is_empty());
    }

    #[test]
    fn build_help_lines_quit_matches() {
        let lines = build_help_lines(Some("quit"), None, true);
        let matched: Vec<_> = lines.iter().filter(|l| l.matched).collect();
        assert!(!matched.is_empty(), "Should match at least one Quit entry");
    }

    #[test]
    fn build_help_lines_bookmark_matches_multiple_sections() {
        let lines = build_help_lines(Some("bookmark"), None, true);
        let matched: Vec<_> = lines.iter().filter(|l| l.matched).collect();
        assert!(
            matched.len() >= 2,
            "bookmark should match in multiple sections"
        );
    }

    #[test]
    fn build_help_lines_no_match_returns_all_false() {
        let lines = build_help_lines(Some("zzzzzznonexistent"), None, true);
        assert!(lines.iter().all(|l| !l.matched));
    }

    #[test]
    fn build_help_lines_case_insensitive() {
        let upper = build_help_lines(Some("QUIT"), None, true);
        let lower = build_help_lines(Some("quit"), None, true);
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
        let indices = matching_line_indices("quit", None, true);
        assert!(!indices.is_empty());
        // Verify indices are valid
        let lines = build_help_lines(Some("quit"), None, true);
        for &idx in &indices {
            assert!(lines[idx as usize].matched);
        }
    }

    #[test]
    fn matching_line_indices_empty_for_nonexistent() {
        let indices = matching_line_indices("zzzzz", None, true);
        assert!(indices.is_empty());
    }

    #[test]
    fn build_help_lines_entries_have_is_entry_true() {
        let lines = build_help_lines(None, None, true);
        let entries: Vec<_> = lines.iter().filter(|l| l.is_entry).collect();
        assert!(entries.len() > 20, "Should have many key binding entries");
    }

    // --- Synonym expansion unit tests ---

    #[test]
    fn expand_synonyms_commit_returns_related() {
        let result = expand_synonyms("commit");
        assert!(result.contains(&"describe"), "should contain describe");
        assert!(result.contains(&"new"), "should contain new");
        assert!(result.contains(&"squash"), "should contain squash");
    }

    #[test]
    fn expand_synonyms_prefix_match() {
        let result = expand_synonyms("reb");
        assert!(result.contains(&"move"), "should contain move");
        assert!(result.contains(&"source"), "should contain source");
    }

    #[test]
    fn expand_synonyms_empty_returns_empty() {
        let result = expand_synonyms("");
        assert!(result.is_empty());
    }

    #[test]
    fn expand_synonyms_no_match_returns_empty() {
        let result = expand_synonyms("zzz");
        assert!(result.is_empty());
    }

    // --- Synonym search integration tests ---

    #[test]
    fn build_help_lines_commit_highlights_describe() {
        let lines = build_help_lines(Some("commit"), None, true);
        let matched_descs: Vec<_> = lines
            .iter()
            .filter(|l| l.matched && l.is_entry)
            .filter_map(|l| l.line.spans.get(1).map(|s| s.content.to_lowercase()))
            .collect();
        assert!(
            matched_descs.iter().any(|d| d.contains("describe")),
            "commit search should highlight Describe entry via synonyms, got: {matched_descs:?}"
        );
    }

    #[test]
    fn build_help_lines_rebase_prefix_highlights_move() {
        let lines = build_help_lines(Some("reb"), None, true);
        let matched_descs: Vec<_> = lines
            .iter()
            .filter(|l| l.matched && l.is_entry)
            .filter_map(|l| l.line.spans.get(1).map(|s| s.content.to_lowercase()))
            .collect();
        assert!(
            matched_descs.iter().any(|d| d.contains("move")),
            "reb search should highlight Move entries via rebase synonyms, got: {matched_descs:?}"
        );
    }

    #[test]
    fn build_help_lines_original_search_unaffected() {
        let lines = build_help_lines(Some("quit"), None, true);
        let matched: Vec<_> = lines.iter().filter(|l| l.matched).collect();
        assert!(
            !matched.is_empty(),
            "quit should still match via original substring search"
        );
    }

    #[test]
    fn matching_line_indices_includes_synonyms() {
        let commit_indices = matching_line_indices("commit", None, true);
        let describe_indices = matching_line_indices("describe", None, true);
        // "commit" should pick up at least one "describe" match via synonyms
        assert!(
            !describe_indices.is_empty(),
            "describe should match at least one entry"
        );
        let overlap = describe_indices
            .iter()
            .filter(|idx| commit_indices.contains(idx))
            .count();
        assert!(
            overlap > 0,
            "commit search should include at least one describe match via synonyms"
        );
    }

    // --- Current-view mode tests ---

    #[test]
    fn build_help_lines_current_view_shows_view_global_nav() {
        use crate::app::View;
        let lines = build_help_lines(None, Some(View::Log), false);
        let section_titles: Vec<String> = lines
            .iter()
            .filter(|l| !l.is_entry && !l.line.spans.is_empty())
            .filter_map(|l| {
                let txt = l.line.spans.first()?.content.to_string();
                if txt.ends_with(':') && txt != "Key bindings:" {
                    Some(txt)
                } else {
                    None
                }
            })
            .collect();
        assert!(
            section_titles.iter().any(|t| t.contains("Log View")),
            "should include Log View section"
        );
        assert!(
            section_titles.iter().any(|t| t.contains("Global")),
            "should include Global section"
        );
        assert!(
            section_titles.iter().any(|t| t.contains("Navigation")),
            "should include Navigation section"
        );
        assert!(
            !section_titles.iter().any(|t| t.contains("Diff View")),
            "should NOT include Diff View section"
        );
    }

    #[test]
    fn build_help_lines_show_all_includes_blame_resolve_evolog() {
        let lines = build_help_lines(None, None, true);
        let section_titles: Vec<String> = lines
            .iter()
            .filter(|l| !l.is_entry && !l.line.spans.is_empty())
            .filter_map(|l| {
                let txt = l.line.spans.first()?.content.to_string();
                if txt.ends_with(':') && txt != "Key bindings:" {
                    Some(txt)
                } else {
                    None
                }
            })
            .collect();
        assert!(
            section_titles.iter().any(|t| t.contains("Blame View")),
            "all-mode should include Blame View"
        );
        assert!(
            section_titles.iter().any(|t| t.contains("Resolve View")),
            "all-mode should include Resolve View"
        );
        assert!(
            section_titles.iter().any(|t| t.contains("Evolog View")),
            "all-mode should include Evolog View"
        );
    }

    #[test]
    fn build_help_lines_current_view_dedups_quit_key() {
        use crate::app::View;
        // Diff の専用配列にも `q`、Global にも `q` がある。current-view モードでは
        // first-wins dedup により `q` のエントリ行が 1 度だけ現れる（P3）。
        let lines = build_help_lines(None, Some(View::Diff), false);
        let q_count = lines
            .iter()
            .filter(|l| l.is_entry)
            .filter(|l| {
                l.line
                    .spans
                    .first()
                    .map(|s| s.content.trim() == "q")
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(q_count, 1, "q must appear exactly once after dedup");
    }

    #[test]
    fn keys_for_view_maps_evolog_and_log() {
        use crate::app::View;
        // keys_for_view routes each View to its own array (exhaustive match).
        assert_eq!(
            keys::keys_for_view(View::Evolog).len(),
            keys::EVOLOG_KEYS.len()
        );
        assert_eq!(keys::keys_for_view(View::Log).len(), keys::LOG_KEYS.len());
    }

    #[test]
    fn current_view_evolog_excludes_diff_specific_keys() {
        use crate::app::View;
        // P2: Evolog must not inherit Diff-specific keys (m/y/w).
        let lines = build_help_lines(None, Some(View::Evolog), false);
        let entry_keys: Vec<String> = lines
            .iter()
            .filter(|l| l.is_entry)
            .filter_map(|l| l.line.spans.first().map(|s| s.content.trim().to_string()))
            .collect();
        for k in ["m", "y", "w"] {
            assert!(
                !entry_keys.contains(&k.to_string()),
                "Evolog current-view help must not contain Diff key '{k}'"
            );
        }
    }

    #[test]
    fn matching_indices_within_current_view_diff() {
        use crate::app::View;
        // P1: search indices must stay within the current-view filtered line set.
        let lines = build_help_lines(Some("q"), Some(View::Diff), false);
        let indices = matching_line_indices("q", Some(View::Diff), false);
        for i in &indices {
            assert!(
                (*i as usize) < lines.len(),
                "index {i} out of current-view line bounds {}",
                lines.len()
            );
            assert!(lines[*i as usize].matched, "indexed line must be a match");
        }
    }
}
