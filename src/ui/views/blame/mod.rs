//! Blame View - Shows file annotation (blame) information
//!
//! Displays which change is responsible for each line of a file.

mod input;
mod render;

use crate::model::AnnotationContent;
use crate::ui::navigation;

/// Action returned by BlameView input handling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlameAction {
    /// No action needed
    None,
    /// Go back to previous view
    Back,
    /// Open diff for the selected change
    OpenDiff(String),
    /// Jump to this change in Log View
    JumpToLog(String),
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
    /// Agent Trace AI badges keyed by line commit_id (Phase 4a).
    /// Empty = no overlay (the AI column is not rendered at all).
    ai_badges: crate::trace::AiBadgeSets,
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
            ai_badges: crate::trace::AiBadgeSets::default(),
        }
    }

    /// Set the annotation content with optional revision
    pub fn set_content(&mut self, content: AnnotationContent, revision: Option<String>) {
        self.content = content;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.revision = revision;
        // Badges index into the old content; cleared until App recomputes
        self.ai_badges = crate::trace::AiBadgeSets::default();
    }

    /// Set Agent Trace AI badges (Phase 4a; recomputed by App on open_blame)
    pub fn set_ai_badges(&mut self, badges: crate::trace::AiBadgeSets) {
        self.ai_badges = badges;
    }

    /// Current Agent Trace AI badges (for tests / introspection)
    pub fn ai_badges(&self) -> &crate::trace::AiBadgeSets {
        &self.ai_badges
    }

    /// (change_id, commit_id) pairs for every annotation line — for App to
    /// build the badge sets without reaching into `content`.
    pub fn line_revisions(&self) -> Vec<(&str, &str)> {
        self.content
            .lines
            .iter()
            .map(|l| (l.change_id.as_str(), l.commit_id.as_str()))
            .collect()
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

    /// Get the currently selected line's change_id (for UI identification)
    pub fn selected_change_id(&self) -> Option<&str> {
        self.content
            .lines
            .get(self.selected_index)
            .map(|line| line.change_id.as_str())
    }

    /// Get the currently selected line's commit_id (for jj command execution)
    pub fn selected_commit_id(&self) -> Option<&str> {
        self.content
            .lines
            .get(self.selected_index)
            .map(|line| line.commit_id.as_str())
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        let max = self.content.len().saturating_sub(1);
        self.selected_index = navigation::select_next(self.selected_index, max);
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        self.selected_index = navigation::select_prev(self.selected_index);
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

    /// Calculate scroll offset to keep selection visible (used at render time)
    fn calculate_scroll_offset(&self, visible_height: usize) -> usize {
        navigation::adjust_scroll(self.selected_index, self.scroll_offset, visible_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AnnotationLine, ChangeId, CommitId};

    fn make_test_content() -> AnnotationContent {
        let mut content = AnnotationContent::new("test.rs".to_string());
        for i in 1..=10 {
            content.lines.push(AnnotationLine {
                change_id: ChangeId::new(format!("change{:02}", i)),
                commit_id: CommitId::new(format!("commit{:02}", i)),
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
    fn line_revisions_pairs_change_and_commit() {
        let mut view = BlameView::new();
        view.set_content(make_test_content(), None);
        let revs = view.line_revisions();
        assert_eq!(revs.len(), 10);
        assert_eq!(revs[0], ("change01", "commit01"));
        assert_eq!(revs[9], ("change10", "commit10"));
    }

    #[test]
    fn set_content_clears_ai_badges() {
        let mut view = BlameView::new();
        let mut badges = crate::trace::AiBadgeSets::default();
        badges.confirmed.insert("commit01".to_string());
        view.set_ai_badges(badges);
        assert!(!view.ai_badges.is_empty());

        // New annotation content invalidates badges (they index old lines)
        view.set_content(make_test_content(), None);
        assert!(view.ai_badges.is_empty());
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
