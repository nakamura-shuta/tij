//! Snapshot tests for the Trace Detail View (header + body frame, record rows).

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::trace::{
    ContributorKind, TraceContributor, TraceConversation, TraceFile, TraceRange, TraceRecord,
    TraceRelated,
};
use tij::ui::views::TraceDetailView;

fn sample_record() -> TraceRecord {
    TraceRecord {
        timestamp: "2026-06-05T14:20:00Z".to_string(),
        vcs: None,
        tool_name: Some("claude-code".to_string()),
        tool_version: Some("2.0".to_string()),
        files: vec![TraceFile {
            path: "src/greet.py".to_string(),
            conversations: vec![TraceConversation {
                url: Some("claude-code://session/demo".to_string()),
                contributor: Some(TraceContributor {
                    kind: ContributorKind::Ai,
                    model_id: Some("anthropic/claude-opus-4-8".to_string()),
                }),
                ranges: vec![TraceRange {
                    start_line: 1,
                    end_line: 6,
                    contributor: None,
                }],
                related: vec![TraceRelated {
                    rel_type: "pull-request".to_string(),
                    url: "https://example.com/pr/1".to_string(),
                }],
            }],
        }],
    }
}

#[test]
fn test_trace_detail_view_basic() {
    let view = TraceDetailView::new("xqnktzml".to_string(), vec![sample_record()]);

    let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
    terminal
        .draw(|frame| view.render(frame, frame.area(), None))
        .unwrap();

    assert_snapshot!(terminal.backend());
}
