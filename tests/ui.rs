//! UI snapshot tests using insta and ratatui's TestBackend
//!
//! These tests capture terminal output for regression detection.
//! Reference: https://ratatui.rs/recipes/testing/snapshots/

#[path = "ui/test_dialog.rs"]
mod test_dialog;

#[path = "ui/test_help.rs"]
mod test_help;
