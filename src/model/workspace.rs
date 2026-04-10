//! Workspace model for `jj workspace list`

use super::id::ChangeId;

/// Workspace information from `jj workspace list`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInfo {
    /// Workspace name (e.g., "default", "feature-a")
    pub name: String,
    /// Workspace root path (from self.root(), jj 0.40+)
    /// None if path is not recorded
    pub root_path: Option<String>,
    /// Working copy change_id (short form, 8 chars)
    pub change_id: ChangeId,
    /// Working copy commit description (first line)
    pub description: String,
}

impl WorkspaceInfo {
    /// Whether this is the default workspace
    pub fn is_default(&self) -> bool {
        self.name == "default"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_default() {
        let ws = WorkspaceInfo {
            name: "default".into(),
            root_path: Some("/tmp/repo".into()),
            change_id: ChangeId::from("ltyxkzyp"),
            description: "(no description set)".into(),
        };
        assert!(ws.is_default());
    }

    #[test]
    fn test_not_default() {
        let ws = WorkspaceInfo {
            name: "feature-a".into(),
            root_path: Some("/tmp/feature-ws".into()),
            change_id: ChangeId::from("xyzpqrst"),
            description: "implement feature A".into(),
        };
        assert!(!ws.is_default());
    }
}
