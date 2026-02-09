//! Status output parser (jj status)

use super::super::JjError;
use crate::model::{FileState, FileStatus, Status};

use super::Parser;

impl Parser {
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
            // Format: "Working copy  (@) : <change_id> <commit_id> <description>"
            if line.starts_with("Working copy")
                && let Some(colon_pos) = line.find(": ")
            {
                let info = &line[colon_pos + 2..];
                if let Some(change_id) = info.split_whitespace().next() {
                    working_copy_change_id = change_id.to_string();
                }
            }

            // Parse parent commit info
            // Format: "Parent commit (@-): <change_id> <commit_id> <description>"
            if line.starts_with("Parent commit")
                && let Some(colon_pos) = line.find(": ")
            {
                let info = &line[colon_pos + 2..];
                if let Some(change_id) = info.split_whitespace().next() {
                    parent_change_id = change_id.to_string();
                }
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
    ///
    /// Formats:
    /// - "A path" (added)
    /// - "M path" (modified)
    /// - "D path" (deleted)
    /// - "R prefix{old => new}" (renamed, jj format)
    /// - "C path" (conflicted)
    pub(super) fn parse_status_line(line: &str) -> Option<FileStatus> {
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
                // Renamed: "R prefix{old => new}" (jj format)
                if let Some(brace_start) = rest.find('{')
                    && let Some(brace_end) = rest.find('}')
                {
                    let prefix = &rest[..brace_start];
                    let inner = &rest[brace_start + 1..brace_end];
                    if let Some((old_part, new_part)) = inner.split_once(" => ") {
                        let from = format!("{}{}", prefix, old_part);
                        let to = format!("{}{}", prefix, new_part);
                        return Some(FileStatus {
                            path: to,
                            state: FileState::Renamed { from },
                        });
                    }
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
