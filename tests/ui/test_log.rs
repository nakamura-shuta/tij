//! Snapshot tests for Log View
//!
//! Uses insta + ratatui TestBackend for visual regression testing.
//! Reference: https://ratatui.rs/recipes/testing/snapshots/

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::model::Change;
use tij::ui::views::{InputMode, LogView};

/// Helper: create a Change with common defaults
fn make_change(
    change_id: &str,
    commit_id: &str,
    description: &str,
    graph_prefix: &str,
    is_working_copy: bool,
    bookmarks: Vec<&str>,
    has_conflict: bool,
) -> Change {
    Change {
        change_id: change_id.to_string(),
        commit_id: commit_id.to_string(),
        author: "test@example.com".to_string(),
        timestamp: "2025-01-15 10:30:00".to_string(),
        description: description.to_string(),
        is_working_copy,
        is_empty: false,
        bookmarks: bookmarks.into_iter().map(String::from).collect(),
        graph_prefix: graph_prefix.to_string(),
        is_graph_only: false,
        has_conflict,
    }
}

#[test]
fn test_log_view_empty() {
    let mut view = LogView::new();

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_log_view_with_changes() {
    let mut view = LogView::new();
    view.set_changes(vec![
        make_change(
            "kxryzmql",
            "a1b2c3d4",
            "Add user authentication",
            "@  ",
            true,
            vec!["main"],
            false,
        ),
        make_change(
            "mzvwqtsr",
            "e5f6g7h8",
            "Refactor database layer",
            "○  ",
            false,
            vec!["feature/db"],
            false,
        ),
        make_change(
            "pqlnrxwv",
            "i9j0k1l2",
            "Initial commit",
            "○  ",
            false,
            vec![],
            false,
        ),
    ]);

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_log_view_with_revset() {
    let mut view = LogView::new();
    view.current_revset = Some("ancestors(@, 5)".to_string());
    view.set_changes(vec![
        make_change(
            "kxryzmql",
            "a1b2c3d4",
            "Latest change",
            "@  ",
            true,
            vec![],
            false,
        ),
        make_change(
            "mzvwqtsr",
            "e5f6g7h8",
            "Parent change",
            "○  ",
            false,
            vec![],
            false,
        ),
    ]);

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_log_view_search_input() {
    let mut view = LogView::new();
    view.set_changes(vec![make_change(
        "kxryzmql",
        "a1b2c3d4",
        "Some change",
        "@  ",
        true,
        vec![],
        false,
    )]);
    view.input_mode = InputMode::SearchInput;
    view.input_buffer = "auth".to_string();

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_log_view_with_conflict() {
    let mut view = LogView::new();
    view.set_changes(vec![
        make_change(
            "kxryzmql",
            "a1b2c3d4",
            "Merge branch with conflicts",
            "@  ",
            true,
            vec![],
            true,
        ),
        make_change(
            "mzvwqtsr",
            "e5f6g7h8",
            "Clean commit",
            "○  ",
            false,
            vec!["main"],
            false,
        ),
    ]);

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}
