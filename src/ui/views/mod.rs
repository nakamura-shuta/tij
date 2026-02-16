//! View components
//!
//! Each view represents a screen in the application.

mod blame;
mod bookmark;
mod diff;
mod evolog;
mod log;
mod operation;
mod resolve;
mod status;

pub use blame::{BlameAction, BlameView};
pub use bookmark::{BookmarkAction, BookmarkView, RenameState};
pub use diff::{DiffAction, DiffView};
pub use evolog::{EvologAction, EvologView};
pub use log::{InputMode, LogAction, LogView, RebaseMode};
pub use operation::{OperationAction, OperationView};
pub use resolve::{ResolveAction, ResolveView};
pub use status::{StatusAction, StatusInputMode, StatusView};
