//! Rendering for BlameView

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::model::Notification;
use crate::ui::components;

use super::BlameView;

/// Constants for blame display layout
mod layout {
    /// Width for change_id display (8 chars)
    pub const CHANGE_ID_WIDTH: usize = 8;
    /// Width for author display (truncated)
    pub const AUTHOR_WIDTH: usize = 10;
    /// Width for timestamp display (MM-DD)
    pub const TIMESTAMP_WIDTH: usize = 5;
    /// Width for line number (dynamic, but max 6 digits)
    pub const LINE_NUMBER_WIDTH: usize = 6;
    /// Width for the Agent Trace AI badge column (fits "[AI?]")
    pub const AI_BADGE_WIDTH: usize = 5;
}

/// Colors for blame view
mod colors {
    use super::Color;
    use crate::ui::theme;

    pub const CHANGE_ID: Color = Color::Cyan;
    pub const AUTHOR: Color = Color::White;
    /// Timestamp color - using a lighter gray for better visibility on dark terminals
    pub const TIMESTAMP: Color = Color::Gray;
    /// Line number color - same as timestamp for consistency
    pub const LINE_NUMBER: Color = Color::Gray;
    /// Continuation marker (↑) - can be darker as it's less important
    pub const CONTINUATION: Color = Color::DarkGray;
    /// Selected line background (uses common theme)
    pub const SELECTED_BG: Color = theme::selection::BG;
    /// Selected line foreground (uses common theme)
    pub const SELECTED_FG: Color = theme::selection::FG;
}

impl BlameView {
    /// Render the blame view
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let title = format!(" Blame View: {} ", self.file_path());

        // Build title with optional notification
        let title_width = title.len();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let block = components::bordered_block_with_notification(
            Line::from(title).bold().cyan().centered(),
            notif_line,
        );

        if self.is_empty() {
            let paragraph = components::empty_state("No content to annotate", None).block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize;
        if inner_height == 0 {
            return;
        }

        // Calculate scroll offset
        let scroll_offset = self.calculate_scroll_offset(inner_height);

        // Build lines
        let mut lines: Vec<Line> = Vec::new();
        for (idx, annotation) in self.content.lines.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }

            let is_selected = idx == self.selected_index;
            let line = self.build_annotation_line(annotation, is_selected);
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }

    /// Build a single annotation line
    fn build_annotation_line(
        &self,
        annotation: &crate::model::AnnotationLine,
        is_selected: bool,
    ) -> Line<'static> {
        let mut spans = Vec::new();

        // Agent Trace AI column (Phase 4a). Reserved for the WHOLE view only
        // when at least one line matched a trace record — otherwise no column
        // is emitted and the layout is byte-identical to before (P5).
        let has_ai_overlay = !self.ai_badges.is_empty();

        if annotation.first_in_hunk {
            // Full display for first line in hunk
            // Change ID
            spans.push(Span::styled(
                format!(
                    "{:<width$}",
                    annotation.change_id,
                    width = layout::CHANGE_ID_WIDTH
                ),
                Style::default().fg(colors::CHANGE_ID),
            ));
            spans.push(Span::raw(" "));

            // AI badge column ([AI] / [AI?] / blank), keyed by line commit_id
            if has_ai_overlay {
                let commit = annotation.commit_id.as_str();
                if self.ai_badges.confirmed.contains(commit) {
                    spans.push(Span::styled(
                        format!("{:<width$} ", "[AI]", width = layout::AI_BADGE_WIDTH),
                        Style::default()
                            .fg(crate::ui::theme::log_view::AI_BADGE)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if self.ai_badges.heuristic.contains(commit) {
                    spans.push(Span::styled(
                        format!("{:<width$} ", "[AI?]", width = layout::AI_BADGE_WIDTH),
                        Style::default().fg(crate::ui::theme::log_view::AI_BADGE),
                    ));
                } else {
                    spans.push(Span::raw(" ".repeat(layout::AI_BADGE_WIDTH + 1)));
                }
            }

            // Author (truncated)
            let author = annotation.short_author(layout::AUTHOR_WIDTH);
            spans.push(Span::styled(
                format!("{:<width$}", author, width = layout::AUTHOR_WIDTH),
                Style::default().fg(colors::AUTHOR),
            ));
            spans.push(Span::raw(" "));

            // Timestamp
            let timestamp = annotation.short_timestamp();
            spans.push(Span::styled(
                format!("{:<width$}", timestamp, width = layout::TIMESTAMP_WIDTH),
                Style::default().fg(colors::TIMESTAMP),
            ));
            spans.push(Span::raw(" "));
        } else {
            // Continuation line - show "↑" indicator. Include the AI column
            // width when the overlay is active so the ↑ stays aligned and the
            // AI column shows blank (badges are hunk-head only).
            let mut continuation_width =
                layout::CHANGE_ID_WIDTH + 1 + layout::AUTHOR_WIDTH + 1 + layout::TIMESTAMP_WIDTH;
            if has_ai_overlay {
                continuation_width += layout::AI_BADGE_WIDTH + 1;
            }
            spans.push(Span::styled(
                format!("{:>width$} ", "↑", width = continuation_width),
                Style::default().fg(colors::CONTINUATION),
            ));
        }

        // Line number
        spans.push(Span::styled(
            format!(
                "{:>width$}: ",
                annotation.line_number,
                width = layout::LINE_NUMBER_WIDTH
            ),
            Style::default().fg(colors::LINE_NUMBER),
        ));

        // Content (trim trailing newline if present)
        let content = annotation.content.trim_end_matches('\n');
        spans.push(Span::raw(content.to_string()));

        let mut line = Line::from(spans);

        // Apply selection styling - use explicit fg/bg for dark terminal visibility
        if is_selected {
            line = line.style(
                Style::default()
                    .fg(colors::SELECTED_FG)
                    .bg(colors::SELECTED_BG)
                    .add_modifier(Modifier::BOLD),
            );
        }

        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AnnotationContent, AnnotationLine, ChangeId, CommitId};
    use crate::trace::AiBadgeSets;

    fn line(change: &str, commit: &str, n: usize, first_in_hunk: bool) -> AnnotationLine {
        AnnotationLine {
            change_id: ChangeId::new(change.to_string()),
            commit_id: CommitId::new(commit.to_string()),
            author: "nakamura".to_string(),
            timestamp: "2026-06-05 14:20".to_string(),
            line_number: n,
            content: format!("line {n}"),
            first_in_hunk,
        }
    }

    /// Flatten a rendered line to plain text for assertions.
    fn text(line: &Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn view_with(lines: Vec<AnnotationLine>, badges: AiBadgeSets) -> BlameView {
        let mut content = AnnotationContent::new("greet.py".to_string());
        content.lines = lines;
        let mut bv = BlameView::new();
        bv.set_content(content, None);
        bv.set_ai_badges(badges);
        bv
    }

    #[test]
    fn confirmed_line_renders_ai_badge() {
        let mut badges = AiBadgeSets::default();
        badges.confirmed.insert("c0ffee01".to_string());
        let bv = view_with(vec![line("xqnktzml", "c0ffee01", 1, true)], badges);

        let t = text(&bv.build_annotation_line(&bv.content.lines[0], false));
        assert!(t.contains("[AI]"), "got: {t}");
        assert!(!t.contains("[AI?]"), "got: {t}");
    }

    #[test]
    fn heuristic_line_renders_ai_question_badge() {
        let mut badges = AiBadgeSets::default();
        badges.heuristic.insert("c0ffee01".to_string());
        let bv = view_with(vec![line("rlxnnrwv", "c0ffee01", 1, true)], badges);

        let t = text(&bv.build_annotation_line(&bv.content.lines[0], false));
        assert!(t.contains("[AI?]"), "got: {t}");
    }

    #[test]
    fn unmatched_line_in_overlay_view_shows_blank_column() {
        // Overlay is active (one badge exists) but THIS line doesn't match →
        // blank AI column of the same width, so columns still align.
        let mut badges = AiBadgeSets::default();
        badges.confirmed.insert("c0ffee01".to_string());
        let bv = view_with(
            vec![
                line("xqnktzml", "c0ffee01", 1, true), // matches
                line("lrptplro", "b06e4c8c", 2, true), // no match
            ],
            badges,
        );

        let matched = text(&bv.build_annotation_line(&bv.content.lines[0], false));
        let unmatched = text(&bv.build_annotation_line(&bv.content.lines[1], false));
        assert!(matched.contains("[AI]"));
        assert!(!unmatched.contains("[AI"), "got: {unmatched}");
        // Same overall prefix width → line content starts at the same column.
        // Count display chars (the ↑ marker is multi-byte, so byte offsets lie).
        let col = |s: &str| s.split("line ").next().unwrap().chars().count();
        assert_eq!(col(&matched), col(&unmatched));
    }

    #[test]
    fn no_overlay_output_is_unchanged_and_has_no_ai_column() {
        // Empty badge set → AI column is not emitted at all (P5).
        let bv = view_with(
            vec![line("xqnktzml", "c0ffee01", 1, true)],
            AiBadgeSets::default(),
        );
        let t = text(&bv.build_annotation_line(&bv.content.lines[0], false));
        assert!(!t.contains("[AI"), "no AI column when no trace: {t}");
        // change_id is immediately followed by author (single space), proving
        // no reserved AI column was inserted
        assert!(t.contains("xqnktzml nakamura"), "got: {t}");
    }

    #[test]
    fn continuation_line_has_no_badge_but_stays_aligned() {
        let mut badges = AiBadgeSets::default();
        badges.confirmed.insert("c0ffee01".to_string());
        let bv = view_with(
            vec![
                line("xqnktzml", "c0ffee01", 1, true),  // hunk head
                line("xqnktzml", "c0ffee01", 2, false), // continuation
            ],
            badges,
        );

        let head = text(&bv.build_annotation_line(&bv.content.lines[0], false));
        let cont = text(&bv.build_annotation_line(&bv.content.lines[1], false));
        // Continuation shows ↑, never a badge…
        assert!(cont.contains('↑'));
        assert!(!cont.contains("[AI"), "continuation must not badge: {cont}");
        // …and the line content still starts at the same column as the head.
        // Count display chars (the ↑ marker is multi-byte, so byte offsets lie).
        let col = |s: &str| s.split("line ").next().unwrap().chars().count();
        assert_eq!(col(&head), col(&cont));
    }
}
