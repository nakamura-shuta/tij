//! Tag View rendering

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::TagView;
use crate::model::{Notification, TagInfo};
use crate::ui::{components, navigation, theme};

impl TagView {
    /// Render the tag view with optional notification in title bar
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let count = self.tag_count();
        let title = Line::from(format!(" Tags ({}) ", count))
            .bold()
            .cyan()
            .centered();

        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let block = components::bordered_block_with_notification(title, notif_line);

        if self.tags.is_empty() {
            let paragraph = Paragraph::new("No tags found").block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize;
        if inner_height == 0 {
            return;
        }

        let scroll_offset =
            navigation::adjust_scroll(self.selected, self.scroll_offset, inner_height);

        let mut lines: Vec<Line> = Vec::new();
        for (idx, tag) in self.tags.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }
            let is_selected = idx == self.selected;
            lines.push(build_tag_line(tag, is_selected));
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

fn build_tag_line(tag: &TagInfo, is_selected: bool) -> Line<'static> {
    let name = &tag.name;

    let mut spans = vec![
        Span::raw("  "),
        Span::styled(
            format!("{:<20}", truncate_str(name, 20)),
            Style::default().fg(Color::Green),
        ),
    ];

    if let Some(ref change_id) = tag.change_id {
        spans.push(Span::styled(
            format!("  {:<10}", change_id),
            Style::default().fg(Color::Yellow),
        ));
    } else {
        spans.push(Span::raw(format!("{:12}", "")));
    }

    let desc = tag.description.as_deref().unwrap_or("(no description)");
    spans.push(Span::styled(
        desc.to_string(),
        Style::default().fg(Color::White),
    ));

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

fn truncate_str(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len > 3 {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    } else {
        s.chars().take(max_len).collect()
    }
}
