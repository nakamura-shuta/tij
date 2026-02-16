//! Tests for LogView

use crossterm::event::{KeyCode, KeyEvent};

use super::{InputMode, LogAction, LogView, RebaseMode};
use crate::jj::constants;
use crate::keys;
use crate::model::Change;

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
            graph_prefix: "@  ".to_string(),
            is_graph_only: false,
            has_conflict: false,
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
            graph_prefix: "○  ".to_string(),
            is_graph_only: false,
            has_conflict: false,
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
            graph_prefix: "◆  ".to_string(),
            is_graph_only: false,
            has_conflict: false,
        },
    ]
}

fn press_key(view: &mut LogView, key: KeyCode) -> LogAction {
    view.handle_key(KeyEvent::from(key))
}

fn type_text(view: &mut LogView, text: &str) {
    for c in text.chars() {
        press_key(view, KeyCode::Char(c));
    }
}

fn submit(view: &mut LogView) -> LogAction {
    press_key(view, keys::SUBMIT)
}

fn escape(view: &mut LogView) -> LogAction {
    press_key(view, keys::ESC)
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

    let action = press_key(&mut view, keys::MOVE_DOWN);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.selected_index, 1);

    let action = press_key(&mut view, keys::MOVE_UP);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.selected_index, 0);
}

#[test]
fn test_handle_key_open_diff() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    let action = press_key(&mut view, keys::OPEN_DIFF);
    assert_eq!(action, LogAction::OpenDiff("abc12345".to_string()));
}

#[test]
fn test_handle_key_search_input() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Start search mode with /
    let action = press_key(&mut view, keys::SEARCH_INPUT);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::SearchInput);

    // Type search query
    type_text(&mut view, "Init");
    assert_eq!(view.input_buffer, "Init");

    // Submit - should store query and jump to match
    let action = submit(&mut view);
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
    let action = press_key(&mut view, keys::REVSET_INPUT);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::RevsetInput);

    // Type revset
    type_text(&mut view, "all");
    assert_eq!(view.input_buffer, "all");

    // Submit
    let action = submit(&mut view);
    assert_eq!(action, LogAction::ExecuteRevset("all".to_string()));
    assert_eq!(view.input_mode, InputMode::Normal);
    assert!(view.input_buffer.is_empty());
    assert_eq!(view.revset_history, vec!["all".to_string()]);
}

#[test]
fn test_handle_key_revset_cancel() {
    let mut view = LogView::new();

    view.start_revset_input();
    type_text(&mut view, "te");
    assert_eq!(view.input_buffer, "te");

    // Cancel with Esc
    let action = escape(&mut view);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal);
    assert!(view.input_buffer.is_empty());
}

#[test]
fn test_handle_key_backspace() {
    let mut view = LogView::new();
    view.start_revset_input();

    type_text(&mut view, "ab");
    assert_eq!(view.input_buffer, "ab");

    press_key(&mut view, KeyCode::Backspace);
    assert_eq!(view.input_buffer, "a");
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

    let action = press_key(&mut view, keys::SEARCH_NEXT);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.selected_index, 1);
}

#[test]
fn test_handle_key_search_prev() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());
    view.selected_index = 2;
    view.last_search_query = Some("First".to_string());

    let action = press_key(&mut view, keys::SEARCH_PREV);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.selected_index, 0);
}

#[test]
fn test_search_input_stores_query() {
    let mut view = LogView::new();
    view.start_search_input();

    // Type query
    type_text(&mut view, "main");

    // Submit
    submit(&mut view);

    // Should store as search query
    assert_eq!(view.last_search_query, Some("main".to_string()));
}

#[test]
fn test_revset_input_does_not_store_search_query() {
    let mut view = LogView::new();
    view.start_revset_input();

    // Type revset
    type_text(&mut view, "all");

    // Submit
    submit(&mut view);

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
    submit(&mut view);

    // Should clear search query
    assert_eq!(view.last_search_query, None);
}

#[test]
fn test_revset_empty_enter_returns_clear_action() {
    let mut view = LogView::new();

    // Start revset input and submit empty
    view.start_revset_input();
    let action = submit(&mut view);

    // Should return ClearRevset action
    assert_eq!(action, LogAction::ClearRevset);
}

// =============================================================================
// Squash tests (SquashSelect mode)
// =============================================================================

#[test]
fn test_squash_key_enters_select_mode() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    let action = press_key(&mut view, keys::SQUASH);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::SquashSelect);
    assert_eq!(view.squash_source, Some("abc12345".to_string()));
}

#[test]
fn test_squash_select_cancel() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());
    press_key(&mut view, keys::SQUASH);

    let action = press_key(&mut view, keys::ESC);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal);
    assert_eq!(view.squash_source, None);
}

#[test]
fn test_squash_select_confirm() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());
    press_key(&mut view, keys::SQUASH); // Select first change
    press_key(&mut view, keys::MOVE_DOWN); // Move to second

    let action = press_key(&mut view, KeyCode::Enter);
    assert!(matches!(
        action,
        LogAction::SquashInto { source, destination }
        if source == "abc12345" && destination == "xyz98765"
    ));
    assert_eq!(view.input_mode, InputMode::Normal);
}

#[test]
fn test_squash_into_same_revision_blocked() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());
    press_key(&mut view, keys::SQUASH); // Select first change
    // Don't move - try to squash into self

    let action = press_key(&mut view, KeyCode::Enter);
    assert_eq!(action, LogAction::None); // Blocked
    assert_eq!(view.input_mode, InputMode::SquashSelect); // Still in mode
    assert_eq!(view.squash_source, Some("abc12345".to_string())); // Source preserved
}

#[test]
fn test_squash_no_selection() {
    let mut view = LogView::new();
    // Empty changes list

    let action = press_key(&mut view, keys::SQUASH);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal); // Didn't enter mode
    assert_eq!(view.squash_source, None);
}

#[test]
fn test_squash_select_navigation() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter squash mode
    press_key(&mut view, keys::SQUASH);
    assert_eq!(view.input_mode, InputMode::SquashSelect);

    // Should be on first change
    assert_eq!(view.selected_index, 0);

    // Move down
    press_key(&mut view, keys::MOVE_DOWN);
    assert_eq!(view.selected_index, 1);

    // Move up
    press_key(&mut view, keys::MOVE_UP);
    assert_eq!(view.selected_index, 0);

    // Still in SquashSelect mode
    assert_eq!(view.input_mode, InputMode::SquashSelect);
}

#[test]
fn test_squash_select_ignores_other_keys() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter squash mode
    press_key(&mut view, keys::SQUASH);

    // Try pressing other keys - should be ignored
    let action = press_key(&mut view, KeyCode::Char('d')); // describe
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::SquashSelect);

    let action = press_key(&mut view, KeyCode::Char('e')); // edit
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::SquashSelect);

    let action = press_key(&mut view, KeyCode::Char('/')); // search
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::SquashSelect);
}

// =============================================================================
// Abandon tests
// =============================================================================

#[test]
fn test_handle_key_abandon() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Select first change
    assert_eq!(view.selected_change().unwrap().change_id, "abc12345");

    let action = press_key(&mut view, keys::ABANDON);
    assert_eq!(action, LogAction::Abandon("abc12345".to_string()));
}

#[test]
fn test_handle_key_abandon_on_root() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Move to root
    view.move_to_bottom();
    assert_eq!(
        view.selected_change().unwrap().change_id,
        constants::ROOT_CHANGE_ID
    );

    // Should still return action (state.rs will handle the guard)
    let action = press_key(&mut view, keys::ABANDON);
    assert_eq!(
        action,
        LogAction::Abandon(constants::ROOT_CHANGE_ID.to_string())
    );
}

#[test]
fn test_handle_key_abandon_no_selection() {
    let mut view = LogView::new();
    // Empty changes list

    let action = press_key(&mut view, keys::ABANDON);
    assert_eq!(action, LogAction::None);
}

// =============================================================================
// Split tests
// =============================================================================

#[test]
fn test_handle_key_split() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Select first change
    assert_eq!(view.selected_change().unwrap().change_id, "abc12345");

    let action = press_key(&mut view, keys::SPLIT);
    assert_eq!(action, LogAction::Split("abc12345".to_string()));
}

#[test]
fn test_handle_key_split_no_selection() {
    let mut view = LogView::new();
    // Empty changes list

    let action = press_key(&mut view, keys::SPLIT);
    assert_eq!(action, LogAction::None);
}

// =============================================================================
// Bookmark tests
// =============================================================================

#[test]
fn test_handle_key_bookmark_create() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Press b to start bookmark input
    let action = press_key(&mut view, keys::BOOKMARK);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::BookmarkInput);
    assert_eq!(view.editing_change_id, Some("abc12345".to_string()));
}

#[test]
fn test_handle_key_bookmark_create_no_selection() {
    let mut view = LogView::new();
    // Empty changes list

    let action = press_key(&mut view, keys::BOOKMARK);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal); // Should stay in normal mode
}

#[test]
fn test_bookmark_input_submit() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Start bookmark input
    press_key(&mut view, keys::BOOKMARK);
    assert_eq!(view.input_mode, InputMode::BookmarkInput);

    // Type bookmark name
    type_text(&mut view, "my-bookmark");
    assert_eq!(view.input_buffer, "my-bookmark");

    // Submit
    let action = submit(&mut view);
    assert_eq!(
        action,
        LogAction::CreateBookmark {
            change_id: "abc12345".to_string(),
            name: "my-bookmark".to_string()
        }
    );
    assert_eq!(view.input_mode, InputMode::Normal);
    assert!(view.input_buffer.is_empty());
}

#[test]
fn test_bookmark_input_empty_submit_cancels() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Start bookmark input
    press_key(&mut view, keys::BOOKMARK);

    // Submit empty - should cancel
    let action = submit(&mut view);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal);
}

#[test]
fn test_bookmark_input_cancel() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Start bookmark input
    press_key(&mut view, keys::BOOKMARK);
    type_text(&mut view, "test");

    // Cancel with Esc
    let action = escape(&mut view);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal);
    assert!(view.input_buffer.is_empty());
    assert!(view.editing_change_id.is_none());
}

#[test]
fn test_handle_key_bookmark_delete() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Press D to start bookmark delete
    let action = press_key(&mut view, keys::BOOKMARK_DELETE);
    assert_eq!(action, LogAction::StartBookmarkDelete);
}

#[test]
fn test_bookmark_delete_on_change_with_bookmarks() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // First change has "main" bookmark
    assert_eq!(view.selected_change().unwrap().bookmarks, vec!["main"]);

    let action = press_key(&mut view, keys::BOOKMARK_DELETE);
    assert_eq!(action, LogAction::StartBookmarkDelete);
}

#[test]
fn test_bookmark_delete_on_change_without_bookmarks() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Move to second change (no bookmarks)
    view.move_down();
    assert!(view.selected_change().unwrap().bookmarks.is_empty());

    // Should still return action - state.rs handles the "no bookmarks" case
    let action = press_key(&mut view, keys::BOOKMARK_DELETE);
    assert_eq!(action, LogAction::StartBookmarkDelete);
}

// =============================================================================
// Rebase tests
// =============================================================================

#[test]
fn test_rebase_mode_enter() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Press R to enter rebase mode select
    let action = press_key(&mut view, keys::REBASE);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::RebaseModeSelect);
    assert_eq!(view.rebase_source, Some("abc12345".to_string()));
}

#[test]
fn test_rebase_mode_cancel_from_mode_select() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter rebase mode select
    press_key(&mut view, keys::REBASE);
    assert_eq!(view.input_mode, InputMode::RebaseModeSelect);

    // Press Esc to cancel
    let action = press_key(&mut view, keys::ESC);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal);
    assert_eq!(view.rebase_source, None);
}

#[test]
fn test_rebase_mode_cancel_from_rebase_select() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter rebase mode select, then choose revision mode
    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('r'));
    assert_eq!(view.input_mode, InputMode::RebaseSelect);

    // Press Esc to cancel
    let action = press_key(&mut view, keys::ESC);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal);
    assert_eq!(view.rebase_source, None);
}

#[test]
fn test_rebase_mode_navigation() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter rebase mode -> select Revision
    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('r'));

    // Should be on first change
    assert_eq!(view.selected_index, 0);

    // Move down
    press_key(&mut view, keys::MOVE_DOWN);
    assert_eq!(view.selected_index, 1);

    // Move up
    press_key(&mut view, keys::MOVE_UP);
    assert_eq!(view.selected_index, 0);

    // Still in RebaseSelect mode
    assert_eq!(view.input_mode, InputMode::RebaseSelect);
}

#[test]
fn test_rebase_action_revision_mode() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter rebase mode -> select Revision
    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('r'));
    assert_eq!(view.rebase_source, Some("abc12345".to_string()));

    // Move down to second change as destination
    press_key(&mut view, keys::MOVE_DOWN);
    assert_eq!(view.selected_change().unwrap().change_id, "xyz98765");

    // Press Enter to confirm
    let action = press_key(&mut view, KeyCode::Enter);
    assert!(
        matches!(action, LogAction::Rebase { source, destination, mode }
        if source == "abc12345" && destination == "xyz98765" && mode == RebaseMode::Revision)
    );
    assert_eq!(view.input_mode, InputMode::Normal);
}

#[test]
fn test_rebase_select_ignores_other_keys() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter rebase mode -> select Revision
    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('r'));

    // Try pressing other keys - should be ignored
    let action = press_key(&mut view, KeyCode::Char('d')); // describe
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::RebaseSelect);

    let action = press_key(&mut view, KeyCode::Char('e')); // edit
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::RebaseSelect);

    let action = press_key(&mut view, KeyCode::Char('/')); // search
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::RebaseSelect);
}

#[test]
fn test_rebase_mode_select_ignores_unrelated_keys() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter RebaseModeSelect
    press_key(&mut view, keys::REBASE);

    // Try pressing unrelated keys - should be ignored
    let action = press_key(&mut view, KeyCode::Char('d')); // describe
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::RebaseModeSelect);

    let action = press_key(&mut view, KeyCode::Char('j')); // move down
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::RebaseModeSelect);
}

#[test]
fn test_rebase_no_selection() {
    let mut view = LogView::new();
    // Empty changes
    view.set_changes(vec![]);

    // Try to enter rebase mode - should fail silently
    let action = press_key(&mut view, keys::REBASE);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal);
    assert_eq!(view.rebase_source, None);
}

// =============================================================================
// RebaseModeSelect tests (r/s/A/B selection)
// =============================================================================

#[test]
fn test_rebase_mode_select_r_enters_revision() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    press_key(&mut view, keys::REBASE);
    assert_eq!(view.input_mode, InputMode::RebaseModeSelect);

    press_key(&mut view, KeyCode::Char('r'));
    assert_eq!(view.input_mode, InputMode::RebaseSelect);
    assert_eq!(view.rebase_mode, RebaseMode::Revision);
}

#[test]
fn test_rebase_mode_select_s_enters_source() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('s'));
    assert_eq!(view.input_mode, InputMode::RebaseSelect);
    assert_eq!(view.rebase_mode, RebaseMode::Source);
}

#[test]
fn test_rebase_mode_select_a_enters_insert_after() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('A'));
    assert_eq!(view.input_mode, InputMode::RebaseSelect);
    assert_eq!(view.rebase_mode, RebaseMode::InsertAfter);
}

#[test]
fn test_rebase_mode_select_b_enters_insert_before() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('B'));
    assert_eq!(view.input_mode, InputMode::RebaseSelect);
    assert_eq!(view.rebase_mode, RebaseMode::InsertBefore);
}

#[test]
fn test_rebase_source_mode_action() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // R -> s -> navigate -> Enter
    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('s'));
    press_key(&mut view, keys::MOVE_DOWN);

    let action = press_key(&mut view, KeyCode::Enter);
    assert!(
        matches!(action, LogAction::Rebase { source, destination, mode }
        if source == "abc12345" && destination == "xyz98765" && mode == RebaseMode::Source)
    );
}

#[test]
fn test_rebase_insert_after_action() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // R -> A -> navigate -> Enter
    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('A'));
    press_key(&mut view, keys::MOVE_DOWN);

    let action = press_key(&mut view, KeyCode::Enter);
    assert!(
        matches!(action, LogAction::Rebase { source, destination, mode }
        if source == "abc12345" && destination == "xyz98765" && mode == RebaseMode::InsertAfter)
    );
}

#[test]
fn test_rebase_insert_before_action() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // R -> B -> navigate -> Enter
    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('B'));
    press_key(&mut view, keys::MOVE_DOWN);

    let action = press_key(&mut view, KeyCode::Enter);
    assert!(
        matches!(action, LogAction::Rebase { source, destination, mode }
        if source == "abc12345" && destination == "xyz98765" && mode == RebaseMode::InsertBefore)
    );
}

#[test]
fn test_rebase_self_select_blocked_all_modes() {
    // Test that selecting self as destination is blocked in all modes
    for mode_key in ['r', 's', 'A', 'B'] {
        let mut view = LogView::new();
        view.set_changes(create_test_changes());

        press_key(&mut view, keys::REBASE);
        press_key(&mut view, KeyCode::Char(mode_key));
        // Don't move - try to rebase onto self
        let action = press_key(&mut view, KeyCode::Enter);
        assert_eq!(
            action,
            LogAction::None,
            "Self-select should be blocked for mode key '{}'",
            mode_key
        );
        assert_eq!(
            view.input_mode,
            InputMode::RebaseSelect,
            "Should stay in RebaseSelect for mode key '{}'",
            mode_key
        );
    }
}

// =============================================================================
// Absorb tests
// =============================================================================

#[test]
fn test_absorb_key_returns_action() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Press B to trigger absorb
    let action = press_key(&mut view, keys::ABSORB);
    assert!(matches!(action, LogAction::Absorb));
}

#[test]
fn test_absorb_key_works_without_selection() {
    let mut view = LogView::new();
    // Empty changes - absorb should still return action
    // (state.rs handles whether there's anything to absorb)

    let action = press_key(&mut view, keys::ABSORB);
    assert!(matches!(action, LogAction::Absorb));
}

// =============================================================================
// Describe tests (1-line input bar + Ctrl+E external editor)
// =============================================================================

#[test]
fn test_describe_key_returns_start_describe_action() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Press d - should return StartDescribe action (App will fetch full description)
    let action = press_key(&mut view, keys::DESCRIBE);
    assert_eq!(action, LogAction::StartDescribe("abc12345".to_string()));
    // View should NOT yet be in DescribeInput mode (App sets it via set_describe_input)
    assert_eq!(view.input_mode, InputMode::Normal);
}

#[test]
fn test_set_describe_input_prefills_first_line() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Simulate App calling set_describe_input with single-line description
    view.set_describe_input("abc12345".to_string(), "First commit".to_string());

    assert_eq!(view.input_mode, InputMode::DescribeInput);
    assert_eq!(view.editing_change_id, Some("abc12345".to_string()));
    // input_buffer should be prefilled with the description
    assert_eq!(view.input_buffer, "First commit");
}

#[test]
fn test_describe_input_cancel() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Start describe input
    view.set_describe_input("abc12345".to_string(), "Test".to_string());
    assert_eq!(view.input_mode, InputMode::DescribeInput);

    // Cancel with cancel_input
    view.cancel_input();

    assert_eq!(view.input_mode, InputMode::Normal);
    assert!(view.input_buffer.is_empty());
    assert!(view.editing_change_id.is_none());
}

#[test]
fn test_describe_input_escape_cancels() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Start describe input via set_describe_input (simulating App)
    view.set_describe_input("abc12345".to_string(), "Test description".to_string());
    assert_eq!(view.input_mode, InputMode::DescribeInput);

    // Press Esc to cancel
    let action = escape(&mut view);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal);
    assert!(view.input_buffer.is_empty());
}

#[test]
fn test_describe_key_no_selection_returns_none() {
    let mut view = LogView::new();
    // Empty changes

    // Press d - should return None when no change is selected
    let action = press_key(&mut view, keys::DESCRIBE);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal);
}

#[test]
fn test_describe_input_enter_submits() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Start describe input with prefilled text
    view.set_describe_input("abc12345".to_string(), "First commit".to_string());
    assert_eq!(view.editing_change_id, Some("abc12345".to_string()));

    // Press Enter to submit
    let action = submit(&mut view);

    assert!(matches!(
        action,
        LogAction::Describe { change_id, message }
        if change_id == "abc12345" && message == "First commit"
    ));
    assert_eq!(view.input_mode, InputMode::Normal);
    assert!(view.input_buffer.is_empty());
}

#[test]
fn test_describe_input_empty_submit_cancels() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Start describe input with empty text
    view.set_describe_input("abc12345".to_string(), String::new());

    // Submit empty - should cancel (return None)
    let action = submit(&mut view);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::Normal);
}

#[test]
fn test_describe_input_type_and_submit() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Start describe input with empty text
    view.set_describe_input("abc12345".to_string(), String::new());

    // Type new description
    type_text(&mut view, "new desc");
    assert_eq!(view.input_buffer, "new desc");

    // Submit
    let action = submit(&mut view);
    assert!(matches!(
        action,
        LogAction::Describe { change_id, message }
        if change_id == "abc12345" && message == "new desc"
    ));
}

// =============================================================================
// Describe External (Ctrl+E) tests
// =============================================================================

#[test]
fn test_describe_external_key_returns_action() {
    use crossterm::event::KeyModifiers;

    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Press Ctrl+E
    let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
    let action = view.handle_key(key);
    assert_eq!(action, LogAction::DescribeExternal("abc12345".to_string()));
}

#[test]
fn test_describe_external_no_selection_returns_none() {
    use crossterm::event::KeyModifiers;

    let mut view = LogView::new();
    // Empty changes

    let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
    let action = view.handle_key(key);
    assert_eq!(action, LogAction::None);
}

#[test]
fn test_e_without_ctrl_returns_edit_action() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Plain 'e' should be Edit, not DescribeExternal
    let action = press_key(&mut view, keys::EDIT);
    assert_eq!(action, LogAction::Edit("abc12345".to_string()));
}

#[test]
fn test_ctrl_e_in_search_input_mode_ignored() {
    use crossterm::event::KeyModifiers;

    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter search mode
    view.start_search_input();
    assert_eq!(view.input_mode, InputMode::SearchInput);

    // Ctrl+E in search mode should NOT trigger DescribeExternal
    // and should NOT add 'e' to input_buffer
    let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
    let action = view.handle_key(key);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::SearchInput);
    assert!(
        view.input_buffer.is_empty(),
        "Ctrl+E should not add 'e' to input_buffer"
    );
}

#[test]
fn test_ctrl_e_in_describe_input_mode_ignored() {
    use crossterm::event::KeyModifiers;

    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter describe input mode
    view.set_describe_input("abc12345".to_string(), "test".to_string());
    assert_eq!(view.input_mode, InputMode::DescribeInput);

    // Ctrl+E in describe input mode should NOT trigger DescribeExternal
    // and should NOT add 'e' to input_buffer
    let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
    let action = view.handle_key(key);
    assert_eq!(action, LogAction::None);
    assert_eq!(view.input_mode, InputMode::DescribeInput);
    assert_eq!(
        view.input_buffer, "test",
        "Ctrl+E should not modify input_buffer"
    );
}

// =============================================================================
// New from selected (C key) tests
// =============================================================================

#[test]
fn test_new_from_key_returns_action() {
    // create_test_changes() は [working_copy, non_wc, root] を返す
    let mut view = LogView::default();
    view.set_changes(create_test_changes());
    view.selected_index = 1; // non working copy (xyz98765)

    let result = view.handle_key(KeyEvent::from(KeyCode::Char('C')));
    match result {
        LogAction::NewChangeFrom {
            change_id,
            display_name,
        } => {
            assert_eq!(change_id, "xyz98765");
            assert_eq!(display_name, "xyz98765"); // bookmark なし → short_id
        }
        _ => panic!("Expected NewChangeFrom action"),
    }
}

#[test]
fn test_new_from_key_on_working_copy() {
    let mut view = LogView::default();
    view.set_changes(create_test_changes());
    view.selected_index = 0; // working copy

    let result = view.handle_key(KeyEvent::from(KeyCode::Char('C')));
    assert!(matches!(result, LogAction::NewChangeFromCurrent));
}

#[test]
fn test_new_from_key_with_bookmark() {
    let mut view = LogView::default();
    let mut changes = create_test_changes();
    changes[1].bookmarks = vec!["feature".to_string()];
    view.set_changes(changes);
    view.selected_index = 1;

    let result = view.handle_key(KeyEvent::from(KeyCode::Char('C')));
    match result {
        LogAction::NewChangeFrom { display_name, .. } => {
            assert_eq!(display_name, "feature"); // 先頭 bookmark を表示
        }
        _ => panic!("Expected NewChangeFrom action"),
    }
}

#[test]
fn test_new_from_no_selection() {
    let mut view = LogView::default();
    view.set_changes(vec![]);

    let result = view.handle_key(KeyEvent::from(KeyCode::Char('C')));
    assert!(matches!(result, LogAction::None));
}

#[test]
fn test_track_key_returns_start_track() {
    let mut view = LogView::default();
    view.set_changes(create_test_changes());

    let result = view.handle_key(KeyEvent::from(KeyCode::Char('T')));
    assert!(matches!(result, LogAction::StartTrack));
}

// =============================================================================
// Bookmark Jump tests
// =============================================================================

#[test]
fn test_bookmark_jump_key_returns_start_bookmark_jump() {
    let mut view = LogView::default();
    view.set_changes(create_test_changes());

    let result = view.handle_key(KeyEvent::from(KeyCode::Char('\'')));
    assert!(matches!(result, LogAction::StartBookmarkJump));
}

#[test]
fn test_select_change_by_id_found() {
    let mut view = LogView::default();
    view.set_changes(create_test_changes());

    // Initially at first change
    assert_eq!(view.selected_index, 0);

    // Jump to second change
    let found = view.select_change_by_id("xyz98765");
    assert!(found);
    assert_eq!(view.selected_index, 1);
    assert_eq!(view.selected_change().unwrap().change_id, "xyz98765");
}

#[test]
fn test_compare_same_revision_returns_notification() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());
    press_key(&mut view, keys::COMPARE); // Enter CompareSelect mode
    // Don't move — try to compare with self

    let action = press_key(&mut view, KeyCode::Enter);
    assert_eq!(action, LogAction::CompareSameRevision); // Notification action
    assert_eq!(view.input_mode, InputMode::CompareSelect); // Still in mode
    assert_eq!(view.compare_from, Some("abc12345".to_string())); // Source preserved
}

#[test]
fn test_select_change_by_id_not_found() {
    let mut view = LogView::default();
    view.set_changes(create_test_changes());

    // Initially at first change
    assert_eq!(view.selected_index, 0);

    // Try to jump to non-existent change
    let found = view.select_change_by_id("nonexistent");
    assert!(!found);

    // Selection should remain unchanged
    assert_eq!(view.selected_index, 0);
}

// =============================================================================
// select_change_by_prefix tests (Blame → Log jump)
// =============================================================================

#[test]
fn test_select_change_by_prefix_exact_match() {
    let mut view = LogView::default();
    view.set_changes(create_test_changes());

    let found = view.select_change_by_prefix("abc12345");
    assert!(found);
    assert_eq!(view.selected_index, 0);
    assert_eq!(view.selected_change().unwrap().change_id, "abc12345");
}

#[test]
fn test_select_change_by_prefix_short_prefix() {
    let mut view = LogView::default();
    view.set_changes(create_test_changes());

    // Shorter prefix should still match
    let found = view.select_change_by_prefix("xyz9");
    assert!(found);
    assert_eq!(view.selected_index, 1);
    assert_eq!(view.selected_change().unwrap().change_id, "xyz98765");
}

#[test]
fn test_select_change_by_prefix_no_match() {
    let mut view = LogView::default();
    view.set_changes(create_test_changes());

    let found = view.select_change_by_prefix("qqq");
    assert!(!found);
    assert_eq!(view.selected_index, 0); // unchanged
}

#[test]
fn test_select_change_by_prefix_empty() {
    let mut view = LogView::default();
    view.set_changes(vec![]);

    let found = view.select_change_by_prefix("abc");
    assert!(!found);
}

// =============================================================================
// Next / Prev tests (] / [ keys)
// =============================================================================

#[test]
fn test_next_change_key_returns_action() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    let action = press_key(&mut view, keys::NEXT_CHANGE);
    assert_eq!(action, LogAction::NextChange);
}

#[test]
fn test_prev_change_key_returns_action() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    let action = press_key(&mut view, keys::PREV_CHANGE);
    assert_eq!(action, LogAction::PrevChange);
}

#[test]
fn test_next_change_no_selection() {
    let mut view = LogView::new();
    // Empty changes list - key still returns action (App handles validation)
    let action = press_key(&mut view, keys::NEXT_CHANGE);
    assert_eq!(action, LogAction::NextChange);
}

#[test]
fn test_prev_change_no_selection() {
    let mut view = LogView::new();
    let action = press_key(&mut view, keys::PREV_CHANGE);
    assert_eq!(action, LogAction::PrevChange);
}

#[test]
fn test_next_prev_ignored_in_squash_select_mode() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter squash mode
    press_key(&mut view, keys::SQUASH);
    assert_eq!(view.input_mode, InputMode::SquashSelect);

    // ] and [ should be ignored in SquashSelect mode
    let action = press_key(&mut view, keys::NEXT_CHANGE);
    assert_eq!(action, LogAction::None);

    let action = press_key(&mut view, keys::PREV_CHANGE);
    assert_eq!(action, LogAction::None);
}

#[test]
fn test_next_prev_ignored_in_rebase_select_mode() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Enter rebase mode -> select Revision
    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('r'));
    assert_eq!(view.input_mode, InputMode::RebaseSelect);

    let action = press_key(&mut view, keys::NEXT_CHANGE);
    assert_eq!(action, LogAction::None);

    let action = press_key(&mut view, keys::PREV_CHANGE);
    assert_eq!(action, LogAction::None);
}

// =============================================================================
// select_working_copy tests
// =============================================================================

#[test]
fn test_select_working_copy_found() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Move away from working copy
    view.move_down();
    assert_eq!(view.selected_index, 1);

    // select_working_copy should move back to index 0 (is_working_copy=true)
    let found = view.select_working_copy();
    assert!(found);
    assert_eq!(view.selected_index, 0);
    assert!(view.selected_change().unwrap().is_working_copy);
}

#[test]
fn test_select_working_copy_not_found() {
    let mut view = LogView::new();
    // Create changes with no working copy
    let changes = vec![
        Change {
            change_id: "abc12345".to_string(),
            commit_id: "def67890".to_string(),
            author: "user@example.com".to_string(),
            timestamp: "2024-01-29".to_string(),
            description: "First commit".to_string(),
            is_working_copy: false,
            is_empty: false,
            bookmarks: vec![],
            graph_prefix: "○  ".to_string(),
            is_graph_only: false,
            has_conflict: false,
        },
        Change {
            change_id: "xyz98765".to_string(),
            commit_id: "uvw43210".to_string(),
            author: "user@example.com".to_string(),
            timestamp: "2024-01-28".to_string(),
            description: "Second commit".to_string(),
            is_working_copy: false,
            is_empty: false,
            bookmarks: vec![],
            graph_prefix: "○  ".to_string(),
            is_graph_only: false,
            has_conflict: false,
        },
    ];
    view.set_changes(changes);
    view.move_down();

    let found = view.select_working_copy();
    assert!(!found);
    // Selection should remain unchanged
    assert_eq!(view.selected_index, 1);
}

#[test]
fn test_select_working_copy_already_selected() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Already on working copy at index 0
    assert_eq!(view.selected_index, 0);
    assert!(view.selected_change().unwrap().is_working_copy);

    let found = view.select_working_copy();
    assert!(found);
    assert_eq!(view.selected_index, 0);
}

#[test]
fn test_select_working_copy_empty_changes() {
    let mut view = LogView::new();
    view.set_changes(vec![]);

    let found = view.select_working_copy();
    assert!(!found);
}

// =============================================================================
// Reverse toggle tests (V key)
// =============================================================================

#[test]
fn test_reverse_key_returns_action() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    let action = press_key(&mut view, keys::LOG_REVERSE);
    assert_eq!(action, LogAction::ToggleReversed);
}

#[test]
fn test_reverse_default_is_false() {
    let view = LogView::new();
    assert!(!view.reversed);
}

#[test]
fn test_reverse_preserves_selection() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Move to second change
    view.move_down();
    assert_eq!(view.selected_change().unwrap().change_id, "xyz98765");

    // Toggle reversed
    view.reversed = !view.reversed;
    assert!(view.reversed);

    // Re-set changes (simulating refresh with reversed order)
    let mut reversed_changes = create_test_changes();
    reversed_changes.reverse();
    view.set_changes(reversed_changes);

    // select_change_by_id should find the change
    let found = view.select_change_by_id("xyz98765");
    assert!(found);
    assert_eq!(view.selected_change().unwrap().change_id, "xyz98765");
}

#[test]
fn test_reverse_falls_back_to_working_copy() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // Move to root
    view.move_to_bottom();
    assert_eq!(
        view.selected_change().unwrap().change_id,
        constants::ROOT_CHANGE_ID
    );

    // Toggle reversed and set changes without root
    view.reversed = true;
    let changes = vec![
        Change {
            change_id: "new11111".to_string(),
            commit_id: "com11111".to_string(),
            author: "user@example.com".to_string(),
            timestamp: "2024-01-30".to_string(),
            description: "New change".to_string(),
            is_working_copy: true,
            is_empty: false,
            bookmarks: vec![],
            graph_prefix: "@  ".to_string(),
            is_graph_only: false,
            has_conflict: false,
        },
        Change {
            change_id: "abc12345".to_string(),
            commit_id: "def67890".to_string(),
            author: "user@example.com".to_string(),
            timestamp: "2024-01-29".to_string(),
            description: "First commit".to_string(),
            is_working_copy: false,
            is_empty: false,
            bookmarks: vec![],
            graph_prefix: "○  ".to_string(),
            is_graph_only: false,
            has_conflict: false,
        },
    ];
    view.set_changes(changes);

    // Original change_id (root) not found → fallback to working copy
    let found = view.select_change_by_id(constants::ROOT_CHANGE_ID);
    assert!(!found);
    let found = view.select_working_copy();
    assert!(found);
    assert!(view.selected_change().unwrap().is_working_copy);
}

// =============================================================================
// DiffEdit tests (E key)
// =============================================================================

#[test]
fn test_diffedit_key_returns_action() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    let action = press_key(&mut view, keys::DIFFEDIT);
    assert_eq!(action, LogAction::DiffEdit("abc12345".to_string()));
}

#[test]
fn test_diffedit_no_selection() {
    let mut view = LogView::new();
    // Empty changes
    let action = press_key(&mut view, keys::DIFFEDIT);
    assert_eq!(action, LogAction::None);
}

#[test]
fn test_diffedit_does_not_conflict_with_edit() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // 'e' (lowercase) = Edit
    let action = press_key(&mut view, keys::EDIT);
    assert_eq!(action, LogAction::Edit("abc12345".to_string()));

    // 'E' (uppercase) = DiffEdit — different action
    let action = press_key(&mut view, keys::DIFFEDIT);
    assert_eq!(action, LogAction::DiffEdit("abc12345".to_string()));
}

// =============================================================================
// Evolog tests (L key)
// =============================================================================

#[test]
fn test_evolog_key_returns_action() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    let action = press_key(&mut view, keys::EVOLOG);
    assert_eq!(action, LogAction::OpenEvolog("abc12345".to_string()));
}

#[test]
fn test_evolog_no_selection() {
    let mut view = LogView::new();
    // Empty changes
    let action = press_key(&mut view, keys::EVOLOG);
    assert_eq!(action, LogAction::None);
}

#[test]
fn test_evolog_does_not_conflict_with_edit() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // 'e' (lowercase) = Edit
    let action = press_key(&mut view, keys::EDIT);
    assert_eq!(action, LogAction::Edit("abc12345".to_string()));

    // 'L' (uppercase) = Evolog — completely different key
    let action = press_key(&mut view, keys::EVOLOG);
    assert_eq!(action, LogAction::OpenEvolog("abc12345".to_string()));
}

#[test]
fn test_diffedit_ignored_in_squash_select_mode() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    press_key(&mut view, keys::SQUASH);
    assert_eq!(view.input_mode, InputMode::SquashSelect);

    let action = press_key(&mut view, keys::DIFFEDIT);
    assert_eq!(action, LogAction::None);
}

#[test]
fn test_evolog_ignored_in_rebase_select_mode() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    press_key(&mut view, keys::REBASE);
    press_key(&mut view, KeyCode::Char('r'));
    assert_eq!(view.input_mode, InputMode::RebaseSelect);

    let action = press_key(&mut view, keys::EVOLOG);
    assert_eq!(action, LogAction::None);
}

#[test]
fn test_reverse_ignored_in_special_modes() {
    let mut view = LogView::new();
    view.set_changes(create_test_changes());

    // In SquashSelect mode, V should be ignored
    press_key(&mut view, keys::SQUASH);
    assert_eq!(view.input_mode, InputMode::SquashSelect);
    let action = press_key(&mut view, keys::LOG_REVERSE);
    assert_eq!(action, LogAction::None);
}
