//! Annotation (blame) data model

/// Blame information for a single line
#[derive(Debug, Clone)]
pub struct AnnotationLine {
    /// Change ID (short form, 8 chars)
    pub change_id: String,
    /// Author name
    pub author: String,
    /// Timestamp (YYYY-MM-DD HH:MM format)
    pub timestamp: String,
    /// 1-based line number
    pub line_number: usize,
    /// Line content (may contain tabs)
    pub content: String,
    /// true = first line of hunk (show full info), false = continuation (show "↑")
    pub first_in_hunk: bool,
}

/// Blame information for an entire file
#[derive(Debug, Clone, Default)]
pub struct AnnotationContent {
    /// File path being annotated
    pub file_path: String,
    /// Annotation lines
    pub lines: Vec<AnnotationLine>,
}

impl AnnotationContent {
    /// Create new empty annotation content
    pub fn new(file_path: String) -> Self {
        Self {
            file_path,
            lines: Vec::new(),
        }
    }

    /// Check if content is empty
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get line count
    pub fn len(&self) -> usize {
        self.lines.len()
    }
}

impl AnnotationLine {
    /// Get short timestamp for display (MM-DD or YY-MM-DD)
    pub fn short_timestamp(&self) -> String {
        // Input format: "YYYY-MM-DD HH:MM"
        if self.timestamp.len() >= 10 {
            let date_part = &self.timestamp[..10]; // "YYYY-MM-DD"
            // For simplicity, always show MM-DD (could compare with current year)
            if date_part.len() >= 10 {
                return date_part[5..10].to_string(); // "MM-DD"
            }
        }
        self.timestamp.clone()
    }

    /// Get truncated author name for display
    pub fn short_author(&self, max_len: usize) -> String {
        if self.author.chars().count() <= max_len {
            self.author.clone()
        } else {
            self.author.chars().take(max_len - 1).collect::<String>() + "…"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_content_new() {
        let content = AnnotationContent::new("src/main.rs".to_string());
        assert_eq!(content.file_path, "src/main.rs");
        assert!(content.is_empty());
    }

    #[test]
    fn test_annotation_line_short_timestamp() {
        let line = AnnotationLine {
            change_id: "twzksoxt".to_string(),
            author: "nakamura".to_string(),
            timestamp: "2026-01-30 10:43".to_string(),
            line_number: 1,
            content: "test".to_string(),
            first_in_hunk: true,
        };
        assert_eq!(line.short_timestamp(), "01-30");
    }

    #[test]
    fn test_annotation_line_short_author() {
        let line = AnnotationLine {
            change_id: "twzksoxt".to_string(),
            author: "nakamura.shuta".to_string(),
            timestamp: "2026-01-30 10:43".to_string(),
            line_number: 1,
            content: "test".to_string(),
            first_in_hunk: true,
        };
        assert_eq!(line.short_author(8), "nakamur…");
        assert_eq!(line.short_author(20), "nakamura.shuta");
    }
}
