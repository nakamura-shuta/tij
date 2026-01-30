//! Tij - Text-mode Interface for Jujutsu
//!
//! A TUI application for the Jujutsu version control system.

mod app;
mod jj;
mod keys;
mod model;
mod ui;

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::app::App;

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
fn handle_events(app: &mut App) -> color_eyre::Result<()> {
    match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            app.on_key_event(key);
        }
        _ => {}
    }
    Ok(())
}
