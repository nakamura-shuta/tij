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
