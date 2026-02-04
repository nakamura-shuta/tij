//! Color theme definitions
//!
//! Centralized color constants for consistent UI appearance.

use ratatui::style::Color;

/// Common selection colors (used across all views)
pub mod selection {
    use super::*;

    /// Selected row background - dark blue for good contrast on both light/dark terminals
    /// Note: Indexed(24) requires 256-color terminal. Falls back to different color
    /// on 16-color terminals (rare in modern use).
    pub const BG: Color = Color::Indexed(24); // xterm-256: dark blue (#005f87)
    /// Selected row foreground - bright white for visibility
    pub const FG: Color = Color::White;
}

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
    /// Graph line color (DAG structure)
    pub const GRAPH_LINE: Color = Color::Blue;
}

/// Colors for Diff View
pub mod diff_view {
    use super::*;

    /// Added line color
    pub const ADDED: Color = Color::Green;
    /// Deleted line color
    pub const DELETED: Color = Color::Red;
    /// File header color (bold applied in rendering)
    pub const FILE_HEADER: Color = Color::Cyan;
    /// Line number color
    pub const LINE_NUMBER: Color = Color::DarkGray;
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
    /// Header text color (change ID, etc.)
    pub const HEADER: Color = Color::Cyan;
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
        let _ = diff_view::DELETED;
        let _ = diff_view::FILE_HEADER;
        let _ = diff_view::LINE_NUMBER;
    }

    #[test]
    fn test_status_view_colors_defined() {
        let _ = status_view::ADDED;
        let _ = status_view::CONFLICTED;
    }
}
