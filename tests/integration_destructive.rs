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

// =============================================================================
// Rebase -s (source: move with descendants)
// =============================================================================

#[test]
fn test_rebase_source_moves_descendants() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // Create chain: root → A → B → C
    repo.write_file("a.txt", "a");
    repo.jj(&["describe", "-m", "A"]);
    repo.jj(&["new", "-m", "B"]);
    repo.write_file("b.txt", "b");
    let b_id = repo.current_change_id();
    repo.jj(&["new", "-m", "C"]);
    repo.write_file("c.txt", "c");
    let c_id = repo.current_change_id();

    // Create a separate branch: root → D
    repo.jj(&["new", "root()", "-m", "D"]);
    repo.write_file("d.txt", "d");
    let d_id = repo.current_change_id();

    // Move B (with descendants B, C) under D
    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .rebase_source(&b_id, &d_id)
        .expect("rebase_source should succeed");

    // Verify B's parent is now D
    let b_parent = repo
        .jj(&[
            "log",
            "-r",
            &format!("{}-", b_id),
            "--no-graph",
            "-T",
            "description.first_line()",
        ])
        .trim()
        .to_string();
    assert_eq!(b_parent, "D", "B's parent should now be D");

    // Verify C's parent is still B (descendants moved together)
    let c_parent = repo
        .jj(&[
            "log",
            "-r",
            &format!("{}-", c_id),
            "--no-graph",
            "-T",
            "description.first_line()",
        ])
        .trim()
        .to_string();
    assert_eq!(c_parent, "B", "C's parent should still be B");
}

// =============================================================================
// Rebase -A (insert-after)
// =============================================================================

#[test]
fn test_rebase_insert_after() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // Create: root → J → K (current)
    //                J → L → M
    repo.write_file("j.txt", "j");
    repo.jj(&["describe", "-m", "J"]);
    let j_id = repo.current_change_id();

    repo.jj(&["new", "-m", "K"]);
    repo.write_file("k.txt", "k");
    let k_id = repo.current_change_id();

    // Create L as child of J
    repo.jj(&["new", &j_id, "-m", "L"]);
    repo.write_file("l.txt", "l");
    let l_id = repo.current_change_id();

    repo.jj(&["new", "-m", "M"]);
    repo.write_file("m.txt", "m");

    // Insert K after L (K becomes child of L, M becomes child of K)
    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .rebase_insert_after(&k_id, &l_id)
        .expect("rebase_insert_after should succeed");

    // Verify K's parent is now L
    let k_parent_desc = repo
        .jj(&[
            "log",
            "-r",
            &format!("{}-", k_id),
            "--no-graph",
            "-T",
            "description.first_line()",
        ])
        .trim()
        .to_string();
    assert_eq!(k_parent_desc, "L", "K's parent should now be L");
}

// =============================================================================
// Rebase -B (insert-before)
// =============================================================================

#[test]
fn test_rebase_insert_before() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // Create: root → J → L → M (current)
    //                J → K
    repo.write_file("j.txt", "j");
    repo.jj(&["describe", "-m", "J"]);
    let j_id = repo.current_change_id();

    repo.jj(&["new", "-m", "L"]);
    repo.write_file("l.txt", "l");
    let l_id = repo.current_change_id();

    repo.jj(&["new", "-m", "M"]);
    repo.write_file("m.txt", "m");

    // Create K as child of J
    repo.jj(&["new", &j_id, "-m", "K"]);
    repo.write_file("k.txt", "k");
    let k_id = repo.current_change_id();

    // Insert K before L (K becomes parent of L, K's parent is J)
    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .rebase_insert_before(&k_id, &l_id)
        .expect("rebase_insert_before should succeed");

    // Verify K's parent is J
    let k_parent_desc = repo
        .jj(&[
            "log",
            "-r",
            &format!("{}-", k_id),
            "--no-graph",
            "-T",
            "description.first_line()",
        ])
        .trim()
        .to_string();
    assert_eq!(k_parent_desc, "J", "K's parent should be J");

    // Verify L's parent is now K
    let l_parent_desc = repo
        .jj(&[
            "log",
            "-r",
            &format!("{}-", l_id),
            "--no-graph",
            "-T",
            "description.first_line()",
        ])
        .trim()
        .to_string();
    assert_eq!(l_parent_desc, "K", "L's parent should now be K");
}
