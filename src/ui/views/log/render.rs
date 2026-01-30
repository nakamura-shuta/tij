//! Rendering for LogView

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::jj::constants;
use crate::model::Change;
use crate::ui::{symbols, theme};

use super::{InputMode, LogView, empty_text};

impl LogView {
    /// Render the view
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Split area for input bar if in input mode
        let (log_area, input_area) = match self.input_mode {
            InputMode::Normal => (area, None),
            InputMode::SearchInput | InputMode::RevsetInput => {
                let chunks =
                    Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);
                (chunks[0], Some(chunks[1]))
            }
        };

        // Render log list
        self.render_log_list(frame, log_area);

        // Render input bar if in input mode
        if let Some(input_area) = input_area {
            self.render_input_bar(frame, input_area);
        }
    }

    fn render_log_list(&self, frame: &mut Frame, area: Rect) {
        let title = self.build_title();

        if self.changes.is_empty() {
            self.render_empty_state(frame, area, &title);
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

        let paragraph =
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title));

        frame.render_widget(paragraph, area);
    }

    fn build_title(&self) -> Line<'static> {
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

    fn render_empty_state(&self, frame: &mut Frame, area: Rect, title: &Line) {
        let empty_text = vec![
            Line::from(""),
            Line::from(empty_text::TITLE).centered(),
            Line::from(""),
            Line::from(empty_text::HINT).dark_gray().centered(),
            Line::from(""),
        ];

        let paragraph = Paragraph::new(empty_text)
            .block(Block::default().borders(Borders::ALL).title(title.clone()));

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

        // Highlight selected line
        if is_selected {
            line = line.style(
                Style::default()
                    .bg(theme::log_view::SELECTED_BG)
                    .add_modifier(Modifier::BOLD),
            );
        }

        line
    }

    #[allow(dead_code)]
    pub(crate) fn marker_for_change(&self, change: &Change) -> (char, ratatui::style::Color) {
        if change.change_id == constants::ROOT_CHANGE_ID {
            (symbols::markers::ROOT, theme::log_view::ROOT_MARKER)
        } else if change.is_working_copy {
            (
                symbols::markers::WORKING_COPY,
                theme::log_view::WORKING_COPY_MARKER,
            )
        } else {
            (symbols::markers::NORMAL, theme::log_view::NORMAL_MARKER)
        }
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
            Paragraph::new(display_text).block(Block::default().borders(Borders::ALL).title(title));

        frame.render_widget(paragraph, area);

        // Show cursor (clamped to available width, character-based)
        let cursor_pos = char_count.min(available_width);
        frame.set_cursor_position((area.x + cursor_pos as u16 + 1, area.y + 1));
    }
}
