//! Story 3: Code Review Fix (Past Commit Modification)
//!
//! Scenario: Code review requires fixing a past commit.
//!
//! 1. Multiple commits in history
//! 2. Receive review feedback
//! 3. Edit past commit
//! 4. Make fix
//! 5. Return to new work position (new)

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

#[test]
fn story_code_review_fix() {
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup: Create 3 commits
    repo.write_file("api.rs", "fn api_v1() {}");
    repo.jj(&["describe", "-m", "Add API v1"]);
    let api_id = repo.current_change_id();

    repo.jj(&["new", "-m", "Add tests"]);
    repo.write_file("tests.rs", "fn test_api() {}");

    repo.jj(&["new", "-m", "Add docs"]);
    repo.write_file("README.md", "# API Documentation");

    // Step 1: Review feedback - API has an issue
    // Step 2: Edit past commit
    executor.edit(&api_id).expect("edit should succeed");
    assert_eq!(repo.get_description("@"), "Add API v1");

    // Step 3: Make fix
    repo.write_file("api.rs", "fn api_v1() { validate(); }");

    // Step 4: Return to latest work position
    executor.new_change().expect("new should succeed");

    // Verify: Fix is preserved
    executor.edit(&api_id).expect("edit back should succeed");
    assert!(repo.read_file("api.rs").contains("validate"));

    // Verify: Subsequent commits still exist (auto-rebase)
    let changes = executor.log(Some("all()")).expect("log should succeed");
    assert!(changes.iter().any(|c| c.description == "Add tests"));
    assert!(changes.iter().any(|c| c.description == "Add docs"));
}

#[test]
fn story_fix_multiple_past_commits() {
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup: Multiple commits with issues
    repo.write_file("module_a.rs", "fn a() { printn!(); }"); // typo
    repo.jj(&["describe", "-m", "Add module A"]);
    let a_id = repo.current_change_id();

    repo.jj(&["new", "-m", "Add module B"]);
    repo.write_file("module_b.rs", "fn b() { printn!(); }"); // typo
    let b_id = repo.current_change_id();

    repo.jj(&["new", "-m", "Add main"]);
    repo.write_file("main.rs", "fn main() {}");

    // Fix module A
    executor.edit(&a_id).expect("edit should succeed");
    repo.write_file("module_a.rs", "fn a() { println!(); }");

    // Fix module B
    executor.edit(&b_id).expect("edit should succeed");
    repo.write_file("module_b.rs", "fn b() { println!(); }");

    // Return to latest
    executor.new_change().expect("new should succeed");

    // Verify both fixes
    executor.edit(&a_id).expect("edit should succeed");
    assert!(repo.read_file("module_a.rs").contains("println!"));

    executor.edit(&b_id).expect("edit should succeed");
    assert!(repo.read_file("module_b.rs").contains("println!"));
}
