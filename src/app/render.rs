//! Rendering logic for the application

use ratatui::{Frame, prelude::*};

use super::state::{App, View};
use crate::ui::widgets::{
    render_error_banner, render_help_panel, render_placeholder, render_status_bar,
};

impl App {
    /// Render the UI
    pub fn render(&self, frame: &mut Frame) {
        match self.current_view {
            View::Log => self.render_log_view(frame),
            View::Diff => self.render_diff_view(frame),
            View::Status => self.render_status_view(frame),
            View::Help => self.render_help_view(frame),
        }

        // Render error message if present
        if let Some(ref error) = self.error_message {
            render_error_banner(frame, error);
        }
    }

    fn render_log_view(&self, frame: &mut Frame) {
        let area = frame.area();

        // Reserve space for status bar
        let main_area = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };

        self.log_view.render(frame, main_area);
        render_status_bar(frame);
    }

    fn render_diff_view(&self, frame: &mut Frame) {
        render_placeholder(
            frame,
            " Tij - Diff View ",
            Color::Yellow,
            "Diff view - Press q to go back",
        );
    }

    fn render_status_view(&self, frame: &mut Frame) {
        render_placeholder(
            frame,
            " Tij - Status View ",
            Color::Green,
            "Status view - Press q or Tab to go back",
        );
    }

    fn render_help_view(&self, frame: &mut Frame) {
        render_help_panel(frame);
    }
}
