//! Snapshot tests for Dialog components
//!
//! Uses insta + ratatui TestBackend for visual regression testing.
//! Reference: https://ratatui.rs/recipes/testing/snapshots/

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::ui::components::dialog::{Dialog, DialogCallback, SelectItem};

#[test]
fn test_confirm_dialog_basic() {
    let dialog = Dialog::confirm(
        "Confirm",
        "Delete bookmark 'main'?",
        None,
        DialogCallback::DeleteBookmarks,
    );

    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    terminal
        .draw(|frame| {
            dialog.render(frame, frame.area());
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_confirm_dialog_with_detail() {
    let dialog = Dialog::confirm(
        "Warning",
        "Force push to remote?",
        Some("This will overwrite remote history.".to_string()),
        DialogCallback::GitPush,
    );

    let mut terminal = Terminal::new(TestBackend::new(60, 14)).unwrap();
    terminal
        .draw(|frame| {
            dialog.render(frame, frame.area());
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_select_dialog_empty() {
    let dialog = Dialog::select(
        "Select Bookmarks",
        "Choose bookmarks to delete:",
        vec![],
        None,
        DialogCallback::DeleteBookmarks,
    );

    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    terminal
        .draw(|frame| {
            dialog.render(frame, frame.area());
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_select_dialog_with_items() {
    let items = vec![
        SelectItem {
            label: "main".to_string(),
            value: "main".to_string(),
            selected: false,
        },
        SelectItem {
            label: "feature/auth".to_string(),
            value: "feature/auth".to_string(),
            selected: true,
        },
        SelectItem {
            label: "bugfix/login".to_string(),
            value: "bugfix/login".to_string(),
            selected: false,
        },
    ];

    let dialog = Dialog::select(
        "Delete Bookmarks",
        "Select bookmarks to delete:",
        items,
        Some("Warning: This cannot be undone.".to_string()),
        DialogCallback::DeleteBookmarks,
    );

    let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
    terminal
        .draw(|frame| {
            dialog.render(frame, frame.area());
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_select_single_dialog() {
    let items = vec![
        SelectItem {
            label: "feature/auth".to_string(),
            value: "feature/auth".to_string(),
            selected: false,
        },
        SelectItem {
            label: "main".to_string(),
            value: "main".to_string(),
            selected: false,
        },
        SelectItem {
            label: "develop".to_string(),
            value: "develop".to_string(),
            selected: false,
        },
    ];

    let dialog = Dialog::select_single(
        "Jump to Bookmark",
        "Select bookmark:",
        items,
        None,
        DialogCallback::BookmarkJump,
    );

    let mut terminal = Terminal::new(TestBackend::new(50, 14)).unwrap();
    terminal
        .draw(|frame| {
            dialog.render(frame, frame.area());
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}
