//! Story 2: Feature Branch Workflow
//!
//! Scenario: Branch from main, implement feature, push.
//!
//! 1. Fetch latest from remote (fetch)
//! 2. Branch from main (new from main)
//! 3. Implement feature
//! 4. Create bookmark
//! 5. Push

#[path = "common/mod.rs"]
mod common;

use common::{RemoteRepo, TestRepo};
use tij::jj::JjExecutor;

#[test]
fn story_feature_branch_workflow() {
    // Setup: Remote with main branch
    let remote = RemoteRepo::new_bare();
    let setup_repo = TestRepo::with_remote(&remote);
    setup_repo.write_file("README.md", "# Project");
    setup_repo.jj(&["describe", "-m", "Initial commit"]);
    let main_id = setup_repo.current_change_id();
    setup_repo.jj(&["git", "push", "--named", &format!("main={}", main_id)]);

    // Developer's repository (clean start)
    let repo = TestRepo::with_remote(&remote);
    let executor = JjExecutor::with_repo_path(repo.path());

    // Step 1: Fetch latest (fetch)
    executor.git_fetch().expect("fetch should succeed");

    // Step 2: Track and branch from main
    executor
        .bookmark_track(&["main@origin"])
        .expect("track should succeed");
    executor
        .new_change_from("main")
        .expect("new_change_from should succeed");

    // Step 3: Implement feature
    repo.write_file("feature.rs", "pub fn awesome() {}");
    executor
        .describe("@", "Implement awesome feature")
        .expect("describe should succeed");
    let feature_id = repo.current_change_id();

    // Step 4: Create bookmark and push (--named creates + pushes in one step)
    let result = executor.git_push_named("feature/awesome", &feature_id);
    assert!(result.is_ok(), "push should succeed: {:?}", result);
    // Verify bookmark was created
    assert!(repo.bookmark_exists("feature/awesome"));

    // Verify: History structure is correct (parent is main's commit)
    let parent_desc = repo.get_description("@-");
    assert_eq!(parent_desc, "Initial commit");
}

#[test]
fn story_multiple_features_in_progress() {
    let remote = RemoteRepo::new_bare();

    // Setup remote with main
    let setup_repo = TestRepo::with_remote(&remote);
    setup_repo.write_file("README.md", "# Project");
    setup_repo.jj(&["describe", "-m", "Initial"]);
    let main_id = setup_repo.current_change_id();
    setup_repo.jj(&["git", "push", "--named", &format!("main={}", main_id)]);

    // Developer works on multiple features
    let repo = TestRepo::with_remote(&remote);
    let executor = JjExecutor::with_repo_path(repo.path());

    executor.git_fetch().expect("fetch should succeed");
    executor
        .bookmark_track(&["main@origin"])
        .expect("track should succeed");

    // Feature A
    executor
        .new_change_from("main")
        .expect("new from main should succeed");
    repo.write_file("feature_a.rs", "// A");
    executor
        .describe("@", "Feature A")
        .expect("describe should succeed");
    let a_id = repo.current_change_id();
    executor
        .bookmark_create("feature/a", &a_id)
        .expect("bookmark should succeed");

    // Feature B (also from main, parallel to A)
    executor
        .new_change_from("main")
        .expect("new from main should succeed");
    repo.write_file("feature_b.rs", "// B");
    executor
        .describe("@", "Feature B")
        .expect("describe should succeed");
    let b_id = repo.current_change_id();
    executor
        .bookmark_create("feature/b", &b_id)
        .expect("bookmark should succeed");

    // Verify: Both features have main (Initial) as parent
    let a_parent = repo
        .jj(&[
            "log",
            "-r",
            &format!("{}-", a_id), // `-` = parent (not `--` = grandparent)
            "--no-graph",
            "-T",
            "description",
        ])
        .trim()
        .to_string();
    let b_parent = repo
        .jj(&[
            "log",
            "-r",
            &format!("{}-", b_id),
            "--no-graph",
            "-T",
            "description",
        ])
        .trim()
        .to_string();

    // Both features should have main (Initial) as their direct parent
    assert!(
        a_parent.contains("Initial"),
        "Feature A's parent should be Initial, got: '{}'",
        a_parent
    );
    assert!(
        b_parent.contains("Initial"),
        "Feature B's parent should be Initial, got: '{}'",
        b_parent
    );
}
