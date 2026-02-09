//! Common test utilities for integration and scenario tests.
//!
//! This module provides helpers for creating and managing temporary
//! jj repositories in tests.
//!
//! Note: Each integration test file compiles as a separate crate,
//! so not all helpers are used in every test file. We suppress
//! dead_code warnings at the module level.

#![allow(dead_code)]
#![allow(unused_imports)]

pub mod remote_repo;
pub mod test_repo;

pub use remote_repo::RemoteRepo;
pub use test_repo::TestRepo;

/// Check if the `jj` command is available on this system.
pub fn jj_available() -> bool {
    std::process::Command::new("jj")
        .arg("version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Macro to skip a test if `jj` is not available.
///
/// Use at the beginning of any test that requires the `jj` CLI.
#[macro_export]
macro_rules! skip_if_no_jj {
    () => {
        if !common::jj_available() {
            eprintln!("Skipping test: jj command not found");
            return;
        }
    };
}
