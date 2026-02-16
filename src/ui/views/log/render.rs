//! Rendering for LogView

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::jj::constants;
use crate::model::{Change, Notification};
use crate::ui::{components, symbols, theme};

use super::{InputMode, LogView, RebaseMode, empty_text};

impl LogView {
    /// Render the view with optional notification in title bar
    pub fn render(&mut self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        // Split area for input bar if in input modes
        let (log_area, input_area) = match self.input_mode {
            InputMode::Normal
            | InputMode::RebaseModeSelect
            | InputMode::RebaseSelect
            | InputMode::SquashSelect
            | InputMode::CompareSelect => (area, None),
            InputMode::SearchInput
            | InputMode::RevsetInput
            | InputMode::DescribeInput
            | InputMode::BookmarkInput => {
                let chunks =
                    Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);
                (chunks[0], Some(chunks[1]))
            }
        };

        self.render_log_list(frame, log_area, notification);

        // Render input bar if in input mode
        if let Some(input_area) = input_area {
            self.render_input_bar(frame, input_area);
        }
    }

    fn render_log_list(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let title = self.build_title();

        // Build notification line for title bar (with truncation if needed)
        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize; // +4 for borders/padding
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let block = components::bordered_block_with_notification(title, notif_line);

        if self.changes.is_empty() {
            self.render_empty_state(frame, area, block);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize; // borders
        if inner_height == 0 {
            return;
        }

        // Calculate scroll offset to keep selection visible
        let scroll_offset = self.calculate_scroll_offset(inner_height);

        // Build lines - each change is one line (graph prefix from jj)
        let mut lines: Vec<Line> = Vec::new();
        for (idx, change) in self.changes.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }

            let is_selected = idx == self.selected_index && !change.is_graph_only;
            let line = self.build_change_line(change, is_selected);
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines).block(block);

        frame.render_widget(paragraph, area);
    }

    fn build_title(&self) -> Line<'static> {
        // Special title for RebaseModeSelect mode
        if self.input_mode == InputMode::RebaseModeSelect {
            return Line::from(" Tij - Log View [Rebase: Select mode (r/s/A/B)] ")
                .bold()
                .yellow()
                .centered();
        }

        // Special title for RebaseSelect mode (varies by rebase_mode)
        if self.input_mode == InputMode::RebaseSelect {
            let title = match self.rebase_mode {
                RebaseMode::Revision => " Tij - Log View [Rebase: Select destination] ".to_string(),
                RebaseMode::Source => {
                    " Tij - Log View [Rebase -s: Select destination (with descendants)] "
                        .to_string()
                }
                RebaseMode::InsertAfter => {
                    " Tij - Log View [Rebase: Select insert-after target] ".to_string()
                }
                RebaseMode::InsertBefore => {
                    " Tij - Log View [Rebase: Select insert-before target] ".to_string()
                }
            };
            return Line::from(title).bold().yellow().centered();
        }

        // Special title for SquashSelect mode
        if self.input_mode == InputMode::SquashSelect {
            return Line::from(" Tij - Log View [Squash: Select destination] ")
                .bold()
                .yellow()
                .centered();
        }

        // Special title for CompareSelect mode
        if self.input_mode == InputMode::CompareSelect {
            let from_id = self.compare_from.as_deref().unwrap_or("?");
            return Line::from(format!(
                " Tij - Log View [Compare: From={}, Select To] ",
                from_id
            ))
            .bold()
            .yellow()
            .centered();
        }

        let title_text = match (&self.current_revset, &self.last_search_query) {
            (Some(revset), Some(query)) => {
                format!(" Tij - Log View [{}] [Search: {}] ", revset, query)
            }
            (Some(revset), None) => {
                format!(" Tij - Log View [{}] ", revset)
            }
            (None, Some(query)) => {
                format!(" Tij - Log View [Search: {}] ", query)
            }
            (None, None) => " Tij - Log View ".to_string(),
        };
        Line::from(title_text).bold().cyan().centered()
    }

    fn render_empty_state(
        &self,
        frame: &mut Frame,
        area: Rect,
        block: ratatui::widgets::Block<'static>,
    ) {
        let paragraph =
            components::empty_state(empty_text::TITLE, Some(empty_text::HINT)).block(block);

        frame.render_widget(paragraph, area);
    }

    fn calculate_scroll_offset(&self, visible_changes: usize) -> usize {
        if visible_changes == 0 {
            return 0;
        }

        let mut offset = self.scroll_offset;

        // Ensure selected item is visible
        if self.selected_index < offset {
            offset = self.selected_index;
        } else if self.selected_index >= offset + visible_changes {
            offset = self.selected_index - visible_changes + 1;
        }

        offset
    }

    fn build_change_line(&self, change: &Change, is_selected: bool) -> Line<'static> {
        let mut spans = Vec::new();

        // Graph prefix (from jj output)
        if !change.graph_prefix.is_empty() {
            spans.push(Span::styled(
                change.graph_prefix.clone(),
                Style::default().fg(theme::log_view::GRAPH_LINE),
            ));
        }

        // For graph-only lines, just return the prefix
        if change.is_graph_only {
            return Line::from(spans);
        }

        // Change ID
        spans.push(Span::styled(
            format!("{} ", change.short_id()),
            Style::default().fg(theme::log_view::CHANGE_ID),
        ));

        // Author (if not root)
        if change.change_id != constants::ROOT_CHANGE_ID {
            spans.push(Span::raw(format!("{} ", change.author)));
            spans.push(Span::styled(
                format!("{} ", change.timestamp),
                Style::default().fg(theme::log_view::TIMESTAMP),
            ));
        }

        // Bookmarks
        if !change.bookmarks.is_empty() {
            spans.push(Span::styled(
                format!("{} ", change.bookmarks.join(", ")),
                Style::default().fg(theme::log_view::BOOKMARK),
            ));
        }

        // Conflict indicator
        if change.has_conflict {
            spans.push(Span::styled(
                "[CONFLICT] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        }

        // Description
        let description = change.display_description();
        if change.is_empty && description == symbols::empty::NO_DESCRIPTION {
            spans.push(Span::styled(
                format!("{} ", symbols::empty::CHANGE_LABEL),
                Style::default().fg(theme::log_view::EMPTY_LABEL),
            ));
        }
        spans.push(Span::raw(description.to_string()));

        let mut line = Line::from(spans);

        // Check if this is the rebase source (in RebaseModeSelect or RebaseSelect mode)
        let is_rebase_source = matches!(
            self.input_mode,
            InputMode::RebaseModeSelect | InputMode::RebaseSelect
        ) && self.rebase_source.as_ref() == Some(&change.change_id);

        // Check if this is the squash source (in SquashSelect mode)
        let is_squash_source = self.input_mode == InputMode::SquashSelect
            && self.squash_source.as_ref() == Some(&change.change_id);

        // Check if this is the compare "from" (in CompareSelect mode)
        let is_compare_from = self.input_mode == InputMode::CompareSelect
            && self.compare_from.as_ref() == Some(&change.change_id);

        // Apply styling
        if is_rebase_source || is_squash_source || is_compare_from {
            // Highlight rebase/squash source with distinct background
            line = line.style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        } else if is_selected {
            line = line.style(
                Style::default()
                    .fg(theme::selection::FG)
                    .bg(theme::selection::BG)
                    .add_modifier(Modifier::BOLD),
            );
        }

        line
    }

    fn render_input_bar(&self, frame: &mut Frame, area: Rect) {
        let Some((prompt, title)) = self.input_mode.input_bar_meta() else {
            return;
        };

        let input_text = format!("{}{}", prompt, self.input_buffer);

        // Calculate available width (area width minus borders)
        let available_width = area.width.saturating_sub(2) as usize;

        // Early return if no space for input
        if available_width == 0 {
            return;
        }

        // Truncate display text if too long (show end of input, UTF-8 safe)
        let char_count = input_text.chars().count();
        let display_text = if char_count > available_width {
            let skip = char_count.saturating_sub(available_width.saturating_sub(1)); // -1 for ellipsis
            format!("…{}", input_text.chars().skip(skip).collect::<String>())
        } else {
            input_text.clone()
        };

        let paragraph =
            Paragraph::new(display_text).block(components::bordered_block(Line::from(title)));

        frame.render_widget(paragraph, area);

        // Show cursor (clamped to available width, character-based)
        let cursor_pos = char_count.min(available_width);
        frame.set_cursor_position((area.x + cursor_pos as u16 + 1, area.y + 1));
    }
}
