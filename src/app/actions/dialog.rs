//! Dialog result handling (dispatch confirmed/cancelled dialog results)

use crate::jj::PushBulkMode;
use crate::ui::components::{DialogCallback, DialogResult};

use crate::app::state::App;

impl App {
    /// Handle dialog result
    ///
    /// Called when a dialog is closed.
    ///
    /// Implementation order (important):
    /// 1. Clone callback_id from active_dialog
    /// 2. Set active_dialog to None
    /// 3. Match on callback and result
    pub(crate) fn handle_dialog_result(&mut self, result: DialogResult) {
        let callback = self.active_dialog.as_ref().map(|d| d.callback_id.clone());
        self.active_dialog = None;

        let Some(callback) = callback else { return };

        match result {
            DialogResult::Cancelled => self.handle_dialog_cancel(callback),
            DialogResult::Confirmed(values) => match callback {
                // Git Push
                DialogCallback::GitPush
                | DialogCallback::GitPushChange { .. }
                | DialogCallback::GitPushRemoteSelect
                | DialogCallback::GitPushModeSelect { .. }
                | DialogCallback::GitPushBulkConfirm { .. }
                | DialogCallback::GitPushRevisions { .. }
                | DialogCallback::GitPushMultiBookmarkMode { .. } => {
                    self.handle_git_push_dialog(callback, values);
                }
                // Git Fetch
                DialogCallback::GitFetch | DialogCallback::GitFetchBranch => {
                    self.handle_git_fetch_dialog(callback, values);
                }
                // Bookmark
                DialogCallback::DeleteBookmarks
                | DialogCallback::MoveBookmark { .. }
                | DialogCallback::BookmarkJump
                | DialogCallback::BookmarkForget
                | DialogCallback::BookmarkMoveToWc { .. }
                | DialogCallback::BookmarkMoveBackwards { .. } => {
                    self.handle_bookmark_dialog(callback, values);
                }
                // Misc
                DialogCallback::OpRestore
                | DialogCallback::Track
                | DialogCallback::RestoreFile { .. }
                | DialogCallback::RestoreAll
                | DialogCallback::Revert { .. }
                | DialogCallback::SimplifyParents { .. }
                | DialogCallback::Parallelize { .. } => {
                    self.handle_misc_dialog(callback, values);
                }
            },
        }
    }

    /// Handle dialog cancellation — clean up any pending state
    fn handle_dialog_cancel(&mut self, callback: DialogCallback) {
        match callback {
            DialogCallback::GitPush => {
                self.pending_push_bookmarks.clear();
                self.push_target_remote = None;
            }
            DialogCallback::GitPushChange { .. }
            | DialogCallback::GitPushRemoteSelect
            | DialogCallback::GitPushModeSelect { .. }
            | DialogCallback::GitPushBulkConfirm { .. }
            | DialogCallback::GitPushRevisions { .. }
            | DialogCallback::GitPushMultiBookmarkMode { .. } => {
                self.push_target_remote = None;
            }
            DialogCallback::BookmarkForget => {
                self.pending_forget_bookmark = None;
            }
            // All others: no cleanup needed on cancel
            DialogCallback::DeleteBookmarks
            | DialogCallback::MoveBookmark { .. }
            | DialogCallback::OpRestore
            | DialogCallback::Track
            | DialogCallback::BookmarkJump
            | DialogCallback::GitFetch
            | DialogCallback::GitFetchBranch
            | DialogCallback::BookmarkMoveToWc { .. }
            | DialogCallback::BookmarkMoveBackwards { .. }
            | DialogCallback::RestoreFile { .. }
            | DialogCallback::RestoreAll
            | DialogCallback::Revert { .. }
            | DialogCallback::SimplifyParents { .. }
            | DialogCallback::Parallelize { .. } => {}
        }
    }

    /// Handle confirmed Git Push dialog results
    fn handle_git_push_dialog(&mut self, callback: DialogCallback, values: Vec<String>) {
        match callback {
            DialogCallback::GitPush => {
                if values.is_empty() {
                    let bookmarks = std::mem::take(&mut self.pending_push_bookmarks);
                    self.execute_push(&bookmarks);
                } else {
                    self.execute_push(&values);
                }
            }
            DialogCallback::GitPushChange { change_id } => {
                self.execute_push_change(&change_id);
            }
            DialogCallback::GitPushRemoteSelect => {
                if let Some(remote) = values.first() {
                    self.push_target_remote = Some(remote.clone());
                    self.start_push();
                }
            }
            DialogCallback::GitPushModeSelect { change_id } => {
                match values.first().map(|s| s.as_str()) {
                    Some("change") => self.start_push_change(&change_id),
                    Some("all") => self.start_push_bulk(PushBulkMode::All),
                    Some("tracked") => self.start_push_bulk(PushBulkMode::Tracked),
                    Some("deleted") => self.start_push_bulk(PushBulkMode::Deleted),
                    _ => {}
                }
            }
            DialogCallback::GitPushBulkConfirm { mode, remote } => {
                self.execute_push_bulk(mode, remote.as_deref());
            }
            DialogCallback::GitPushRevisions {
                change_id,
                bookmarks,
            } => {
                self.execute_push_revisions(&change_id, &bookmarks);
            }
            DialogCallback::GitPushMultiBookmarkMode {
                change_id,
                bookmarks,
            } => match values.first().map(|s| s.as_str()) {
                Some("revisions") => self.start_push_revisions(&change_id, &bookmarks),
                Some("individual") => self.show_individual_bookmark_select(&change_id, &bookmarks),
                _ => {}
            },
            _ => {}
        }
    }

    /// Handle confirmed Git Fetch dialog results
    fn handle_git_fetch_dialog(&mut self, callback: DialogCallback, values: Vec<String>) {
        match callback {
            DialogCallback::GitFetch => {
                if let Some(value) = values.first() {
                    if value == "__branch__" {
                        self.start_fetch_branch_select();
                    } else {
                        self.execute_fetch_with_option(value);
                    }
                }
            }
            DialogCallback::GitFetchBranch => {
                if let Some(branch) = values.first() {
                    self.execute_fetch_branch(branch);
                }
            }
            _ => {}
        }
    }

    /// Handle confirmed Bookmark dialog results
    fn handle_bookmark_dialog(&mut self, callback: DialogCallback, values: Vec<String>) {
        match callback {
            DialogCallback::DeleteBookmarks => {
                self.execute_bookmark_delete(&values);
            }
            DialogCallback::MoveBookmark { name, change_id } => {
                self.execute_bookmark_move(&name, &change_id);
            }
            DialogCallback::BookmarkJump => {
                if let Some(change_id) = values.first() {
                    self.execute_bookmark_jump(change_id);
                }
            }
            DialogCallback::BookmarkForget => {
                self.execute_bookmark_forget();
            }
            DialogCallback::BookmarkMoveToWc { name } => {
                self.execute_bookmark_move_to_wc(&name);
            }
            DialogCallback::BookmarkMoveBackwards { name } => {
                self.execute_bookmark_move_backwards(&name);
            }
            _ => {}
        }
    }

    /// Handle confirmed Misc dialog results (restore, revert, etc.)
    fn handle_misc_dialog(&mut self, callback: DialogCallback, values: Vec<String>) {
        match callback {
            DialogCallback::OpRestore => {
                // TODO: Implement op restore with dialog
            }
            DialogCallback::Track => {
                self.execute_track(&values);
            }
            DialogCallback::RestoreFile { file_path } => {
                self.execute_restore_file(&file_path);
            }
            DialogCallback::RestoreAll => {
                self.execute_restore_all();
            }
            DialogCallback::Revert { change_id } => {
                self.execute_revert(&change_id);
            }
            DialogCallback::SimplifyParents { change_id } => {
                self.execute_simplify_parents(&change_id);
            }
            DialogCallback::Parallelize { from, to } => {
                self.execute_parallelize(&from, &to);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::components::{Dialog, DialogCallback, DialogResult};

    // =========================================================================
    // Revert dialog callback tests
    // =========================================================================

    #[test]
    fn test_revert_dialog_confirmed_calls_execute_revert() {
        // Verifies that handle_dialog_result routes DialogCallback::Revert
        // to execute_revert(). Since jj is not available in test, the revert
        // command will fail, but we verify the routing by checking that
        // error_message is set (proving execute_revert was called).
        let mut app = App::new_for_test();
        app.active_dialog = Some(Dialog::confirm(
            "Revert Change",
            "Revert changes from abc12345?",
            Some("Creates a new commit that undoes these changes.".to_string()),
            DialogCallback::Revert {
                change_id: "abc12345".to_string(),
            },
        ));
        app.handle_dialog_result(DialogResult::Confirmed(vec![]));
        // execute_revert was called → jj revert fails in test env → error_message set
        assert!(
            app.error_message.is_some(),
            "execute_revert should have been called (error expected in test env)"
        );
        assert!(
            app.error_message
                .as_ref()
                .unwrap()
                .contains("Revert failed"),
            "Error should be from execute_revert, got: {}",
            app.error_message.as_ref().unwrap()
        );
    }

    #[test]
    fn test_revert_dialog_cancelled_does_nothing() {
        let mut app = App::new_for_test();
        app.active_dialog = Some(Dialog::confirm(
            "Revert Change",
            "Revert changes from abc12345?",
            None,
            DialogCallback::Revert {
                change_id: "abc12345".to_string(),
            },
        ));
        app.handle_dialog_result(DialogResult::Cancelled);
        // No action taken
        assert!(app.error_message.is_none());
        assert!(app.notification.is_none());
    }

    // =========================================================================
    // Simplify Parents dialog callback tests
    // =========================================================================

    #[test]
    fn test_simplify_parents_dialog_confirmed_calls_execute() {
        let mut app = App::new_for_test();
        app.active_dialog = Some(Dialog::confirm(
            "Simplify Parents",
            "Simplify parents for abc12345?",
            None,
            DialogCallback::SimplifyParents {
                change_id: "abc12345".to_string(),
            },
        ));
        app.handle_dialog_result(DialogResult::Confirmed(vec![]));
        // execute_simplify_parents was called → jj fails in test env → error_message set
        assert!(
            app.error_message.is_some(),
            "execute_simplify_parents should have been called (error expected in test env)"
        );
        assert!(
            app.error_message
                .as_ref()
                .unwrap()
                .contains("Simplify parents failed"),
            "Error should be from execute_simplify_parents, got: {}",
            app.error_message.as_ref().unwrap()
        );
    }

    #[test]
    fn test_simplify_parents_dialog_cancelled_does_nothing() {
        let mut app = App::new_for_test();
        app.active_dialog = Some(Dialog::confirm(
            "Simplify Parents",
            "Simplify parents for abc12345?",
            None,
            DialogCallback::SimplifyParents {
                change_id: "abc12345".to_string(),
            },
        ));
        app.handle_dialog_result(DialogResult::Cancelled);
        assert!(app.error_message.is_none());
        assert!(app.notification.is_none());
    }

    // =========================================================================
    // Parallelize dialog callback tests
    // =========================================================================

    #[test]
    fn test_parallelize_dialog_confirmed_calls_execute() {
        let mut app = App::new_for_test();
        app.active_dialog = Some(Dialog::confirm(
            "Parallelize",
            "Parallelize abc12345::xyz98765?",
            None,
            DialogCallback::Parallelize {
                from: "abc12345".to_string(),
                to: "xyz98765".to_string(),
            },
        ));
        app.handle_dialog_result(DialogResult::Confirmed(vec![]));
        // execute_parallelize was called → jj fails in test env → error_message set
        assert!(
            app.error_message.is_some(),
            "execute_parallelize should have been called (error expected in test env)"
        );
        assert!(
            app.error_message
                .as_ref()
                .unwrap()
                .contains("Parallelize failed"),
            "Error should be from execute_parallelize, got: {}",
            app.error_message.as_ref().unwrap()
        );
    }

    #[test]
    fn test_parallelize_dialog_cancelled_does_nothing() {
        let mut app = App::new_for_test();
        app.active_dialog = Some(Dialog::confirm(
            "Parallelize",
            "Parallelize abc12345::xyz98765?",
            None,
            DialogCallback::Parallelize {
                from: "abc12345".to_string(),
                to: "xyz98765".to_string(),
            },
        ));
        app.handle_dialog_result(DialogResult::Cancelled);
        assert!(app.error_message.is_none());
        assert!(app.notification.is_none());
    }
}
