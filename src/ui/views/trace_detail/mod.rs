//! Trace Detail View (A6 + A3)
//!
//! Rich, read-only view of the Agent Trace records anchored to one change:
//! timestamp / tool+version / contributor breakdown / per-file ranges, and the
//! URLs (conversation + `related[]`). Replaces the Phase 2 Select dialog (which
//! fit "list + single action" but not this multi-section view).
//!
//! Navigation is a single row cursor over ALL body rows (row-based scroll), so
//! every detail line is reachable with `j/k`. `y` copies the URL when the
//! cursor is on a URL row, otherwise shows a "No URL" notice.

mod input;
mod render;

use crate::trace::TraceRecord;

/// Action returned by the Trace Detail View after handling input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceDetailAction {
    /// No action needed
    None,
    /// Return to the previous view (Log)
    Back,
    /// Copy this URL to the clipboard
    CopyUrl(String),
    /// `y` pressed on a non-URL row — show an info notice
    NoUrl,
}

/// One logical body row. The view owns these (data); `render` maps them to
/// styled lines (presentation) and highlights the cursor row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DetailRow {
    /// Spacer between records
    Blank,
    /// `2026-06-05 14:20  claude-code 2.0`
    Header {
        timestamp: String,
        tool: String,
        version: Option<String>,
    },
    /// `contributor: ai×5  human×1  (model)`
    Contributor {
        summary: String,
        model: Option<String>,
    },
    /// `src/main.rs  L1-6, L20-25`
    Range { path: String, ranges: String },
    /// `URLs:` (present) or `URLs: (none)`
    UrlsHeader { present: bool },
    /// A copyable URL row: `[label] url`
    Url { label: String, url: String },
}

/// Trace Detail View state
#[derive(Debug, Default)]
pub struct TraceDetailView {
    /// Short change id shown in the title
    change_short: String,
    /// Number of records (for the header summary)
    record_count: usize,
    /// All logical body rows in display order
    rows: Vec<DetailRow>,
    /// Cursor over `rows` (row-based scroll driver)
    cursor: usize,
}

impl TraceDetailView {
    /// Build the view from the records anchored to `change_short`.
    pub fn new(change_short: String, records: Vec<TraceRecord>) -> Self {
        let rows = build_rows(&records);
        Self {
            change_short,
            record_count: records.len(),
            rows,
            cursor: 0,
        }
    }

    pub fn change_short(&self) -> &str {
        &self.change_short
    }

    pub fn record_count(&self) -> usize {
        self.record_count
    }

    pub(super) fn rows(&self) -> &[DetailRow] {
        &self.rows
    }

    pub(super) fn cursor(&self) -> usize {
        self.cursor
    }

    /// Whether any row is a copyable URL (drives the `[y]` hint)
    pub fn has_urls(&self) -> bool {
        self.rows.iter().any(|r| matches!(r, DetailRow::Url { .. }))
    }

    /// URL under the cursor, if the cursor is on a URL row
    pub fn current_url(&self) -> Option<&str> {
        match self.rows.get(self.cursor) {
            Some(DetailRow::Url { url, .. }) => Some(url),
            _ => None,
        }
    }

    /// Move the cursor down one row (row-based scroll; clamped)
    pub fn select_next(&mut self) {
        if !self.rows.is_empty() {
            self.cursor = (self.cursor + 1).min(self.rows.len() - 1);
        }
    }

    /// Move the cursor up one row
    pub fn select_prev(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }
}

/// Flatten records into logical rows (record blocks separated by Blank).
fn build_rows(records: &[TraceRecord]) -> Vec<DetailRow> {
    let mut rows = Vec::new();
    for (i, record) in records.iter().enumerate() {
        if i > 0 {
            rows.push(DetailRow::Blank);
        }
        rows.push(DetailRow::Header {
            timestamp: record.timestamp.clone(),
            tool: record
                .tool_name
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            version: record.tool_version.clone(),
        });
        rows.push(DetailRow::Contributor {
            summary: record.contributor_counts().summary(),
            model: record.primary_model_id().map(str::to_string),
        });
        // Code files only (pseudo-files are not code attribution)
        for file in record.code_files() {
            let ranges: Vec<String> = file
                .conversations
                .iter()
                .flat_map(|c| &c.ranges)
                .map(|r| format!("L{}-{}", r.start_line, r.end_line))
                .collect();
            rows.push(DetailRow::Range {
                path: file.path.clone(),
                ranges: ranges.join(", "),
            });
        }
        let urls = record.all_urls();
        rows.push(DetailRow::UrlsHeader {
            present: !urls.is_empty(),
        });
        for (label, url) in urls {
            rows.push(DetailRow::Url { label, url });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{
        ContributorKind, TraceContributor, TraceConversation, TraceFile, TraceRecord, TraceRelated,
    };

    fn record(url: Option<&str>, related: &[(&str, &str)]) -> TraceRecord {
        TraceRecord {
            timestamp: "2026-06-05T14:20:00Z".to_string(),
            vcs: None,
            tool_name: Some("claude-code".to_string()),
            tool_version: Some("2.0".to_string()),
            files: vec![TraceFile {
                path: "src/main.rs".to_string(),
                conversations: vec![TraceConversation {
                    url: url.map(str::to_string),
                    contributor: Some(TraceContributor {
                        kind: ContributorKind::Ai,
                        model_id: Some("anthropic/claude-opus-4-8".to_string()),
                    }),
                    ranges: vec![],
                    related: related
                        .iter()
                        .map(|(t, u)| TraceRelated {
                            rel_type: t.to_string(),
                            url: u.to_string(),
                        })
                        .collect(),
                }],
            }],
        }
    }

    fn url_rows(v: &TraceDetailView) -> Vec<(&str, &str)> {
        v.rows()
            .iter()
            .filter_map(|r| match r {
                DetailRow::Url { label, url } => Some((label.as_str(), url.as_str())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn flattens_urls_in_order() {
        let v = TraceDetailView::new(
            "xqnktzml".to_string(),
            vec![record(Some("conv"), &[("pr", "prurl")])],
        );
        assert_eq!(
            url_rows(&v),
            vec![("conversation", "conv"), ("pr", "prurl")]
        );
    }

    #[test]
    fn no_urls_means_nothing_copyable() {
        let v = TraceDetailView::new("x".to_string(), vec![record(None, &[])]);
        assert!(!v.has_urls());
        // cursor starts on the header row, which is not a URL
        assert!(v.current_url().is_none());
    }

    #[test]
    fn cursor_moves_over_all_rows_and_clamps() {
        let mut v = TraceDetailView::new("x".to_string(), vec![record(Some("u1"), &[])]);
        let n = v.rows().len();
        assert!(n >= 4, "header+contributor+range+urlsheader+url");
        assert_eq!(v.cursor(), 0);
        for _ in 0..(n + 5) {
            v.select_next();
        }
        assert_eq!(v.cursor(), n - 1, "clamped at last row");
        for _ in 0..(n + 5) {
            v.select_prev();
        }
        assert_eq!(v.cursor(), 0, "clamped at first row");
    }

    #[test]
    fn current_url_set_when_cursor_on_url_row() {
        let mut v = TraceDetailView::new("x".to_string(), vec![record(Some("u1"), &[])]);
        // walk the cursor down until it lands on the URL row
        while v.current_url().is_none() && v.cursor() + 1 < v.rows().len() {
            v.select_next();
        }
        assert_eq!(v.current_url(), Some("u1"));
    }
}
