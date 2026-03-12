//! Revision lookup and display helpers
//!
//! Centralizes common patterns for ID truncation, lookup between
//! change_id and commit_id, and root commit detection.

use crate::jj::constants::ROOT_CHANGE_ID;
use crate::model::{Change, ChangeId, CommitId};
use crate::ui::views::LogView;

/// Common payload extracted from `selected_change()`.
///
/// Provides the two IDs that most actions need, avoiding repeated
/// destructuring of `Change` at each call site.
#[derive(Debug, Clone)]
pub struct SelectedRevision {
    pub change_id: ChangeId,
    pub commit_id: CommitId,
    pub is_working_copy: bool,
}

impl SelectedRevision {
    /// Extract from the currently selected change in a LogView.
    pub fn from_log_view(log_view: &LogView) -> Option<Self> {
        log_view.selected_change().map(|c| Self {
            change_id: c.change_id.clone(),
            commit_id: c.commit_id.clone(),
            is_working_copy: c.is_working_copy,
        })
    }
}

/// Truncate an ID to 8 characters for display.
///
/// Safe for inputs shorter than 8 characters (returns the full string).
pub fn short_id(id: &str) -> &str {
    &id[..8.min(id.len())]
}

/// Reverse-lookup: find the change_id for a given commit_id.
///
/// Returns `None` if the commit_id is not in the changes list.
pub fn change_id_for_commit<'a>(changes: &'a [Change], commit_id: &str) -> Option<&'a str> {
    changes
        .iter()
        .find(|c| c.commit_id == commit_id)
        .map(|c| c.change_id.as_str())
}

/// Forward-lookup: find the commit_id for a given change_id.
///
/// For divergent changes (same change_id, multiple commits),
/// returns the first match.
#[allow(dead_code)] // API for Phase 39
pub fn commit_id_for_change<'a>(changes: &'a [Change], change_id: &str) -> Option<&'a str> {
    changes
        .iter()
        .find(|c| c.change_id == change_id)
        .map(|c| c.commit_id.as_str())
}

/// Check if a commit_id corresponds to the root commit.
///
/// Reverse-looks up the change_id and compares with ROOT_CHANGE_ID.
pub fn is_root_by_commit_id(changes: &[Change], commit_id: &str) -> bool {
    change_id_for_commit(changes, commit_id)
        .map(|cid| cid == ROOT_CHANGE_ID)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_change(change_id: &str, commit_id: &str) -> Change {
        Change {
            change_id: crate::model::ChangeId::new(change_id.to_string()),
            commit_id: crate::model::CommitId::new(commit_id.to_string()),
            ..Default::default()
        }
    }

    // ── short_id ─────────────────────────────────────────────────────

    #[test]
    fn test_short_id_normal() {
        assert_eq!(short_id("abcdef1234567890"), "abcdef12");
    }

    #[test]
    fn test_short_id_exact_8() {
        assert_eq!(short_id("abcdef12"), "abcdef12");
    }

    #[test]
    fn test_short_id_short_input() {
        assert_eq!(short_id("abc"), "abc");
    }

    #[test]
    fn test_short_id_empty() {
        assert_eq!(short_id(""), "");
    }

    // ── change_id_for_commit ─────────────────────────────────────────

    #[test]
    fn test_change_id_for_commit_found() {
        let changes = vec![make_change("aaa11111", "bbb22222")];
        assert_eq!(change_id_for_commit(&changes, "bbb22222"), Some("aaa11111"));
    }

    #[test]
    fn test_change_id_for_commit_not_found() {
        let changes = vec![make_change("aaa11111", "bbb22222")];
        assert_eq!(change_id_for_commit(&changes, "ccc33333"), None);
    }

    #[test]
    fn test_change_id_for_commit_empty_list() {
        let changes: Vec<Change> = vec![];
        assert_eq!(change_id_for_commit(&changes, "bbb22222"), None);
    }

    // ── commit_id_for_change ─────────────────────────────────────────

    #[test]
    fn test_commit_id_for_change_found() {
        let changes = vec![make_change("aaa11111", "bbb22222")];
        assert_eq!(commit_id_for_change(&changes, "aaa11111"), Some("bbb22222"));
    }

    #[test]
    fn test_commit_id_for_change_not_found() {
        let changes = vec![make_change("aaa11111", "bbb22222")];
        assert_eq!(commit_id_for_change(&changes, "xxx99999"), None);
    }

    #[test]
    fn test_commit_id_for_change_divergent_returns_first() {
        let changes = vec![
            make_change("aaa11111", "bbb22222"),
            make_change("aaa11111", "ccc33333"), // divergent
        ];
        // Returns the first match
        assert_eq!(commit_id_for_change(&changes, "aaa11111"), Some("bbb22222"));
    }

    // ── is_root_by_commit_id ─────────────────────────────────────────

    #[test]
    fn test_is_root_by_commit_id_true() {
        let changes = vec![
            make_change("zzzzzzzz", "root_cid"),
            make_change("normal11", "normal22"),
        ];
        assert!(is_root_by_commit_id(&changes, "root_cid"));
    }

    #[test]
    fn test_is_root_by_commit_id_false() {
        let changes = vec![
            make_change("zzzzzzzz", "root_cid"),
            make_change("normal11", "normal22"),
        ];
        assert!(!is_root_by_commit_id(&changes, "normal22"));
    }

    #[test]
    fn test_is_root_by_commit_id_unknown() {
        let changes = vec![make_change("normal11", "normal22")];
        assert!(!is_root_by_commit_id(&changes, "unknown_"));
    }
}
