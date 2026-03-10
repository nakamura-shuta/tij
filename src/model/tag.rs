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

    /// Whether this tag can be jumped to (has change_id)
    pub fn is_jumpable(&self) -> bool {
        self.change_id.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_name_local() {
        let tag = TagInfo {
            name: "v0.4.10".into(),
            remote: None,
            present: true,
            change_id: Some(ChangeId::from("mzslzzzz")),
            commit_id: Some(CommitId::from("57d01adc")),
            description: Some("fix: something".into()),
        };
        assert_eq!(tag.full_name(), "v0.4.10");
    }

    #[test]
    fn test_full_name_remote() {
        let tag = TagInfo {
            name: "v0.4.10".into(),
            remote: Some("origin".into()),
            present: true,
            change_id: None,
            commit_id: None,
            description: None,
        };
        assert_eq!(tag.full_name(), "v0.4.10@origin");
    }

    #[test]
    fn test_is_local() {
        let local = TagInfo {
            name: "v1.0".into(),
            remote: None,
            present: true,
            change_id: Some(ChangeId::from("abc")),
            commit_id: None,
            description: None,
        };
        assert!(local.is_local());

        let remote = TagInfo {
            name: "v1.0".into(),
            remote: Some("origin".into()),
            present: true,
            change_id: None,
            commit_id: None,
            description: None,
        };
        assert!(!remote.is_local());
    }

    #[test]
    fn test_is_jumpable() {
        let jumpable = TagInfo {
            name: "v1.0".into(),
            remote: None,
            present: true,
            change_id: Some(ChangeId::from("abc12345")),
            commit_id: Some(CommitId::from("def67890")),
            description: Some("release".into()),
        };
        assert!(jumpable.is_jumpable());

        let not_jumpable = TagInfo {
            name: "v1.0".into(),
            remote: None,
            present: true,
            change_id: None,
            commit_id: None,
            description: None,
        };
        assert!(!not_jumpable.is_jumpable());
    }
}
