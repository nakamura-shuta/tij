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

impl TraceRecord {
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
        let code_files: Vec<&TraceFile> = self
            .files
            .iter()
            .filter(|f| !PSEUDO_FILES.contains(&f.path.as_str()))
            .collect();
        if code_files.is_empty() {
            return false;
        }

        let mut saw_contributor = false;
        for file in &code_files {
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
        self.files
            .iter()
            .filter(|f| !PSEUDO_FILES.contains(&f.path.as_str()))
            .count()
    }
}
