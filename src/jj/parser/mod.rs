//! jj output parser
//!
//! Parses the output from jj commands into structured data.

mod annotation;
mod bookmark;
mod diff;
mod log;
mod operation;
mod push;
mod resolve;
mod status;

pub use bookmark::parse_bookmark_list;
pub use push::{PushPreviewAction, PushPreviewResult, parse_push_dry_run};

#[cfg(test)]
mod tests;

use regex::Regex;
use std::sync::LazyLock;

/// Regex for parsing jj file annotate default output
/// Format: `<change_id> <author> <timestamp>  <line_number>: <content>`
/// Example: `twzksoxt nakamura 2026-01-30 10:43:19    1: //! Tij`
///
/// Groups:
/// 1. change_id (first token, variable length)
/// 2. author (between change_id and timestamp)
/// 3. timestamp (YYYY-MM-DD HH:MM:SS)
/// 4. line_number (digits after timestamp, before colon)
/// 5. content (everything after `: ` or `:`)
static ANNOTATE_LINE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\S+)\s+(.+?)\s+(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2})\s+(\d+):\s?(.*)$")
        .expect("Invalid annotate line regex")
});

/// Regex for parsing `jj resolve --list` output when using space delimiter
/// Matches: `<path>  <N>-sided conflict` (2+ spaces between path and description)
static RESOLVE_LIST_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.+?)\s{2,}(\d+-sided\s+conflict)$").expect("Invalid resolve list regex")
});

/// File operation type from jj show output
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileOperation {
    Added,
    Modified,
    Deleted,
}

/// Parser for jj command output
pub struct Parser;
