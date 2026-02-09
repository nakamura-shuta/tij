//! Rendering logic for the application

use ratatui::{Frame, prelude::*};

use super::state::{App, View};
use crate::ui::widgets::{
    blame_view_status_bar_height, log_view_status_bar_height, operation_view_status_bar_height,
    render_blame_status_bar, render_diff_status_bar, render_error_banner, render_help_panel,
    render_operation_status_bar, render_placeholder, render_status_bar,
    render_status_view_status_bar, status_view_status_bar_height,
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
            View::Blame => self.render_blame_view(frame),
            View::Resolve => self.render_resolve_view(frame, notification),
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
            View::Log => log_view_status_bar_height(width, self.log_view.input_mode),
            View::Diff => 1,
            View::Status => status_view_status_bar_height(width),
            View::Operation => operation_view_status_bar_height(width),
            View::Blame => blame_view_status_bar_height(width),
            View::Resolve => 1, // Simple status bar
            View::Help => 0,
        }
    }

    fn render_log_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = frame.area();
        let input_mode = self.log_view.input_mode;
        let status_bar_height = log_view_status_bar_height(area.width, input_mode);

        // Reserve space for status bar at bottom
        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(status_bar_height),
        };

        self.log_view.render(frame, main_area, notification);
        render_status_bar(frame, input_mode);
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
        let status_bar_height = status_view_status_bar_height(area.width);

        // Reserve space for status bar at bottom
        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(status_bar_height),
        };

        // Store visible height for file list (2 borders + 3 header lines)
        // This is used by key handling for accurate scroll bounds
        let file_list_height = main_area.height.saturating_sub(5);
        self.last_frame_height.set(file_list_height);

        self.status_view.render(frame, main_area, notification);
        render_status_view_status_bar(frame);
    }

    fn render_operation_view(
        &self,
        frame: &mut Frame,
        notification: Option<&crate::model::Notification>,
    ) {
        let area = frame.area();
        let status_bar_height = operation_view_status_bar_height(area.width);

        // Reserve space for status bar at bottom
        let main_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(status_bar_height),
        };

        self.operation_view.render(frame, main_area, notification);
        render_operation_status_bar(frame);
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

            // Reserve 1 line for status bar
            let main_area = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height.saturating_sub(1),
            };

            resolve_view.render(frame, main_area, notification);
            self.render_resolve_status_bar(frame, resolve_view);
        } else {
            render_placeholder(
                frame,
                " Tij - Resolve View ",
                Color::Red,
                "No conflicts loaded - Press q to go back",
            );
        }
    }

    fn render_resolve_status_bar(
        &self,
        frame: &mut Frame,
        resolve_view: &crate::ui::views::ResolveView,
    ) {
        use ratatui::style::{Style, Stylize};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;

        let area = frame.area();
        let status_area = Rect {
            x: area.x,
            y: area.height.saturating_sub(1),
            width: area.width,
            height: 1,
        };

        let mut spans = Vec::new();

        if resolve_view.is_working_copy {
            spans.push(Span::styled(
                " Enter ",
                Style::default().fg(Color::Black).bg(Color::Green),
            ));
            spans.push(Span::raw(" Resolve "));
        }

        spans.push(Span::styled(
            " o ",
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ));
        spans.push(Span::raw(" Ours "));

        spans.push(Span::styled(
            " t ",
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ));
        spans.push(Span::raw(" Theirs "));

        spans.push(Span::styled(
            " d ",
            Style::default().fg(Color::Black).bg(Color::Magenta),
        ));
        spans.push(Span::raw(" Diff "));

        spans.push(Span::styled(
            " q ",
            Style::default().fg(Color::Black).bg(Color::Red),
        ));
        spans.push(Span::raw(" Back "));

        let status_line =
            Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::DarkGray).bold());
        frame.render_widget(status_line, status_area);
    }

    fn render_blame_view(&self, frame: &mut Frame) {
        if let Some(ref blame_view) = self.blame_view {
            let area = frame.area();
            let status_bar_height = blame_view_status_bar_height(area.width);

            // Reserve space for status bar at bottom
            let main_area = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height.saturating_sub(status_bar_height),
            };

            // Store visible height for blame content
            let blame_content_height = main_area.height.saturating_sub(2);
            self.last_frame_height.set(blame_content_height);

            blame_view.render(frame, main_area);
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
