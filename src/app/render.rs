//! Rendering logic for the application

use ratatui::{
    Frame,
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use super::state::{App, View};
use crate::app::helpers::revision::short_id;
use crate::keys::{self, BookmarkKind, DialogHintKind, HintContext};
use crate::model::{DiffContent, DiffLineKind, FileOperation};
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
            View::Tag => self.render_tag_view(frame, notification.as_ref()),
            View::Workspace => self.render_workspace_view(frame, notification.as_ref()),
            View::Evolog => self.render_evolog_view(frame, notification.as_ref()),
            View::CommandHistory => self.render_command_history_view(frame, notification.as_ref()),
            View::TraceDetail => self.render_trace_detail_view(frame, notification.as_ref()),
            View::Help => self.render_help_view(frame),
        }

        // Command echo bar (transparency P2): the last jj command, on the
        // row between the view content and the status bar. Views received a
        // one-row-shorter area via `view_area`, so this row is free.
        if self.command_echo_enabled {
            self.render_command_echo(frame);
        }

        // Render error banner above status bar (errors are always shown prominently)
        if let Some(ref error) = self.error_message {
            let status_bar_height = self.get_current_status_bar_height(frame.area().width);
            // With the echo bar on, the banner sits above it, not over it.
            let echo = if self.command_echo_enabled { 1 } else { 0 };
            render_error_banner(frame, error, status_bar_height + echo);
        }

        // Render dialog on top of everything
        if let Some(ref dialog) = self.active_dialog {
            dialog.render(frame, frame.area());
        }

        // Render command palette overlay (Phase 46-C), above all else
        if self.palette_active {
            crate::ui::components::render_palette(
                frame,
                &self.palette_input,
                self.palette_selected,
            );
        }
    }

    /// The area a view may draw into: the full frame, minus one row when the
    /// command echo bar is enabled. The single place that decides how much
    /// screen the views get — views must use this instead of `frame.area()`.
    fn view_area(&self, frame: &Frame) -> Rect {
        let full = frame.area();
        if self.command_echo_enabled {
            Rect {
                height: full.height.saturating_sub(1),
                ..full
            }
        } else {
            full
        }
    }

    /// Area for unloaded-view placeholders: `view_area` minus the rows the
    /// view's status bar occupies. Placeholder routes don't draw a status
    /// bar, but they must reserve the same bottom rows as the loaded view so
    /// the content / echo / status-bar geometry holds uniformly — otherwise
    /// the echo row would paint over the placeholder's bottom border.
    fn placeholder_area(&self, frame: &Frame) -> Rect {
        let area = self.view_area(frame);
        let sb = self.get_current_status_bar_height(area.width);
        Rect {
            height: area.height.saturating_sub(sb),
            ..area
        }
    }

    /// Draw the last executed jj command on the row directly above the
    /// status bar (the row `view_area` freed up).
    fn render_command_echo(&self, frame: &mut Frame) {
        use ratatui::style::{Color, Style};
        use ratatui::text::Span;
        use ratatui::widgets::Paragraph;

        let full = frame.area();
        let sb_height = self.get_current_status_bar_height(full.width);
        if full.height <= sb_height + 1 {
            return; // terminal too small for the echo row
        }
        let echo_area = Rect {
            x: full.x,
            y: full.y + full.height - sb_height - 1,
            width: full.width,
            height: 1,
        };

        let text = match self.command_echo_last.as_ref() {
            Some(record) => {
                let repeat = if record.repeat > 1 {
                    format!(" ×{}", record.repeat)
                } else {
                    String::new()
                };
                format!(
                    " jj {} ({}ms){}",
                    crate::model::display_args(&record.args),
                    record.duration_ms,
                    repeat
                )
            }
            None => " (no jj commands yet)".to_string(),
        };
        let line = Paragraph::new(Span::styled(text, Style::default().fg(Color::DarkGray)));
        frame.render_widget(line, echo_area);
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
            View::Tag | View::Workspace => {
                let ctx = keys::HintContext::default();
                let hints = keys::current_hints(self.current_view, self.log_view.input_mode, &ctx);
                status_hints_height(&hints, width)
            }
            View::Resolve => {
                let ctx = self.build_resolve_hint_context();
                let hints = keys::current_hints(View::Resolve, self.log_view.input_mode, &ctx);
                status_hints_height(&hints, width)
            }
            View::CommandHistory => {
                let ctx = keys::HintContext::default();
                let hints =
                    keys::current_hints(View::CommandHistory, self.log_view.input_mode, &ctx);
                status_hints_height(&hints, width)
            }
            View::Evolog => status_hints_height(keys::EVOLOG_VIEW_HINTS, width),
            View::Diff => self
                .diff_view
                .as_ref()
                .map_or(1, |dv| crate::ui::widgets::diff_status_height(dv, width)),
            View::Blame => self
                .blame_view
                .as_ref()
                .map_or(1, |bv| crate::ui::widgets::blame_status_height(bv, width)),
            View::TraceDetail => status_hints_height(self.trace_detail_hints(), width),
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
            simplify_parents: self.log_view.simplify_parents,
            rebase_mode: self.log_view.rebase_mode,
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
            DialogKind::Input { .. } => DialogHintKind::Confirm,
        })
    }

    fn render_log_view(
        &mut self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = self.view_area(frame);
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
                let commit_short = short_id(entry.content.commit_id.as_str());
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
        &mut self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = self.view_area(frame);
        if let Some(ref mut diff_view) = self.diff_view {
            // Reserve space for the status bar at the bottom. The diff/blame
            // bars are prefix-aware and may wrap to multiple rows, so reserve
            // the actual height (not a hardcoded 1) — otherwise the content's
            // bottom border overlaps the wrapped status bar.
            let sb_height = crate::ui::widgets::diff_status_height(diff_view, area.width);
            let main_area = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height.saturating_sub(sb_height),
            };

            // Mirror DiffView's layout so scroll bounds stay accurate as the
            // header grows/shrinks with description length or the expand
            // toggle. Re-clamp scroll_offset *before* render so a previously
            // valid offset (e.g. end-of-file while expanded) doesn't leave
            // blank rows at the bottom after collapsing.
            let diff_content_height = diff_view.diff_content_height(main_area.height);
            diff_view.set_visible_height_and_clamp(diff_content_height as usize);
            self.last_frame_height.set(diff_content_height);

            diff_view.render(frame, main_area, notification);
            render_diff_status_bar(frame, diff_view);
        } else {
            render_placeholder(
                frame,
                self.placeholder_area(frame),
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
        let area = self.view_area(frame);
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

        // Store visible height for the file list, used by key handling for
        // scroll bounds. Comes from the view (conflict-line aware) so the
        // bounds match what's actually rendered.
        let file_list_height = self.status_view.file_list_height(main_area.height);
        self.last_frame_height.set(file_list_height);

        self.status_view.render(frame, main_area, notification);
        render_status_hints(frame, &hints);
    }

    fn render_operation_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = self.view_area(frame);
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
        let area = self.view_area(frame);
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

    fn render_tag_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = self.view_area(frame);
        let ctx = keys::HintContext::default();
        let hints = keys::current_hints(View::Tag, self.log_view.input_mode, &ctx);
        let sb_height = status_hints_height(&hints, area.width);

        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(sb_height),
        };

        self.tag_view.render(frame, main_area, notification);
        render_status_hints(frame, &hints);
    }

    fn render_workspace_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = self.view_area(frame);
        let ctx = keys::HintContext::default();
        let hints = keys::current_hints(View::Workspace, self.log_view.input_mode, &ctx);
        let sb_height = status_hints_height(&hints, area.width);

        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(sb_height),
        };

        self.workspace_view.render(frame, main_area, notification);
        render_status_hints(frame, &hints);
    }

    fn render_evolog_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        if let Some(ref evolog_view) = self.evolog_view {
            // Reserve a status bar row at the bottom (Phase 46-D follow-up).
            let area = self.view_area(frame);
            let sb_height = status_hints_height(keys::EVOLOG_VIEW_HINTS, area.width);
            let main_area = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height.saturating_sub(sb_height),
            };
            evolog_view.render(frame, main_area, notification);
            render_status_hints(frame, keys::EVOLOG_VIEW_HINTS);
        } else {
            render_placeholder(
                frame,
                self.placeholder_area(frame),
                " Tij - Evolution Log ",
                Color::Cyan,
                "No evolution log loaded - Press q to go back",
            );
        }
    }

    fn render_command_history_view(
        &mut self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = self.view_area(frame);
        let ctx = keys::HintContext::default();
        let hints = keys::current_hints(View::CommandHistory, self.log_view.input_mode, &ctx);
        let sb_height = status_hints_height(&hints, area.width);

        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(sb_height),
        };

        // Split-borrow: the view (mut, re-syncs its filter) and the record
        // store (shared) are different fields of App.
        let Self {
            command_history_view,
            command_history,
            ..
        } = self;
        command_history_view.render(frame, main_area, command_history, notification);
        render_status_hints(frame, &hints);
    }

    /// Status bar hints for the Trace Detail View — drops `[y] Copy URL` when
    /// there is nothing to copy (G4: only show working actions).
    pub(crate) fn trace_detail_hints(&self) -> &'static [keys::KeyHint] {
        match &self.trace_detail_view {
            Some(v) if v.has_urls() => keys::TRACE_DETAIL_VIEW_HINTS,
            _ => keys::TRACE_DETAIL_VIEW_HINTS_NO_URL,
        }
    }

    fn render_trace_detail_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = self.view_area(frame);
        let hints = self.trace_detail_hints();
        let sb_height = status_hints_height(hints, area.width);
        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(sb_height),
        };

        if let Some(ref view) = self.trace_detail_view {
            view.render(frame, main_area, notification);
        } else {
            render_placeholder(
                frame,
                self.placeholder_area(frame),
                " Tij - Agent Traces ",
                Color::Magenta,
                "No traces loaded - Press q to go back",
            );
        }
        render_status_hints(frame, hints);
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
            self.view_area(frame),
            self.help_scroll,
            search_query,
            search_input,
            self.previous_view(),
            self.help_show_all,
        );
    }

    fn render_resolve_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        if let Some(ref resolve_view) = self.resolve_view {
            let area = self.view_area(frame);
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
                self.placeholder_area(frame),
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
            let area = self.view_area(frame);
            // Prefix-aware (file path can push the bar to wrap) — mirrors the
            // height render_blame_status_bar actually uses.
            let sb_height = crate::ui::widgets::blame_status_height(blame_view, area.width);

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
                self.placeholder_area(frame),
                " Tij - Blame View ",
                Color::Yellow,
                "No file loaded - Press q to go back",
            );
        }
    }
}

/// Per-file summary extracted from diff lines
struct FileSummaryEntry {
    path: String,
    op: FileOperation,
    insertions: usize,
    deletions: usize,
}

/// Extract per-file summaries from diff lines.
///
/// Uses `file_op` from the DiffLine if available (from `parse_show` / `parse_diff_body`).
/// Falls back to `infer_file_op` heuristic when `file_op` is None (git format / stat format).
fn extract_file_summaries(lines: &[crate::model::DiffLine]) -> Vec<FileSummaryEntry> {
    let mut summaries = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_file_op: Option<FileOperation> = None;
    let mut insertions = 0usize;
    let mut deletions = 0usize;

    for line in lines {
        match line.kind {
            DiffLineKind::FileHeader => {
                // Flush previous file
                if let Some(path) = current_path.take() {
                    let op =
                        current_file_op.unwrap_or_else(|| infer_file_op(insertions, deletions));
                    summaries.push(FileSummaryEntry {
                        path,
                        op,
                        insertions,
                        deletions,
                    });
                }
                current_path = Some(line.content.clone());
                current_file_op = line.file_op;
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
        let op = current_file_op.unwrap_or_else(|| infer_file_op(insertions, deletions));
        summaries.push(FileSummaryEntry {
            path,
            op,
            insertions,
            deletions,
        });
    }

    summaries
}

/// Infer file operation from line counts (fallback heuristic).
///
/// Only used when `file_op` is not available on the DiffLine (e.g. git format
/// parsed via `parse_git_diff_lines`). This heuristic can misclassify
/// modifications that have only additions or only deletions.
fn infer_file_op(insertions: usize, deletions: usize) -> FileOperation {
    if deletions == 0 && insertions > 0 {
        FileOperation::Added
    } else if insertions == 0 && deletions > 0 {
        FileOperation::Deleted
    } else {
        FileOperation::Modified
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
        FileOperation::Added => (Color::Green, 'A'),
        FileOperation::Deleted => (Color::Red, 'D'),
        FileOperation::Modified => (Color::Yellow, 'M'),
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

    /// All text on the TestBackend buffer as one string.
    fn buffer_text(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn command_echo_bar_shows_last_command_when_enabled() {
        use crate::model::{CommandKind, CommandRecord, CommandStatus};

        let mut app = App::new_for_test();
        app.command_echo_last = Some(CommandRecord {
            operation: "log (read)".to_string(),
            args: vec!["--color=never".to_string(), "log".to_string()],
            kind: CommandKind::Read,
            repeat: 2,
            timestamp: std::time::SystemTime::UNIX_EPOCH,
            duration_ms: 12,
            status: CommandStatus::Success,
            error: None,
        });

        let backend = ratatui::backend::TestBackend::new(80, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        // Off by default: no echo line.
        terminal.draw(|f| app.render(f)).unwrap();
        assert!(!buffer_text(&terminal).contains("jj --color=never log (12ms)"));

        // On: the last command appears with duration and repeat count.
        app.command_echo_enabled = true;
        terminal.draw(|f| app.render(f)).unwrap();
        let text = buffer_text(&terminal);
        assert!(
            text.contains("jj --color=never log (12ms) ×2"),
            "echo line missing: {text}"
        );
    }

    #[test]
    fn placeholder_route_respects_echo_row() {
        // Unloaded-view routes (Diff with no diff_view, etc.) draw a
        // placeholder. It must use `view_area` like loaded views, so the
        // echo row stays its own row instead of being painted over the
        // placeholder frame.
        let mut app = App::new_for_test();
        app.current_view = View::Diff; // diff_view is None → placeholder route
        app.command_echo_enabled = true;

        let backend = ratatui::backend::TestBackend::new(60, 12);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();

        let buf = terminal.backend().buffer();
        // Echo row = bottom row minus the Diff placeholder's status height (1).
        let echo_y = 12 - 1 - 1;
        let echo_row: String = (0..60).map(|x| buf[(x, echo_y)].symbol()).collect();
        assert!(
            echo_row.contains("(no jj commands yet)"),
            "echo row not rendered on placeholder route: {echo_row:?}"
        );
        // The placeholder's bottom border closes ABOVE the echo row, not on it.
        let border_y = echo_y - 1;
        let border_row: String = (0..60).map(|x| buf[(x, border_y)].symbol()).collect();
        assert!(
            border_row.starts_with('└'),
            "placeholder frame must close above the echo row: {border_row:?}"
        );
    }

    #[test]
    fn view_area_reserves_one_row_only_when_echo_enabled() {
        let mut app = App::new_for_test();
        let backend = ratatui::backend::TestBackend::new(80, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                assert_eq!(app.view_area(f).height, 20, "echo off → full frame");
                app.command_echo_enabled = true;
                assert_eq!(app.view_area(f).height, 19, "echo on → one row reserved");
            })
            .unwrap();
    }

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
                    file_op: None,
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
                file_op: None,
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
                    file_op: None,
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
                    file_op: None,
                },
                DiffLine::separator(),
                DiffLine::file_header("src/b.rs"),
                DiffLine {
                    kind: DiffLineKind::Added,
                    line_numbers: Some((None, Some(1))),
                    content: "new".to_string(),
                    file_op: None,
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
                file_op: None,
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
                file_op: None,
            },
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: Some((Some(1), None)),
                content: "old".to_string(),
                file_op: None,
            },
            DiffLine::separator(),
            // File 2: Added (only added lines)
            DiffLine::file_header("src/new.rs"),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "fn new()".to_string(),
                file_op: None,
            },
            DiffLine::separator(),
            // File 3: Deleted (only deleted lines)
            DiffLine::file_header("src/old.rs"),
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: Some((Some(1), None)),
                content: "fn old()".to_string(),
                file_op: None,
            },
        ];

        let summaries = extract_file_summaries(&lines);
        assert_eq!(summaries.len(), 3);

        assert_eq!(summaries[0].path, "src/main.rs");
        assert_eq!(summaries[0].op, FileOperation::Modified);
        assert_eq!(summaries[0].insertions, 1);
        assert_eq!(summaries[0].deletions, 1);

        assert_eq!(summaries[1].path, "src/new.rs");
        assert_eq!(summaries[1].op, FileOperation::Added);
        assert_eq!(summaries[1].insertions, 1);
        assert_eq!(summaries[1].deletions, 0);

        assert_eq!(summaries[2].path, "src/old.rs");
        assert_eq!(summaries[2].op, FileOperation::Deleted);
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
        assert_eq!(infer_file_op(5, 0), FileOperation::Added);
        assert_eq!(infer_file_op(0, 3), FileOperation::Deleted);
        assert_eq!(infer_file_op(3, 2), FileOperation::Modified);
        assert_eq!(infer_file_op(0, 0), FileOperation::Modified); // empty file → M (fallback)
    }

    #[test]
    fn test_extract_file_summaries_totals() {
        let lines = vec![
            DiffLine::file_header("src/main.rs"),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "new line".to_string(),
                file_op: None,
            },
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(2))),
                content: "another new".to_string(),
                file_op: None,
            },
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: Some((Some(1), None)),
                content: "old line".to_string(),
                file_op: None,
            },
            DiffLine::separator(),
            DiffLine::file_header("src/lib.rs"),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "pub fn hello()".to_string(),
                file_op: None,
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
            change_id: crate::model::ChangeId::new("abc12345".to_string()),
            commit_id: crate::model::CommitId::new("commit_aaa".to_string()),
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
            change_id: crate::model::ChangeId::new("abc12345".to_string()),
            commit_id: crate::model::CommitId::new("commit_bbb".to_string()),
            ..Change::default()
        }];
        cache.validate(&changes_stale);
        assert_eq!(cache.len(), 0);
    }

    // =========================================================================
    // file_op boundary case tests (bug fix verification)
    // =========================================================================

    #[test]
    fn test_extract_file_summaries_modified_with_only_adds() {
        // Bug fix: Modified file with only additions was misclassified as 'A'
        let lines = vec![
            DiffLine::file_header_with_op("src/main.rs", FileOperation::Modified),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(5))),
                content: "new line".to_string(),
                file_op: None,
            },
        ];
        let summaries = extract_file_summaries(&lines);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].op, FileOperation::Modified);
    }

    #[test]
    fn test_extract_file_summaries_modified_with_only_deletes() {
        // Bug fix: Modified file with only deletions was misclassified as 'D'
        let lines = vec![
            DiffLine::file_header_with_op("src/main.rs", FileOperation::Modified),
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: Some((Some(5), None)),
                content: "old line".to_string(),
                file_op: None,
            },
        ];
        let summaries = extract_file_summaries(&lines);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].op, FileOperation::Modified);
    }

    #[test]
    fn test_extract_file_summaries_added_file() {
        let lines = vec![
            DiffLine::file_header_with_op("src/new.rs", FileOperation::Added),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "fn new() {}".to_string(),
                file_op: None,
            },
        ];
        let summaries = extract_file_summaries(&lines);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].op, FileOperation::Added);
    }

    #[test]
    fn test_extract_file_summaries_deleted_file() {
        let lines = vec![
            DiffLine::file_header_with_op("src/old.rs", FileOperation::Deleted),
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: Some((Some(1), None)),
                content: "fn old() {}".to_string(),
                file_op: None,
            },
        ];
        let summaries = extract_file_summaries(&lines);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].op, FileOperation::Deleted);
    }

    #[test]
    fn test_extract_file_summaries_fallback_without_file_op() {
        // When file_op is None (e.g. git format), infer_file_op is used as fallback
        let lines = vec![
            DiffLine::file_header("src/main.rs"), // no file_op
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: None,
                content: "added".to_string(),
                file_op: None,
            },
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: None,
                content: "deleted".to_string(),
                file_op: None,
            },
        ];
        let summaries = extract_file_summaries(&lines);
        assert_eq!(summaries.len(), 1);
        // Both additions and deletions → fallback infers Modified
        assert_eq!(summaries[0].op, FileOperation::Modified);
    }

    #[test]
    fn test_extract_file_summaries_rename_is_modified() {
        // Rename shows as Modified in the file_op
        let lines = vec![
            DiffLine::file_header_with_op("src/renamed.rs", FileOperation::Modified),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "content".to_string(),
                file_op: None,
            },
        ];
        let summaries = extract_file_summaries(&lines);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].op, FileOperation::Modified);
    }

    #[test]
    fn test_parse_show_to_file_summaries_preserves_file_op() {
        // Integration: parse_show output → extract_file_summaries should preserve file_op
        use crate::jj::parser::Parser;

        let output = "\
Commit ID: abc123
Change ID: xyz789
Author   : Test <test@example.com> (2024-01-30 12:00:00)
Committer: Test <test@example.com> (2024-01-30 12:00:00)

    Append only

Modified regular file src/main.rs:
   10   10:     fn main() {
        11: +       println!(\"new line\");
   11   12:     }
";
        let content = Parser::parse_show(output).unwrap();
        let summaries = extract_file_summaries(&content.lines);

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].path, "src/main.rs");
        // Key assertion: file_op from parse_show prevents misclassification as 'A'
        assert_eq!(summaries[0].op, FileOperation::Modified);
        assert_eq!(summaries[0].insertions, 1);
        assert_eq!(summaries[0].deletions, 0);
    }

    #[test]
    fn test_extract_file_summaries_mixed_operations() {
        // Realistic scenario: one commit with Added + Modified(adds-only) + Modified(deletes-only) + Deleted
        let lines = vec![
            // File 1: Added
            DiffLine::file_header_with_op("src/brand_new.rs", FileOperation::Added),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(1))),
                content: "pub fn new() {}".to_string(),
                file_op: None,
            },
            DiffLine::separator(),
            // File 2: Modified but only additions (was buggy: showed A)
            DiffLine::file_header_with_op("src/main.rs", FileOperation::Modified),
            DiffLine {
                kind: DiffLineKind::Added,
                line_numbers: Some((None, Some(5))),
                content: "appended line".to_string(),
                file_op: None,
            },
            DiffLine::separator(),
            // File 3: Modified but only deletions (was buggy: showed D)
            DiffLine::file_header_with_op("src/lib.rs", FileOperation::Modified),
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: Some((Some(3), None)),
                content: "removed line".to_string(),
                file_op: None,
            },
            DiffLine::separator(),
            // File 4: Deleted
            DiffLine::file_header_with_op("src/old.rs", FileOperation::Deleted),
            DiffLine {
                kind: DiffLineKind::Deleted,
                line_numbers: Some((Some(1), None)),
                content: "fn old() {}".to_string(),
                file_op: None,
            },
        ];

        let summaries = extract_file_summaries(&lines);
        assert_eq!(summaries.len(), 4);

        assert_eq!(summaries[0].path, "src/brand_new.rs");
        assert_eq!(summaries[0].op, FileOperation::Added);

        assert_eq!(summaries[1].path, "src/main.rs");
        assert_eq!(summaries[1].op, FileOperation::Modified); // NOT Added

        assert_eq!(summaries[2].path, "src/lib.rs");
        assert_eq!(summaries[2].op, FileOperation::Modified); // NOT Deleted

        assert_eq!(summaries[3].path, "src/old.rs");
        assert_eq!(summaries[3].op, FileOperation::Deleted);
    }

    #[test]
    fn test_parse_diff_body_to_file_summaries_preserves_file_op() {
        // Integration: parse_diff_body (compare diff path) → extract_file_summaries
        use crate::jj::parser::Parser;

        let output = "\
Modified regular file src/main.rs:
   10   10:     fn main() {
        11: +       println!(\"appended\");
   11   12:     }
Added regular file src/new.rs:
        1: pub fn new() {}
Removed regular file src/old.rs:
    1    : fn old() {}
";
        let content = Parser::parse_diff_body(output);
        let summaries = extract_file_summaries(&content.lines);

        assert_eq!(summaries.len(), 3);

        // Modified with only additions — must NOT fall back to 'A'
        assert_eq!(summaries[0].path, "src/main.rs");
        assert_eq!(summaries[0].op, FileOperation::Modified);
        assert_eq!(summaries[0].insertions, 1);
        assert_eq!(summaries[0].deletions, 0);

        assert_eq!(summaries[1].path, "src/new.rs");
        assert_eq!(summaries[1].op, FileOperation::Added);

        assert_eq!(summaries[2].path, "src/old.rs");
        assert_eq!(summaries[2].op, FileOperation::Deleted);
    }

    #[test]
    fn test_git_format_falls_back_to_infer() {
        // Git format has file_op=None, so extract_file_summaries must use infer_file_op
        use crate::jj::parser::Parser;

        let output = "\
diff --git a/src/main.rs b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!(\"new\");
-    println!(\"old\");
 }";
        let content = Parser::parse_diff_body_git(output);
        let summaries = extract_file_summaries(&content.lines);

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].path, "src/main.rs");
        // Both +1 and -1 → infer_file_op returns Modified
        assert_eq!(summaries[0].op, FileOperation::Modified);
    }
}
