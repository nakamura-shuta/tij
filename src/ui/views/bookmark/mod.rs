//! Bookmark View for displaying all bookmarks grouped by type

mod input;
mod render;

use crate::model::BookmarkInfo;
use crate::ui::navigation;

/// Action returned by the Bookmark View after handling input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookmarkAction {
    /// No action needed
    None,
    /// Jump to bookmark's change in Log View (change_id)
    Jump(String),
    /// Track selected remote bookmark (full_name)
    Track(String),
    /// Untrack selected tracked remote bookmark (full_name)
    Untrack(String),
    /// Delete selected local bookmark (name)
    Delete(String),
}

/// Display row type for rendering
#[derive(Debug, Clone)]
pub(super) enum DisplayRow {
    /// Group header (e.g., "── Local ──")
    Header(String),
    /// Bookmark entry (index into BookmarkView.bookmarks)
    Bookmark(usize),
}

/// Bookmark View state
#[derive(Debug)]
pub struct BookmarkView {
    /// All bookmarks with info
    bookmarks: Vec<BookmarkInfo>,
    /// Display rows (headers + bookmark indices)
    display_rows: Vec<DisplayRow>,
    /// Selected row index (within display_rows, only Bookmark rows are selectable)
    selected: usize,
    /// Scroll offset
    scroll_offset: usize,
}

impl Default for BookmarkView {
    fn default() -> Self {
        Self::new()
    }
}

impl BookmarkView {
    /// Create a new Bookmark View
    pub fn new() -> Self {
        Self {
            bookmarks: Vec::new(),
            display_rows: Vec::new(),
            selected: 0,
            scroll_offset: 0,
        }
    }

    /// Set the bookmarks to display, sorted and grouped
    pub fn set_bookmarks(&mut self, mut bookmarks: Vec<BookmarkInfo>) {
        // Filter out @git remote entries (internal jj representation)
        bookmarks.retain(|b| b.bookmark.remote.as_deref() != Some("git"));

        // Sort: local first, then tracked remote, then untracked remote
        // Within each group, sort alphabetically
        bookmarks.sort_by(|a, b| {
            let group_a = bookmark_group_order(&a.bookmark);
            let group_b = bookmark_group_order(&b.bookmark);
            group_a
                .cmp(&group_b)
                .then(a.bookmark.full_name().cmp(&b.bookmark.full_name()))
        });

        // Build display rows with headers
        let mut rows = Vec::new();
        let mut current_group = None;

        for (idx, info) in bookmarks.iter().enumerate() {
            let group = bookmark_group_order(&info.bookmark);
            if current_group != Some(group) {
                current_group = Some(group);
                let header = match group {
                    0 => "── Local ──",
                    1 => "── Remote (tracked) ──",
                    2 => "── Remote (untracked) ──",
                    _ => "── Other ──",
                };
                rows.push(DisplayRow::Header(header.to_string()));
            }
            rows.push(DisplayRow::Bookmark(idx));
        }

        self.bookmarks = bookmarks;
        self.display_rows = rows;
        self.selected = self.first_bookmark_row().unwrap_or(0);
        self.scroll_offset = 0;
    }

    /// Get the currently selected bookmark
    pub fn selected_bookmark(&self) -> Option<&BookmarkInfo> {
        if let Some(DisplayRow::Bookmark(idx)) = self.display_rows.get(self.selected) {
            self.bookmarks.get(*idx)
        } else {
            None
        }
    }

    /// Total number of bookmarks (excluding headers)
    pub fn bookmark_count(&self) -> usize {
        self.bookmarks.len()
    }

    /// Move selection to next bookmark row (skip headers)
    pub fn select_next(&mut self) {
        let max = self.display_rows.len().saturating_sub(1);
        let mut next = navigation::select_next(self.selected, max);
        while next <= max {
            if matches!(self.display_rows.get(next), Some(DisplayRow::Bookmark(_))) {
                break;
            }
            if next == max {
                return;
            }
            next = navigation::select_next(next, max);
        }
        self.selected = next;
    }

    /// Move selection to previous bookmark row (skip headers)
    pub fn select_prev(&mut self) {
        let mut prev = navigation::select_prev(self.selected);
        loop {
            if matches!(self.display_rows.get(prev), Some(DisplayRow::Bookmark(_))) {
                break;
            }
            if prev == 0 {
                return;
            }
            prev = navigation::select_prev(prev);
        }
        self.selected = prev;
    }

    /// Go to first bookmark row
    pub fn select_first(&mut self) {
        if let Some(first) = self.first_bookmark_row() {
            self.selected = first;
            self.scroll_offset = 0;
        }
    }

    /// Go to last bookmark row
    pub fn select_last(&mut self) {
        if let Some(last) = self.last_bookmark_row() {
            self.selected = last;
        }
    }

    fn first_bookmark_row(&self) -> Option<usize> {
        self.display_rows
            .iter()
            .position(|r| matches!(r, DisplayRow::Bookmark(_)))
    }

    fn last_bookmark_row(&self) -> Option<usize> {
        self.display_rows
            .iter()
            .rposition(|r| matches!(r, DisplayRow::Bookmark(_)))
    }
}

/// Return sort order: 0=local, 1=tracked remote, 2=untracked remote
fn bookmark_group_order(bookmark: &crate::model::Bookmark) -> u8 {
    if bookmark.remote.is_none() {
        0
    } else if bookmark.is_tracked {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Bookmark;
    use crossterm::event::{KeyCode, KeyEvent};

    fn make_local(name: &str, change_id: Option<&str>, desc: Option<&str>) -> BookmarkInfo {
        BookmarkInfo {
            bookmark: Bookmark {
                name: name.to_string(),
                remote: None,
                is_tracked: false,
            },
            change_id: change_id.map(|s| s.to_string()),
            commit_id: None,
            description: desc.map(|s| s.to_string()),
        }
    }

    fn make_tracked_remote(name: &str, remote: &str) -> BookmarkInfo {
        BookmarkInfo {
            bookmark: Bookmark {
                name: name.to_string(),
                remote: Some(remote.to_string()),
                is_tracked: true,
            },
            change_id: None,
            commit_id: None,
            description: None,
        }
    }

    fn make_untracked_remote(name: &str, remote: &str) -> BookmarkInfo {
        BookmarkInfo {
            bookmark: Bookmark {
                name: name.to_string(),
                remote: Some(remote.to_string()),
                is_tracked: false,
            },
            change_id: None,
            commit_id: None,
            description: None,
        }
    }

    fn make_git_remote(name: &str) -> BookmarkInfo {
        BookmarkInfo {
            bookmark: Bookmark {
                name: name.to_string(),
                remote: Some("git".to_string()),
                is_tracked: true,
            },
            change_id: None,
            commit_id: None,
            description: None,
        }
    }

    fn create_test_bookmarks() -> Vec<BookmarkInfo> {
        vec![
            make_local("main", Some("abc12345"), Some("Fix critical bug")),
            make_local("feature-x", Some("yolqpmvr"), Some("Add new feature")),
            make_tracked_remote("main", "origin"),
            make_tracked_remote("feature-x", "origin"),
            make_untracked_remote("dependabot/cargo", "origin"),
            make_git_remote("main"),
            make_git_remote("feature-x"),
        ]
    }

    #[test]
    fn test_new_bookmark_view() {
        let view = BookmarkView::new();
        assert!(view.bookmarks.is_empty());
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_set_bookmarks_filters_git_remotes() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        assert_eq!(view.bookmark_count(), 5);
    }

    #[test]
    fn test_set_bookmarks_group_order() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        assert_eq!(view.display_rows.len(), 8);
        assert!(matches!(&view.display_rows[0], DisplayRow::Header(h) if h.contains("Local")));
        assert!(matches!(&view.display_rows[3], DisplayRow::Header(h) if h.contains("tracked")));
        assert!(matches!(&view.display_rows[6], DisplayRow::Header(h) if h.contains("untracked")));
    }

    #[test]
    fn test_set_bookmarks_alphabetical_within_group() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        if let DisplayRow::Bookmark(idx) = &view.display_rows[1] {
            assert_eq!(view.bookmarks[*idx].bookmark.name, "feature-x");
        }
        if let DisplayRow::Bookmark(idx) = &view.display_rows[2] {
            assert_eq!(view.bookmarks[*idx].bookmark.name, "main");
        }
    }

    #[test]
    fn test_selected_bookmark() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        let selected = view.selected_bookmark().unwrap();
        assert_eq!(selected.bookmark.name, "feature-x");
        assert!(selected.bookmark.remote.is_none());
    }

    #[test]
    fn test_navigation_skip_headers() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        assert_eq!(view.selected, 1);
        view.select_next();
        assert_eq!(view.selected, 2);
        view.select_next();
        assert_eq!(view.selected, 4); // skips header at 3
        view.select_prev();
        assert_eq!(view.selected, 2);
    }

    #[test]
    fn test_select_first_last() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        view.select_last();
        assert_eq!(
            view.selected_bookmark().unwrap().bookmark.name,
            "dependabot/cargo"
        );
        view.select_first();
        assert_eq!(view.selected_bookmark().unwrap().bookmark.name, "feature-x");
    }

    #[test]
    fn test_empty_bookmarks() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(vec![]);
        assert_eq!(view.bookmark_count(), 0);
        assert!(view.selected_bookmark().is_none());
    }

    #[test]
    fn test_handle_key_enter_jumpable() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(action, BookmarkAction::Jump(id) if id == "yolqpmvr"));
    }

    #[test]
    fn test_handle_key_enter_not_jumpable() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        view.select_next();
        view.select_next();
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(action, BookmarkAction::None));
    }

    #[test]
    fn test_handle_key_track() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        view.select_last();
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('T')));
        assert!(matches!(action, BookmarkAction::Track(n) if n == "dependabot/cargo@origin"));
    }

    #[test]
    fn test_handle_key_track_on_local_noop() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('T')));
        assert!(matches!(action, BookmarkAction::None));
    }

    #[test]
    fn test_handle_key_untrack() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        view.select_next();
        view.select_next(); // tracked remote
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('U')));
        assert!(matches!(action, BookmarkAction::Untrack(n) if n == "feature-x@origin"));
    }

    #[test]
    fn test_handle_key_untrack_on_local_noop() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('U')));
        assert!(matches!(action, BookmarkAction::None));
    }

    #[test]
    fn test_handle_key_untrack_on_untracked_noop() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        view.select_last();
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('U')));
        assert!(matches!(action, BookmarkAction::None));
    }

    #[test]
    fn test_handle_key_delete() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('D')));
        assert!(matches!(action, BookmarkAction::Delete(n) if n == "feature-x"));
    }

    #[test]
    fn test_handle_key_delete_on_remote_noop() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        view.select_next();
        view.select_next();
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('D')));
        assert!(matches!(action, BookmarkAction::None));
    }

    #[test]
    fn test_only_locals_group() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(vec![
            make_local("main", Some("abc"), Some("desc")),
            make_local("dev", Some("def"), Some("dev branch")),
        ]);
        assert_eq!(view.display_rows.len(), 3);
        assert_eq!(view.bookmark_count(), 2);
    }
}
