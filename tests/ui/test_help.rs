//! Snapshot tests for Help panel
//!
//! Uses insta + ratatui TestBackend for visual regression testing.
//! Reference: https://ratatui.rs/recipes/testing/snapshots/

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::ui::widgets::render_help_panel;

#[test]
fn test_help_panel_full() {
    let mut terminal = Terminal::new(TestBackend::new(80, 90)).unwrap();
    terminal
        .draw(|frame| {
            render_help_panel(frame, frame.area(), 0, None, None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_help_panel_narrow() {
    // Test how help panel looks in a narrow terminal
    let mut terminal = Terminal::new(TestBackend::new(50, 30)).unwrap();
    terminal
        .draw(|frame| {
            render_help_panel(frame, frame.area(), 0, None, None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}
