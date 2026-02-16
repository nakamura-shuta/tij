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
    /// Start rename input mode (old_name)
    StartRename(String),
    /// Confirm rename (old_name, new_name)
    ConfirmRename { old_name: String, new_name: String },
    /// Cancel rename
    CancelRename,
    /// Forget bookmark (name) - removes remote tracking info
    Forget(String),
    /// Move bookmark to working copy (name)
    Move(String),
    /// Move attempted on remote bookmark (show info notification)
    MoveUnavailable,
}

/// Bookmark rename inline edit state
#[derive(Debug, Clone)]
pub struct RenameState {
    /// Original bookmark name
    pub old_name: String,
    /// Current input buffer
    pub input_buffer: String,
    /// Cursor position in char count (NOT byte offset)
    pub cursor: usize,
}

impl RenameState {
    pub fn new(old_name: String) -> Self {
        let cursor = old_name.chars().count();
        Self {
            input_buffer: old_name.clone(),
            old_name,
            cursor,
        }
    }

    /// Get byte offset for current cursor position (char-based → byte-based)
    fn cursor_byte_offset(&self) -> usize {
        self.input_buffer
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.input_buffer.len())
    }

    /// Remove the character before the cursor (backspace)
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            // Find the byte range of the char before cursor
            let byte_offset = self
                .input_buffer
                .char_indices()
                .nth(self.cursor - 1)
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input_buffer.remove(byte_offset);
            self.cursor -= 1;
        }
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        let byte_offset = self.cursor_byte_offset();
        self.input_buffer.insert(byte_offset, c);
        self.cursor += 1;
    }
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
    /// Rename input state (Some = rename mode active)
    pub(crate) rename_state: Option<RenameState>,
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
            rename_state: None,
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

    // --- Rename action tests ---

    #[test]
    fn test_rename_action_local_only() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        // First selection is a local bookmark (feature-x)
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('r')));
        assert!(matches!(action, BookmarkAction::StartRename(name) if name == "feature-x"));
    }

    #[test]
    fn test_rename_action_remote_ignored() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        // Navigate to tracked remote
        view.select_next();
        view.select_next(); // tracked remote
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('r')));
        assert!(matches!(action, BookmarkAction::None));
    }

    #[test]
    fn test_rename_input_confirm() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        // Start rename
        view.rename_state = Some(RenameState::new("feature-x".to_string()));
        // Type backspace to delete last char
        view.handle_key(KeyEvent::from(KeyCode::Backspace));
        // Type 'y'
        view.handle_key(KeyEvent::from(KeyCode::Char('y')));
        // Confirm
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(
            matches!(action, BookmarkAction::ConfirmRename { old_name, new_name }
                if old_name == "feature-x" && new_name == "feature-y")
        );
        assert!(view.rename_state.is_none());
    }

    #[test]
    fn test_rename_input_cancel() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        view.rename_state = Some(RenameState::new("feature-x".to_string()));
        let action = view.handle_key(KeyEvent::from(KeyCode::Esc));
        assert!(matches!(action, BookmarkAction::CancelRename));
        assert!(view.rename_state.is_none());
    }

    // --- Forget action tests ---

    #[test]
    fn test_forget_action_local_only() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('f')));
        assert!(matches!(action, BookmarkAction::Forget(name) if name == "feature-x"));
    }

    #[test]
    fn test_forget_action_remote_ignored() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        // Navigate to tracked remote
        view.select_next();
        view.select_next();
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('f')));
        assert!(matches!(action, BookmarkAction::None));
    }

    #[test]
    fn test_forget_action_untracked_remote_ignored() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        view.select_last(); // untracked remote
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('f')));
        assert!(matches!(action, BookmarkAction::None));
    }

    // --- Non-ASCII rename safety tests ---

    #[test]
    fn test_rename_non_ascii_backspace_no_panic() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(vec![make_local(
            "機能ブランチ",
            Some("abc12345"),
            Some("desc"),
        )]);
        // Start rename with Japanese name (6 chars, 18 bytes)
        view.rename_state = Some(RenameState::new("機能ブランチ".to_string()));
        // Backspace should remove last *char*, not last byte
        view.handle_key(KeyEvent::from(KeyCode::Backspace));
        let state = view.rename_state.as_ref().unwrap();
        assert_eq!(state.input_buffer, "機能ブラン");
        assert_eq!(state.cursor, 5);

        // Another backspace
        view.handle_key(KeyEvent::from(KeyCode::Backspace));
        let state = view.rename_state.as_ref().unwrap();
        assert_eq!(state.input_buffer, "機能ブラ");
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn test_rename_non_ascii_insert_char() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(vec![make_local("テスト", Some("abc12345"), Some("desc"))]);
        view.rename_state = Some(RenameState::new("テスト".to_string()));
        // Insert ASCII char at end
        view.handle_key(KeyEvent::from(KeyCode::Char('2')));
        let state = view.rename_state.as_ref().unwrap();
        assert_eq!(state.input_buffer, "テスト2");
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn test_rename_emoji_backspace() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(vec![make_local("feat-🚀", Some("abc12345"), Some("desc"))]);
        view.rename_state = Some(RenameState::new("feat-🚀".to_string()));
        // Backspace should remove the emoji (4 bytes), not just 1 byte
        view.handle_key(KeyEvent::from(KeyCode::Backspace));
        let state = view.rename_state.as_ref().unwrap();
        assert_eq!(state.input_buffer, "feat-");
        assert_eq!(state.cursor, 5);
    }

    #[test]
    fn test_rename_state_cursor_char_count() {
        // Verify cursor is char count, not byte length
        let state = RenameState::new("日本語".to_string());
        assert_eq!(state.cursor, 3); // 3 chars, not 9 bytes
        assert_eq!(state.input_buffer.len(), 9); // 9 bytes
    }

    #[test]
    fn test_non_ascii_bookmark_names() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(vec![
            make_local("機能ブランチ", Some("abc12345"), Some("日本語の説明")),
            make_local("feat-🚀-rocket", Some("def67890"), Some("Emoji branch")),
            make_untracked_remote("功能分支", "origin"),
        ]);
        assert_eq!(view.bookmark_count(), 3);
        // Verify navigation works with non-ASCII names
        let selected = view.selected_bookmark().unwrap();
        assert_eq!(selected.bookmark.name, "feat-🚀-rocket");
        view.select_next();
        let selected = view.selected_bookmark().unwrap();
        assert_eq!(selected.bookmark.name, "機能ブランチ");
    }

    // --- Move action tests ---

    #[test]
    fn test_move_action_local_bookmark() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        // First selection is a local bookmark (feature-x)
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('m')));
        assert!(matches!(action, BookmarkAction::Move(name) if name == "feature-x"));
    }

    #[test]
    fn test_move_action_remote_shows_unavailable() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        // Navigate to tracked remote
        view.select_next();
        view.select_next(); // tracked remote
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('m')));
        assert!(matches!(action, BookmarkAction::MoveUnavailable));
    }

    #[test]
    fn test_move_action_untracked_remote_shows_unavailable() {
        let mut view = BookmarkView::new();
        view.set_bookmarks(create_test_bookmarks());
        view.select_last(); // untracked remote
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('m')));
        assert!(matches!(action, BookmarkAction::MoveUnavailable));
    }
}
