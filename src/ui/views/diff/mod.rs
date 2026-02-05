//! Diff View
//!
//! Displays the diff for a selected change from the log view.

mod input;
mod render;

use crate::model::DiffContent;

/// Action returned by DiffView key handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffAction {
    /// No action needed
    None,
    /// Return to log view
    Back,
    /// Open blame/annotation for current file
    OpenBlame {
        /// File path to annotate
        file_path: String,
    },
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
    /// Default visible height for scroll calculations when not specified
    const DEFAULT_VISIBLE_HEIGHT: usize = 20;

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
        use crate::model::DiffLineKind;

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

    /// Clear the view (test-only helper)
    #[cfg(test)]
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

    /// Jump to a specific file by path
    ///
    /// If the file is found, scrolls to its header position.
    /// If not found, does nothing.
    ///
    /// This handles renamed files where jj show outputs `prefix{old => new}`
    /// but StatusView passes just the new path `prefix/new`.
    pub fn jump_to_file(&mut self, file_path: &str) {
        // First try exact match
        if let Some(idx) = self.file_names.iter().position(|name| name == file_path) {
            if let Some(&pos) = self.file_header_positions.get(idx) {
                self.scroll_offset = pos;
                self.current_file_index = idx;
                return;
            }
        }

        // Try matching renamed files: "prefix{old => new}" should match "prefix/new"
        for (idx, name) in self.file_names.iter().enumerate() {
            if let Some(new_path) = Self::extract_new_path_from_rename(name) {
                if new_path == file_path {
                    if let Some(&pos) = self.file_header_positions.get(idx) {
                        self.scroll_offset = pos;
                        self.current_file_index = idx;
                        return;
                    }
                }
            }
        }
    }

    /// Extract the new path from a rename pattern like "prefix{old => new}"
    ///
    /// Returns the reconstructed new path: "prefix/new"
    fn extract_new_path_from_rename(name: &str) -> Option<String> {
        let brace_start = name.find('{')?;
        let brace_end = name.find('}')?;
        let prefix = &name[..brace_start];
        let inner = &name[brace_start + 1..brace_end];
        let (_, new_part) = inner.split_once(" => ")?;
        Some(format!("{}{}", prefix, new_part))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DiffContent, DiffLine};
    use crossterm::event::KeyEvent;

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

    #[test]
    fn test_diff_view_jump_to_file() {
        let mut view = DiffView::new("test".to_string(), create_test_content());

        // Start at first file
        assert_eq!(view.current_file_index, 0);
        assert_eq!(view.scroll_offset, 0);

        // Jump to second file by path
        view.jump_to_file("src/lib.rs");
        assert_eq!(view.current_file_index, 1);
        assert_eq!(view.scroll_offset, 6); // Second file header is at position 6

        // Jump back to first file
        view.jump_to_file("src/main.rs");
        assert_eq!(view.current_file_index, 0);
        assert_eq!(view.scroll_offset, 0);

        // Jump to non-existent file should do nothing
        view.jump_to_file("non_existent.rs");
        assert_eq!(view.current_file_index, 0);
        assert_eq!(view.scroll_offset, 0);
    }

    #[test]
    fn test_extract_new_path_from_rename() {
        // Standard rename with prefix
        assert_eq!(
            DiffView::extract_new_path_from_rename("src/{old.rs => new.rs}"),
            Some("src/new.rs".to_string())
        );

        // Rename without prefix
        assert_eq!(
            DiffView::extract_new_path_from_rename("{old.rs => new.rs}"),
            Some("new.rs".to_string())
        );

        // Deep path rename
        assert_eq!(
            DiffView::extract_new_path_from_rename("src/components/{Button.tsx => button.tsx}"),
            Some("src/components/button.tsx".to_string())
        );

        // Not a rename pattern
        assert_eq!(DiffView::extract_new_path_from_rename("src/main.rs"), None);
    }

    #[test]
    fn test_jump_to_file_with_rename() {
        // Create content with a renamed file
        let content = DiffContent {
            commit_id: "test123".to_string(),
            author: "Test".to_string(),
            timestamp: "2024-01-30".to_string(),
            description: "Test".to_string(),
            lines: vec![
                DiffLine::file_header("src/{old.rs => new.rs}"),
                DiffLine::added(1, "content"),
            ],
        };

        let mut view = DiffView::new("test".to_string(), content);
        assert_eq!(view.file_names, vec!["src/{old.rs => new.rs}"]);

        // Jump using the new path (as StatusView would provide)
        view.jump_to_file("src/new.rs");
        assert_eq!(view.current_file_index, 0);
        assert_eq!(view.scroll_offset, 0);
    }
}
