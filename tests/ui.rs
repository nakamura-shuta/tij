//! UI snapshot tests using insta and ratatui's TestBackend
//!
//! These tests capture terminal output for regression detection.
//! Reference: https://ratatui.rs/recipes/testing/snapshots/

#[path = "ui/test_dialog.rs"]
mod test_dialog;

#[path = "ui/test_help.rs"]
mod test_help;

#[path = "ui/test_log.rs"]
mod test_log;

#[path = "ui/test_diff.rs"]
mod test_diff;

#[path = "ui/test_status.rs"]
mod test_status;
