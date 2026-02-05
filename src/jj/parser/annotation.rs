//! File annotation parser (jj file annotate)

use super::super::JjError;
use super::{ANNOTATE_LINE_REGEX, Parser};
use crate::model::{AnnotationContent, AnnotationLine};

impl Parser {
    /// Parse `jj file annotate` default output into AnnotationContent
    ///
    /// Default output format (jj 0.37.x compatible):
    /// `<change_id> <author> <timestamp>  <line_number>: <content>`
    ///
    /// Example: `twzksoxt nakamura 2026-01-30 10:43:19    1: //! Tij`
    ///
    /// Note: first_in_hunk is calculated by comparing consecutive change_ids.
    pub fn parse_file_annotate(
        output: &str,
        file_path: &str,
    ) -> Result<AnnotationContent, JjError> {
        let mut content = AnnotationContent::new(file_path.to_string());
        let mut prev_change_id: Option<String> = None;

        for line in output.lines() {
            if line.is_empty() {
                continue;
            }

            // Parse the default annotate output format
            if let Some(annotation) = Self::parse_annotate_line(line, &prev_change_id) {
                prev_change_id = Some(annotation.change_id.clone());
                content.lines.push(annotation);
            }
        }

        Ok(content)
    }

    /// Parse a single line of `jj file annotate` default output using regex
    ///
    /// Format: `<change_id> <author> <timestamp>  <line_number>: <content>`
    /// Example: `twzksoxt nakamura 2026-01-30 10:43:19    1: //! Tij`
    pub(super) fn parse_annotate_line(
        line: &str,
        prev_change_id: &Option<String>,
    ) -> Option<AnnotationLine> {
        let caps = ANNOTATE_LINE_REGEX.captures(line)?;

        let change_id = caps.get(1)?.as_str().to_string();
        let author = caps.get(2)?.as_str().trim().to_string();
        let timestamp = caps.get(3)?.as_str().to_string();
        let line_number: usize = caps.get(4)?.as_str().parse().ok()?;
        let content = caps
            .get(5)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        // Determine if this is the first line in hunk (different change_id from previous)
        let first_in_hunk = match prev_change_id {
            Some(prev) => prev != &change_id,
            None => true,
        };

        Some(AnnotationLine {
            change_id,
            author,
            timestamp,
            line_number,
            content,
            first_in_hunk,
        })
    }
}
