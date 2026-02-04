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
    ///
    /// Notes:
    /// - jj doesn't interpret `\x1f` escape sequences in templates,
    ///   so we use tab characters with explicit concatenation instead of `separate()`.
    /// - `current_working_copy` is available in jj 0.20.0+.
    ///   (Earlier versions used `self.working_copy()` which no longer exists)
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
}
