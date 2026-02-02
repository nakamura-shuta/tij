//! Empty state components
//!
//! Display messages when there's no content to show.

use ratatui::{style::Stylize, text::Line, widgets::Paragraph};

/// Create a centered empty state display
///
/// # Arguments
/// * `title` - Main message to display
/// * `hint` - Optional hint text (displayed in gray)
pub fn empty_state(title: &str, hint: Option<&str>) -> Paragraph<'static> {
    let mut lines = vec![Line::from(""), Line::from(title.to_string()).centered()];

    if let Some(hint_text) = hint {
        lines.push(Line::from(""));
        lines.push(Line::from(hint_text.to_string()).dark_gray().centered());
    }

    lines.push(Line::from(""));

    Paragraph::new(lines)
}

/// Create an empty state for "no changes" scenario
pub fn no_changes_state() -> Paragraph<'static> {
    empty_state("No changes in this revision.", None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_state_with_hint() {
        let para = empty_state("No items", Some("Try adding some"));
        // Paragraph is created without panic
        let _ = para;
    }

    #[test]
    fn test_empty_state_without_hint() {
        let para = empty_state("Nothing here", None);
        let _ = para;
    }

    #[test]
    fn test_no_changes_state() {
        let para = no_changes_state();
        let _ = para;
    }
}
