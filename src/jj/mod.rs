//! jj command execution layer
//!
//! This module handles executing jj commands and parsing their output.

pub mod constants;
mod executor;
mod interactive;
/// Parser module (public for integration testing)
pub mod parser;
mod template;

pub use executor::{JjExecutor, PushBulkMode};
pub use parser::{PushPreviewAction, PushPreviewResult, parse_push_dry_run};

use std::io;
use thiserror::Error;

/// Errors that can occur when executing jj commands
#[derive(Error, Debug)]
pub enum JjError {
    #[error("Not a jj repository")]
    NotARepository,

    #[error("jj command failed (exit code {exit_code}): {stderr}")]
    CommandFailed { stderr: String, exit_code: i32 },

    #[error("Failed to parse jj output: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("jj is not installed or not in PATH")]
    JjNotFound,
}
