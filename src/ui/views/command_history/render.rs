//! Command History View rendering

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::CommandHistoryView;
use crate::model::{CommandHistory, CommandRecord, CommandStatus, Notification};
use crate::ui::{components, navigation, theme};

/// Maximum number of error lines to show in detail view
const MAX_ERROR_LINES: usize = 5;

impl CommandHistoryView {
    /// Render the command history view
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        history: &CommandHistory,
        notification: Option<&Notification>,
    ) {
        let count = history.len();
        let title = Line::from(format!(" Command History ({}) ", count))
            .bold()
            .cyan()
            .centered();

        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let block = components::bordered_block_with_notification(title, notif_line);

        if history.is_empty() {
            let paragraph = Paragraph::new("No commands recorded yet").block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize;
        if inner_height == 0 {
            return;
        }

        let records: Vec<&CommandRecord> = history.records().iter().collect();
        let inner_width = area.width.saturating_sub(2) as usize;

        // Calculate scroll offset, accounting for expanded detail height
        let mut scroll_offset =
            navigation::adjust_scroll(self.selected, self.scroll_offset, inner_height);

        // If the selected record is expanded, ensure detail lines are visible
        if let Some(exp_idx) = self.expanded_index
            && exp_idx == self.selected
        {
            let detail_height = detail_line_count(records[exp_idx], inner_width);
            // Total lines needed: 1 (record) + detail_height
            let total_needed = 1 + detail_height;
            // Position of selected within viewport
            let pos_in_view = self.selected.saturating_sub(scroll_offset);
            // If detail extends past viewport bottom, scroll down
            if pos_in_view + total_needed > inner_height {
                scroll_offset = (self.selected + total_needed).saturating_sub(inner_height);
            }
        }

        let mut lines: Vec<Line> = Vec::new();
        for (idx, record) in records.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }
            let is_selected = idx == self.selected;
            lines.push(build_record_line(record, is_selected, inner_width));

            // If this record is expanded, add detail lines
            if self.expanded_index == Some(idx) {
                let detail_lines = build_detail_lines(record, inner_width);
                for dl in detail_lines {
                    if lines.len() >= inner_height {
                        break;
                    }
                    lines.push(dl);
                }
            }
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

/// Build a single record line:
/// `  HH:MM:SS  OK  Operation     jj command args...`
fn build_record_line(record: &CommandRecord, is_selected: bool, _width: usize) -> Line<'static> {
    // Time column
    let time_str = format_timestamp(&record.timestamp);

    // Status column
    let (status_str, status_color) = match record.status {
        CommandStatus::Success => ("OK", Color::Green),
        CommandStatus::Failed => ("NG", Color::Red),
    };

    // Operation column (12 chars, cyan)
    let op = format!("{:<12}", truncate_str(&record.operation, 12));

    // Command column: "jj " + args joined by space
    let cmd = format!("jj {}", record.args.join(" "));

    let spans = vec![
        Span::raw("  "),
        Span::styled(time_str, Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(
            format!("{:<2}", status_str),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(op, Style::default().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled(cmd, Style::default().fg(Color::White)),
    ];

    let mut line = Line::from(spans);
    if is_selected {
        line = line.style(
            Style::default()
                .fg(theme::selection::FG)
                .bg(theme::selection::BG)
                .add_modifier(Modifier::BOLD),
        );
    }
    line
}

/// Calculate the number of detail lines for an expanded record (without building them)
fn detail_line_count(record: &CommandRecord, _width: usize) -> usize {
    // Command line + Duration line = 2
    let mut count = 2;
    if let Some(ref error) = record.error {
        let total_error_lines = error.lines().count();
        let shown = total_error_lines.min(MAX_ERROR_LINES);
        count += shown;
        if total_error_lines > shown {
            count += 1; // "... (N more lines)"
        }
    }
    count += 1; // separator line
    count
}

/// Build detail lines for an expanded record
fn build_detail_lines(record: &CommandRecord, _width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let indent = "      ";
    let detail_style = Style::default().fg(Color::DarkGray);
    let label_style = Style::default().fg(Color::Yellow);

    // Command detail
    let full_cmd = format!("jj {}", record.args.join(" "));
    lines.push(Line::from(vec![
        Span::raw(indent.to_string()),
        Span::styled("Command: ", label_style),
        Span::styled(full_cmd, detail_style),
    ]));

    // Duration
    let duration = if record.duration_ms >= 1000 {
        format!("{:.1}s", record.duration_ms as f64 / 1000.0)
    } else {
        format!("{}ms", record.duration_ms)
    };
    lines.push(Line::from(vec![
        Span::raw(indent.to_string()),
        Span::styled("Duration: ", label_style),
        Span::styled(duration, detail_style),
    ]));

    // Error (if any)
    if let Some(ref error) = record.error {
        let error_lines: Vec<&str> = error.lines().collect();
        let total = error_lines.len();
        let shown = total.min(MAX_ERROR_LINES);

        for (i, line) in error_lines.iter().take(shown).enumerate() {
            let prefix = if i == 0 { "Error: " } else { "       " };
            lines.push(Line::from(vec![
                Span::raw(indent.to_string()),
                Span::styled(prefix.to_string(), label_style),
                Span::styled(line.to_string(), Style::default().fg(Color::Red)),
            ]));
        }
        if total > shown {
            lines.push(Line::from(vec![
                Span::raw(indent.to_string()),
                Span::styled(
                    format!("       ... ({} more lines)", total - shown),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    // Separator
    lines.push(Line::from(""));

    lines
}

/// Format a SystemTime as HH:MM:SS
fn format_timestamp(timestamp: &std::time::SystemTime) -> String {
    use std::time::UNIX_EPOCH;
    let secs = timestamp
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Convert to local time (approximate: use UTC offset from libc)
    // For simplicity, calculate hours/minutes/seconds in UTC and adjust
    // We'll use a simple approach: get local time via seconds since epoch
    let local_secs = secs as i64 + local_utc_offset_secs();
    let day_secs = ((local_secs % 86400) + 86400) % 86400;
    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

/// Get local UTC offset in seconds (best-effort)
fn local_utc_offset_secs() -> i64 {
    #[cfg(unix)]
    {
        #[repr(C)]
        struct Tm {
            tm_sec: i32,
            tm_min: i32,
            tm_hour: i32,
            tm_mday: i32,
            tm_mon: i32,
            tm_year: i32,
            tm_wday: i32,
            tm_yday: i32,
            tm_isdst: i32,
            tm_gmtoff: i64,
            tm_zone: *const i8,
        }

        unsafe extern "C" {
            fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
        }

        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        unsafe {
            let mut tm: Tm = std::mem::zeroed();
            localtime_r(&now, &mut tm);
            tm.tm_gmtoff
        }
    }
    #[cfg(not(unix))]
    {
        0 // Fallback to UTC on non-unix
    }
}

/// Truncate a string to max_len characters
fn truncate_str(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len > 3 {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    } else {
        s.chars().take(max_len).collect()
    }
}
