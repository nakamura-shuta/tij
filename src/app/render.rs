//! Rendering logic for the application

use ratatui::{
    Frame,
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use super::state::{App, View};
use crate::keys::{self, BookmarkKind, DialogHintKind, HintContext};
use crate::model::{DiffContent, DiffLineKind};
use crate::ui::components::dialog::DialogKind;
use crate::ui::widgets::{
    render_blame_status_bar, render_diff_status_bar, render_error_banner, render_help_panel,
    render_placeholder, render_status_hints, status_hints_height,
};

impl App {
    /// Render the UI
    pub fn render(&mut self, frame: &mut Frame) {
        // Clone notification to avoid borrow conflict with &mut self in render_log_view
        let notification = self
            .notification
            .as_ref()
            .filter(|n| !n.is_expired())
            .cloned();

        // Render main view (notification is passed to views for title bar display)
        match self.current_view {
            View::Log => self.render_log_view(frame, notification.as_ref()),
            View::Diff => self.render_diff_view(frame, notification.as_ref()),
            View::Status => self.render_status_view(frame, notification.as_ref()),
            View::Operation => self.render_operation_view(frame, notification.as_ref()),
            View::Blame => self.render_blame_view(frame, notification.as_ref()),
            View::Resolve => self.render_resolve_view(frame, notification.as_ref()),
            View::Bookmark => self.render_bookmark_view(frame, notification.as_ref()),
            View::Evolog => self.render_evolog_view(frame, notification.as_ref()),
            View::Help => self.render_help_view(frame),
        }

        // Render error banner above status bar (errors are always shown prominently)
        if let Some(ref error) = self.error_message {
            let status_bar_height = self.get_current_status_bar_height(frame.area().width);
            render_error_banner(frame, error, status_bar_height);
        }

        // Render dialog on top of everything
        if let Some(ref dialog) = self.active_dialog {
            dialog.render(frame, frame.area());
        }
    }

    /// Get the status bar height for the current view
    fn get_current_status_bar_height(&self, width: u16) -> u16 {
        match self.current_view {
            View::Log | View::Status | View::Operation => {
                let ctx = self.build_hint_context();
                let hints = keys::current_hints(self.current_view, self.log_view.input_mode, &ctx);
                status_hints_height(&hints, width)
            }
            View::Bookmark => {
                let ctx = self.build_bookmark_hint_context();
                let hints = keys::current_hints(View::Bookmark, self.log_view.input_mode, &ctx);
                status_hints_height(&hints, width)
            }
            View::Resolve => {
                let ctx = self.build_resolve_hint_context();
                let hints = keys::current_hints(View::Resolve, self.log_view.input_mode, &ctx);
                status_hints_height(&hints, width)
            }
            View::Evolog | View::Diff => 1,
            View::Blame => status_hints_height(keys::BLAME_VIEW_HINTS, width),
            View::Help => 0,
        }
    }

    /// Build HintContext from current App state (Log/Status/Operation views)
    fn build_hint_context(&self) -> HintContext {
        let change = self.log_view.selected_change();
        HintContext {
            has_bookmarks: change.is_some_and(|c| !c.bookmarks.is_empty()),
            has_conflicts: change.is_some_and(|c| c.has_conflict),
            is_working_copy: change.is_some_and(|c| c.is_working_copy),
            skip_emptied: self.log_view.skip_emptied,
            dialog: self.dialog_hint_kind(),
            ..HintContext::default()
        }
    }

    /// Build HintContext for Resolve view (uses resolve_view.is_working_copy)
    fn build_resolve_hint_context(&self) -> HintContext {
        HintContext {
            is_working_copy: self
                .resolve_view
                .as_ref()
                .is_some_and(|rv| rv.is_working_copy),
            dialog: self.dialog_hint_kind(),
            ..HintContext::default()
        }
    }

    /// Convert active dialog to DialogHintKind
    fn dialog_hint_kind(&self) -> Option<DialogHintKind> {
        self.active_dialog.as_ref().map(|d| match &d.kind {
            DialogKind::Confirm { .. } => DialogHintKind::Confirm,
            DialogKind::Select {
                single_select: true,
                ..
            } => DialogHintKind::SingleSelect,
            DialogKind::Select { .. } => DialogHintKind::Select,
        })
    }

    fn render_log_view(
        &mut self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = frame.area();
        let ctx = self.build_hint_context();
        let hints = keys::current_hints(View::Log, self.log_view.input_mode, &ctx);
        let sb_height = status_hints_height(&hints, area.width);

        // Reserve space for status bar at bottom
        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(sb_height),
        };

        // Auto-disable preview for small terminals (does not modify preview_enabled)
        self.preview_auto_disabled = main_area.height < 20;

        let preview_active = self.preview_enabled && !self.preview_auto_disabled;

        if preview_active {
            // Split: log (top 50%) / preview (bottom 50%)
            let chunks = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(main_area);

            self.log_view.render(frame, chunks[0], notification);
            self.render_preview_pane(frame, chunks[1]);
        } else {
            self.log_view.render(frame, main_area, notification);
        }

        render_status_hints(frame, &hints);
    }

    fn render_preview_pane(&self, frame: &mut Frame, area: Rect) {
        // Look up cached entry for the currently selected change
        let selected_change_id = self
            .log_view
            .selected_change()
            .map(|c| c.change_id.as_str());
        let cached = selected_change_id.and_then(|id| self.preview_cache.peek(id));

        let title = match cached {
            Some(entry) => {
                let commit_short = if entry.content.commit_id.len() >= 8 {
                    &entry.content.commit_id[..8]
                } else {
                    &entry.content.commit_id
                };
                format!(" Preview: {} ({}) ", &entry.change_id, commit_short)
            }
            None => " Preview ".to_string(),
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(title).bold().cyan());

        match cached {
            Some(entry) => {
                let inner = block.inner(area);
                let lines = build_preview_lines(
                    &entry.content,
                    &entry.bookmarks,
                    inner.height as usize,
                    inner.width as usize,
                );
                let paragraph = Paragraph::new(lines).block(block);
                frame.render_widget(paragraph, area);
            }
            None => {
                let paragraph = Paragraph::new("  No preview available").block(block);
                frame.render_widget(paragraph, area);
            }
        }
    }

    fn render_diff_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        if let Some(ref diff_view) = self.diff_view {
            let area = frame.area();

            // Reserve space for status bar at bottom
            let main_area = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height.saturating_sub(1),
            };

            // Store visible height for diff content (header=4, context=1)
            // This is used by key handling for accurate scroll bounds
            let diff_content_height = main_area.height.saturating_sub(5);
            self.last_frame_height.set(diff_content_height);

            diff_view.render(frame, main_area, notification);
            render_diff_status_bar(frame, diff_view);
        } else {
            render_placeholder(
                frame,
                " Tij - Diff View ",
                Color::Yellow,
                "No diff loaded - Press q to go back",
            );
        }
    }

    fn render_status_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = frame.area();
        let ctx = self.build_hint_context();
        let hints = keys::current_hints(View::Status, self.log_view.input_mode, &ctx);
        let sb_height = status_hints_height(&hints, area.width);

        // Reserve space for status bar at bottom
        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(sb_height),
        };

        // Store visible height for file list (2 borders + 3 header lines)
        // This is used by key handling for accurate scroll bounds
        let file_list_height = main_area.height.saturating_sub(5);
        self.last_frame_height.set(file_list_height);

        self.status_view.render(frame, main_area, notification);
        render_status_hints(frame, &hints);
    }

    fn render_operation_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = frame.area();
        let ctx = self.build_hint_context();
        let hints = keys::current_hints(View::Operation, self.log_view.input_mode, &ctx);
        let sb_height = status_hints_height(&hints, area.width);

        // Reserve space for status bar at bottom
        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(sb_height),
        };

        self.operation_view.render(frame, main_area, notification);
        render_status_hints(frame, &hints);
    }

    /// Build HintContext for Bookmark View (uses selected bookmark kind)
    fn build_bookmark_hint_context(&self) -> HintContext {
        let kind = self.bookmark_view.selected_bookmark().map(|info| {
            if info.bookmark.remote.is_none() {
                if info.change_id.is_some() {
                    BookmarkKind::LocalJumpable
                } else {
                    BookmarkKind::LocalNoChange
                }
            } else if info.bookmark.is_untracked_remote() {
                BookmarkKind::UntrackedRemote
            } else {
                BookmarkKind::TrackedRemote
            }
        });
        HintContext {
            selected_bookmark_kind: kind,
            dialog: self.dialog_hint_kind(),
            ..HintContext::default()
        }
    }

    fn render_bookmark_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = frame.area();
        let ctx = self.build_bookmark_hint_context();
        let hints = keys::current_hints(View::Bookmark, self.log_view.input_mode, &ctx);
        let sb_height = status_hints_height(&hints, area.width);

        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(sb_height),
        };

        self.bookmark_view.render(frame, main_area, notification);
        render_status_hints(frame, &hints);
    }

    fn render_evolog_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        if let Some(ref evolog_view) = self.evolog_view {
            evolog_view.render(frame, frame.area(), notification);
        } else {
            render_placeholder(
                frame,
                " Tij - Evolution Log ",
                Color::Cyan,
                "No evolution log loaded - Press q to go back",
            );
        }
    }

    fn render_help_view(&self, frame: &mut Frame) {
        let search_query = self.help_search_query.as_deref();
        let search_input = if self.help_search_input {
            Some(self.help_input_buffer.as_str())
        } else {
            None
        };
        render_help_panel(
            frame,
            frame.area(),
            self.help_scroll,
            search_query,
            search_input,
        );
    }

    fn render_resolve_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        if let Some(ref resolve_view) = self.resolve_view {
            let area = frame.area();
            let ctx = self.build_resolve_hint_context();
            let hints = keys::current_hints(View::Resolve, self.log_view.input_mode, &ctx);
            let sb_height = status_hints_height(&hints, area.width);

            // Reserve space for status bar
            let main_area = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height.saturating_sub(sb_height),
            };

            resolve_view.render(frame, main_area, notification);
            render_status_hints(frame, &hints);
        } else {
            render_placeholder(
                frame,
                " Tij - Resolve View ",
                Color::Red,
                "No conflicts loaded - Press q to go back",
            );
        }
    }

    fn render_blame_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        if let Some(ref blame_view) = self.blame_view {
            let area = frame.area();
            let sb_height = status_hints_height(keys::BLAME_VIEW_HINTS, area.width);

            // Reserve space for status bar at bottom
            let main_area = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height.saturating_sub(sb_height),
            };

            // Store visible height for blame content
            let blame_content_height = main_area.height.saturating_sub(2);
            self.last_frame_height.set(blame_content_height);

            blame_view.render(frame, main_area, notification);
            render_blame_status_bar(frame, blame_view);
        } else {
            render_placeholder(
                frame,
                " Tij - Blame View ",
                Color::Yellow,
                "No file loaded - Press q to go back",
            );
        }
    }
}

/// Per-file summary extracted from diff lines (approximate: see SoW phase14-4)
struct FileSummaryEntry {
    path: String,
    op: char, // 'M', 'A', 'D'
    insertions: usize,
    deletions: usize,
}

/// Extract per-file summaries from diff lines.
///
/// Operation type is inferred (approximate):
/// - Added only (no deletions) → 'A'
/// - Deleted only (no additions) → 'D'
/// - Otherwise → 'M'
fn extract_file_summaries(lines: &[crate::model::DiffLine]) -> Vec<FileSummaryEntry> {
    let mut summaries = Vec::new();
    let mut current_path: Option<String> = None;
    let mut insertions = 0usize;
    let mut deletions = 0usize;

    for line in lines {
        match line.kind {
            DiffLineKind::FileHeader => {
                // Flush previous file
                if let Some(path) = current_path.take() {
                    let op = infer_file_op(insertions, deletions);
                    summaries.push(FileSummaryEntry {
                        path,
                        op,
                        insertions,
                        deletions,
                    });
                }
                current_path = Some(line.content.clone());
                insertions = 0;
                deletions = 0;
            }
            DiffLineKind::Added => insertions += 1,
            DiffLineKind::Deleted => deletions += 1,
            _ => {}
        }
    }
    // Flush last file
    if let Some(path) = current_path {
        let op = infer_file_op(insertions, deletions);
        summaries.push(FileSummaryEntry {
            path,
            op,
            insertions,
            deletions,
        });
    }

    summaries
}

/// Infer file operation from line counts (approximate).
fn infer_file_op(insertions: usize, deletions: usize) -> char {
    if deletions == 0 && insertions > 0 {
        'A'
    } else if insertions == 0 && deletions > 0 {
        'D'
    } else {
        'M'
    }
}

/// Build preview lines from DiffContent, limited to max_lines.
///
/// Shows: Author, Bookmarks (if any), Description, file stats summary,
/// then file change list (M/A/D + path + per-file stats).
fn build_preview_lines(
    content: &DiffContent,
    bookmarks: &[String],
    max_lines: usize,
    max_width: usize,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Author + timestamp
    if !content.author.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Author: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}  {}", content.author, content.timestamp)),
        ]));
    }

    // Bookmarks
    if !bookmarks.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Bookmarks: ", Style::default().fg(Color::DarkGray)),
            Span::styled(bookmarks.join(", "), Style::default().fg(Color::Magenta)),
        ]));
    }

    // Description
    if !content.description.is_empty() {
        lines.push(Line::from(Span::styled(
            content.description.clone(),
            Style::default().bold(),
        )));
    }

    // File change statistics (total)
    let summaries = extract_file_summaries(&content.lines);
    let total_files = summaries.len();
    let total_insertions: usize = summaries.iter().map(|s| s.insertions).sum();
    let total_deletions: usize = summaries.iter().map(|s| s.deletions).sum();

    if total_files > 0 {
        let stats_text = format!(
            "{} file{} changed, +{}, -{}",
            total_files,
            if total_files == 1 { "" } else { "s" },
            total_insertions,
            total_deletions,
        );
        lines.push(Line::from(Span::styled(
            stats_text,
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Blank separator
    if !lines.is_empty() {
        lines.push(Line::default());
    }

    // File summary list
    if summaries.is_empty() && content.description.is_empty() && content.author.is_empty() {
        // Truly empty content — no lines at all
        return lines;
    }

    if summaries.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no changes)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let mut remaining = max_lines.saturating_sub(lines.len());

        // If no room but files exist, sacrifice blank separator for overflow indicator
        if remaining == 0 && !lines.is_empty() {
            lines.pop(); // remove blank separator
            remaining = 1;
        }

        let need_overflow = summaries.len() > remaining && remaining > 0;
        let display_count = if need_overflow {
            remaining.saturating_sub(1) // reserve 1 line for "… and N more"
        } else {
            summaries.len().min(remaining)
        };

        for entry in summaries.iter().take(display_count) {
            lines.push(format_file_summary_line(entry, max_width));
        }

        if need_overflow {
            let more = summaries.len() - display_count;
            lines.push(Line::from(Span::styled(
                format!(
                    "… and {} more file{}",
                    more,
                    if more == 1 { "" } else { "s" }
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    lines.truncate(max_lines);
    lines
}

/// Format a single file summary line with path truncation and right-aligned stats.
fn format_file_summary_line(entry: &FileSummaryEntry, max_width: usize) -> Line<'static> {
    let (op_color, op_char) = match entry.op {
        'A' => (Color::Green, 'A'),
        'D' => (Color::Red, 'D'),
        _ => (Color::Yellow, 'M'),
    };

    // Build stats string: "+N -N", "+N", or "-N" (omit zero side)
    let stats = match (entry.insertions, entry.deletions) {
        (0, 0) => String::new(),
        (ins, 0) => format!("+{}", ins),
        (0, del) => format!("-{}", del),
        (ins, del) => format!("+{} -{}", ins, del),
    };

    // Layout: " {op} {path} {pad} {stats}"
    // op_prefix = " M " = 3 chars
    let op_prefix = format!(" {} ", op_char);
    let op_width = 3;

    // If pane is extremely narrow (< 20), skip stats
    let stats_width = if !stats.is_empty() && max_width >= 20 {
        stats.chars().count() + 1 // +1 for leading space
    } else {
        0
    };

    let path_budget = max_width
        .saturating_sub(op_width)
        .saturating_sub(stats_width);

    let display_path = truncate_path(&entry.path, path_budget);
    let display_path_width = display_path.chars().count();

    let mut spans = vec![
        Span::styled(op_prefix, Style::default().fg(op_color)),
        Span::styled(display_path, Style::default().fg(op_color)),
    ];

    if stats_width > 0 {
        // Right-align: pad between path and stats
        let used = op_width + display_path_width + stats_width;
        let pad = max_width.saturating_sub(used);
        let padded_stats = format!("{:>width$}", stats, width = pad + stats.chars().count());
        spans.push(Span::styled(
            padded_stats,
            Style::default().fg(Color::DarkGray),
        ));
    }

    Line::from(spans)
}

/// Truncate a path to fit within budget (char count), using ".." suffix.
fn truncate_path(path: &str, budget: usize) -> String {
    if budget == 0 {
        return String::new();
    }
    let char_count = path.chars().count();
    if char_count <= budget {
        return path.to_string();
    }
    if budget <= 2 {
        return "..".chars().take(budget).collect();
    }
    // Keep first (budget - 2) chars + ".."
    let keep = budget - 2;
    let truncated: String = path.chars().take(keep).collect();
    format!("{}..", truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DiffContent, DiffLine};

    const TEST_WIDTH: usize = 40;

    #[test]
    fn test_build_preview_lines_empty_content() {
        let content = DiffContent::default();
        let lines = build_preview_lines(&content, &[], 10, TEST_WIDTH);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_build_preview_lines_header_only() {
        let content = DiffContent {
            author: "alice@example.com".to_string(),
            timestamp: "2025-01-15 10:30".to_string(),
            description: "Fix login bug".to_string(),
            ..DiffContent::default()
        };
        let lines = build_preview_lines(&content, &[], 10, TEST_WIDTH);
        // Author + description + blank + (no changes) = 4 lines
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn test_build_preview_lines_with_bookmarks() {
        let content = DiffContent {
            author: "alice@example.com".to_string(),
            timestamp: "2025-01-15 10:30".to_string(),
            description: "Fix login bug".to_string(),
            ..DiffContent::default()
        };
        let bookmarks = vec!["main".to_string(), "feature/login".to_string()];
        let lines = build_preview_lines(&content, &bookmarks, 10, TEST_WIDTH);
        // Author + bookmarks + description + blank + (no changes) = 5 lines
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn test_build_preview_lines_file_summary() {
        let content = DiffContent {
            author: "alice@example.com".to_string(),
            timestamp: "2025-01-15".to_string(),
            description: "Add feature".to_string(),
            lines: vec![
                DiffLine::file_header("src/main.rs"),
                DiffLine {
                    kind: DiffLineKind::Added,
                    line_numbers: Some((None, Some(1))),
                    content: "fn main() {}".to_string(),
                },
            ],
            ..DiffContent::default()
        };
        let lines = build_preview_lines(&content, &[], 20, TEST_WIDTH);
        // Author + desc + stats("1 file changed, +1, -0") + blank + "A src/main.rs" = 5
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn test_build_preview_lines_overflow() {
        // Create 10 files, each with 1 added line
        let mut diff_lines = Vec::new();
        for i in 0..10 {
            if i > 0 {
                diff_lines.push(DiffLine::separator());
            }
            diff_lines.push(DiffLine::file_header(format!("file{}.rs", i)));
            diff_lines.push(DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "content".to_string(),
            });
        }
        let content = DiffContent {
            author: "alice".to_string(),
            timestamp: "2025-01-15".to_string(),
            description: "Many files".to_string(),
            lines: diff_lines,
            ..DiffContent::default()
        };
        // max_lines=8: header uses 4 (author + desc + stats + blank), leaving 4 for files
        // 10 files > 4 → show 3 files + "… and 7 more files"
        let lines = build_preview_lines(&content, &[], 8, TEST_WIDTH);
        assert_eq!(lines.len(), 8);
        // Last line should be the overflow indicator
        let last_line_text: String = lines
            .last()
            .unwrap()
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(
            last_line_text.contains("7 more file"),
            "Expected overflow indicator, got: {}",
            last_line_text
        );
    }

    #[test]
    fn test_build_preview_lines_zero_remaining_sacrifices_blank() {
        // When max_lines == header lines, blank separator is sacrificed to show files
        let content = DiffContent {
            author: "alice".to_string(),
            timestamp: "2025-01-15".to_string(),
            description: "Tight".to_string(),
            lines: vec![
                DiffLine::file_header("src/main.rs"),
                DiffLine {
                    kind: DiffLineKind::Added,
                    line_numbers: Some((None, Some(1))),
                    content: "new".to_string(),
                },
            ],
            ..DiffContent::default()
        };
        // max_lines=4: author + desc + stats = 3 header lines, blank = 4th → remaining = 0
        // Fix: blank is sacrificed, file summary shown in its place
        let lines = build_preview_lines(&content, &[], 4, TEST_WIDTH);
        assert_eq!(lines.len(), 4);
        // Last line should be the file summary (not blank, not missing)
        let last_line_text: String = lines
            .last()
            .unwrap()
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(
            last_line_text.contains("src/main.rs"),
            "Expected file summary, got: {}",
            last_line_text
        );
    }

    #[test]
    fn test_build_preview_lines_zero_remaining_overflow() {
        // When max_lines == header lines and multiple files, blank is sacrificed for overflow
        let content = DiffContent {
            author: "alice".to_string(),
            timestamp: "2025-01-15".to_string(),
            description: "Tight".to_string(),
            lines: vec![
                DiffLine::file_header("src/a.rs"),
                DiffLine {
                    kind: DiffLineKind::Added,
                    line_numbers: Some((None, Some(1))),
                    content: "new".to_string(),
                },
                DiffLine::separator(),
                DiffLine::file_header("src/b.rs"),
                DiffLine {
                    kind: DiffLineKind::Added,
                    line_numbers: Some((None, Some(1))),
                    content: "new".to_string(),
                },
            ],
            ..DiffContent::default()
        };
        // max_lines=4: header=3, blank=4th → remaining=0 → sacrifice blank → remaining=1
        // 2 files > 1 remaining → overflow: 0 files shown + "… and 2 more files"
        let lines = build_preview_lines(&content, &[], 4, TEST_WIDTH);
        assert_eq!(lines.len(), 4);
        let last_line_text: String = lines
            .last()
            .unwrap()
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(
            last_line_text.contains("2 more file"),
            "Expected overflow indicator, got: {}",
            last_line_text
        );
    }

    #[test]
    fn test_build_preview_lines_no_changes() {
        let content = DiffContent {
            author: "alice".to_string(),
            timestamp: "2025-01-15".to_string(),
            description: "Empty commit".to_string(),
            ..DiffContent::default()
        };
        let lines = build_preview_lines(&content, &[], 10, TEST_WIDTH);
        // Author + desc + blank + "(no changes)" = 4
        assert_eq!(lines.len(), 4);
        let last_line_text: String = lines
            .last()
            .unwrap()
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(
            last_line_text.contains("no changes"),
            "Expected '(no changes)', got: {}",
            last_line_text
        );
    }

    #[test]
    fn test_build_preview_lines_truncated() {
        // Create 20 files to ensure truncation
        let mut diff_lines = Vec::new();
        for i in 0..20 {
            if i > 0 {
                diff_lines.push(DiffLine::separator());
            }
            diff_lines.push(DiffLine::file_header(format!("file{}.rs", i)));
            diff_lines.push(DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "line".to_string(),
            });
        }
        let content = DiffContent {
            author: "alice@example.com".to_string(),
            timestamp: "2025-01-15".to_string(),
            description: "Long diff".to_string(),
            lines: diff_lines,
            ..DiffContent::default()
        };
        // Max 5 lines total
        let lines = build_preview_lines(&content, &[], 5, TEST_WIDTH);
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn test_extract_file_summaries_basic() {
        let lines = vec![
            // File 1: Modified (has both added and deleted)
            DiffLine::file_header("src/main.rs"),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "new".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: Some((Some(1), None)),
                content: "old".to_string(),
            },
            DiffLine::separator(),
            // File 2: Added (only added lines)
            DiffLine::file_header("src/new.rs"),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "fn new()".to_string(),
            },
            DiffLine::separator(),
            // File 3: Deleted (only deleted lines)
            DiffLine::file_header("src/old.rs"),
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: Some((Some(1), None)),
                content: "fn old()".to_string(),
            },
        ];

        let summaries = extract_file_summaries(&lines);
        assert_eq!(summaries.len(), 3);

        assert_eq!(summaries[0].path, "src/main.rs");
        assert_eq!(summaries[0].op, 'M');
        assert_eq!(summaries[0].insertions, 1);
        assert_eq!(summaries[0].deletions, 1);

        assert_eq!(summaries[1].path, "src/new.rs");
        assert_eq!(summaries[1].op, 'A');
        assert_eq!(summaries[1].insertions, 1);
        assert_eq!(summaries[1].deletions, 0);

        assert_eq!(summaries[2].path, "src/old.rs");
        assert_eq!(summaries[2].op, 'D');
        assert_eq!(summaries[2].insertions, 0);
        assert_eq!(summaries[2].deletions, 1);
    }

    #[test]
    fn test_extract_file_summaries_empty() {
        let summaries = extract_file_summaries(&[]);
        assert!(summaries.is_empty());
    }

    #[test]
    fn test_truncate_path_fits() {
        assert_eq!(truncate_path("src/main.rs", 20), "src/main.rs");
    }

    #[test]
    fn test_truncate_path_truncated() {
        assert_eq!(
            truncate_path("src/very/long/path/to/file.rs", 15),
            "src/very/long.."
        );
    }

    #[test]
    fn test_truncate_path_budget_zero() {
        assert_eq!(truncate_path("src/main.rs", 0), "");
    }

    #[test]
    fn test_truncate_path_budget_two() {
        assert_eq!(truncate_path("src/main.rs", 2), "..");
    }

    #[test]
    fn test_infer_file_op() {
        assert_eq!(infer_file_op(5, 0), 'A');
        assert_eq!(infer_file_op(0, 3), 'D');
        assert_eq!(infer_file_op(3, 2), 'M');
        assert_eq!(infer_file_op(0, 0), 'M'); // empty file → M (fallback)
    }

    #[test]
    fn test_extract_file_summaries_totals() {
        let lines = vec![
            DiffLine::file_header("src/main.rs"),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "new line".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(2))),
                content: "another new".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: Some((Some(1), None)),
                content: "old line".to_string(),
            },
            DiffLine::separator(),
            DiffLine::file_header("src/lib.rs"),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "pub fn hello()".to_string(),
            },
        ];
        let summaries = extract_file_summaries(&lines);
        assert_eq!(summaries.len(), 2);
        let total_ins: usize = summaries.iter().map(|s| s.insertions).sum();
        let total_del: usize = summaries.iter().map(|s| s.deletions).sum();
        assert_eq!(total_ins, 3);
        assert_eq!(total_del, 1);
    }

    /// Verify that preview cache validate evicts stale entries and keeps valid ones.
    #[test]
    fn test_preview_cache_validated_on_refresh_log() {
        use crate::app::state::{PreviewCache, PreviewCacheEntry};
        use crate::model::Change;

        let mut cache = PreviewCache::new();
        cache.insert(PreviewCacheEntry {
            change_id: "abc12345".to_string(),
            commit_id: "commit_aaa".to_string(),
            content: DiffContent {
                author: "alice@example.com".to_string(),
                description: "Old description".to_string(),
                ..DiffContent::default()
            },
            bookmarks: vec!["main".to_string()],
        });

        // Simulate refresh_log with same commit_id → entry kept
        let changes = vec![Change {
            change_id: "abc12345".to_string(),
            commit_id: "commit_aaa".to_string(),
            bookmarks: vec!["main".to_string(), "dev".to_string()],
            ..Change::default()
        }];
        cache.validate(&changes);
        assert_eq!(cache.len(), 1);
        // Bookmarks should be updated
        let entry = cache.peek("abc12345").unwrap();
        assert_eq!(entry.bookmarks, vec!["main".to_string(), "dev".to_string()]);

        // Now commit_id changes → entry evicted
        let changes_stale = vec![Change {
            change_id: "abc12345".to_string(),
            commit_id: "commit_bbb".to_string(),
            ..Change::default()
        }];
        cache.validate(&changes_stale);
        assert_eq!(cache.len(), 0);
    }
}
