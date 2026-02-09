//! Destructive operation integration tests.
//!
//! Tests for abandon and rebase operations.

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

#[test]
fn test_abandon_removes_change() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    // Create a chain with some content so abandon doesn't affect current @
    repo.jj(&["new", "-m", "base"]);
    repo.jj(&["new", "-m", "to-abandon"]);
    let abandon_id = repo.current_change_id();
    repo.jj(&["new", "-m", "current"]);

    let before = repo.count_changes("all()");

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .abandon(&abandon_id)
        .expect("abandon should succeed");

    let after = repo.count_changes("all()");
    assert_eq!(
        after,
        before - 1,
        "Should have one less change after abandon"
    );
}

#[test]
fn test_abandon_rebases_descendants() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.jj(&["new", "-m", "middle"]);
    let middle_id = repo.current_change_id();
    repo.jj(&["new", "-m", "child"]);

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .abandon(&middle_id)
        .expect("abandon should succeed");

    // child's parent is now root (middle is skipped)
    let parent_desc = repo.get_description("@-");
    assert_ne!(
        parent_desc, "middle",
        "middle should be removed from ancestry"
    );
}

#[test]
fn test_rebase_moves_change() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // Create history: A → B → C
    repo.write_file("a.txt", "a");
    repo.jj(&["describe", "-m", "A"]);
    repo.jj(&["new", "-m", "B"]);
    repo.write_file("b.txt", "b");
    repo.jj(&["new", "-m", "C"]);
    repo.write_file("c.txt", "c");

    // Get C's change_id
    let c_id = repo.current_change_id();

    // Get A's change_id
    let a_id = repo
        .jj(&["log", "-r", "@--", "--no-graph", "-T", "change_id.short(8)"])
        .trim()
        .to_string();

    // Move C directly under A (skip B)
    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .rebase(&c_id, &a_id)
        .expect("rebase should succeed");

    // C's parent is now A
    let parent_desc = repo.get_description("@-");
    assert_eq!(parent_desc, "A", "C's parent should now be A");
}

#[test]
fn test_rebase_with_conflict() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // A creates file.txt
    repo.write_file("file.txt", "original");
    repo.jj(&["describe", "-m", "A"]);

    // B modifies file.txt
    repo.jj(&["new", "-m", "B"]);
    repo.write_file("file.txt", "modified by B");
    let b_id = repo.current_change_id();

    // Go back to A and create C with different modification
    repo.jj(&["edit", "@-"]);
    repo.jj(&["new", "-m", "C"]);
    repo.write_file("file.txt", "modified by C");
    let c_id = repo.current_change_id();

    // Move C under B (should cause conflict)
    let executor = JjExecutor::with_repo_path(repo.path());
    let result = executor.rebase(&c_id, &b_id);

    // rebase itself succeeds but creates conflict
    assert!(result.is_ok(), "rebase should complete");

    // Check conflict state
    let has_conflict = executor.has_conflict(&c_id).unwrap_or(false);
    assert!(has_conflict, "Should have conflict after rebase");
}

#[test]
fn test_abandon_current_change() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // Create a chain: root → A → B (current)
    repo.jj(&["new", "-m", "A"]);
    repo.jj(&["new", "-m", "B"]);
    let b_id = repo.current_change_id();

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .abandon(&b_id)
        .expect("abandon current should succeed");

    // After abandoning current @, jj creates a new empty change
    // The new @ is empty and its parent is A
    let parent_desc = repo.get_description("@-");
    assert_eq!(parent_desc, "A", "Parent should be A after abandoning B");

    // The current @ should be empty (no description)
    let current_desc = repo.get_description("@");
    assert!(
        current_desc.is_empty(),
        "New current change should have no description"
    );
}
