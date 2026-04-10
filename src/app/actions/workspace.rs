//! Workspace operations (list, add, forget, rename)

use crate::app::state::{App, DirtyFlags, View};
use crate::ui::components::{Dialog, DialogCallback};
use crate::ui::views::WorkspaceAction;

impl App {
    /// Open the workspace view
    pub(crate) fn open_workspace_view(&mut self) {
        let current_root = self.jj.workspace_root().unwrap_or_else(|_| String::new());

        match self.jj.workspace_list() {
            Ok(workspaces) => {
                self.workspace_view
                    .set_workspaces(workspaces, &current_root);
                self.go_to_view(View::Workspace);
            }
            Err(e) => {
                self.set_error(format!("Failed to list workspaces: {}", e));
            }
        }
    }

    /// Refresh the workspace view data
    pub(crate) fn refresh_workspace_view(&mut self) {
        let current_root = self.jj.workspace_root().unwrap_or_else(|_| String::new());

        match self.jj.workspace_list() {
            Ok(workspaces) => {
                self.workspace_view
                    .set_workspaces(workspaces, &current_root);
            }
            Err(e) => {
                self.set_error(format!("Failed to list workspaces: {}", e));
            }
        }
    }

    /// Handle workspace view actions
    pub(crate) fn handle_workspace_action(&mut self, action: WorkspaceAction) {
        match action {
            WorkspaceAction::None => {}
            WorkspaceAction::StartAdd => {
                self.active_dialog = Some(Dialog::input(
                    "Add Workspace",
                    "Path for new workspace (e.g., ../feature-ws)",
                    DialogCallback::WorkspaceAdd,
                ));
            }
            WorkspaceAction::Jump(change_id) => {
                self.jump_to_log(&change_id);
            }
            WorkspaceAction::ForgetCurrentBlocked => {
                self.notify_info("Cannot forget the current workspace");
            }
            WorkspaceAction::RenameNonCurrentBlocked => {
                self.notify_info("Can only rename the current workspace");
            }
            WorkspaceAction::Forget(name) => {
                self.active_dialog = Some(Dialog::confirm(
                    "Forget Workspace",
                    format!(
                        "Forget workspace '{}'?\n(disk files won't be deleted)",
                        name
                    ),
                    None,
                    DialogCallback::WorkspaceForget { name },
                ));
            }
            WorkspaceAction::StartRename(current_name) => {
                self.active_dialog = Some(Dialog::input(
                    "Rename Workspace",
                    format!("New name for '{}' workspace", current_name),
                    DialogCallback::WorkspaceRename {
                        old_name: current_name,
                    },
                ));
            }
        }
    }

    /// Handle confirmed workspace dialog results
    pub(crate) fn handle_workspace_dialog(
        &mut self,
        callback: DialogCallback,
        values: Vec<String>,
    ) {
        match callback {
            DialogCallback::WorkspaceAdd => {
                if let Some(path) = values.first()
                    && !path.is_empty()
                {
                    self.execute_workspace_add(path);
                }
            }
            DialogCallback::WorkspaceForget { name } => {
                self.execute_workspace_forget(&name);
            }
            DialogCallback::WorkspaceRename { old_name } => {
                if let Some(new_name) = values.first()
                    && !new_name.is_empty()
                {
                    self.execute_workspace_rename(&old_name, new_name);
                }
            }
            _ => {}
        }
    }

    /// Execute workspace add
    fn execute_workspace_add(&mut self, path: &str) {
        match self.run_and_record("Workspace add", &["workspace", "add", path]) {
            Ok(_) => {
                self.notify_success(format!("Workspace created at '{}'", path));
                self.refresh_workspace_view();
                self.mark_dirty_and_refresh_current(DirtyFlags::log());
            }
            Err(e) => {
                self.set_error(format!("Workspace add failed: {}", e));
            }
        }
    }

    /// Execute workspace forget
    fn execute_workspace_forget(&mut self, name: &str) {
        match self.run_and_record("Workspace forget", &["workspace", "forget", name]) {
            Ok(_) => {
                self.notify_success(format!("Workspace '{}' forgotten", name));
                self.refresh_workspace_view();
                self.mark_dirty_and_refresh_current(DirtyFlags::log());
            }
            Err(e) => {
                self.set_error(format!("Workspace forget failed: {}", e));
            }
        }
    }

    /// Execute workspace rename (current workspace only)
    fn execute_workspace_rename(&mut self, _old_name: &str, new_name: &str) {
        match self.run_and_record("Workspace rename", &["workspace", "rename", new_name]) {
            Ok(_) => {
                self.notify_success(format!("Workspace renamed to '{}'", new_name));
                self.refresh_workspace_view();
            }
            Err(e) => {
                self.set_error(format!("Workspace rename failed: {}", e));
            }
        }
    }
}
