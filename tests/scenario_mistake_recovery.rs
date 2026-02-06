//! Story 6: Mistake Recovery
//!
//! Scenario: Accidentally abandon a commit, recover with undo.
//!
//! 1. Important work exists
//! 2. Accidentally abandon
//! 3. Recover with undo
//! 4. Verify work is restored

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

#[test]
fn story_mistake_recovery() {
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup: Important work
    repo.write_file("important.rs", "// Critical code");
    repo.jj(&["describe", "-m", "Important feature"]);
    let important_id = repo.current_change_id();

    // Start next work
    repo.jj(&["new", "-m", "Next work"]);

    // Step 1: Accidentally abandon important work
    executor
        .abandon(&important_id)
        .expect("abandon should succeed");

    // Verify: Important work is gone
    let changes = executor.log(Some("all()")).expect("log should succeed");
    assert!(
        !changes.iter().any(|c| c.description == "Important feature"),
        "Important feature should be gone"
    );

    // Step 2: Recover with undo
    executor.undo().expect("undo should succeed");

    // Step 3: Verify work is restored
    let changes_after = executor.log(Some("all()")).expect("log should succeed");
    assert!(
        changes_after
            .iter()
            .any(|c| c.description == "Important feature"),
        "Important feature should be restored"
    );

    // Verify: File is also restored
    repo.jj(&["edit", &important_id]);
    assert!(repo.read_file("important.rs").contains("Critical code"));
}

#[test]
fn story_undo_wrong_rebase() {
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup: A -> B -> C
    repo.write_file("a.txt", "a");
    repo.jj(&["describe", "-m", "A"]);
    let a_id = repo.current_change_id();

    repo.jj(&["new", "-m", "B"]);
    repo.write_file("b.txt", "b");
    let _b_id = repo.current_change_id();

    repo.jj(&["new", "-m", "C"]);
    repo.write_file("c.txt", "c");
    let c_id = repo.current_change_id();

    // Accidentally rebase C to A (wrong parent)
    executor
        .rebase(&c_id, &a_id)
        .expect("rebase should succeed");

    // Verify wrong state
    let wrong_parent = repo.get_description("@-");
    assert_eq!(wrong_parent, "A", "C's parent is wrongly A");

    // Undo the wrong rebase
    executor.undo().expect("undo should succeed");

    // Verify correct state restored
    let correct_parent = repo.get_description("@-");
    assert_eq!(correct_parent, "B", "C's parent should be B again");
}

#[test]
fn story_redo_after_undo() {
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup
    repo.write_file("file.txt", "content");
    repo.jj(&["describe", "-m", "original"]);

    // Make a change
    executor
        .describe("@", "changed")
        .expect("describe should succeed");
    assert_eq!(repo.get_description("@"), "changed");

    // Undo
    executor.undo().expect("undo should succeed");
    assert_eq!(repo.get_description("@"), "original");

    // Redo
    let redo_target = executor
        .get_redo_target()
        .expect("get_redo_target should succeed");
    assert!(redo_target.is_some(), "Should have redo target");

    executor
        .redo(&redo_target.unwrap())
        .expect("redo should succeed");
    assert_eq!(repo.get_description("@"), "changed");
}

#[test]
fn story_op_restore_to_earlier_state() {
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup: Create several operations
    repo.write_file("file.txt", "v1");
    repo.jj(&["describe", "-m", "Version 1"]);

    // Get operation ID at this point
    let ops = executor.op_log(Some(1)).expect("op_log should succeed");
    let checkpoint_op = &ops[0].id;

    // Make more changes
    repo.jj(&["new", "-m", "Version 2"]);
    repo.write_file("file.txt", "v2");

    repo.jj(&["new", "-m", "Version 3"]);
    repo.write_file("file.txt", "v3");

    // Restore to checkpoint
    executor
        .op_restore(checkpoint_op)
        .expect("op_restore should succeed");

    // Verify: We're back at Version 1
    assert_eq!(repo.get_description("@"), "Version 1");
}
