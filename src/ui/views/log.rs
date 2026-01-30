//! Log View - displays jj log output
//!
//! The main view of Tij, showing the change history.

use crossterm::event::{KeyCode, KeyEvent};
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

/// Input mode for Log View
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Normal navigation mode
    #[default]
    Normal,
    /// Text search input mode (for n/N navigation)
    SearchInput,
    /// Revset input mode (for jj filtering)
    RevsetInput,
}

/// Actions that LogView can request from App
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogAction {
    /// No action needed
    None,
    /// Open diff view for the given change ID
    OpenDiff(String),
    /// Execute revset filter
    ExecuteRevset(String),
    /// Clear revset filter (reset to default)
    ClearRevset,
}

/// Log View state
#[derive(Debug, Default)]
pub struct LogView {
    /// List of changes to display
    pub changes: Vec<Change>,
    /// Currently selected index
    pub selected_index: usize,
    /// Scroll offset for display
    pub scroll_offset: usize,
    /// Current input mode
    pub input_mode: InputMode,
    /// Input buffer for revset
    pub input_buffer: String,
    /// Revset input history
    pub revset_history: Vec<String>,
    /// Current revset filter (None = default)
    pub current_revset: Option<String>,
    /// Last search query for n/N navigation
    last_search_query: Option<String>,
}

impl LogView {
    /// Create a new LogView
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the changes to display
    pub fn set_changes(&mut self, changes: Vec<Change>) {
        self.changes = changes;
        // Reset selection if out of bounds
        if self.selected_index >= self.changes.len() && !self.changes.is_empty() {
            self.selected_index = self.changes.len() - 1;
        }
        if self.changes.is_empty() {
            self.selected_index = 0;
        }
    }

    /// Get the currently selected change
    pub fn selected_change(&self) -> Option<&Change> {
        self.changes.get(self.selected_index)
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if !self.changes.is_empty() && self.selected_index < self.changes.len() - 1 {
            self.selected_index += 1;
        }
    }

    /// Move to top
    pub fn move_to_top(&mut self) {
        self.selected_index = 0;
    }

    /// Move to bottom
    pub fn move_to_bottom(&mut self) {
        if !self.changes.is_empty() {
            self.selected_index = self.changes.len() - 1;
        }
    }

    /// Check if a change matches the search query
    fn change_matches(&self, change: &Change, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        change.change_id.to_lowercase().contains(&query_lower)
            || change.description.to_lowercase().contains(&query_lower)
            || change.author.to_lowercase().contains(&query_lower)
            || change
                .bookmarks
                .iter()
                .any(|b| b.to_lowercase().contains(&query_lower))
    }

    /// Search for first match from beginning (used when search is confirmed)
    pub fn search_first(&mut self) -> bool {
        let Some(ref query) = self.last_search_query else {
            return false;
        };
        if self.changes.is_empty() {
            return false;
        }

        let query = query.clone();

        // Search from beginning
        for i in 0..self.changes.len() {
            if self.change_matches(&self.changes[i], &query) {
                self.selected_index = i;
                return true;
            }
        }

        false
    }

    /// Search for next match (n key)
    pub fn search_next(&mut self) -> bool {
        let Some(ref query) = self.last_search_query else {
            return false;
        };
        if self.changes.is_empty() {
            return false;
        }

        let query = query.clone();
        let start = self.selected_index + 1;

        // Search from current position to end
        for i in start..self.changes.len() {
            if self.change_matches(&self.changes[i], &query) {
                self.selected_index = i;
                return true;
            }
        }

        // Wrap around: search from beginning to current position
        for i in 0..self.selected_index {
            if self.change_matches(&self.changes[i], &query) {
                self.selected_index = i;
                return true;
            }
        }

        false
    }

    /// Search for previous match (N key)
    pub fn search_prev(&mut self) -> bool {
        let Some(ref query) = self.last_search_query else {
            return false;
        };
        if self.changes.is_empty() {
            return false;
        }

        let query = query.clone();

        // Search from current position to beginning
        for i in (0..self.selected_index).rev() {
            if self.change_matches(&self.changes[i], &query) {
                self.selected_index = i;
                return true;
            }
        }

        // Wrap around: search from end to current position
        for i in (self.selected_index + 1..self.changes.len()).rev() {
            if self.change_matches(&self.changes[i], &query) {
                self.selected_index = i;
                return true;
            }
        }

        false
    }

    /// Start text search input mode
    pub fn start_search_input(&mut self) {
        self.input_mode = InputMode::SearchInput;
        self.input_buffer.clear();
    }

    /// Start revset input mode
    pub fn start_revset_input(&mut self) {
        self.input_mode = InputMode::RevsetInput;
        self.input_buffer.clear();
    }

    /// Cancel input mode
    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    /// Handle key event and return action
    pub fn handle_key(&mut self, key: KeyEvent) -> LogAction {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::SearchInput => self.handle_search_input_key(key),
            InputMode::RevsetInput => self.handle_revset_input_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_down();
                LogAction::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_up();
                LogAction::None
            }
            KeyCode::Char('g') => {
                self.move_to_top();
                LogAction::None
            }
            KeyCode::Char('G') => {
                self.move_to_bottom();
                LogAction::None
            }
            KeyCode::Char('/') => {
                self.start_search_input();
                LogAction::None
            }
            KeyCode::Char('r') => {
                self.start_revset_input();
                LogAction::None
            }
            KeyCode::Char('n') => {
                self.search_next();
                LogAction::None
            }
            KeyCode::Char('N') => {
                self.search_prev();
                LogAction::None
            }
            KeyCode::Enter => {
                if let Some(change) = self.selected_change() {
                    LogAction::OpenDiff(change.change_id.clone())
                } else {
                    LogAction::None
                }
            }
            _ => LogAction::None,
        }
    }

    fn handle_search_input_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            KeyCode::Esc => {
                self.cancel_input();
                LogAction::None
            }
            KeyCode::Enter => {
                let query = self.input_buffer.clone();
                if query.is_empty() {
                    // Clear search query
                    self.last_search_query = None;
                } else {
                    self.last_search_query = Some(query);
                    // Jump to first match from beginning
                    self.search_first();
                }
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
                LogAction::None
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                LogAction::None
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                LogAction::None
            }
            _ => LogAction::None,
        }
    }

    fn handle_revset_input_key(&mut self, key: KeyEvent) -> LogAction {
        match key.code {
            KeyCode::Esc => {
                self.cancel_input();
                LogAction::None
            }
            KeyCode::Enter => {
                let revset = self.input_buffer.clone();
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
                if revset.is_empty() {
                    // Clear revset (reset to default)
                    LogAction::ClearRevset
                } else {
                    self.revset_history.push(revset.clone());
                    LogAction::ExecuteRevset(revset)
                }
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                LogAction::None
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                LogAction::None
            }
            _ => LogAction::None,
        }
    }

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

        // Calculate visible range
        // N items = N lines + (N-1) connectors = 2N-1 lines
        // So visible_changes = (inner_height + 1) / 2
        let inner_height = area.height.saturating_sub(2) as usize; // borders
        let visible_changes = if inner_height == 0 {
            0
        } else {
            (inner_height + 1) / 2
        };

        // Adjust scroll offset to keep selection visible
        let scroll_offset = self.calculate_scroll_offset(visible_changes);

        // Build lines
        let mut lines: Vec<Line> = Vec::new();
        for (idx, change) in self.changes.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }

            let is_selected = idx == self.selected_index;
            let change_line = self.build_change_line(change, is_selected);
            lines.push(change_line);

            // Add connector line (except for last)
            if idx < self.changes.len() - 1 && lines.len() < inner_height {
                lines.push(Line::from(format!("{}", symbols::markers::CONNECTOR)));
            }
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
            Line::from("No changes found.").centered(),
            Line::from(""),
            Line::from("Hint: Try '/' with revset all()")
                .dark_gray()
                .centered(),
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
        let (marker, marker_color) = self.marker_for_change(change);

        let mut spans = vec![
            Span::styled(format!("{}  ", marker), Style::default().fg(marker_color)),
            Span::styled(
                format!("{} ", change.short_id()),
                Style::default().fg(theme::log_view::CHANGE_ID),
            ),
        ];

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

    fn marker_for_change(&self, change: &Change) -> (char, ratatui::style::Color) {
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
        let (prompt, title) = match self.input_mode {
            InputMode::SearchInput => ("Search: ", " / Search "),
            InputMode::RevsetInput => ("Revset: ", " r Revset "),
            InputMode::Normal => return,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_changes() -> Vec<Change> {
        vec![
            Change {
                change_id: "abc12345".to_string(),
                commit_id: "def67890".to_string(),
                author: "user@example.com".to_string(),
                timestamp: "2024-01-29".to_string(),
                description: "First commit".to_string(),
                is_working_copy: true,
                is_empty: false,
                bookmarks: vec!["main".to_string()],
            },
            Change {
                change_id: "xyz98765".to_string(),
                commit_id: "uvw43210".to_string(),
                author: "user@example.com".to_string(),
                timestamp: "2024-01-28".to_string(),
                description: "Initial commit".to_string(),
                is_working_copy: false,
                is_empty: false,
                bookmarks: vec![],
            },
            Change {
                change_id: constants::ROOT_CHANGE_ID.to_string(),
                commit_id: "0".repeat(40),
                author: "".to_string(),
                timestamp: "".to_string(),
                description: "".to_string(),
                is_working_copy: false,
                is_empty: true,
                bookmarks: vec![],
            },
        ]
    }

    #[test]
    fn test_log_view_new() {
        let view = LogView::new();
        assert!(view.changes.is_empty());
        assert_eq!(view.selected_index, 0);
        assert_eq!(view.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_set_changes() {
        let mut view = LogView::new();
        let changes = create_test_changes();
        view.set_changes(changes.clone());
        assert_eq!(view.changes.len(), 3);
    }

    #[test]
    fn test_navigation() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());

        assert_eq!(view.selected_index, 0);

        view.move_down();
        assert_eq!(view.selected_index, 1);

        view.move_down();
        assert_eq!(view.selected_index, 2);

        // Should not go past last item
        view.move_down();
        assert_eq!(view.selected_index, 2);

        view.move_up();
        assert_eq!(view.selected_index, 1);

        view.move_to_top();
        assert_eq!(view.selected_index, 0);

        view.move_to_bottom();
        assert_eq!(view.selected_index, 2);
    }

    #[test]
    fn test_navigation_bounds_empty() {
        let mut view = LogView::new();

        // Should not panic on empty list
        view.move_down();
        view.move_up();
        view.move_to_top();
        view.move_to_bottom();

        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_selected_change() {
        let mut view = LogView::new();
        assert!(view.selected_change().is_none());

        view.set_changes(create_test_changes());
        assert!(view.selected_change().is_some());
        assert_eq!(view.selected_change().unwrap().change_id, "abc12345");

        view.move_down();
        assert_eq!(view.selected_change().unwrap().change_id, "xyz98765");
    }

    #[test]
    fn test_input_mode_toggle() {
        let mut view = LogView::new();
        assert_eq!(view.input_mode, InputMode::Normal);

        view.start_revset_input();
        assert_eq!(view.input_mode, InputMode::RevsetInput);

        view.cancel_input();
        assert_eq!(view.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_handle_key_navigation() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(action, LogAction::None);
        assert_eq!(view.selected_index, 1);

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('k')));
        assert_eq!(action, LogAction::None);
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_handle_key_open_diff() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());

        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(action, LogAction::OpenDiff("abc12345".to_string()));
    }

    #[test]
    fn test_handle_key_search_input() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());

        // Start search mode with /
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('/')));
        assert_eq!(action, LogAction::None);
        assert_eq!(view.input_mode, InputMode::SearchInput);

        // Type search query
        view.handle_key(KeyEvent::from(KeyCode::Char('I')));
        view.handle_key(KeyEvent::from(KeyCode::Char('n')));
        view.handle_key(KeyEvent::from(KeyCode::Char('i')));
        view.handle_key(KeyEvent::from(KeyCode::Char('t')));
        assert_eq!(view.input_buffer, "Init");

        // Submit - should store query and jump to match
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(action, LogAction::None); // Search doesn't execute revset
        assert_eq!(view.input_mode, InputMode::Normal);
        assert!(view.input_buffer.is_empty());
        assert_eq!(view.last_search_query, Some("Init".to_string()));
        assert_eq!(view.selected_index, 1); // Jumped to "Initial commit"
    }

    #[test]
    fn test_handle_key_revset_input() {
        let mut view = LogView::new();

        // Start revset mode with r
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('r')));
        assert_eq!(action, LogAction::None);
        assert_eq!(view.input_mode, InputMode::RevsetInput);

        // Type revset
        view.handle_key(KeyEvent::from(KeyCode::Char('a')));
        view.handle_key(KeyEvent::from(KeyCode::Char('l')));
        view.handle_key(KeyEvent::from(KeyCode::Char('l')));
        assert_eq!(view.input_buffer, "all");

        // Submit
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(action, LogAction::ExecuteRevset("all".to_string()));
        assert_eq!(view.input_mode, InputMode::Normal);
        assert!(view.input_buffer.is_empty());
        assert_eq!(view.revset_history, vec!["all".to_string()]);
    }

    #[test]
    fn test_handle_key_revset_cancel() {
        let mut view = LogView::new();

        view.start_revset_input();
        view.handle_key(KeyEvent::from(KeyCode::Char('t')));
        view.handle_key(KeyEvent::from(KeyCode::Char('e')));
        assert_eq!(view.input_buffer, "te");

        // Cancel with Esc
        let action = view.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(action, LogAction::None);
        assert_eq!(view.input_mode, InputMode::Normal);
        assert!(view.input_buffer.is_empty());
    }

    #[test]
    fn test_handle_key_backspace() {
        let mut view = LogView::new();
        view.start_revset_input();

        view.handle_key(KeyEvent::from(KeyCode::Char('a')));
        view.handle_key(KeyEvent::from(KeyCode::Char('b')));
        assert_eq!(view.input_buffer, "ab");

        view.handle_key(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(view.input_buffer, "a");
    }

    #[test]
    fn test_marker_for_change() {
        let view = LogView::new();

        let working_copy = Change {
            change_id: "abc".to_string(),
            is_working_copy: true,
            ..Default::default()
        };
        let (marker, color) = view.marker_for_change(&working_copy);
        assert_eq!(marker, symbols::markers::WORKING_COPY);
        assert_eq!(color, theme::log_view::WORKING_COPY_MARKER);

        let root = Change {
            change_id: constants::ROOT_CHANGE_ID.to_string(),
            is_working_copy: false,
            ..Default::default()
        };
        let (marker, color) = view.marker_for_change(&root);
        assert_eq!(marker, symbols::markers::ROOT);
        assert_eq!(color, theme::log_view::ROOT_MARKER);

        let normal = Change {
            change_id: "xyz".to_string(),
            is_working_copy: false,
            ..Default::default()
        };
        let (marker, color) = view.marker_for_change(&normal);
        assert_eq!(marker, symbols::markers::NORMAL);
        assert_eq!(color, theme::log_view::NORMAL_MARKER);
    }

    #[test]
    fn test_set_changes_resets_selection() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.selected_index = 2;

        // Set fewer changes
        view.set_changes(vec![create_test_changes()[0].clone()]);
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_search_first_finds_from_beginning() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.selected_index = 2; // Start at root
        view.last_search_query = Some("First".to_string());

        // Should find "First commit" at index 0, regardless of current position
        assert!(view.search_first());
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_search_first_no_match() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.last_search_query = Some("nonexistent".to_string());

        assert!(!view.search_first());
        assert_eq!(view.selected_index, 0); // Position unchanged
    }

    #[test]
    fn test_search_next_no_query() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());

        // No search query set
        assert!(!view.search_next());
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_search_next_finds_match() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.last_search_query = Some("Initial".to_string());

        // Should find "Initial commit" at index 1
        assert!(view.search_next());
        assert_eq!(view.selected_index, 1);
    }

    #[test]
    fn test_search_next_wraps_around() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.selected_index = 1; // Start at "Initial commit"
        view.last_search_query = Some("First".to_string());

        // Should wrap to find "First commit" at index 0
        assert!(view.search_next());
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_search_prev_finds_match() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.selected_index = 2; // Start at root
        view.last_search_query = Some("Initial".to_string());

        // Should find "Initial commit" at index 1
        assert!(view.search_prev());
        assert_eq!(view.selected_index, 1);
    }

    #[test]
    fn test_search_prev_wraps_around() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.selected_index = 0;
        view.last_search_query = Some("Initial".to_string());

        // Should wrap to find "Initial commit" at index 1
        assert!(view.search_prev());
        assert_eq!(view.selected_index, 1);
    }

    #[test]
    fn test_search_no_match() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.last_search_query = Some("nonexistent".to_string());

        assert!(!view.search_next());
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_search_by_author() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.last_search_query = Some("example.com".to_string());

        // Should match by author email
        assert!(view.search_next());
        assert_eq!(view.selected_index, 1); // Skips 0, finds 1
    }

    #[test]
    fn test_search_by_bookmark() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.selected_index = 1; // Start at index 1
        view.last_search_query = Some("main".to_string());

        // Should wrap and find "main" bookmark at index 0
        assert!(view.search_next());
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_search_case_insensitive() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.selected_index = 1; // Start at index 1
        view.last_search_query = Some("FIRST".to_string());

        // Should wrap and find "First commit" case-insensitively at index 0
        assert!(view.search_next());
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_handle_key_search_next() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.last_search_query = Some("Initial".to_string());

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('n')));
        assert_eq!(action, LogAction::None);
        assert_eq!(view.selected_index, 1);
    }

    #[test]
    fn test_handle_key_search_prev() {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());
        view.selected_index = 2;
        view.last_search_query = Some("First".to_string());

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('N')));
        assert_eq!(action, LogAction::None);
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_search_input_stores_query() {
        let mut view = LogView::new();
        view.start_search_input();

        // Type query
        view.handle_key(KeyEvent::from(KeyCode::Char('m')));
        view.handle_key(KeyEvent::from(KeyCode::Char('a')));
        view.handle_key(KeyEvent::from(KeyCode::Char('i')));
        view.handle_key(KeyEvent::from(KeyCode::Char('n')));

        // Submit
        view.handle_key(KeyEvent::from(KeyCode::Enter));

        // Should store as search query
        assert_eq!(view.last_search_query, Some("main".to_string()));
    }

    #[test]
    fn test_revset_input_does_not_store_search_query() {
        let mut view = LogView::new();
        view.start_revset_input();

        // Type revset
        view.handle_key(KeyEvent::from(KeyCode::Char('a')));
        view.handle_key(KeyEvent::from(KeyCode::Char('l')));
        view.handle_key(KeyEvent::from(KeyCode::Char('l')));

        // Submit
        view.handle_key(KeyEvent::from(KeyCode::Enter));

        // Revset should NOT be stored as search query
        assert_eq!(view.last_search_query, None);
    }

    #[test]
    fn test_search_empty_enter_clears_query() {
        let mut view = LogView::new();

        // Set a search query first
        view.last_search_query = Some("test".to_string());

        // Start search input and submit empty
        view.start_search_input();
        view.handle_key(KeyEvent::from(KeyCode::Enter));

        // Should clear search query
        assert_eq!(view.last_search_query, None);
    }

    #[test]
    fn test_revset_empty_enter_returns_clear_action() {
        let mut view = LogView::new();

        // Start revset input and submit empty
        view.start_revset_input();
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));

        // Should return ClearRevset action
        assert_eq!(action, LogAction::ClearRevset);
    }
}
