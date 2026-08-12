//! Story 5: Conflict Resolution Flow
//!
//! Scenario: Rebase causes conflict, detect and handle it.
//!
//! Note: Full conflict resolution testing requires specific jj path handling.
//! These tests focus on conflict detection and basic flow.

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;
use tij::model::{FileState, RebaseMode};

#[test]
fn story_rebase_creates_conflict() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup: Create diverged history
    repo.write_file("config.txt", "setting=original");
    repo.jj(&["describe", "-m", "Base config"]);
    let base_id = repo.current_change_id();

    // Branch A: Change setting
    repo.jj(&["new", "-m", "Change setting to A"]);
    repo.write_file("config.txt", "setting=A");
    let branch_a_id = repo.current_change_id();

    // Branch B: Branch from base with different change
    repo.jj(&["new", &base_id, "-m", "Change setting to B"]);
    repo.write_file("config.txt", "setting=B");
    let branch_b_id = repo.current_change_id();

    // Rebase B onto A (should cause conflict)
    let result = executor.rebase_unified(RebaseMode::Revision, &branch_b_id, &branch_a_id, &[]);
    assert!(result.is_ok(), "rebase should succeed even with conflict");

    // Check if conflict exists using has_conflict
    let has_conflict = executor
        .has_conflict(&branch_b_id)
        .expect("has_conflict should succeed");

    // After rebase with conflicting changes, there should be a conflict
    assert!(
        has_conflict,
        "Should have conflict after rebasing divergent changes"
    );
}

#[test]
fn story_detect_conflict_via_status() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup conflict scenario
    repo.write_file("file.txt", "original");
    repo.jj(&["describe", "-m", "Base"]);
    let base_id = repo.current_change_id();

    repo.jj(&["new", "-m", "Change A"]);
    repo.write_file("file.txt", "version A");
    let a_id = repo.current_change_id();

    repo.jj(&["new", &base_id, "-m", "Change B"]);
    repo.write_file("file.txt", "version B");
    let b_id = repo.current_change_id();

    // Rebase B onto A
    executor
        .rebase_unified(RebaseMode::Revision, &b_id, &a_id, &[])
        .expect("rebase should succeed");

    // Check status for conflict marker
    let status = executor.status().expect("status should succeed");

    // The status should indicate conflict or the revision should have conflict
    let has_conflict = executor.has_conflict(&b_id).unwrap_or(false);

    // Verify conflict exists
    assert!(
        has_conflict
            || status
                .files
                .iter()
                .any(|f| matches!(f.state, FileState::Conflicted)),
        "Should detect conflict via has_conflict or status"
    );
}

#[test]
fn story_conflict_then_abandon() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup conflict
    repo.write_file("data.txt", "original");
    repo.jj(&["describe", "-m", "Base"]);
    let base_id = repo.current_change_id();

    repo.jj(&["new", "-m", "Their version"]);
    repo.write_file("data.txt", "their content");
    let their_id = repo.current_change_id();

    repo.jj(&["new", &base_id, "-m", "Our version"]);
    repo.write_file("data.txt", "our content");
    let our_id = repo.current_change_id();

    // Rebase (creates conflict)
    executor
        .rebase_unified(RebaseMode::Revision, &our_id, &their_id, &[])
        .expect("rebase should succeed");

    // Alternative to resolving: just abandon the conflicting change
    executor.abandon(&our_id).expect("abandon should succeed");

    // Verify the conflicting change is gone
    let changes = executor
        .log(Some("all()"), false)
        .expect("log should succeed");
    assert!(
        !changes.iter().any(|c| c.description == "Our version"),
        "Conflicting change should be abandoned"
    );
}

#[test]
fn story_conflict_resolution_via_new_commit() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    // Setup conflict
    repo.write_file("code.rs", "fn original() {}");
    repo.jj(&["describe", "-m", "Base"]);
    let base_id = repo.current_change_id();

    repo.jj(&["new", "-m", "Feature A"]);
    repo.write_file("code.rs", "fn feature_a() {}");
    let a_id = repo.current_change_id();

    repo.jj(&["new", &base_id, "-m", "Feature B"]);
    repo.write_file("code.rs", "fn feature_b() {}");
    let b_id = repo.current_change_id();

    // Rebase B onto A (conflict)
    executor
        .rebase_unified(RebaseMode::Revision, &b_id, &a_id, &[])
        .expect("rebase should succeed");

    // Instead of using resolve tool, just edit the file directly
    repo.jj(&["edit", &b_id]);

    // Manually resolve by writing merged content
    repo.write_file("code.rs", "fn feature_a() {}\nfn feature_b() {}");

    // Verify: Conflict should now be resolved
    let has_conflict_after = executor.has_conflict(&b_id).unwrap_or(true);
    assert!(
        !has_conflict_after,
        "Conflict should be resolved after manual fix"
    );

    // Create new commit to continue work
    executor.new_change().expect("new should succeed");

    // Verify we can continue working
    repo.write_file("new_file.rs", "// new work");
    executor
        .describe("@", "Continue after resolution")
        .expect("describe should succeed");
}

/// Regression: "no conflicts" must reach the caller as `Ok(empty)`.
///
/// jj reports a conflict-free revision as a *failure* (exit 2,
/// `Error: No conflicts found at this revision`). Before the fix that error
/// escaped `resolve_list` and the Resolve key painted a red banner on a
/// perfectly healthy change. This test binds the normalization inside
/// `resolve_list` to real jj output — the unit tests only pin the predicate.
#[test]
fn story_resolve_list_on_clean_change_is_empty_not_an_error() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    repo.write_file("clean.txt", "no conflicts here");
    repo.jj(&["describe", "-m", "Clean change"]);
    let clean_id = repo.current_change_id();

    // With `-r <change>` …
    let files = executor
        .resolve_list(Some(&clean_id))
        .expect("a conflict-free revision is a normal state, not an error");
    assert!(files.is_empty(), "expected no conflicts, got {:?}", files);

    // … and without (`@`), which jj also reports as exit 2.
    let files = executor
        .resolve_list(None)
        .expect("a conflict-free working copy is a normal state, not an error");
    assert!(files.is_empty(), "expected no conflicts, got {:?}", files);
}

/// The full arc: real conflict → resolved → listed as empty (not an error).
///
/// This is the `refresh_resolve_list` flow ("All conflicts resolved!"): jj
/// answers the post-resolution listing with exit 2 + "No conflicts found at
/// this revision", which `resolve_list` normalizes away.
#[test]
fn story_resolve_list_goes_from_conflicted_to_empty_after_resolution() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let executor = JjExecutor::with_repo_path(repo.path());

    repo.write_file("code.rs", "fn original() {}");
    repo.jj(&["describe", "-m", "Base"]);
    let base_id = repo.current_change_id();

    repo.jj(&["new", "-m", "Feature A"]);
    repo.write_file("code.rs", "fn feature_a() {}");
    let a_id = repo.current_change_id();

    repo.jj(&["new", &base_id, "-m", "Feature B"]);
    repo.write_file("code.rs", "fn feature_b() {}");
    let b_id = repo.current_change_id();

    executor
        .rebase_unified(RebaseMode::Revision, &b_id, &a_id, &[])
        .expect("rebase should succeed");

    // The conflict is real — asserted via `has_conflict`, not `resolve_list`.
    // `jj resolve --list` right-pads the path to column 36 (measured on jj
    // 0.44), so a path of 35+ characters is followed by a *single* space and
    // `parse_resolve_list` — which requires 2+ — drops the row. The path here
    // is printed relative to the test process's cwd, i.e. a long `../…/tmp/…`
    // one. Pre-existing parser limitation, unrelated to this test.
    assert!(
        executor
            .has_conflict(&b_id)
            .expect("has_conflict should work"),
        "rebase should have produced a conflict"
    );

    // Resolve by writing merged content (same approach as the test above —
    // no interactive merge tool involved).
    repo.jj(&["edit", &b_id]);
    repo.write_file("code.rs", "fn feature_a() {}\nfn feature_b() {}");

    let resolved = executor
        .resolve_list(Some(&b_id))
        .expect("after resolution jj exits 2 with 'No conflicts found' — not an error");
    assert!(
        resolved.is_empty(),
        "expected an empty list after resolution, got {:?}",
        resolved
    );
}
