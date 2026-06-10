//! Snapshot tests for the Evolog View (frame, entry rows, [empty] marker).

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::model::EvologEntry;
use tij::ui::views::EvologView;

fn sample_entries() -> Vec<EvologEntry> {
    vec![
        EvologEntry {
            commit_id: "33333333".into(),
            change_id: "xqnktzml".into(),
            author: "test@example.com".to_string(),
            timestamp: "2026-06-03 12:00:00".to_string(),
            is_empty: false,
            description: "feat: final wording".to_string(),
        },
        EvologEntry {
            commit_id: "22222222".into(),
            change_id: "xqnktzml".into(),
            author: "test@example.com".to_string(),
            timestamp: "2026-06-02 12:00:00".to_string(),
            is_empty: false,
            description: "feat: first draft".to_string(),
        },
        EvologEntry {
            commit_id: "11111111".into(),
            change_id: "xqnktzml".into(),
            author: "test@example.com".to_string(),
            timestamp: "2026-06-01 12:00:00".to_string(),
            is_empty: true,
            description: "(no description set)".to_string(),
        },
    ]
}

#[test]
fn test_evolog_view_basic() {
    let view = EvologView::new("xqnktzml".to_string(), sample_entries());

    let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
    terminal
        .draw(|frame| view.render(frame, frame.area(), None))
        .unwrap();

    assert_snapshot!(terminal.backend());
}
