//! Undo/Redo integration tests.
//!
//! Tests for undo, redo, operation log, and operation restore.

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

#[test]
fn test_undo_reverts_last_operation() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.write_file("test.txt", "original");
    repo.jj(&["describe", "-m", "original"]);

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .describe("@", "changed")
        .expect("describe should succeed");

    assert_eq!(repo.get_description("@"), "changed");

    executor.undo().expect("undo should succeed");

    assert_eq!(repo.get_description("@"), "original");
}

#[test]
fn test_redo_after_undo() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.write_file("test.txt", "content");
    repo.jj(&["describe", "-m", "original"]);

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .describe("@", "changed")
        .expect("describe should succeed");
    executor.undo().expect("undo should succeed");

    // Get redo target
    let redo_target = executor
        .get_redo_target()
        .expect("get_redo_target should succeed");
    assert!(redo_target.is_some(), "Should have a redo target");

    executor
        .redo(&redo_target.unwrap())
        .expect("redo should succeed");

    assert_eq!(repo.get_description("@"), "changed");
}

#[test]
fn test_op_log_returns_operations() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.jj(&["new", "-m", "first"]);
    repo.jj(&["new", "-m", "second"]);

    let executor = JjExecutor::with_repo_path(repo.path());
    let ops = executor.op_log(Some(5)).expect("op_log should succeed");

    assert!(!ops.is_empty(), "Should have operations");
    // Most recent operation is first
    assert!(
        ops[0].description.contains("new") || ops[0].description.contains("snapshot"),
        "Latest op should be new or snapshot"
    );
}

#[test]
fn test_op_restore_reverts_to_previous_state() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.jj(&["new", "-m", "before"]);

    let executor = JjExecutor::with_repo_path(repo.path());
    let ops_before = executor.op_log(Some(1)).expect("op_log should succeed");
    let op_id = &ops_before[0].id;

    repo.jj(&["describe", "-m", "after"]);
    assert_eq!(repo.get_description("@"), "after");

    executor
        .op_restore(op_id)
        .expect("op_restore should succeed");

    assert_eq!(repo.get_description("@"), "before");
}

// =============================================================================
// Phase 48-M4: create_target is cleared after undo/redo
// =============================================================================

/// After a successful undo ('u' key), create_target must be cleared so that
/// a change-id captured before the undo cannot survive as a stale bookmark/tag
/// creation target after the op-log has advanced.
#[test]
fn test_undo_key_clears_create_target() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.jj(&["describe", "-m", "original"]);
    repo.jj(&["describe", "-m", "changed"]);

    // Use new_for_test() to avoid a spurious jj subprocess against the CWD.
    let mut app = tij::app::App::new_for_test();
    app.jj = JjExecutor::with_repo_path(repo.path());

    // Pre-set a captured create_target that would become stale after undo.
    app.create_target = Some("stale-change-id".to_string());

    // Trigger undo via the 'u' key (same path used by the real UI).
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.on_key_event(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));

    assert_eq!(
        app.create_target, None,
        "undo ('u') must clear create_target; stale id survived the undo"
    );
}

/// After a successful redo (Ctrl+R), create_target must be cleared for the
/// same reason: the op-log has advanced, old ids may be gone.
#[test]
fn test_redo_key_clears_create_target() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.jj(&["describe", "-m", "original"]);
    repo.jj(&["describe", "-m", "changed"]);

    // Use new_for_test() to avoid a spurious jj subprocess against the CWD.
    let mut app = tij::app::App::new_for_test();
    app.jj = JjExecutor::with_repo_path(repo.path());

    // Perform an undo first (via key) so that a redo target exists.
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.on_key_event(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));

    // Pre-set a stale target.
    app.create_target = Some("stale-change-id".to_string());

    // Trigger redo via Ctrl+R.
    app.on_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));

    assert_eq!(
        app.create_target, None,
        "redo (Ctrl+R) must clear create_target; stale id survived the redo"
    );
}
