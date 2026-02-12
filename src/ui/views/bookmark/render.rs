//! Bookmark View rendering

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{BookmarkView, DisplayRow};
use crate::model::{BookmarkInfo, Notification};
use crate::ui::{components, navigation, theme};

impl BookmarkView {
    /// Render the bookmark view with optional notification in title bar
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let count = self.bookmark_count();
        let title = Line::from(format!(" Bookmarks ({}) ", count))
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

        if self.bookmarks.is_empty() {
            let paragraph = Paragraph::new("No bookmarks found").block(block);
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
        for (idx, row) in self.display_rows.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }
            let is_selected = idx == self.selected;
            let line = match row {
                DisplayRow::Header(text) => build_header_line(text),
                DisplayRow::Bookmark(bm_idx) => {
                    build_bookmark_line(&self.bookmarks[*bm_idx], is_selected)
                }
            };
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

fn build_header_line(text: &str) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!("  {}", text),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )])
}

fn build_bookmark_line(info: &BookmarkInfo, is_selected: bool) -> Line<'static> {
    let is_local = info.bookmark.remote.is_none();
    let is_untracked = info.bookmark.is_untracked_remote();

    let name = info.bookmark.full_name();
    let name_color = if is_untracked {
        Color::Yellow
    } else if is_local {
        Color::White
    } else {
        Color::DarkGray
    };

    let mut spans = vec![
        Span::raw("  "),
        Span::styled(
            format!("{:<30}", truncate_str(&name, 30)),
            Style::default().fg(name_color),
        ),
    ];

    if is_local {
        if let Some(ref change_id) = info.change_id {
            spans.push(Span::styled(
                format!("  {:<10}", change_id),
                Style::default().fg(Color::Yellow),
            ));
        } else {
            spans.push(Span::raw(format!("{:12}", "")));
        }
        let desc = info.description.as_deref().unwrap_or("(no description)");
        spans.push(Span::styled(
            desc.to_string(),
            Style::default().fg(Color::White),
        ));
    }

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
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}
