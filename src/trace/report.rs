//! Markdown AI-attribution report (A8).
//!
//! Pure formatting: [`build_report`] turns a set of log changes plus the
//! trace index into a Markdown document the App writes to disk. No I/O and no
//! clock access here (the caller passes `generated_at`) so the whole thing is
//! unit-testable.
//!
//! Aggregation is **not** reinvented: the Summary section reuses A1's
//! [`TraceIndex::summarize`], and each detail row's confidence comes from the
//! same [`TraceIndex::ai_status`] the summary counts with — so `[AI]`/`[AI?]`
//! can never diverge between the headline and the table.

use crate::model::Change;

use super::index::{AiConfidence, TraceIndex};

/// Build the Markdown report for `changes` against `index`.
///
/// `generated_at` is a caller-supplied timestamp string (e.g.
/// `"2026-06-08 10:30"`) — kept as a parameter so this function stays
/// clock-independent and deterministic in tests.
///
/// The change set is the caller's choice (A8 passes **all loaded changes**, so
/// the report is independent of the A2 view filter). Graph-only rows are
/// excluded by `summarize`/`ai_status`. With no AI trace the report still
/// renders (`AI 0/N`, "No AI-attributed changes").
pub fn build_report(changes: &[Change], index: &TraceIndex, generated_at: &str) -> String {
    let summary = index.summarize(changes);
    let mut out = String::new();

    out.push_str("# Agent Trace Report\n\n");
    out.push_str(&format!("- Generated: {}\n", generated_at));
    out.push_str(&format!(
        "- Scope: {} loaded changes (jj --limit range)\n\n",
        summary.total
    ));

    // --- Summary -----------------------------------------------------------
    out.push_str("## Summary\n\n");
    out.push_str(&format!(
        "AI {}/{} ({}%) · [AI] {} · [AI?] {}\n\n",
        summary.ai_total,
        summary.total,
        summary.ai_percent(),
        summary.ai_confirmed,
        summary.ai_heuristic
    ));
    out.push_str("### By model\n");
    if summary.by_model.is_empty() {
        out.push_str("- (none)\n");
    } else {
        // count desc, then name asc — same order as A1's one_line().
        let mut entries: Vec<(&String, &usize)> = summary.by_model.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        for (name, n) in entries {
            out.push_str(&format!("- {}: {} changes\n", name, n));
        }
    }
    out.push('\n');

    // --- Per-change detail -------------------------------------------------
    out.push_str("## AI-attributed changes\n\n");
    let rows = detail_rows(changes, index);
    if rows.is_empty() {
        out.push_str("No AI-attributed changes in the loaded set.\n");
    } else {
        out.push_str("| change | conf | description | model | files | session |\n");
        out.push_str("|--------|------|-------------|-------|-------|---------|\n");
        for row in rows {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                row.change, row.conf, row.description, row.model, row.files, row.session
            ));
        }
    }

    out
}

/// One rendered detail row (all fields already Markdown-escaped).
struct DetailRow {
    change: String,
    conf: &'static str,
    description: String,
    model: String,
    files: String,
    session: String,
}

/// Build the AI-change rows in log order. Only changes `ai_status` marks AI
/// appear; the confidence label comes straight from that same call.
fn detail_rows(changes: &[Change], index: &TraceIndex) -> Vec<DetailRow> {
    let mut rows = Vec::new();
    for change in changes {
        if change.is_graph_only {
            continue;
        }
        let cid = change.change_id.as_str();
        let coid = change.commit_id.as_str();
        let conf = match index.ai_status(cid, coid) {
            Some(AiConfidence::Confirmed) => "[AI]",
            Some(AiConfidence::Heuristic) => "[AI?]",
            None => continue,
        };

        let records = index.records_for(cid, coid);

        // model: every distinct model across the change's records, in
        // first-seen order, comma-joined.
        let mut models: Vec<String> = Vec::new();
        for record in &records {
            for m in record.model_ids() {
                if !models.iter().any(|x| x == m) {
                    models.push(m.to_string());
                }
            }
        }
        let model = if models.is_empty() {
            "—".to_string()
        } else {
            escape_cell(&models.join(", "))
        };

        // session: the first URL across the change's records (if any).
        let session = records
            .iter()
            .flat_map(|r| r.all_urls())
            .map(|(_, url)| url)
            .next()
            .map(|u| escape_cell(&u))
            .unwrap_or_default();

        rows.push(DetailRow {
            change: escape_cell(cid),
            conf,
            description: escape_cell(change.display_description()),
            model,
            files: escape_cell(&format_files(index, cid, coid, &records)),
            session,
        });
    }
    rows
}

/// Maximum files listed in the `files` column before eliding with `…`.
const MAX_FILES: usize = 3;

/// Format the per-change files column: `path L1-8` per AI file, sorted by
/// path, joined with `; `, truncated to [`MAX_FILES`]. Falls back to listing
/// code-file paths (no ranges) when the records carry no AI line ranges.
fn format_files(
    index: &TraceIndex,
    change_id: &str,
    commit_id: &str,
    records: &[&super::TraceRecord],
) -> String {
    let ranges = index.ai_ranges_for(change_id, commit_id);

    let mut entries: Vec<String> = if ranges.is_empty() {
        // No line ranges recorded — list the code-file paths so the column is
        // still informative (e.g. whole-file edits with empty ranges).
        let mut paths: Vec<String> = records
            .iter()
            .flat_map(|r| r.code_files())
            .map(|f| f.path.clone())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    } else {
        let mut by_file: Vec<(&String, &Vec<(usize, usize)>)> = ranges.iter().collect();
        by_file.sort_by(|a, b| a.0.cmp(b.0));
        by_file
            .into_iter()
            .map(|(path, rs)| {
                let mut rs = rs.clone();
                rs.sort();
                let range_str = rs
                    .iter()
                    .map(|(s, e)| format!("L{}-{}", s, e))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{} {}", path, range_str)
            })
            .collect()
    };

    if entries.is_empty() {
        return String::new();
    }
    let elided = entries.len() > MAX_FILES;
    entries.truncate(MAX_FILES);
    let mut joined = entries.join("; ");
    if elided {
        joined.push_str("; …");
    }
    joined
}

/// Make a string safe inside a Markdown table cell: escape `|` and collapse
/// any newlines (a stray newline would break the row).
fn escape_cell(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Change;
    use crate::trace::model::{
        ContributorKind, TraceContributor, TraceConversation, TraceFile, TraceRange, TraceRecord,
        TraceVcs, TraceVcsType,
    };

    fn change(change_id: &str, commit_id: &str, desc: &str) -> Change {
        Change {
            change_id: change_id.into(),
            commit_id: commit_id.into(),
            description: desc.to_string(),
            ..Default::default()
        }
    }

    /// jj-anchored AI record for `revision` touching `path` with model + url.
    fn ai_record(
        revision: &str,
        path: &str,
        model: Option<&str>,
        url: Option<&str>,
    ) -> TraceRecord {
        TraceRecord {
            timestamp: String::new(),
            vcs: Some(TraceVcs {
                vcs_type: TraceVcsType::Jj,
                revision: revision.to_string(),
            }),
            tool_name: Some("claude-code".to_string()),
            tool_version: None,
            files: vec![TraceFile {
                path: path.to_string(),
                conversations: vec![TraceConversation {
                    url: url.map(|u| u.to_string()),
                    contributor: Some(TraceContributor {
                        kind: ContributorKind::Ai,
                        model_id: model.map(|m| m.to_string()),
                    }),
                    ranges: vec![TraceRange {
                        start_line: 1,
                        end_line: 8,
                        contributor: None,
                    }],
                    related: vec![],
                }],
            }],
        }
    }

    #[test]
    fn report_summary_and_table() {
        let rec = ai_record(
            "xqnktzmlworukplnyrropmtzylsuxxlv",
            "src/main.rs",
            Some("anthropic/claude-opus-4-8"),
            Some("conv-url"),
        );
        let index = TraceIndex::build(&[rec]);
        let changes = vec![
            change("xqnktzml", "2d31c7f1", "feat: add navigation"),
            change("aaaaaaaa", "bbbbbbbb", "chore: human edit"),
        ];

        let md = build_report(&changes, &index, "2026-06-08 10:30");

        assert!(md.contains("# Agent Trace Report"));
        assert!(md.contains("- Generated: 2026-06-08 10:30"));
        assert!(md.contains("- Scope: 2 loaded changes"));
        assert!(md.contains("AI 1/2 (50%) · [AI] 1 · [AI?] 0"));
        assert!(md.contains("- anthropic/claude-opus-4-8: 1 changes"));
        // detail table: AI change present, human change absent
        assert!(md.contains("| xqnktzml | [AI] | feat: add navigation |"));
        assert!(md.contains("src/main.rs L1-8"));
        assert!(md.contains("conv-url"));
        assert!(!md.contains("chore: human edit"));
    }

    #[test]
    fn report_no_ai_changes() {
        let index = TraceIndex::default();
        let changes = vec![change("aaaaaaaa", "bbbbbbbb", "human only")];
        let md = build_report(&changes, &index, "t");
        assert!(md.contains("AI 0/1 (0%)"));
        assert!(md.contains("### By model\n- (none)"));
        assert!(md.contains("No AI-attributed changes in the loaded set."));
        assert!(!md.contains("| change |"), "no table when 0 AI changes");
    }

    #[test]
    fn report_empty_index_counts_total() {
        // No trace at all → AI 0/N, not 0/0.
        let index = TraceIndex::default();
        let changes = vec![
            change("aaaaaaaa", "bbbbbbbb", "one"),
            change("cccccccc", "dddddddd", "two"),
        ];
        let md = build_report(&changes, &index, "t");
        assert!(md.contains("AI 0/2 (0%)"));
        assert!(md.contains("- Scope: 2 loaded changes"));
    }

    #[test]
    fn report_escapes_pipe_in_description() {
        let rec = ai_record(
            "xqnktzmlworukplnyrropmtzylsuxxlv",
            "src/main.rs",
            None,
            None,
        );
        let index = TraceIndex::build(&[rec]);
        let changes = vec![change("xqnktzml", "2d31c7f1", "feat: a | b table")];
        let md = build_report(&changes, &index, "t");
        assert!(md.contains("feat: a \\| b table"));
        assert!(!md.contains("feat: a | b table"));
    }

    #[test]
    fn report_joins_multiple_models() {
        let mut rec = ai_record(
            "xqnktzmlworukplnyrropmtzylsuxxlv",
            "src/main.rs",
            Some("anthropic/claude-opus-4-8"),
            None,
        );
        // second conversation with a different model on the same file
        rec.files[0].conversations.push(TraceConversation {
            url: None,
            contributor: Some(TraceContributor {
                kind: ContributorKind::Ai,
                model_id: Some("openai/gpt-4o".to_string()),
            }),
            ranges: vec![TraceRange {
                start_line: 9,
                end_line: 12,
                contributor: None,
            }],
            related: vec![],
        });
        let index = TraceIndex::build(&[rec]);
        let changes = vec![change("xqnktzml", "2d31c7f1", "multi-model")];
        let md = build_report(&changes, &index, "t");
        assert!(md.contains("anthropic/claude-opus-4-8, openai/gpt-4o"));
    }

    #[test]
    fn report_excludes_graph_only_rows() {
        let index = TraceIndex::default();
        let mut graph = change("xxxxxxxx", "yyyyyyyy", "graph row");
        graph.is_graph_only = true;
        let changes = vec![change("aaaaaaaa", "bbbbbbbb", "real"), graph];
        let md = build_report(&changes, &index, "t");
        // graph-only excluded from the denominator
        assert!(md.contains("- Scope: 1 loaded changes"));
        assert!(md.contains("AI 0/1"));
    }
}
