//! Diff output parser (jj show)

use super::super::JjError;
use super::Parser;
use crate::model::{CommitId, DiffContent, DiffLine, DiffLineKind, FileOperation};

impl Parser {
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
        // Header (Commit ID, Author, Bookmarks, Tags, description, …) is
        // parsed by the shared helper — this keeps the skip-list of header
        // fields in ONE place (a previous duplicate here missed Bookmarks/
        // Tags lines, losing the description for bookmarked changes).
        let (mut content, body_start) = Self::parse_show_header(output);
        let body_content = Self::parse_diff_body(&output[body_start..]);
        content.lines = body_content.lines;
        Ok(content)
    }

    /// Parse multi-revision `jj show <revset>` output (jj 0.42+) into a single
    /// DiffContent (Stack mode)
    ///
    /// The output contains one `parse_show()`-style block per revision, each
    /// starting with a `Commit ID: ` line (newest first). Each block is parsed
    /// with `inner` and prefixed with a `ChangeHeader` boundary line
    /// (`◉ <change_short> <description first line>`).
    ///
    /// Combined metadata: `commit_id`/`author`/`timestamp` come from the first
    /// (newest) block; `description` becomes a per-change summary list so the
    /// Diff View header shows a stack overview.
    fn parse_show_stack_with(
        output: &str,
        inner: fn(&str) -> Result<DiffContent, JjError>,
    ) -> Result<DiffContent, JjError> {
        let mut combined = DiffContent::default();
        let mut summary_lines = Vec::new();

        for block in Self::split_show_blocks(output) {
            let change_short = block
                .lines()
                .find_map(|l| l.strip_prefix("Change ID: "))
                .map(|id| id.trim().chars().take(8).collect::<String>())
                .unwrap_or_default();

            let content = inner(block)?;

            let desc_first = content
                .description
                .lines()
                .next()
                .unwrap_or("(no description)")
                .to_string();

            // First (newest) block provides the combined metadata
            if combined.commit_id.as_str().is_empty() {
                combined.commit_id = content.commit_id.clone();
                combined.author = content.author.clone();
                combined.timestamp = content.timestamp.clone();
            }
            summary_lines.push(format!("{} {}", change_short, desc_first));

            // Change boundary header, then the block's diff lines
            if !combined.lines.is_empty() {
                combined.lines.push(DiffLine::separator());
            }
            combined.lines.push(DiffLine::change_header(format!(
                "◉ {} {}",
                change_short, desc_first
            )));
            combined.lines.extend(content.lines);
        }

        combined.description = summary_lines.join("\n");
        Ok(combined)
    }

    /// Split multi-revision `jj show` output into per-revision blocks at
    /// `Commit ID: ` boundary lines.
    fn split_show_blocks(output: &str) -> Vec<&str> {
        let mut starts: Vec<usize> = Vec::new();
        let mut pos = 0;
        for line in output.split_inclusive('\n') {
            if line.starts_with("Commit ID: ") {
                starts.push(pos);
            }
            pos += line.len();
        }
        // Handle output not ending with '\n' is covered: split_inclusive yields
        // the final partial line too.
        let mut blocks = Vec::with_capacity(starts.len());
        for (i, &start) in starts.iter().enumerate() {
            let end = starts.get(i + 1).copied().unwrap_or(output.len());
            blocks.push(&output[start..end]);
        }
        blocks
    }

    /// Parse multi-revision `jj show <revset>` (color-words) output — Stack mode
    pub fn parse_show_stack(output: &str) -> Result<DiffContent, JjError> {
        Self::parse_show_stack_with(output, Self::parse_show)
    }

    /// Parse multi-revision `jj show <revset> --stat` output — Stack mode
    pub fn parse_show_stack_stat(output: &str) -> Result<DiffContent, JjError> {
        Self::parse_show_stack_with(output, Self::parse_show_stat)
    }

    /// Parse multi-revision `jj show <revset> --git` output — Stack mode
    pub fn parse_show_stack_git(output: &str) -> Result<DiffContent, JjError> {
        Self::parse_show_stack_with(output, Self::parse_show_git)
    }

    /// Parse `jj diff --from --to` output into DiffContent
    ///
    /// Unlike `parse_show()`, this output has no header (no Commit ID, Author, etc.)
    /// It starts directly with file headers like "Modified regular file src/main.rs:"
    pub fn parse_diff_body(output: &str) -> DiffContent {
        let mut content = DiffContent::default();
        let mut file_count = 0;
        let mut current_file_op = FileOperation::Modified;

        for line in output.lines() {
            // File header detection
            if let Some((path, file_op)) = Self::extract_file_info(line) {
                current_file_op = file_op;

                // Add separator before file (except first file)
                if file_count > 0 {
                    content.lines.push(DiffLine::separator());
                }
                content
                    .lines
                    .push(DiffLine::file_header_with_op(path, file_op));
                file_count += 1;
                continue;
            }

            // Diff line parsing (only after we've seen at least one file header)
            if file_count > 0
                && let Some(diff_line) = Self::parse_diff_line(line, current_file_op)
            {
                content.lines.push(diff_line);
            }
        }

        content
    }

    /// Parse author line "Name <email> (timestamp)" into (author, timestamp)
    pub(super) fn parse_author_line(line: &str) -> Option<(String, String)> {
        // Find the timestamp in parentheses at the end
        if let Some(ts_start) = line.rfind('(')
            && let Some(ts_end) = line.rfind(')')
        {
            let author = line[..ts_start].trim().to_string();
            let timestamp = line[ts_start + 1..ts_end].to_string();
            return Some((author, timestamp));
        }
        // Fallback: whole line is author
        Some((line.trim().to_string(), String::new()))
    }

    /// Extract file path and operation type from file header line
    ///
    /// Examples:
    /// - "Modified regular file src/main.rs:" -> ("src/main.rs", Modified)
    /// - "Added regular file src/new.rs:" -> ("src/new.rs", Added)
    /// - "Created conflict in test.txt:" -> ("test.txt", Modified)
    /// - "Resolved conflict in test.txt:" -> ("test.txt", Modified)
    pub(super) fn extract_file_info(line: &str) -> Option<(String, FileOperation)> {
        let patterns = [
            ("Added regular file ", FileOperation::Added),
            ("Removed regular file ", FileOperation::Deleted),
            ("Deleted regular file ", FileOperation::Deleted),
            ("Modified regular file ", FileOperation::Modified),
            ("Renamed regular file ", FileOperation::Modified),
            ("Copied regular file ", FileOperation::Added),
            ("Created conflict in ", FileOperation::Modified),
            ("Resolved conflict in ", FileOperation::Modified),
        ];

        for (pattern, op) in patterns {
            if let Some(rest) = line.strip_prefix(pattern) {
                // Remove trailing ":"
                let path = rest.strip_suffix(':').unwrap_or(rest);
                return Some((path.to_string(), op));
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
    /// - "        1: // content"            (added file - no prefix)
    pub(super) fn parse_diff_line(line: &str, file_op: FileOperation) -> Option<DiffLine> {
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
            // No explicit +/- prefix - determine kind from context
            let kind = match file_op {
                FileOperation::Added => DiffLineKind::Added,
                FileOperation::Deleted => DiffLineKind::Deleted,
                FileOperation::Modified => {
                    // For modified files, check line numbers to determine kind
                    match (old_line, new_line) {
                        (Some(_), Some(_)) => DiffLineKind::Context,
                        (None, Some(_)) => DiffLineKind::Added,
                        (Some(_), None) => DiffLineKind::Deleted,
                        (None, None) => DiffLineKind::Context, // fallback
                    }
                }
            };
            (kind, content_part.to_string())
        };

        Some(DiffLine {
            kind,
            line_numbers: Some((old_line, new_line)),
            content,
            file_op: None,
        })
    }

    /// Parse `jj show --stat` output into DiffContent
    ///
    /// The header (Commit ID, Author, etc.) is parsed the same way as `parse_show()`.
    /// The stat body lines are stored as plain-text DiffLines (no line numbers).
    pub fn parse_show_stat(output: &str) -> Result<DiffContent, JjError> {
        let (mut content, body_start) = Self::parse_show_header(output);

        let body = &output[body_start..];
        let trimmed = body.trim();
        if trimmed.is_empty() {
            // Empty commit: no changes
            content.lines.push(DiffLine {
                kind: DiffLineKind::Context,
                line_numbers: None,
                content: "(no changes)".to_string(),
                file_op: None,
            });
        } else {
            for line in body.lines() {
                content.lines.push(DiffLine {
                    kind: DiffLineKind::Context,
                    line_numbers: None,
                    content: line.to_string(),
                    file_op: None,
                });
            }
        }

        Ok(content)
    }

    /// Parse `jj diff --stat` (no header) output into DiffContent
    pub fn parse_diff_body_stat(output: &str) -> DiffContent {
        let mut content = DiffContent::default();
        let trimmed = output.trim();
        if trimmed.is_empty() {
            content.lines.push(DiffLine {
                kind: DiffLineKind::Context,
                line_numbers: None,
                content: "(no changes)".to_string(),
                file_op: None,
            });
        } else {
            for line in output.lines() {
                content.lines.push(DiffLine {
                    kind: DiffLineKind::Context,
                    line_numbers: None,
                    content: line.to_string(),
                    file_op: None,
                });
            }
        }
        content
    }

    /// Parse `jj show --git` output into DiffContent
    ///
    /// The header (Commit ID, Author, etc.) is parsed the same way as `parse_show()`.
    /// The git diff body is parsed with proper header line detection to avoid
    /// misclassifying `--- a/...` / `+++ b/...` as Deleted/Added.
    pub fn parse_show_git(output: &str) -> Result<DiffContent, JjError> {
        let (mut content, body_start) = Self::parse_show_header(output);

        let body = &output[body_start..];
        Self::parse_git_diff_lines(body, &mut content);

        Ok(content)
    }

    /// Parse `jj diff --git --from --to` (no header) output into DiffContent
    pub fn parse_diff_body_git(output: &str) -> DiffContent {
        let mut content = DiffContent::default();
        Self::parse_git_diff_lines(output, &mut content);
        content
    }

    /// Parse git unified diff lines into DiffContent
    ///
    /// Detection order (important to avoid misclassification):
    /// 1. `diff --git` → FileHeader
    /// 2. `index ` → skip
    /// 3. `--- ` → skip (old file header)
    /// 4. `+++ ` → skip (new file header)
    /// 5. `@@ ` → Context (hunk header)
    /// 6. `+` → Added
    /// 7. `-` → Deleted
    /// 8. others → Context
    fn parse_git_diff_lines(body: &str, content: &mut DiffContent) {
        let mut file_count = 0;

        for line in body.lines() {
            if let Some(rest) = line.strip_prefix("diff --git ") {
                // Extract path from "a/<path> b/<path>"
                let path = if let Some(b_pos) = rest.find(" b/") {
                    rest[b_pos + 3..].to_string()
                } else {
                    rest.to_string()
                };
                if file_count > 0 {
                    content.lines.push(DiffLine::separator());
                }
                content.lines.push(DiffLine::file_header(path));
                file_count += 1;
            } else if line.starts_with("index ")
                || line.starts_with("--- ")
                || line.starts_with("+++ ")
            {
                // Git metadata headers — skip
            } else if line.starts_with("@@ ") {
                // Hunk header
                content.lines.push(DiffLine {
                    kind: DiffLineKind::Context,
                    line_numbers: None,
                    content: line.to_string(),
                    file_op: None,
                });
            } else if let Some(rest) = line.strip_prefix('+') {
                content.lines.push(DiffLine {
                    kind: DiffLineKind::Added,
                    line_numbers: None,
                    content: rest.to_string(),
                    file_op: None,
                });
            } else if let Some(rest) = line.strip_prefix('-') {
                content.lines.push(DiffLine {
                    kind: DiffLineKind::Deleted,
                    line_numbers: None,
                    content: rest.to_string(),
                    file_op: None,
                });
            } else {
                // Context line (leading space stripped if present)
                let ctx = line.strip_prefix(' ').unwrap_or(line);
                content.lines.push(DiffLine {
                    kind: DiffLineKind::Context,
                    line_numbers: None,
                    content: ctx.to_string(),
                    file_op: None,
                });
            }
        }
    }

    /// Extract the header section (Commit ID, Author, Description) from `jj show` output.
    ///
    /// Returns (DiffContent with header fields populated, byte offset where body starts).
    /// Shared between parse_show, parse_show_stat, and parse_show_git.
    fn parse_show_header(output: &str) -> (DiffContent, usize) {
        let mut content = DiffContent::default();
        let mut description_lines = Vec::new();
        let mut byte_offset = 0;

        for line in output.lines() {
            // Track byte offset (line + newline)
            let line_end = byte_offset + line.len() + 1; // +1 for '\n'

            if let Some(commit_id) = line.strip_prefix("Commit ID: ") {
                content.commit_id = CommitId::new(commit_id.trim().to_string());
                byte_offset = line_end;
                continue;
            }

            if let Some(author_line) = line.strip_prefix("Author   : ") {
                if let Some((author, timestamp)) = Self::parse_author_line(author_line) {
                    content.author = author;
                    content.timestamp = timestamp;
                }
                byte_offset = line_end;
                continue;
            }

            if line.starts_with("Change ID: ")
                || line.starts_with("Committer: ")
                || line.starts_with("Bookmarks: ")
                || line.starts_with("Tags     : ")
            {
                byte_offset = line_end;
                continue;
            }

            if line.is_empty() {
                if !description_lines.is_empty() {
                    description_lines.push(String::new());
                }
                byte_offset = line_end;
                continue;
            }

            if line.starts_with("    ") {
                description_lines.push(line.trim_start().to_string());
                byte_offset = line_end;
                continue;
            }

            // Non-header, non-description line — body starts here
            while description_lines.last().is_some_and(|l| l.is_empty()) {
                description_lines.pop();
            }
            if !description_lines.is_empty() {
                content.description = description_lines.join("\n");
            }
            // byte_offset points to the start of this line (body start)
            break;
        }

        // Handle case where output is all header (no body)
        if !description_lines.is_empty() && content.description.is_empty() {
            while description_lines.last().is_some_and(|l| l.is_empty()) {
                description_lines.pop();
            }
            content.description = description_lines.join("\n");
            byte_offset = output.len();
        }

        // Clamp to output length
        let body_start = byte_offset.min(output.len());
        (content, body_start)
    }

    /// Parse line numbers from the prefix part before ':'
    ///
    /// Format: "  old  new" where numbers are right-aligned
    pub(super) fn parse_line_numbers(s: &str) -> (Option<usize>, Option<usize>) {
        let parts: Vec<&str> = s.split_whitespace().collect();

        match parts.len() {
            0 => (None, None),
            1 => {
                // Single number - determine if old or new based on position
                let num = parts[0].parse().ok();
                let leading_spaces = s.len() - s.trim_start().len();
                if leading_spaces > s.len() / 2 {
                    // Number is right-aligned = new line number
                    (None, num)
                } else {
                    // Number is left-aligned = old line number
                    (num, None)
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
}
