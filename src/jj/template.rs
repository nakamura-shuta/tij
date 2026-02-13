//! jj template definitions for stable output parsing
//!
//! These templates ensure consistent, parseable output from jj commands
//! regardless of user configuration.

/// Separator used between fields in template output (tab character)
pub const FIELD_SEPARATOR: char = '\t';

/// Templates for jj commands
pub struct Templates;

impl Templates {
    /// Template for `jj log` output
    ///
    /// Fields (separated by tab):
    /// 1. change_id (short)
    /// 2. commit_id (short)
    /// 3. author email
    /// 4. timestamp (ISO 8601)
    /// 5. description (first line)
    /// 6. is_working_copy ("true" or "false")
    /// 7. is_empty ("true" or "false")
    /// 8. bookmarks (comma-separated)
    /// 9. has_conflict ("true" or "false") - requires jj 0.12.0+
    ///
    /// Notes:
    /// - jj doesn't interpret `\x1f` escape sequences in templates,
    ///   so we use tab characters with explicit concatenation instead of `separate()`.
    /// - `current_working_copy` is available in jj 0.20.0+.
    ///   (Earlier versions used `self.working_copy()` which no longer exists)
    /// - `conflict` keyword is available in jj 0.12.0+.
    pub fn log() -> &'static str {
        concat!(
            "change_id.short(8)",
            " ++ \"\\t\" ++ ",
            "commit_id.short(8)",
            " ++ \"\\t\" ++ ",
            "author.email()",
            " ++ \"\\t\" ++ ",
            "author.timestamp().format('%Y-%m-%dT%H:%M:%S%z')",
            " ++ \"\\t\" ++ ",
            "description.first_line()",
            " ++ \"\\t\" ++ ",
            "if(current_working_copy, 'true', 'false')",
            " ++ \"\\t\" ++ ",
            "if(empty, 'true', 'false')",
            " ++ \"\\t\" ++ ",
            "bookmarks.map(|b| b.name()).join(',')",
            " ++ \"\\t\" ++ ",
            "if(conflict, 'true', 'false')",
            " ++ \"\\n\""
        )
    }

    /// Template for `jj op log` output
    ///
    /// Fields (separated by tab):
    /// 1. operation_id (short, 12 chars)
    /// 2. user
    /// 3. timestamp
    /// 4. description
    pub fn op_log() -> &'static str {
        concat!(
            "self.id().short(12)",
            " ++ \"\\t\" ++ ",
            "self.user()",
            " ++ \"\\t\" ++ ",
            "self.time().start().ago()",
            " ++ \"\\t\" ++ ",
            "self.description().first_line()",
            " ++ \"\\n\""
        )
    }

    /// Template for getting change metadata (for compare info)
    ///
    /// Fields (separated by tab):
    /// 1. change_id (short, 8 chars)
    /// 2. bookmarks (comma-separated)
    /// 3. author email
    /// 4. timestamp
    /// 5. description (first line)
    pub fn change_info() -> &'static str {
        concat!(
            "change_id.short(8)",
            " ++ \"\\t\" ++ ",
            "bookmarks.map(|b| b.name()).join(',')",
            " ++ \"\\t\" ++ ",
            "author.email()",
            " ++ \"\\t\" ++ ",
            "author.timestamp().format('%Y-%m-%dT%H:%M:%S%z')",
            " ++ \"\\t\" ++ ",
            "description.first_line()",
            " ++ \"\\n\""
        )
    }

    /// Template for `jj file annotate` output
    ///
    /// Uses `commit.change_id().short(8)` to ensure change_id length matches
    /// the log template (8 chars), enabling reliable cross-view ID matching.
    ///
    /// Output format (same structure as default but with fixed-length change_id):
    /// `<change_id> <author> <timestamp>  <line_number>: <content>`
    ///
    /// Note: Uses AnnotationLine template methods available in jj 0.38+.
    pub fn file_annotate() -> &'static str {
        concat!(
            "commit.change_id().short(8)",
            " ++ \" \" ++ ",
            "commit.author().name()",
            " ++ \" \" ++ ",
            "commit.committer().timestamp().format(\"%Y-%m-%d %H:%M:%S\")",
            " ++ \"    \" ++ ",
            "self.line_number()",
            " ++ \": \" ++ ",
            "self.content()",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_template_is_valid() {
        let template = Templates::log();
        assert!(template.contains("change_id"));
        assert!(template.contains("commit_id"));
        assert!(template.contains("\\t")); // tab separator
        assert!(template.contains("\\n")); // newline at end
    }

    #[test]
    fn test_field_separator_is_tab() {
        assert_eq!(FIELD_SEPARATOR, '\t');
    }

    #[test]
    fn test_file_annotate_template_uses_short_8() {
        let template = Templates::file_annotate();
        assert!(template.contains("change_id().short(8)"));
    }

    #[test]
    fn test_file_annotate_template_has_required_fields() {
        let template = Templates::file_annotate();
        assert!(template.contains("change_id"));
        assert!(template.contains("author"));
        assert!(template.contains("line_number"));
        assert!(template.contains("content"));
    }
}
