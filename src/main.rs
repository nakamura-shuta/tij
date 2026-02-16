//! Tij - Text-mode Interface for Jujutsu
//!
//! Binary entry point for the TUI application.

use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use tij::app::App;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    result
}

/// Run the application's main loop.
fn run(mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
    let mut app = App::new();

    while app.running {
        terminal.draw(|frame| app.render(frame))?;
        handle_events(&mut app)?;
    }

    Ok(())
}

/// Handle crossterm events.
///
/// Uses poll with 200ms timeout to support idle processing (e.g., debounced preview fetch).
/// When no key event arrives within the timeout, pending preview fetches are resolved.
fn handle_events(app: &mut App) -> color_eyre::Result<()> {
    if event::poll(Duration::from_millis(200))? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                app.on_key_event(key);
            }
            _ => {}
        }
    } else {
        // Idle: resolve any pending preview fetch
        app.resolve_pending_preview();
    }
    Ok(())
}
