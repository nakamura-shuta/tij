//! Rendering for DiffView

use ratatui::{
    prelude::*,
    style::Stylize,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::model::{CompareInfo, DiffLine, DiffLineKind, Notification};
use crate::ui::{components, theme};

use super::DiffView;

impl DiffView {
    /// Render the diff view (without status bar - rendered by App)
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        if self.compare_info.is_some() {
            self.render_compare(frame, area, notification);
        } else {
            self.render_normal(frame, area, notification);
        }
    }

    /// Render normal single-revision diff
    fn render_normal(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        // Layout: header (dynamic) + context bar (1) + diff (rest)
        // Header height = 1 (top border) + 2 (commit, author) + description lines
        // Cap so context bar (1) + diff (1) always have space
        let desc_lines = self.description_line_count();
        let max_header = area.height.saturating_sub(2); // reserve context bar + min diff
        let header_height = ((1 + 2 + desc_lines) as u16).min(max_header).max(1);
        let chunks = Layout::vertical([
            Constraint::Length(header_height), // Header (commit, author, description)
            Constraint::Length(1),             // Context bar
            Constraint::Min(1),                // Diff content
        ])
        .split(area);

        self.render_header(frame, chunks[0], notification);
        self.render_context_bar(frame, chunks[1]);
        self.render_diff_content(frame, chunks[2]);
    }

    /// Render compare (two-revision) diff
    fn render_compare(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let compare_info = self.compare_info.as_ref().unwrap();

        // Layout: compare header (5 lines: border + from + to + summary + border) + context bar (1) + diff (rest)
        let header_height = 5_u16.min(area.height.saturating_sub(2));
        let chunks = Layout::vertical([
            Constraint::Length(header_height),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

        self.render_compare_header(frame, chunks[0], compare_info, notification);
        self.render_context_bar(frame, chunks[1]);
        self.render_diff_content(frame, chunks[2]);
    }

    /// Render the header (commit info including description)
    fn render_header(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let title = Line::from(vec![
            Span::raw(" Tij - Diff View ").bold(),
            Span::raw("["),
            Span::styled(
                self.change_id.chars().take(8).collect::<String>(),
                Style::default().fg(theme::log_view::CHANGE_ID),
            ),
            Span::raw("] "),
        ])
        .centered();

        // Build notification line for title bar (right-aligned)
        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let mut header_text = vec![
            Line::from(vec![
                Span::raw("Commit: "),
                Span::styled(
                    self.content.commit_id.chars().take(40).collect::<String>(),
                    Style::default().fg(theme::log_view::CHANGE_ID),
                ),
            ]),
            Line::from(vec![
                Span::raw("Author: "),
                Span::raw(&self.content.author),
                Span::raw("  "),
                Span::styled(
                    &self.content.timestamp,
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ];

        // Show full description (all lines)
        if self.content.description.is_empty() {
            header_text.push(Line::from(vec![Span::styled(
                "(no description)",
                Style::default().fg(Color::DarkGray).italic(),
            )]));
        } else {
            for line in self.content.description.lines() {
                header_text.push(Line::from(vec![Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::White).bold(),
                )]));
            }
        }

        // Use header_block with notification on right
        let block = if let Some(notif) = notif_line {
            components::header_block(title).title(notif.right_aligned())
        } else {
            components::header_block(title)
        };

        let header = Paragraph::new(header_text).block(block);

        frame.render_widget(header, area);
    }

    /// Render compare header with From/To revision info
    fn render_compare_header(
        &self,
        frame: &mut Frame,
        area: Rect,
        compare_info: &CompareInfo,
        notification: Option<&Notification>,
    ) {
        let title = Line::from(" Tij - Compare Diff ").bold().cyan().centered();

        // Build notification for title bar
        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        // Build from/to lines
        let from = &compare_info.from;
        let to = &compare_info.to;

        let from_bookmarks = if from.bookmarks.is_empty() {
            String::new()
        } else {
            format!(" ({})", from.bookmarks.join(", "))
        };
        let to_bookmarks = if to.bookmarks.is_empty() {
            String::new()
        } else {
            format!(" ({})", to.bookmarks.join(", "))
        };

        let from_desc = if from.description.is_empty() {
            "(no description)".to_string()
        } else {
            from.description.clone()
        };
        let to_desc = if to.description.is_empty() {
            "(no description)".to_string()
        } else {
            to.description.clone()
        };

        let header_text = vec![
            Line::from(vec![
                Span::styled("From: ", Style::default().fg(Color::Red).bold()),
                Span::styled(
                    from.change_id.clone(),
                    Style::default().fg(theme::log_view::CHANGE_ID),
                ),
                Span::styled(from_bookmarks, Style::default().fg(Color::Magenta)),
                Span::raw(format!(" {} ", from.author)),
                Span::raw(from_desc),
            ]),
            Line::from(vec![
                Span::styled("To:   ", Style::default().fg(Color::Green).bold()),
                Span::styled(
                    to.change_id.clone(),
                    Style::default().fg(theme::log_view::CHANGE_ID),
                ),
                Span::styled(to_bookmarks, Style::default().fg(Color::Magenta)),
                Span::raw(format!(" {} ", to.author)),
                Span::raw(to_desc),
            ]),
            Line::from(vec![Span::styled(
                format!("{} file(s) changed", self.file_count()),
                Style::default().fg(Color::DarkGray),
            )]),
        ];

        let block = if let Some(notif) = notif_line {
            components::header_block(title).title(notif.right_aligned())
        } else {
            components::header_block(title)
        };

        let header = Paragraph::new(header_text).block(block);
        frame.render_widget(header, area);
    }

    /// Render the context bar (current file name + progress)
    fn render_context_bar(&self, frame: &mut Frame, area: Rect) {
        let file_info = if self.file_count() > 0 {
            let file_name = self.current_file_name().unwrap_or("(unknown)");
            format!(
                " {} [{}/{}]",
                file_name,
                self.current_file_index + 1,
                self.file_count()
            )
        } else {
            " (no files)".to_string()
        };

        let bar = Paragraph::new(Line::from(vec![Span::styled(
            file_info,
            Style::default().fg(Color::Cyan).bold(),
        )]))
        .block(components::side_borders_block());

        frame.render_widget(bar, area);
    }

    /// Render the diff content (scrollable)
    fn render_diff_content(&self, frame: &mut Frame, area: Rect) {
        // No top/bottom borders, only left/right, so use full height
        let inner_height = area.height as usize;

        if !self.has_changes() {
            // Empty state
            let empty_msg = components::no_changes_state().block(components::side_borders_block());
            frame.render_widget(empty_msg, area);
            return;
        }

        // Build visible lines
        let lines: Vec<Line> = self
            .content
            .lines
            .iter()
            .skip(self.scroll_offset)
            .take(inner_height)
            .map(|diff_line| self.render_diff_line(diff_line))
            .collect();

        let diff = Paragraph::new(lines).block(components::side_borders_block());

        frame.render_widget(diff, area);
    }

    /// Render a single diff line
    fn render_diff_line(&self, line: &DiffLine) -> Line<'static> {
        match line.kind {
            DiffLineKind::FileHeader => Line::from(Span::styled(
                format!("── {} ──", line.content),
                Style::default().fg(theme::diff_view::FILE_HEADER).bold(),
            )),
            DiffLineKind::Separator => Line::from(""),
            DiffLineKind::Context => {
                let line_nums = self.format_line_numbers(line.line_numbers);
                Line::from(vec![
                    Span::styled(
                        line_nums,
                        Style::default().fg(theme::diff_view::LINE_NUMBER),
                    ),
                    Span::raw("  "),
                    Span::raw(line.content.clone()),
                ])
            }
            DiffLineKind::Added => {
                let line_nums = self.format_line_numbers(line.line_numbers);
                Line::from(vec![
                    Span::styled(
                        line_nums,
                        Style::default().fg(theme::diff_view::LINE_NUMBER),
                    ),
                    Span::styled(" +", Style::default().fg(theme::diff_view::ADDED)),
                    Span::styled(
                        line.content.clone(),
                        Style::default().fg(theme::diff_view::ADDED),
                    ),
                ])
            }
            DiffLineKind::Deleted => {
                let line_nums = self.format_line_numbers(line.line_numbers);
                Line::from(vec![
                    Span::styled(
                        line_nums,
                        Style::default().fg(theme::diff_view::LINE_NUMBER),
                    ),
                    Span::styled(" -", Style::default().fg(theme::diff_view::DELETED)),
                    Span::styled(
                        line.content.clone(),
                        Style::default().fg(theme::diff_view::DELETED),
                    ),
                ])
            }
        }
    }

    /// Format line numbers for display
    fn format_line_numbers(&self, line_nums: Option<(Option<usize>, Option<usize>)>) -> String {
        match line_nums {
            Some((old, new)) => {
                let old_str = old
                    .map(|n| format!("{:4}", n))
                    .unwrap_or_else(|| "    ".to_string());
                let new_str = new
                    .map(|n| format!("{:4}", n))
                    .unwrap_or_else(|| "    ".to_string());
                format!("{} {}", old_str, new_str)
            }
            None => "         ".to_string(),
        }
    }
}
