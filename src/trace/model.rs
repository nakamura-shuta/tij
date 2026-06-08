//! Internal model for Agent Trace records (tolerant subset of the spec)
//!
//! Field names mirror the upstream schema (`schemas.ts`), but every field
//! that real-world writers omit or mangle is an `Option`. `version` / `id`
//! are intentionally not modeled — they are never displayed.

/// One trace record (one edit event recorded by an agent host)
#[derive(Debug, Clone, Default)]
pub struct TraceRecord {
    /// RFC 3339 timestamp (kept as-is for Phase 2 display)
    pub timestamp: String,
    /// VCS anchor — absent records cannot be matched to log rows
    pub vcs: Option<TraceVcs>,
    /// `tool.name` (e.g. "claude-code", "cursor")
    pub tool_name: Option<String>,
    /// `tool.version` — required by the schema but missing in real data
    pub tool_version: Option<String>,
    /// Files touched by this edit event
    pub files: Vec<TraceFile>,
}

/// VCS revision anchor of a record
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceVcs {
    pub vcs_type: TraceVcsType,
    /// git: 40-char commit SHA / jj: change ID
    pub revision: String,
}

/// Supported VCS kinds (hg/svn collapse into Other — unmatched in tij)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceVcsType {
    Git,
    Jj,
    Other,
}

/// A file entry within a record
#[derive(Debug, Clone, Default)]
pub struct TraceFile {
    /// Workspace-relative path
    pub path: String,
    pub conversations: Vec<TraceConversation>,
}

/// A conversation (session) that contributed ranges to a file
#[derive(Debug, Clone, Default)]
pub struct TraceConversation {
    /// Link to the originating AI session (Phase 2: copy to clipboard)
    pub url: Option<String>,
    pub contributor: Option<TraceContributor>,
    pub ranges: Vec<TraceRange>,
    /// Related resources (session / prompt / pull-request …) — spec `related[]`
    pub related: Vec<TraceRelated>,
}

/// A related resource attached to a conversation (spec `related[]` entry)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRelated {
    /// spec field `type` (renamed: `type` is a Rust keyword)
    pub rel_type: String,
    pub url: String,
}

/// Who wrote a range (conversation-level default, range-level override)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceContributor {
    pub kind: ContributorKind,
    /// models.dev-style ID (e.g. "anthropic/claude-opus-4-8")
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContributorKind {
    Human,
    Ai,
    Mixed,
    Unknown,
}

/// A 1-indexed line range at the recorded revision
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRange {
    pub start_line: usize,
    pub end_line: usize,
    /// Overrides the conversation-level contributor when present
    pub contributor: Option<TraceContributor>,
}

/// Pseudo-file paths the reference implementation uses for non-code events
/// (shell history, session start/end) — excluded from attribution.
const PSEUDO_FILES: [&str; 2] = [".shell-history", ".sessions"];

impl TraceFile {
    /// Whether this file is a reference-implementation pseudo-file
    /// (`.shell-history` / `.sessions`) rather than real source code.
    pub fn is_pseudo(&self) -> bool {
        PSEUDO_FILES.contains(&self.path.as_str())
    }
}

impl TraceRecord {
    /// Iterator over the record's real code files (pseudo-files excluded).
    /// The single place that defines "what counts as code" — used by AI
    /// detection, file counts, contributor breakdown, and range display so
    /// pseudo-file ranges never leak into code attribution.
    pub fn code_files(&self) -> impl Iterator<Item = &TraceFile> {
        self.files.iter().filter(|f| !f.is_pseudo())
    }

    /// Whether this record represents an AI contribution to code (§5.3)
    ///
    /// True when:
    /// 1. any contributor (conversation- or range-level) is `ai`/`mixed`, or
    /// 2. no contributor is recorded anywhere but `tool.name` exists
    ///    (reference hooks often omit contributor — being written via an
    ///    agent hook at all implies AI involvement)
    ///
    /// Records touching only pseudo-files (`.shell-history` / `.sessions`)
    /// are never AI contributions to code.
    pub fn has_ai_contribution(&self) -> bool {
        if self.code_files().next().is_none() {
            return false;
        }

        let mut saw_contributor = false;
        for file in self.code_files() {
            for conv in &file.conversations {
                if let Some(c) = &conv.contributor {
                    saw_contributor = true;
                    if matches!(c.kind, ContributorKind::Ai | ContributorKind::Mixed) {
                        return true;
                    }
                }
                for range in &conv.ranges {
                    if let Some(c) = &range.contributor {
                        saw_contributor = true;
                        if matches!(c.kind, ContributorKind::Ai | ContributorKind::Mixed) {
                            return true;
                        }
                    }
                }
            }
        }

        !saw_contributor && self.tool_name.is_some()
    }

    /// First conversation URL in the record (Phase 2: copy target)
    pub fn primary_url(&self) -> Option<&str> {
        self.files
            .iter()
            .flat_map(|f| &f.conversations)
            .find_map(|c| c.url.as_deref())
    }

    /// First model ID found in any contributor (conversation- or range-level)
    pub fn primary_model_id(&self) -> Option<&str> {
        self.files
            .iter()
            .flat_map(|f| &f.conversations)
            .find_map(|c| {
                c.contributor
                    .as_ref()
                    .and_then(|ct| ct.model_id.as_deref())
                    .or_else(|| {
                        c.ranges
                            .iter()
                            .find_map(|r| r.contributor.as_ref()?.model_id.as_deref())
                    })
            })
    }

    /// Number of code files in the record (pseudo-files excluded)
    pub fn code_file_count(&self) -> usize {
        self.code_files().count()
    }

    /// Every distinct model_id in the record (conversation- and range-level),
    /// in first-seen order. Unlike `primary_model_id` (first only), this is for
    /// aggregation (A1 `by_model`) where one record may use several models.
    pub fn model_ids(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for conv in self.files.iter().flat_map(|f| &f.conversations) {
            let conv_model = conv
                .contributor
                .as_ref()
                .and_then(|c| c.model_id.as_deref());
            if let Some(m) = conv_model
                && !out.contains(&m)
            {
                out.push(m);
            }
            for range in &conv.ranges {
                let rm = range
                    .contributor
                    .as_ref()
                    .and_then(|c| c.model_id.as_deref());
                if let Some(m) = rm
                    && !out.contains(&m)
                {
                    out.push(m);
                }
            }
        }
        out
    }

    /// All URLs in the record as `(label, url)` (A3 — Trace Detail).
    ///
    /// Order: each conversation's `url` (label "conversation") first, then its
    /// `related[]` entries (label = the related `type`). Across files and
    /// conversations in document order. Empty when the record has no URLs.
    pub fn all_urls(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for conv in self.files.iter().flat_map(|f| &f.conversations) {
            if let Some(url) = &conv.url {
                out.push(("conversation".to_string(), url.clone()));
            }
            for rel in &conv.related {
                out.push((rel.rel_type.clone(), rel.url.clone()));
            }
        }
        out
    }

    /// Per-kind counts of effective contributors over the record's ranges
    /// (A6). Effective contributor follows the Phase 3 rule: range override →
    /// conversation contributor → (tool present ? ai : unknown). Ranges only —
    /// conversations with no ranges don't contribute counts (they have no
    /// lines to attribute). Pseudo-files (`.shell-history` / `.sessions`) are
    /// excluded — they are not code attribution. Returns (ai, mixed, human,
    /// unknown).
    pub fn contributor_counts(&self) -> ContributorCounts {
        let tool_fallback = self.tool_name.is_some();
        let mut counts = ContributorCounts::default();
        for file in self.code_files() {
            for conv in &file.conversations {
                for range in &conv.ranges {
                    let effective = range.contributor.as_ref().or(conv.contributor.as_ref());
                    let kind = match effective {
                        Some(c) => c.kind,
                        None if tool_fallback => ContributorKind::Ai,
                        None => ContributorKind::Unknown,
                    };
                    match kind {
                        ContributorKind::Ai => counts.ai += 1,
                        ContributorKind::Mixed => counts.mixed += 1,
                        ContributorKind::Human => counts.human += 1,
                        ContributorKind::Unknown => counts.unknown += 1,
                    }
                }
            }
        }
        counts
    }
}

/// Per-kind effective-contributor counts over a record's ranges (A6)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContributorCounts {
    pub ai: usize,
    pub mixed: usize,
    pub human: usize,
    pub unknown: usize,
}

impl ContributorCounts {
    /// Compact display like `ai×5  human×1` (omits zero kinds; empty → "—")
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.ai > 0 {
            parts.push(format!("ai×{}", self.ai));
        }
        if self.mixed > 0 {
            parts.push(format!("mixed×{}", self.mixed));
        }
        if self.human > 0 {
            parts.push(format!("human×{}", self.human));
        }
        if self.unknown > 0 {
            parts.push(format!("unknown×{}", self.unknown));
        }
        if parts.is_empty() {
            "—".to_string()
        } else {
            parts.join("  ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(start: usize, end: usize, kind: Option<ContributorKind>) -> TraceRange {
        TraceRange {
            start_line: start,
            end_line: end,
            contributor: kind.map(|k| TraceContributor {
                kind: k,
                model_id: None,
            }),
        }
    }

    fn record(conv_kind: Option<ContributorKind>, ranges: Vec<TraceRange>) -> TraceRecord {
        TraceRecord {
            timestamp: "t".to_string(),
            vcs: None,
            tool_name: Some("claude-code".to_string()),
            tool_version: None,
            files: vec![TraceFile {
                path: "a.rs".to_string(),
                conversations: vec![TraceConversation {
                    url: None,
                    contributor: conv_kind.map(|k| TraceContributor {
                        kind: k,
                        model_id: None,
                    }),
                    ranges,
                    related: vec![],
                }],
            }],
        }
    }

    #[test]
    fn contributor_counts_and_file_count_exclude_pseudo_files() {
        // A record touching real code AND .shell-history: the pseudo-file's
        // ranges must not inflate the contributor counts or the file count.
        let mut r = record(Some(ContributorKind::Ai), vec![range(1, 5, None)]);
        r.files.push(TraceFile {
            path: ".shell-history".to_string(),
            conversations: vec![TraceConversation {
                url: None,
                contributor: Some(TraceContributor {
                    kind: ContributorKind::Ai,
                    model_id: None,
                }),
                ranges: vec![range(1, 99, None)],
                related: vec![],
            }],
        });
        // only the code file's single range counts
        assert_eq!(r.contributor_counts().ai, 1);
        assert_eq!(r.code_file_count(), 1);
    }

    #[test]
    fn contributor_counts_use_effective_rule() {
        // conversation = ai; range 2 overrides to human → ai×1 human×1
        let r = record(
            Some(ContributorKind::Ai),
            vec![range(1, 5, None), range(6, 9, Some(ContributorKind::Human))],
        );
        let c = r.contributor_counts();
        assert_eq!((c.ai, c.human, c.mixed, c.unknown), (1, 1, 0, 0));
        assert_eq!(c.summary(), "ai×1  human×1");
    }

    #[test]
    fn contributor_counts_tool_fallback_when_no_contributor() {
        // no contributor anywhere, tool present → ranges count as ai (§5.3)
        let r = record(None, vec![range(1, 3, None), range(4, 6, None)]);
        assert_eq!(r.contributor_counts().ai, 2);
    }

    #[test]
    fn contributor_counts_ignore_conversations_without_ranges() {
        // a conversation with no ranges contributes nothing (no lines)
        let r = record(Some(ContributorKind::Ai), vec![]);
        assert_eq!(r.contributor_counts(), ContributorCounts::default());
        assert_eq!(r.contributor_counts().summary(), "—");
    }

    #[test]
    fn all_urls_orders_conversation_then_related() {
        let mut r = record(Some(ContributorKind::Ai), vec![range(1, 2, None)]);
        r.files[0].conversations[0].url = Some("conv".to_string());
        r.files[0].conversations[0].related = vec![TraceRelated {
            rel_type: "pr".to_string(),
            url: "prurl".to_string(),
        }];
        assert_eq!(
            r.all_urls(),
            vec![
                ("conversation".to_string(), "conv".to_string()),
                ("pr".to_string(), "prurl".to_string()),
            ]
        );
    }
}
