use super::*;
use crate::model::{DiffLineKind, FileState};

#[test]
fn test_parse_log_record() {
    // Tab-separated fields
    let record = "abc12345\tdef67890\tuser@example.com\t2024-01-29T15:30:00+0900\tInitial commit\ttrue\tfalse\tmain,feature";
    let change = Parser::parse_log_record(record).unwrap();

    assert_eq!(change.change_id, "abc12345");
    assert_eq!(change.commit_id, "def67890");
    assert_eq!(change.author, "user@example.com");
    assert_eq!(change.description, "Initial commit");
    assert!(change.is_working_copy);
    assert!(!change.is_empty);
    assert_eq!(change.bookmarks, vec!["main", "feature"]);
}

#[test]
fn test_parse_log_record_no_bookmarks() {
    let record =
        "abc12345\tdef67890\tuser@example.com\t2024-01-29T15:30:00+0900\tTest\tfalse\ttrue\t";
    let change = Parser::parse_log_record(record).unwrap();

    assert!(change.bookmarks.is_empty());
    assert!(change.is_empty);
}

#[test]
fn test_parse_log_record_empty_fields() {
    // Root commit has empty author and description
    let record = "zzzzzzzz\t00000000\t\t1970-01-01T00:00:00+0000\t\tfalse\ttrue\t";
    let change = Parser::parse_log_record(record).unwrap();

    assert_eq!(change.change_id, "zzzzzzzz");
    assert_eq!(change.commit_id, "00000000");
    assert_eq!(change.author, "");
    assert_eq!(change.description, "");
    assert!(!change.is_working_copy);
    assert!(change.is_empty);
}

#[test]
fn test_parse_log_multiple_records() {
    let output = "abc12345\tdef67890\tuser@example.com\t2024-01-29T15:30:00+0900\tFirst\ttrue\tfalse\t\n\
                  xyz98765\tuvw43210\tother@example.com\t2024-01-28T10:00:00+0900\tSecond\tfalse\tfalse\t\n";

    let changes = Parser::parse_log(output).unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].description, "First");
    assert_eq!(changes[1].description, "Second");
}

#[test]
fn test_parse_status_line_added() {
    let file = Parser::parse_status_line("A new_file.rs").unwrap();
    assert_eq!(file.path, "new_file.rs");
    assert!(matches!(file.state, FileState::Added));
}

#[test]
fn test_parse_status_line_modified() {
    let file = Parser::parse_status_line("M src/main.rs").unwrap();
    assert_eq!(file.path, "src/main.rs");
    assert!(matches!(file.state, FileState::Modified));
}

#[test]
fn test_parse_status_line_deleted() {
    let file = Parser::parse_status_line("D old_file.txt").unwrap();
    assert_eq!(file.path, "old_file.txt");
    assert!(matches!(file.state, FileState::Deleted));
}

#[test]
fn test_parse_status_line_conflicted() {
    let file = Parser::parse_status_line("C conflicted.rs").unwrap();
    assert_eq!(file.path, "conflicted.rs");
    assert!(matches!(file.state, FileState::Conflicted));
}

#[test]
fn test_parse_status_output() {
    let output = r#"Working copy changes:
A new_file.rs
M src/main.rs
Working copy : abc12345 def67890 (empty) (no description set)
Parent commit: xyz98765 uvw43210 Initial commit"#;

    let status = Parser::parse_status(output).unwrap();
    assert_eq!(status.files.len(), 2);
    assert!(!status.has_conflicts);
    assert_eq!(status.working_copy_change_id, "abc12345");
    assert_eq!(status.parent_change_id, "xyz98765");
}

#[test]
fn test_parse_status_with_conflict() {
    let output = r#"Working copy changes:
C conflicted.rs
Working copy : abc12345 def67890 description
Parent commit: xyz98765 uvw43210 parent"#;

    let status = Parser::parse_status(output).unwrap();
    assert!(status.has_conflicts);
    assert_eq!(status.files.len(), 1);
    assert!(matches!(status.files[0].state, FileState::Conflicted));
}

#[test]
fn test_parse_status_with_markers() {
    // jj 0.37+ format with (@) and (@-) markers
    let output = r#"Working copy changes:
A new_file.rs
M src/main.rs
Working copy  (@) : abc12345 def67890 (empty) (no description set)
Parent commit (@-): xyz98765 uvw43210 Initial commit"#;

    let status = Parser::parse_status(output).unwrap();
    assert_eq!(status.files.len(), 2);
    assert_eq!(status.working_copy_change_id, "abc12345");
    assert_eq!(status.parent_change_id, "xyz98765");
}

#[test]
fn test_parse_status_line_renamed() {
    // jj format: "R prefix{old => new}"
    let file = Parser::parse_status_line("R src/{old_name.rs => new_name.rs}").unwrap();
    assert_eq!(file.path, "src/new_name.rs");
    match file.state {
        FileState::Renamed { from } => assert_eq!(from, "src/old_name.rs"),
        _ => panic!("Expected Renamed state"),
    }
}

#[test]
fn test_parse_status_line_renamed_no_prefix() {
    // jj format without common prefix
    let file = Parser::parse_status_line("R {old.rs => new.rs}").unwrap();
    assert_eq!(file.path, "new.rs");
    match file.state {
        FileState::Renamed { from } => assert_eq!(from, "old.rs"),
        _ => panic!("Expected Renamed state"),
    }
}

#[test]
fn test_parse_status_line_renamed_deep_path() {
    // Nested path
    let file = Parser::parse_status_line("R src/ui/views/{status.rs => status_view.rs}").unwrap();
    assert_eq!(file.path, "src/ui/views/status_view.rs");
    match file.state {
        FileState::Renamed { from } => assert_eq!(from, "src/ui/views/status.rs"),
        _ => panic!("Expected Renamed state"),
    }
}

// =========================================================================
// parse_show tests
// =========================================================================

#[test]
fn test_parse_show_header() {
    let output = r#"Commit ID: abc123def456
Change ID: xyz789uvw012
Author   : Test User <test@example.com> (2024-01-30 12:00:00)
Committer: Test User <test@example.com> (2024-01-30 12:00:00)

    Add new feature
"#;
    let content = Parser::parse_show(output).unwrap();

    assert_eq!(content.commit_id, "abc123def456");
    assert_eq!(content.author, "Test User <test@example.com>");
    assert_eq!(content.timestamp, "2024-01-30 12:00:00");
    assert_eq!(content.description, "Add new feature");
}

#[test]
fn test_parse_show_empty_no_changes() {
    let output = r#"Commit ID: abc123
Change ID: xyz789
Author   : Test <test@example.com> (2024-01-30 12:00:00)
Committer: Test <test@example.com> (2024-01-30 12:00:00)

    (no description set)
"#;
    let content = Parser::parse_show(output).unwrap();

    assert_eq!(content.commit_id, "abc123");
    assert!(!content.has_changes());
    assert_eq!(content.file_count(), 0);
}

#[test]
fn test_parse_show_with_file_diff() {
    let output = r#"Commit ID: abc123
Change ID: xyz789
Author   : Test <test@example.com> (2024-01-30 12:00:00)
Committer: Test <test@example.com> (2024-01-30 12:00:00)

    Fix bug

Modified regular file src/main.rs:
   10   10:     fn main() {
   11     : -       println!("old");
        11: +       println!("new");
   12   12:     }
"#;
    let content = Parser::parse_show(output).unwrap();

    assert_eq!(content.commit_id, "abc123");
    assert_eq!(content.description, "Fix bug");
    assert!(content.has_changes());
    assert_eq!(content.file_count(), 1);

    // Check file header
    assert_eq!(content.lines[0].kind, DiffLineKind::FileHeader);
    assert_eq!(content.lines[0].content, "src/main.rs");
}

#[test]
fn test_parse_show_multiple_files() {
    let output = r#"Commit ID: abc123
Change ID: xyz789
Author   : Test <test@example.com> (2024-01-30 12:00:00)
Committer: Test <test@example.com> (2024-01-30 12:00:00)

    Add files

Added regular file src/new.rs:
    1: + pub fn hello() {}

Modified regular file src/lib.rs:
   10   10: mod existing;
        11: + mod new;
"#;
    let content = Parser::parse_show(output).unwrap();

    assert_eq!(content.file_count(), 2);

    // First file
    assert_eq!(content.lines[0].kind, DiffLineKind::FileHeader);
    assert_eq!(content.lines[0].content, "src/new.rs");

    // Separator before second file
    let sep_pos = content
        .lines
        .iter()
        .position(|l| l.kind == DiffLineKind::Separator)
        .unwrap();
    assert!(sep_pos > 0);

    // Second file header
    let second_header = content
        .lines
        .iter()
        .filter(|l| l.kind == DiffLineKind::FileHeader)
        .nth(1)
        .unwrap();
    assert_eq!(second_header.content, "src/lib.rs");
}

#[test]
fn test_parse_show_conflict_diff() {
    let output = "Commit ID: c285b17e
Change ID: lqxuvokn
Author   : Test <test@example.com> (2026-02-05 10:33:31)
Committer: Test <test@example.com> (2026-02-05 10:33:50)

    branch B

Created conflict in test.txt:
   1     : branch A content
        1: <<<<<<< conflict 1 of 1
        2: %%%%%%% changes from initial
        3: -line 1
        4: +branch A content
        5: +++++++ branch B
        6: branch B content
        7: >>>>>>> conflict 1 of 1 ends
";
    let content = Parser::parse_show(output).unwrap();

    assert_eq!(content.commit_id, "c285b17e");
    assert_eq!(content.description, "branch B");
    assert!(content.has_changes());
    assert_eq!(content.file_count(), 1);

    // File header is "test.txt"
    assert_eq!(content.lines[0].kind, DiffLineKind::FileHeader);
    assert_eq!(content.lines[0].content, "test.txt");

    // Diff lines were parsed (conflict markers appear as content)
    assert!(content.lines.len() > 1);
}

#[test]
fn test_extract_file_info() {
    let (path, op) = Parser::extract_file_info("Modified regular file src/main.rs:").unwrap();
    assert_eq!(path, "src/main.rs");
    assert_eq!(op, FileOperation::Modified);

    let (path, op) = Parser::extract_file_info("Added regular file src/new.rs:").unwrap();
    assert_eq!(path, "src/new.rs");
    assert_eq!(op, FileOperation::Added);

    let (path, op) = Parser::extract_file_info("Removed regular file old.txt:").unwrap();
    assert_eq!(path, "old.txt");
    assert_eq!(op, FileOperation::Deleted);

    let (path, op) = Parser::extract_file_info("Created conflict in test.txt:").unwrap();
    assert_eq!(path, "test.txt");
    assert_eq!(op, FileOperation::Modified);

    let (path, op) = Parser::extract_file_info("Resolved conflict in test.txt:").unwrap();
    assert_eq!(path, "test.txt");
    assert_eq!(op, FileOperation::Modified);

    assert!(Parser::extract_file_info("Some other line").is_none());
}

#[test]
fn test_parse_author_line() {
    let (author, ts) =
        Parser::parse_author_line("Test User <test@example.com> (2024-01-30 12:00:00)").unwrap();
    assert_eq!(author, "Test User <test@example.com>");
    assert_eq!(ts, "2024-01-30 12:00:00");
}

#[test]
fn test_parse_diff_line_context() {
    let line =
        Parser::parse_diff_line("   10   10:     fn main() {", FileOperation::Modified).unwrap();
    assert_eq!(line.kind, DiffLineKind::Context);
    assert_eq!(line.line_numbers, Some((Some(10), Some(10))));
}

#[test]
fn test_parse_diff_line_added() {
    let line = Parser::parse_diff_line(
        "        11: +       println!(\"new\");",
        FileOperation::Modified,
    )
    .unwrap();
    assert_eq!(line.kind, DiffLineKind::Added);
}

#[test]
fn test_parse_diff_line_deleted() {
    let line = Parser::parse_diff_line(
        "   11     : -       println!(\"old\");",
        FileOperation::Modified,
    )
    .unwrap();
    assert_eq!(line.kind, DiffLineKind::Deleted);
}

#[test]
fn test_parse_diff_line_added_file_no_prefix() {
    // Lines in added files don't have + prefix
    let line = Parser::parse_diff_line("        1: // Hotfix", FileOperation::Added).unwrap();
    assert_eq!(line.kind, DiffLineKind::Added);
}

#[test]
fn test_parse_diff_line_deleted_file_no_prefix() {
    // Lines in deleted files don't have - prefix
    let line = Parser::parse_diff_line("    1    : old content", FileOperation::Deleted).unwrap();
    assert_eq!(line.kind, DiffLineKind::Deleted);
}

// =========================================================================
// Graph parsing tests (Phase 3.5)
// =========================================================================

#[test]
fn test_split_graph_prefix_simple() {
    let (prefix, id) = Parser::split_graph_prefix("@  oqwroxvu").unwrap();
    assert_eq!(prefix, "@  ");
    assert_eq!(id, "oqwroxvu");
}

#[test]
fn test_split_graph_prefix_one_level() {
    let (prefix, id) = Parser::split_graph_prefix("│ ○  nuzyqrpm").unwrap();
    assert_eq!(prefix, "│ ○  ");
    assert_eq!(id, "nuzyqrpm");
}

#[test]
fn test_split_graph_prefix_two_level() {
    let (prefix, id) = Parser::split_graph_prefix("│ │ ○  uslxsspn").unwrap();
    assert_eq!(prefix, "│ │ ○  ");
    assert_eq!(id, "uslxsspn");
}

#[test]
fn test_split_graph_prefix_complex() {
    let (prefix, id) = Parser::split_graph_prefix("│ ○ │  rnstomqt").unwrap();
    assert_eq!(prefix, "│ ○ │  ");
    assert_eq!(id, "rnstomqt");
}

#[test]
fn test_split_graph_prefix_no_change_id() {
    // Graph-only lines shouldn't reach this function (filtered by TAB check)
    // but if they do, it should error
    let result = Parser::split_graph_prefix("│ ├─╮");
    assert!(result.is_err());
}

#[test]
fn test_parse_log_with_graph_simple() {
    let output = "@  oqwroxvu\t1f7a8c00\tuser@example.com\t2026-01-30T16:17:51+0900\texperimental: results\ttrue\tfalse\t\n\
                  ○  vxvxrlkn\tdd9bda5a\tuser@example.com\t2026-01-30T16:17:51+0900\texperimental: try\tfalse\tfalse\t";

    let changes = Parser::parse_log(output).unwrap();
    assert_eq!(changes.len(), 2);

    assert_eq!(changes[0].graph_prefix, "@  ");
    assert_eq!(changes[0].change_id, "oqwroxvu");
    assert!(changes[0].is_working_copy);
    assert!(!changes[0].is_graph_only);

    assert_eq!(changes[1].graph_prefix, "○  ");
    assert_eq!(changes[1].change_id, "vxvxrlkn");
    assert!(!changes[1].is_working_copy);
}

#[test]
fn test_parse_log_with_graph_branch() {
    // Simulates a branch with graph-only line
    let output = "@  oqwroxvu\t1f7a8c00\tuser@example.com\t2026-01-30T16:17:51+0900\tfeature\ttrue\tfalse\t\n\
                  │ ○  nuzyqrpm\t8b644ab5\tuser@example.com\t2026-01-30T16:17:46+0900\tmain\tfalse\tfalse\t\n\
                  ├─╯\n\
                  ○  basecommit\tbase1234\tuser@example.com\t2026-01-30T16:15:24+0900\tbase\tfalse\tfalse\t";

    let changes = Parser::parse_log(output).unwrap();
    assert_eq!(changes.len(), 4);

    // First change
    assert_eq!(changes[0].graph_prefix, "@  ");
    assert_eq!(changes[0].change_id, "oqwroxvu");
    assert!(!changes[0].is_graph_only);

    // Second change (in branch)
    assert_eq!(changes[1].graph_prefix, "│ ○  ");
    assert_eq!(changes[1].change_id, "nuzyqrpm");
    assert!(!changes[1].is_graph_only);

    // Graph-only line (branch merge)
    assert_eq!(changes[2].graph_prefix, "├─╯");
    assert!(changes[2].is_graph_only);
    assert!(changes[2].change_id.is_empty());

    // Base commit
    assert_eq!(changes[3].graph_prefix, "○  ");
    assert_eq!(changes[3].change_id, "basecommit");
    assert!(!changes[3].is_graph_only);
}

#[test]
fn test_parse_log_graph_only_lines() {
    // Various graph-only patterns
    let patterns = ["│ ├─╮", "├─╯ │", "├───╯", "│ │", "│"];

    for pattern in patterns {
        let changes = Parser::parse_log(pattern).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].is_graph_only);
        assert_eq!(changes[0].graph_prefix, pattern);
    }
}

#[test]
fn test_parse_log_empty_lines_skipped() {
    let output = "@  oqwroxvu\t1f7a8c00\tuser@example.com\t2026-01-30T16:17:51+0900\ttest\ttrue\tfalse\t\n\
                  \n\
                  ○  vxvxrlkn\tdd9bda5a\tuser@example.com\t2026-01-30T16:17:51+0900\ttest2\tfalse\tfalse\t";

    let changes = Parser::parse_log(output).unwrap();
    assert_eq!(changes.len(), 2);
}

// =========================================================================
// parse_op_log tests
// =========================================================================

#[test]
fn test_parse_op_log_single() {
    // Tab-separated: id, user, timestamp, description
    let output = "abc123def456\tuser@example.com\t5 minutes ago\tsnapshot working copy";

    let operations = Parser::parse_op_log(output).unwrap();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].id, "abc123def456");
    assert_eq!(operations[0].user, "user@example.com");
    assert_eq!(operations[0].timestamp, "5 minutes ago");
    assert_eq!(operations[0].description, "snapshot working copy");
    assert!(operations[0].is_current); // First operation is current
}

#[test]
fn test_parse_op_log_multiple() {
    let output = "abc123def456\tuser@example.com\t5 minutes ago\tsnapshot working copy\n\
                  xyz789uvw012\tuser@example.com\t10 minutes ago\tdescribe commit abc\n\
                  def456ghi789\tuser@example.com\t1 hour ago\tnew empty commit";

    let operations = Parser::parse_op_log(output).unwrap();
    assert_eq!(operations.len(), 3);

    // First is current
    assert!(operations[0].is_current);
    assert!(!operations[1].is_current);
    assert!(!operations[2].is_current);

    // Check order preserved
    assert_eq!(operations[0].description, "snapshot working copy");
    assert_eq!(operations[1].description, "describe commit abc");
    assert_eq!(operations[2].description, "new empty commit");
}

#[test]
fn test_parse_op_log_empty_lines_skipped() {
    let output = "abc123\tuser\t5 min ago\top1\n\n\nxyz789\tuser\t10 min ago\top2";

    let operations = Parser::parse_op_log(output).unwrap();
    assert_eq!(operations.len(), 2);
}

#[test]
fn test_parse_op_log_malformed_lines_skipped() {
    // Lines with fewer than 4 tab-separated fields should be skipped
    let output = "abc123\tuser\t5 min ago\tvalid op\n\
                  malformed line\n\
                  xyz789\tuser\t10 min ago\tanother valid op";

    let operations = Parser::parse_op_log(output).unwrap();
    assert_eq!(operations.len(), 2);
}

// =========================================================================
// parse_file_annotate tests
// =========================================================================

#[test]
fn test_parse_file_annotate_basic() {
    // Default output format: "<change_id> <author> <timestamp>  <line_number>: <content>"
    let output = "twzksoxt nakamura 2026-01-30 10:43:19    1: //! Tij\n\
                  twzksoxt nakamura 2026-01-30 10:43:19    2: //!\n\
                  qplomrst taro 2026-01-28 15:22:00    3: mod app;";

    let content = Parser::parse_file_annotate(output, "src/main.rs").unwrap();
    assert_eq!(content.file_path, "src/main.rs");
    assert_eq!(content.len(), 3);

    // First line - first in hunk
    assert_eq!(content.lines[0].change_id, "twzksoxt");
    assert_eq!(content.lines[0].author, "nakamura");
    assert_eq!(content.lines[0].line_number, 1);
    assert!(content.lines[0].first_in_hunk);
    assert_eq!(content.lines[0].content, "//! Tij");

    // Second line - continuation (same change_id)
    assert_eq!(content.lines[1].change_id, "twzksoxt");
    assert_eq!(content.lines[1].line_number, 2);
    assert!(!content.lines[1].first_in_hunk);

    // Third line - new hunk (different change_id)
    assert_eq!(content.lines[2].change_id, "qplomrst");
    assert_eq!(content.lines[2].line_number, 3);
    assert!(content.lines[2].first_in_hunk);
}

#[test]
fn test_parse_file_annotate_with_tabs_in_content() {
    // Content contains tabs - should be handled correctly
    let output = "twzksoxt nakamura 2026-01-30 10:43:19    1: \tindented\twith\ttabs";

    let content = Parser::parse_file_annotate(output, "test.rs").unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content.lines[0].content, "\tindented\twith\ttabs");
}

#[test]
fn test_parse_file_annotate_empty_content() {
    // Empty content after line number
    let output = "twzksoxt nakamura 2026-01-30 10:43:19    1:";

    let content = Parser::parse_file_annotate(output, "test.rs").unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content.lines[0].content, "");
}

#[test]
fn test_parse_file_annotate_skips_empty_lines() {
    // Empty lines in output are skipped
    let output = "twzksoxt nakamura 2026-01-30 10:43:19    1: line1\n\n\nqplomrst taro 2026-01-28 15:22:00    4: line4";

    let content = Parser::parse_file_annotate(output, "test.rs").unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content.lines[0].line_number, 1);
    assert_eq!(content.lines[1].line_number, 4); // Preserves original line numbers from jj
}

#[test]
fn test_parse_file_annotate_author_with_digits() {
    // Author name contains digits - should still parse correctly
    let output = "abc12345 user1 2026-01-30 10:43:19    1: some content";

    let content = Parser::parse_file_annotate(output, "test.rs").unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content.lines[0].change_id, "abc12345");
    assert_eq!(content.lines[0].author, "user1");
    assert_eq!(content.lines[0].line_number, 1);
    assert_eq!(content.lines[0].content, "some content");
}

#[test]
fn test_parse_file_annotate_content_with_colon_pattern() {
    // Content contains patterns like "1: foo" - should not confuse parser
    let output = "twzksoxt nakamura 2026-01-30 10:43:19   10: let x = 1: foo";

    let content = Parser::parse_file_annotate(output, "test.rs").unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content.lines[0].line_number, 10);
    assert_eq!(content.lines[0].content, "let x = 1: foo");
}

#[test]
fn test_parse_file_annotate_variable_change_id_length() {
    // change_id longer than 8 chars (if jj config changes)
    let output = "abcdefghij user 2026-01-30 10:43:19    1: content";

    let content = Parser::parse_file_annotate(output, "test.rs").unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content.lines[0].change_id, "abcdefghij");

    // change_id shorter than 8 chars
    let output2 = "abc user 2026-01-30 10:43:19    1: content";

    let content2 = Parser::parse_file_annotate(output2, "test.rs").unwrap();
    assert_eq!(content2.len(), 1);
    assert_eq!(content2.lines[0].change_id, "abc");
}

#[test]
fn test_parse_file_annotate_complex_author_name() {
    // Author with spaces (full name) or special characters
    let output = "twzksoxt John Doe 2026-01-30 10:43:19    1: content";

    let content = Parser::parse_file_annotate(output, "test.rs").unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content.lines[0].author, "John Doe");
}

// =========================================================================
// parse_resolve_list tests (Phase 9)
// =========================================================================

#[test]
fn test_parse_resolve_list_tab_delimiter() {
    let output = "test.txt\t2-sided conflict\n";
    let files = Parser::parse_resolve_list(output);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "test.txt");
    assert_eq!(files[0].description, "2-sided conflict");
}

#[test]
fn test_parse_resolve_list_space_delimiter() {
    // jj 0.37.x uses spaces (verified with xxd)
    let output = "test.txt    2-sided conflict\n";
    let files = Parser::parse_resolve_list(output);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "test.txt");
    assert_eq!(files[0].description, "2-sided conflict");
}

#[test]
fn test_parse_resolve_list_multiple_spaces() {
    let output = "src/main.rs    2-sided conflict\nsrc/lib.rs     3-sided conflict\n";
    let files = Parser::parse_resolve_list(output);
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, "src/main.rs");
    assert_eq!(files[0].description, "2-sided conflict");
    assert_eq!(files[1].path, "src/lib.rs");
    assert_eq!(files[1].description, "3-sided conflict");
}

#[test]
fn test_parse_resolve_list_path_with_spaces() {
    let output = "my file.txt    2-sided conflict\n";
    let files = Parser::parse_resolve_list(output);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "my file.txt");
    assert_eq!(files[0].description, "2-sided conflict");
}

#[test]
fn test_parse_resolve_list_empty() {
    let output = "";
    let files = Parser::parse_resolve_list(output);
    assert!(files.is_empty());
}

// =========================================================================
// conflict field in log parser tests (Phase 9)
// =========================================================================

#[test]
fn test_parse_log_with_conflict() {
    let record = "abc12345\tdef67890\tuser@example.com\t2026-01-01T00:00:00+0900\tdescription\ttrue\tfalse\tmain\ttrue";
    let change = Parser::parse_log_record(record).unwrap();
    assert!(change.has_conflict);
}

#[test]
fn test_parse_log_without_conflict() {
    let record = "abc12345\tdef67890\tuser@example.com\t2026-01-01T00:00:00+0900\tdescription\ttrue\tfalse\tmain\tfalse";
    let change = Parser::parse_log_record(record).unwrap();
    assert!(!change.has_conflict);
}

#[test]
fn test_parse_log_missing_conflict_field() {
    // Backward compat: old format without conflict field
    let record = "abc12345\tdef67890\tuser@example.com\t2026-01-01T00:00:00+0900\tdescription\ttrue\tfalse\tmain";
    let change = Parser::parse_log_record(record).unwrap();
    assert!(!change.has_conflict); // defaults to false
}

// =========================================================================
// Multi-line description tests (parse_show)
// =========================================================================

#[test]
fn test_parse_show_multiline_description() {
    let output = "Commit ID: abc123
Change ID: xyz789
Author   : Test <test@example.com> (2024-01-30 12:00:00)
Committer: Test <test@example.com> (2024-01-30 12:00:00)

    test-2

    test-3

Modified regular file src/main.rs:
   10   10:     fn main() {
";
    let content = Parser::parse_show(output).unwrap();

    assert_eq!(content.description, "test-2\n\ntest-3");
    assert_eq!(content.file_count(), 1);
}

#[test]
fn test_parse_show_multiline_description_no_diff() {
    // Empty commit with multi-line description (no file changes)
    let output = "Commit ID: abc123
Change ID: xyz789
Author   : Test <test@example.com> (2024-01-30 12:00:00)
Committer: Test <test@example.com> (2024-01-30 12:00:00)

    test-2

    test-3
";
    let content = Parser::parse_show(output).unwrap();

    assert_eq!(content.description, "test-2\n\ntest-3");
    assert_eq!(content.file_count(), 0);
}

#[test]
fn test_parse_show_three_paragraph_description() {
    let output = "Commit ID: abc123
Change ID: xyz789
Author   : Test <test@example.com> (2024-01-30 12:00:00)
Committer: Test <test@example.com> (2024-01-30 12:00:00)

    Summary line

    Detailed paragraph one.

    Detailed paragraph two.

Modified regular file src/main.rs:
   10   10:     fn main() {
";
    let content = Parser::parse_show(output).unwrap();

    assert_eq!(
        content.description,
        "Summary line\n\nDetailed paragraph one.\n\nDetailed paragraph two."
    );
}
