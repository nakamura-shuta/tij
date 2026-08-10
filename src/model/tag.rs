//! Tag model for `jj tag list`

use super::id::{ChangeId, CommitId};

/// Tag information with target commit details
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagInfo {
    /// Tag name (e.g., "v0.4.10")
    pub name: String,
    /// Remote name (None = local, Some("origin") = remote)
    pub remote: Option<String>,
    /// Whether the tag is present (not deleted on remote)
    pub present: bool,
    /// Whether a remote tag is tracked by a local tag (jj 0.44+)
    ///
    /// Local rows always report `false`; only remote rows can be tracked.
    pub tracked: bool,
    /// Whether the tag target is conflicted
    pub conflict: bool,
    /// Target change_id (short form, 8 chars)
    pub change_id: Option<ChangeId>,
    /// Target commit_id (short form, 8 chars)
    pub commit_id: Option<CommitId>,
    /// Target commit description (first line)
    pub description: Option<String>,
}

impl TagInfo {
    /// Full display name: "v0.4.10" or "v0.4.10@origin"
    pub fn full_name(&self) -> String {
        match &self.remote {
            Some(remote) => format!("{}@{}", self.name, remote),
            None => self.name.clone(),
        }
    }

    /// Whether this is a local tag (no remote)
    pub fn is_local(&self) -> bool {
        self.remote.is_none()
    }

    /// Check if this is a remote-only tag (untracked)
    ///
    /// Mirrors `Bookmark::is_untracked_remote()`.
    pub fn is_untracked_remote(&self) -> bool {
        self.remote.is_some() && !self.tracked
    }

    /// Check if this is a remote tag that is tracked locally
    pub fn is_tracked_remote(&self) -> bool {
        self.remote.is_some() && self.tracked
    }

    /// Whether this tag can be jumped to (has change_id)
    pub fn is_jumpable(&self) -> bool {
        self.change_id.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Local tag row (`tracked` is always false for local rows in jj output)
    fn local_tag(name: &str) -> TagInfo {
        TagInfo {
            name: name.into(),
            remote: None,
            present: true,
            tracked: false,
            conflict: false,
            change_id: Some(ChangeId::from("mzslzzzz")),
            commit_id: Some(CommitId::from("57d01adc")),
            description: Some("fix: something".into()),
        }
    }

    fn remote_tag(name: &str, remote: &str, tracked: bool) -> TagInfo {
        TagInfo {
            name: name.into(),
            remote: Some(remote.into()),
            present: true,
            tracked,
            conflict: false,
            change_id: None,
            commit_id: None,
            description: None,
        }
    }

    #[test]
    fn test_full_name_local() {
        assert_eq!(local_tag("v0.4.10").full_name(), "v0.4.10");
    }

    #[test]
    fn test_full_name_remote() {
        assert_eq!(
            remote_tag("v0.4.10", "origin", false).full_name(),
            "v0.4.10@origin"
        );
    }

    #[test]
    fn test_is_local() {
        assert!(local_tag("v1.0").is_local());
        assert!(!remote_tag("v1.0", "origin", false).is_local());
    }

    #[test]
    fn test_is_jumpable() {
        let jumpable = local_tag("v1.0");
        assert!(jumpable.is_jumpable());

        let mut not_jumpable = local_tag("v1.0");
        not_jumpable.change_id = None;
        not_jumpable.commit_id = None;
        not_jumpable.description = None;
        assert!(!not_jumpable.is_jumpable());
    }

    #[test]
    fn test_is_untracked_remote() {
        // Local tag - not an untracked remote
        assert!(!local_tag("v1.0").is_untracked_remote());

        // Remote tracked tag - not an untracked remote
        assert!(!remote_tag("v1.0", "origin", true).is_untracked_remote());

        // Remote untracked tag - IS an untracked remote
        assert!(remote_tag("v1.0", "origin", false).is_untracked_remote());
    }

    #[test]
    fn test_is_tracked_remote() {
        // Local tag - never a tracked remote, even if `tracked` were set
        assert!(!local_tag("v1.0").is_tracked_remote());
        let mut local_but_tracked = local_tag("v1.0");
        local_but_tracked.tracked = true;
        assert!(!local_but_tracked.is_tracked_remote());

        // Remote tracked tag - IS a tracked remote
        assert!(remote_tag("v1.0", "origin", true).is_tracked_remote());

        // Remote untracked tag - not a tracked remote
        assert!(!remote_tag("v1.0", "origin", false).is_tracked_remote());
    }

    #[test]
    fn test_tracked_helpers_are_mutually_exclusive() {
        for tag in [
            local_tag("v1.0"),
            remote_tag("v1.0", "origin", true),
            remote_tag("v1.0", "origin", false),
        ] {
            assert!(!(tag.is_untracked_remote() && tag.is_tracked_remote()));
        }
    }
}
