//! jj output parser
//!
//! Parses the output from jj commands into structured data.

use super::JjError;
use super::template::FIELD_SEPARATOR;
use crate::model::{Change, FileState, FileStatus, Status};

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
}
