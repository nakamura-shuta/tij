//! Data models for Tij
//!
//! This module contains UI-independent data structures representing
//! jj concepts like changes, diffs, and file status.

mod annotation;
mod change;
mod diff;
mod file_status;
mod notification;
mod operation;

pub use annotation::{AnnotationContent, AnnotationLine};
pub use change::Change;
pub use diff::{DiffContent, DiffLine, DiffLineKind};
pub use file_status::{FileState, FileStatus, Status};
pub use notification::{Notification, NotificationKind};
pub use operation::Operation;
