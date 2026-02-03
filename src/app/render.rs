//! Rendering logic for the application

use ratatui::{Frame, prelude::*};

use super::state::{App, View};
use crate::ui::widgets::{
    log_view_status_bar_height, operation_view_status_bar_height, render_diff_status_bar,
    render_error_banner, render_help_panel, render_notification_banner,
    render_operation_status_bar, render_placeholder, render_status_bar,
    render_status_view_status_bar, status_view_status_bar_height,
};

impl App {
    /// Render the UI
    pub fn render(&self, frame: &mut Frame) {
        match self.current_view {
            View::Log => self.render_log_view(frame),
            View::Diff => self.render_diff_view(frame),
            View::Status => self.render_status_view(frame),
            View::Operation => self.render_operation_view(frame),
            View::Help => self.render_help_view(frame),
        }

        // Calculate status bar height for notification positioning
        let area = frame.area();
        let status_bar_height = match self.current_view {
            View::Log => log_view_status_bar_height(area.width),
            View::Status => status_view_status_bar_height(area.width),
            View::Operation => operation_view_status_bar_height(area.width),
            View::Diff | View::Help => 1,
        };

        // Render error message if present (takes priority over notification)
        if let Some(ref error) = self.error_message {
            render_error_banner(frame, error, status_bar_height);
        } else if let Some(ref notification) = self.notification {
            // Render notification if no error and notification exists
            if !notification.is_expired() {
                render_notification_banner(frame, notification, status_bar_height);
            }
        }

        // Render dialog on top of everything
        if let Some(ref dialog) = self.active_dialog {
            dialog.render(frame, frame.area());
        }
    }

    fn render_log_view(&self, frame: &mut Frame) {
        let area = frame.area();
        let status_bar_height = log_view_status_bar_height(area.width);

        // Reserve space for status bar (1 or 2 rows depending on width)
        let main_area = Rect {
            height: area.height.saturating_sub(status_bar_height),
            ..area
        };

        self.log_view.render(frame, main_area);
        render_status_bar(frame);
    }

    fn render_diff_view(&self, frame: &mut Frame) {
        if let Some(ref diff_view) = self.diff_view {
            let area = frame.area();

            // Reserve space for status bar
            let main_area = Rect {
                height: area.height.saturating_sub(1),
                ..area
            };

            // Store visible height for diff content (header=4, context=1)
            // This is used by key handling for accurate scroll bounds
            let diff_content_height = main_area.height.saturating_sub(5);
            self.last_frame_height.set(diff_content_height);

            diff_view.render(frame, main_area);
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

    fn render_status_view(&self, frame: &mut Frame) {
        let area = frame.area();
        let status_bar_height = status_view_status_bar_height(area.width);

        // Reserve space for status bar (1 or 2 rows depending on width)
        let main_area = Rect {
            height: area.height.saturating_sub(status_bar_height),
            ..area
        };

        // Store visible height for file list (2 borders + 3 header lines)
        // This is used by key handling for accurate scroll bounds
        let file_list_height = main_area.height.saturating_sub(5);
        self.last_frame_height.set(file_list_height);

        self.status_view.render(frame, main_area);
        render_status_view_status_bar(frame);
    }

    fn render_operation_view(&self, frame: &mut Frame) {
        let area = frame.area();
        let status_bar_height = operation_view_status_bar_height(area.width);

        // Reserve space for status bar (1 or 2 rows depending on width)
        let main_area = Rect {
            height: area.height.saturating_sub(status_bar_height),
            ..area
        };

        self.operation_view.render(frame, main_area);
        render_operation_status_bar(frame);
    }

    fn render_help_view(&self, frame: &mut Frame) {
        render_help_panel(frame);
    }
}
