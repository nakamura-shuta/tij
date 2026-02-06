//! Bookmark model for `jj bookmark list --all`

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
}
