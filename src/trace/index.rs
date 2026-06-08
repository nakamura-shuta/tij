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
