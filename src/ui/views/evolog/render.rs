//! Evolog View rendering

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::EvologView;
use crate::model::{EvologEntry, Notification};
use crate::ui::{components, navigation, theme};

impl EvologView {
    /// Render the evolog view
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let title = Line::from(format!(" Evolution Log: {} ", self.change_id))
            .bold()
            .cyan()
            .centered();

        // Build notification line for title bar
        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let block = components::bordered_block_with_notification(title, notif_line);

        if self.entries.is_empty() {
            let paragraph = Paragraph::new("No evolution history found").block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize; // borders
        if inner_height == 0 {
            return;
        }

        // Calculate scroll offset to keep selection visible
        let scroll_offset =
            navigation::adjust_scroll(self.selected, self.scroll_offset, inner_height);

        // Build lines
        let mut lines: Vec<Line> = Vec::new();
        for (idx, entry) in self.entries.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }

            let is_selected = idx == self.selected;
            let line = self.build_entry_line(entry, is_selected);
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }

    /// Build a line for an evolog entry
    fn build_entry_line(&self, entry: &EvologEntry, is_selected: bool) -> Line<'static> {
        let id_style = Style::default().fg(Color::Magenta);
        let time_style = Style::default().fg(Color::Yellow);
        let desc_style = Style::default().fg(Color::White);
        let empty_style = Style::default().fg(Color::DarkGray);

        let mut spans = vec![
            Span::styled(entry.commit_id.clone(), id_style),
            Span::raw("  "),
            Span::styled(entry.timestamp.clone(), time_style),
            Span::raw("  "),
        ];

        if entry.is_empty {
            spans.push(Span::styled("[empty] ", empty_style));
        }

        spans.push(Span::styled(entry.description.clone(), desc_style));

        let mut line = Line::from(spans);

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
