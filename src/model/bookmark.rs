//! Bookmark model for `jj bookmark list --all`

use super::id::{ChangeId, CommitId};

/// Bookmark information from `jj bookmark list --all`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    /// Bookmark name (e.g., "main", "feature-x")
    pub name: String,
    /// Remote name if this is a remote bookmark (e.g., "origin")
    pub remote: Option<String>,
    /// Whether this is tracked locally
    pub is_tracked: bool,
}

impl Bookmark {
    /// Full name including remote (e.g., "feature-x@origin")
    pub fn full_name(&self) -> String {
        match &self.remote {
            Some(remote) => format!("{}@{}", self.name, remote),
            None => self.name.clone(),
        }
    }

    /// Check if this is a remote-only bookmark (untracked)
    pub fn is_untracked_remote(&self) -> bool {
        self.remote.is_some() && !self.is_tracked
    }
}

/// Extended bookmark information with revision details
///
/// Used for Bookmark Jump and Bookmark View features.
/// Remote-only bookmarks may not have change_id/commit_id/description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkInfo {
    /// Basic bookmark information
    pub bookmark: Bookmark,
    /// Change ID (short form, e.g., "kxryzmor")
    /// None for remote-only bookmarks not in local repository
    pub change_id: Option<ChangeId>,
    /// Commit ID (short form)
    pub commit_id: Option<CommitId>,
    /// Commit description (first line)
    pub description: Option<String>,
}

impl BookmarkInfo {
    /// Check if this bookmark can be jumped to (has change_id)
    pub fn is_jumpable(&self) -> bool {
        self.change_id.is_some()
    }

    /// Get display label for dialog (name + description)
    pub fn display_label(&self, max_width: usize) -> String {
        let name = self.bookmark.full_name();
        let desc = self.description.as_deref().unwrap_or("(no description)");

        // Format: "name: description" with truncation
        let prefix = format!("{}: ", name);
        let available = max_width.saturating_sub(prefix.len());

        if desc.len() <= available {
            format!("{}{}", prefix, desc)
        } else if available > 3 {
            format!("{}{}...", prefix, &desc[..available - 3])
        } else {
            prefix
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_name_local() {
        let bookmark = Bookmark {
            name: "main".into(),
            remote: None,
            is_tracked: true,
        };
        assert_eq!(bookmark.full_name(), "main");
    }

    #[test]
    fn test_full_name_remote() {
        let bookmark = Bookmark {
            name: "feature-x".into(),
            remote: Some("origin".into()),
            is_tracked: false,
        };
        assert_eq!(bookmark.full_name(), "feature-x@origin");
    }

    #[test]
    fn test_is_untracked_remote() {
        // Local bookmark - not untracked remote
        let local = Bookmark {
            name: "main".into(),
            remote: None,
            is_tracked: true,
        };
        assert!(!local.is_untracked_remote());

        // Remote tracked bookmark - not untracked remote
        let tracked_remote = Bookmark {
            name: "main".into(),
            remote: Some("origin".into()),
            is_tracked: true,
        };
        assert!(!tracked_remote.is_untracked_remote());

        // Remote untracked bookmark - IS untracked remote
        let untracked_remote = Bookmark {
            name: "feature".into(),
            remote: Some("origin".into()),
            is_tracked: false,
        };
        assert!(untracked_remote.is_untracked_remote());
    }

    #[test]
    fn test_bookmark_info_is_jumpable() {
        let jumpable = BookmarkInfo {
            bookmark: Bookmark {
                name: "main".into(),
                remote: None,
                is_tracked: true,
            },
            change_id: Some(ChangeId::new("abc12345".to_string())),
            commit_id: Some(CommitId::new("def67890".to_string())),
            description: Some("Test commit".into()),
        };
        assert!(jumpable.is_jumpable());

        let not_jumpable = BookmarkInfo {
            bookmark: Bookmark {
                name: "remote-only".into(),
                remote: Some("origin".into()),
                is_tracked: false,
            },
            change_id: None,
            commit_id: None,
            description: None,
        };
        assert!(!not_jumpable.is_jumpable());
    }

    #[test]
    fn test_bookmark_info_display_label() {
        let info = BookmarkInfo {
            bookmark: Bookmark {
                name: "main".into(),
                remote: None,
                is_tracked: true,
            },
            change_id: Some(ChangeId::new("abc12345".to_string())),
            commit_id: Some(CommitId::new("def67890".to_string())),
            description: Some("Fix critical bug".into()),
        };

        let label = info.display_label(40);
        assert_eq!(label, "main: Fix critical bug");

        // Test truncation
        let label_short = info.display_label(20);
        assert!(label_short.ends_with("..."));
    }

    #[test]
    fn test_bookmark_info_display_label_no_description() {
        let info = BookmarkInfo {
            bookmark: Bookmark {
                name: "orphan".into(),
                remote: None,
                is_tracked: true,
            },
            change_id: Some(ChangeId::new("abc12345".to_string())),
            commit_id: Some(CommitId::new("def67890".to_string())),
            description: None,
        };

        let label = info.display_label(40);
        assert_eq!(label, "orphan: (no description)");
    }
}
