//! Agent Trace actions
//!
//! - Phase 3: Diff View AI range overlay (`apply_ai_diff_overlay`)
//! - A6+A3: Trace Detail View (`open_trace_detail` / `handle_trace_detail_action`)

use crate::app::helpers::revision::short_id;
use crate::app::state::{App, View};
use crate::ui::views::{TraceDetailAction, TraceDetailView};

impl App {
    /// Open the Trace Detail View for the Agent Trace records anchored to the
    /// given change (palette: `show-traces`). Replaces the Phase 2 Select
    /// dialog — the richer multi-section display outgrew "list + single action".
    ///
    /// No trace / no matching record → info notice, View not opened (P5).
    pub(crate) fn open_trace_detail(&mut self, change_id: &str, commit_id: &str) {
        let sid = short_id(change_id);

        let Some(index) = self.trace_index.as_ref() else {
            self.notify_info("No agent trace data (no .agent-trace/traces.jsonl)");
            return;
        };
        let records: Vec<_> = index
            .records_for(change_id, commit_id)
            .into_iter()
            .cloned()
            .collect();
        if records.is_empty() {
            self.notify_info(format!("No agent traces on {}", sid));
            return;
        }

        self.trace_detail_view = Some(TraceDetailView::new(sid.to_string(), records));
        self.go_to_view(View::TraceDetail);
        self.error_message = None;
    }

    /// Show the AI contribution summary for the loaded changes (A1).
    ///
    /// Filter-independent: always summarizes the full loaded change set
    /// (`--limit 200` range), not the AI-filtered view — the AI ratio is a
    /// property of the loaded set, not of the current display filter.
    pub(crate) fn show_ai_summary(&mut self) {
        // No trace index → summarize against an empty index so `total` still
        // counts the loaded changes (AI 0/N, not AI 0/0). The denominator is
        // always the loaded change set.
        let empty;
        let index = match self.trace_index.as_ref() {
            Some(i) => i,
            None => {
                empty = crate::trace::TraceIndex::default();
                &empty
            }
        };
        let summary = index.summarize(&self.log_view.changes);
        self.notify_info(summary.one_line());
    }

    /// Write the AI attribution Markdown report to `<workspace
    /// root>/agent-trace-report.md` (palette: `ai-report`, A8).
    ///
    /// Like A1, the denominator is the full loaded change set (filter-
    /// independent) and a missing trace still produces a valid `AI 0/N`
    /// report. Report generation is the pure `trace::build_report`; this
    /// method only resolves the path, supplies the timestamp, and writes —
    /// any failure degrades to a notification, never affecting tij (P6).
    pub(crate) fn export_ai_report(&mut self) {
        let root = match self.jj.workspace_root() {
            Ok(r) => r,
            Err(e) => {
                self.set_error(format!("Cannot resolve workspace root: {}", e));
                return;
            }
        };

        let empty;
        let index = match self.trace_index.as_ref() {
            Some(i) => i,
            None => {
                empty = crate::trace::TraceIndex::default();
                &empty
            }
        };

        let summary = index.summarize(&self.log_view.changes);
        let now = report_timestamp();
        let markdown = crate::trace::build_report(&self.log_view.changes, index, &now);

        let path = std::path::Path::new(&root).join("agent-trace-report.md");
        match std::fs::write(&path, &markdown) {
            Ok(()) => self.notify_info(format!(
                "Wrote agent-trace-report.md ({} AI changes)",
                summary.ai_total
            )),
            Err(e) => self.set_error(format!("Failed to write agent-trace-report.md: {}", e)),
        }
    }

    /// One-line glance at orphaned traces over the loaded changes (palette:
    /// `ai-orphans`, A7) — the trace-first companion to `ai-summary`.
    ///
    /// Filter-independent (full loaded set, like A1/A8). "Orphaned" means a
    /// trace anchored to a revision matching none of the loaded changes; the
    /// wording avoids asserting deletion (it may be out of the `--limit`
    /// window). No trace → 0 orphans.
    pub(crate) fn show_orphans(&mut self) {
        use crate::trace::TraceVcsType;

        let empty;
        let index = match self.trace_index.as_ref() {
            Some(i) => i,
            None => {
                empty = crate::trace::TraceIndex::default();
                &empty
            }
        };

        let orphans = index.orphaned_anchors(&self.log_view.changes);
        if orphans.is_empty() {
            self.notify_info("No orphaned traces");
            return;
        }
        let jj = orphans
            .iter()
            .filter(|o| o.vcs_type == TraceVcsType::Jj)
            .count();
        let git = orphans
            .iter()
            .filter(|o| o.vcs_type == TraceVcsType::Git)
            .count();
        self.notify_info(format!(
            "Orphaned traces: {} (jj {}, git {}) — not in loaded changes",
            orphans.len(),
            jj,
            git
        ));
    }

    /// Dispatch a Trace Detail View action.
    pub(crate) fn handle_trace_detail_action(&mut self, action: TraceDetailAction) {
        match action {
            TraceDetailAction::None => {}
            TraceDetailAction::Back => {
                self.trace_detail_view = None;
                self.go_to_view(View::Log);
            }
            TraceDetailAction::CopyUrl(url) => {
                match crate::app::clipboard::copy_to_clipboard(&url) {
                    Ok(()) => self.notify_success(format!("Copied URL: {}", url)),
                    Err(e) => self.set_error(e),
                }
            }
            TraceDetailAction::NoUrl => {
                self.notify_info("No URL to copy");
            }
        }
    }

    /// Apply the Agent Trace AI overlay to the current Diff View (Phase 3).
    ///
    /// `revision` follows the OpenDiff convention (change_id for the working
    /// copy, commit_id otherwise) — but trace matching needs BOTH IDs (jj
    /// anchors match change_id, git anchors match commit_id), so the Change
    /// is looked up in the current log list. When the revision is not in the
    /// log (e.g. Blame jump), fall back to passing `revision` as both keys:
    /// change-ID (k–z) and SHA (hex) alphabets are disjoint, so the wrong-
    /// kind prefix never FALSE-matches — only same-kind anchors can apply.
    ///
    /// Applies only to Single mode + ColorWords format — the only combination
    /// whose parsed lines carry the new-side line numbers the trace ranges
    /// refer to. All other cases keep the marks cleared by `set_content`.
    pub(crate) fn apply_ai_diff_overlay(&mut self, revision: &str) {
        use crate::model::{DiffDisplayFormat, DiffMode};

        if self.trace_index.is_none() {
            return;
        }
        match self.diff_view.as_ref() {
            Some(dv)
                if dv.mode == DiffMode::Single
                    && dv.display_format == DiffDisplayFormat::ColorWords => {}
            _ => return,
        }

        // Resolve both IDs from the log row when possible (see doc comment)
        let (change_id, commit_id) = self
            .log_view
            .changes
            .iter()
            .find(|c| {
                !c.is_graph_only
                    && (c.commit_id.as_str() == revision || c.change_id.as_str() == revision)
            })
            .map(|c| (c.change_id.to_string(), c.commit_id.to_string()))
            .unwrap_or_else(|| (revision.to_string(), revision.to_string()));

        let ranges = self
            .trace_index
            .as_ref()
            .expect("checked above")
            .ai_ranges_for(&change_id, &commit_id);
        if ranges.is_empty() {
            return;
        }
        let diff_view = self.diff_view.as_mut().expect("checked above");
        let marks = crate::trace::compute_ai_line_marks(&diff_view.content, &ranges);
        diff_view.set_ai_line_marks(marks);
    }
}

/// Current UTC time as `YYYY-MM-DD HH:MM UTC` for the report header.
///
/// UTC (not local) keeps it dependency-free and unambiguous — the report is a
/// shareable artifact, so an explicit zone beats a silent local one. The
/// `build_report` function stays clock-free; the App supplies this string.
fn report_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs.div_euclid(86400);
    let day_secs = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let hh = day_secs / 3600;
    let mm = (day_secs % 3600) / 60;
    format!("{:04}-{:02}-{:02} {:02}:{:02} UTC", y, m, d, hh, mm)
}

/// Civil (Gregorian) date from a count of days since the Unix epoch
/// (1970-01-01). Howard Hinnant's `civil_from_days` — exact, no leap-year
/// special-casing bugs. Returns `(year, month [1-12], day [1-31])`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{
        ContributorKind, TraceContributor, TraceConversation, TraceFile, TraceIndex, TraceRecord,
        TraceVcs, TraceVcsType,
    };
    use crate::ui::views::TraceDetailAction;

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
                    related: vec![],
                }],
            }],
        }
    }

    #[test]
    fn open_trace_detail_opens_view_with_matching_records() {
        let mut app = App::new_for_test();
        app.trace_index = Some(TraceIndex::build(&[record_with(
            Some("https://x.test/s/1"),
            None,
        )]));

        app.open_trace_detail("xqnktzml", "2d31c7f1");
        assert_eq!(app.current_view, View::TraceDetail);
        assert!(app.trace_detail_view.is_some());
    }

    #[test]
    fn no_records_shows_notification_not_view() {
        let mut app = App::new_for_test();
        app.trace_index = Some(TraceIndex::build(&[]));

        app.open_trace_detail("zzzzzzzz", "00000000");
        assert_ne!(app.current_view, View::TraceDetail);
        assert!(app.trace_detail_view.is_none());
        assert!(app.notification.is_some());
    }

    #[test]
    fn no_index_shows_notification_not_view() {
        let mut app = App::new_for_test();
        app.open_trace_detail("xqnktzml", "2d31c7f1");
        assert_ne!(app.current_view, View::TraceDetail);
        assert!(app.notification.is_some());
    }

    #[test]
    fn trace_detail_copy_url_action_succeeds() {
        let mut app = App::new_for_test();
        app.handle_trace_detail_action(TraceDetailAction::CopyUrl("https://x/1".to_string()));
        // clipboard may be unavailable in CI; either success or a set error,
        // but never a panic and never an info-NoUrl
        assert!(app.notification.is_some() || app.error_message.is_some());
    }

    #[test]
    fn ai_summary_notifies_with_ratio() {
        use crate::model::{Change, ChangeId, CommitId};
        let mut app = App::new_for_test();
        // record_with builds a jj-anchored AI record on xqnktzml…
        app.trace_index = Some(TraceIndex::build(&[record_with(None, None)]));
        app.log_view.set_changes(vec![Change {
            change_id: ChangeId::new("xqnktzml".to_string()),
            commit_id: CommitId::new("2d31c7f1".to_string()),
            author: String::new(),
            timestamp: String::new(),
            description: String::new(),
            is_working_copy: false,
            is_empty: false,
            bookmarks: vec![],
            graph_prefix: String::new(),
            is_graph_only: false,
            has_conflict: false,
            working_copy_names: vec![],
        }]);

        app.show_ai_summary();
        let msg = app
            .notification
            .as_ref()
            .map(|n| n.message.clone())
            .unwrap_or_default();
        assert!(msg.starts_with("AI 1/1 (100%)"), "got: {msg}");
    }

    #[test]
    fn ai_summary_without_index_counts_loaded_total() {
        use crate::model::{Change, ChangeId, CommitId};
        let mut app = App::new_for_test();
        // No trace_index, but two loaded changes → AI 0/2 (denominator counts)
        let mk = |c: &str, h: &str| Change {
            change_id: ChangeId::new(c.to_string()),
            commit_id: CommitId::new(h.to_string()),
            author: String::new(),
            timestamp: String::new(),
            description: String::new(),
            is_working_copy: false,
            is_empty: false,
            bookmarks: vec![],
            graph_prefix: String::new(),
            is_graph_only: false,
            has_conflict: false,
            working_copy_names: vec![],
        };
        app.log_view.set_changes(vec![mk("a", "1"), mk("b", "2")]);
        app.show_ai_summary();
        let msg = app
            .notification
            .as_ref()
            .map(|n| n.message.clone())
            .unwrap_or_default();
        assert!(msg.starts_with("AI 0/2 (0%)"), "got: {msg}");
    }

    #[test]
    fn trace_detail_no_url_action_notifies() {
        let mut app = App::new_for_test();
        app.handle_trace_detail_action(TraceDetailAction::NoUrl);
        assert!(app.notification.is_some());
        assert!(app.error_message.is_none());
    }

    #[test]
    fn trace_detail_hints_drop_copy_when_no_url() {
        use crate::ui::views::TraceDetailView;
        let mut app = App::new_for_test();

        // With a URL → hints include [y] Copy URL
        app.trace_detail_view = Some(TraceDetailView::new(
            "x".to_string(),
            vec![record_with(Some("u1"), None)],
        ));
        assert!(app.trace_detail_hints().iter().any(|h| h.key == "y"));

        // Without a URL → [y] Copy URL is dropped (G4)
        app.trace_detail_view = Some(TraceDetailView::new(
            "x".to_string(),
            vec![record_with(None, None)],
        ));
        assert!(!app.trace_detail_hints().iter().any(|h| h.key == "y"));
    }

    // ── Phase 3: Diff View AI overlay ──

    fn record_with_range(start: usize, end: usize) -> TraceRecord {
        use crate::trace::TraceRange;
        let mut r = record_with(None, None);
        r.files[0].conversations[0].ranges = vec![TraceRange {
            start_line: start,
            end_line: end,
            contributor: None,
        }];
        r
    }

    fn diff_view_for(revision: &str) -> crate::ui::views::DiffView {
        use crate::model::{DiffContent, DiffLine, DiffLineKind, FileOperation};
        let content = DiffContent {
            lines: vec![
                DiffLine::file_header_with_op("src/main.rs", FileOperation::Modified),
                DiffLine {
                    kind: DiffLineKind::Added,
                    line_numbers: Some((None, Some(2))),
                    content: "new line".to_string(),
                    file_op: None,
                },
            ],
            ..Default::default()
        };
        crate::ui::views::DiffView::new(revision.to_string(), content)
    }

    #[test]
    fn overlay_marks_single_colorwords_diff() {
        let mut app = App::new_for_test();
        app.trace_index = Some(TraceIndex::build(&[record_with_range(1, 5)]));
        app.diff_view = Some(diff_view_for("xqnktzml"));

        app.apply_ai_diff_overlay("xqnktzml");

        let dv = app.diff_view.as_ref().unwrap();
        assert!(dv.has_ai_overlay());
        assert_eq!(dv.ai_line_marks, vec![false, true]);
    }

    #[test]
    fn overlay_skips_non_colorwords_format() {
        use crate::model::DiffDisplayFormat;
        let mut app = App::new_for_test();
        app.trace_index = Some(TraceIndex::build(&[record_with_range(1, 5)]));
        let mut dv = diff_view_for("xqnktzml");
        dv.display_format = DiffDisplayFormat::Stat;
        app.diff_view = Some(dv);

        app.apply_ai_diff_overlay("xqnktzml");
        assert!(!app.diff_view.as_ref().unwrap().has_ai_overlay());
    }

    #[test]
    fn overlay_resolves_commit_id_revision_via_log_lookup() {
        // OpenDiff passes commit_id for non-working-copy changes; jj-anchored
        // records match change_id — the Change lookup must bridge the two.
        // (Regression: found live — badge showed but overlay stayed empty.)
        use crate::model::{Change, ChangeId, CommitId};
        let mut app = App::new_for_test();
        app.trace_index = Some(TraceIndex::build(&[record_with_range(1, 5)]));
        app.log_view.set_changes(vec![Change {
            change_id: ChangeId::new("xqnktzml".to_string()),
            commit_id: CommitId::new("2d31c7f1".to_string()),
            author: String::new(),
            timestamp: String::new(),
            description: String::new(),
            is_working_copy: false,
            is_empty: false,
            bookmarks: vec![],
            graph_prefix: String::new(),
            is_graph_only: false,
            has_conflict: false,
            working_copy_names: vec![],
        }]);
        app.diff_view = Some(diff_view_for("2d31c7f1")); // commit_id, not change_id

        app.apply_ai_diff_overlay("2d31c7f1");
        assert!(
            app.diff_view.as_ref().unwrap().has_ai_overlay(),
            "jj-anchored record must apply when diff was opened by commit_id"
        );
    }

    #[test]
    fn overlay_skips_unmatched_revision() {
        let mut app = App::new_for_test();
        app.trace_index = Some(TraceIndex::build(&[record_with_range(1, 5)]));
        app.diff_view = Some(diff_view_for("zzzzzzzz"));

        app.apply_ai_diff_overlay("zzzzzzzz");
        assert!(!app.diff_view.as_ref().unwrap().has_ai_overlay());
    }

    // ── Phase 4a: Blame AI badges ──

    fn blame_view_with_lines() -> crate::ui::views::BlameView {
        use crate::model::{AnnotationContent, AnnotationLine, ChangeId, CommitId};
        let mut content = AnnotationContent::new("greet.py".to_string());
        // line 1 anchored to the AI change, line 2 to an unrelated change
        content.lines.push(AnnotationLine {
            change_id: ChangeId::new("xqnktzml".to_string()),
            commit_id: CommitId::new("2d31c7f1".to_string()),
            author: "nakamura".to_string(),
            timestamp: "2026-06-05 14:20".to_string(),
            line_number: 1,
            content: "def greet(name):".to_string(),
            first_in_hunk: true,
        });
        content.lines.push(AnnotationLine {
            change_id: ChangeId::new("lrptplro".to_string()),
            commit_id: CommitId::new("b06e4c8c".to_string()),
            author: "nakamura".to_string(),
            timestamp: "2026-06-05 14:30".to_string(),
            line_number: 9,
            content: "def shout(name):".to_string(),
            first_in_hunk: true,
        });
        let mut bv = crate::ui::views::BlameView::new();
        bv.set_content(content, None);
        bv
    }

    #[test]
    fn blame_badges_mark_only_ai_anchored_lines() {
        let mut app = App::new_for_test();
        app.trace_index = Some(TraceIndex::build(&[record_with_range(1, 6)]));
        app.blame_view = Some(blame_view_with_lines());

        app.apply_blame_ai_badges();

        let badges = &app.blame_view.as_ref().unwrap().ai_badges();
        assert!(
            badges.confirmed.contains("2d31c7f1"),
            "AI change line badged"
        );
        assert!(
            !badges.confirmed.contains("b06e4c8c"),
            "other change not badged"
        );
    }

    #[test]
    fn blame_badges_empty_without_index() {
        let mut app = App::new_for_test();
        app.blame_view = Some(blame_view_with_lines());
        app.apply_blame_ai_badges();
        assert!(app.blame_view.as_ref().unwrap().ai_badges().is_empty());
    }

    #[test]
    fn set_content_clears_overlay_marks() {
        let mut app = App::new_for_test();
        app.trace_index = Some(TraceIndex::build(&[record_with_range(1, 5)]));
        app.diff_view = Some(diff_view_for("xqnktzml"));
        app.apply_ai_diff_overlay("xqnktzml");
        assert!(app.diff_view.as_ref().unwrap().has_ai_overlay());

        // New content invalidates the marks (e.g. format cycle re-fetch)
        let dv = app.diff_view.as_mut().unwrap();
        dv.set_content("xqnktzml".to_string(), Default::default());
        assert!(!dv.has_ai_overlay());
        assert!(dv.ai_line_marks.is_empty());
    }

    #[test]
    fn civil_from_days_edge_dates() {
        // Unix epoch
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // Day before the epoch (negative days must not break the era math)
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        // Leap day: 2024-02-29 = 19_782 days after the epoch
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        // Day after a leap day rolls into March
        assert_eq!(civil_from_days(19_783), (2024, 3, 1));
        // Century non-leap boundary: 2100 is NOT a leap year
        // 2100-02-28 = 47_540; the next day is March 1st, not Feb 29th.
        assert_eq!(civil_from_days(47_540), (2100, 2, 28));
        assert_eq!(civil_from_days(47_541), (2100, 3, 1));
        // Year boundary
        assert_eq!(civil_from_days(20_088), (2024, 12, 31));
        assert_eq!(civil_from_days(20_089), (2025, 1, 1));
    }
}
