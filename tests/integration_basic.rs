//! Basic operation integration tests.
//!
//! Tests for fundamental jj operations: describe, edit, new.

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

#[test]
fn test_describe_updates_message() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.write_file("test.txt", "hello");

    let executor = JjExecutor::with_repo_path(repo.path());
    let change_id = repo.current_change_id();

    executor
        .describe(&change_id, "Updated description")
        .expect("describe should succeed");

    let desc = repo.get_description("@");
    assert_eq!(desc, "Updated description");
}

#[test]
fn test_edit_changes_working_copy() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.jj(&["new", "-m", "parent"]);
    repo.jj(&["new", "-m", "child"]);

    // child is @ now
    let parent_id = repo
        .jj(&["log", "-r", "@-", "--no-graph", "-T", "change_id.short(8)"])
        .trim()
        .to_string();

    let executor = JjExecutor::with_repo_path(repo.path());
    executor.edit(&parent_id).expect("edit should succeed");

    // parent is now @
    let desc = repo.get_description("@");
    assert_eq!(desc, "parent");
}

#[test]
fn test_new_creates_empty_change() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let before = repo.count_changes("all()");

    let executor = JjExecutor::with_repo_path(repo.path());
    executor.new_change().expect("new should succeed");

    let after = repo.count_changes("all()");
    assert_eq!(after, before + 1);
}

#[test]
fn test_new_from_creates_child_of_specified_revision() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.jj(&["new", "-m", "base"]);
    let base_id = repo.current_change_id();
    repo.jj(&["new", "-m", "other"]);

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .new_change_from(&base_id)
        .expect("new_change_from should succeed");

    // new change's parent is base
    let parent_desc = repo.get_description("@-");
    assert_eq!(parent_desc, "base");
}
