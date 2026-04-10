//! Workspace View for displaying jj workspaces

mod input;
mod render;

use crate::model::WorkspaceInfo;
use crate::ui::navigation;

/// Action returned by the Workspace View after handling input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceAction {
    /// No action needed
    None,
    /// Add new workspace (open path input dialog)
    StartAdd,
    /// Forget selected workspace (open confirm dialog)
    Forget(String),
    /// Cannot forget: selected workspace is the current one
    ForgetCurrentBlocked,
    /// Rename current workspace (open input dialog)
    StartRename(String),
    /// Cannot rename: selected workspace is not the current one
    RenameNonCurrentBlocked,
    /// Jump to workspace's working copy in Log View
    Jump(String),
}

/// Workspace View state
#[derive(Debug)]
pub struct WorkspaceView {
    /// All workspaces
    workspaces: Vec<WorkspaceInfo>,
    /// Selected index
    selected: usize,
    /// Scroll offset
    scroll_offset: usize,
    /// Current workspace name (determined at open time by root path comparison)
    current_workspace_name: Option<String>,
}

impl Default for WorkspaceView {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceView {
    /// Create a new Workspace View
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            current_workspace_name: None,
        }
    }

    /// Set the workspaces to display and determine which is current
    pub fn set_workspaces(&mut self, workspaces: Vec<WorkspaceInfo>, current_root: &str) {
        // Determine current workspace by matching root path
        self.current_workspace_name = workspaces
            .iter()
            .find(|ws| ws.root_path.as_deref() == Some(current_root))
            .map(|ws| ws.name.clone())
            .or_else(|| {
                // Fallback: if only one workspace exists, it's the current one
                if workspaces.len() == 1 {
                    Some(workspaces[0].name.clone())
                } else {
                    // Last resort: assume "default"
                    Some("default".to_string())
                }
            });

        self.workspaces = workspaces;
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Get the currently selected workspace
    pub fn selected_workspace(&self) -> Option<&WorkspaceInfo> {
        self.workspaces.get(self.selected)
    }

    /// Total number of workspaces
    pub fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    /// Check if the given workspace is the current (active) workspace
    pub fn is_current(&self, ws: &WorkspaceInfo) -> bool {
        self.current_workspace_name
            .as_ref()
            .is_some_and(|name| *name == ws.name)
    }

    /// Move selection to next workspace
    pub fn select_next(&mut self) {
        let max = self.workspaces.len().saturating_sub(1);
        self.selected = navigation::select_next(self.selected, max);
    }

    /// Move selection to previous workspace
    pub fn select_prev(&mut self) {
        self.selected = navigation::select_prev(self.selected);
    }

    /// Go to first workspace
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Go to last workspace
    pub fn select_last(&mut self) {
        if !self.workspaces.is_empty() {
            self.selected = self.workspaces.len() - 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ChangeId;
    use crossterm::event::{KeyCode, KeyEvent};

    fn make_ws(name: &str, root: &str, change_id: &str, desc: &str) -> WorkspaceInfo {
        WorkspaceInfo {
            name: name.to_string(),
            root_path: Some(root.to_string()),
            change_id: ChangeId::new(change_id.to_string()),
            description: desc.to_string(),
        }
    }

    fn create_test_workspaces() -> Vec<WorkspaceInfo> {
        vec![
            make_ws("default", "/tmp/repo", "ltyxkzyp", "(no description set)"),
            make_ws(
                "feature-a",
                "/tmp/feature-ws",
                "xyzpqrst",
                "implement feature A",
            ),
        ]
    }

    #[test]
    fn test_new_workspace_view() {
        let view = WorkspaceView::new();
        assert!(view.workspaces.is_empty());
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_set_workspaces() {
        let mut view = WorkspaceView::new();
        view.set_workspaces(create_test_workspaces(), "/tmp/repo");
        assert_eq!(view.workspace_count(), 2);
        assert_eq!(view.selected, 0);
        assert_eq!(view.current_workspace_name.as_deref(), Some("default"));
    }

    #[test]
    fn test_current_workspace_detection() {
        let mut view = WorkspaceView::new();
        view.set_workspaces(create_test_workspaces(), "/tmp/feature-ws");
        assert_eq!(view.current_workspace_name.as_deref(), Some("feature-a"));
    }

    #[test]
    fn test_is_current() {
        let mut view = WorkspaceView::new();
        let workspaces = create_test_workspaces();
        view.set_workspaces(workspaces.clone(), "/tmp/repo");
        assert!(view.is_current(&workspaces[0]));
        assert!(!view.is_current(&workspaces[1]));
    }

    #[test]
    fn test_navigation() {
        let mut view = WorkspaceView::new();
        view.set_workspaces(create_test_workspaces(), "/tmp/repo");
        assert_eq!(view.selected, 0);

        view.select_next();
        assert_eq!(view.selected, 1);

        view.select_next();
        assert_eq!(view.selected, 1); // at end

        view.select_prev();
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_select_first_last() {
        let mut view = WorkspaceView::new();
        view.set_workspaces(create_test_workspaces(), "/tmp/repo");

        view.select_last();
        assert_eq!(view.selected_workspace().unwrap().name, "feature-a");

        view.select_first();
        assert_eq!(view.selected_workspace().unwrap().name, "default");
    }

    #[test]
    fn test_handle_key_add() {
        let mut view = WorkspaceView::new();
        view.set_workspaces(create_test_workspaces(), "/tmp/repo");
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('a')));
        assert!(matches!(action, WorkspaceAction::StartAdd));
    }

    #[test]
    fn test_handle_key_forget_non_current() {
        let mut view = WorkspaceView::new();
        view.set_workspaces(create_test_workspaces(), "/tmp/repo");
        view.select_next(); // select feature-a (not current)
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('D')));
        assert!(matches!(action, WorkspaceAction::Forget(ref name) if name == "feature-a"));
    }

    #[test]
    fn test_handle_key_forget_current_blocked() {
        let mut view = WorkspaceView::new();
        view.set_workspaces(create_test_workspaces(), "/tmp/repo");
        // selected is "default" which is current
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('D')));
        assert!(matches!(action, WorkspaceAction::ForgetCurrentBlocked));
    }

    #[test]
    fn test_handle_key_rename_current() {
        let mut view = WorkspaceView::new();
        view.set_workspaces(create_test_workspaces(), "/tmp/repo");
        // selected is "default" which is current → rename allowed
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('r')));
        assert!(matches!(action, WorkspaceAction::StartRename(ref name) if name == "default"));
    }

    #[test]
    fn test_handle_key_rename_non_current_blocked() {
        let mut view = WorkspaceView::new();
        view.set_workspaces(create_test_workspaces(), "/tmp/repo");
        view.select_next(); // select feature-a (not current)
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('r')));
        assert!(matches!(action, WorkspaceAction::RenameNonCurrentBlocked));
    }
}
