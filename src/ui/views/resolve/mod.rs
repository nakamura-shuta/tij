//! Resolve List View - Shows conflict files and resolution options
//!
//! Displays `jj resolve --list` output and provides tools to resolve conflicts.

mod input;
mod render;

use crate::model::ConflictFile;

/// Action returned by ResolveView input handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveAction {
    /// No action needed
    None,
    /// Go back to previous view
    Back,
    /// Resolve selected file with external merge tool (@ only)
    ResolveExternal(String),
    /// Resolve selected file with :ours
    ResolveOurs(String),
    /// Resolve selected file with :theirs
    ResolveTheirs(String),
    /// Show diff for selected file
    ShowDiff(String),
}

/// View state for conflict resolution
#[derive(Debug, Clone)]
pub struct ResolveView {
    /// Target change ID
    pub change_id: String,
    /// Whether the target is the working copy (@)
    pub is_working_copy: bool,
    /// List of conflict files
    files: Vec<ConflictFile>,
    /// Currently selected file index
    selected_index: usize,
    /// Scroll offset for display
    scroll_offset: usize,
}

impl ResolveView {
    /// Create a new resolve view for a change
    pub fn new(change_id: String, is_working_copy: bool, files: Vec<ConflictFile>) -> Self {
        Self {
            change_id,
            is_working_copy,
            files,
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    /// Get the conflict files
    #[allow(dead_code)] // public API accessor
    pub fn files(&self) -> &[ConflictFile] {
        &self.files
    }

    /// Get the number of conflict files
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Check if the view is empty
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Get the currently selected file path
    pub fn selected_file_path(&self) -> Option<&str> {
        self.files.get(self.selected_index).map(|f| f.path.as_str())
    }

    /// Update the file list (after resolving a conflict)
    pub fn set_files(&mut self, files: Vec<ConflictFile>) {
        self.files = files;
        // Clamp selected_index
        if !self.files.is_empty() {
            self.selected_index = self.selected_index.min(self.files.len() - 1);
        } else {
            self.selected_index = 0;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if !self.files.is_empty() && self.selected_index < self.files.len() - 1 {
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
        if !self.files.is_empty() {
            self.selected_index = self.files.len() - 1;
        }
    }

    /// Calculate scroll offset to keep selection visible
    fn calculate_scroll_offset(&self, visible_height: usize) -> usize {
        if visible_height == 0 {
            return 0;
        }

        let mut offset = self.scroll_offset;

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

    fn make_test_files() -> Vec<ConflictFile> {
        vec![
            ConflictFile {
                path: "test.txt".to_string(),
                description: "2-sided conflict".to_string(),
            },
            ConflictFile {
                path: "src/main.rs".to_string(),
                description: "2-sided conflict".to_string(),
            },
            ConflictFile {
                path: "src/lib.rs".to_string(),
                description: "3-sided conflict".to_string(),
            },
        ]
    }

    #[test]
    fn test_resolve_view_new() {
        let view = ResolveView::new("abc12345".to_string(), true, make_test_files());
        assert_eq!(view.change_id, "abc12345");
        assert!(view.is_working_copy);
        assert_eq!(view.file_count(), 3);
        assert!(!view.is_empty());
    }

    #[test]
    fn test_resolve_view_empty() {
        let view = ResolveView::new("abc12345".to_string(), false, vec![]);
        assert!(view.is_empty());
        assert_eq!(view.file_count(), 0);
        assert_eq!(view.selected_file_path(), None);
    }

    #[test]
    fn test_resolve_view_navigation() {
        let mut view = ResolveView::new("abc12345".to_string(), true, make_test_files());

        assert_eq!(view.selected_file_path(), Some("test.txt"));

        view.move_down();
        assert_eq!(view.selected_file_path(), Some("src/main.rs"));

        view.move_down();
        assert_eq!(view.selected_file_path(), Some("src/lib.rs"));

        // Can't go below max
        view.move_down();
        assert_eq!(view.selected_file_path(), Some("src/lib.rs"));

        view.move_up();
        assert_eq!(view.selected_file_path(), Some("src/main.rs"));

        view.move_to_top();
        assert_eq!(view.selected_file_path(), Some("test.txt"));

        view.move_to_bottom();
        assert_eq!(view.selected_file_path(), Some("src/lib.rs"));
    }

    #[test]
    fn test_resolve_view_set_files() {
        let mut view = ResolveView::new("abc12345".to_string(), true, make_test_files());
        view.move_to_bottom(); // index = 2

        // Update with fewer files
        view.set_files(vec![ConflictFile {
            path: "remaining.txt".to_string(),
            description: "2-sided conflict".to_string(),
        }]);

        // Selected index clamped to new length
        assert_eq!(view.selected_index, 0);
        assert_eq!(view.selected_file_path(), Some("remaining.txt"));
    }

    #[test]
    fn test_resolve_view_set_files_empty() {
        let mut view = ResolveView::new("abc12345".to_string(), true, make_test_files());
        view.set_files(vec![]);
        assert!(view.is_empty());
        assert_eq!(view.selected_index, 0);
    }

    #[test]
    fn test_resolve_view_not_working_copy() {
        let view = ResolveView::new("lqwwsqpm".to_string(), false, make_test_files());
        assert!(!view.is_working_copy);
        assert_eq!(view.change_id, "lqwwsqpm");
    }
}
