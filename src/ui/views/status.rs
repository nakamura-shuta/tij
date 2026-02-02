//! Status View
//!
//! Displays the current working copy status with changed files.

use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::keys;
use crate::model::{FileState, Status};
use crate::ui::{components, theme};

/// Action returned from StatusView key handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusAction {
    /// Show diff for selected file (opens DiffView, jumps to file)
    ShowFileDiff {
        /// Working copy change ID
        change_id: String,
        /// File path to jump to
        file_path: String,
    },
    /// Go back to Log View
    Back,
    /// Switch to Log View (Tab)
    SwitchToLog,
    /// No action
    None,
}

/// Status View state
#[derive(Debug)]
pub struct StatusView {
    /// Current status (None if not loaded)
    status: Option<Status>,

    /// Selected file index
    selected_index: usize,

    /// Scroll offset for display
    scroll_offset: usize,
}

impl Default for StatusView {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusView {
    /// Create a new StatusView
    pub fn new() -> Self {
        Self {
            status: None,
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    /// Set the status data
    pub fn set_status(&mut self, status: Status) {
        self.status = Some(status);
        // Reset selection and scroll if out of bounds
        if let Some(ref s) = self.status {
            if self.selected_index >= s.files.len() {
                self.selected_index = 0;
                self.scroll_offset = 0;
            }
            // Also reset scroll if it would show empty area
            if self.scroll_offset >= s.files.len() {
                self.scroll_offset = 0;
            }
        }
    }

    /// Get the current status
    pub fn status(&self) -> Option<&Status> {
        self.status.as_ref()
    }

    /// Get the selected file path
    pub fn selected_file_path(&self) -> Option<&str> {
        self.status
            .as_ref()
            .and_then(|s| s.files.get(self.selected_index))
            .map(|f| f.path.as_str())
    }

    /// Get the working copy change ID
    pub fn working_copy_id(&self) -> Option<&str> {
        self.status
            .as_ref()
            .map(|s| s.working_copy_change_id.as_str())
    }

    /// Move selection down
    fn move_down(&mut self, visible_count: usize) {
        if let Some(ref status) = self.status {
            if !status.files.is_empty() && self.selected_index < status.files.len() - 1 {
                self.selected_index += 1;
                self.adjust_scroll(visible_count);
            }
        }
    }

    /// Move selection up
    fn move_up(&mut self, visible_count: usize) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.adjust_scroll(visible_count);
        }
    }

    /// Jump to top
    fn jump_to_top(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Jump to bottom
    fn jump_to_bottom(&mut self, visible_count: usize) {
        if let Some(ref status) = self.status {
            if !status.files.is_empty() {
                self.selected_index = status.files.len() - 1;
                self.adjust_scroll(visible_count);
            }
        }
    }

    /// Adjust scroll offset to keep selection visible
    fn adjust_scroll(&mut self, visible_count: usize) {
        if visible_count == 0 {
            return;
        }
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible_count {
            self.scroll_offset = self.selected_index - visible_count + 1;
        }
    }

    /// Default visible count for scroll calculations
    const DEFAULT_VISIBLE_COUNT: usize = 20;

    /// Handle key event
    pub fn handle_key(&mut self, key: KeyEvent) -> StatusAction {
        self.handle_key_with_height(key, Self::DEFAULT_VISIBLE_COUNT)
    }

    /// Handle key event with explicit visible height
    pub fn handle_key_with_height(&mut self, key: KeyEvent, visible_count: usize) -> StatusAction {
        match key.code {
            code if code == keys::MOVE_DOWN => {
                self.move_down(visible_count);
                StatusAction::None
            }
            code if code == keys::MOVE_UP => {
                self.move_up(visible_count);
                StatusAction::None
            }
            code if code == keys::GO_TOP => {
                self.jump_to_top();
                StatusAction::None
            }
            code if code == keys::GO_BOTTOM => {
                self.jump_to_bottom(visible_count);
                StatusAction::None
            }
            code if code == keys::OPEN_DIFF => {
                if let (Some(change_id), Some(file_path)) =
                    (self.working_copy_id(), self.selected_file_path())
                {
                    StatusAction::ShowFileDiff {
                        change_id: change_id.to_string(),
                        file_path: file_path.to_string(),
                    }
                } else {
                    StatusAction::None
                }
            }
            // Note: QUIT, TAB, ESC are handled by global key handler in input.rs
            _ => StatusAction::None,
        }
    }

    /// Render the view
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let title = Line::from(" Tij - Status View ").bold().cyan().centered();

        match &self.status {
            None => {
                // Loading state
                let content = components::empty_state("Loading...", None)
                    .block(components::bordered_block(title));
                frame.render_widget(content, area);
            }
            Some(status) if status.is_clean() => {
                // Clean state
                let content =
                    components::empty_state("Working copy is clean.", Some("No modified files."))
                        .block(components::bordered_block(title));
                frame.render_widget(content, area);
            }
            Some(status) => {
                // Has changes
                self.render_file_list(frame, area, status, &title);
            }
        }
    }

    /// Render the file list
    fn render_file_list(&self, frame: &mut Frame, area: Rect, status: &Status, title: &Line) {
        // Calculate available height for files (minus borders and header)
        let inner_height = area.height.saturating_sub(5) as usize; // 2 borders + 3 header lines

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
        lines.push(Line::from("")); // Separator

        // File list
        for (idx, file) in status.files.iter().enumerate().skip(self.scroll_offset) {
            if lines.len() >= inner_height + 3 {
                // +3 for header
                break;
            }

            let is_selected = idx == self.selected_index;
            let line = self.build_file_line(file, is_selected);
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines).block(components::bordered_block(title.clone()));
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
                    .bg(theme::status_view::SELECTED_BG)
                    .add_modifier(Modifier::BOLD),
            );
        }

        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FileStatus;
    use crossterm::event::KeyCode;

    fn sample_status() -> Status {
        Status {
            files: vec![
                FileStatus {
                    path: "src/main.rs".to_string(),
                    state: FileState::Modified,
                },
                FileStatus {
                    path: "src/new.rs".to_string(),
                    state: FileState::Added,
                },
                FileStatus {
                    path: "old.rs".to_string(),
                    state: FileState::Deleted,
                },
            ],
            has_conflicts: false,
            working_copy_change_id: "abc12345".to_string(),
            parent_change_id: "xyz98765".to_string(),
        }
    }

    #[test]
    fn test_new_status_view() {
        let view = StatusView::new();
        assert!(view.status.is_none());
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_set_status() {
        let mut view = StatusView::new();
        view.set_status(sample_status());

        assert!(view.status.is_some());
        assert_eq!(view.status.as_ref().unwrap().files.len(), 3);
    }

    #[test]
    fn test_move_down() {
        let mut view = StatusView::new();
        view.set_status(sample_status());

        assert_eq!(view.selected_index, 0);
        view.move_down(20);
        assert_eq!(view.selected_index, 1);
        view.move_down(20);
        assert_eq!(view.selected_index, 2);
        view.move_down(20); // Should not go beyond last item
        assert_eq!(view.selected_index, 2);
    }

    #[test]
    fn test_move_up() {
        let mut view = StatusView::new();
        view.set_status(sample_status());
        view.selected_index = 2;

        view.move_up(20);
        assert_eq!(view.selected_index, 1);
        view.move_up(20);
        assert_eq!(view.selected_index, 0);
        view.move_up(20); // Should not go below 0
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_jump_to_top_bottom() {
        let mut view = StatusView::new();
        view.set_status(sample_status());
        view.selected_index = 1;

        view.jump_to_bottom(20);
        assert_eq!(view.selected_index, 2);

        view.jump_to_top();
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_selected_file_path() {
        let mut view = StatusView::new();
        view.set_status(sample_status());

        assert_eq!(view.selected_file_path(), Some("src/main.rs"));
        view.selected_index = 1;
        assert_eq!(view.selected_file_path(), Some("src/new.rs"));
    }

    #[test]
    fn test_working_copy_id() {
        let mut view = StatusView::new();
        view.set_status(sample_status());

        assert_eq!(view.working_copy_id(), Some("abc12345"));
    }

    #[test]
    fn test_handle_key_navigation() {
        let mut view = StatusView::new();
        view.set_status(sample_status());

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(action, StatusAction::None);
        assert_eq!(view.selected_index, 1);

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('k')));
        assert_eq!(action, StatusAction::None);
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_handle_key_open_diff() {
        let mut view = StatusView::new();
        view.set_status(sample_status());

        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        match action {
            StatusAction::ShowFileDiff {
                change_id,
                file_path,
            } => {
                assert_eq!(change_id, "abc12345");
                assert_eq!(file_path, "src/main.rs");
            }
            _ => panic!("Expected ShowFileDiff action"),
        }
    }

    // Note: QUIT and TAB are handled by global key handler in input.rs,
    // not by StatusView.handle_key(), so no tests here for those keys.

    #[test]
    fn test_empty_status() {
        let mut view = StatusView::new();
        let empty_status = Status {
            files: vec![],
            has_conflicts: false,
            working_copy_change_id: "abc".to_string(),
            parent_change_id: "xyz".to_string(),
        };
        view.set_status(empty_status);

        assert!(view.status.as_ref().unwrap().is_clean());
    }
}
