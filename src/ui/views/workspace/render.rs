//! Workspace View rendering

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::WorkspaceView;
use crate::model::{Notification, WorkspaceInfo};
use crate::ui::{components, navigation, theme};

impl WorkspaceView {
    /// Render the workspace view with optional notification in title bar
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let count = self.workspace_count();
        let title = Line::from(format!(" Workspaces ({}) ", count))
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

        if self.workspaces.is_empty() {
            let paragraph = Paragraph::new("No workspaces found").block(block);
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
        for (idx, ws) in self.workspaces.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }
            let is_selected = idx == self.selected;
            let is_current = self.is_current(ws);
            lines.push(build_workspace_line(ws, is_selected, is_current));
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

fn build_workspace_line(ws: &WorkspaceInfo, is_selected: bool, is_current: bool) -> Line<'static> {
    let marker = if is_current { " * " } else { "   " };

    let mut spans = vec![
        Span::styled(
            marker.to_string(),
            Style::default().fg(if is_current {
                Color::Green
            } else {
                Color::DarkGray
            }),
        ),
        Span::styled(
            format!("{:<16}", truncate_str(&ws.name, 16)),
            Style::default().fg(Color::Green),
        ),
        Span::styled(
            format!("  {:<10}", ws.change_id),
            Style::default().fg(Color::Yellow),
        ),
    ];

    // Show root path if available
    if let Some(ref path) = ws.root_path {
        spans.push(Span::styled(
            format!("  {}", truncate_str(path, 30)),
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Description
    let desc = if ws.description.is_empty() {
        "(no description)"
    } else {
        &ws.description
    };
    spans.push(Span::styled(
        format!("  {}", desc),
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
