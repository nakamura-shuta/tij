//! Property-based tests for jj output parsers
//!
//! Uses proptest to verify parsers handle arbitrary input without panicking.
//! Reference: https://lib.rs/crates/proptest

use proptest::prelude::*;
use tij::jj::parser::{Parser, parse_bookmark_list};
use tij::model::FileState;

// =============================================================================
// Strategy generators for realistic-ish jj output
// =============================================================================

/// Generate a change_id-like string (8 hex chars)
fn change_id_strategy() -> impl Strategy<Value = String> {
    "[a-z]{8}".prop_map(|s| s.to_string())
}

/// Generate a commit_id-like string (40 hex chars)
fn commit_id_strategy() -> impl Strategy<Value = String> {
    "[a-f0-9]{40}".prop_map(|s| s.to_string())
}

/// Generate a file path (no tabs, reasonable length)
fn file_path_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_/.-]{1,50}".prop_map(|s| s.to_string())
}

/// Generate a description (single line, no tabs)
fn description_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 :_-]{0,100}".prop_map(|s| s.to_string())
}

// =============================================================================
// Robustness tests: parsers should never panic on arbitrary input
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Log parser should not panic on arbitrary input
    #[test]
    fn log_parser_does_not_panic(input in ".*") {
        // Should return Ok or Err, never panic
        let _ = Parser::parse_log(&input);
    }

    /// Status parser should not panic on arbitrary input
    #[test]
    fn status_parser_does_not_panic(input in ".*") {
        let _ = Parser::parse_status(&input);
    }

    /// Bookmark parser should not panic on arbitrary input
    #[test]
    fn bookmark_parser_does_not_panic(input in ".*") {
        let _ = parse_bookmark_list(&input);
    }

    /// Diff/show parser should not panic on arbitrary input
    #[test]
    fn diff_parser_does_not_panic(input in ".*") {
        let _ = Parser::parse_show(&input);
    }

    /// Operation log parser should not panic on arbitrary input
    #[test]
    fn op_log_parser_does_not_panic(input in ".*") {
        let _ = Parser::parse_op_log(&input);
    }

    /// Resolve list parser should not panic on arbitrary input
    #[test]
    fn resolve_parser_does_not_panic(input in ".*") {
        let _ = Parser::parse_resolve_list(&input);
    }
}

// =============================================================================
// Structured input tests: parsers handle well-formed input correctly
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Log parser handles structured log records
    #[test]
    fn log_parser_handles_structured_input(
        change_id in change_id_strategy(),
        commit_id in commit_id_strategy(),
        desc in description_strategy(),
        author in "[a-z]+@[a-z]+\\.[a-z]{2,3}",
        timestamp in "[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}",
        bookmarks in prop::option::of("[a-z]+"),
    ) {
        // Construct a log record in expected format (tab-separated)
        let bookmark_str = bookmarks.unwrap_or_default();
        let record = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t\t",
            change_id, commit_id, desc, author, timestamp, bookmark_str
        );

        let result = Parser::parse_log(&record);
        // Should parse without error (may be empty if format doesn't match exactly)
        prop_assert!(result.is_ok(), "Failed to parse: {:?}", result);
    }

    /// Status parser handles file status lines
    #[test]
    fn status_parser_handles_status_lines(
        status_char in prop::sample::select(vec!['A', 'M', 'D', 'R', 'C']),
        path in file_path_strategy(),
    ) {
        let line = format!("{} {}", status_char, path);
        let input = format!("Working copy changes:\n{}\n", line);

        let result = Parser::parse_status(&input);
        prop_assert!(result.is_ok(), "Failed to parse status: {:?}", result);
    }

    /// Bookmark parser handles local bookmark lines (tab-separated format)
    #[test]
    fn bookmark_parser_handles_local_bookmark(
        name in "[a-z][a-z0-9_/-]{1,30}",
        is_tracked in prop::bool::ANY,
    ) {
        // Local bookmark format: name\ttracked (2 fields)
        let tracked_str = if is_tracked { "true" } else { "false" };
        let line = format!("{}\t{}\n", name, tracked_str);

        let bookmarks = parse_bookmark_list(&line);

        // Invariant: valid local bookmark should parse correctly
        prop_assert_eq!(bookmarks.len(), 1, "Should parse one bookmark");
        prop_assert_eq!(&bookmarks[0].name, &name, "Name should match");
        prop_assert!(bookmarks[0].remote.is_none(), "Local bookmark has no remote");
        prop_assert_eq!(bookmarks[0].is_tracked, is_tracked, "Tracked flag should match");
    }

    /// Bookmark parser handles remote bookmark lines (tab-separated format)
    #[test]
    fn bookmark_parser_handles_remote_bookmark(
        name in "[a-z][a-z0-9_/-]{1,30}",
        remote in "[a-z]+",
        is_tracked in prop::bool::ANY,
    ) {
        // Remote bookmark format: name\tremote\ttracked (3 fields)
        let tracked_str = if is_tracked { "true" } else { "false" };
        let line = format!("{}\t{}\t{}\n", name, remote, tracked_str);

        let bookmarks = parse_bookmark_list(&line);

        // Invariant: valid remote bookmark should parse correctly
        prop_assert_eq!(bookmarks.len(), 1, "Should parse one bookmark");
        prop_assert_eq!(&bookmarks[0].name, &name, "Name should match");
        prop_assert_eq!(bookmarks[0].remote.as_deref(), Some(remote.as_str()), "Remote should match");
        prop_assert_eq!(bookmarks[0].is_tracked, is_tracked, "Tracked flag should match");
    }
}

// =============================================================================
// Edge case tests
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Parser handles lines with many tabs
    #[test]
    fn log_parser_handles_many_tabs(num_tabs in 1usize..20) {
        let input = "\t".repeat(num_tabs);
        let _ = Parser::parse_log(&input);
    }

    /// Parser handles very long lines
    #[test]
    fn log_parser_handles_long_lines(len in 100usize..10000) {
        let input = "a".repeat(len);
        let _ = Parser::parse_log(&input);
    }

    /// Parser handles unicode
    #[test]
    fn log_parser_handles_unicode(s in "\\PC{1,100}") {
        let _ = Parser::parse_log(&s);
    }

    /// Status parser handles empty and whitespace
    #[test]
    fn status_parser_handles_whitespace(
        spaces in " {0,10}",
        newlines in "\n{0,5}",
    ) {
        let input = format!("{}{}", spaces, newlines);
        let _ = Parser::parse_status(&input);
    }
}

// =============================================================================
// FileState indicator round-trip
// =============================================================================

#[test]
fn file_state_indicator_values() {
    // Verify known indicators
    use tij::model::FileStatus;

    let states = [
        (FileState::Added, 'A'),
        (FileState::Modified, 'M'),
        (FileState::Deleted, 'D'),
        (FileState::Conflicted, 'C'),
    ];

    for (state, expected_char) in states {
        let status = FileStatus {
            path: "test.rs".to_string(),
            state,
        };
        assert_eq!(status.indicator(), expected_char);
    }
}
