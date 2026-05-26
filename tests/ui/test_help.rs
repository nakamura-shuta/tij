//! Snapshot tests for Help panel
//!
//! Uses insta + ratatui TestBackend for visual regression testing.
//! Reference: https://ratatui.rs/recipes/testing/snapshots/

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::app::View;
use tij::ui::widgets::render_help_panel;

#[test]
fn test_help_panel_full() {
    // All-views mode. Height sized to fit every section without scrolling so the
    // snapshot catches accidental drops of trailing sections when new keys are added.
    let mut terminal = Terminal::new(TestBackend::new(80, 200)).unwrap();
    terminal
        .draw(|frame| {
            render_help_panel(frame, frame.area(), 0, None, None, None, true);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_help_panel_narrow() {
    // All-views mode in a narrow terminal.
    let mut terminal = Terminal::new(TestBackend::new(50, 30)).unwrap();
    terminal
        .draw(|frame| {
            render_help_panel(frame, frame.area(), 0, None, None, None, true);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_help_panel_current_view_log() {
    // Current-view mode: only Log View keys + Global + Navigation, title [Log View].
    let mut terminal = Terminal::new(TestBackend::new(80, 80)).unwrap();
    terminal
        .draw(|frame| {
            render_help_panel(frame, frame.area(), 0, None, None, Some(View::Log), false);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}
