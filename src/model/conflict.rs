//! Conflict resolution data model

/// Information about a conflicted file from `jj resolve --list`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictFile {
    /// File path (relative)
    pub path: String,

    /// Conflict description (e.g., "2-sided conflict")
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_file_creation() {
        let file = ConflictFile {
            path: "src/main.rs".to_string(),
            description: "2-sided conflict".to_string(),
        };
        assert_eq!(file.path, "src/main.rs");
        assert_eq!(file.description, "2-sided conflict");
    }
}
