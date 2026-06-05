//! Tij - Text-mode Interface for Jujutsu
//!
//! A TUI application for the Jujutsu version control system.
//!
//! This library provides:
//! - [`app`]: Application state and logic
//! - [`jj`]: Jujutsu command execution and parsing
//! - [`keys`]: Key binding definitions
//! - [`model`]: Domain models
//! - [`trace`]: Agent Trace (AI code attribution) reader
//! - [`ui`]: User interface components

pub mod app;
pub mod jj;
pub mod keys;
pub mod model;
pub mod trace;
pub mod ui;
