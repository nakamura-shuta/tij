//! Story 7: Team Collaboration Workflow
//!
//! Scenario: Work with other team members' changes.
//!
//! 1. Fetch from remote
//! 2. Track another member's branch
//! 3. Branch from their work
//! 4. Push own changes

#[path = "common/mod.rs"]
mod common;

use common::{RemoteRepo, TestRepo};
use tij::jj::JjExecutor;

#[test]
fn story_team_collaboration() {
    skip_if_no_jj!();
    // Setup: Two repositories (Alice and Bob)
    let remote = RemoteRepo::new_bare();

    // Alice: Work first and push
    let alice = TestRepo::with_remote(&remote);
    alice.write_file("shared.rs", "// Alice's code");
    alice.jj(&["describe", "-m", "Alice: add shared module"]);
    let alice_change = alice.current_change_id();
    alice.jj(&[
        "git",
        "push",
        "--named",
        &format!("feature/shared={}", alice_change),
    ]);

    // Bob: Pull Alice's work
    let bob = TestRepo::with_remote(&remote);
    let bob_executor = JjExecutor::with_repo_path(bob.path());

    // Step 1: Fetch
    bob_executor.git_fetch().expect("fetch should succeed");

    // Step 2: Track
    bob_executor
        .bookmark_track(&["feature/shared@origin"])
        .expect("track should succeed");

    // Step 3: Branch from Alice's branch
    bob_executor
        .new_change_from("feature/shared")
        .expect("new_change_from should succeed");

    // Step 4: Bob's work
    bob.write_file("shared.rs", "// Alice's code\n// Bob's addition");
    bob_executor
        .describe("@", "Bob: extend shared module")
        .expect("describe should succeed");
    let bob_change = bob.current_change_id();

    // Step 5: Create bookmark and push
    // Use --named to create bookmark AND push in one step (avoids "already exists" error)
    let result = bob_executor.git_push_named("feature/shared-v2", &bob_change);
    assert!(result.is_ok(), "push should succeed: {:?}", result);
    // Verify bookmark was created
    assert!(bob.bookmark_exists("feature/shared-v2"));

    // Verify: Bob's change is on top of Alice's
    let parent_desc = bob.get_description("@-");
    assert_eq!(parent_desc, "Alice: add shared module");
}

#[test]
fn story_pull_and_rebase_local_work() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();

    // Alice pushes initial work
    let alice = TestRepo::with_remote(&remote);
    alice.write_file("main.rs", "fn main() {}");
    alice.jj(&["describe", "-m", "Initial main"]);
    let alice_v1 = alice.current_change_id();
    alice.jj(&["git", "push", "--named", &format!("main={}", alice_v1)]);

    // Bob starts work based on Alice's initial version
    let bob = TestRepo::with_remote(&remote);
    let bob_executor = JjExecutor::with_repo_path(bob.path());

    bob_executor.git_fetch().expect("fetch should succeed");
    bob_executor
        .bookmark_track(&["main@origin"])
        .expect("track should succeed");
    bob_executor
        .new_change_from("main")
        .expect("new from main should succeed");
    bob.write_file("feature.rs", "// Bob's feature");
    bob_executor
        .describe("@", "Bob: add feature")
        .expect("describe should succeed");
    let bob_feature = bob.current_change_id();

    // Alice pushes an update
    alice.write_file("main.rs", "fn main() { init(); }");
    alice.jj(&["commit", "-m", "Add init call"]);
    let alice_v2 = alice
        .jj(&["log", "-r", "@-", "--no-graph", "-T", "change_id.short(8)"])
        .trim()
        .to_string();
    alice.jj(&[
        "bookmark",
        "set",
        "main",
        "-r",
        &alice_v2,
        "--allow-backwards",
    ]);
    alice.jj(&["git", "push", "--bookmark", "main"]);

    // Bob fetches and rebases
    bob_executor.git_fetch().expect("fetch should succeed");

    // Bob rebases his work onto updated main
    bob_executor
        .rebase(&bob_feature, "main@origin")
        .expect("rebase should succeed");

    // Verify: Bob's work is now on top of Alice's v2
    let parent_desc = bob.get_description("@-");
    assert!(
        parent_desc.contains("init") || parent_desc.contains("Add init"),
        "Bob's parent should be Alice's v2"
    );
}

#[test]
fn story_fetch_and_merge_upstream() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();

    // Maintainer creates main
    let maintainer = TestRepo::with_remote(&remote);
    maintainer.write_file("README.md", "# Project");
    maintainer.jj(&["describe", "-m", "Initial commit"]);
    let main_id = maintainer.current_change_id();
    maintainer.jj(&["git", "push", "--named", &format!("main={}", main_id)]);

    // Contributor clones and works
    let contributor = TestRepo::with_remote(&remote);
    let executor = JjExecutor::with_repo_path(contributor.path());

    executor.git_fetch().expect("fetch should succeed");
    executor
        .bookmark_track(&["main@origin"])
        .expect("track should succeed");

    // Work on feature
    executor
        .new_change_from("main")
        .expect("new from main should succeed");
    contributor.write_file("feature.rs", "pub fn feature() {}");
    executor
        .describe("@", "Add feature")
        .expect("describe should succeed");

    // Verify can log the history
    let changes = executor.log(Some("::@")).expect("log should succeed");
    assert!(
        changes.len() >= 2,
        "Should have at least 2 changes in history"
    );
}
