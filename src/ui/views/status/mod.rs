//! Status View
//!
//! Displays the current working copy status with changed files.

mod input;
mod render;

use crate::model::{FileState, Status};
use crate::ui::navigation;

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
    /// Show blame/annotation for selected file
    OpenBlame {
        /// File path to annotate
        file_path: String,
    },
    /// Commit with message
    Commit { message: String },
    /// Jump to first conflict file
    JumpToConflict,
    /// No action
    None,
}

/// Status View state
#[derive(Debug)]
pub struct StatusView {
    /// Current status (None if not loaded)
    pub(super) status: Option<Status>,

    /// Selected file index
    pub(super) selected_index: usize,

    /// Scroll offset for display
    pub(super) scroll_offset: usize,

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
    /// Default visible count for scroll calculations
    pub(super) const DEFAULT_VISIBLE_COUNT: usize = 20;

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

    /// Check if there are any conflicts in the current status
    #[allow(dead_code)] // Phase 9: conflict resolution
    pub fn has_conflicts(&self) -> bool {
        self.status.as_ref().is_some_and(|s| s.has_conflicts)
    }

    /// Jump to the first conflicted file in the list
    ///
    /// Returns true if a conflict file was found and selection moved.
    fn jump_to_first_conflict(&mut self) -> bool {
        if let Some(ref status) = self.status
            && let Some(idx) = status
                .files
                .iter()
                .position(|f| matches!(f.state, FileState::Conflicted))
        {
            self.selected_index = idx;
            self.scroll_offset = navigation::adjust_scroll(
                self.selected_index,
                self.scroll_offset,
                Self::DEFAULT_VISIBLE_COUNT,
            );
            return true;
        }
        false
    }

    /// Move selection down
    fn move_down(&mut self, visible_count: usize) {
        if let Some(ref status) = self.status {
            let max = status.files.len().saturating_sub(1);
            self.selected_index = navigation::select_next(self.selected_index, max);
            self.scroll_offset =
                navigation::adjust_scroll(self.selected_index, self.scroll_offset, visible_count);
        }
    }

    /// Move selection up
    fn move_up(&mut self, visible_count: usize) {
        self.selected_index = navigation::select_prev(self.selected_index);
        self.scroll_offset =
            navigation::adjust_scroll(self.selected_index, self.scroll_offset, visible_count);
    }

    /// Jump to top
    fn jump_to_top(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Jump to bottom
    fn jump_to_bottom(&mut self, visible_count: usize) {
        if let Some(ref status) = self.status
            && !status.files.is_empty()
        {
            self.selected_index = status.files.len() - 1;
            self.scroll_offset =
                navigation::adjust_scroll(self.selected_index, self.scroll_offset, visible_count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FileStatus;
    use crossterm::event::{KeyCode, KeyEvent};

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

    #[test]
    fn test_has_conflicts() {
        let mut view = StatusView::new();

        // No status set - no conflicts
        assert!(!view.has_conflicts());

        // Status without conflicts
        let no_conflict_status = Status {
            files: vec![],
            has_conflicts: false,
            working_copy_change_id: "abc".to_string(),
            parent_change_id: "xyz".to_string(),
        };
        view.set_status(no_conflict_status);
        assert!(!view.has_conflicts());

        // Status with conflicts
        let conflict_status = Status {
            files: vec![],
            has_conflicts: true,
            working_copy_change_id: "abc".to_string(),
            parent_change_id: "xyz".to_string(),
        };
        view.set_status(conflict_status);
        assert!(view.has_conflicts());
    }

    fn status_with_conflicts() -> Status {
        Status {
            files: vec![
                FileStatus {
                    path: "src/main.rs".to_string(),
                    state: FileState::Modified,
                },
                FileStatus {
                    path: "src/conflict.rs".to_string(),
                    state: FileState::Conflicted,
                },
                FileStatus {
                    path: "src/other.rs".to_string(),
                    state: FileState::Added,
                },
            ],
            has_conflicts: true,
            working_copy_change_id: "abc12345".to_string(),
            parent_change_id: "xyz98765".to_string(),
        }
    }

    #[test]
    fn test_jump_to_first_conflict() {
        let mut view = StatusView::new();
        view.set_status(status_with_conflicts());

        assert_eq!(view.selected_index, 0);
        assert!(view.jump_to_first_conflict());
        assert_eq!(view.selected_index, 1); // conflict.rs is at index 1
    }

    #[test]
    fn test_jump_to_first_conflict_no_conflicts() {
        let mut view = StatusView::new();
        view.set_status(sample_status()); // no conflicted files

        assert_eq!(view.selected_index, 0);
        assert!(!view.jump_to_first_conflict());
        assert_eq!(view.selected_index, 0); // unchanged
    }

    #[test]
    fn test_f_key_with_conflicts() {
        let mut view = StatusView::new();
        view.set_status(status_with_conflicts());

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('f')));
        assert_eq!(action, StatusAction::JumpToConflict);
        assert_eq!(view.selected_index, 1);
    }

    #[test]
    fn test_f_key_without_conflicts() {
        let mut view = StatusView::new();
        view.set_status(sample_status());

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('f')));
        assert_eq!(action, StatusAction::None);
        assert_eq!(view.selected_index, 0);
    }
}
