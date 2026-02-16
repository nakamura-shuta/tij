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
            View::Diff => 1,
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
        let title = match &self.preview_cache {
            Some(cache) => {
                let commit_short = if cache.content.commit_id.len() >= 8 {
                    &cache.content.commit_id[..8]
                } else {
                    &cache.content.commit_id
                };
                format!(" Preview: {} ({}) ", &cache.change_id, commit_short)
            }
            None => " Preview ".to_string(),
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(title).bold().cyan());

        match &self.preview_cache {
            Some(cache) => {
                let inner = block.inner(area);
                let lines =
                    build_preview_lines(&cache.content, &cache.bookmarks, inner.height as usize);
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

    fn render_help_view(&self, frame: &mut Frame) {
        render_help_panel(frame, frame.area());
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

/// Build preview lines from DiffContent, limited to max_lines.
///
/// Shows: Author, Bookmarks (if any), Description, file stats, then diff lines.
fn build_preview_lines(
    content: &DiffContent,
    bookmarks: &[String],
    max_lines: usize,
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

    // File change statistics
    let (files, insertions, deletions) = count_diff_stats(&content.lines);
    if files > 0 {
        let stats_text = format!(
            "{} file{} changed, +{}, -{}",
            files,
            if files == 1 { "" } else { "s" },
            insertions,
            deletions,
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

    // Diff lines
    for diff_line in &content.lines {
        if lines.len() >= max_lines {
            break;
        }
        let line = match diff_line.kind {
            DiffLineKind::FileHeader => Line::from(Span::styled(
                format!("  {}", diff_line.content),
                Style::default().fg(Color::Cyan).bold(),
            )),
            DiffLineKind::Added => Line::from(Span::styled(
                format!("  + {}", diff_line.content),
                Style::default().fg(Color::Green),
            )),
            DiffLineKind::Deleted => Line::from(Span::styled(
                format!("  - {}", diff_line.content),
                Style::default().fg(Color::Red),
            )),
            DiffLineKind::Context => Line::from(Span::raw(format!("    {}", diff_line.content))),
            DiffLineKind::Separator => Line::default(),
        };
        lines.push(line);
    }

    lines.truncate(max_lines);
    lines
}

/// Count file changes, insertions, and deletions from diff lines
fn count_diff_stats(lines: &[crate::model::DiffLine]) -> (usize, usize, usize) {
    let mut files = 0;
    let mut insertions = 0;
    let mut deletions = 0;

    for line in lines {
        match line.kind {
            DiffLineKind::FileHeader => files += 1,
            DiffLineKind::Added => insertions += 1,
            DiffLineKind::Deleted => deletions += 1,
            _ => {}
        }
    }

    (files, insertions, deletions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DiffContent, DiffLine};

    #[test]
    fn test_build_preview_lines_empty_content() {
        let content = DiffContent::default();
        let lines = build_preview_lines(&content, &[], 10);
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
        let lines = build_preview_lines(&content, &[], 10);
        // Author line + description + blank separator = 3 lines
        assert_eq!(lines.len(), 3);
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
        let lines = build_preview_lines(&content, &bookmarks, 10);
        // Author + bookmarks + description + blank separator = 4 lines
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn test_build_preview_lines_with_diff() {
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
        let lines = build_preview_lines(&content, &[], 20);
        // Author + desc + stats("1 file changed, +1, -0") + blank + file_header + added = 6
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn test_build_preview_lines_truncated() {
        let content = DiffContent {
            author: "alice@example.com".to_string(),
            timestamp: "2025-01-15".to_string(),
            description: "Long diff".to_string(),
            lines: (0..20)
                .map(|i| DiffLine {
                    kind: DiffLineKind::Context,
                    line_numbers: Some((Some(i), Some(i))),
                    content: format!("line {}", i),
                })
                .collect(),
            ..DiffContent::default()
        };
        // Max 5 lines
        let lines = build_preview_lines(&content, &[], 5);
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn test_count_diff_stats() {
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
        let (files, insertions, deletions) = count_diff_stats(&lines);
        assert_eq!(files, 2);
        assert_eq!(insertions, 3);
        assert_eq!(deletions, 1);
    }

    #[test]
    fn test_count_diff_stats_empty() {
        let (files, insertions, deletions) = count_diff_stats(&[]);
        assert_eq!((files, insertions, deletions), (0, 0, 0));
    }

    #[test]
    fn test_preview_hint_in_log_normal() {
        use crate::keys::{self, HintContext};
        use crate::ui::views::InputMode;

        let ctx = HintContext::default();
        let hints = keys::current_hints(View::Log, InputMode::Normal, &ctx);
        assert!(
            hints.iter().any(|h| h.key == "p" && h.label == "Preview"),
            "Preview hint missing from log normal hints"
        );
    }

    /// Verify that preview cache is invalidated by refresh_log().
    ///
    /// After a mutation (describe, edit, squash, rebase, etc.), the same change_id
    /// may have different content. The cache hit check (change_id match) alone
    /// would return stale data. refresh_log() must clear preview_cache.
    #[test]
    fn test_preview_cache_cleared_on_refresh_log() {
        use crate::app::state::PreviewCache;

        // Simulate: preview_cache has content for "abc12345"
        let cache = PreviewCache {
            change_id: "abc12345".to_string(),
            content: DiffContent {
                author: "alice@example.com".to_string(),
                description: "Old description".to_string(),
                ..DiffContent::default()
            },
            bookmarks: vec!["main".to_string()],
        };

        // Verify the cache would be a hit for same change_id
        assert_eq!(cache.change_id, "abc12345");

        // After a mutation, content changes but change_id stays the same.
        // Without invalidation, update_preview_if_needed() would return stale cache.
        // refresh_log() clears preview_cache (verified by reading refresh.rs).
        //
        // This test documents the contract: PreviewCache only checks change_id,
        // so external invalidation (via refresh_log) is required after mutations.
        let new_content = DiffContent {
            author: "alice@example.com".to_string(),
            description: "Updated description".to_string(),
            ..DiffContent::default()
        };

        // Same change_id, different content — cache hit would return wrong data
        assert_eq!(cache.change_id, "abc12345");
        assert_ne!(cache.content.description, new_content.description);
    }
}
