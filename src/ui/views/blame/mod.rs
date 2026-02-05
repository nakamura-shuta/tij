//! Blame View - Shows file annotation (blame) information
//!
//! Displays which change is responsible for each line of a file.

mod input;
mod render;

use crate::model::AnnotationContent;

/// Action returned by BlameView input handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlameAction {
    /// No action needed
    None,
    /// Go back to previous view
    Back,
    /// Open diff for the selected change
    OpenDiff(String),
}

/// View state for blame/annotation display
#[derive(Debug, Clone)]
pub struct BlameView {
    /// Annotation content
    content: AnnotationContent,
    /// Currently selected line index (0-based)
    selected_index: usize,
    /// Scroll offset for display
    scroll_offset: usize,
    /// Revision used for annotation (None = working copy)
    revision: Option<String>,
}

impl Default for BlameView {
    fn default() -> Self {
        Self::new()
    }
}

impl BlameView {
    /// Create a new empty blame view
    pub fn new() -> Self {
        Self {
            content: AnnotationContent::default(),
            selected_index: 0,
            scroll_offset: 0,
            revision: None,
        }
    }

    /// Set the annotation content with optional revision
    pub fn set_content(&mut self, content: AnnotationContent, revision: Option<String>) {
        self.content = content;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.revision = revision;
    }

    /// Get the revision used for this blame view
    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }

    /// Get the file path being displayed
    pub fn file_path(&self) -> &str {
        &self.content.file_path
    }

    /// Check if the view is empty
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Get the number of lines
    #[allow(dead_code)] // public API for future use
    pub fn line_count(&self) -> usize {
        self.content.len()
    }

    /// Get the currently selected line's change_id
    pub fn selected_change_id(&self) -> Option<&str> {
        self.content
            .lines
            .get(self.selected_index)
            .map(|line| line.change_id.as_str())
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if !self.content.is_empty() && self.selected_index < self.content.len() - 1 {
            self.selected_index += 1;
        }
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move to top
    pub fn move_to_top(&mut self) {
        self.selected_index = 0;
    }

    /// Move to bottom
    pub fn move_to_bottom(&mut self) {
        if !self.content.is_empty() {
            self.selected_index = self.content.len() - 1;
        }
    }

    /// Calculate scroll offset to keep selection visible
    fn calculate_scroll_offset(&self, visible_height: usize) -> usize {
        if visible_height == 0 {
            return 0;
        }

        let mut offset = self.scroll_offset;

        // Ensure selected item is visible
        if self.selected_index < offset {
            offset = self.selected_index;
        } else if self.selected_index >= offset + visible_height {
            offset = self.selected_index - visible_height + 1;
        }

        offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AnnotationLine;

    fn make_test_content() -> AnnotationContent {
        let mut content = AnnotationContent::new("test.rs".to_string());
        for i in 1..=10 {
            content.lines.push(AnnotationLine {
                change_id: format!("change{:02}", i),
                author: "test".to_string(),
                timestamp: "2026-01-30 10:00".to_string(),
                line_number: i,
                content: format!("line {}", i),
                first_in_hunk: i == 1 || i == 5,
            });
        }
        content
    }

    #[test]
    fn test_blame_view_new() {
        let view = BlameView::new();
        assert!(view.is_empty());
        assert_eq!(view.line_count(), 0);
    }

    #[test]
    fn test_blame_view_set_content() {
        let mut view = BlameView::new();
        view.set_content(make_test_content(), None);
        assert!(!view.is_empty());
        assert_eq!(view.line_count(), 10);
        assert_eq!(view.file_path(), "test.rs");
        assert_eq!(view.revision(), None);
    }

    #[test]
    fn test_blame_view_set_content_with_revision() {
        let mut view = BlameView::new();
        view.set_content(make_test_content(), Some("abc12345".to_string()));
        assert_eq!(view.revision(), Some("abc12345"));
    }

    #[test]
    fn test_blame_view_navigation() {
        let mut view = BlameView::new();
        view.set_content(make_test_content(), None);

        assert_eq!(view.selected_index, 0);

        view.move_down();
        assert_eq!(view.selected_index, 1);

        view.move_up();
        assert_eq!(view.selected_index, 0);

        // Can't go above 0
        view.move_up();
        assert_eq!(view.selected_index, 0);

        view.move_to_bottom();
        assert_eq!(view.selected_index, 9);

        // Can't go below max
        view.move_down();
        assert_eq!(view.selected_index, 9);

        view.move_to_top();
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_blame_view_selected_change_id() {
        let mut view = BlameView::new();
        view.set_content(make_test_content(), None);

        assert_eq!(view.selected_change_id(), Some("change01"));

        view.move_down();
        assert_eq!(view.selected_change_id(), Some("change02"));
    }
}
