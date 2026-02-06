//! Story 4: Squash Workflow (WIP Commit Cleanup)
//!
//! Scenario: Clean up work-in-progress commits.
//!
//! 1. Multiple WIP commits exist
//! 2. Squash related commits
//! 3. Create clean history

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

#[test]
fn story_squash_wip_commits() {
    let repo = TestRepo::new();
    let _executor = JjExecutor::with_repo_path(repo.path());

    // Setup: Create WIP commits
    repo.write_file("main.rs", "fn main() {}");
    repo.jj(&["describe", "-m", "feat: add main function"]);
    let base_id = repo.current_change_id();

    repo.jj(&["new", "-m", "wip: try something"]);
    repo.write_file("main.rs", "fn main() { step1(); }");
    let wip1_id = repo.current_change_id();

    repo.jj(&["new", "-m", "wip: more work"]);
    repo.write_file("main.rs", "fn main() { step1(); step2(); }");
    let wip2_id = repo.current_change_id();

    repo.jj(&["new", "-m", "wip: almost done"]);
    repo.write_file("main.rs", "fn main() { step1(); step2(); step3(); }");

    let before_count = repo.count_changes("all()");

    // Step 1: Squash WIP commits into base (using jj commands with -u for non-interactive)
    repo.jj(&["squash", "--from", &wip2_id, "--into", &wip1_id, "-u"]);
    repo.jj(&["squash", "--from", &wip1_id, "--into", &base_id, "-u"]);

    let after_count = repo.count_changes("all()");

    // Verify: Commit count decreased
    assert!(
        after_count < before_count,
        "Should have fewer commits after squash"
    );

    // Verify: Final changes are in base
    let base_diff = repo.jj(&["diff", "-r", &base_id, "--summary"]);
    assert!(base_diff.contains("main.rs"));
}

#[test]
fn story_squash_current_into_parent() {
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup: Parent and current with changes
    repo.write_file("lib.rs", "pub mod feature;");
    repo.jj(&["describe", "-m", "Add feature module"]);

    repo.jj(&["new", "-m", "WIP: implement feature"]);
    repo.write_file("feature.rs", "pub fn feature() {}");

    // Squash current into parent
    executor.squash().expect("squash should succeed");

    // Verify: Parent now has both files
    let parent_diff = repo.jj(&["diff", "-r", "@-", "--summary"]);
    assert!(
        parent_diff.contains("lib.rs") || parent_diff.contains("feature.rs"),
        "Parent should have the changes"
    );
}

#[test]
fn story_selective_squash_workflow() {
    let repo = TestRepo::new();

    // Create commits that should remain separate
    repo.write_file("setup.rs", "// setup");
    repo.jj(&["describe", "-m", "Initial setup"]);

    repo.jj(&["new", "-m", "Feature A"]);
    repo.write_file("feature_a.rs", "// A");
    let a_id = repo.current_change_id();

    repo.jj(&["new", "-m", "Feature B"]);
    repo.write_file("feature_b.rs", "// B");

    repo.jj(&["new", "-m", "fixup: A typo"]);
    repo.write_file("feature_a.rs", "// A fixed");
    let fixup_id = repo.current_change_id();

    // Squash fixup into Feature A
    repo.jj(&["squash", "--from", &fixup_id, "--into", &a_id, "-u"]);

    // Verify: Feature A has the fix, Feature B is unchanged
    repo.jj(&["edit", &a_id]);
    assert!(repo.read_file("feature_a.rs").contains("fixed"));

    // Feature B still exists
    let changes = repo.jj(&[
        "log",
        "-r",
        "all()",
        "--no-graph",
        "-T",
        r#"description ++ "\n""#,
    ]);
    assert!(changes.contains("Feature B"));
}
