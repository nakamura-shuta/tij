//! Diff data model
//!
//! Represents parsed output from `jj show` command.

/// Parsed diff content from `jj show`
#[derive(Debug, Default, Clone)]
pub struct DiffContent {
    /// Commit ID (full hash)
    pub commit_id: String,
    /// Author name and email
    pub author: String,
    /// Timestamp
    pub timestamp: String,
    /// Commit description
    pub description: String,
    /// All diff lines (including file headers)
    pub lines: Vec<DiffLine>,
}

impl DiffContent {
    /// Check if there are any file changes
    pub fn has_changes(&self) -> bool {
        self.lines
            .iter()
            .any(|l| l.kind == DiffLineKind::FileHeader)
    }

    /// Count the number of files changed (test-only helper)
    #[cfg(test)]
    pub fn file_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind == DiffLineKind::FileHeader)
            .count()
    }
}

/// A single line in the diff output
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// Type of this line
    pub kind: DiffLineKind,
    /// Line numbers (old, new) - None for headers/separators
    pub line_numbers: Option<(Option<usize>, Option<usize>)>,
    /// Content of the line
    pub content: String,
}

impl DiffLine {
    /// Create a file header line
    pub fn file_header(path: impl Into<String>) -> Self {
        Self {
            kind: DiffLineKind::FileHeader,
            line_numbers: None,
            content: path.into(),
        }
    }

    /// Create a separator line (empty line between files)
    pub fn separator() -> Self {
        Self {
            kind: DiffLineKind::Separator,
            line_numbers: None,
            content: String::new(),
        }
    }

    /// Create a context line (unchanged, test-only helper)
    #[cfg(test)]
    pub fn context(
        old_line: Option<usize>,
        new_line: Option<usize>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            kind: DiffLineKind::Context,
            line_numbers: Some((old_line, new_line)),
            content: content.into(),
        }
    }

    /// Create an added line (test-only helper)
    #[cfg(test)]
    pub fn added(new_line: usize, content: impl Into<String>) -> Self {
        Self {
            kind: DiffLineKind::Added,
            line_numbers: Some((None, Some(new_line))),
            content: content.into(),
        }
    }

    /// Create a deleted line (test-only helper)
    #[cfg(test)]
    pub fn deleted(old_line: usize, content: impl Into<String>) -> Self {
        Self {
            kind: DiffLineKind::Deleted,
            line_numbers: Some((Some(old_line), None)),
            content: content.into(),
        }
    }
}

/// Type of diff line
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    /// File header (e.g., "src/main.rs")
    FileHeader,
    /// Context line (unchanged)
    Context,
    /// Added line
    Added,
    /// Deleted line
    Deleted,
    /// Separator between files
    Separator,
}

/// Display format for diff view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffDisplayFormat {
    /// jj default (color-words style)
    #[default]
    ColorWords,
    /// Histogram overview (--stat)
    Stat,
    /// Git unified diff (--git)
    Git,
}

impl DiffDisplayFormat {
    /// Cycle to next format
    pub fn next(self) -> Self {
        match self {
            Self::ColorWords => Self::Stat,
            Self::Stat => Self::Git,
            Self::Git => Self::ColorWords,
        }
    }

    /// Human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            Self::ColorWords => "color-words",
            Self::Stat => "stat",
            Self::Git => "git",
        }
    }

    /// 1-indexed position in cycle (for notification)
    pub fn position(&self) -> usize {
        match self {
            Self::ColorWords => 1,
            Self::Stat => 2,
            Self::Git => 3,
        }
    }

    /// Total number of formats
    pub const COUNT: usize = 3;
}

/// Info for a revision in a compare diff
#[derive(Debug, Clone)]
pub struct CompareRevisionInfo {
    /// Change ID (short)
    pub change_id: String,
    /// Bookmarks on this revision
    pub bookmarks: Vec<String>,
    /// Author
    pub author: String,
    /// Timestamp
    pub timestamp: String,
    /// Description (first line)
    pub description: String,
}

/// Context for a compare (two-revision) diff
#[derive(Debug, Clone)]
pub struct CompareInfo {
    /// "From" revision info
    pub from: CompareRevisionInfo,
    /// "To" revision info
    pub to: CompareRevisionInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_content_default() {
        let content = DiffContent::default();
        assert!(content.commit_id.is_empty());
        assert!(content.lines.is_empty());
        assert!(!content.has_changes());
        assert_eq!(content.file_count(), 0);
    }

    #[test]
    fn test_diff_content_has_changes() {
        let mut content = DiffContent::default();
        assert!(!content.has_changes());

        content.lines.push(DiffLine::file_header("src/main.rs"));
        assert!(content.has_changes());
    }

    #[test]
    fn test_diff_content_file_count() {
        let mut content = DiffContent::default();
        assert_eq!(content.file_count(), 0);

        content.lines.push(DiffLine::file_header("src/main.rs"));
        content.lines.push(DiffLine::added(1, "fn main() {}"));
        content.lines.push(DiffLine::separator());
        content.lines.push(DiffLine::file_header("src/lib.rs"));
        content.lines.push(DiffLine::added(1, "pub fn hello() {}"));

        assert_eq!(content.file_count(), 2);
    }

    #[test]
    fn test_diff_line_file_header() {
        let line = DiffLine::file_header("src/main.rs");
        assert_eq!(line.kind, DiffLineKind::FileHeader);
        assert!(line.line_numbers.is_none());
        assert_eq!(line.content, "src/main.rs");
    }

    #[test]
    fn test_diff_line_separator() {
        let line = DiffLine::separator();
        assert_eq!(line.kind, DiffLineKind::Separator);
        assert!(line.line_numbers.is_none());
        assert!(line.content.is_empty());
    }

    #[test]
    fn test_diff_line_context() {
        let line = DiffLine::context(Some(10), Some(10), "    fn main() {");
        assert_eq!(line.kind, DiffLineKind::Context);
        assert_eq!(line.line_numbers, Some((Some(10), Some(10))));
        assert_eq!(line.content, "    fn main() {");
    }

    #[test]
    fn test_diff_line_added() {
        let line = DiffLine::added(11, "        println!(\"new\");");
        assert_eq!(line.kind, DiffLineKind::Added);
        assert_eq!(line.line_numbers, Some((None, Some(11))));
        assert_eq!(line.content, "        println!(\"new\");");
    }

    #[test]
    fn test_diff_line_deleted() {
        let line = DiffLine::deleted(11, "        println!(\"old\");");
        assert_eq!(line.kind, DiffLineKind::Deleted);
        assert_eq!(line.line_numbers, Some((Some(11), None)));
        assert_eq!(line.content, "        println!(\"old\");");
    }

    #[test]
    fn test_diff_line_kind_equality() {
        assert_eq!(DiffLineKind::FileHeader, DiffLineKind::FileHeader);
        assert_ne!(DiffLineKind::FileHeader, DiffLineKind::Added);
        assert_ne!(DiffLineKind::Added, DiffLineKind::Deleted);
    }

    // =========================================================================
    // DiffDisplayFormat tests
    // =========================================================================

    #[test]
    fn test_display_format_default() {
        let fmt = DiffDisplayFormat::default();
        assert_eq!(fmt, DiffDisplayFormat::ColorWords);
    }

    #[test]
    fn test_display_format_cycle() {
        let fmt = DiffDisplayFormat::ColorWords;
        assert_eq!(fmt.next(), DiffDisplayFormat::Stat);
        assert_eq!(fmt.next().next(), DiffDisplayFormat::Git);
        assert_eq!(fmt.next().next().next(), DiffDisplayFormat::ColorWords);
    }

    #[test]
    fn test_display_format_labels() {
        assert_eq!(DiffDisplayFormat::ColorWords.label(), "color-words");
        assert_eq!(DiffDisplayFormat::Stat.label(), "stat");
        assert_eq!(DiffDisplayFormat::Git.label(), "git");
    }

    #[test]
    fn test_display_format_positions() {
        assert_eq!(DiffDisplayFormat::ColorWords.position(), 1);
        assert_eq!(DiffDisplayFormat::Stat.position(), 2);
        assert_eq!(DiffDisplayFormat::Git.position(), 3);
        assert_eq!(DiffDisplayFormat::COUNT, 3);
    }
}
