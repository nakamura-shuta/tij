//! Application module
//!
//! Contains the main application state and logic, split into:
//! - `state`: App struct and view management
//! - `input`: Key event handling
//! - `render`: UI rendering

mod input;
mod render;
mod state;

pub use state::App;
