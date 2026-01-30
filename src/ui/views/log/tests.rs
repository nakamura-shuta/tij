//! Tests for LogView

use crossterm::event::{KeyCode, KeyEvent};

use crate::jj::constants;
use crate::keys;
use crate::model::Change;
use crate::ui::{symbols, theme};

use super::{InputMode, LogAction, LogView};

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
