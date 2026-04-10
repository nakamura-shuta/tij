//! Data models for Tij
//!
//! This module contains UI-independent data structures representing
//! jj concepts like changes, diffs, and file status.

mod annotation;
mod bookmark;
mod change;
mod command_record;
mod conflict;
mod diff;
mod evolog;
mod file_status;
mod id;
mod notification;
mod operation;
mod rebase;
mod tag;
mod workspace;

pub use annotation::{AnnotationContent, AnnotationLine};
pub use bookmark::{Bookmark, BookmarkInfo};
pub use change::Change;
pub use command_record::{CommandHistory, CommandRecord, CommandStatus};
pub use conflict::ConflictFile;
pub use diff::{
    CompareInfo, CompareRevisionInfo, DiffContent, DiffDisplayFormat, DiffLine, DiffLineKind,
    DiffMode, FileOperation,
};
pub use evolog::EvologEntry;
pub use file_status::{FileState, FileStatus, Status};
pub use id::{ChangeId, CommitId};
pub use notification::{Notification, NotificationKind};
pub use operation::Operation;
pub use rebase::RebaseMode;
pub use tag::TagInfo;
pub use workspace::WorkspaceInfo;
