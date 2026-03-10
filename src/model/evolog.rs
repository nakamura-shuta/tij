//! Evolution log entry model

use super::id::{ChangeId, CommitId};

/// A single entry in the evolution log of a change
#[derive(Debug, Clone)]
pub struct EvologEntry {
    /// Commit ID (changes with each rewrite)
    pub commit_id: CommitId,
    /// Change ID (stays the same)
    pub change_id: ChangeId,
    /// Author email
    pub author: String,
    /// Timestamp
    pub timestamp: String,
    /// Is this an empty commit
    pub is_empty: bool,
    /// First line of description
    pub description: String,
}
