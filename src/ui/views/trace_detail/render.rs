//! Rendering for the Trace Detail View

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::model::Notification;
use crate::ui::{components, theme};

use super::{DetailRow, TraceDetailView};

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
                "  —  [j/k] scroll  [y] copy URL"
            } else {
                "  —  [j/k] scroll  (no URLs to copy)"
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
        let rows = self.rows();
        let height = area.height as usize;
        let cursor = self.cursor();
        let start = scroll_start(rows.len(), cursor, height);

        let lines: Vec<Line> = rows
            .iter()
            .enumerate()
            .skip(start)
            .take(height)
            .map(|(i, row)| {
                let line = row_to_line(row);
                if i == cursor {
                    line.style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    line
                }
            })
            .collect();

        let para = Paragraph::new(lines).block(components::side_borders_block());
        frame.render_widget(para, area);
    }
}

/// First visible row so the cursor stays on screen (keeps 2 rows of context).
fn scroll_start(total: usize, cursor: usize, height: usize) -> usize {
    if height == 0 || total <= height {
        return 0;
    }
    cursor.saturating_sub(2).min(total - height)
}

/// Map a logical row to a styled line (presentation only).
fn row_to_line(row: &DetailRow) -> Line<'static> {
    match row {
        DetailRow::Blank => Line::from(""),
        DetailRow::Header {
            timestamp,
            tool,
            version,
        } => {
            let v = version
                .as_deref()
                .map(|v| format!(" {}", v))
                .unwrap_or_default();
            Line::from(vec![
                Span::styled(
                    format_timestamp(timestamp),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{}{}", tool, v),
                    Style::default().fg(Color::Cyan).bold(),
                ),
            ])
        }
        DetailRow::Contributor { summary, model } => {
            let m = model
                .as_deref()
                .map(|m| format!(" ({})", m))
                .unwrap_or_default();
            Line::from(vec![
                Span::raw("  contributor: "),
                Span::styled(
                    summary.clone(),
                    Style::default().fg(theme::log_view::AI_BADGE),
                ),
                Span::raw(m),
            ])
        }
        DetailRow::Range { path, ranges } => {
            let range_text = if ranges.is_empty() {
                String::new()
            } else {
                format!("  {}", ranges)
            };
            Line::from(vec![
                Span::raw("  "),
                Span::styled(path.clone(), Style::default().fg(Color::White)),
                Span::styled(range_text, Style::default().fg(Color::DarkGray)),
            ])
        }
        DetailRow::UrlsHeader { present } => {
            if *present {
                Line::from(Span::raw("  URLs:"))
            } else {
                Line::from(Span::styled(
                    "  URLs: (none)",
                    Style::default().fg(Color::DarkGray),
                ))
            }
        }
        DetailRow::Url { label, url } => Line::from(vec![
            Span::raw("    "),
            Span::styled(format!("[{}] ", label), Style::default().fg(Color::Magenta)),
            Span::raw(url.clone()),
        ]),
    }
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

    fn body_text(v: &TraceDetailView) -> String {
        v.rows()
            .iter()
            .map(|r| {
                let line = row_to_line(r);
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn rows_include_version_contributor_ranges_and_urls() {
        let v = TraceDetailView::new("xqnktzml".to_string(), vec![full_record()]);
        let text = body_text(&v);
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
    fn no_url_record_shows_none_marker() {
        let mut rec = full_record();
        rec.files[0].conversations[0].url = None;
        rec.files[0].conversations[0].related = vec![];
        let v = TraceDetailView::new("x".to_string(), vec![rec]);
        assert!(body_text(&v).contains("URLs: (none)"), "{}", body_text(&v));
    }

    #[test]
    fn scroll_start_keeps_cursor_visible() {
        // 20 rows, height 5: cursor near the end scrolls down, clamped.
        assert_eq!(scroll_start(20, 0, 5), 0);
        assert_eq!(scroll_start(20, 3, 5), 1); // 3 - 2 context
        assert_eq!(scroll_start(20, 19, 5), 15); // clamp to total - height
        assert_eq!(scroll_start(3, 2, 5), 0); // fits, no scroll
    }
}
