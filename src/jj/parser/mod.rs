//! jj output parser
//!
//! Parses the output from jj commands into structured data.

mod annotation;
mod bookmark;
mod diff;
mod evolog;
mod log;
mod operation;
mod push;
mod resolve;
mod status;
mod tag;
mod workspace;

pub use bookmark::{parse_advance_bookmarks, parse_bookmark_list};
pub use evolog::parse_evolog;
pub use push::{
    PushPreviewAction, PushPreviewResult, SkippedRef, parse_push_dry_run, parse_push_skipped,
};
pub use tag::parse_tag_list;
pub use workspace::parse_workspace_list;

#[cfg(test)]
mod tests;

use regex::Regex;
use std::sync::LazyLock;

/// Regex for parsing jj file annotate output with commit_id
/// Format: `<change_id>\t<commit_id> <author> <timestamp>  <line_number>: <content>`
/// Example: `twzksoxt\tabcd1234 nakamura 2026-01-30 10:43:19    1: //! Tij`
///
/// Groups:
/// 1. change_id (first token before tab)
/// 2. commit_id (token after tab, before space)
/// 3. author (between commit_id and timestamp)
/// 4. timestamp (YYYY-MM-DD HH:MM:SS)
/// 5. line_number (digits after timestamp, before colon)
/// 6. content (everything after `: ` or `:`)
static ANNOTATE_LINE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\S+)\t(\S+)\s+(.+?)\s+(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2})\s+(\d+):\s?(.*)$")
        .expect("Invalid annotate line regex")
});

/// Regex for parsing `jj resolve --list` output when using space delimiter
/// Matches: `<path>  <N>-sided conflict` (2+ spaces between path and description)
static RESOLVE_LIST_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // `\s+`, not `\s{2,}`: jj right-pads the path column to 36 cells, so a path
    // of 35+ characters is followed by a SINGLE space and the old pattern
    // dropped the line silently — a conflict in e.g.
    // `src/ui/views/command_history/render.rs` was invisible in the Resolve
    // View. `.+?` stays non-greedy and `$` anchors the description, so paths
    // containing spaces still split at the right place via backtracking.
    Regex::new(r"^(.+?)\s+(\d+-sided\s+conflict)$").expect("Invalid resolve list regex")
});

/// Parser for jj command output
pub struct Parser;
