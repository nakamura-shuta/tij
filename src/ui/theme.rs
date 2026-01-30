//! Color theme definitions
//!
//! Centralized color constants for consistent UI appearance.

use ratatui::style::Color;

/// Colors for Log View
pub mod log_view {
    use super::*;

    /// Working copy marker color
    pub const WORKING_COPY_MARKER: Color = Color::Green;
    /// Normal change marker color
    pub const NORMAL_MARKER: Color = Color::Blue;
    /// Root change marker color
    pub const ROOT_MARKER: Color = Color::Magenta;
    /// Change ID color
    pub const CHANGE_ID: Color = Color::Yellow;
    /// Bookmark color
    pub const BOOKMARK: Color = Color::Cyan;
    /// Timestamp color
    pub const TIMESTAMP: Color = Color::DarkGray;
    /// Empty label color
    pub const EMPTY_LABEL: Color = Color::DarkGray;
    /// Selected row background
    pub const SELECTED_BG: Color = Color::DarkGray;
}

/// Colors for Diff View
pub mod diff_view {
    use super::*;

    /// Added line color
    pub const ADDED: Color = Color::Green;
    /// Removed line color
    pub const REMOVED: Color = Color::Red;
    /// Context line color
    pub const CONTEXT: Color = Color::Reset;
    /// Hunk header color
    pub const HUNK_HEADER: Color = Color::Cyan;
}

/// Colors for Status View
pub mod status_view {
    use super::*;

    /// Added file color
    pub const ADDED: Color = Color::Green;
    /// Modified file color
    pub const MODIFIED: Color = Color::Yellow;
    /// Deleted file color
    pub const DELETED: Color = Color::Red;
    /// Renamed file color
    pub const RENAMED: Color = Color::Cyan;
    /// Conflicted file color
    pub const CONFLICTED: Color = Color::Magenta;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_view_colors_defined() {
        // Ensure all colors are valid Color variants
        let _ = log_view::WORKING_COPY_MARKER;
        let _ = log_view::CHANGE_ID;
        let _ = log_view::BOOKMARK;
    }

    #[test]
    fn test_diff_view_colors_defined() {
        let _ = diff_view::ADDED;
        let _ = diff_view::REMOVED;
    }

    #[test]
    fn test_status_view_colors_defined() {
        let _ = status_view::ADDED;
        let _ = status_view::CONFLICTED;
    }
}
