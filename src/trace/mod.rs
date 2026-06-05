//! Agent Trace (AI code attribution) reader
//!
//! Implements the read-only side of the Agent Trace spec
//! (<https://github.com/cursor/agent-trace>, v0.1.0 RFC): tij never writes
//! traces — agent hosts (Claude Code / Cursor hooks) do — it only reads the
//! sidecar JSONL and surfaces AI-contribution badges in the Log View.
//!
//! Design principles (see `.work/docs/spec-detail/agent-trace.md`):
//! - Tolerant reader: real-world writers emit schema-violating records
//!   (`version: "1.0"`, missing `tool.version`), so parsing is field-by-field
//!   via `serde_json::Value` and never rejects a whole record for one bad
//!   field.
//! - Trace problems must never affect tij itself: missing file, bad JSON, or
//!   jj config errors all degrade to "no badges" silently.
//! - `vcs.type: "jj"` matches are **confirmed** (`[AI]`); `vcs.type: "git"`
//!   matches are **heuristic** (`[AI?]`) because git-only writers record
//!   `git rev-parse HEAD`, which in jj colocated repos points at @- (the
//!   parent of the change actually being edited).

mod index;
mod loader;
mod model;
mod overlay;

pub use index::{AiBadgeSets, TraceIndex};
pub use loader::{DEFAULT_TRACE_PATH, LoadResult, load};
pub use model::{
    ContributorKind, TraceContributor, TraceConversation, TraceFile, TraceRange, TraceRecord,
    TraceVcs, TraceVcsType,
};
pub use overlay::compute_ai_line_marks;
