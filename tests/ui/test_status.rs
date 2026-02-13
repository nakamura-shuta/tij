//! Snapshot tests for Status View
//!
//! Uses insta + ratatui TestBackend for visual regression testing.
//! Reference: https://ratatui.rs/recipes/testing/snapshots/

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::model::{FileState, FileStatus, Status};
use tij::ui::views::{StatusInputMode, StatusView};

#[test]
fn test_status_view_loading() {
    let view = StatusView::new();

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_status_view_clean() {
    let mut view = StatusView::new();
    view.set_status(Status {
        files: vec![],
        has_conflicts: false,
        working_copy_change_id: "kxryzmql".to_string(),
        parent_change_id: "mzvwqtsr".to_string(),
    });

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_status_view_with_files() {
    let mut view = StatusView::new();
    view.set_status(Status {
        files: vec![
            FileStatus {
                path: "src/main.rs".to_string(),
                state: FileState::Modified,
            },
            FileStatus {
                path: "src/lib.rs".to_string(),
                state: FileState::Added,
            },
            FileStatus {
                path: "old_config.toml".to_string(),
                state: FileState::Deleted,
            },
            FileStatus {
                path: "src/utils.rs".to_string(),
                state: FileState::Renamed {
                    from: "src/helpers.rs".to_string(),
                },
            },
        ],
        has_conflicts: false,
        working_copy_change_id: "kxryzmql".to_string(),
        parent_change_id: "mzvwqtsr".to_string(),
    });

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_status_view_with_conflicts() {
    let mut view = StatusView::new();
    view.set_status(Status {
        files: vec![
            FileStatus {
                path: "src/main.rs".to_string(),
                state: FileState::Conflicted,
            },
            FileStatus {
                path: "src/lib.rs".to_string(),
                state: FileState::Conflicted,
            },
            FileStatus {
                path: "README.md".to_string(),
                state: FileState::Modified,
            },
        ],
        has_conflicts: true,
        working_copy_change_id: "kxryzmql".to_string(),
        parent_change_id: "mzvwqtsr".to_string(),
    });

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}

#[test]
fn test_status_view_commit_input() {
    let mut view = StatusView::new();
    view.set_status(Status {
        files: vec![FileStatus {
            path: "src/main.rs".to_string(),
            state: FileState::Modified,
        }],
        has_conflicts: false,
        working_copy_change_id: "kxryzmql".to_string(),
        parent_change_id: "mzvwqtsr".to_string(),
    });
    view.input_mode = StatusInputMode::CommitInput;
    view.input_buffer = "fix: resolve login bug".to_string();

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            view.render(frame, frame.area(), None);
        })
        .unwrap();

    assert_snapshot!(terminal.backend());
}
