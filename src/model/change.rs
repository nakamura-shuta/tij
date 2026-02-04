//! Change (commit) data model

/// Represents a jj change (similar to a Git commit)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Change {
    /// Short change ID (jj's unique identifier)
    pub change_id: String,

    /// Short commit ID (Git-compatible)
    pub commit_id: String,

    /// Author email
    pub author: String,

    /// Timestamp (ISO 8601 format)
    pub timestamp: String,

    /// First line of the description
    pub description: String,

    /// Is this the current working copy?
    pub is_working_copy: bool,

    /// Is this change empty (no file changes)?
    pub is_empty: bool,

    /// Associated bookmarks (branch names)
    pub bookmarks: Vec<String>,

    /// DAG graph prefix from jj log output
    ///
    /// Examples:
    /// - `"@  "` (working copy, simple)
    /// - `"│ ○  "` (1-level branch)
    /// - `"│ │ ○  "` (2-level branch)
    /// - `"├─╮"` (graph-only, branch start)
    pub graph_prefix: String,

    /// True if this is a graph-only line (no change data, just branch lines)
    pub is_graph_only: bool,
}

impl Change {
    /// Get a display-friendly short ID
    pub fn short_id(&self) -> &str {
        &self.change_id
    }

    /// Get a display string for the description
    pub fn display_description(&self) -> &str {
        if self.description.is_empty() {
            "(no description set)"
        } else {
            &self.description
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Change {
        /// Get the marker character for this change (test-only helper)
        pub fn marker(&self) -> char {
            if self.is_working_copy { '@' } else { '○' }
        }
    }

    fn sample_change() -> Change {
        Change {
            change_id: "abc12345".to_string(),
            commit_id: "def67890".to_string(),
            author: "user@example.com".to_string(),
            timestamp: "2024-01-29T15:30:00+0900".to_string(),
            description: "Initial commit".to_string(),
            is_working_copy: false,
            is_empty: false,
            bookmarks: vec!["main".to_string()],
            graph_prefix: String::new(),
            is_graph_only: false,
        }
    }

    #[test]
    fn test_short_id() {
        let change = sample_change();
        assert_eq!(change.short_id(), "abc12345");
    }

    #[test]
    fn test_display_description() {
        let change = sample_change();
        assert_eq!(change.display_description(), "Initial commit");

        let empty_desc = Change {
            description: String::new(),
            ..sample_change()
        };
        assert_eq!(empty_desc.display_description(), "(no description set)");
    }

    #[test]
    fn test_marker_working_copy() {
        let working_copy = Change {
            is_working_copy: true,
            ..sample_change()
        };
        assert_eq!(working_copy.marker(), '@');
    }

    #[test]
    fn test_marker_regular() {
        let regular = sample_change();
        assert_eq!(regular.marker(), '○');
    }
}
