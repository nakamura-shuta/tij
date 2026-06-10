//! Snapshot tests for the Blame View (frame, hunk layout, AI badge column).
//!
//! Border/layout regressions in Blame were previously invisible to CI (only
//! Log/Diff/Status had snapshots) — these lock the frame shape down.

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::model::{AnnotationContent, AnnotationLine};
use tij::trace::AiBadgeSets;
use tij::ui::views::BlameView;

fn line(
    change: &str,
    commit: &str,
    n: usize,
    content: &str,
    first_in_hunk: bool,
) -> AnnotationLine {
    AnnotationLine {
        change_id: change.into(),
        commit_id: commit.into(),
        author: "test@example.com".to_string(),
        timestamp: "2026-06-01 10:00".to_string(),
        line_number: n,
        content: content.to_string(),
        first_in_hunk,
    }
}

fn sample_content() -> AnnotationContent {
    AnnotationContent {
        file_path: "src/calc.py".to_string(),
        lines: vec![
            line("aaaaaaaa", "11111111", 1, "def add(a, b):", true),
            line("aaaaaaaa", "11111111", 2, "    return a + b", false),
            line("bbbbbbbb", "22222222", 3, "def sub(a, b):", true),
            line("bbbbbbbb", "22222222", 4, "    return a - b", false),
        ],
    }
}

#[test]
fn test_blame_view_basic() {
    let mut view = BlameView::new();
    view.set_content(sample_content(), Some("aaaaaaaa".to_string()));

    let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
    terminal
        .draw(|frame| view.render(frame, frame.area(), None))
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_blame_view_with_ai_badges() {
    let mut view = BlameView::new();
    view.set_content(sample_content(), Some("aaaaaaaa".to_string()));

    // First hunk's change is AI-confirmed → [AI] on its head line only.
    let mut badges = AiBadgeSets::default();
    badges.confirmed.insert("11111111".to_string());
    view.set_ai_badges(badges);

    let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
    terminal
        .draw(|frame| view.render(frame, frame.area(), None))
        .unwrap();

    assert_snapshot!(terminal.backend());
}
