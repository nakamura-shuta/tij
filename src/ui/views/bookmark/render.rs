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

        // Render rename input bar at the bottom if active
        if let Some(ref state) = self.rename_state {
            let input_area = Rect {
                x: area.x,
                y: area.y + area.height.saturating_sub(3),
                width: area.width,
                height: 3.min(area.height),
            };
            let input_text = format!("Rename bookmark: {}", state.input_buffer);
            let input_line = Line::from(vec![
                Span::styled("Rename bookmark: ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    state.input_buffer.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("█", Style::default().fg(Color::White)),
            ]);
            let hint_line = Line::from(vec![
                Span::styled("[Enter]", Style::default().fg(Color::Green)),
                Span::raw(" Confirm  "),
                Span::styled("[Esc]", Style::default().fg(Color::Red)),
                Span::raw(" Cancel"),
            ]);
            let input_block = ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray));
            let _ = input_text; // suppress unused warning
            let input_paragraph = Paragraph::new(vec![input_line, hint_line]).block(input_block);
            frame.render_widget(input_paragraph, input_area);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_str_short_string_unchanged() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_str_exact_length_unchanged() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn truncate_str_long_string_adds_ellipsis() {
        assert_eq!(truncate_str("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_str_max_len_3_no_ellipsis() {
        assert_eq!(truncate_str("hello", 3), "hel");
    }

    #[test]
    fn truncate_str_multibyte_japanese() {
        // Japanese characters are 3 bytes each in UTF-8
        let s = "ブックマーク名前テスト";
        // 11 chars, truncate to 8 → 5 chars + "..."
        let result = truncate_str(s, 8);
        assert_eq!(result, "ブックマー...");
        assert_eq!(result.chars().count(), 8);
    }

    #[test]
    fn truncate_str_multibyte_exact_fit() {
        let s = "日本語";
        assert_eq!(truncate_str(s, 3), "日本語");
    }

    #[test]
    fn truncate_str_emoji() {
        let s = "feat-🚀-rocket-launch";
        let result = truncate_str(s, 10);
        assert_eq!(result.chars().count(), 10);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn truncate_str_empty_string() {
        assert_eq!(truncate_str("", 10), "");
    }
}
