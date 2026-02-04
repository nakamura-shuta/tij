//! Status View
//!
//! Displays the current working copy status with changed files.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::keys;
use crate::model::{FileState, Notification, Status};
use crate::ui::{components, theme};

/// Input mode for Status View
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusInputMode {
    /// Normal navigation mode
    #[default]
    Normal,
    /// Commit message input mode
    CommitInput,
}

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
    /// Commit with message
    Commit { message: String },
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

    /// Current input mode
    pub input_mode: StatusInputMode,

    /// Input buffer for commit message
    pub input_buffer: String,
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
            input_mode: StatusInputMode::Normal,
            input_buffer: String::new(),
        }
    }

    /// Start commit input mode
    pub fn start_commit_input(&mut self) {
        self.input_mode = StatusInputMode::CommitInput;
        self.input_buffer.clear();
    }

    /// Cancel input mode
    pub fn cancel_input(&mut self) {
        self.input_mode = StatusInputMode::Normal;
        self.input_buffer.clear();
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
        match self.input_mode {
            StatusInputMode::Normal => self.handle_normal_key(key, visible_count),
            StatusInputMode::CommitInput => self.handle_commit_input_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent, visible_count: usize) -> StatusAction {
        match key.code {
            code if keys::is_move_down(code) => {
                self.move_down(visible_count);
                StatusAction::None
            }
            code if keys::is_move_up(code) => {
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
            code if code == keys::COMMIT => {
                // Only allow commit if there are changes
                if self.status.as_ref().is_some_and(|s| !s.is_clean()) {
                    self.start_commit_input();
                }
                StatusAction::None
            }
            // Note: QUIT, TAB, ESC are handled by global key handler in input.rs
            _ => StatusAction::None,
        }
    }

    fn handle_commit_input_key(&mut self, key: KeyEvent) -> StatusAction {
        match key.code {
            KeyCode::Esc => {
                self.cancel_input();
                StatusAction::None
            }
            KeyCode::Enter => {
                let message = std::mem::take(&mut self.input_buffer);
                self.input_mode = StatusInputMode::Normal;
                if message.is_empty() {
                    // Empty message = cancel
                    StatusAction::None
                } else {
                    StatusAction::Commit { message }
                }
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                StatusAction::None
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                StatusAction::None
            }
            _ => StatusAction::None,
        }
    }

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

        let title = Line::from(" Tij - Status View ").bold().cyan().centered();

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
