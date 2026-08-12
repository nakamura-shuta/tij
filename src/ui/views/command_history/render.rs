//! Command History View rendering

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::CommandHistoryView;
use crate::model::{CommandHistory, CommandKind, CommandRecord, CommandStatus, Notification};
use crate::ui::{components, navigation, theme};

/// Maximum number of error lines to show in detail view
const MAX_ERROR_LINES: usize = 5;

impl CommandHistoryView {
    /// Render the command history view
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        history: &CommandHistory,
        notification: Option<&Notification>,
    ) {
        // The filtered `visible` set is the single source of truth for
        // everything below (selection, detail, rows).
        self.sync(history);
        let count = self.visible_len();
        let title = Line::from(format!(
            " Command History [{}] ({}) ",
            self.filter().label(),
            count
        ))
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

        if count == 0 {
            let msg = if history.is_empty() {
                "No commands recorded yet"
            } else {
                "No commands match this filter — [f] to cycle"
            };
            let paragraph = Paragraph::new(msg).block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize;
        if inner_height == 0 {
            return;
        }

        // Visible (filtered) records, in history order
        let records: Vec<&CommandRecord> = (0..count)
            .filter_map(|pos| self.raw_index(pos))
            .filter_map(|raw| history.records().get(raw))
            .collect();
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
/// `  HH:MM:SS  OK  [W]  Operation     jj command args... ×N`
fn build_record_line(record: &CommandRecord, is_selected: bool, _width: usize) -> Line<'static> {
    // Time column
    let time_str = format_timestamp(&record.timestamp);

    // Status column
    let (status_str, status_color) = match record.status {
        CommandStatus::Success => ("OK", Color::Green),
        CommandStatus::Failed => ("NG", Color::Red),
    };

    // Kind column: [R]ead / [W]rite / [I]nteractive
    let kind_color = match record.kind {
        CommandKind::Read => Color::DarkGray,
        CommandKind::Write => Color::Yellow,
        CommandKind::Interactive => Color::Magenta,
    };

    // Operation column (14 chars, cyan)
    let op = crate::ui::text::fit_display_width(&record.operation, 14);

    // Command column: "jj " + args (long -T templates elided in list rows;
    // the Enter detail shows the full argv)
    let cmd = format!("jj {}", crate::model::display_args(&record.args));

    // ×N right after the operation (NOT at the line end — collapsed reads
    // carry long template args that run past the right edge, which would
    // hide a trailing repeat count exactly where it matters most)
    let repeat = if record.repeat > 1 {
        format!("×{:<3}", record.repeat)
    } else {
        "    ".to_string()
    };

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
        Span::styled(
            format!("[{}]", record.kind.tag()),
            Style::default().fg(kind_color),
        ),
        Span::raw("  "),
        Span::styled(op, Style::default().fg(Color::Cyan)),
        Span::raw(" "),
        Span::styled(repeat, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    /// jj's real two-line answer when a tag would create an untracked remote
    /// ref: the second line is the actionable half.
    const MULTILINE_STDERR: &str = "Error: Refusing to create new remote tag v1.0@other\n\
                                    Hint: Run `jj tag track v1.0@other` and try again.";

    fn failed_record(error: Option<&str>) -> CommandRecord {
        CommandRecord {
            operation: "Tag set".to_string(),
            args: vec![
                "--color=never".to_string(),
                "tag".to_string(),
                "set".to_string(),
                "v1.0".to_string(),
            ],
            kind: CommandKind::Write,
            repeat: 1,
            timestamp: SystemTime::UNIX_EPOCH,
            duration_ms: 12,
            status: CommandStatus::Failed,
            error: error.map(str::to_string),
        }
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    /// The detail view is the only place a jj hint is reachable (the error
    /// banner is one row tall), so every stderr line must be rendered.
    #[test]
    fn detail_lines_show_the_hint_line() {
        let record = failed_record(Some(MULTILINE_STDERR));
        let rendered: Vec<String> = build_detail_lines(&record, 100)
            .iter()
            .map(line_text)
            .collect();

        assert!(
            rendered
                .iter()
                .any(|l| l.contains("Error: Refusing to create new remote tag v1.0@other")),
            "{rendered:#?}"
        );
        assert!(
            rendered
                .iter()
                .any(|l| l.contains("Hint: Run `jj tag track v1.0@other` and try again.")),
            "the Hint line must be visible in the detail view: {rendered:#?}"
        );
        // Continuation lines are indented under the "Error: " label rather
        // than repeating it.
        let hint = rendered
            .iter()
            .find(|l| l.contains("Hint: Run"))
            .expect("hint line");
        assert!(
            !hint.contains("Error: "),
            "hint row keeps its own text: {hint:?}"
        );
    }

    /// `detail_line_count` drives the scroll reservation; if it disagrees with
    /// what `build_detail_lines` produces, expanding a multi-line error scrolls
    /// the hint off screen.
    #[test]
    fn detail_line_count_matches_built_lines() {
        for error in [
            None,
            Some(""),
            Some("Error: single"),
            Some(MULTILINE_STDERR),
            Some("l1\nl2\nl3\nl4\nl5\nl6\nl7"), // over MAX_ERROR_LINES
        ] {
            let record = failed_record(error);
            assert_eq!(
                detail_line_count(&record, 100),
                build_detail_lines(&record, 100).len(),
                "mismatch for {error:?}"
            );
        }
    }

    #[test]
    fn detail_lines_cap_at_max_error_lines_with_a_more_marker() {
        let record = failed_record(Some("l1\nl2\nl3\nl4\nl5\nl6\nl7"));
        let rendered: Vec<String> = build_detail_lines(&record, 100)
            .iter()
            .map(line_text)
            .collect();
        assert!(rendered.iter().any(|l| l.contains("l5")));
        assert!(!rendered.iter().any(|l| l.contains("l6")));
        assert!(rendered.iter().any(|l| l.contains("... (2 more lines)")));
    }

    /// The one-line list row is a summary: it never grows with the error, and
    /// a multi-line stderr must not leak a newline into it.
    #[test]
    fn record_row_stays_a_single_summary_line() {
        let with_error = build_record_line(&failed_record(Some(MULTILINE_STDERR)), false, 100);
        let without = build_record_line(&failed_record(None), false, 100);

        let text = line_text(&with_error);
        assert!(!text.contains('\n'), "row must stay one line: {text:?}");
        assert!(!text.contains("Hint:"), "row shows no stderr: {text:?}");
        assert_eq!(
            text,
            line_text(&without),
            "the row is unaffected by the recorded stderr"
        );
        assert!(text.contains("NG"), "failure is still flagged: {text:?}");
    }
}
