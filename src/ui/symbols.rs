//! UI symbols (markers, borders, etc.)
//!
//! ## Character Set Policy
//! - **Unicode adopted**: For consistency with jj default output
//! - Existing UI (app.rs) already uses Unicode characters
//! - ASCII fallback (theme feature) to be considered in future
//!
//! ASCII alternatives (for reference):
//! - WORKING_COPY: '@' (same)
//! - NORMAL: 'o' or '*'
//! - ROOT: '+' or '#'
//! - CONNECTOR: '|'

/// Change markers in Log View
pub mod markers {
    /// Working copy marker (@)
    pub const WORKING_COPY: char = '@';
    /// Normal change marker (○)
    pub const NORMAL: char = '○';
    /// Root change marker (◆)
    pub const ROOT: char = '◆';
    /// Vertical connector (│)
    #[allow(dead_code)]
    pub const CONNECTOR: char = '│';
}

/// Empty state indicators
pub mod empty {
    /// Label for empty changes
    pub const CHANGE_LABEL: &str = "(empty)";
    /// Label for changes with no description
    pub const NO_DESCRIPTION: &str = "(no description set)";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markers_are_single_char() {
        assert_eq!(markers::WORKING_COPY.len_utf8(), 1);
        assert!(markers::NORMAL.len_utf8() <= 3); // Unicode char
        assert!(markers::ROOT.len_utf8() <= 3);
        assert!(markers::CONNECTOR.len_utf8() <= 3);
    }

    #[test]
    fn test_empty_labels_not_empty() {
        assert!(!empty::CHANGE_LABEL.is_empty());
        assert!(!empty::NO_DESCRIPTION.is_empty());
    }
}
