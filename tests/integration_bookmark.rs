//! Bookmark integration tests.
//!
//! Tests for bookmark create, set, delete, and list operations.

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

#[test]
fn test_bookmark_create() {
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .bookmark_create("feature", &change_id)
        .expect("bookmark_create should succeed");

    assert!(
        repo.bookmark_exists("feature"),
        "bookmark 'feature' should exist"
    );
}

#[test]
fn test_bookmark_create_duplicate_fails() {
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .bookmark_create("main", &change_id)
        .expect("first bookmark_create should succeed");

    // Second creation should fail
    let result = executor.bookmark_create("main", &change_id);
    assert!(result.is_err(), "duplicate bookmark creation should fail");
}

#[test]
fn test_bookmark_set_moves_existing() {
    let repo = TestRepo::new();
    repo.jj(&["new", "-m", "first"]);
    let first_id = repo.current_change_id();
    repo.jj(&["bookmark", "create", "mybranch", "-r", &first_id]);

    repo.jj(&["new", "-m", "second"]);
    let second_id = repo.current_change_id();

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .bookmark_set("mybranch", &second_id)
        .expect("bookmark_set should succeed");

    // bookmark now points to second
    let bookmark_rev = repo
        .jj(&[
            "log",
            "-r",
            "mybranch",
            "--no-graph",
            "-T",
            "change_id.short(8)",
        ])
        .trim()
        .to_string();
    assert_eq!(
        bookmark_rev, second_id,
        "bookmark should point to second revision"
    );
}

#[test]
fn test_bookmark_delete() {
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();
    repo.jj(&["bookmark", "create", "to-delete", "-r", &change_id]);

    assert!(
        repo.bookmark_exists("to-delete"),
        "bookmark should exist before delete"
    );

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .bookmark_delete(&["to-delete"])
        .expect("bookmark_delete should succeed");

    assert!(
        !repo.bookmark_exists("to-delete"),
        "bookmark should not exist after delete"
    );
}

#[test]
fn test_bookmark_list_all() {
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();
    repo.jj(&["bookmark", "create", "main", "-r", &change_id]);
    repo.jj(&["bookmark", "create", "develop", "-r", &change_id]);

    let executor = JjExecutor::with_repo_path(repo.path());
    let bookmarks = executor
        .bookmark_list_all()
        .expect("bookmark_list_all should succeed");

    let names: Vec<_> = bookmarks.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"main"), "should contain 'main'");
    assert!(names.contains(&"develop"), "should contain 'develop'");
}

#[test]
fn test_bookmark_set_creates_if_not_exists() {
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();

    let executor = JjExecutor::with_repo_path(repo.path());
    // bookmark_set with --allow-backwards should work on new bookmarks too
    executor
        .bookmark_set("new-branch", &change_id)
        .expect("bookmark_set on new bookmark should succeed");

    assert!(
        repo.bookmark_exists("new-branch"),
        "new bookmark should be created"
    );
}
