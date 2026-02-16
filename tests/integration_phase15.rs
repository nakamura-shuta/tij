//! Integration tests for Phase 15 features.
//!
//! Tests for bookmark rename, bookmark forget, next/prev, and git remote list.

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

// =============================================================================
// Bookmark Rename
// =============================================================================

#[test]
fn test_bookmark_rename_success() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();
    repo.jj(&["bookmark", "create", "old-name", "-r", &change_id]);

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .bookmark_rename("old-name", "new-name")
        .expect("bookmark_rename should succeed");

    assert!(
        !repo.bookmark_exists("old-name"),
        "old bookmark should not exist"
    );
    assert!(
        repo.bookmark_exists("new-name"),
        "new bookmark should exist"
    );
}

#[test]
fn test_bookmark_rename_nonexistent_fails() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    let executor = JjExecutor::with_repo_path(repo.path());
    let result = executor.bookmark_rename("nonexistent", "new-name");
    assert!(result.is_err(), "renaming nonexistent bookmark should fail");
}

#[test]
fn test_bookmark_rename_to_existing_fails() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();
    repo.jj(&["bookmark", "create", "first", "-r", &change_id]);
    repo.jj(&["bookmark", "create", "second", "-r", &change_id]);

    let executor = JjExecutor::with_repo_path(repo.path());
    let result = executor.bookmark_rename("first", "second");
    assert!(
        result.is_err(),
        "renaming to existing bookmark name should fail"
    );
}

// =============================================================================
// Bookmark Forget
// =============================================================================

#[test]
fn test_bookmark_forget_success() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();
    repo.jj(&["bookmark", "create", "to-forget", "-r", &change_id]);

    assert!(
        repo.bookmark_exists("to-forget"),
        "bookmark should exist before forget"
    );

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .bookmark_forget(&["to-forget"])
        .expect("bookmark_forget should succeed");

    assert!(
        !repo.bookmark_exists("to-forget"),
        "bookmark should not exist after forget"
    );
}

#[test]
fn test_bookmark_forget_nonexistent_succeeds_with_warning() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    let executor = JjExecutor::with_repo_path(repo.path());
    // jj bookmark forget with nonexistent name exits 0 (prints warning, not error)
    let result = executor.bookmark_forget(&["nonexistent"]);
    assert!(
        result.is_ok(),
        "forgetting nonexistent bookmark should succeed (jj shows warning only)"
    );
}

// =============================================================================
// Next / Prev
// =============================================================================

#[test]
fn test_next_moves_working_copy() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // Create a child commit: new → describe → new (so @ is on second new)
    // We need a linear chain: root → A → B (@ on A), then `jj next` moves @ to B
    repo.jj(&["describe", "-m", "parent commit"]);
    let parent_id = repo.current_change_id();
    repo.jj(&["new", "-m", "child commit"]);
    let child_id = repo.current_change_id();

    // Move back to parent
    repo.jj(&["edit", &parent_id]);
    assert_eq!(repo.current_change_id(), parent_id);

    let executor = JjExecutor::with_repo_path(repo.path());
    let result = executor.next();
    assert!(result.is_ok(), "jj next should succeed: {:?}", result.err());

    // @ should now be on child
    assert_eq!(repo.current_change_id(), child_id);
}

#[test]
fn test_prev_moves_working_copy() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    repo.jj(&["describe", "-m", "parent commit"]);
    let parent_id = repo.current_change_id();
    repo.jj(&["new", "-m", "child commit"]);

    // @ is on child, prev should move to parent
    let executor = JjExecutor::with_repo_path(repo.path());
    let result = executor.prev();
    assert!(result.is_ok(), "jj prev should succeed: {:?}", result.err());

    assert_eq!(repo.current_change_id(), parent_id);
}

#[test]
fn test_prev_at_root_fails() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // @ is already the first commit (parent is root), prev should fail
    let executor = JjExecutor::with_repo_path(repo.path());
    let result = executor.prev();
    // jj prev from the first commit should error (root has no parent)
    assert!(result.is_err(), "jj prev at root should fail");
}

// =============================================================================
// Git Remote List
// =============================================================================

#[test]
fn test_git_remote_list_with_origin() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // The default jj git init doesn't have remotes, add one
    // We use a dummy URL since we won't actually fetch
    repo.add_remote("origin", "https://example.com/repo.git");

    let executor = JjExecutor::with_repo_path(repo.path());
    let remotes = executor
        .git_remote_list()
        .expect("git_remote_list should succeed");

    assert!(
        remotes.contains(&"origin".to_string()),
        "should contain 'origin', got: {:?}",
        remotes
    );
}

#[test]
fn test_git_remote_list_multiple_remotes() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    repo.add_remote("origin", "https://example.com/repo.git");
    repo.add_remote("upstream", "https://example.com/upstream.git");

    let executor = JjExecutor::with_repo_path(repo.path());
    let remotes = executor
        .git_remote_list()
        .expect("git_remote_list should succeed");

    assert!(remotes.contains(&"origin".to_string()));
    assert!(remotes.contains(&"upstream".to_string()));
    assert_eq!(remotes.len(), 2);
}

#[test]
fn test_git_remote_list_empty() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    let executor = JjExecutor::with_repo_path(repo.path());
    let remotes = executor
        .git_remote_list()
        .expect("git_remote_list should succeed");

    assert!(
        remotes.is_empty(),
        "new repo should have no remotes, got: {:?}",
        remotes
    );
}
