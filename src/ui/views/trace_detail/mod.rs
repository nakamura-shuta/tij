//! Trace Detail View (A6 + A3)
//!
//! Rich, read-only view of the Agent Trace records anchored to one change:
//! timestamp / tool+version / contributor breakdown / per-file ranges, and a
//! flattened list of URLs (conversation + `related[]`). Only URL rows are
//! selectable; `y` copies the selected URL. Replaces the Phase 2 Select
//! dialog (which fit "list + single action" but not this multi-section view).

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
    /// `y` pressed with no selectable URL — show an info notice
    NoUrl,
}

/// A selectable URL row (flattened across records / conversations)
#[derive(Debug, Clone)]
pub struct UrlRow {
    /// Index of the owning record (for grouping in render)
    pub record_index: usize,
    /// Label (`conversation` or the related `type`)
    pub label: String,
    pub url: String,
}

/// Trace Detail View state
#[derive(Debug, Default)]
pub struct TraceDetailView {
    /// Short change id shown in the title
    change_short: String,
    /// Records anchored to the change (owned snapshot taken at open time)
    records: Vec<TraceRecord>,
    /// Flattened, selectable URL rows (in record/document order)
    url_rows: Vec<UrlRow>,
    /// Index into `url_rows` (meaningless when `url_rows` is empty)
    selected_url: usize,
}

impl TraceDetailView {
    /// Build the view from the records anchored to `change_short`.
    pub fn new(change_short: String, records: Vec<TraceRecord>) -> Self {
        let mut url_rows = Vec::new();
        for (record_index, record) in records.iter().enumerate() {
            for (label, url) in record.all_urls() {
                url_rows.push(UrlRow {
                    record_index,
                    label,
                    url,
                });
            }
        }
        Self {
            change_short,
            records,
            url_rows,
            selected_url: 0,
        }
    }

    pub fn change_short(&self) -> &str {
        &self.change_short
    }

    pub fn records(&self) -> &[TraceRecord] {
        &self.records
    }

    pub fn url_rows(&self) -> &[UrlRow] {
        &self.url_rows
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Whether any URL is selectable
    pub fn has_urls(&self) -> bool {
        !self.url_rows.is_empty()
    }

    pub fn selected_url_index(&self) -> usize {
        self.selected_url
    }

    /// The currently selected URL, if any
    pub fn selected_url(&self) -> Option<&UrlRow> {
        self.url_rows.get(self.selected_url)
    }

    /// Move to the next selectable URL row (no-op when empty)
    pub fn select_next(&mut self) {
        if !self.url_rows.is_empty() {
            self.selected_url = (self.selected_url + 1).min(self.url_rows.len() - 1);
        }
    }

    /// Move to the previous selectable URL row
    pub fn select_prev(&mut self) {
        self.selected_url = self.selected_url.saturating_sub(1);
    }
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

    #[test]
    fn flattens_urls_in_order() {
        let v = TraceDetailView::new(
            "xqnktzml".to_string(),
            vec![record(Some("conv"), &[("pr", "prurl")])],
        );
        let rows = v.url_rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].label.as_str(), rows[0].url.as_str()),
            ("conversation", "conv")
        );
        assert_eq!(
            (rows[1].label.as_str(), rows[1].url.as_str()),
            ("pr", "prurl")
        );
    }

    #[test]
    fn no_urls_means_nothing_selectable() {
        let v = TraceDetailView::new("x".to_string(), vec![record(None, &[])]);
        assert!(!v.has_urls());
        assert!(v.selected_url().is_none());
    }

    #[test]
    fn navigation_clamps_to_url_rows() {
        let mut v =
            TraceDetailView::new("x".to_string(), vec![record(Some("u1"), &[("pr", "u2")])]);
        assert_eq!(v.selected_url_index(), 0);
        v.select_next();
        assert_eq!(v.selected_url_index(), 1);
        v.select_next(); // clamp
        assert_eq!(v.selected_url_index(), 1);
        v.select_prev();
        v.select_prev(); // clamp
        assert_eq!(v.selected_url_index(), 0);
    }

    #[test]
    fn selected_url_returns_current_row() {
        let v = TraceDetailView::new("x".to_string(), vec![record(Some("u1"), &[])]);
        assert_eq!(v.selected_url().map(|r| r.url.as_str()), Some("u1"));
    }
}
