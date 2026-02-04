//! View components
//!
//! Each view represents a screen in the application.

mod blame;
mod diff;
mod log;
mod operation;
mod status;

pub use blame::{BlameAction, BlameView};
pub use diff::{DiffAction, DiffView};
pub use log::{InputMode, LogAction, LogView};
pub use operation::{OperationAction, OperationView};
pub use status::{StatusAction, StatusInputMode, StatusView};
