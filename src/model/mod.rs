//! Data models for Tij
//!
//! This module contains UI-independent data structures representing
//! jj concepts like changes, diffs, and file status.

mod annotation;
mod bookmark;
mod change;
mod conflict;
mod diff;
mod evolog;
mod file_status;
mod notification;
mod operation;

pub use annotation::{AnnotationContent, AnnotationLine};
pub use bookmark::{Bookmark, BookmarkInfo};
pub use change::Change;
pub use conflict::ConflictFile;
pub use diff::{
    CompareInfo, CompareRevisionInfo, DiffContent, DiffDisplayFormat, DiffLine, DiffLineKind,
};
pub use evolog::EvologEntry;
pub use file_status::{FileState, FileStatus, Status};
pub use notification::{Notification, NotificationKind};
pub use operation::Operation;
