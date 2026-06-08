//! View components
//!
//! Each view represents a screen in the application.

mod blame;
mod bookmark;
mod command_history;
mod diff;
mod evolog;
mod log;
mod operation;
mod resolve;
mod status;
mod tag;
mod trace_detail;
mod workspace;

pub use blame::{BlameAction, BlameView};
pub use bookmark::{BookmarkAction, BookmarkView, RenameState};
pub use command_history::{CommandHistoryAction, CommandHistoryView};
pub use diff::{DiffAction, DiffView};
pub use evolog::{EvologAction, EvologView};
pub use log::{InputMode, LogAction, LogCommand, LogView, RebaseMode};
pub use operation::{OperationAction, OperationView};
pub use resolve::{ResolveAction, ResolveView};
pub use status::{StatusAction, StatusInputMode, StatusView};
pub use tag::{TagAction, TagView};
pub use trace_detail::{TraceDetailAction, TraceDetailView};
pub use workspace::{WorkspaceAction, WorkspaceView};
