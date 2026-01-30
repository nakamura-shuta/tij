//! jj output parser
//!
//! Parses the output from jj commands into structured data.

use super::JjError;
use super::template::FIELD_SEPARATOR;
use crate::model::{Change, DiffContent, DiffLine, DiffLineKind, FileState, FileStatus, Status};

/// Parser for jj command output
pub struct Parser;

impl Parser {
    /// Parse `jj log` output into a list of Changes
    ///
    /// Handles graph output with TAB-based detection:
    /// - Lines with TAB: Change lines (graph prefix + TAB-separated fields)
    /// - Lines without TAB: Graph-only lines (branch/merge lines)
    pub fn parse_log(output: &str) -> Result<Vec<Change>, JjError> {
        let mut changes = Vec::new();

        for line in output.lines() {
            if line.is_empty() {
                continue;
            }

            // TAB presence determines line type
            if let Some(tab_pos) = line.find(FIELD_SEPARATOR) {
                // Change line: extract graph prefix and parse fields
                let graph_and_id = &line[..tab_pos];
                let data_fields = &line[tab_pos + 1..];

                let (graph_prefix, change_id) = Self::split_graph_prefix(graph_and_id)?;
                let mut change = Self::parse_log_fields(change_id, data_fields)?;
                change.graph_prefix = graph_prefix;
                change.is_graph_only = false;
                changes.push(change);
            } else {
                // Graph-only line (no TAB = no data fields)
                changes.push(Change {
                    graph_prefix: line.to_string(),
                    is_graph_only: true,
                    ..Default::default()
                });
            }
        }

        Ok(changes)
    }

    /// Split graph prefix and change_id from the part before TAB
    ///
    /// Input: "│ │ ○  oqwroxvu"
    /// Output: Ok(("│ │ ○  ", "oqwroxvu"))
    ///
    /// jj's change_id uses "reversed hex" encoding with lowercase letters only.
    /// The template uses `.short(8)` which outputs `[a-z]{8}`.
    fn split_graph_prefix(graph_and_id: &str) -> Result<(String, &str), JjError> {
        let bytes = graph_and_id.as_bytes();
        let mut id_start = bytes.len();

        // Find where the change_id starts (consecutive lowercase letters from end)
        for i in (0..bytes.len()).rev() {
            if bytes[i].is_ascii_lowercase() {
                id_start = i;
            } else if id_start < bytes.len() {
                // Hit non-lowercase after finding some lowercase chars
                break;
            }
        }

        if id_start < bytes.len() {
            let graph_prefix = graph_and_id[..id_start].to_string();
            let change_id = &graph_and_id[id_start..];
            Ok((graph_prefix, change_id))
        } else {
            // TAB exists but no change_id found - invalid format
            Err(JjError::ParseError(format!(
                "Cannot extract change_id from: {}",
                graph_and_id
            )))
        }
    }

    /// Parse TAB-separated fields after change_id
    ///
    /// Fields: commit_id, author, timestamp, description, is_working_copy, is_empty, bookmarks
    fn parse_log_fields(change_id: &str, data: &str) -> Result<Change, JjError> {
        let fields: Vec<&str> = data.split(FIELD_SEPARATOR).collect();

        if fields.len() < 6 {
            return Err(JjError::ParseError(format!(
                "Expected at least 6 fields after change_id, got {}: {:?}",
                fields.len(),
                fields
            )));
        }

        Ok(Change {
            change_id: change_id.to_string(),
            commit_id: fields[0].to_string(),
            author: fields[1].to_string(),
            timestamp: fields[2].to_string(),
            description: fields[3].to_string(),
            is_working_copy: fields[4] == "true",
            is_empty: fields[5] == "true",
            bookmarks: if fields.len() > 6 && !fields[6].is_empty() {
                fields[6].split(',').map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            },
            graph_prefix: String::new(), // Set by caller
            is_graph_only: false,
        })
    }

    // Legacy function for tests - kept for backwards compatibility
    #[cfg(test)]
    fn parse_log_record(record: &str) -> Result<Change, JjError> {
        let fields: Vec<&str> = record.split(FIELD_SEPARATOR).collect();

        if fields.len() < 7 {
            return Err(JjError::ParseError(format!(
                "Expected at least 7 fields, got {}: {:?}",
                fields.len(),
                fields
            )));
        }

        Ok(Change {
            change_id: fields[0].to_string(),
            commit_id: fields[1].to_string(),
            author: fields[2].to_string(),
            timestamp: fields[3].to_string(),
            description: fields[4].to_string(),
            is_working_copy: fields[5] == "true",
            is_empty: fields[6] == "true",
            bookmarks: if fields.len() > 7 && !fields[7].is_empty() {
                fields[7].split(',').map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            },
            graph_prefix: String::new(),
            is_graph_only: false,
        })
    }

    /// Parse `jj status` output
    pub fn parse_status(output: &str) -> Result<Status, JjError> {
        let mut files = Vec::new();
        let mut has_conflicts = false;
        let mut working_copy_change_id = String::new();
        let mut parent_change_id = String::new();

        for line in output.lines() {
            let line = line.trim();

            // Parse file status lines
            if let Some(file_status) = Self::parse_status_line(line) {
                if matches!(file_status.state, FileState::Conflicted) {
                    has_conflicts = true;
                }
                files.push(file_status);
            }

            // Parse working copy info
            if line.starts_with("Working copy")
                && let Some(rest) = line.strip_prefix("Working copy")
                && let Some(info) = rest.strip_prefix(" : ").or(rest.strip_prefix(": "))
                && let Some(change_id) = info.split_whitespace().next()
            {
                working_copy_change_id = change_id.to_string();
            }

            // Parse parent commit info
            if line.starts_with("Parent commit")
                && let Some(rest) = line.strip_prefix("Parent commit")
                && let Some(info) = rest.strip_prefix(" : ").or(rest.strip_prefix(": "))
                && let Some(change_id) = info.split_whitespace().next()
            {
                parent_change_id = change_id.to_string();
            }
        }

        Ok(Status {
            files,
            has_conflicts,
            working_copy_change_id,
            parent_change_id,
        })
    }

    /// Parse `jj show` output into DiffContent
    ///
    /// Format:
    /// ```text
    /// Commit ID: <hash>
    /// Change ID: <hash>
    /// Author   : Name <email> (timestamp)
    /// Committer: Name <email> (timestamp)
    ///
    ///     Description text
    ///
    /// Modified regular file src/main.rs:
    ///    10   10:     fn main() {
    /// ```
    pub fn parse_show(output: &str) -> Result<DiffContent, JjError> {
        let mut content = DiffContent::default();
        let mut description_lines = Vec::new();
        let mut file_count = 0;
        let mut header_done = false;
        let mut in_diff_section = false;

        for line in output.lines() {
            // Parse header fields (before diff section)
            if !header_done {
                if let Some(commit_id) = line.strip_prefix("Commit ID: ") {
                    content.commit_id = commit_id.trim().to_string();
                    continue;
                }

                if let Some(author_line) = line.strip_prefix("Author   : ") {
                    if let Some((author, timestamp)) = Self::parse_author_line(author_line) {
                        content.author = author;
                        content.timestamp = timestamp;
                    }
                    continue;
                }

                // Skip Change ID and Committer lines
                if line.starts_with("Change ID: ") || line.starts_with("Committer: ") {
                    continue;
                }

                // Empty line in header section - skip
                if line.is_empty() && description_lines.is_empty() {
                    continue;
                }

                // Description lines are indented with 4 spaces
                if line.starts_with("    ") {
                    description_lines.push(line.trim_start().to_string());
                    continue;
                }

                // Empty line after description marks end of header
                if line.is_empty() && !description_lines.is_empty() {
                    content.description = description_lines.join("\n");
                    description_lines.clear();
                    header_done = true;
                    continue;
                }
            }

            // File header detection - marks start of diff section
            if let Some(path) = Self::extract_file_path(line) {
                // If we haven't saved description yet, do it now
                if !description_lines.is_empty() {
                    content.description = description_lines.join("\n");
                    description_lines.clear();
                }
                header_done = true;
                in_diff_section = true;

                // Add separator before file (except first file)
                if file_count > 0 {
                    content.lines.push(DiffLine::separator());
                }
                content.lines.push(DiffLine::file_header(path));
                file_count += 1;
                continue;
            }

            // Diff line parsing (only after we're in diff section)
            if in_diff_section {
                if let Some(diff_line) = Self::parse_diff_line(line) {
                    content.lines.push(diff_line);
                }
            }
        }

        // Handle description if no empty line after it
        if !description_lines.is_empty() {
            content.description = description_lines.join("\n");
        }

        Ok(content)
    }

    /// Parse author line "Name <email> (timestamp)" into (author, timestamp)
    fn parse_author_line(line: &str) -> Option<(String, String)> {
        // Find the timestamp in parentheses at the end
        if let Some(ts_start) = line.rfind('(') {
            if let Some(ts_end) = line.rfind(')') {
                let author = line[..ts_start].trim().to_string();
                let timestamp = line[ts_start + 1..ts_end].to_string();
                return Some((author, timestamp));
            }
        }
        // Fallback: whole line is author
        Some((line.trim().to_string(), String::new()))
    }

    /// Extract file path from file header line
    ///
    /// Examples:
    /// - "Modified regular file src/main.rs:" -> "src/main.rs"
    /// - "Added regular file src/new.rs:" -> "src/new.rs"
    fn extract_file_path(line: &str) -> Option<String> {
        // Patterns: "Modified regular file", "Added regular file", "Deleted regular file"
        let patterns = [
            "Modified regular file ",
            "Added regular file ",
            "Deleted regular file ",
            "Renamed regular file ",
            "Copied regular file ",
        ];

        for pattern in patterns {
            if let Some(rest) = line.strip_prefix(pattern) {
                // Remove trailing ":"
                let path = rest.strip_suffix(':').unwrap_or(rest);
                return Some(path.to_string());
            }
        }

        None
    }

    /// Parse a diff line with line numbers
    ///
    /// Format examples:
    /// - "   10   10:     fn main() {"      (context)
    /// - "   11     : -       old"          (deleted)
    /// - "        11: +       new"          (added)
    fn parse_diff_line(line: &str) -> Option<DiffLine> {
        // Skip empty lines or lines without the colon separator
        if !line.contains(':') {
            return None;
        }

        // Split at the first colon after line numbers
        let colon_pos = line.find(':')?;
        let line_num_part = &line[..colon_pos];
        let content_part = &line[colon_pos + 1..];

        // Parse line numbers (format: "  old  new" where either can be blank)
        let (old_line, new_line) = Self::parse_line_numbers(line_num_part);

        // Determine line kind from content prefix
        let content_trimmed = content_part.trim_start();
        let (kind, content) = if let Some(rest) = content_trimmed.strip_prefix("+ ") {
            (DiffLineKind::Added, rest.to_string())
        } else if let Some(rest) = content_trimmed.strip_prefix("- ") {
            (DiffLineKind::Deleted, rest.to_string())
        } else if content_trimmed.starts_with('+') && content_trimmed.len() == 1 {
            // Empty added line
            (DiffLineKind::Added, String::new())
        } else if content_trimmed.starts_with('-') && content_trimmed.len() == 1 {
            // Empty deleted line
            (DiffLineKind::Deleted, String::new())
        } else {
            // Context line - preserve original content after colon
            (DiffLineKind::Context, content_part.to_string())
        };

        Some(DiffLine {
            kind,
            line_numbers: Some((old_line, new_line)),
            content,
        })
    }

    /// Parse line numbers from the prefix part before ':'
    ///
    /// Format: "  old  new" where numbers are right-aligned
    fn parse_line_numbers(s: &str) -> (Option<usize>, Option<usize>) {
        let parts: Vec<&str> = s.split_whitespace().collect();

        match parts.len() {
            0 => (None, None),
            1 => {
                // Single number - need to determine if old or new
                // If there's leading space, it might be new line only
                let num = parts[0].parse().ok();
                // Check if number is at the end (new line) or start (old line)
                if s.trim_start().len() < s.len() / 2 {
                    (num, None)
                } else {
                    (None, num)
                }
            }
            _ => {
                // Two numbers: old and new
                let old = parts[0].parse().ok();
                let new = parts[1].parse().ok();
                (old, new)
            }
        }
    }

    /// Parse a single status line into FileStatus
    fn parse_status_line(line: &str) -> Option<FileStatus> {
        // Status line format: "X path" or "X path -> new_path"
        if line.len() < 2 {
            return None;
        }

        let status_char = line.chars().next()?;
        let rest = line.get(2..)?.trim();

        if rest.is_empty() {
            return None;
        }

        let state = match status_char {
            'A' => FileState::Added,
            'M' => FileState::Modified,
            'D' => FileState::Deleted,
            'R' => {
                // Renamed: "R old_path -> new_path"
                if let Some((from, to)) = rest.split_once(" -> ") {
                    return Some(FileStatus {
                        path: to.to_string(), // 新パスを使用
                        state: FileState::Renamed {
                            from: from.to_string(),
                        },
                    });
                }
                return None;
            }
            'C' => FileState::Conflicted,
            _ => return None,
        };

        Some(FileStatus {
            path: rest.to_string(),
            state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_parse_status_line_renamed() {
        let file = Parser::parse_status_line("R old_name.rs -> new_name.rs").unwrap();
        // path should be the NEW path, not "old -> new"
        assert_eq!(file.path, "new_name.rs");
        match file.state {
            FileState::Renamed { from } => assert_eq!(from, "old_name.rs"),
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
    fn test_extract_file_path() {
        assert_eq!(
            Parser::extract_file_path("Modified regular file src/main.rs:"),
            Some("src/main.rs".to_string())
        );
        assert_eq!(
            Parser::extract_file_path("Added regular file src/new.rs:"),
            Some("src/new.rs".to_string())
        );
        assert_eq!(
            Parser::extract_file_path("Deleted regular file old.txt:"),
            Some("old.txt".to_string())
        );
        assert_eq!(Parser::extract_file_path("Some other line"), None);
    }

    #[test]
    fn test_parse_author_line() {
        let (author, ts) =
            Parser::parse_author_line("Test User <test@example.com> (2024-01-30 12:00:00)")
                .unwrap();
        assert_eq!(author, "Test User <test@example.com>");
        assert_eq!(ts, "2024-01-30 12:00:00");
    }

    #[test]
    fn test_parse_diff_line_context() {
        let line = Parser::parse_diff_line("   10   10:     fn main() {").unwrap();
        assert_eq!(line.kind, DiffLineKind::Context);
        assert_eq!(line.line_numbers, Some((Some(10), Some(10))));
    }

    #[test]
    fn test_parse_diff_line_added() {
        let line = Parser::parse_diff_line("        11: +       println!(\"new\");").unwrap();
        assert_eq!(line.kind, DiffLineKind::Added);
    }

    #[test]
    fn test_parse_diff_line_deleted() {
        let line = Parser::parse_diff_line("   11     : -       println!(\"old\");").unwrap();
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
}
