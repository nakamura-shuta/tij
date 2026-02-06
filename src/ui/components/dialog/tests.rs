use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn test_confirm_dialog_yes() {
    let dialog = Dialog::confirm(
        "Test",
        "Are you sure?",
        None,
        DialogCallback::DeleteBookmarks,
    );

    let mut d = dialog.clone();
    assert_eq!(
        d.handle_key(key(KeyCode::Char('y'))),
        Some(DialogResult::Confirmed(vec![]))
    );

    let mut d = dialog.clone();
    assert_eq!(
        d.handle_key(key(KeyCode::Char('Y'))),
        Some(DialogResult::Confirmed(vec![]))
    );

    let mut d = dialog.clone();
    assert_eq!(
        d.handle_key(key(KeyCode::Enter)),
        Some(DialogResult::Confirmed(vec![]))
    );
}

#[test]
fn test_confirm_dialog_no() {
    let dialog = Dialog::confirm(
        "Test",
        "Are you sure?",
        None,
        DialogCallback::DeleteBookmarks,
    );

    let mut d = dialog.clone();
    assert_eq!(
        d.handle_key(key(KeyCode::Char('n'))),
        Some(DialogResult::Cancelled)
    );

    let mut d = dialog.clone();
    assert_eq!(
        d.handle_key(key(KeyCode::Char('N'))),
        Some(DialogResult::Cancelled)
    );

    let mut d = dialog.clone();
    assert_eq!(
        d.handle_key(key(KeyCode::Esc)),
        Some(DialogResult::Cancelled)
    );
}

#[test]
fn test_select_dialog_toggle() {
    let items = vec![
        SelectItem {
            label: "Item 1".to_string(),
            value: "1".to_string(),
            selected: false,
        },
        SelectItem {
            label: "Item 2".to_string(),
            value: "2".to_string(),
            selected: false,
        },
    ];

    let mut dialog = Dialog::select(
        "Test",
        "Select items",
        items,
        None,
        DialogCallback::DeleteBookmarks,
    );

    // Toggle first item
    assert!(dialog.handle_key(key(KeyCode::Char(' '))).is_none());
    if let DialogKind::Select { items, .. } = &dialog.kind {
        assert!(items[0].selected);
        assert!(!items[1].selected);
    }

    // Move down and toggle
    dialog.handle_key(key(KeyCode::Char('j')));
    dialog.handle_key(key(KeyCode::Char(' ')));
    if let DialogKind::Select { items, .. } = &dialog.kind {
        assert!(items[0].selected);
        assert!(items[1].selected);
    }
}

#[test]
fn test_select_dialog_confirm() {
    let items = vec![
        SelectItem {
            label: "Item 1".to_string(),
            value: "value1".to_string(),
            selected: true,
        },
        SelectItem {
            label: "Item 2".to_string(),
            value: "value2".to_string(),
            selected: false,
        },
        SelectItem {
            label: "Item 3".to_string(),
            value: "value3".to_string(),
            selected: true,
        },
    ];

    let mut dialog = Dialog::select(
        "Test",
        "Select items",
        items,
        None,
        DialogCallback::DeleteBookmarks,
    );

    let result = dialog.handle_key(key(KeyCode::Enter));
    assert_eq!(
        result,
        Some(DialogResult::Confirmed(vec![
            "value1".to_string(),
            "value3".to_string()
        ]))
    );
}

#[test]
fn test_select_dialog_empty_confirm_is_cancelled() {
    let items = vec![
        SelectItem {
            label: "Item 1".to_string(),
            value: "1".to_string(),
            selected: false,
        },
        SelectItem {
            label: "Item 2".to_string(),
            value: "2".to_string(),
            selected: false,
        },
    ];

    let mut dialog = Dialog::select(
        "Test",
        "Select items",
        items,
        None,
        DialogCallback::DeleteBookmarks,
    );

    // Confirm with nothing selected should cancel
    let result = dialog.handle_key(key(KeyCode::Enter));
    assert_eq!(result, Some(DialogResult::Cancelled));
}

#[test]
fn test_select_dialog_cancel() {
    let items = vec![SelectItem {
        label: "Item 1".to_string(),
        value: "1".to_string(),
        selected: true,
    }];

    let mut dialog = Dialog::select(
        "Test",
        "Select items",
        items,
        None,
        DialogCallback::DeleteBookmarks,
    );

    assert_eq!(
        dialog.handle_key(key(KeyCode::Esc)),
        Some(DialogResult::Cancelled)
    );

    let mut dialog2 = Dialog::select(
        "Test",
        "Select items",
        vec![],
        None,
        DialogCallback::DeleteBookmarks,
    );
    assert_eq!(
        dialog2.handle_key(key(KeyCode::Char('q'))),
        Some(DialogResult::Cancelled)
    );
}

// ─────────────────────────────────────────────────────────────────────────
// DialogCallback tests (Phase 5.2)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn test_dialog_callback_clone_with_data() {
    let callback = DialogCallback::MoveBookmark {
        name: "main".to_string(),
        change_id: "abc123".to_string(),
    };
    let cloned = callback.clone();
    assert_eq!(callback, cloned);
}

#[test]
fn test_dialog_callback_equality_different_data() {
    let callback1 = DialogCallback::MoveBookmark {
        name: "main".to_string(),
        change_id: "abc123".to_string(),
    };
    let callback2 = DialogCallback::MoveBookmark {
        name: "feature".to_string(), // Different name
        change_id: "abc123".to_string(),
    };
    assert_ne!(callback1, callback2);

    let callback3 = DialogCallback::MoveBookmark {
        name: "main".to_string(),
        change_id: "xyz789".to_string(), // Different change_id
    };
    assert_ne!(callback1, callback3);
}

#[test]
fn test_dialog_callback_different_variants() {
    let move_bm = DialogCallback::MoveBookmark {
        name: "main".to_string(),
        change_id: "abc123".to_string(),
    };
    let delete_bm = DialogCallback::DeleteBookmarks;
    assert_ne!(move_bm, delete_bm);
}

#[test]
fn test_confirm_dialog_for_move_bookmark() {
    let dialog = Dialog::confirm(
        "Move Bookmark",
        "Move bookmark \"main\" to this change?",
        Some("Bookmark will be updated.".to_string()),
        DialogCallback::MoveBookmark {
            name: "main".to_string(),
            change_id: "abc123".to_string(),
        },
    );

    assert!(matches!(dialog.kind, DialogKind::Confirm { .. }));
    assert!(matches!(
        dialog.callback_id,
        DialogCallback::MoveBookmark { .. }
    ));
}

#[test]
fn test_confirm_dialog_with_detail() {
    let dialog = Dialog::confirm(
        "Test",
        "Message",
        Some("Warning detail".to_string()),
        DialogCallback::DeleteBookmarks,
    );

    if let DialogKind::Confirm { detail, .. } = &dialog.kind {
        assert_eq!(detail.as_deref(), Some("Warning detail"));
    } else {
        panic!("Expected Confirm dialog");
    }
}

#[test]
fn test_confirm_dialog_ignores_other_keys() {
    let mut dialog = Dialog::confirm("Test", "Message", None, DialogCallback::DeleteBookmarks);

    // Other keys are ignored
    assert!(dialog.handle_key(key(KeyCode::Char('x'))).is_none());
    assert!(dialog.handle_key(key(KeyCode::Char(' '))).is_none());
    assert!(dialog.handle_key(key(KeyCode::Tab)).is_none());
}

// ─────────────────────────────────────────────────────────────────────────
// Single-select dialog tests (Phase 7.4.1)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn test_single_select_dialog_enter_confirms_current() {
    let items = vec![
        SelectItem {
            label: "Item 1".to_string(),
            value: "value1".to_string(),
            selected: false,
        },
        SelectItem {
            label: "Item 2".to_string(),
            value: "value2".to_string(),
            selected: false,
        },
    ];

    let mut dialog = Dialog::select_single(
        "Test",
        "Select one",
        items,
        None,
        DialogCallback::BookmarkJump,
    );

    // Move to second item
    dialog.handle_key(key(KeyCode::Char('j')));
    assert_eq!(dialog.cursor, 1);

    // Press Enter - should confirm with current cursor item
    let result = dialog.handle_key(key(KeyCode::Enter));
    assert_eq!(
        result,
        Some(DialogResult::Confirmed(vec!["value2".to_string()]))
    );
}

#[test]
fn test_single_select_dialog_space_does_not_toggle() {
    let items = vec![
        SelectItem {
            label: "Item 1".to_string(),
            value: "1".to_string(),
            selected: false,
        },
        SelectItem {
            label: "Item 2".to_string(),
            value: "2".to_string(),
            selected: false,
        },
    ];

    let mut dialog = Dialog::select_single(
        "Test",
        "Select one",
        items,
        None,
        DialogCallback::BookmarkJump,
    );

    // Space should not toggle selection in single_select mode
    assert!(dialog.handle_key(key(KeyCode::Char(' '))).is_none());
    if let DialogKind::Select { items, .. } = &dialog.kind {
        assert!(!items[0].selected);
        assert!(!items[1].selected);
    }
}

#[test]
fn test_single_select_dialog_cancel() {
    let items = vec![SelectItem {
        label: "Item 1".to_string(),
        value: "1".to_string(),
        selected: false,
    }];

    let mut dialog = Dialog::select_single(
        "Test",
        "Select one",
        items,
        None,
        DialogCallback::BookmarkJump,
    );

    assert_eq!(
        dialog.handle_key(key(KeyCode::Esc)),
        Some(DialogResult::Cancelled)
    );
}
