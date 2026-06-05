//! Diff View AI-range overlay (Phase 3)
//!
//! Maps trace line ranges (positions at the recorded revision) onto parsed
//! diff lines. Only color-words `jj show` output carries per-line numbers,
//! so the overlay applies to Single mode + ColorWords format; everything
//! else simply gets no marks.
//!
//! Precision note (SoW principle P3): writer-side line attribution is
//! approximate (upstream PR #7), and the recorded revision may be a working-
//! copy snapshot slightly older than the final commit — the gutter marker is
//! a HINT, not a per-line verdict.

use std::collections::HashMap;

use crate::model::{DiffContent, DiffLineKind};

/// Compute one mark per diff line: `true` when the line's NEW line number
/// falls inside an AI range for the file it belongs to.
///
/// - File context comes from `FileHeader` lines (their content is the path,
///   matching the workspace-relative paths in trace records).
/// - Only `Context`/`Added` lines can match — `Deleted` lines have no new
///   line number (they don't exist at the recorded revision).
pub fn compute_ai_line_marks(
    content: &DiffContent,
    ranges_by_file: &HashMap<String, Vec<(usize, usize)>>,
) -> Vec<bool> {
    let mut current_ranges: Option<&Vec<(usize, usize)>> = None;

    content
        .lines
        .iter()
        .map(|line| match line.kind {
            DiffLineKind::FileHeader => {
                current_ranges = ranges_by_file.get(&line.content);
                false
            }
            DiffLineKind::Context | DiffLineKind::Added => {
                let Some(ranges) = current_ranges else {
                    return false;
                };
                let Some((_, Some(new_line))) = line.line_numbers else {
                    return false;
                };
                ranges
                    .iter()
                    .any(|&(start, end)| start <= new_line && new_line <= end)
            }
            DiffLineKind::Deleted | DiffLineKind::Separator | DiffLineKind::ChangeHeader => false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DiffLine, FileOperation};

    fn content_with(lines: Vec<DiffLine>) -> DiffContent {
        DiffContent {
            lines,
            ..Default::default()
        }
    }

    fn ctx(new_line: usize, text: &str) -> DiffLine {
        DiffLine {
            kind: DiffLineKind::Context,
            line_numbers: Some((Some(new_line), Some(new_line))),
            content: text.to_string(),
            file_op: None,
        }
    }

    fn added(new_line: usize, text: &str) -> DiffLine {
        DiffLine {
            kind: DiffLineKind::Added,
            line_numbers: Some((None, Some(new_line))),
            content: text.to_string(),
            file_op: None,
        }
    }

    fn deleted(old_line: usize, text: &str) -> DiffLine {
        DiffLine {
            kind: DiffLineKind::Deleted,
            line_numbers: Some((Some(old_line), None)),
            content: text.to_string(),
            file_op: None,
        }
    }

    #[test]
    fn marks_lines_inside_range_for_matching_file() {
        let content = content_with(vec![
            DiffLine::file_header_with_op("src/main.rs", FileOperation::Modified),
            ctx(1, "fn main() {"),
            added(2, "    new line"),
            ctx(3, "}"),
        ]);
        let ranges = HashMap::from([("src/main.rs".to_string(), vec![(2, 3)])]);

        let marks = compute_ai_line_marks(&content, &ranges);
        assert_eq!(marks, vec![false, false, true, true]);
    }

    #[test]
    fn other_files_are_not_marked() {
        let content = content_with(vec![
            DiffLine::file_header_with_op("src/a.rs", FileOperation::Modified),
            added(1, "in a"),
            DiffLine::separator(),
            DiffLine::file_header_with_op("src/b.rs", FileOperation::Modified),
            added(1, "in b"),
        ]);
        let ranges = HashMap::from([("src/b.rs".to_string(), vec![(1, 10)])]);

        let marks = compute_ai_line_marks(&content, &ranges);
        assert_eq!(marks, vec![false, false, false, false, true]);
    }

    #[test]
    fn deleted_lines_are_never_marked() {
        let content = content_with(vec![
            DiffLine::file_header_with_op("src/main.rs", FileOperation::Modified),
            deleted(2, "old line"),
            added(2, "new line"),
        ]);
        let ranges = HashMap::from([("src/main.rs".to_string(), vec![(1, 10)])]);

        let marks = compute_ai_line_marks(&content, &ranges);
        assert_eq!(marks, vec![false, false, true]);
    }

    #[test]
    fn lines_without_numbers_are_not_marked() {
        // Stat/git-parsed lines carry no line numbers → never marked
        let content = content_with(vec![
            DiffLine::file_header_with_op("src/main.rs", FileOperation::Modified),
            DiffLine {
                kind: DiffLineKind::Context,
                line_numbers: None,
                content: "src/main.rs | 2 +-".to_string(),
                file_op: None,
            },
        ]);
        let ranges = HashMap::from([("src/main.rs".to_string(), vec![(1, 10)])]);

        let marks = compute_ai_line_marks(&content, &ranges);
        assert_eq!(marks, vec![false, false]);
    }

    #[test]
    fn empty_ranges_yield_no_marks() {
        let content = content_with(vec![
            DiffLine::file_header_with_op("src/main.rs", FileOperation::Modified),
            ctx(1, "x"),
        ]);
        let marks = compute_ai_line_marks(&content, &HashMap::new());
        assert!(marks.iter().all(|&m| !m));
    }
}
