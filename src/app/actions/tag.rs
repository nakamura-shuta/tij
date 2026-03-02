//! Tag operations (create, delete, list)

use crate::app::state::{App, DirtyFlags, View};
use crate::ui::components::DialogCallback;

impl App {
    /// Open the tag view
    pub(crate) fn open_tag_view(&mut self) {
        match self.jj.tag_list() {
            Ok(tags) => {
                self.tag_view.set_tags(tags);
                self.go_to_view(View::Tag);
            }
            Err(e) => {
                self.set_error(format!("Failed to list tags: {}", e));
            }
        }
    }

    /// Refresh the tag view data
    pub(crate) fn refresh_tag_view(&mut self) {
        match self.jj.tag_list() {
            Ok(tags) => {
                self.tag_view.set_tags(tags);
            }
            Err(e) => {
                self.set_error(format!("Failed to list tags: {}", e));
            }
        }
    }

    /// Handle confirmed Tag dialog results
    pub(crate) fn handle_tag_dialog(&mut self, callback: DialogCallback, values: Vec<String>) {
        match callback {
            DialogCallback::TagCreate => {
                if let Some(name) = values.first()
                    && !name.is_empty()
                {
                    self.execute_tag_create(name);
                }
            }
            DialogCallback::TagDelete { name } => {
                self.execute_tag_delete(&name);
            }
            _ => {}
        }
    }

    /// Execute tag creation on @ (working copy)
    fn execute_tag_create(&mut self, name: &str) {
        match self.run_and_record("Tag create", &["tag", "set", name, "-r", "@"]) {
            Ok(_) => {
                self.notify_success(format!("Tag '{}' created", name));
                self.refresh_tag_view();
                self.mark_dirty_and_refresh_current(DirtyFlags::log());
            }
            Err(e) => {
                self.set_error(format!("Tag creation failed: {}", e));
            }
        }
    }

    /// Execute tag deletion
    fn execute_tag_delete(&mut self, name: &str) {
        match self.run_and_record("Tag delete", &["tag", "delete", name]) {
            Ok(_) => {
                self.notify_success(format!("Tag '{}' deleted", name));
                self.refresh_tag_view();
                self.mark_dirty_and_refresh_current(DirtyFlags::log());
            }
            Err(e) => {
                self.set_error(format!("Tag deletion failed: {}", e));
            }
        }
    }
}
