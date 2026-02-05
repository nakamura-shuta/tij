//! Rendering for ResolveView

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::model::Notification;
use crate::ui::{components, theme};

use super::ResolveView;

impl ResolveView {
    /// Render the resolve view
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let title = Line::from(format!(" Conflicts ({} files) ", self.file_count()))
            .bold()
            .red()
            .centered();

        // Build notification for title bar
        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let block = components::bordered_block_with_notification(title, notif_line);

        if self.is_empty() {
            let paragraph = components::empty_state("All conflicts resolved!", None).block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize;
        if inner_height == 0 {
            return;
        }

        // Build content lines
        let mut lines: Vec<Line> = Vec::new();

        // Header: change info
        let header = if self.is_working_copy {
            Line::from(vec![
                Span::styled("  Working copy: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    self.change_id.clone(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled("  Change: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    self.change_id.clone(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" (not working copy)", Style::default().fg(Color::Yellow)),
            ])
        };
        lines.push(header);

        // Warning for non-@ changes
        if !self.is_working_copy {
            lines.push(Line::from(Span::styled(
                "  ⚠ External merge tool not available for non-@ changes",
                Style::default().fg(Color::Yellow),
            )));
        }

        lines.push(Line::from("")); // blank line

        // File list
        let header_lines = lines.len();
        let available_for_files = inner_height.saturating_sub(header_lines + 1); // +1 for hint line
        let scroll_offset = self.calculate_scroll_offset(available_for_files);

        for (idx, file) in self.files.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height.saturating_sub(1) {
                break;
            }

            let is_selected = idx == self.selected_index;
            let marker = if is_selected { "> " } else { "  " };

            let mut spans = vec![
                Span::raw(format!("  {}", marker)),
                Span::styled(
                    "C ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ];

            // Extract N-sided count for compact display
            let sided = if let Some(n) = file.description.split('-').next() {
                format!("({}-sided)", n)
            } else {
                format!("({})", file.description)
            };

            spans.push(Span::raw(format!("{:<30} ", file.path)));
            spans.push(Span::styled(sided, Style::default().fg(Color::DarkGray)));

            let mut line = Line::from(spans);

            if is_selected {
                line = line.style(
                    Style::default()
                        .fg(theme::selection::FG)
                        .bg(theme::selection::BG)
                        .add_modifier(Modifier::BOLD),
                );
            }

            lines.push(line);
        }

        // Hint line at bottom
        let hint = if self.is_working_copy {
            Line::from(Span::styled(
                "  Hint: You can continue working with conflicts. Resolve later with 'X'.",
                Style::default().fg(Color::DarkGray),
            ))
        } else {
            Line::from(Span::styled(
                "  Hint: Only :ours / :theirs available for non-working-copy changes.",
                Style::default().fg(Color::DarkGray),
            ))
        };
        // Pad to push hint to bottom if there's space
        while lines.len() < inner_height.saturating_sub(1) {
            lines.push(Line::from(""));
        }
        lines.push(hint);

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}
