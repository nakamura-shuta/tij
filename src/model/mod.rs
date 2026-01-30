//! Data models for Tij
//!
//! This module contains UI-independent data structures representing
//! jj concepts like changes, diffs, and file status.

mod change;
mod file_status;

pub use change::Change;
pub use file_status::{FileState, FileStatus, Status};
