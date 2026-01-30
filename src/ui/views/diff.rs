//! Diff View
//!
//! Displays the diff for a selected change from the log view.

use crossterm::event::KeyEvent;
use ratatui::{
    prelude::*,
    style::Stylize,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::keys;
use crate::model::{DiffContent, DiffLine, DiffLineKind};
use crate::ui::theme;

/// Action returned by DiffView key handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffAction {
    /// No action needed
    None,
    /// Return to log view
    Back,
}

/// Diff view state
#[derive(Debug)]
pub struct DiffView {
    /// Change ID being displayed
    pub change_id: String,
    /// Parsed diff content
    pub content: DiffContent,
    /// Scroll offset (line index)
    pub scroll_offset: usize,
    /// Positions of file headers in the lines array
    pub file_header_positions: Vec<usize>,
    /// File names (extracted from headers)
    pub file_names: Vec<String>,
    /// Current file index (for context bar)
    pub current_file_index: usize,
    /// Last known visible height (updated during render)
    visible_height: usize,
}

impl Default for DiffView {
    fn default() -> Self {
        Self::empty()
    }
}

impl DiffView {
    /// Create a new empty DiffView
    pub fn empty() -> Self {
        Self {
            change_id: String::new(),
            content: DiffContent::default(),
            scroll_offset: 0,
            file_header_positions: Vec::new(),
            file_names: Vec::new(),
            current_file_index: 0,
            visible_height: Self::DEFAULT_VISIBLE_HEIGHT,
        }
    }

    /// Create a new DiffView with content
    pub fn new(change_id: String, content: DiffContent) -> Self {
        let mut view = Self::empty();
        view.set_content(change_id, content);
        view
    }

    /// Set the content to display
    pub fn set_content(&mut self, change_id: String, content: DiffContent) {
        // Extract file header positions and names
        let (positions, names): (Vec<_>, Vec<_>) = content
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.kind == DiffLineKind::FileHeader)
            .map(|(i, line)| (i, line.content.clone()))
            .unzip();

        self.file_header_positions = positions;
        self.file_names = names;
        self.change_id = change_id;
        self.content = content;
        self.scroll_offset = 0;
        self.current_file_index = 0;
    }

    /// Clear the view
    pub fn clear(&mut self) {
        self.change_id.clear();
        self.content = DiffContent::default();
        self.scroll_offset = 0;
        self.file_header_positions.clear();
        self.file_names.clear();
        self.current_file_index = 0;
        self.visible_height = Self::DEFAULT_VISIBLE_HEIGHT;
    }

    /// Get current file name for context bar
    pub fn current_file_name(&self) -> Option<&str> {
        self.file_names
            .get(self.current_file_index)
            .map(|s| s.as_str())
    }

    /// Get total file count
    pub fn file_count(&self) -> usize {
        self.file_names.len()
    }

    /// Check if there are any changes to display
    pub fn has_changes(&self) -> bool {
        self.content.has_changes()
    }

    /// Total number of diff lines
    pub fn total_lines(&self) -> usize {
        self.content.lines.len()
    }

    /// Get current context string for status bar
    pub fn current_context(&self) -> String {
        if self.file_count() > 0 {
            let file_name = self.current_file_name().unwrap_or("(unknown)");
            format!(
                "{} [{}/{}]",
                file_name,
                self.current_file_index + 1,
                self.file_count()
            )
        } else {
            "(no files)".to_string()
        }
    }

    // =========================================================================
    // Navigation
    // =========================================================================

    /// Scroll up by one line
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
        self.update_current_file_index();
    }

    /// Scroll down by one line
    pub fn scroll_down(&mut self) {
        let max_offset = self.max_scroll_offset();
        if self.scroll_offset < max_offset {
            self.scroll_offset += 1;
        }
        self.update_current_file_index();
    }

    /// Calculate maximum scroll offset based on visible height
    fn max_scroll_offset(&self) -> usize {
        // If visible_height is 0, don't allow scrolling
        if self.visible_height == 0 {
            return 0;
        }
        let total = self.total_lines();
        total.saturating_sub(self.visible_height)
    }

    /// Scroll up by half page
    pub fn scroll_half_page_up(&mut self, visible_height: usize) {
        self.visible_height = visible_height;
        let half = visible_height / 2;
        self.scroll_offset = self.scroll_offset.saturating_sub(half);
        self.update_current_file_index();
    }

    /// Scroll down by half page
    pub fn scroll_half_page_down(&mut self, visible_height: usize) {
        self.visible_height = visible_height;
        let half = visible_height / 2;
        let max_offset = self.max_scroll_offset();
        self.scroll_offset = (self.scroll_offset + half).min(max_offset);
        self.update_current_file_index();
    }

    /// Jump to the top
    pub fn jump_to_top(&mut self) {
        self.scroll_offset = 0;
        self.current_file_index = 0;
    }

    /// Jump to the bottom
    pub fn jump_to_bottom(&mut self, visible_height: usize) {
        self.visible_height = visible_height;
        self.scroll_offset = self.max_scroll_offset();
        self.update_current_file_index();
    }

    /// Jump to the next file
    pub fn next_file(&mut self) {
        if self.file_header_positions.is_empty() {
            return;
        }

        // Find the next file header position after current scroll
        for (i, &pos) in self.file_header_positions.iter().enumerate() {
            if pos > self.scroll_offset {
                self.scroll_offset = pos;
                self.current_file_index = i;
                return;
            }
        }

        // Wrap around to first file
        if let Some(&first_pos) = self.file_header_positions.first() {
            self.scroll_offset = first_pos;
            self.current_file_index = 0;
        }
    }

    /// Jump to the previous file
    pub fn prev_file(&mut self) {
        if self.file_header_positions.is_empty() {
            return;
        }

        // Find the previous file header position before current scroll
        for (i, &pos) in self.file_header_positions.iter().enumerate().rev() {
            if pos < self.scroll_offset {
                self.scroll_offset = pos;
                self.current_file_index = i;
                return;
            }
        }

        // Wrap around to last file
        if let Some(&last_pos) = self.file_header_positions.last() {
            self.scroll_offset = last_pos;
            self.current_file_index = self.file_header_positions.len() - 1;
        }
    }

    /// Update current_file_index based on scroll position
    fn update_current_file_index(&mut self) {
        self.current_file_index = self
            .file_header_positions
            .iter()
            .rposition(|&pos| pos <= self.scroll_offset)
            .unwrap_or(0);
    }

    // =========================================================================
    // Key handling
    // =========================================================================

    /// Default visible height for scroll calculations when not specified
    const DEFAULT_VISIBLE_HEIGHT: usize = 20;

    /// Handle key input
    pub fn handle_key(&mut self, key: KeyEvent) -> DiffAction {
        self.handle_key_with_height(key, Self::DEFAULT_VISIBLE_HEIGHT)
    }

    /// Handle key input with explicit visible height
    pub fn handle_key_with_height(&mut self, key: KeyEvent, visible_height: usize) -> DiffAction {
        // Always update visible_height to ensure accurate scroll bounds
        self.visible_height = visible_height;

        match key.code {
            keys::MOVE_DOWN => {
                self.scroll_down();
                DiffAction::None
            }
            keys::MOVE_UP => {
                self.scroll_up();
                DiffAction::None
            }
            keys::HALF_PAGE_DOWN => {
                self.scroll_half_page_down(visible_height);
                DiffAction::None
            }
            keys::HALF_PAGE_UP => {
                self.scroll_half_page_up(visible_height);
                DiffAction::None
            }
            keys::GO_TOP => {
                self.jump_to_top();
                DiffAction::None
            }
            keys::GO_BOTTOM => {
                self.jump_to_bottom(visible_height);
                DiffAction::None
            }
            keys::NEXT_FILE => {
                self.next_file();
                DiffAction::None
            }
            keys::PREV_FILE => {
                self.prev_file();
                DiffAction::None
            }
            keys::QUIT | keys::ESC => DiffAction::Back,
            _ => DiffAction::None,
        }
    }

    // =========================================================================
    // Rendering
    // =========================================================================

    /// Render the diff view (without status bar - rendered by App)
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Layout: header (4) + context bar (1) + diff (rest)
        // Header increased to 4 lines to include description
        let chunks = Layout::vertical([
            Constraint::Length(4), // Header (commit, author, description)
            Constraint::Length(1), // Context bar
            Constraint::Min(1),    // Diff content
        ])
        .split(area);

        self.render_header(frame, chunks[0]);
        self.render_context_bar(frame, chunks[1]);
        self.render_diff_content(frame, chunks[2]);
    }

    /// Render the header (commit info including description)
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let title = Line::from(vec![
            Span::raw(" Tij - Diff View ").bold(),
            Span::raw("["),
            Span::styled(
                self.change_id.chars().take(8).collect::<String>(),
                Style::default().fg(theme::log_view::CHANGE_ID),
            ),
            Span::raw("]"),
        ])
        .centered();

        // Truncate description to fit in one line
        let description = if self.content.description.is_empty() {
            "(no description)".to_string()
        } else {
            self.content
                .description
                .lines()
                .next()
                .unwrap_or("")
                .to_string()
        };

        let header_text = vec![
            Line::from(vec![
                Span::raw("Commit: "),
                Span::styled(
                    self.content.commit_id.chars().take(40).collect::<String>(),
                    Style::default().fg(theme::log_view::CHANGE_ID),
                ),
            ]),
            Line::from(vec![
                Span::raw("Author: "),
                Span::raw(&self.content.author),
                Span::raw("  "),
                Span::styled(
                    &self.content.timestamp,
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(vec![Span::styled(
                description,
                Style::default().fg(Color::White).bold(),
            )]),
        ];

        let header = Paragraph::new(header_text).block(
            Block::default()
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .title(title),
        );

        frame.render_widget(header, area);
    }

    /// Render the context bar (current file name + progress)
    fn render_context_bar(&self, frame: &mut Frame, area: Rect) {
        let file_info = if self.file_count() > 0 {
            let file_name = self.current_file_name().unwrap_or("(unknown)");
            format!(
                " {} [{}/{}]",
                file_name,
                self.current_file_index + 1,
                self.file_count()
            )
        } else {
            " (no files)".to_string()
        };

        let bar = Paragraph::new(Line::from(vec![Span::styled(
            file_info,
            Style::default().fg(Color::Cyan).bold(),
        )]))
        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT));

        frame.render_widget(bar, area);
    }

    /// Render the diff content (scrollable)
    fn render_diff_content(&self, frame: &mut Frame, area: Rect) {
        // No top/bottom borders, only left/right, so use full height
        let inner_height = area.height as usize;

        if !self.has_changes() {
            // Empty state
            let empty_msg = Paragraph::new(vec![
                Line::from(""),
                Line::from("No changes in this revision.").centered(),
            ])
            .block(Block::default().borders(Borders::LEFT | Borders::RIGHT));
            frame.render_widget(empty_msg, area);
            return;
        }

        // Build visible lines
        let lines: Vec<Line> = self
            .content
            .lines
            .iter()
            .skip(self.scroll_offset)
            .take(inner_height)
            .map(|diff_line| self.render_diff_line(diff_line))
            .collect();

        let diff =
            Paragraph::new(lines).block(Block::default().borders(Borders::LEFT | Borders::RIGHT));

        frame.render_widget(diff, area);
    }

    /// Render a single diff line
    fn render_diff_line(&self, line: &DiffLine) -> Line<'static> {
        match line.kind {
            DiffLineKind::FileHeader => Line::from(Span::styled(
                format!("── {} ──", line.content),
                Style::default().fg(theme::diff_view::FILE_HEADER).bold(),
            )),
            DiffLineKind::Separator => Line::from(""),
            DiffLineKind::Context => {
                let line_nums = self.format_line_numbers(line.line_numbers);
                Line::from(vec![
                    Span::styled(
                        line_nums,
                        Style::default().fg(theme::diff_view::LINE_NUMBER),
                    ),
                    Span::raw("  "),
                    Span::raw(line.content.clone()),
                ])
            }
            DiffLineKind::Added => {
                let line_nums = self.format_line_numbers(line.line_numbers);
                Line::from(vec![
                    Span::styled(
                        line_nums,
                        Style::default().fg(theme::diff_view::LINE_NUMBER),
                    ),
                    Span::styled(" +", Style::default().fg(theme::diff_view::ADDED)),
                    Span::styled(
                        line.content.clone(),
                        Style::default().fg(theme::diff_view::ADDED),
                    ),
                ])
            }
            DiffLineKind::Deleted => {
                let line_nums = self.format_line_numbers(line.line_numbers);
                Line::from(vec![
                    Span::styled(
                        line_nums,
                        Style::default().fg(theme::diff_view::LINE_NUMBER),
                    ),
                    Span::styled(" -", Style::default().fg(theme::diff_view::DELETED)),
                    Span::styled(
                        line.content.clone(),
                        Style::default().fg(theme::diff_view::DELETED),
                    ),
                ])
            }
        }
    }

    /// Format line numbers for display
    fn format_line_numbers(&self, line_nums: Option<(Option<usize>, Option<usize>)>) -> String {
        match line_nums {
            Some((old, new)) => {
                let old_str = old
                    .map(|n| format!("{:4}", n))
                    .unwrap_or_else(|| "    ".to_string());
                let new_str = new
                    .map(|n| format!("{:4}", n))
                    .unwrap_or_else(|| "    ".to_string());
                format!("{} {}", old_str, new_str)
            }
            None => "         ".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_content() -> DiffContent {
        let mut content = DiffContent {
            commit_id: "abc123def456".to_string(),
            author: "Test User <test@example.com>".to_string(),
            timestamp: "2024-01-30 12:00:00".to_string(),
            description: "Test commit".to_string(),
            lines: Vec::new(),
        };

        // Add some test diff lines
        content.lines.push(DiffLine::file_header("src/main.rs"));
        content
            .lines
            .push(DiffLine::context(Some(10), Some(10), "fn main() {"));
        content
            .lines
            .push(DiffLine::deleted(11, "    println!(\"old\");"));
        content
            .lines
            .push(DiffLine::added(11, "    println!(\"new\");"));
        content
            .lines
            .push(DiffLine::context(Some(12), Some(12), "}"));
        content.lines.push(DiffLine::separator());
        content.lines.push(DiffLine::file_header("src/lib.rs"));
        content.lines.push(DiffLine::added(1, "pub fn hello() {}"));

        content
    }

    #[test]
    fn test_diff_view_empty() {
        let view = DiffView::empty();
        assert!(view.change_id.is_empty());
        assert!(!view.has_changes());
        assert_eq!(view.file_count(), 0);
    }

    #[test]
    fn test_diff_view_new() {
        let view = DiffView::new("testchange".to_string(), create_test_content());

        assert_eq!(view.change_id, "testchange");
        assert!(view.has_changes());
        assert_eq!(view.file_count(), 2);
        assert_eq!(view.file_names, vec!["src/main.rs", "src/lib.rs"]);
        assert_eq!(view.file_header_positions, vec![0, 6]);
    }

    #[test]
    fn test_diff_view_scroll() {
        let mut view = DiffView::new("test".to_string(), create_test_content());
        // Set visible height smaller than total lines to allow scrolling
        view.visible_height = 5;

        assert_eq!(view.scroll_offset, 0);

        view.scroll_down();
        assert_eq!(view.scroll_offset, 1);

        view.scroll_down();
        view.scroll_down();
        assert_eq!(view.scroll_offset, 3);

        view.scroll_up();
        assert_eq!(view.scroll_offset, 2);

        view.jump_to_top();
        assert_eq!(view.scroll_offset, 0);
    }

    #[test]
    fn test_diff_view_scroll_bounds() {
        let mut view = DiffView::new("test".to_string(), create_test_content());

        // Scroll up at top should stay at 0
        view.scroll_up();
        assert_eq!(view.scroll_offset, 0);

        // Set a visible height smaller than total lines
        view.visible_height = 5;

        // Scroll to bottom
        for _ in 0..20 {
            view.scroll_down();
        }

        // With 8 total lines and 5 visible, max offset should be 3
        // (so the last 5 lines are visible)
        let expected_max = view.total_lines().saturating_sub(view.visible_height);
        assert_eq!(view.scroll_offset, expected_max);
    }

    #[test]
    fn test_diff_view_file_jump() {
        let mut view = DiffView::new("test".to_string(), create_test_content());

        assert_eq!(view.current_file_index, 0);
        assert_eq!(view.scroll_offset, 0);

        // Jump to next file (src/lib.rs at position 6)
        view.next_file();
        assert_eq!(view.current_file_index, 1);
        assert_eq!(view.scroll_offset, 6);

        // Jump to next file wraps to first
        view.next_file();
        assert_eq!(view.current_file_index, 0);
        assert_eq!(view.scroll_offset, 0);

        // Jump to previous file
        view.prev_file();
        assert_eq!(view.current_file_index, 1);
        assert_eq!(view.scroll_offset, 6);
    }

    #[test]
    fn test_diff_view_current_file_name() {
        let mut view = DiffView::new("test".to_string(), create_test_content());

        assert_eq!(view.current_file_name(), Some("src/main.rs"));

        view.next_file();
        assert_eq!(view.current_file_name(), Some("src/lib.rs"));
    }

    #[test]
    fn test_diff_view_handle_key_scroll() {
        let mut view = DiffView::new("test".to_string(), create_test_content());

        // Use handle_key_with_height to set visible height smaller than total lines
        let action =
            view.handle_key_with_height(KeyEvent::from(crossterm::event::KeyCode::Char('j')), 5);
        assert_eq!(action, DiffAction::None);
        assert_eq!(view.scroll_offset, 1);
    }

    #[test]
    fn test_diff_view_handle_key_back() {
        let mut view = DiffView::empty();

        let action = view.handle_key(KeyEvent::from(crossterm::event::KeyCode::Char('q')));
        assert_eq!(action, DiffAction::Back);

        let action = view.handle_key(KeyEvent::from(crossterm::event::KeyCode::Esc));
        assert_eq!(action, DiffAction::Back);
    }

    #[test]
    fn test_diff_view_half_page_scroll() {
        let mut view = DiffView::new("test".to_string(), create_test_content());

        // With 8 total lines and visible_height 4, max offset is 4
        // Half page is 2, so first scroll goes to 2
        view.scroll_half_page_down(4);
        assert_eq!(view.scroll_offset, 2);

        view.scroll_half_page_up(4);
        assert_eq!(view.scroll_offset, 0);
    }

    #[test]
    fn test_diff_view_clear() {
        let mut view = DiffView::new("test".to_string(), create_test_content());
        // Set visible height smaller than total lines to allow scrolling
        view.visible_height = 5;
        view.scroll_down();

        assert!(view.has_changes());
        assert_eq!(view.scroll_offset, 1);

        view.clear();

        assert!(!view.has_changes());
        assert_eq!(view.scroll_offset, 0);
        assert!(view.change_id.is_empty());
    }

    #[test]
    fn test_diff_view_update_current_file_index() {
        let mut view = DiffView::new("test".to_string(), create_test_content());

        // At start, should be file 0
        assert_eq!(view.current_file_index, 0);

        // After scrolling past file header of second file
        view.scroll_offset = 7;
        view.update_current_file_index();
        assert_eq!(view.current_file_index, 1);

        // Back before second file
        view.scroll_offset = 3;
        view.update_current_file_index();
        assert_eq!(view.current_file_index, 0);
    }

    #[test]
    fn test_diff_view_current_context() {
        let mut view = DiffView::new("test".to_string(), create_test_content());

        assert_eq!(view.current_context(), "src/main.rs [1/2]");

        view.next_file();
        assert_eq!(view.current_context(), "src/lib.rs [2/2]");
    }

    #[test]
    fn test_diff_view_current_context_empty() {
        let view = DiffView::empty();
        assert_eq!(view.current_context(), "(no files)");
    }

    #[test]
    fn test_diff_view_scroll_with_zero_visible_height() {
        let mut view = DiffView::new("test".to_string(), create_test_content());

        // When visible_height is 0, scrolling should not be allowed
        view.visible_height = 0;

        // Try to scroll down - should stay at 0
        view.scroll_down();
        assert_eq!(view.scroll_offset, 0);

        // Try half page down with 0 height
        view.scroll_half_page_down(0);
        assert_eq!(view.scroll_offset, 0);
    }
}
