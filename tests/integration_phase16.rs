//! Integration tests for Phase 16 features.
//!
//! Tests for duplicate command.

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

// =============================================================================
// Duplicate
// =============================================================================

#[test]
fn test_duplicate_creates_new_change() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // Create a change with description
    repo.jj(&["describe", "-m", "original change"]);
    repo.write_file("test.txt", "hello");
    let original_id = repo.current_change_id();

    // Count changes before duplicate
    let before_count = repo.count_changes("all()");

    // Duplicate via executor
    let executor = JjExecutor::with_repo_path(repo.path());
    let output = executor
        .duplicate(&original_id)
        .expect("duplicate should succeed");

    // Verify output contains "Duplicated"
    assert!(
        output.contains("Duplicated"),
        "output should contain 'Duplicated': {}",
        output
    );

    // Count changes after duplicate
    let after_count = repo.count_changes("all()");
    assert_eq!(
        after_count,
        before_count + 1,
        "should have one more change after duplicate"
    );

    // Parse the new change_id from output
    let new_id = parse_new_change_id(&output);
    assert!(
        new_id.is_some(),
        "should be able to parse new change_id from output: {}",
        output
    );

    // Verify duplicated change has same description
    let new_id = new_id.unwrap();
    let new_desc = repo.get_description(&new_id);
    assert_eq!(
        new_desc, "original change",
        "duplicated change should have same description"
    );
}

#[test]
fn test_duplicate_invalid_revision_fails() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    let executor = JjExecutor::with_repo_path(repo.path());
    let result = executor.duplicate("zzzzzzzz");
    assert!(result.is_err(), "duplicating invalid revision should fail");
}

#[test]
fn test_duplicate_output_parsing_matches_app() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    repo.jj(&["describe", "-m", "test parsing"]);
    let original_id = repo.current_change_id();

    let executor = JjExecutor::with_repo_path(repo.path());
    let output = executor
        .duplicate(&original_id)
        .expect("duplicate should succeed");

    // Use the same parsing logic as App::parse_duplicate_output
    let new_id = parse_new_change_id(&output);
    assert!(
        new_id.is_some(),
        "App parsing logic should work with real jj output: {}",
        output
    );

    // Verify the parsed ID is a valid change
    let new_id = new_id.unwrap();
    let desc = repo.get_description(&new_id);
    assert_eq!(desc, "test parsing");
}

/// Test that duplicating under a narrow revset shows "not in current revset" notification.
///
/// Exercises the full App::duplicate() path via on_key_event(Y), including
/// the select_change_by_prefix() == false branch at src/app/actions.rs.
#[test]
fn test_duplicate_not_in_revset_notification() {
    skip_if_no_jj!();
    let repo = TestRepo::new();

    // Create a change with content so there's something to duplicate
    repo.jj(&["describe", "-m", "my-change"]);
    repo.write_file("file.txt", "content");

    // Build App pointing at test repo
    let mut app = tij::app::App::new();
    app.jj = JjExecutor::with_repo_path(repo.path());
    app.error_message = None;

    // Load only "@" into the log view (narrow revset)
    app.refresh_log(Some("@"));
    assert!(
        !app.log_view.changes.is_empty(),
        "should have at least 1 change"
    );

    // Simulate pressing Y (Duplicate)
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.on_key_event(KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::NONE));

    // Notification should indicate the duplicate is outside the current revset
    let notif = app
        .notification
        .as_ref()
        .expect("should have notification after duplicate");
    assert!(
        notif.message.contains("not in current revset"),
        "expected 'not in current revset' in notification, got: {}",
        notif.message
    );
}

/// Same parsing logic as App::parse_duplicate_output
fn parse_new_change_id(output: &str) -> Option<String> {
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("Duplicated ") {
            let parts: Vec<&str> = rest.splitn(4, ' ').collect();
            if parts.len() >= 3 && parts[1] == "as" {
                return Some(parts[2].to_string());
            }
        }
    }
    None
}
