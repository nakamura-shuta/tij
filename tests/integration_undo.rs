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
