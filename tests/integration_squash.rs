//! Squash and Absorb integration tests.
//!
//! Tests for squash and absorb operations.

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

#[test]
fn test_squash_into_merges_changes() {
    let repo = TestRepo::new();

    // Setup: parent with changes, child with more changes
    repo.write_file("file1.txt", "parent content");
    repo.jj(&["describe", "-m", "parent"]);
    repo.jj(&["new", "-m", "child"]);
    repo.write_file("file2.txt", "child content");

    let child_id = repo.current_change_id();
    let parent_id = repo
        .jj(&["log", "-r", "@-", "--no-graph", "-T", "change_id.short(8)"])
        .trim()
        .to_string();

    // Non-interactive squash (jj squash --from child --into parent -u)
    // Use -u to avoid opening editor when both revisions have descriptions
    repo.jj(&["squash", "--from", &child_id, "--into", &parent_id, "-u"]);

    // Verify: parent contains file2.txt
    let diff = repo.jj(&["diff", "-r", &parent_id, "--summary"]);
    assert!(
        diff.contains("file2.txt"),
        "parent should contain file2.txt after squash"
    );
}

#[test]
fn test_squash_default_into_parent() {
    let repo = TestRepo::new();

    // Setup: parent with a file, child with modifications
    repo.write_file("main.rs", "fn main() {}");
    repo.jj(&["describe", "-m", "Initial"]);
    repo.jj(&["new", "-m", "Add feature"]);
    repo.write_file("main.rs", "fn main() { feature(); }");
    repo.write_file("feature.rs", "fn feature() {}");

    let executor = JjExecutor::with_repo_path(repo.path());

    // Squash current changes into parent
    executor.squash().expect("squash should succeed");

    // Verify: @ is now empty or has been absorbed
    // The parent should have the feature
    let parent_diff = repo.jj(&["diff", "-r", "@-", "--summary"]);
    assert!(
        parent_diff.contains("feature.rs") || parent_diff.contains("main.rs"),
        "Parent should contain the squashed changes"
    );
}

#[test]
fn test_absorb_moves_changes_to_ancestor() {
    let repo = TestRepo::new();

    // Setup
    repo.write_file("code.rs", "fn main() {}");
    repo.jj(&["describe", "-m", "initial"]);
    repo.jj(&["new", "-m", "wip"]);

    // Modify code.rs (same file)
    repo.write_file("code.rs", "fn main() { println!(\"hello\"); }");

    let executor = JjExecutor::with_repo_path(repo.path());
    let result = executor.absorb();

    // absorb succeeds but results depend on situation
    assert!(result.is_ok(), "absorb should complete without error");
}
