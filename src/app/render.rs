//! Rendering logic for the application

use ratatui::{Frame, prelude::*};

use super::state::{App, View};
use crate::keys::{self, BookmarkKind, DialogHintKind, HintContext};
use crate::ui::components::dialog::DialogKind;
use crate::ui::widgets::{
    render_blame_status_bar, render_diff_status_bar, render_error_banner, render_help_panel,
    render_placeholder, render_status_hints, status_hints_height,
};

impl App {
    /// Render the UI
    pub fn render(&self, frame: &mut Frame) {
        // Get active notification (not expired)
        let notification = self.notification.as_ref().filter(|n| !n.is_expired());

        // Render main view (notification is passed to views for title bar display)
        match self.current_view {
            View::Log => self.render_log_view(frame, notification),
            View::Diff => self.render_diff_view(frame, notification),
            View::Status => self.render_status_view(frame, notification),
            View::Operation => self.render_operation_view(frame, notification),
            View::Blame => self.render_blame_view(frame, notification),
            View::Resolve => self.render_resolve_view(frame, notification),
            View::Bookmark => self.render_bookmark_view(frame, notification),
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
        &self,
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

        self.log_view.render(frame, main_area, notification);
        render_status_hints(frame, &hints);
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
