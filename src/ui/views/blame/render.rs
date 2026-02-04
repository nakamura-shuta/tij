//! Rendering for BlameView

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::ui::components;

use super::BlameView;

/// Constants for blame display layout
mod layout {
    /// Width for change_id display (8 chars)
    pub const CHANGE_ID_WIDTH: usize = 8;
    /// Width for author display (truncated)
    pub const AUTHOR_WIDTH: usize = 10;
    /// Width for timestamp display (MM-DD)
    pub const TIMESTAMP_WIDTH: usize = 5;
    /// Width for line number (dynamic, but max 6 digits)
    pub const LINE_NUMBER_WIDTH: usize = 6;
    /// Total metadata width (excluding content)
    pub const METADATA_WIDTH: usize =
        CHANGE_ID_WIDTH + 1 + AUTHOR_WIDTH + 1 + TIMESTAMP_WIDTH + 1 + LINE_NUMBER_WIDTH + 2;
}

/// Colors for blame view
mod colors {
    use super::Color;
    use crate::ui::theme;

    pub const CHANGE_ID: Color = Color::Cyan;
    pub const AUTHOR: Color = Color::White;
    /// Timestamp color - using a lighter gray for better visibility on dark terminals
    pub const TIMESTAMP: Color = Color::Gray;
    /// Line number color - same as timestamp for consistency
    pub const LINE_NUMBER: Color = Color::Gray;
    /// Continuation marker (↑) - can be darker as it's less important
    pub const CONTINUATION: Color = Color::DarkGray;
    /// Selected line background (uses common theme)
    pub const SELECTED_BG: Color = theme::selection::BG;
    /// Selected line foreground (uses common theme)
    pub const SELECTED_FG: Color = theme::selection::FG;
}

impl BlameView {
    /// Render the blame view
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let title = format!(" Blame View: {} ", self.file_path());
        let block = components::bordered_block(Line::from(title).bold().cyan().centered());

        if self.is_empty() {
            let paragraph = components::empty_state("No content to annotate", None).block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize;
        if inner_height == 0 {
            return;
        }

        // Calculate scroll offset
        let scroll_offset = self.calculate_scroll_offset(inner_height);

        // Build lines
        let mut lines: Vec<Line> = Vec::new();
        for (idx, annotation) in self.content.lines.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }

            let is_selected = idx == self.selected_index;
            let line = self.build_annotation_line(annotation, is_selected);
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }

    /// Build a single annotation line
    fn build_annotation_line(
        &self,
        annotation: &crate::model::AnnotationLine,
        is_selected: bool,
    ) -> Line<'static> {
        let mut spans = Vec::new();

        if annotation.first_in_hunk {
            // Full display for first line in hunk
            // Change ID
            spans.push(Span::styled(
                format!(
                    "{:<width$}",
                    annotation.change_id,
                    width = layout::CHANGE_ID_WIDTH
                ),
                Style::default().fg(colors::CHANGE_ID),
            ));
            spans.push(Span::raw(" "));

            // Author (truncated)
            let author = annotation.short_author(layout::AUTHOR_WIDTH);
            spans.push(Span::styled(
                format!("{:<width$}", author, width = layout::AUTHOR_WIDTH),
                Style::default().fg(colors::AUTHOR),
            ));
            spans.push(Span::raw(" "));

            // Timestamp
            let timestamp = annotation.short_timestamp();
            spans.push(Span::styled(
                format!("{:<width$}", timestamp, width = layout::TIMESTAMP_WIDTH),
                Style::default().fg(colors::TIMESTAMP),
            ));
            spans.push(Span::raw(" "));
        } else {
            // Continuation line - show "↑" indicator
            let continuation_width =
                layout::CHANGE_ID_WIDTH + 1 + layout::AUTHOR_WIDTH + 1 + layout::TIMESTAMP_WIDTH;
            spans.push(Span::styled(
                format!("{:>width$} ", "↑", width = continuation_width),
                Style::default().fg(colors::CONTINUATION),
            ));
        }

        // Line number
        spans.push(Span::styled(
            format!(
                "{:>width$}: ",
                annotation.line_number,
                width = layout::LINE_NUMBER_WIDTH
            ),
            Style::default().fg(colors::LINE_NUMBER),
        ));

        // Content (trim trailing newline if present)
        let content = annotation.content.trim_end_matches('\n');
        spans.push(Span::raw(content.to_string()));

        let mut line = Line::from(spans);

        // Apply selection styling - use explicit fg/bg for dark terminal visibility
        if is_selected {
            line = line.style(
                Style::default()
                    .fg(colors::SELECTED_FG)
                    .bg(colors::SELECTED_BG)
                    .add_modifier(Modifier::BOLD),
            );
        }

        line
    }
}
