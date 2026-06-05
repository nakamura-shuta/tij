//! Agent Trace actions (Phase 2: trace detail dialog + conversation URL copy)

use crate::app::helpers::revision::short_id;
use crate::app::state::App;
use crate::trace::TraceRecord;
use crate::ui::components::{Dialog, DialogCallback, SelectItem};

impl App {
    /// Open a select dialog listing the Agent Trace records anchored to the
    /// given change (palette: `show-traces`).
    ///
    /// Enter copies the record's conversation URL to the clipboard. Records
    /// without a URL are listed too (selecting them shows an info notice) so
    /// the dialog reflects everything tij knows about the change.
    pub(crate) fn open_trace_dialog(&mut self, change_id: &str, commit_id: &str) {
        let sid = short_id(change_id);

        let Some(index) = self.trace_index.as_ref() else {
            self.notify_info("No agent trace data (no .agent-trace/traces.jsonl)");
            return;
        };
        let records = index.records_for(change_id, commit_id);
        if records.is_empty() {
            self.notify_info(format!("No agent traces on {}", sid));
            return;
        }

        let items: Vec<SelectItem> = records
            .iter()
            .map(|r| SelectItem {
                label: trace_item_label(r),
                value: r.primary_url().unwrap_or_default().to_string(),
                selected: false,
            })
            .collect();

        self.active_dialog = Some(Dialog::select_single(
            "Agent Traces",
            format!(
                "{} trace record(s) on {} — Enter: copy conversation URL",
                items.len(),
                sid
            ),
            items,
            None,
            DialogCallback::TraceCopyUrl,
        ));
    }

    /// Handle the confirmed trace dialog: copy the selected conversation URL.
    pub(crate) fn handle_trace_dialog(&mut self, values: Vec<String>) {
        let url = values.into_iter().find(|v| !v.is_empty());
        match url {
            Some(url) => match crate::app::clipboard::copy_to_clipboard(&url) {
                Ok(()) => self.notify_success(format!("Copied conversation URL: {}", url)),
                Err(e) => self.set_error(e),
            },
            None => self.notify_info("This trace record has no conversation URL"),
        }
    }
}

/// One-line label for a trace record in the select dialog:
/// `2026-06-05 14:20  claude-code (anthropic/claude-opus-4-8) — 2 file(s)`
/// with ` [no URL]` appended when there is nothing to copy.
fn trace_item_label(record: &TraceRecord) -> String {
    let time = format_trace_timestamp(&record.timestamp);
    let tool = record.tool_name.as_deref().unwrap_or("unknown");
    let model = record
        .primary_model_id()
        .map(|m| format!(" ({})", m))
        .unwrap_or_default();
    let files = record.code_file_count();
    let url_marker = if record.primary_url().is_none() {
        " [no URL]"
    } else {
        ""
    };
    format!(
        "{}  {}{} — {} file(s){}",
        time, tool, model, files, url_marker
    )
}

/// Render an RFC 3339 timestamp as `YYYY-MM-DD HH:MM` (best-effort; unknown
/// formats pass through unchanged).
fn format_trace_timestamp(ts: &str) -> String {
    if ts.len() >= 16 && ts.as_bytes().get(10) == Some(&b'T') {
        format!("{} {}", &ts[..10], &ts[11..16])
    } else {
        ts.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{
        ContributorKind, TraceContributor, TraceConversation, TraceFile, TraceIndex, TraceVcs,
        TraceVcsType,
    };

    fn record_with(url: Option<&str>, model: Option<&str>) -> TraceRecord {
        TraceRecord {
            timestamp: "2026-06-05T14:20:00Z".to_string(),
            vcs: Some(TraceVcs {
                vcs_type: TraceVcsType::Jj,
                revision: "xqnktzmlworukplnyrropmtzylsuxxlv".to_string(),
            }),
            tool_name: Some("claude-code".to_string()),
            tool_version: None,
            files: vec![TraceFile {
                path: "src/main.rs".to_string(),
                conversations: vec![TraceConversation {
                    url: url.map(str::to_string),
                    contributor: Some(TraceContributor {
                        kind: ContributorKind::Ai,
                        model_id: model.map(str::to_string),
                    }),
                    ranges: vec![],
                }],
            }],
        }
    }

    #[test]
    fn label_includes_time_tool_model_and_files() {
        let r = record_with(
            Some("https://x.test/s/1"),
            Some("anthropic/claude-opus-4-8"),
        );
        let label = trace_item_label(&r);
        assert_eq!(
            label,
            "2026-06-05 14:20  claude-code (anthropic/claude-opus-4-8) — 1 file(s)"
        );
    }

    #[test]
    fn label_marks_missing_url() {
        let r = record_with(None, None);
        let label = trace_item_label(&r);
        assert!(label.ends_with("[no URL]"), "got: {label}");
        assert!(!label.contains("()"), "empty model parens: {label}");
    }

    #[test]
    fn timestamp_formats_rfc3339_and_passes_through_unknown() {
        assert_eq!(
            format_trace_timestamp("2026-06-05T14:20:00Z"),
            "2026-06-05 14:20"
        );
        assert_eq!(format_trace_timestamp("yesterday"), "yesterday");
        assert_eq!(format_trace_timestamp(""), "");
    }

    #[test]
    fn dialog_opens_with_matching_records() {
        let mut app = App::new_for_test();
        app.trace_index = Some(TraceIndex::build(&[record_with(
            Some("https://x.test/s/1"),
            None,
        )]));

        app.open_trace_dialog("xqnktzml", "2d31c7f1");
        assert!(app.active_dialog.is_some(), "dialog should open");
    }

    #[test]
    fn no_records_shows_notification_not_dialog() {
        let mut app = App::new_for_test();
        app.trace_index = Some(TraceIndex::build(&[]));

        app.open_trace_dialog("zzzzzzzz", "00000000");
        assert!(app.active_dialog.is_none());
        assert!(app.notification.is_some());
    }

    #[test]
    fn no_index_shows_notification_not_dialog() {
        let mut app = App::new_for_test();
        app.open_trace_dialog("xqnktzml", "2d31c7f1");
        assert!(app.active_dialog.is_none());
        assert!(app.notification.is_some());
    }

    #[test]
    fn handle_trace_dialog_without_url_notifies() {
        let mut app = App::new_for_test();
        app.handle_trace_dialog(vec![String::new()]);
        assert!(app.notification.is_some());
        assert!(app.error_message.is_none());
    }
}
