//! Resolve list parser (jj resolve --list)

use super::{Parser, RESOLVE_LIST_REGEX};
use crate::model::ConflictFile;

impl Parser {
    /// Parse `jj resolve --list` output into conflict file list
    ///
    /// The delimiter varies by jj version:
    /// - Some versions use tab (`\t`)
    /// - jj 0.37.x uses multiple spaces
    ///
    /// Strategy: try tab first, fall back to regex matching `\d+-sided conflict`.
    pub fn parse_resolve_list(output: &str) -> Vec<ConflictFile> {
        output
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                // Strategy 1: tab delimiter
                if line.contains('\t') {
                    let parts: Vec<&str> = line.splitn(2, '\t').collect();
                    if parts.len() == 2 {
                        return Some(ConflictFile {
                            path: parts[0].trim().to_string(),
                            description: parts[1].trim().to_string(),
                        });
                    }
                }

                // Strategy 2: regex for space-delimited output
                if let Some(caps) = RESOLVE_LIST_REGEX.captures(line) {
                    return Some(ConflictFile {
                        path: caps[1].trim().to_string(),
                        description: caps[2].trim().to_string(),
                    });
                }

                None
            })
            .collect()
    }
}
