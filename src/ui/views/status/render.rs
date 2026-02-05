//! Status View rendering

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{StatusInputMode, StatusView};
use crate::model::{FileState, Notification, Status};
use crate::ui::{components, theme};

impl StatusView {
    /// Render the view with optional notification in title bar
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        // Split area for input bar if in input mode
        let (status_area, input_area) = match self.input_mode {
            StatusInputMode::Normal => (area, None),
            StatusInputMode::CommitInput => {
                let chunks =
                    Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);
                (chunks[0], Some(chunks[1]))
            }
        };

        let title = Line::from(" Tij - Status View ")
            .style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            )
            .centered();

        // Build notification line for title bar
        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let block = components::bordered_block_with_notification(title.clone(), notif_line);

        match &self.status {
            None => {
                // Loading state
                let content = components::empty_state("Loading...", None).block(block);
                frame.render_widget(content, status_area);
            }
            Some(status) if status.is_clean() => {
                // Clean state
                let content =
                    components::empty_state("Working copy is clean.", Some("No modified files."))
                        .block(block);
                frame.render_widget(content, status_area);
            }
            Some(status) => {
                // Has changes
                self.render_file_list(frame, status_area, status, &title, notification);
            }
        }

        // Render input bar if in input mode
        if let Some(input_area) = input_area {
            self.render_input_bar(frame, input_area);
        }
    }

    /// Render the commit input bar
    fn render_input_bar(&self, frame: &mut Frame, area: Rect) {
        let prompt = "Commit message: ";
        let input_text = format!("{}{}", prompt, self.input_buffer);

        // Calculate available width (area width minus borders)
        let available_width = area.width.saturating_sub(2) as usize;

        if available_width == 0 {
            return;
        }

        // Truncate display text if too long (show end of input, UTF-8 safe)
        let char_count = input_text.chars().count();
        let display_text = if char_count > available_width {
            let skip = char_count.saturating_sub(available_width.saturating_sub(1));
            format!("…{}", input_text.chars().skip(skip).collect::<String>())
        } else {
            input_text.clone()
        };

        // Title with key hints
        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled("[Enter]", Style::default().fg(theme::status_view::ADDED)),
            Span::raw(" Save  "),
            Span::styled("[Esc]", Style::default().fg(theme::status_view::DELETED)),
            Span::raw(" Cancel "),
        ]);

        let paragraph = Paragraph::new(display_text).block(components::bordered_block(title));

        frame.render_widget(paragraph, area);

        // Show cursor
        let cursor_pos = char_count.min(available_width);
        frame.set_cursor_position((area.x + cursor_pos as u16 + 1, area.y + 1));
    }

    /// Render the file list
    fn render_file_list(
        &self,
        frame: &mut Frame,
        area: Rect,
        status: &Status,
        title: &Line,
        notification: Option<&Notification>,
    ) {
        // Count conflict files for header
        let conflict_count = status
            .files
            .iter()
            .filter(|f| matches!(f.state, FileState::Conflicted))
            .count();
        let has_conflict_line = conflict_count > 0;

        // Calculate available height for files (minus borders and header)
        // 2 borders + 3 header lines (+ 1 if conflict line shown)
        let header_lines = if has_conflict_line { 4 } else { 3 };
        let inner_height = area.height.saturating_sub(2 + header_lines as u16) as usize;

        // Build lines
        let mut lines = Vec::new();

        // Header: Working copy and Parent info
        lines.push(Line::from(vec![
            Span::styled(
                " Working copy: ",
                Style::default().fg(theme::status_view::HEADER),
            ),
            Span::raw(&status.working_copy_change_id),
        ]));
        lines.push(Line::from(vec![
            Span::styled(
                " Parent:       ",
                Style::default().fg(theme::status_view::HEADER),
            ),
            Span::raw(&status.parent_change_id),
        ]));

        // Conflict count header (only when conflicts exist)
        if has_conflict_line {
            let label = if conflict_count == 1 {
                " Conflicts: 1 file".to_string()
            } else {
                format!(" Conflicts: {} files", conflict_count)
            };
            lines.push(Line::from(vec![Span::styled(
                label,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]));
        }

        lines.push(Line::from("")); // Separator

        // File list
        let header_count = header_lines + 1; // +1 for separator
        for (idx, file) in status.files.iter().enumerate().skip(self.scroll_offset) {
            if lines.len() >= inner_height + header_count {
                break;
            }

            let is_selected = idx == self.selected_index;
            let line = self.build_file_line(file, is_selected);
            lines.push(line);
        }

        // Build notification line for title bar
        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let block = components::bordered_block_with_notification(title.clone(), notif_line);
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }

    /// Build a line for a file entry
    fn build_file_line(&self, file: &crate::model::FileStatus, is_selected: bool) -> Line<'static> {
        let indicator = file.indicator();
        let color = match &file.state {
            FileState::Added => theme::status_view::ADDED,
            FileState::Modified => theme::status_view::MODIFIED,
            FileState::Deleted => theme::status_view::DELETED,
            FileState::Renamed { .. } => theme::status_view::RENAMED,
            FileState::Conflicted => theme::status_view::CONFLICTED,
        };

        let mut spans = vec![
            Span::raw(if is_selected { " > " } else { "   " }),
            Span::styled(format!("{} ", indicator), Style::default().fg(color)),
        ];

        // File path
        match &file.state {
            FileState::Renamed { from } => {
                spans.push(Span::raw(format!("{} -> {}", from, file.path)));
            }
            _ => {
                spans.push(Span::raw(file.path.clone()));
            }
        }

        // Conflict indicator
        if matches!(file.state, FileState::Conflicted) {
            spans.push(Span::styled(
                " [conflict]",
                Style::default()
                    .fg(theme::status_view::CONFLICTED)
                    .add_modifier(Modifier::BOLD),
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
}
