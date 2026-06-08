//! Revision matching between trace records and log rows
//!
//! Log rows carry `short(8)` IDs while trace records carry full identifiers,
//! so matching is prefix-based: `full_revision.starts_with(short_id)`.
//!
//! Confidence levels (SoW §6.2):
//! - `vcs.type: "jj"` (change ID) → **confirmed** → `[AI]`
//! - `vcs.type: "git"` (commit SHA) → **heuristic** → `[AI?]`, because the
//!   reference writer records `git rev-parse HEAD`, which in jj colocated
//!   repos points at @- — the trace may be anchored one change off.

use std::collections::{HashMap, HashSet};

use crate::model::Change;

use super::model::{TraceRecord, TraceVcsType};

/// Minimum revision length accepted for matching. Shorter strings (or empty
/// ones from broken writers) would prefix-match far too loosely.
const MIN_REVISION_LEN: usize = 8;

/// Commit keys (log-row `commit_id` strings) carrying AI badges, by confidence
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AiBadgeSets {
    /// `vcs.type: "jj"` matches → `[AI]`
    pub confirmed: HashSet<String>,
    /// `vcs.type: "git"` matches → `[AI?]`
    pub heuristic: HashSet<String>,
}

impl AiBadgeSets {
    pub fn is_empty(&self) -> bool {
        self.confirmed.is_empty() && self.heuristic.is_empty()
    }
}

/// Per-change AI confidence (mirrors the badge: confirmed → `[AI]`,
/// heuristic → `[AI?]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiConfidence {
    Confirmed,
    Heuristic,
}

/// Aggregated AI attribution over a set of changes (A1). Reused by A8/A7.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraceSummary {
    /// Changes considered (graph-only excluded)
    pub total: usize,
    /// Changes with any AI attribution (`ai_confirmed + ai_heuristic`)
    pub ai_total: usize,
    /// jj-anchored AI changes (`[AI]`)
    pub ai_confirmed: usize,
    /// git-anchored AI changes (`[AI?]`)
    pub ai_heuristic: usize,
    /// model_id → number of AI changes carrying that model (tally, may exceed
    /// `ai_total` when a change uses multiple models)
    pub by_model: std::collections::BTreeMap<String, usize>,
}

impl TraceSummary {
    /// AI percentage of the total, 0 when there are no changes.
    pub fn ai_percent(&self) -> u32 {
        if self.total == 0 {
            0
        } else {
            ((self.ai_total as f64 / self.total as f64) * 100.0).round() as u32
        }
    }

    /// One-line summary (A1 display):
    /// `AI 12/40 (30%) · [AI] 9 [AI?] 3 · models: opus ×8, gpt ×3`
    pub fn one_line(&self) -> String {
        let models = if self.by_model.is_empty() {
            "—".to_string()
        } else {
            // count desc, then name asc
            let mut entries: Vec<(&String, &usize)> = self.by_model.iter().collect();
            entries.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            entries
                .iter()
                .map(|(name, n)| format!("{} ×{}", name, n))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "AI {}/{} ({}%) · [AI] {} [AI?] {} · models: {}",
            self.ai_total,
            self.total,
            self.ai_percent(),
            self.ai_confirmed,
            self.ai_heuristic,
            models
        )
    }
}

/// One AI-contributing record with a usable VCS anchor
#[derive(Debug, Clone)]
struct AnchoredRecord {
    vcs_type: TraceVcsType,
    /// Full revision string (jj change ID or git commit SHA)
    revision: String,
    record: TraceRecord,
}

/// Pre-filtered AI-contributing records, anchored by VCS revision
#[derive(Debug, Clone, Default)]
pub struct TraceIndex {
    anchored: Vec<AnchoredRecord>,
}

impl TraceIndex {
    /// Build an index from parsed records, keeping only AI-contributing
    /// records with a usable VCS anchor.
    pub fn build(records: &[TraceRecord]) -> Self {
        let mut index = TraceIndex::default();
        for record in records {
            if !record.has_ai_contribution() {
                continue;
            }
            let Some(vcs) = &record.vcs else { continue };
            if vcs.revision.len() < MIN_REVISION_LEN {
                continue;
            }
            if vcs.vcs_type == TraceVcsType::Other {
                continue;
            }
            index.anchored.push(AnchoredRecord {
                vcs_type: vcs.vcs_type,
                revision: vcs.revision.clone(),
                record: record.clone(),
            });
        }
        index
    }

    /// True when no record can ever match (skip per-refresh work)
    pub fn is_empty(&self) -> bool {
        self.anchored.is_empty()
    }

    /// Classify a single (change_id, commit_id) pair into the badge sets
    /// (prefix match, §6.2). Shared by `match_commits` and `match_blame_lines`
    /// so the jj-confirmed / git-heuristic rule lives in one place. Keyed by
    /// `commit_id` (unique per row even for divergent changes).
    fn classify_into(&self, change_id: &str, commit_id: &str, sets: &mut AiBadgeSets) {
        if change_id.is_empty() || commit_id.is_empty() {
            return;
        }
        if self
            .anchored
            .iter()
            .any(|a| a.vcs_type == TraceVcsType::Jj && a.revision.starts_with(change_id))
        {
            sets.confirmed.insert(commit_id.to_string());
        } else if self
            .anchored
            .iter()
            .any(|a| a.vcs_type == TraceVcsType::Git && a.revision.starts_with(commit_id))
        {
            sets.heuristic.insert(commit_id.to_string());
        }
    }

    /// Match log rows against the index (prefix match, §6.2).
    ///
    /// Returned sets are keyed by the row's `commit_id` string — unique per
    /// row even for divergent changes, and what the renderer has at hand.
    pub fn match_commits(&self, changes: &[Change]) -> AiBadgeSets {
        let mut sets = AiBadgeSets::default();
        for change in changes {
            if change.is_graph_only {
                continue;
            }
            self.classify_into(
                change.change_id.as_str(),
                change.commit_id.as_str(),
                &mut sets,
            );
        }
        sets
    }

    /// Per-change AI confidence (None = no AI trace) — the single-change form
    /// of `classify_into`, reused by `summarize`.
    fn confidence_of(&self, change_id: &str, commit_id: &str) -> Option<AiConfidence> {
        if change_id.is_empty() || commit_id.is_empty() {
            return None;
        }
        if self
            .anchored
            .iter()
            .any(|a| a.vcs_type == TraceVcsType::Jj && a.revision.starts_with(change_id))
        {
            Some(AiConfidence::Confirmed)
        } else if self
            .anchored
            .iter()
            .any(|a| a.vcs_type == TraceVcsType::Git && a.revision.starts_with(commit_id))
        {
            Some(AiConfidence::Heuristic)
        } else {
            None
        }
    }

    /// Aggregate AI attribution over a set of log changes (A1 — the base that
    /// A8 report / A7 orphan-detection reuse).
    ///
    /// Counting unit is the change. `ai_confirmed + ai_heuristic == ai_total`
    /// (confirmed wins, no double count). `by_model` counts, per model_id, how
    /// many AI changes carry a record with that model — a change with two
    /// models counts once per model (a tally of model usage, NOT a breakdown
    /// of `ai_total`). Pseudo-file-only records never reach here (the index
    /// only keeps AI-contributing records). The caller decides the change set
    /// (e.g. all loaded changes — filter-independent).
    pub fn summarize(&self, changes: &[Change]) -> TraceSummary {
        let mut s = TraceSummary::default();
        for change in changes {
            if change.is_graph_only {
                continue;
            }
            s.total += 1;
            let cid = change.change_id.as_str();
            let coid = change.commit_id.as_str();
            match self.confidence_of(cid, coid) {
                Some(AiConfidence::Confirmed) => {
                    s.ai_total += 1;
                    s.ai_confirmed += 1;
                }
                Some(AiConfidence::Heuristic) => {
                    s.ai_total += 1;
                    s.ai_heuristic += 1;
                }
                None => continue,
            }
            // Tally every distinct model on this AI change (a change with two
            // models counts once per model — model_ids() enumerates all of
            // them, deduped across the change's records).
            let mut seen = std::collections::BTreeSet::new();
            for record in self.records_for(cid, coid) {
                for model in record.model_ids() {
                    if seen.insert(model.to_string()) {
                        *s.by_model.entry(model.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
        s
    }

    /// Match blame lines against the index (Phase 4a — change-unit badges).
    ///
    /// Each item is a `(change_id, commit_id)` pair (the short IDs a blame
    /// line carries). Uses the same prefix rule as [`Self::match_commits`];
    /// returned sets are keyed by `commit_id`. Duplicate IDs across lines
    /// collapse naturally (HashSet).
    pub fn match_blame_lines(&self, lines: &[(&str, &str)]) -> AiBadgeSets {
        let mut sets = AiBadgeSets::default();
        for &(change_id, commit_id) in lines {
            self.classify_into(change_id, commit_id, &mut sets);
        }
        sets
    }

    /// All records anchored to the given change (Phase 2: trace detail).
    ///
    /// `change_id` / `commit_id` are the log row's short(8) IDs; matching is
    /// the same prefix rule as [`Self::match_commits`]. Both confirmed (jj)
    /// and heuristic (git) anchors are returned — the caller distinguishes
    /// them per-record via the record's `vcs` field if needed.
    pub fn records_for(&self, change_id: &str, commit_id: &str) -> Vec<&TraceRecord> {
        if change_id.is_empty() || commit_id.is_empty() {
            return Vec::new();
        }
        self.anchored
            .iter()
            .filter(|a| match a.vcs_type {
                TraceVcsType::Jj => a.revision.starts_with(change_id),
                TraceVcsType::Git => a.revision.starts_with(commit_id),
                TraceVcsType::Other => false,
            })
            .map(|a| &a.record)
            .collect()
    }

    /// AI-contributed line ranges per file for the given change (Phase 3:
    /// Diff View overlay). Line numbers are 1-indexed positions at the
    /// recorded revision (spec semantics — matches what `jj show` displays).
    ///
    /// A range counts as AI when its effective contributor (range-level
    /// override, else conversation-level) is `ai`/`mixed`, or — mirroring the
    /// record-level heuristic of §5.3 — when no contributor is recorded at
    /// all but the record names a tool.
    pub fn ai_ranges_for(
        &self,
        change_id: &str,
        commit_id: &str,
    ) -> HashMap<String, Vec<(usize, usize)>> {
        use super::model::ContributorKind;

        let mut by_file: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
        for record in self.records_for(change_id, commit_id) {
            let tool_fallback = record.tool_name.is_some();
            for file in &record.files {
                for conv in &file.conversations {
                    for range in &conv.ranges {
                        let effective = range.contributor.as_ref().or(conv.contributor.as_ref());
                        let is_ai = match effective {
                            Some(c) => {
                                matches!(c.kind, ContributorKind::Ai | ContributorKind::Mixed)
                            }
                            None => tool_fallback,
                        };
                        if is_ai {
                            by_file
                                .entry(file.path.clone())
                                .or_default()
                                .push((range.start_line, range.end_line));
                        }
                    }
                }
            }
        }
        by_file
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChangeId, CommitId};
    use crate::trace::model::{TraceVcs, TraceVcsType};

    fn record(vcs_type: TraceVcsType, revision: &str) -> TraceRecord {
        use crate::trace::model::{
            ContributorKind, TraceContributor, TraceConversation, TraceFile,
        };
        TraceRecord {
            timestamp: String::new(),
            vcs: Some(TraceVcs {
                vcs_type,
                revision: revision.to_string(),
            }),
            tool_name: Some("claude-code".to_string()),
            tool_version: None,
            files: vec![TraceFile {
                path: "src/main.rs".to_string(),
                conversations: vec![TraceConversation {
                    url: None,
                    contributor: Some(TraceContributor {
                        kind: ContributorKind::Ai,
                        model_id: None,
                    }),
                    ranges: vec![],
                    related: vec![],
                }],
            }],
        }
    }

    fn change(change_id: &str, commit_id: &str) -> Change {
        Change {
            change_id: ChangeId::new(change_id.to_string()),
            commit_id: CommitId::new(commit_id.to_string()),
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
        }
    }

    fn record_model(vcs_type: TraceVcsType, revision: &str, model: &str) -> TraceRecord {
        let mut r = record(vcs_type, revision);
        r.files[0].conversations[0]
            .contributor
            .as_mut()
            .unwrap()
            .model_id = Some(model.to_string());
        r
    }

    #[test]
    fn summarize_counts_confidence_and_models() {
        let index = TraceIndex::build(&[
            record_model(TraceVcsType::Jj, "aaaaaaaa1111", "opus"),
            record_model(TraceVcsType::Jj, "bbbbbbbb2222", "opus"),
            record_model(
                TraceVcsType::Git,
                "cccccccc3333333333333333333333333333cccc",
                "gpt",
            ),
        ]);
        let changes = [
            change("aaaaaaaa", "dead0001"), // jj → [AI] opus
            change("bbbbbbbb", "dead0002"), // jj → [AI] opus
            change("zzzzzzzz", "cccccccc3333333333333333333333333333cccc"), // git → [AI?] gpt
            change("nomatch1", "nomatch1"), // no AI
        ];
        let s = index.summarize(&changes);
        assert_eq!(s.total, 4);
        assert_eq!(s.ai_total, 3);
        assert_eq!(s.ai_confirmed, 2);
        assert_eq!(s.ai_heuristic, 1);
        assert_eq!(s.ai_confirmed + s.ai_heuristic, s.ai_total);
        assert_eq!(s.by_model.get("opus"), Some(&2));
        assert_eq!(s.by_model.get("gpt"), Some(&1));
        assert_eq!(s.ai_percent(), 75);
    }

    #[test]
    fn summarize_skips_graph_only_and_handles_empty() {
        let index = TraceIndex::build(&[record(TraceVcsType::Jj, "aaaaaaaa1111")]);
        let mut graph = change("zzzz", "zzzz");
        graph.is_graph_only = true;
        let changes = [change("aaaaaaaa", "d1"), graph];
        let s = index.summarize(&changes);
        assert_eq!(s.total, 1, "graph-only excluded");
        assert_eq!(s.ai_total, 1);

        // empty change set → 0/0, 0%
        let empty = index.summarize(&[]);
        assert_eq!(empty.total, 0);
        assert_eq!(empty.ai_percent(), 0);
    }

    #[test]
    fn summarize_one_line_format() {
        let index = TraceIndex::build(&[record_model(TraceVcsType::Jj, "aaaaaaaa1111", "opus")]);
        let s = index.summarize(&[change("aaaaaaaa", "d1"), change("nomatch", "nomatch")]);
        assert_eq!(
            s.one_line(),
            "AI 1/2 (50%) · [AI] 1 [AI?] 0 · models: opus ×1"
        );

        // no AI → models: —
        let none = index.summarize(&[change("nomatch", "nomatch")]);
        assert_eq!(none.one_line(), "AI 0/1 (0%) · [AI] 0 [AI?] 0 · models: —");
    }

    #[test]
    fn summarize_model_tally_dedups_within_change() {
        // a change with two records of the SAME model counts that model once
        let index = TraceIndex::build(&[
            record_model(TraceVcsType::Jj, "aaaaaaaa1111", "opus"),
            record_model(TraceVcsType::Jj, "aaaaaaaa1111", "opus"),
        ]);
        let s = index.summarize(&[change("aaaaaaaa", "d1")]);
        assert_eq!(s.ai_total, 1);
        assert_eq!(
            s.by_model.get("opus"),
            Some(&1),
            "same model once per change"
        );
    }

    #[test]
    fn summarize_change_with_two_models_counts_each() {
        // one change, two records with DIFFERENT models → each model +1
        // (ai_total stays 1; by_model sum may exceed ai_total — usage tally)
        let index = TraceIndex::build(&[
            record_model(TraceVcsType::Jj, "aaaaaaaa1111", "opus"),
            record_model(TraceVcsType::Jj, "aaaaaaaa1111", "gpt"),
        ]);
        let s = index.summarize(&[change("aaaaaaaa", "d1")]);
        assert_eq!(s.ai_total, 1);
        assert_eq!(s.by_model.get("opus"), Some(&1));
        assert_eq!(s.by_model.get("gpt"), Some(&1));
    }

    #[test]
    fn summarize_collects_multiple_models_in_one_record() {
        // a SINGLE record with two conversations of different models →
        // model_ids() enumerates both (primary_model_id would miss the 2nd)
        use crate::trace::model::{ContributorKind, TraceContributor, TraceConversation};
        let mut r = record(TraceVcsType::Jj, "aaaaaaaa1111");
        r.files[0].conversations[0]
            .contributor
            .as_mut()
            .unwrap()
            .model_id = Some("opus".to_string());
        r.files[0].conversations.push(TraceConversation {
            url: None,
            contributor: Some(TraceContributor {
                kind: ContributorKind::Ai,
                model_id: Some("gpt".to_string()),
            }),
            ranges: vec![],
            related: vec![],
        });
        let index = TraceIndex::build(&[r]);
        let s = index.summarize(&[change("aaaaaaaa", "d1")]);
        assert_eq!(s.by_model.get("opus"), Some(&1));
        assert_eq!(
            s.by_model.get("gpt"),
            Some(&1),
            "2nd conversation's model counted"
        );
    }

    #[test]
    fn jj_revision_matches_change_id_as_confirmed() {
        let index =
            TraceIndex::build(&[record(TraceVcsType::Jj, "xqnktzmlworukplnyrropmtzylsuxxlv")]);
        let changes = [change("xqnktzml", "2d31c7f1")];
        let sets = index.match_commits(&changes);
        assert!(sets.confirmed.contains("2d31c7f1"));
        assert!(sets.heuristic.is_empty());
    }

    #[test]
    fn git_revision_matches_commit_id_as_heuristic() {
        let index = TraceIndex::build(&[record(
            TraceVcsType::Git,
            "a6b2ed5ac3b509694c746a4763b97995f395172b",
        )]);
        let changes = [change("rlxnnrwv", "a6b2ed5a")];
        let sets = index.match_commits(&changes);
        assert!(sets.heuristic.contains("a6b2ed5a"));
        assert!(sets.confirmed.is_empty());
    }

    #[test]
    fn unmatched_changes_get_no_badge() {
        let index = TraceIndex::build(&[record(TraceVcsType::Git, "deadbeef00000000")]);
        let sets = index.match_commits(&[change("zzzzzzzz", "00000000")]);
        assert!(sets.is_empty());
    }

    #[test]
    fn non_ai_records_are_excluded() {
        let mut r = record(TraceVcsType::Jj, "xqnktzmlworukplnyrropmtzylsuxxlv");
        r.files[0].conversations[0].contributor = Some(crate::trace::model::TraceContributor {
            kind: crate::trace::model::ContributorKind::Human,
            model_id: None,
        });
        let index = TraceIndex::build(&[r]);
        assert!(index.is_empty());
    }

    #[test]
    fn short_revisions_are_rejected() {
        // Empty/short revisions would prefix-match everything
        let index = TraceIndex::build(&[record(TraceVcsType::Jj, "ab")]);
        assert!(index.is_empty());
    }

    #[test]
    fn vcs_less_records_are_excluded() {
        let mut r = record(TraceVcsType::Jj, "xqnktzmlworukplnyrropmtzylsuxxlv");
        r.vcs = None;
        let index = TraceIndex::build(&[r]);
        assert!(index.is_empty());
    }

    #[test]
    fn match_blame_lines_classifies_jj_and_git() {
        let index = TraceIndex::build(&[
            record(TraceVcsType::Jj, "xqnktzmlworukplnyrropmtzylsuxxlv"),
            record(
                TraceVcsType::Git,
                "a6b2ed5ac3b509694c746a4763b97995f395172b",
            ),
        ]);
        // line A → jj change, line B → git commit, line C → unmatched
        let lines = [
            ("xqnktzml", "2d31c7f1"),
            ("rlxnnrwv", "a6b2ed5a"),
            ("zzzzzzzz", "00000000"),
        ];
        let sets = index.match_blame_lines(&lines);
        assert!(sets.confirmed.contains("2d31c7f1"));
        assert!(sets.heuristic.contains("a6b2ed5a"));
        assert!(!sets.confirmed.contains("00000000"));
        assert!(!sets.heuristic.contains("00000000"));
    }

    #[test]
    fn match_blame_lines_skips_empty_ids() {
        let index =
            TraceIndex::build(&[record(TraceVcsType::Jj, "xqnktzmlworukplnyrropmtzylsuxxlv")]);
        let sets = index.match_blame_lines(&[("", ""), ("xqnktzml", "")]);
        assert!(sets.is_empty());
    }

    #[test]
    fn records_for_returns_jj_anchored_records() {
        let index =
            TraceIndex::build(&[record(TraceVcsType::Jj, "xqnktzmlworukplnyrropmtzylsuxxlv")]);
        let records = index.records_for("xqnktzml", "2d31c7f1");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tool_name.as_deref(), Some("claude-code"));
    }

    #[test]
    fn records_for_returns_git_anchored_records() {
        let index = TraceIndex::build(&[record(
            TraceVcsType::Git,
            "a6b2ed5ac3b509694c746a4763b97995f395172b",
        )]);
        assert_eq!(index.records_for("rlxnnrwv", "a6b2ed5a").len(), 1);
        assert_eq!(index.records_for("rlxnnrwv", "deadbeef").len(), 0);
    }

    #[test]
    fn records_for_empty_ids_returns_nothing() {
        let index =
            TraceIndex::build(&[record(TraceVcsType::Jj, "xqnktzmlworukplnyrropmtzylsuxxlv")]);
        assert!(index.records_for("", "").is_empty());
    }

    #[test]
    fn ai_ranges_for_collects_ranges_per_file() {
        use crate::trace::model::TraceRange;
        let mut r = record(TraceVcsType::Jj, "xqnktzmlworukplnyrropmtzylsuxxlv");
        r.files[0].conversations[0].ranges = vec![
            TraceRange {
                start_line: 1,
                end_line: 10,
                contributor: None,
            },
            TraceRange {
                start_line: 20,
                end_line: 25,
                contributor: None,
            },
        ];
        let index = TraceIndex::build(&[r]);

        let ranges = index.ai_ranges_for("xqnktzml", "2d31c7f1");
        assert_eq!(
            ranges.get("src/main.rs"),
            Some(&vec![(1, 10), (20, 25)]),
            "conversation-level ai contributor applies to its ranges"
        );
        // Unrelated change → empty
        assert!(index.ai_ranges_for("zzzzzzzz", "00000000").is_empty());
    }

    #[test]
    fn ai_ranges_for_excludes_human_ranges() {
        use crate::trace::model::{ContributorKind, TraceContributor, TraceRange};
        let mut r = record(TraceVcsType::Jj, "xqnktzmlworukplnyrropmtzylsuxxlv");
        // Conversation is ai, but one range is overridden to human
        r.files[0].conversations[0].ranges = vec![
            TraceRange {
                start_line: 1,
                end_line: 5,
                contributor: Some(TraceContributor {
                    kind: ContributorKind::Human,
                    model_id: None,
                }),
            },
            TraceRange {
                start_line: 6,
                end_line: 9,
                contributor: None,
            },
        ];
        let index = TraceIndex::build(&[r]);

        let ranges = index.ai_ranges_for("xqnktzml", "2d31c7f1");
        assert_eq!(ranges.get("src/main.rs"), Some(&vec![(6, 9)]));
    }

    #[test]
    fn graph_only_rows_are_skipped() {
        let index =
            TraceIndex::build(&[record(TraceVcsType::Jj, "xqnktzmlworukplnyrropmtzylsuxxlv")]);
        let mut c = change("xqnktzml", "2d31c7f1");
        c.is_graph_only = true;
        let sets = index.match_commits(&[c]);
        assert!(sets.is_empty());
    }
}
