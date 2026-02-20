//! Bookmark operations (create, move, delete, rename, forget, track, jump)

use crate::model::Notification;
use crate::ui::components::{Dialog, DialogCallback, SelectItem};

use crate::app::state::{App, DirtyFlags, View};

impl App {
    /// Execute bookmark creation or show move confirmation dialog
    ///
    /// First tries `jj bookmark create`. If the bookmark already exists,
    /// shows a confirmation dialog before moving it.
    pub(crate) fn execute_bookmark_create(&mut self, change_id: &str, name: &str) {
        match self.jj.bookmark_create(name, change_id) {
            Ok(_) => {
                self.notification =
                    Some(Notification::success(format!("Created bookmark: {}", name)));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_bookmarks());
            }
            Err(e) => {
                // Check if bookmark already exists - show confirmation dialog
                if is_bookmark_exists_error(&e) {
                    // Build detail with From/To info
                    let detail = self.build_bookmark_move_detail(name, change_id);
                    self.active_dialog = Some(Dialog::confirm(
                        "Move Bookmark",
                        format!("Move bookmark \"{}\" to this change?", name),
                        Some(detail),
                        DialogCallback::MoveBookmark {
                            name: name.to_string(),
                            change_id: change_id.to_string(),
                        },
                    ));
                } else {
                    self.set_error(format!("Failed to create bookmark: {}", e));
                }
            }
        }
    }

    /// Build detail text for bookmark move confirmation dialog
    ///
    /// Shows From/To positions and undo hint.
    /// First tries to find the bookmark in log_view.changes (no extra jj command),
    /// falls back to `get_change_info()` if the bookmark is outside the current view.
    fn build_bookmark_move_detail(&self, name: &str, to_change_id: &str) -> String {
        // Look up current bookmark position
        let from_info = self
            .log_view
            .changes
            .iter()
            .find(|c| !c.is_graph_only && c.bookmarks.contains(&name.to_string()))
            .map(|c| (c.change_id.clone(), c.description.clone()));

        // Fallback: query jj directly if not in current view
        let from_info = from_info.or_else(|| {
            self.jj
                .get_change_info(name)
                .ok()
                .map(|(id, _, _, _, desc)| (id, desc))
        });

        // Get destination description
        let to_desc = self
            .log_view
            .selected_change()
            .map(|c| c.display_description().to_string())
            .unwrap_or_default();

        let to_id_short = &to_change_id[..8.min(to_change_id.len())];

        match from_info {
            Some((from_id, from_desc)) => {
                format!(
                    "From: {}  {}\n  To: {}  {}\n\nCan be undone with 'u'.",
                    from_id,
                    truncate_description(&from_desc, 40),
                    to_id_short,
                    truncate_description(&to_desc, 40),
                )
            }
            None => "Can be undone with 'u'.".to_string(),
        }
    }

    /// Execute bookmark move (called after confirmation)
    pub(super) fn execute_bookmark_move(&mut self, name: &str, change_id: &str) {
        let msg = format!("Moved bookmark: {}", name);
        let result = self.jj.bookmark_set(name, change_id);
        self.run_jj_action(
            result,
            "Failed to move bookmark",
            &msg,
            DirtyFlags::log_and_bookmarks(),
        );
    }

    /// Start bookmark deletion flow (opens dialog)
    ///
    /// Gets bookmarks from the currently selected change in LogView.
    pub(crate) fn start_bookmark_delete(&mut self) {
        // Get bookmarks from currently selected change
        let (change_id, bookmarks) = match self.log_view.selected_change() {
            Some(change) => (change.change_id.clone(), change.bookmarks.clone()),
            None => return,
        };

        if bookmarks.is_empty() {
            self.notify_info("No bookmarks to delete");
            return;
        }

        // Create selection dialog
        let items: Vec<SelectItem> = bookmarks
            .iter()
            .map(|name| SelectItem {
                label: name.clone(),
                value: name.clone(),
                selected: false,
            })
            .collect();

        self.active_dialog = Some(Dialog::select(
            "Delete Bookmarks",
            format!(
                "Select bookmarks to delete from {}:",
                &change_id[..8.min(change_id.len())]
            ),
            items,
            Some("Deletions will propagate to remotes on push.".to_string()),
            DialogCallback::DeleteBookmarks,
        ));
    }

    /// Execute bookmark deletion
    pub(crate) fn execute_bookmark_delete(&mut self, names: &[String]) {
        if names.is_empty() {
            return;
        }

        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let msg = format!("Deleted bookmarks: {}", names.join(", "));
        let result = self.jj.bookmark_delete(&name_refs);
        self.run_jj_action(
            result,
            "Failed to delete bookmarks",
            &msg,
            DirtyFlags::log_and_bookmarks(),
        );
    }

    /// Execute bookmark rename
    pub(crate) fn execute_bookmark_rename(&mut self, old_name: &str, new_name: &str) {
        if old_name == new_name {
            self.notify_info("Name unchanged");
            return;
        }
        if new_name.trim().is_empty() {
            self.notify_warning("Bookmark name cannot be empty");
            return;
        }
        let msg = format!("Renamed bookmark: {} → {}", old_name, new_name);
        let result = self.jj.bookmark_rename(old_name, new_name);
        self.run_jj_action(
            result,
            "Rename failed",
            &msg,
            DirtyFlags::log_and_bookmarks(),
        );
    }

    /// Execute bookmark forget
    pub(crate) fn execute_bookmark_forget(&mut self) {
        if let Some(name) = self.pending_forget_bookmark.take() {
            let msg = format!("Forgot bookmark: {} (remote tracking removed)", name);
            let result = self.jj.bookmark_forget(&[&name]);
            self.run_jj_action(
                result,
                "Forget failed",
                &msg,
                DirtyFlags::log_and_bookmarks(),
            );
        }
    }

    /// Start bookmark move flow (shows confirmation dialog)
    pub(crate) fn start_bookmark_move(&mut self, name: &str) {
        let detail = self.build_bookmark_move_to_wc_detail(name);

        self.active_dialog = Some(Dialog::confirm(
            "Move Bookmark",
            format!("Move bookmark '{}' to @?", name),
            Some(detail),
            DialogCallback::BookmarkMoveToWc {
                name: name.to_string(),
            },
        ));
    }

    /// Build detail text for bookmark move to @
    fn build_bookmark_move_to_wc_detail(&self, _name: &str) -> String {
        let from_desc = self
            .bookmark_view
            .selected_bookmark()
            .map(|info| {
                let id = info.change_id.as_deref().unwrap_or("?");
                let desc = info.description.as_deref().unwrap_or("(no description)");
                let short_id = &id[..id.len().min(8)];
                format!("{} {}", short_id, desc)
            })
            .unwrap_or_else(|| "?".to_string());

        let to_desc = self
            .log_view
            .changes
            .iter()
            .find(|c| c.is_working_copy)
            .map(|c| {
                let desc = if c.description.is_empty() {
                    "(no description)"
                } else {
                    &c.description
                };
                let short_id = &c.change_id[..c.change_id.len().min(8)];
                format!("{} {}", short_id, desc)
            })
            .unwrap_or_else(|| "@".to_string());

        format!(
            "From: {}\n  To: {}\n\nCan be undone with 'u'.",
            from_desc, to_desc
        )
    }

    /// Execute bookmark move to @ (called after confirmation)
    pub(super) fn execute_bookmark_move_to_wc(&mut self, name: &str) {
        match self.jj.bookmark_move(name, "@") {
            Ok(_) => {
                self.notify_success(format!("Moved bookmark '{}' to @", name));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_bookmarks());
            }
            Err(e) => {
                let err_msg = format!("{}", e);
                if err_msg.contains("backwards or sideways") {
                    self.active_dialog = Some(Dialog::confirm(
                        "Move Bookmark (Force)",
                        format!(
                            "Bookmark '{}' requires backwards/sideways move.\n\
                             Allow --allow-backwards?",
                            name
                        ),
                        Some("This moves the bookmark in a non-forward direction.".to_string()),
                        DialogCallback::BookmarkMoveBackwards {
                            name: name.to_string(),
                        },
                    ));
                } else {
                    self.set_error(format!(
                        "Move failed: {}\nTry: jj bookmark move {} --to @ --allow-backwards",
                        e, name
                    ));
                }
            }
        }
    }

    /// Execute bookmark move with --allow-backwards (called after re-confirmation)
    pub(super) fn execute_bookmark_move_backwards(&mut self, name: &str) {
        let msg = format!("Moved bookmark '{}' to @ (backwards)", name);
        let result = self.jj.bookmark_move_allow_backwards(name, "@");
        self.run_jj_action(result, "Move failed", &msg, DirtyFlags::log_and_bookmarks());
    }

    /// Start track flow - show dialog with untracked remote bookmarks
    pub(crate) fn start_track(&mut self) {
        match self.jj.bookmark_list_all() {
            Ok(bookmarks) => {
                let untracked: Vec<_> = bookmarks
                    .iter()
                    .filter(|b| b.is_untracked_remote())
                    .collect();

                if untracked.is_empty() {
                    self.notify_info("No untracked remote bookmarks");
                    return;
                }

                // SelectDialog を表示
                let items: Vec<SelectItem> = untracked
                    .iter()
                    .map(|b| SelectItem {
                        label: b.full_name(),
                        value: b.full_name(),
                        selected: false,
                    })
                    .collect();

                self.active_dialog = Some(Dialog::select(
                    "Track Remote Bookmarks",
                    "Select bookmarks to track:",
                    items,
                    None,
                    DialogCallback::Track,
                ));
            }
            Err(e) => {
                self.set_error(format!("Failed to list bookmarks: {}", e));
            }
        }
    }

    /// Start bookmark jump flow - show dialog with jumpable bookmarks
    ///
    /// Shows a single-select dialog with all bookmarks that have change_id.
    /// Remote-only bookmarks (without change_id) are excluded.
    pub(crate) fn start_bookmark_jump(&mut self) {
        match self.jj.bookmark_list_with_info() {
            Ok(bookmarks) => {
                // Filter to only jumpable bookmarks (those with change_id)
                let jumpable: Vec<_> = bookmarks.iter().filter(|b| b.is_jumpable()).collect();

                if jumpable.is_empty() {
                    self.notify_info("No bookmarks available");
                    return;
                }

                // Create single-select dialog
                let items: Vec<SelectItem> = jumpable
                    .iter()
                    .map(|b| {
                        let label = b.display_label(40);
                        let value = b.change_id.clone().unwrap_or_default();
                        SelectItem {
                            label,
                            value,
                            selected: false,
                        }
                    })
                    .collect();

                self.active_dialog = Some(Dialog::select_single(
                    "Jump to Bookmark",
                    "Select bookmark:",
                    items,
                    None,
                    DialogCallback::BookmarkJump,
                ));
            }
            Err(e) => {
                self.set_error(format!("Failed to list bookmarks: {}", e));
            }
        }
    }

    /// Jump to a change in Log View (from Blame View)
    ///
    /// Resets the view stack to Log (not push-based) since the intent
    /// is "go to Log" rather than "peek and come back".
    pub(crate) fn jump_to_log(&mut self, change_id: &str) {
        // Step 1: Try to find in current log view
        if self.log_view.select_change_by_prefix(change_id) {
            let short_id = &change_id[..8.min(change_id.len())];
            self.notify_success(format!("Jumped to {} in log", short_id));
            self.pending_jump_change_id = None;
            self.previous_view = None;
            self.current_view = View::Log;
            return;
        }

        // Step 2: Check if this is a retry (user pressed J again after hint)
        if self.pending_jump_change_id.as_deref() == Some(change_id) {
            // Second press — expand revset to include the change
            self.pending_jump_change_id = None;

            let current = self.log_view.current_revset.clone();
            if let Some(base) = current.as_deref() {
                // Custom revset active: add the change to it
                let expanded = format!("{} | {}", base, change_id);
                self.refresh_log(Some(&expanded));
            } else {
                // Default view: reset to default + the target change
                let expanded = format!("ancestors({}) | {}", change_id, change_id);
                self.refresh_log(Some(&expanded));
            }

            if self.log_view.select_change_by_prefix(change_id) {
                let short_id = &change_id[..8.min(change_id.len())];
                self.notify_success(format!(
                    "Jumped to {} (revset expanded, r+Enter to reset)",
                    short_id
                ));
                self.previous_view = None;
                self.current_view = View::Log;
            } else {
                self.notify_warning("Change not found in repository");
            }
        } else {
            // First press — show hint and store pending
            self.pending_jump_change_id = Some(change_id.to_string());
            self.notify_info("Change not in current revset. Press J again to search full log");
        }
    }

    /// Execute bookmark jump - select the change in log view
    pub(crate) fn execute_bookmark_jump(&mut self, change_id: &str) {
        if self.log_view.select_change_by_id(change_id) {
            let short_id = &change_id[..8.min(change_id.len())];
            self.notify_success(format!("Jumped to {}", short_id));
        } else {
            // The change might not be visible in current revset
            self.notify_warning("Bookmark target not visible in current revset");
        }
    }

    /// Open the Bookmark View (only navigates if refresh succeeds)
    pub(crate) fn open_bookmark_view(&mut self) {
        match self.jj.bookmark_list_with_info() {
            Ok(bookmarks) => {
                self.bookmark_view.set_bookmarks(bookmarks);
                self.go_to_view(View::Bookmark);
            }
            Err(e) => {
                self.set_error(format!("Failed to list bookmarks: {}", e));
            }
        }
    }

    /// Refresh the bookmark view data
    pub(crate) fn refresh_bookmark_view(&mut self) {
        match self.jj.bookmark_list_with_info() {
            Ok(bookmarks) => {
                self.bookmark_view.set_bookmarks(bookmarks);
            }
            Err(e) => {
                self.set_error(format!("Failed to list bookmarks: {}", e));
            }
        }
    }

    /// Execute untrack for a remote bookmark
    pub(crate) fn execute_untrack(&mut self, full_name: &str) {
        let display = full_name.split('@').next().unwrap_or(full_name);
        let msg = format!("Stopped tracking: {}", display);
        let result = self.jj.bookmark_untrack(&[full_name]);
        let dirty = DirtyFlags {
            log: true,
            status: true,
            op_log: true,
            bookmarks: true,
        };
        self.run_jj_action(result, "Failed to untrack", &msg, dirty);
    }

    /// Execute track for selected bookmarks
    pub(crate) fn execute_track(&mut self, names: &[String]) {
        if names.is_empty() {
            return;
        }

        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let display = if names.len() == 1 {
            names[0].split('@').next().unwrap_or(&names[0]).to_string()
        } else {
            format!("{} bookmarks", names.len())
        };
        let msg = format!("Started tracking: {}", display);
        let result = self.jj.bookmark_track(&name_refs);
        let dirty = DirtyFlags {
            log: true,
            status: true,
            op_log: true,
            bookmarks: true,
        };
        self.run_jj_action(result, "Failed to track", &msg, dirty);
    }
}

/// Truncate a description string to max_len characters (UTF-8 safe)
///
/// Uses `chars()` to avoid panic on multibyte boundaries.
fn truncate_description(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len > 3 {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    } else {
        s.chars().take(max_len).collect()
    }
}

/// Check if a JjError indicates that a bookmark already exists
///
/// This is used to determine whether to fallback from `bookmark create` to `bookmark set`.
fn is_bookmark_exists_error(error: &crate::jj::JjError) -> bool {
    if let crate::jj::JjError::CommandFailed { stderr, .. } = error {
        let stderr_lower = stderr.to_lowercase();
        stderr_lower.contains("already exists") || stderr_lower.contains("bookmark already")
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jj::JjError;

    #[test]
    fn test_is_bookmark_exists_error_with_already_exists() {
        let error = JjError::CommandFailed {
            stderr: "Error: Bookmark 'main' already exists".to_string(),
            exit_code: 1,
        };
        assert!(is_bookmark_exists_error(&error));
    }

    #[test]
    fn test_is_bookmark_exists_error_with_bookmark_already() {
        let error = JjError::CommandFailed {
            stderr: "Error: bookmark already exists: feature".to_string(),
            exit_code: 1,
        };
        assert!(is_bookmark_exists_error(&error));
    }

    #[test]
    fn test_is_bookmark_exists_error_case_insensitive() {
        let error = JjError::CommandFailed {
            stderr: "Error: BOOKMARK ALREADY EXISTS".to_string(),
            exit_code: 1,
        };
        assert!(is_bookmark_exists_error(&error));
    }

    #[test]
    fn test_is_bookmark_exists_error_different_error() {
        let error = JjError::CommandFailed {
            stderr: "Error: Invalid revision".to_string(),
            exit_code: 1,
        };
        assert!(!is_bookmark_exists_error(&error));
    }

    #[test]
    fn test_is_bookmark_exists_error_not_command_failed() {
        let error = JjError::NotARepository;
        assert!(!is_bookmark_exists_error(&error));
    }

    #[test]
    fn test_truncate_description_short_string() {
        assert_eq!(truncate_description("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_description_exact_length() {
        assert_eq!(truncate_description("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_description_long_string() {
        assert_eq!(
            truncate_description("This is a long description text", 15),
            "This is a lo..."
        );
    }

    #[test]
    fn test_truncate_description_multibyte_japanese() {
        // Japanese characters: each is 3 bytes in UTF-8 but 1 char
        let s = "日本語のテスト文字列です";
        let result = truncate_description(s, 8);
        assert_eq!(result, "日本語のテ...");
        // Verify no panic on multibyte boundary
    }

    #[test]
    fn test_truncate_description_empty() {
        assert_eq!(truncate_description("", 10), "");
    }

    #[test]
    fn test_truncate_description_max_len_3() {
        assert_eq!(truncate_description("abcdef", 3), "abc");
    }
}
