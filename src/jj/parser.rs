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
    /// Each line represents one change, with fields separated by tabs.
    pub fn parse_log(output: &str) -> Result<Vec<Change>, JjError> {
        let mut changes = Vec::new();

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let change = Self::parse_log_record(line)?;
            changes.push(change);
        }

        Ok(changes)
    }

    /// Parse a single log record (one line, tab-separated fields)
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
}
