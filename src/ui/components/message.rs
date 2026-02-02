//! Error message components
//!
//! Provides consistent styling for error messages.
//! For empty states, use `empty_state` module.

use ratatui::{
    prelude::*,
    text::{Line, Span},
};

/// Build an error message line for overlay display
///
/// Returns a styled line suitable for rendering as a banner.
/// Format: `[red bg] Error: [/red bg][red text] message [/red text]`
pub fn build_error_line(error: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(" Error: ", Style::default().fg(Color::White).bg(Color::Red)),
        Span::styled(format!(" {} ", error), Style::default().fg(Color::Red)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_error_line() {
        let line = build_error_line("Connection failed");
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[0].content, " Error: ");
        assert_eq!(line.spans[1].content, " Connection failed ");
    }

    #[test]
    fn test_build_error_line_with_special_chars() {
        let line = build_error_line("Can't find file: /path/to/file");
        assert!(!line.spans.is_empty());
    }
}
