//! Snapshot tests for the Command History View (kind tags, filter title,
//! ×N collapse, -T elision).
//!
//! Wall-clock timestamps are redacted via insta filters (HH:MM:SS → [TIME])
//! so snapshots are stable across machines and timezones.

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::model::{CommandHistory, CommandKind, CommandRecord, CommandStatus};
use tij::ui::views::CommandHistoryView;

fn record(operation: &str, kind: CommandKind, args: &[&str], repeat: u32) -> CommandRecord {
    CommandRecord {
        operation: operation.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        kind,
        repeat,
        timestamp: std::time::SystemTime::UNIX_EPOCH,
        duration_ms: 12,
        status: CommandStatus::Success,
        error: None,
    }
}

fn mixed_history() -> CommandHistory {
    let mut h = CommandHistory::new();
    h.push(record(
        "log (read)",
        CommandKind::Read,
        &[
            "--color=never",
            "--no-integrate-operation",
            "log",
            "-T",
            "separate(\"\\t\", commit_id.short(), change_id.short())",
            "--limit",
            "200",
        ],
        3,
    ));
    h.push(record(
        "Describe",
        CommandKind::Write,
        &["--color=never", "describe", "-r", "abc12345", "-m", "msg"],
        1,
    ));
    h.push(record(
        "Split",
        CommandKind::Interactive,
        &["split", "-r", "abc12345"],
        1,
    ));
    h
}

fn render_to_snapshot(view: &mut CommandHistoryView, history: &CommandHistory) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 10)).unwrap();
    terminal
        .draw(|frame| view.render(frame, frame.area(), history, None))
        .unwrap();
    format!("{:?}", terminal.backend())
}

/// Redact wall-clock times (HH:MM:SS varies by machine/timezone).
fn redact_times(s: &str) -> String {
    regex::Regex::new(r"\d{2}:\d{2}:\d{2}")
        .unwrap()
        .replace_all(s, "[TIME]  ")
        .into_owned()
}

#[test]
fn test_command_history_view_mixed_kinds() {
    let history = mixed_history();
    let mut view = CommandHistoryView::new();
    let out = redact_times(&render_to_snapshot(&mut view, &history));
    assert_snapshot!(out);
}
