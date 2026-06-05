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

use std::collections::HashSet;

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

/// Pre-filtered revision anchors from AI-contributing records
#[derive(Debug, Clone, Default)]
pub struct TraceIndex {
    /// Full jj change IDs from `vcs.type: "jj"` records
    jj_revisions: Vec<String>,
    /// Full git commit SHAs from `vcs.type: "git"` records
    git_revisions: Vec<String>,
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
            match vcs.vcs_type {
                TraceVcsType::Jj => index.jj_revisions.push(vcs.revision.clone()),
                TraceVcsType::Git => index.git_revisions.push(vcs.revision.clone()),
                TraceVcsType::Other => {}
            }
        }
        index
    }

    /// True when no record can ever match (skip per-refresh work)
    pub fn is_empty(&self) -> bool {
        self.jj_revisions.is_empty() && self.git_revisions.is_empty()
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
            let change_id = change.change_id.as_str();
            let commit_id = change.commit_id.as_str();
            if change_id.is_empty() || commit_id.is_empty() {
                continue;
            }

            if self.jj_revisions.iter().any(|r| r.starts_with(change_id)) {
                sets.confirmed.insert(commit_id.to_string());
            } else if self.git_revisions.iter().any(|r| r.starts_with(commit_id)) {
                sets.heuristic.insert(commit_id.to_string());
            }
        }
        sets
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
    fn graph_only_rows_are_skipped() {
        let index =
            TraceIndex::build(&[record(TraceVcsType::Jj, "xqnktzmlworukplnyrropmtzylsuxxlv")]);
        let mut c = change("xqnktzml", "2d31c7f1");
        c.is_graph_only = true;
        let sets = index.match_commits(&[c]);
        assert!(sets.is_empty());
    }
}
