//! Operation View rendering

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::OperationView;
use crate::model::{Notification, Operation};
use crate::ui::{components, theme};

impl OperationView {
    /// Render the operation view with optional notification in title bar
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let title = Line::from(" Operation History ").bold().cyan().centered();

        // Build notification line for title bar
        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let block = components::bordered_block_with_notification(title, notif_line);

        if self.operations.is_empty() {
            let paragraph = Paragraph::new("No operations found").block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize; // borders
        if inner_height == 0 {
            return;
        }

        // Calculate scroll offset to keep selection visible
        let scroll_offset = self.calculate_scroll_offset(inner_height);

        // Build lines
        let mut lines: Vec<Line> = Vec::new();
        for (idx, op) in self.operations.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }

            let is_selected = idx == self.selected;
            let line = self.build_operation_line(op, is_selected);
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }

    /// Calculate scroll offset to keep selection visible
    fn calculate_scroll_offset(&self, visible_height: usize) -> usize {
        if visible_height == 0 {
            return 0;
        }

        let mut offset = self.scroll_offset;

        // Ensure selected item is visible
        if self.selected < offset {
            offset = self.selected;
        } else if self.selected >= offset + visible_height {
            offset = self.selected - visible_height + 1;
        }

        offset
    }

    /// Build a line for an operation
    fn build_operation_line(&self, op: &Operation, is_selected: bool) -> Line<'static> {
        let is_current = op.is_current;

        // Build the line with styled spans
        let marker = if is_current { "@" } else { " " };
        let marker_style = if is_current {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let id_style = Style::default().fg(Color::Magenta);
        let time_style = Style::default().fg(Color::Yellow);
        let desc_style = Style::default().fg(Color::White);

        let mut line = Line::from(vec![
            Span::styled(marker.to_string(), marker_style),
            Span::raw("  "),
            Span::styled(op.short_id().to_string(), id_style),
            Span::raw("  "),
            Span::styled(op.timestamp.clone(), time_style),
            Span::raw("  "),
            Span::styled(op.description.clone(), desc_style),
        ]);

        if is_selected {
            line = line.style(
                Style::default()
                    .fg(theme::selection::FG)
                    .bg(theme::selection::BG)
                    .add_modifier(Modifier::BOLD),
            );
        }

        line
    }
}
