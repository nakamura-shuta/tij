//! Application state and logic for Tij

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    prelude::*,
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

use crate::jj::{JjExecutor, Parser};
use crate::keys;
use crate::ui::views::{InputMode, LogAction, LogView};

/// Available views in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Log,
    Diff,
    Status,
    Help,
}

/// The main application state
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// Current view
    pub current_view: View,
    /// Previous view (for back navigation)
    previous_view: Option<View>,
    /// Log view state
    pub log_view: LogView,
    /// jj executor
    pub jj: JjExecutor,
    /// Error message to display
    pub error_message: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new() -> Self {
        let mut app = Self {
            running: true,
            current_view: View::Log,
            previous_view: None,
            log_view: LogView::new(),
            jj: JjExecutor::new(),
            error_message: None,
        };

        // Load initial log
        app.refresh_log(None);

        app
    }

    /// Refresh the log view with optional revset
    pub fn refresh_log(&mut self, revset: Option<&str>) {
        match self.jj.log_raw(revset) {
            Ok(output) => match Parser::parse_log(&output) {
                Ok(changes) => {
                    self.log_view.set_changes(changes);
                    self.log_view.current_revset = revset.map(|s| s.to_string());
                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(format!("Parse error: {}", e));
                }
            },
            Err(e) => {
                self.error_message = Some(format!("jj error: {}", e));
            }
        }
    }

    /// Handle key events
    pub fn on_key_event(&mut self, key: KeyEvent) {
        // Clear error message on any key press
        self.error_message = None;

        // Handle Ctrl+C globally
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            self.quit();
            return;
        }

        // If in input mode, delegate all keys to LogView (skip global handling)
        if self.current_view == View::Log && self.log_view.input_mode != InputMode::Normal {
            let action = self.log_view.handle_key(key);
            self.handle_log_action(action);
            return;
        }

        // Global keys
        match key.code {
            keys::QUIT => self.handle_quit(),
            keys::ESC => self.handle_back(),
            keys::HELP => self.go_to_view(View::Help),
            keys::TAB => self.next_view(),
            keys::STATUS_VIEW if self.current_view == View::Log => self.go_to_view(View::Status),
            _ => self.handle_view_key(key),
        }
    }

    fn handle_quit(&mut self) {
        if self.current_view == View::Log {
            self.quit();
        } else {
            self.go_back();
        }
    }

    fn handle_back(&mut self) {
        if self.current_view != View::Log {
            self.go_back();
        }
    }

    fn handle_view_key(&mut self, key: KeyEvent) {
        match self.current_view {
            View::Log => {
                let action = self.log_view.handle_key(key);
                self.handle_log_action(action);
            }
            View::Diff => {
                // TODO: Diff view key handling
            }
            View::Status => {
                // TODO: Status view key handling
            }
            View::Help => {
                // Help view only uses global keys
            }
        }
    }

    fn handle_log_action(&mut self, action: LogAction) {
        match action {
            LogAction::None => {}
            LogAction::OpenDiff(change_id) => {
                // TODO: Open diff view for change_id
                let _ = change_id;
                self.go_to_view(View::Diff);
            }
            LogAction::ExecuteRevset(revset) => {
                self.refresh_log(Some(&revset));
            }
        }
    }

    fn next_view(&mut self) {
        let next = match self.current_view {
            View::Log => View::Status,
            View::Status => View::Log,
            View::Diff => View::Log,
            View::Help => View::Log,
        };
        self.go_to_view(next);
    }

    fn go_to_view(&mut self, view: View) {
        if self.current_view != view {
            self.previous_view = Some(self.current_view);
            self.current_view = view;
        }
    }

    fn go_back(&mut self) {
        if let Some(prev) = self.previous_view.take() {
            self.current_view = prev;
        } else {
            self.current_view = View::Log;
        }
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }

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
            self.render_error(frame, error);
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
        self.render_status_bar(frame);
    }

    fn render_diff_view(&self, frame: &mut Frame) {
        let area = frame.area();

        let title = Line::from(" Tij - Diff View ").bold().yellow().centered();

        frame.render_widget(
            Paragraph::new("Diff view - Press q to go back")
                .block(Block::default().borders(Borders::ALL).title(title)),
            area,
        );
    }

    fn render_status_view(&self, frame: &mut Frame) {
        let area = frame.area();

        let title = Line::from(" Tij - Status View ").bold().green().centered();

        frame.render_widget(
            Paragraph::new("Status view - Press q or Tab to go back")
                .block(Block::default().borders(Borders::ALL).title(title)),
            area,
        );
    }

    fn render_help_view(&self, frame: &mut Frame) {
        let area = frame.area();

        let title = Line::from(" Tij - Help ").bold().white().centered();

        let mut lines = vec![
            Line::from("Key bindings:".bold()),
            Line::from(""),
            Line::from("Global:".underlined()),
        ];

        for entry in keys::GLOBAL_KEYS {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:10}", entry.key),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(entry.description),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from("Navigation:".underlined()));

        for entry in keys::NAV_KEYS {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:10}", entry.key),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(entry.description),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from("Log View:".underlined()));

        for entry in keys::LOG_KEYS {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:10}", entry.key),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(entry.description),
            ]));
        }

        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title)),
            area,
        );
    }

    fn render_status_bar(&self, frame: &mut Frame) {
        let area = frame.area();
        if area.height < 2 {
            return;
        }

        let status_area = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };

        let status = Line::from(vec![
            Span::styled(
                " [?] Help ",
                Style::default().fg(Color::Black).bg(Color::Cyan),
            ),
            Span::raw(" "),
            Span::styled(
                " [/] Search ",
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ),
            Span::raw(" "),
            Span::styled(
                " [r] Revset ",
                Style::default().fg(Color::Black).bg(Color::Magenta),
            ),
            Span::raw(" "),
            Span::styled(
                " [Tab] Switch ",
                Style::default().fg(Color::Black).bg(Color::Blue),
            ),
            Span::raw(" "),
            Span::styled(
                " [q] Quit ",
                Style::default().fg(Color::Black).bg(Color::Red),
            ),
        ]);

        frame.render_widget(Paragraph::new(status), status_area);
    }

    fn render_error(&self, frame: &mut Frame, error: &str) {
        let area = frame.area();
        let error_area = Rect {
            x: area.x + 2,
            y: area.y + area.height.saturating_sub(3),
            width: area.width.saturating_sub(4),
            height: 1,
        };

        let error_line = Line::from(vec![
            Span::styled(" Error: ", Style::default().fg(Color::White).bg(Color::Red)),
            Span::styled(format!(" {} ", error), Style::default().fg(Color::Red)),
        ]);

        frame.render_widget(Paragraph::new(error_line), error_area);
    }
}
