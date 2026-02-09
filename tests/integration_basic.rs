//! Basic operation integration tests.
//!
//! Tests for fundamental jj operations: describe, edit, new, diff_range.

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;
use tij::jj::parser::Parser;

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

#[test]
fn test_diff_range_between_two_revisions() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // Create first revision with a file
    repo.write_file("hello.txt", "hello world");
    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .describe(&repo.current_change_id(), "first revision")
        .expect("describe should succeed");
    let first_id = repo.current_change_id();

    // Create a new change and modify the file
    executor.new_change().expect("new should succeed");
    repo.write_file("hello.txt", "hello new world");
    executor
        .describe(&repo.current_change_id(), "second revision")
        .expect("describe should succeed");
    let second_id = repo.current_change_id();

    // Get diff between the two revisions
    let diff_output = executor
        .diff_range(&first_id, &second_id)
        .expect("diff_range should succeed");

    // Parse the diff body
    let content = Parser::parse_diff_body(&diff_output);
    assert!(content.has_changes());
}

#[test]
fn test_get_change_info() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    repo.write_file("test.txt", "content");
    let executor = JjExecutor::with_repo_path(repo.path());
    let change_id = repo.current_change_id();
    executor
        .describe(&change_id, "test description")
        .expect("describe should succeed");

    let (cid, _bookmarks, _author, _timestamp, description) = executor
        .get_change_info(&change_id)
        .expect("get_change_info should succeed");

    assert_eq!(cid, change_id);
    assert_eq!(description, "test description");
}
