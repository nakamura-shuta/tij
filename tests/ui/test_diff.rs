//! Snapshot tests for Diff View display format switching
//!
//! Uses insta + ratatui TestBackend for visual regression testing.

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::model::{DiffContent, DiffDisplayFormat, DiffLine, DiffLineKind};
use tij::ui::views::DiffView;

/// Create a color-words style test content (with line numbers)
fn create_colorwords_content() -> DiffContent {
    let mut content = DiffContent {
        commit_id: "abc123def456".to_string(),
        author: "Test User <test@example.com>".to_string(),
        timestamp: "2024-01-30 12:00:00".to_string(),
        description: "Add new feature".to_string(),
        lines: Vec::new(),
    };

    content.lines.push(DiffLine::file_header("src/main.rs"));
    content.lines.push(DiffLine {
        kind: DiffLineKind::Context,
        line_numbers: Some((Some(10), Some(10))),
        content: "fn main() {".to_string(),
    });
    content.lines.push(DiffLine {
        kind: DiffLineKind::Added,
        line_numbers: Some((None, Some(11))),
        content: "    println!(\"hello\");".to_string(),
    });
    content.lines.push(DiffLine {
        kind: DiffLineKind::Deleted,
        line_numbers: Some((Some(11), None)),
        content: "    println!(\"old\");".to_string(),
    });
    content.lines.push(DiffLine {
        kind: DiffLineKind::Context,
        line_numbers: Some((Some(12), Some(12))),
        content: "}".to_string(),
    });

    content
}

/// Create a stat-style content (no line numbers)
fn create_stat_content() -> DiffContent {
    DiffContent {
        commit_id: "abc123def456".to_string(),
        author: "Test User <test@example.com>".to_string(),
        timestamp: "2024-01-30 12:00:00".to_string(),
        description: "Add new feature".to_string(),
        lines: vec![
            DiffLine {
                kind: DiffLineKind::Context,
                line_numbers: None,
                content: "src/main.rs | 10 ++++------".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Context,
                line_numbers: None,
                content: "src/lib.rs  |  5 +++++".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Context,
                line_numbers: None,
                content: "2 files changed, 9 insertions(+), 6 deletions(-)".to_string(),
            },
        ],
    }
}

/// Create a git-style content (no line numbers, +/- prefix)
fn create_git_content() -> DiffContent {
    DiffContent {
        commit_id: "abc123def456".to_string(),
        author: "Test User <test@example.com>".to_string(),
        timestamp: "2024-01-30 12:00:00".to_string(),
        description: "Add new feature".to_string(),
        lines: vec![
            DiffLine::file_header("src/main.rs"),
            DiffLine {
                kind: DiffLineKind::Context,
                line_numbers: None,
                content: "@@ -10,3 +10,3 @@".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Context,
                line_numbers: None,
                content: "fn main() {".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: None,
                content: "    println!(\"hello\");".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: None,
                content: "    println!(\"old\");".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Context,
                line_numbers: None,
                content: "}".to_string(),
            },
        ],
    }
}

#[test]
fn test_diff_view_colorwords_format() {
    let view = DiffView::new("testchange".to_string(), create_colorwords_content());

    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_diff_view_stat_format() {
    let mut view = DiffView::new("testchange".to_string(), create_stat_content());
    view.display_format = DiffDisplayFormat::Stat;

    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_diff_view_git_format() {
    let mut view = DiffView::new("testchange".to_string(), create_git_content());
    view.display_format = DiffDisplayFormat::Git;

    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}
