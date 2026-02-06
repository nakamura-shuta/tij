//! Tij - Text-mode Interface for Jujutsu
//!
//! A TUI application for the Jujutsu version control system.
//!
//! This library provides:
//! - [`app`]: Application state and logic
//! - [`jj`]: Jujutsu command execution and parsing
//! - [`keys`]: Key binding definitions
//! - [`model`]: Domain models
//! - [`ui`]: User interface components

pub mod app;
pub mod jj;
pub mod keys;
pub mod model;
pub mod ui;
