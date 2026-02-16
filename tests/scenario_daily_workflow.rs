//! Story 1: Daily Development Workflow
//!
//! Scenario: A developer's typical workflow from morning to evening.
//!
//! 1. Open repository
//! 2. Start work (new)
//! 3. Write code (write files)
//! 4. Add description (describe)
//! 5. Continue work (modify files)
//! 6. Finish and move to next task (new)

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

#[test]
fn story_daily_development_workflow() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Step 1: Start work
    executor.new_change().expect("new should succeed");
    let work_id = repo.current_change_id();

    // Step 2: Write code
    repo.write_file("feature.rs", "fn new_feature() {}");

    // Step 3: Add description
    executor
        .describe(&work_id, "Add new feature")
        .expect("describe should succeed");
    assert_eq!(repo.get_description("@"), "Add new feature");

    // Step 4: Continue work
    repo.write_file("feature.rs", "fn new_feature() { todo!() }");
    repo.write_file("tests.rs", "#[test] fn test_feature() {}");

    // Step 5: Finish and move to next task
    executor.new_change().expect("new should succeed");
    let next_id = repo.current_change_id();

    // Verify: Previous work is preserved
    assert_ne!(work_id, next_id);
    assert_eq!(repo.get_description(&work_id), "Add new feature");

    // Verify: Files exist correctly
    assert!(repo.read_file("feature.rs").contains("todo!"));
}

#[test]
fn story_incremental_changes_throughout_day() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Morning: Start feature A
    repo.write_file("feature_a.rs", "// Feature A");
    executor
        .describe("@", "WIP: Feature A")
        .expect("describe should succeed");
    let feature_a_id = repo.current_change_id(); // Save for later

    // Midday: Realize we need a helper
    executor.new_change().expect("new should succeed");
    repo.write_file("helper.rs", "pub fn helper() {}");
    executor
        .describe("@", "Add helper function")
        .expect("describe should succeed");

    // Afternoon: Continue feature A (edit back)
    executor.edit(&feature_a_id).expect("edit should succeed");
    repo.write_file("feature_a.rs", "// Feature A\nuse crate::helper::helper;");

    // Update description
    executor
        .describe("@", "Feature A (uses helper)")
        .expect("describe should succeed");

    // Verify: Both changes exist
    let changes = executor
        .log(Some("all()"), false)
        .expect("log should succeed");
    assert!(changes.iter().any(|c| c.description.contains("helper")));
    assert!(changes.iter().any(|c| c.description.contains("Feature A")));
}
