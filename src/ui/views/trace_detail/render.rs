//! Rendering for the Trace Detail View

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::model::Notification;
use crate::trace::TraceRecord;
use crate::ui::{components, theme};

use super::{TraceDetailView, UrlRow};

/// A rendered body line, tagged so navigation/highlight can find URL rows.
enum Row {
    /// Plain display line (header, contributor, range, label)
    Plain(Line<'static>),
    /// A selectable URL row; carries its index into `url_rows`
    Url {
        url_index: usize,
        line: Line<'static>,
    },
}

impl TraceDetailView {
    /// Render the view (status bar is drawn by App, like other views).
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
        self.render_header(frame, chunks[0], notification);
        self.render_body(frame, chunks[1]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let title = Line::from(vec![
            Span::raw(" Tij - Agent Traces ").bold(),
            Span::raw("["),
            Span::styled(
                self.change_short().to_string(),
                Style::default().fg(theme::log_view::CHANGE_ID),
            ),
            Span::raw("] "),
        ])
        .centered();

        let title_width = title.width();
        let available = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let block = match notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available)))
            .filter(|line| !line.spans.is_empty())
        {
            Some(notif) => components::header_block(title).title(notif.right_aligned()),
            None => components::header_block(title),
        };

        let summary = format!(
            "{} record(s){}",
            self.record_count(),
            if self.has_urls() {
                "  —  [j/k] select URL  [y] copy"
            } else {
                "  —  (no URLs to copy)"
            }
        );
        let para = Paragraph::new(Line::from(Span::styled(
            summary,
            Style::default().fg(Color::DarkGray),
        )))
        .block(block);
        frame.render_widget(para, area);
    }

    fn render_body(&self, frame: &mut Frame, area: Rect) {
        let rows = self.build_rows();
        let height = area.height as usize;

        // Auto-scroll so the selected URL row stays visible (keeps a couple of
        // context rows above it). Computed here — render is &self, no state.
        let start = self.scroll_start(&rows, height);
        let lines: Vec<Line> = rows
            .iter()
            .skip(start)
            .take(height)
            .map(|row| match row {
                Row::Plain(line) => line.clone(),
                Row::Url { url_index, line } => {
                    if *url_index == self.selected_url_index() {
                        line.clone().style(
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        line.clone()
                    }
                }
            })
            .collect();

        let para = Paragraph::new(lines).block(components::side_borders_block());
        frame.render_widget(para, area);
    }

    /// First visible row index so the selected URL row stays on screen.
    fn scroll_start(&self, rows: &[Row], height: usize) -> usize {
        if height == 0 || rows.len() <= height {
            return 0;
        }
        // Absolute row index of the selected URL (if any)
        let sel = self.selected_url_index();
        let Some(abs) = rows
            .iter()
            .position(|r| matches!(r, Row::Url { url_index, .. } if *url_index == sel))
        else {
            return 0;
        };
        // Keep 2 rows of context above; clamp to the last full page.
        let start = abs.saturating_sub(2);
        start.min(rows.len() - height)
    }

    /// Build every body row (plain + selectable URL) in display order.
    fn build_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut url_index = 0;

        for (i, record) in self.records().iter().enumerate() {
            if i > 0 {
                rows.push(Row::Plain(Line::from("")));
            }
            self.push_record_rows(record, &mut rows, &mut url_index);
        }
        if rows.is_empty() {
            rows.push(Row::Plain(Line::from(Span::styled(
                "(no trace records)",
                Style::default().fg(Color::DarkGray).italic(),
            ))));
        }
        rows
    }

    fn push_record_rows(&self, record: &TraceRecord, rows: &mut Vec<Row>, url_index: &mut usize) {
        // Header: time + tool + version
        let time = format_timestamp(&record.timestamp);
        let tool = record.tool_name.as_deref().unwrap_or("unknown");
        let version = record
            .tool_version
            .as_deref()
            .map(|v| format!(" {}", v))
            .unwrap_or_default();
        rows.push(Row::Plain(Line::from(vec![
            Span::styled(time, Style::default().fg(Color::Gray)),
            Span::raw("  "),
            Span::styled(
                format!("{}{}", tool, version),
                Style::default().fg(Color::Cyan).bold(),
            ),
        ])));

        // Contributor breakdown + model
        let model = record
            .primary_model_id()
            .map(|m| format!(" ({})", m))
            .unwrap_or_default();
        rows.push(Row::Plain(Line::from(vec![
            Span::raw("  contributor: "),
            Span::styled(
                record.contributor_counts().summary(),
                Style::default().fg(theme::log_view::AI_BADGE),
            ),
            Span::raw(model),
        ])));

        // Ranges per code file (pseudo-files excluded — not code attribution)
        for file in record.code_files() {
            let ranges: Vec<String> = file
                .conversations
                .iter()
                .flat_map(|c| &c.ranges)
                .map(|r| format!("L{}-{}", r.start_line, r.end_line))
                .collect();
            let range_text = if ranges.is_empty() {
                String::new()
            } else {
                format!("  {}", ranges.join(", "))
            };
            rows.push(Row::Plain(Line::from(vec![
                Span::raw("  "),
                Span::styled(file.path.clone(), Style::default().fg(Color::White)),
                Span::styled(range_text, Style::default().fg(Color::DarkGray)),
            ])));
        }

        // URLs (selectable)
        let urls = record.all_urls();
        if urls.is_empty() {
            rows.push(Row::Plain(Line::from(Span::styled(
                "  URLs: (none)",
                Style::default().fg(Color::DarkGray),
            ))));
        } else {
            rows.push(Row::Plain(Line::from(Span::raw("  URLs:"))));
            for (label, url) in urls {
                let row = UrlRow {
                    record_index: 0, // unused for render
                    label: label.clone(),
                    url: url.clone(),
                };
                rows.push(Row::Url {
                    url_index: *url_index,
                    line: url_line(&row),
                });
                *url_index += 1;
            }
        }
    }
}

/// `    [label] url`
fn url_line(row: &UrlRow) -> Line<'static> {
    Line::from(vec![
        Span::raw("    "),
        Span::styled(
            format!("[{}] ", row.label),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw(row.url.clone()),
    ])
}

/// RFC 3339 → `YYYY-MM-DD HH:MM` (best-effort; unknown passes through).
fn format_timestamp(ts: &str) -> String {
    if ts.len() >= 16 && ts.as_bytes().get(10) == Some(&b'T') {
        format!("{} {}", &ts[..10], &ts[11..16])
    } else {
        ts.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{
        ContributorKind, TraceContributor, TraceConversation, TraceFile, TraceRange, TraceRecord,
        TraceRelated,
    };

    fn full_record() -> TraceRecord {
        TraceRecord {
            timestamp: "2026-06-05T14:20:00Z".to_string(),
            vcs: None,
            tool_name: Some("claude-code".to_string()),
            tool_version: Some("2.0".to_string()),
            files: vec![TraceFile {
                path: "src/greet.py".to_string(),
                conversations: vec![TraceConversation {
                    url: Some("conv-url".to_string()),
                    contributor: Some(TraceContributor {
                        kind: ContributorKind::Ai,
                        model_id: Some("anthropic/claude-opus-4-8".to_string()),
                    }),
                    ranges: vec![TraceRange {
                        start_line: 1,
                        end_line: 6,
                        contributor: None,
                    }],
                    related: vec![TraceRelated {
                        rel_type: "pull-request".to_string(),
                        url: "pr-url".to_string(),
                    }],
                }],
            }],
        }
    }

    fn flat(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|r| {
                let line = match r {
                    Row::Plain(l) => l,
                    Row::Url { line, .. } => line,
                };
                line.spans.iter().map(|s| s.content.as_ref()).collect()
            })
            .collect()
    }

    #[test]
    fn rows_include_version_contributor_ranges_and_urls() {
        let v = TraceDetailView::new("xqnktzml".to_string(), vec![full_record()]);
        let rows = v.build_rows();
        let text = flat(&rows).join("\n");
        assert!(text.contains("claude-code 2.0"), "tool+version: {text}");
        assert!(text.contains("ai×1"), "contributor count: {text}");
        assert!(text.contains("anthropic/claude-opus-4-8"), "model: {text}");
        assert!(text.contains("src/greet.py"), "file: {text}");
        assert!(text.contains("L1-6"), "range: {text}");
        assert!(text.contains("[conversation] conv-url"), "conv url: {text}");
        assert!(
            text.contains("[pull-request] pr-url"),
            "related url: {text}"
        );
    }

    #[test]
    fn url_rows_are_indexed_in_order() {
        let v = TraceDetailView::new("x".to_string(), vec![full_record()]);
        let rows = v.build_rows();
        let url_indices: Vec<usize> = rows
            .iter()
            .filter_map(|r| match r {
                Row::Url { url_index, .. } => Some(*url_index),
                _ => None,
            })
            .collect();
        assert_eq!(url_indices, vec![0, 1]);
    }

    #[test]
    fn no_url_record_shows_none_marker() {
        let mut rec = full_record();
        rec.files[0].conversations[0].url = None;
        rec.files[0].conversations[0].related = vec![];
        let v = TraceDetailView::new("x".to_string(), vec![rec]);
        let text = flat(&v.build_rows()).join("\n");
        assert!(text.contains("URLs: (none)"), "got: {text}");
    }
}
