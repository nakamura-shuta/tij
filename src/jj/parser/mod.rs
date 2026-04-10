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

pub use bookmark::parse_bookmark_list;
pub use evolog::parse_evolog;
pub use push::{PushPreviewAction, PushPreviewResult, parse_push_dry_run};
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
    Regex::new(r"^(.+?)\s{2,}(\d+-sided\s+conflict)$").expect("Invalid resolve list regex")
});

/// Parser for jj command output
pub struct Parser;
