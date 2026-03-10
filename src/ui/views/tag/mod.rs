//! Tag View for displaying all tags

mod input;
mod render;

use crate::model::TagInfo;
use crate::ui::navigation;

/// Action returned by the Tag View after handling input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagAction {
    /// No action needed
    None,
    /// Jump to tag's commit in Log View (change_id)
    Jump(String),
    /// Create new tag (open input dialog)
    StartCreate,
    /// Delete selected tag (open confirm dialog)
    Delete(String),
}

/// Tag View state
#[derive(Debug)]
pub struct TagView {
    /// All tags (local only)
    tags: Vec<TagInfo>,
    /// Selected index
    selected: usize,
    /// Scroll offset
    scroll_offset: usize,
}

impl Default for TagView {
    fn default() -> Self {
        Self::new()
    }
}

impl TagView {
    /// Create a new Tag View
    pub fn new() -> Self {
        Self {
            tags: Vec::new(),
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Set the tags to display
    pub fn set_tags(&mut self, tags: Vec<TagInfo>) {
        self.tags = tags;
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Get the currently selected tag
    pub fn selected_tag(&self) -> Option<&TagInfo> {
        self.tags.get(self.selected)
    }

    /// Total number of tags
    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }

    /// Move selection to next tag
    pub fn select_next(&mut self) {
        let max = self.tags.len().saturating_sub(1);
        self.selected = navigation::select_next(self.selected, max);
    }

    /// Move selection to previous tag
    pub fn select_prev(&mut self) {
        self.selected = navigation::select_prev(self.selected);
    }

    /// Go to first tag
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Go to last tag
    pub fn select_last(&mut self) {
        if !self.tags.is_empty() {
            self.selected = self.tags.len() - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChangeId, CommitId};
    use crossterm::event::{KeyCode, KeyEvent};

    fn make_tag(name: &str, change_id: Option<&str>, desc: Option<&str>) -> TagInfo {
        TagInfo {
            name: name.to_string(),
            remote: None,
            present: true,
            change_id: change_id.map(|s| ChangeId::new(s.to_string())),
            commit_id: Some(CommitId::new("abcd1234".to_string())),
            description: desc.map(|s| s.to_string()),
        }
    }

    fn create_test_tags() -> Vec<TagInfo> {
        vec![
            make_tag("v0.4.10", Some("mzslzzzz"), Some("fix: preview pane")),
            make_tag("v0.4.9", Some("swknqzvs"), Some("feat: highlight")),
            make_tag("v0.4.8", Some("qknsuxln"), Some("fix: notification")),
        ]
    }

    #[test]
    fn test_new_tag_view() {
        let view = TagView::new();
        assert!(view.tags.is_empty());
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_set_tags() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        assert_eq!(view.tag_count(), 3);
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_selected_tag() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        let selected = view.selected_tag().unwrap();
        assert_eq!(selected.name, "v0.4.10");
    }

    #[test]
    fn test_navigation() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        assert_eq!(view.selected, 0);

        view.select_next();
        assert_eq!(view.selected, 1);
        assert_eq!(view.selected_tag().unwrap().name, "v0.4.9");

        view.select_next();
        assert_eq!(view.selected, 2);

        // At end, should not advance
        view.select_next();
        assert_eq!(view.selected, 2);

        view.select_prev();
        assert_eq!(view.selected, 1);
    }

    #[test]
    fn test_select_first_last() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());

        view.select_last();
        assert_eq!(view.selected_tag().unwrap().name, "v0.4.8");

        view.select_first();
        assert_eq!(view.selected_tag().unwrap().name, "v0.4.10");
    }

    #[test]
    fn test_empty_tags() {
        let mut view = TagView::new();
        view.set_tags(vec![]);
        assert_eq!(view.tag_count(), 0);
        assert!(view.selected_tag().is_none());
    }

    #[test]
    fn test_handle_key_enter_jumpable() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(action, TagAction::Jump(id) if id == "mzslzzzz"));
    }

    #[test]
    fn test_handle_key_enter_not_jumpable() {
        let mut view = TagView::new();
        view.set_tags(vec![TagInfo {
            name: "v0.1".into(),
            remote: None,
            present: true,
            change_id: None,
            commit_id: None,
            description: None,
        }]);
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(action, TagAction::None));
    }

    #[test]
    fn test_handle_key_create() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('c')));
        assert!(matches!(action, TagAction::StartCreate));
    }

    #[test]
    fn test_handle_key_delete() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('D')));
        assert!(matches!(action, TagAction::Delete(name) if name == "v0.4.10"));
    }

    #[test]
    fn test_handle_key_delete_empty() {
        let mut view = TagView::new();
        view.set_tags(vec![]);
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('D')));
        assert!(matches!(action, TagAction::None));
    }

    #[test]
    fn test_handle_key_navigation_j_k() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('j')));
        assert!(matches!(action, TagAction::None));
        assert_eq!(view.selected, 1);

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('k')));
        assert!(matches!(action, TagAction::None));
        assert_eq!(view.selected, 0);
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_handle_key_g_G() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());

        view.handle_key(KeyEvent::from(KeyCode::Char('G')));
        assert_eq!(view.selected, 2);

        view.handle_key(KeyEvent::from(KeyCode::Char('g')));
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_set_tags_resets_selection() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        view.select_last();
        assert_eq!(view.selected, 2);

        // Setting new tags should reset selection
        view.set_tags(create_test_tags());
        assert_eq!(view.selected, 0);
    }
}
