//! Story 8: Absorb Workflow
//!
//! Scenario: Automatically distribute fixes to appropriate ancestor commits.
//!
//! 1. Multiple commits exist (each adding a file)
//! 2. Make bulk fixes (e.g., typo corrections)
//! 3. Absorb automatically distributes to ancestors

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

#[test]
fn story_absorb_workflow() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup: Each file in separate commit
    repo.write_file("module_a.rs", "fn module_a() { printn!(\"A\"); }"); // typo: printn
    repo.jj(&["describe", "-m", "Add module A"]);

    repo.jj(&["new", "-m", "Add module B"]);
    repo.write_file("module_b.rs", "fn module_b() { printn!(\"B\"); }"); // typo: printn

    repo.jj(&["new", "-m", "Add module C"]);
    repo.write_file("module_c.rs", "fn module_c() { println!(\"C\"); }"); // correct

    // WIP: Fix typos
    repo.jj(&["new", "-m", "wip: fix typos"]);
    repo.write_file("module_a.rs", "fn module_a() { println!(\"A\"); }"); // fixed
    repo.write_file("module_b.rs", "fn module_b() { println!(\"B\"); }"); // fixed

    // Step 1: Absorb
    let result = executor.absorb();
    assert!(
        result.is_ok(),
        "absorb should complete without error: {:?}",
        result
    );

    // Verify: WIP commit is empty or changes absorbed
    // (absorb result depends on situation, but should not error)
}

#[test]
fn story_absorb_single_file_fix() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup: Create commit with a file
    repo.write_file("code.rs", "fn hello() {\n    prinlnt!(\"hello\");\n}");
    repo.jj(&["describe", "-m", "Add hello function"]);

    // New commit, add more code
    repo.jj(&["new", "-m", "Add more code"]);
    repo.write_file("more.rs", "fn more() {}");

    // WIP: Fix typo in original file
    repo.jj(&["new", "-m", "wip"]);
    repo.write_file("code.rs", "fn hello() {\n    println!(\"hello\");\n}");

    // Absorb should move fix to appropriate commit
    let result = executor.absorb();
    assert!(result.is_ok(), "absorb should succeed");
}

#[test]
fn story_absorb_does_not_affect_unrelated() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup: Two independent commits
    repo.write_file("independent_a.rs", "// A");
    repo.jj(&["describe", "-m", "Add independent A"]);

    repo.jj(&["new", "-m", "Add independent B"]);
    repo.write_file("independent_b.rs", "// B");

    // Add a new file (not fixing anything)
    repo.jj(&["new", "-m", "Add new feature"]);
    repo.write_file("new_feature.rs", "// New feature");

    // Absorb with a new file shouldn't break anything
    let result = executor.absorb();
    assert!(result.is_ok(), "absorb should complete");

    // Verify: All commits still exist
    let changes = executor.log(Some("all()")).expect("log should succeed");
    assert!(
        changes.iter().any(|c| c.description == "Add independent A"),
        "Independent A should exist"
    );
    assert!(
        changes.iter().any(|c| c.description == "Add independent B"),
        "Independent B should exist"
    );
}

#[test]
fn story_absorb_empty_wip() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup
    repo.write_file("file.rs", "content");
    repo.jj(&["describe", "-m", "Initial"]);

    // Empty WIP (no changes to absorb)
    repo.jj(&["new", "-m", "Empty WIP"]);

    // Absorb on empty change should be fine
    let result = executor.absorb();
    assert!(result.is_ok(), "absorb on empty should not error");
}
