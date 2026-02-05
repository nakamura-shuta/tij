//! Application module
//!
//! Contains the main application state and logic, split into:
//! - `state`: App struct, View enum, initialization, basic view switching
//! - `actions`: jj operations (describe, edit, squash, bookmark, etc.)
//! - `navigation`: Opening views with data loading (diff, blame, resolve)
//! - `refresh`: Data refresh operations (reload from jj)
//! - `input`: Key event handling
//! - `render`: UI rendering

mod actions;
mod input;
mod navigation;
mod refresh;
mod render;
mod state;

pub use state::App;
