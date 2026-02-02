//! Block components for UI rendering
//!
//! Common block patterns used across views.

use ratatui::{
    text::Line,
    widgets::{Block, Borders},
};

/// Create a block with title and specified borders
pub fn titled_block<'a>(title: Line<'a>, borders: Borders) -> Block<'a> {
    Block::default().borders(borders).title(title)
}

/// Create a block with all borders and a title
pub fn bordered_block<'a>(title: Line<'a>) -> Block<'a> {
    titled_block(title, Borders::ALL)
}

/// Create a block with only left and right borders (for continuation sections)
pub fn side_borders_block() -> Block<'static> {
    Block::default().borders(Borders::LEFT | Borders::RIGHT)
}

/// Create a block with top, left, and right borders (for header sections)
pub fn header_block<'a>(title: Line<'a>) -> Block<'a> {
    titled_block(title, Borders::TOP | Borders::LEFT | Borders::RIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Line;

    #[test]
    fn test_bordered_block() {
        let title = Line::from("Test");
        let _block = bordered_block(title);
        // Block is created without panic
    }

    #[test]
    fn test_side_borders_block() {
        let _block = side_borders_block();
        // Block is created without panic
    }

    #[test]
    fn test_header_block() {
        let title = Line::from("Header");
        let _block = header_block(title);
        // Block is created without panic
    }
}
