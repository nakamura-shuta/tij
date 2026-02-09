//! Git integration tests.
//!
//! Tests for fetch, push, and bookmark tracking with remote repositories.

#[path = "common/mod.rs"]
mod common;

use common::{RemoteRepo, TestRepo};
use tij::jj::JjExecutor;

#[test]
fn test_git_fetch_from_empty_remote() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();
    let repo = TestRepo::with_remote(&remote);

    let executor = JjExecutor::with_repo_path(repo.path());
    let result = executor.git_fetch();

    // fetch from empty remote succeeds (fetches nothing)
    assert!(result.is_ok(), "fetch from empty remote should succeed");
}

#[test]
fn test_git_push_new_bookmark_with_named() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();
    let repo = TestRepo::with_remote(&remote);

    // Create local changes (but don't create bookmark - let --named do it)
    repo.write_file("README.md", "# Hello");
    repo.jj(&["describe", "-m", "Initial commit"]);
    let change_id = repo.current_change_id();

    let executor = JjExecutor::with_repo_path(repo.path());

    // Push new bookmark using --named for jj 0.37+
    // This creates the local bookmark AND pushes in one step
    let result = executor.git_push_named("main", &change_id);
    assert!(
        result.is_ok(),
        "push new bookmark with --named should succeed: {:?}",
        result
    );

    // Verify bookmark was created locally
    assert!(
        repo.bookmark_exists("main"),
        "bookmark should exist after --named push"
    );
}

#[test]
fn test_git_push_updates_existing_bookmark() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();
    let repo = TestRepo::with_remote(&remote);

    // Initial push using --named (creates bookmark + pushes)
    repo.write_file("README.md", "# Hello");
    repo.jj(&["describe", "-m", "v1"]);
    let v1_id = repo.current_change_id();
    repo.jj(&["git", "push", "--named", &format!("main={}", v1_id)]);

    // Add more changes using commit (which does describe + new atomically)
    repo.write_file("v2.txt", "version 2");
    repo.jj(&["commit", "-m", "v2"]);

    // Now @ is empty, but the previous change (the one we committed) has v2
    // Move main to the v2 change (which is @-)
    let v2_id = repo
        .jj(&["log", "-r", "@-", "--no-graph", "-T", "change_id.short(8)"])
        .trim()
        .to_string();
    repo.jj(&["bookmark", "set", "main", "-r", &v2_id, "--allow-backwards"]);

    let executor = JjExecutor::with_repo_path(repo.path());
    // For existing bookmarks, use --bookmark (not --named)
    let result = executor.git_push_bookmark("main");

    assert!(
        result.is_ok(),
        "push updated bookmark should succeed: {:?}",
        result
    );
}

#[test]
fn test_bookmark_track() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();

    // Create bookmark on remote (via another repo)
    // Use --named to create + push in one step
    let setup_repo = TestRepo::with_remote(&remote);
    setup_repo.write_file("README.md", "# Remote");
    setup_repo.jj(&["describe", "-m", "remote commit"]);
    let change_id = setup_repo.current_change_id();
    setup_repo.jj(&["git", "push", "--named", &format!("feature={}", change_id)]);

    // Another repo fetches and tracks
    let repo = TestRepo::with_remote(&remote);
    repo.jj(&["git", "fetch"]);

    let executor = JjExecutor::with_repo_path(repo.path());
    let result = executor.bookmark_track(&["feature@origin"]);

    assert!(result.is_ok(), "bookmark track should succeed");
}

#[test]
fn test_git_fetch_updates_bookmarks() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();

    // Setup: create initial state on remote using --named
    let setup_repo = TestRepo::with_remote(&remote);
    setup_repo.write_file("file.txt", "v1");
    setup_repo.jj(&["describe", "-m", "Initial"]);
    let initial_id = setup_repo.current_change_id();
    setup_repo.jj(&["git", "push", "--named", &format!("main={}", initial_id)]);

    // Another repo clones
    let repo = TestRepo::with_remote(&remote);
    repo.jj(&["git", "fetch"]);

    // Setup repo pushes an update (bookmark already exists, use --bookmark)
    // Use commit to create a change with description and changes atomically
    setup_repo.write_file("file.txt", "v2");
    setup_repo.jj(&["commit", "-m", "Update"]);

    // Move main to the committed change (which is @-)
    let update_id = setup_repo
        .jj(&["log", "-r", "@-", "--no-graph", "-T", "change_id.short(8)"])
        .trim()
        .to_string();
    setup_repo.jj(&[
        "bookmark",
        "set",
        "main",
        "-r",
        &update_id,
        "--allow-backwards",
    ]);
    setup_repo.jj(&["git", "push", "--bookmark", "main"]);

    // Repo fetches the update
    let executor = JjExecutor::with_repo_path(repo.path());
    let result = executor.git_fetch();

    assert!(result.is_ok(), "fetch update should succeed");

    // Verify the bookmark moved (main@origin should point to new commit)
    let log = repo.jj(&[
        "log",
        "-r",
        "main@origin",
        "--no-graph",
        "-T",
        "description",
    ]);
    assert!(
        log.contains("Update"),
        "main@origin should point to updated commit"
    );
}
