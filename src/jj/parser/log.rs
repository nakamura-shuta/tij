//! Log output parser (jj log)

use super::super::JjError;
use super::super::template::FIELD_SEPARATOR;
use crate::model::Change;

use super::Parser;

impl Parser {
    /// Parse `jj log` output into a list of Changes
    ///
    /// Handles graph output with TAB-based detection:
    /// - Lines with TAB: Change lines (graph prefix + TAB-separated fields)
    /// - Lines without TAB: Graph-only lines (branch/merge lines)
    pub fn parse_log(output: &str) -> Result<Vec<Change>, JjError> {
        let mut changes = Vec::new();

        for line in output.lines() {
            if line.is_empty() {
                continue;
            }

            // TAB presence determines line type
            if let Some(tab_pos) = line.find(FIELD_SEPARATOR) {
                // Change line: extract graph prefix and parse fields
                let graph_and_id = &line[..tab_pos];
                let data_fields = &line[tab_pos + 1..];

                let (graph_prefix, change_id) = Self::split_graph_prefix(graph_and_id)?;
                let mut change = Self::parse_log_fields(change_id, data_fields)?;
                change.graph_prefix = graph_prefix;
                change.is_graph_only = false;
                changes.push(change);
            } else {
                // Graph-only line (no TAB = no data fields)
                changes.push(Change {
                    graph_prefix: line.to_string(),
                    is_graph_only: true,
                    ..Default::default()
                });
            }
        }

        Ok(changes)
    }

    /// Split graph prefix and change_id from the part before TAB
    ///
    /// Input: "│ │ ○  oqwroxvu"
    /// Output: Ok(("│ │ ○  ", "oqwroxvu"))
    ///
    /// jj's change_id uses "reversed hex" encoding with lowercase letters only.
    /// The template uses `.short(8)` which outputs `[a-z]{8}`.
    pub(super) fn split_graph_prefix(graph_and_id: &str) -> Result<(String, &str), JjError> {
        let bytes = graph_and_id.as_bytes();
        let mut id_start = bytes.len();

        // Find where the change_id starts (consecutive lowercase letters from end)
        for i in (0..bytes.len()).rev() {
            if bytes[i].is_ascii_lowercase() {
                id_start = i;
            } else if id_start < bytes.len() {
                // Hit non-lowercase after finding some lowercase chars
                break;
            }
        }

        if id_start < bytes.len() {
            let graph_prefix = graph_and_id[..id_start].to_string();
            let change_id = &graph_and_id[id_start..];
            Ok((graph_prefix, change_id))
        } else {
            // TAB exists but no change_id found - invalid format
            Err(JjError::ParseError(format!(
                "Cannot extract change_id from: {}",
                graph_and_id
            )))
        }
    }

    /// Parse TAB-separated fields after change_id
    ///
    /// Fields: commit_id, author, timestamp, description, is_working_copy, is_empty, bookmarks
    pub(super) fn parse_log_fields(change_id: &str, data: &str) -> Result<Change, JjError> {
        let fields: Vec<&str> = data.split(FIELD_SEPARATOR).collect();

        if fields.len() < 6 {
            return Err(JjError::ParseError(format!(
                "Expected at least 6 fields after change_id, got {}: {:?}",
                fields.len(),
                fields
            )));
        }

        Ok(Change {
            change_id: change_id.to_string(),
            commit_id: fields[0].to_string(),
            author: fields[1].to_string(),
            timestamp: fields[2].to_string(),
            description: fields[3].to_string(),
            is_working_copy: fields[4] == "true",
            is_empty: fields[5] == "true",
            bookmarks: if fields.len() > 6 && !fields[6].is_empty() {
                fields[6].split(',').map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            },
            graph_prefix: String::new(), // Set by caller
            is_graph_only: false,
            has_conflict: fields.get(7).map(|v| *v == "true").unwrap_or(false),
        })
    }

    // Legacy function for tests - kept for backwards compatibility
    #[cfg(test)]
    pub(super) fn parse_log_record(record: &str) -> Result<Change, JjError> {
        let fields: Vec<&str> = record.split(FIELD_SEPARATOR).collect();

        if fields.len() < 7 {
            return Err(JjError::ParseError(format!(
                "Expected at least 7 fields, got {}: {:?}",
                fields.len(),
                fields
            )));
        }

        Ok(Change {
            change_id: fields[0].to_string(),
            commit_id: fields[1].to_string(),
            author: fields[2].to_string(),
            timestamp: fields[3].to_string(),
            description: fields[4].to_string(),
            is_working_copy: fields[5] == "true",
            is_empty: fields[6] == "true",
            bookmarks: if fields.len() > 7 && !fields[7].is_empty() {
                fields[7].split(',').map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            },
            graph_prefix: String::new(),
            is_graph_only: false,
            has_conflict: fields.get(8).map(|v| *v == "true").unwrap_or(false),
        })
    }
}
