//! View components
//!
//! Each view represents a screen in the application.

mod diff;
mod log;

pub use diff::{DiffAction, DiffView};
pub use log::{InputMode, LogAction, LogView};
