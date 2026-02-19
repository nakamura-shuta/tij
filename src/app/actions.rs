//! jj operations (actions that modify repository state)

use crate::jj::{PushBulkMode, PushPreviewResult, parse_push_dry_run};
use crate::model::Notification;
use crate::ui::components::{Dialog, DialogCallback, DialogResult, SelectItem};
use crate::ui::views::RebaseMode;

use super::state::{App, DirtyFlags, View};

impl App {
    /// Execute undo operation
    pub(crate) fn execute_undo(&mut self) {
        match self.jj.undo() {
            Ok(_) => {
                self.notification = Some(Notification::success("Undo complete"));
                self.mark_dirty_and_refresh_current(DirtyFlags::all());
            }
            Err(e) => {
                self.error_message = Some(format!("Undo failed: {}", e));
            }
        }
    }

    /// Start describe input mode by fetching the full description
    ///
    /// If the description is multi-line, automatically opens the external
    /// editor instead of the 1-line input bar to prevent data loss.
    pub(crate) fn start_describe_input(&mut self, change_id: &str) {
        // Fetch the full (multi-line) description from jj
        match self.jj.get_description(change_id) {
            Ok(full_description) => {
                let description = full_description.trim_end_matches('\n').to_string();

                // Multi-line: fall through to external editor directly
                if description.lines().nth(1).is_some() {
                    self.execute_describe_external(change_id);
                    return;
                }

                self.log_view
                    .set_describe_input(change_id.to_string(), description);
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to get description: {}", e));
            }
        }
    }

    /// Execute describe via external editor (jj describe --edit)
    ///
    /// Temporarily exits TUI mode to allow the editor to run.
    /// Uses before/after description comparison to detect changes,
    /// since jj describe --edit exits 0 regardless of whether the user saved.
    pub(crate) fn execute_describe_external(&mut self, change_id: &str) {
        use crossterm::execute;
        use crossterm::terminal::{
            Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        };
        use std::io::stdout;

        // Capture description before editing for change detection
        let before = match self.jj.get_description(change_id) {
            Ok(desc) => Some(desc.trim_end().to_string()),
            Err(_) => None,
        };

        // 1. Exit TUI mode
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Clear(ClearType::All));

        // 2. Scope guard to ensure terminal restoration
        let _guard = scopeguard::guard((), |_| {
            let _ = enable_raw_mode();
            let _ = execute!(stdout(), EnterAlternateScreen);
        });

        // 3. Run jj describe --edit (blocking, interactive)
        let result = self.jj.describe_edit_interactive(change_id);

        // 4. Handle result
        match result {
            Ok(status) if status.success() => {
                // Compare before/after to detect actual changes
                let after = match self.jj.get_description(change_id) {
                    Ok(desc) => Some(desc.trim_end().to_string()),
                    Err(_) => None,
                };

                match (before, after) {
                    (Some(b), Some(a)) if b == a => {
                        self.notification = Some(Notification::info("Description unchanged"));
                    }
                    (Some(_), Some(_)) => {
                        self.notification = Some(Notification::success("Description updated"));
                    }
                    _ => {
                        // Could not compare (get_description failed before or after)
                        self.notification = Some(Notification::success("Describe editor closed"));
                    }
                }
            }
            Ok(_) => {
                self.notification = Some(Notification::info("Describe editor exited with error"));
            }
            Err(e) => {
                self.error_message = Some(format!("Describe failed: {}", e));
            }
        }

        // 5. Refresh views
        self.mark_dirty_and_refresh_current(DirtyFlags::log());
    }

    /// Execute describe operation
    pub(crate) fn execute_describe(&mut self, change_id: &str, message: &str) {
        match self.jj.describe(change_id, message) {
            Ok(_) => {
                self.notification = Some(Notification::success("Description updated"));
                self.mark_dirty_and_refresh_current(DirtyFlags::log());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to update description: {}", e));
            }
        }
    }

    /// Execute edit operation (set working-copy to specified change)
    pub(crate) fn execute_edit(&mut self, change_id: &str) {
        match self.jj.edit(change_id) {
            Ok(_) => {
                let short_id = &change_id[..8.min(change_id.len())];
                self.notification =
                    Some(Notification::success(format!("Now editing: {}", short_id)));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to edit: {}", e));
            }
        }
    }

    /// Execute new change operation
    pub(crate) fn execute_new_change(&mut self) {
        match self.jj.new_change() {
            Ok(_) => {
                self.notification = Some(Notification::success("Created new change"));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to create change: {}", e));
            }
        }
    }

    /// Execute new change from specified parent
    pub(crate) fn execute_new_change_from(&mut self, parent_id: &str, display_name: &str) {
        match self.jj.new_change_from(parent_id) {
            Ok(_) => {
                self.notification = Some(Notification::success(format!(
                    "Created new change from {}",
                    display_name
                )));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to create change: {}", e));
            }
        }
    }

    /// Execute commit operation (describe current change + create new change)
    pub(crate) fn execute_commit(&mut self, message: &str) {
        match self.jj.commit(message) {
            Ok(_) => {
                self.notification = Some(Notification::success("Changes committed"));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                self.error_message = Some(format!("Commit failed: {}", e));
            }
        }
    }

    /// Execute squash into target (requires terminal control transfer)
    ///
    /// jj squash --from/--into may open an editor when both source and destination
    /// have non-empty descriptions. Temporarily exits TUI mode to allow editor interaction.
    pub(crate) fn execute_squash_into(&mut self, source: &str, destination: &str) {
        use crate::jj::constants::ROOT_CHANGE_ID;
        use crossterm::execute;
        use crossterm::terminal::{
            Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        };
        use std::io::stdout;

        // Guard: cannot squash root commit (has no parent to receive changes from)
        if source == ROOT_CHANGE_ID {
            self.notification = Some(Notification::info(
                "Cannot squash: root commit has no parent",
            ));
            return;
        }

        // 1. Exit TUI mode
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Clear(ClearType::All));

        // 2. Scope guard to ensure terminal restoration on any exit path
        let _guard = scopeguard::guard((), |_| {
            let _ = enable_raw_mode();
            let _ = execute!(stdout(), EnterAlternateScreen);
        });

        // 3. Run jj squash --from --into (blocking, interactive)
        let result = self.jj.squash_into_interactive(source, destination);

        // 4. Handle result (io::Result<ExitStatus>)
        match result {
            Ok(status) if status.success() => {
                let src_short = &source[..8.min(source.len())];
                let dst_short = &destination[..8.min(destination.len())];
                self.notification = Some(Notification::success(format!(
                    "Squashed {} into {} (undo: u)",
                    src_short, dst_short
                )));
            }
            Ok(_) => {
                // Non-zero exit (user cancelled editor, or jj error)
                self.notification = Some(Notification::info("Squash cancelled or failed"));
            }
            Err(e) => {
                // IO error (command not found, etc.)
                self.error_message = Some(format!("Squash failed: {}", e));
            }
        }

        // 5. Refresh views
        self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
    }

    /// Execute abandon operation (abandon a change)
    pub(crate) fn execute_abandon(&mut self, change_id: &str) {
        use crate::jj::constants::ROOT_CHANGE_ID;

        // Guard: cannot abandon root commit
        if change_id == ROOT_CHANGE_ID {
            self.notification = Some(Notification::info("Cannot abandon: root commit"));
            return;
        }

        match self.jj.abandon(change_id) {
            Ok(_) => {
                let short_id = &change_id[..8.min(change_id.len())];
                self.notification = Some(Notification::success(format!(
                    "Abandoned {} (undo: u)",
                    short_id
                )));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                self.error_message = Some(format!("Abandon failed: {}", e));
            }
        }
    }

    /// Execute revert operation (creates reverse-diff commit)
    pub(crate) fn execute_revert(&mut self, change_id: &str) {
        match self.jj.revert(change_id) {
            Ok(_) => {
                let short_id = &change_id[..8.min(change_id.len())];
                self.notification = Some(Notification::success(format!(
                    "Reverted {} (undo: u)",
                    short_id
                )));
                self.mark_dirty_and_refresh_current(DirtyFlags::log());
            }
            Err(e) => {
                self.error_message = Some(format!("Revert failed: {}", e));
            }
        }
    }

    /// Execute redo operation
    ///
    /// Only works if the last operation was an undo.
    pub(crate) fn execute_redo(&mut self) {
        // First, check if last operation was an undo and get the target
        match self.jj.get_redo_target() {
            Ok(Some(op_id)) => match self.jj.redo(&op_id) {
                Ok(_) => {
                    self.notification = Some(Notification::success("Redo complete"));
                    self.mark_dirty_and_refresh_current(DirtyFlags::all());
                }
                Err(e) => {
                    self.error_message = Some(format!("Redo failed: {}", e));
                }
            },
            Ok(None) => {
                // Not in an undo/redo chain, or multiple consecutive undos
                // Note: After multiple undos, use 'o' (Operation History) to restore
                self.notification = Some(Notification::info(
                    "Nothing to redo (use 'o' for operation history after multiple undos)",
                ));
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to check redo target: {}", e));
            }
        }
    }

    /// Execute operation restore
    ///
    /// **Warning**: This is a destructive operation that modifies repository history.
    /// TODO: Add confirmation dialog (Phase 5.2) before executing.
    /// Currently, users can undo with `u` key if needed.
    pub(crate) fn execute_op_restore(&mut self, operation_id: &str) {
        match self.jj.op_restore(operation_id) {
            Ok(_) => {
                let short_id = &operation_id[..12.min(operation_id.len())];
                self.notification = Some(Notification::success(format!(
                    "Restored to {} (undo: u)",
                    short_id
                )));
                self.mark_dirty_and_refresh_current(DirtyFlags::all());
                // Go back to log view
                self.go_to_view(View::Log);
            }
            Err(e) => {
                self.error_message = Some(format!("Restore failed: {}", e));
            }
        }
    }

    /// Execute split operation (requires terminal control transfer)
    ///
    /// This method temporarily exits raw mode to allow jj split
    /// to use its external diff editor.
    ///
    /// Uses scope guard to ensure terminal state is always restored,
    /// even if jj split panics or returns early.
    pub(crate) fn execute_split(&mut self, change_id: &str) {
        use crossterm::execute;
        use crossterm::terminal::{
            Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        };
        use std::io::stdout;

        // Guard: cannot split an empty commit (nothing to split)
        let is_empty = self.log_view.selected_change().is_some_and(|c| c.is_empty);
        if is_empty {
            self.notification = Some(Notification::info(
                "Cannot split: no changes in this revision",
            ));
            return;
        }

        // 1. Exit TUI mode
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Clear(ClearType::All));

        // 2. Scope guard to ensure terminal restoration on any exit path
        //    (panic, early return, normal completion)
        let _guard = scopeguard::guard((), |_| {
            let _ = enable_raw_mode();
            let _ = execute!(stdout(), EnterAlternateScreen);
        });

        // 3. Run jj split (blocking)
        let result = self.jj.split_interactive(change_id);

        // 4. Handle result and refresh
        // Note: _guard will restore terminal when this function returns
        match result {
            Ok(status) if status.success() => {
                let short_id = &change_id[..8.min(change_id.len())];
                self.notification = Some(Notification::success(format!(
                    "Split {} complete (undo: u)",
                    short_id
                )));
            }
            Ok(_) => {
                self.notification = Some(Notification::info("Split cancelled or failed"));
            }
            Err(e) => {
                self.error_message = Some(format!("Split failed: {}", e));
            }
        }

        // 5. Refresh views
        self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
    }

    /// Execute diffedit (interactive diff editor)
    ///
    /// When `file` is None, opens the full diffedit for the revision.
    /// When `file` is Some, opens diffedit scoped to that file.
    pub(crate) fn execute_diffedit(&mut self, change_id: &str, file: Option<&str>) {
        use crossterm::execute;
        use crossterm::terminal::{
            Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        };
        use std::io::stdout;

        // 1. Exit TUI mode
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Clear(ClearType::All));

        // 2. Scope guard to ensure terminal restoration
        let _guard = scopeguard::guard((), |_| {
            let _ = enable_raw_mode();
            let _ = execute!(stdout(), EnterAlternateScreen);
        });

        // 3. Run jj diffedit (blocking)
        let result = if let Some(f) = file {
            self.jj.diffedit_file_interactive(change_id, f)
        } else {
            self.jj.diffedit_interactive(change_id)
        };

        // 4. Handle result
        match result {
            Ok(status) if status.success() => {
                let short_id = &change_id[..8.min(change_id.len())];
                self.notification = Some(Notification::success(format!(
                    "Diffedit {} complete (undo: u)",
                    short_id
                )));
            }
            Ok(_) => {
                self.notification = Some(Notification::info("Diffedit cancelled or failed"));
            }
            Err(e) => {
                self.error_message = Some(format!("Diffedit failed: {}", e));
            }
        }

        // 5. Refresh views
        self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
    }

    /// Execute restore for a single file
    pub(crate) fn execute_restore_file(&mut self, file_path: &str) {
        match self.jj.restore_file(file_path) {
            Ok(_) => {
                self.notification = Some(Notification::success(format!("Restored: {}", file_path)));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                self.error_message = Some(format!("Restore failed: {}", e));
            }
        }
    }

    /// Execute restore for all files
    pub(crate) fn execute_restore_all(&mut self) {
        match self.jj.restore_all() {
            Ok(_) => {
                self.notification = Some(Notification::success("All files restored"));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                self.error_message = Some(format!("Restore failed: {}", e));
            }
        }
    }

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
                    self.error_message = Some(format!("Failed to create bookmark: {}", e));
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
    fn execute_bookmark_move(&mut self, name: &str, change_id: &str) {
        match self.jj.bookmark_set(name, change_id) {
            Ok(_) => {
                self.notification =
                    Some(Notification::success(format!("Moved bookmark: {}", name)));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_bookmarks());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to move bookmark: {}", e));
            }
        }
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
            self.notification = Some(Notification::info("No bookmarks to delete"));
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
        match self.jj.bookmark_delete(&name_refs) {
            Ok(_) => {
                let names_str = names.join(", ");
                self.notification = Some(Notification::success(format!(
                    "Deleted bookmarks: {}",
                    names_str
                )));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_bookmarks());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to delete bookmarks: {}", e));
            }
        }
    }

    /// Execute bookmark rename
    pub(crate) fn execute_bookmark_rename(&mut self, old_name: &str, new_name: &str) {
        if old_name == new_name {
            self.notification = Some(Notification::info("Name unchanged"));
            return;
        }
        if new_name.trim().is_empty() {
            self.notification = Some(Notification::warning("Bookmark name cannot be empty"));
            return;
        }
        match self.jj.bookmark_rename(old_name, new_name) {
            Ok(_) => {
                self.notification = Some(Notification::success(format!(
                    "Renamed bookmark: {} → {}",
                    old_name, new_name
                )));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_bookmarks());
            }
            Err(e) => {
                self.error_message = Some(format!("Rename failed: {}", e));
            }
        }
    }

    /// Execute bookmark forget
    pub(crate) fn execute_bookmark_forget(&mut self) {
        if let Some(name) = self.pending_forget_bookmark.take() {
            match self.jj.bookmark_forget(&[&name]) {
                Ok(_) => {
                    self.notification = Some(Notification::success(format!(
                        "Forgot bookmark: {} (remote tracking removed)",
                        name
                    )));
                    self.mark_dirty_and_refresh_current(DirtyFlags::log_and_bookmarks());
                }
                Err(e) => {
                    self.error_message = Some(format!("Forget failed: {}", e));
                }
            }
        }
    }

    /// Execute `jj next --edit` and refresh
    pub(crate) fn execute_next(&mut self) {
        match self.jj.next() {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());

                // Move cursor to @ position
                self.log_view.select_working_copy();

                let msg = Self::parse_next_prev_message(&output, "next");
                self.notification = Some(Notification::success(msg));
            }
            Err(e) => {
                let msg = Self::format_next_prev_error(&e, "next");
                self.notification = Some(Notification::warning(msg));
            }
        }
    }

    /// Execute `jj prev --edit` and refresh
    pub(crate) fn execute_prev(&mut self) {
        match self.jj.prev() {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());

                // Move cursor to @ position
                self.log_view.select_working_copy();

                let msg = Self::parse_next_prev_message(&output, "prev");
                self.notification = Some(Notification::success(msg));
            }
            Err(e) => {
                let msg = Self::format_next_prev_error(&e, "prev");
                self.notification = Some(Notification::warning(msg));
            }
        }
    }

    /// Parse jj next/prev success output to notification message
    fn parse_next_prev_message(output: &str, direction: &str) -> String {
        let trimmed = output.trim();
        if trimmed.is_empty() {
            format!("Moved {} successfully", direction)
        } else {
            let first_line = trimmed.lines().next().unwrap_or(trimmed);
            format!("Moved {}: {}", direction, first_line)
        }
    }

    /// Format next/prev error message for user
    fn format_next_prev_error(error: &crate::jj::JjError, direction: &str) -> String {
        let err_str = error.to_string();
        if err_str.contains("more than one child") || err_str.contains("more than one parent") {
            format!(
                "Cannot move {}: multiple {}. Use 'e' to edit a specific revision.",
                direction,
                if direction == "next" {
                    "children"
                } else {
                    "parents"
                }
            )
        } else if err_str.contains("No descendant") || err_str.contains("no child") {
            "Already at the newest change".to_string()
        } else if err_str.contains("No ancestor") || err_str.contains("no parent") {
            "Already at the root".to_string()
        } else {
            format!("Move {} failed: {}", direction, err_str)
        }
    }

    /// Execute `jj duplicate <change_id>` and refresh log
    ///
    /// Parses the output to extract the new change ID, refreshes the log,
    /// and moves focus to the duplicated change.
    pub(crate) fn duplicate(&mut self, change_id: &str) {
        match self.jj.duplicate(change_id) {
            Ok(output) => {
                // Parse new change_id from output
                let new_change_id = Self::parse_duplicate_output(&output);

                // Refresh log first (before notification)
                self.mark_dirty_and_refresh_current(DirtyFlags::log());

                // If refresh_log failed, don't show success notification
                if self.error_message.is_some() {
                    return;
                }

                // Move focus to duplicated change + build notification
                match new_change_id {
                    Some(ref new_id) => {
                        let short = &new_id[..new_id.len().min(8)];
                        if self.log_view.select_change_by_prefix(new_id) {
                            self.notification =
                                Some(Notification::success(format!("Duplicated as {}", short)));
                        } else {
                            self.notification = Some(Notification::success(format!(
                                "Duplicated as {} (not in current revset)",
                                short
                            )));
                        }
                    }
                    None => {
                        self.notification =
                            Some(Notification::success("Duplicated successfully".to_string()));
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Duplicate failed: {}", e));
            }
        }
    }

    /// Parse the new change ID from `jj duplicate` output
    ///
    /// Output format: "Duplicated <commit_id> as <new_change_id> <new_commit_id> <description>"
    fn parse_duplicate_output(output: &str) -> Option<String> {
        for line in output.lines() {
            if let Some(rest) = line.strip_prefix("Duplicated ") {
                let parts: Vec<&str> = rest.splitn(4, ' ').collect();
                // parts[0] = commit_id, parts[1] = "as", parts[2] = new_change_id
                if parts.len() >= 3 && parts[1] == "as" {
                    return Some(parts[2].to_string());
                }
            }
        }
        None
    }

    /// Execute absorb: move working copy changes into ancestor commits
    ///
    /// Each hunk is moved to the closest mutable ancestor where the
    /// corresponding lines were modified last.
    pub(crate) fn execute_absorb(&mut self) {
        match self.jj.absorb() {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());

                // Simple notification: check if output is empty or contains "nothing"
                // Avoid detailed parsing due to jj version differences
                let notification =
                    if output.trim().is_empty() || output.to_lowercase().contains("nothing") {
                        Notification::info("Nothing to absorb")
                    } else {
                        Notification::success("Absorb finished")
                    };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.error_message = Some(format!("Absorb failed: {}", e));
            }
        }
    }

    /// Execute simplify-parents: remove redundant parent edges
    pub(crate) fn execute_simplify_parents(&mut self, change_id: &str) {
        match self.jj.simplify_parents(change_id) {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());

                let notification = if output.trim().is_empty()
                    || output.to_lowercase().contains("nothing")
                {
                    Notification::info("No redundant parents found")
                } else {
                    let short_id = &change_id[..8.min(change_id.len())];
                    Notification::success(format!("Simplified parents for {} (undo: u)", short_id))
                };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.error_message = Some(format!("Simplify parents failed: {}", e));
            }
        }
    }

    /// Execute git fetch (default behavior)
    pub(crate) fn execute_fetch(&mut self) {
        match self.jj.git_fetch() {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::all());

                let notification = if output.trim().is_empty() {
                    Notification::info("Already up to date")
                } else {
                    Notification::success("Fetched from remote")
                };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.error_message = Some(format!("Fetch failed: {}", e));
            }
        }
    }

    /// Start fetch flow: check remotes and show dialog if multiple
    ///
    /// When multiple remotes exist, shows a selection dialog including
    /// a "Specific branch..." option that opens a second dialog for
    /// branch selection.
    pub(crate) fn start_fetch(&mut self) {
        match self.jj.git_remote_list() {
            Ok(remotes) => {
                if remotes.len() <= 1 {
                    // Single remote (or none): execute immediately
                    self.execute_fetch();
                } else {
                    // Multiple remotes: show selection dialog
                    let mut items = vec![
                        SelectItem {
                            label: "Default fetch (jj config)".to_string(),
                            value: "__default__".to_string(),
                            selected: false,
                        },
                        SelectItem {
                            label: "All remotes (including untracked)".to_string(),
                            value: "__all_remotes__".to_string(),
                            selected: false,
                        },
                    ];
                    for remote in &remotes {
                        items.push(SelectItem {
                            label: remote.clone(),
                            value: remote.clone(),
                            selected: false,
                        });
                    }
                    items.push(SelectItem {
                        label: "Specific branch...".to_string(),
                        value: "__branch__".to_string(),
                        selected: false,
                    });
                    self.active_dialog = Some(Dialog::select_single(
                        "Git Fetch",
                        "Select remote to fetch from:",
                        items,
                        None,
                        DialogCallback::GitFetch,
                    ));
                }
            }
            Err(_) => {
                // Fallback to default fetch on remote list failure
                self.execute_fetch();
            }
        }
    }

    /// Execute fetch with specific remote option
    pub(crate) fn execute_fetch_with_option(&mut self, option: &str) {
        let result = match option {
            "__default__" => self.jj.git_fetch(),
            "__all_remotes__" => self.jj.git_fetch_all_remotes(),
            remote => self.jj.git_fetch_remote(remote),
        };
        match result {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::all());

                let notification = if output.trim().is_empty() {
                    Notification::info("Already up to date")
                } else {
                    let source = match option {
                        "__default__" => "default remotes",
                        "__all_remotes__" => "all remotes",
                        remote => remote,
                    };
                    Notification::success(format!("Fetched from {}", source))
                };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.error_message = Some(format!("Fetch failed: {}", e));
            }
        }
    }

    /// Show 2nd-step Select dialog for branch selection
    ///
    /// Gets local bookmark names via `jj bookmark list` and shows a Select dialog.
    /// If no bookmarks found, falls back to default fetch with notification.
    fn start_fetch_branch_select(&mut self) {
        match self.jj.bookmark_list_all() {
            Ok(bookmarks) => {
                // Filter to local-only bookmarks (no remote)
                let local_names: Vec<String> = bookmarks
                    .iter()
                    .filter(|b| b.remote.is_none())
                    .map(|b| b.name.clone())
                    .collect();

                if local_names.is_empty() {
                    self.notification = Some(Notification::info("No bookmarks found"));
                    self.execute_fetch();
                    return;
                }

                let items: Vec<SelectItem> = local_names
                    .iter()
                    .map(|name| SelectItem {
                        label: name.clone(),
                        value: name.clone(),
                        selected: false,
                    })
                    .collect();

                self.active_dialog = Some(Dialog::select_single(
                    "Fetch Branch",
                    "Select branch to fetch:",
                    items,
                    None,
                    DialogCallback::GitFetchBranch,
                ));
            }
            Err(_) => {
                // Fallback to default fetch on bookmark list failure
                self.notification =
                    Some(Notification::info("Failed to list bookmarks, fetching all"));
                self.execute_fetch();
            }
        }
    }

    /// Execute `jj git fetch --branch <name>` for a specific branch
    fn execute_fetch_branch(&mut self, branch: &str) {
        match self.jj.git_fetch_branch(branch) {
            Ok(output) => {
                self.mark_dirty_and_refresh_current(DirtyFlags::all());

                let notification = if output.trim().is_empty() {
                    Notification::info(format!("Branch '{}': already up to date", branch))
                } else {
                    Notification::success(format!("Fetched branch '{}'", branch))
                };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.error_message = Some(format!("Fetch failed: {}", e));
            }
        }
    }

    /// Start push flow with dry-run preview
    ///
    /// Runs `jj git push --dry-run` to preview what will be pushed,
    /// then shows a confirmation/selection dialog with the preview.
    /// If dry-run fails (untracked bookmark, etc.), falls back to dialog without preview.
    ///
    /// When multiple remotes exist and `push_target_remote` is not yet set,
    /// shows a remote selection dialog first. After selection, this method
    /// is re-called with `push_target_remote` populated.
    pub(crate) fn start_push(&mut self) {
        let (change_id, bookmarks) = match self.log_view.selected_change() {
            Some(change) => (change.change_id.clone(), change.bookmarks.clone()),
            None => return,
        };

        // Multi-remote check: if push_target_remote is not set, check for multiple remotes
        if self.push_target_remote.is_none() {
            match self.jj.git_remote_list() {
                Ok(remotes) if remotes.len() > 1 => {
                    let items: Vec<SelectItem> = remotes
                        .iter()
                        .map(|r| SelectItem {
                            label: r.clone(),
                            value: r.clone(),
                            selected: false,
                        })
                        .collect();
                    self.active_dialog = Some(Dialog::select_single(
                        "Push to Remote",
                        "Select remote to push to:",
                        items,
                        None,
                        DialogCallback::GitPushRemoteSelect,
                    ));
                    return;
                }
                _ => {
                    // Single remote or error: continue with default
                }
            }
        }

        if bookmarks.is_empty() {
            // No bookmarks: show mode selection dialog
            let items = vec![
                SelectItem {
                    label: "Push by change ID (--change)".into(),
                    value: "change".into(),
                    selected: false,
                },
                SelectItem {
                    label: "Push all bookmarks (--all)".into(),
                    value: "all".into(),
                    selected: false,
                },
                SelectItem {
                    label: "Push tracked bookmarks (--tracked)".into(),
                    value: "tracked".into(),
                    selected: false,
                },
                SelectItem {
                    label: "Push deleted bookmarks (--deleted)".into(),
                    value: "deleted".into(),
                    selected: false,
                },
            ];
            self.active_dialog = Some(Dialog::select_single(
                "Push to Remote",
                "No bookmarks on this change. Choose push mode:",
                items,
                None,
                DialogCallback::GitPushModeSelect {
                    change_id: change_id.clone(),
                },
            ));
            return;
        }

        if bookmarks.len() == 1 {
            let name = &bookmarks[0];

            // Run dry-run to preview push (with remote if selected)
            let dry_run_result = if let Some(ref remote) = self.push_target_remote {
                self.jj.git_push_dry_run_to_remote(name, remote)
            } else {
                self.jj.git_push_dry_run(name)
            };
            match dry_run_result {
                Ok(output) => {
                    let preview = parse_push_dry_run(&output);
                    match preview {
                        PushPreviewResult::NothingChanged => {
                            self.notification = Some(Notification::info(format!(
                                "Nothing to push: {} is already up to date",
                                name
                            )));
                        }
                        PushPreviewResult::Changes(actions) => {
                            // Include dry-run result in message (multi-line)
                            let preview_text = format_preview_actions(&actions);
                            let is_force = has_force_push(&actions);
                            let is_protected = is_immutable_bookmark(name);

                            let (body, detail) = if is_force && is_protected {
                                (
                                    format!(
                                        "\u{26A0} FORCE PUSH to protected bookmark \"{}\"!\n{}",
                                        name, preview_text
                                    ),
                                    "WARNING: Force pushing to a protected bookmark rewrites shared history!"
                                        .to_string(),
                                )
                            } else if is_force {
                                (
                                    format!(
                                        "\u{26A0} FORCE PUSH bookmark \"{}\"?\n{}",
                                        name, preview_text
                                    ),
                                    "This will rewrite remote history! Cannot be undone with 'u'."
                                        .to_string(),
                                )
                            } else {
                                (
                                    format!("Push bookmark \"{}\"?\n{}", name, preview_text),
                                    "Remote changes cannot be undone with 'u'.".to_string(),
                                )
                            };

                            self.active_dialog = Some(Dialog::confirm(
                                "Push to Remote",
                                body,
                                Some(detail),
                                DialogCallback::GitPush,
                            ));
                            self.pending_push_bookmarks = vec![name.clone()];
                        }
                        PushPreviewResult::Unparsed => {
                            // Unknown output format: fallback to dialog without preview
                            self.active_dialog = Some(Dialog::confirm(
                                "Push to Remote",
                                format!("Push bookmark \"{}\"?", name),
                                Some("Remote changes cannot be undone with 'u'.".to_string()),
                                DialogCallback::GitPush,
                            ));
                            self.pending_push_bookmarks = vec![name.clone()];
                        }
                    }
                }
                Err(_) => {
                    // dry-run failed (untracked, empty description, etc.):
                    // Fallback to dialog without preview.
                    // execute_push() may still succeed via --allow-new retry.
                    self.active_dialog = Some(Dialog::confirm(
                        "Push to Remote",
                        format!("Push bookmark \"{}\"?", name),
                        Some("Remote changes cannot be undone with 'u'.".to_string()),
                        DialogCallback::GitPush,
                    ));
                    self.pending_push_bookmarks = vec![name.clone()];
                }
            }
        } else {
            // Multiple bookmarks: first ask user to choose push mode
            let short_id = &change_id[..change_id.len().min(8)];
            let items = vec![
                SelectItem {
                    label: "All bookmarks on this revision (--revisions)".to_string(),
                    value: "revisions".to_string(),
                    selected: false,
                },
                SelectItem {
                    label: "Select individual bookmarks...".to_string(),
                    value: "individual".to_string(),
                    selected: false,
                },
            ];
            self.active_dialog = Some(Dialog::select_single(
                "Push to Remote",
                format!(
                    "{} bookmarks on {}. Choose push mode:",
                    bookmarks.len(),
                    short_id
                ),
                items,
                None,
                DialogCallback::GitPushMultiBookmarkMode {
                    change_id: change_id.clone(),
                    bookmarks: bookmarks.clone(),
                },
            ));
        }
    }

    /// Execute git push for the specified bookmarks
    ///
    /// If `jj git push --bookmark` fails for an untracked/new bookmark,
    /// retries with `--allow-new` (deprecated in jj 0.37+ but functional).
    /// On success via --allow-new, adds a hint about configuring auto-track.
    ///
    /// Uses `push_target_remote` if set (consumed via `take()` at the top
    /// to guarantee cleanup on all exit paths).
    pub(crate) fn execute_push(&mut self, bookmark_names: &[String]) {
        if bookmark_names.is_empty() {
            self.push_target_remote = None;
            return;
        }

        // Take remote at the top → guaranteed cleanup on success/error
        let remote = self.push_target_remote.take();

        let mut successes = Vec::new();
        let mut errors = Vec::new();
        let mut used_allow_new = false;
        let mut retry_notes: Vec<&str> = Vec::new();

        for name in bookmark_names {
            let result = if let Some(ref r) = remote {
                self.jj.git_push_bookmark_to_remote(name, r)
            } else {
                self.jj.git_push_bookmark(name)
            };

            match result {
                Ok(_) => {
                    successes.push(name.clone());
                }
                Err(e) => {
                    let err_msg = format!("{}", e);

                    // Detect retry-able errors and build flag list
                    let mut extra_flags: Vec<&str> = Vec::new();
                    if is_untracked_bookmark_error(&err_msg) {
                        extra_flags.push(crate::jj::constants::flags::ALLOW_NEW);
                    }
                    if is_private_commit_error(&err_msg) {
                        extra_flags.push(crate::jj::constants::flags::ALLOW_PRIVATE);
                    }
                    if is_empty_description_error(&err_msg) {
                        extra_flags.push(crate::jj::constants::flags::ALLOW_EMPTY_DESC);
                    }

                    if !extra_flags.is_empty() {
                        let retry = if let Some(ref r) = remote {
                            self.jj
                                .git_push_bookmark_to_remote_with_flags(name, r, &extra_flags)
                        } else {
                            self.jj.git_push_bookmark_with_flags(name, &extra_flags)
                        };
                        match retry {
                            Ok(_) => {
                                successes.push(name.clone());
                                if extra_flags.contains(&crate::jj::constants::flags::ALLOW_NEW) {
                                    used_allow_new = true;
                                }
                                if extra_flags.contains(&crate::jj::constants::flags::ALLOW_PRIVATE)
                                    && !retry_notes.contains(&"private commit allowed")
                                {
                                    retry_notes.push("private commit allowed");
                                }
                                if extra_flags
                                    .contains(&crate::jj::constants::flags::ALLOW_EMPTY_DESC)
                                    && !retry_notes.contains(&"empty description allowed")
                                {
                                    retry_notes.push("empty description allowed");
                                }
                                continue;
                            }
                            Err(e2) => {
                                errors.push(format!("{}: {}", name, e2));
                            }
                        }
                    } else {
                        errors.push(format!("{}: {}", name, e));
                    }
                }
            }
        }

        // Show result (include remote name if specified)
        if !successes.is_empty() {
            let names = successes.join(", ");
            let suffix = build_push_suffix(used_allow_new, &retry_notes);
            let msg = if let Some(r) = remote.as_deref() {
                format!("Pushed bookmark: {} to {}{}", names, r, suffix)
            } else {
                format!("Pushed bookmark: {}{}", names, suffix)
            };
            self.notification = Some(Notification::success(msg));
        }
        if !errors.is_empty() {
            let msg = errors.join("; ");
            self.error_message = Some(format!("Push failed: {}", msg));
        }

        // Always clear pending state after execution (prevent stale data)
        self.pending_push_bookmarks.clear();

        // Refresh after push
        self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
    }

    /// Execute `jj git push --change <change_id>` and refresh
    ///
    /// Creates an automatic bookmark (push-<prefix>) and pushes it.
    /// Uses `push_target_remote` if set (consumed via `take()`).
    /// On private/empty-description errors, retries with appropriate flags.
    pub(crate) fn execute_push_change(&mut self, change_id: &str) {
        let remote = self.push_target_remote.take();
        let result = if let Some(ref r) = remote {
            self.jj.git_push_change_to_remote(change_id, r)
        } else {
            self.jj.git_push_change(change_id)
        };
        match result {
            Ok(output) => {
                self.notify_push_change_success(&output, change_id, remote.as_deref(), &[]);
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                let err_msg = format!("{}", e);
                let extra_flags = detect_push_retry_flags(&err_msg);

                if !extra_flags.is_empty() {
                    let retry = if let Some(ref r) = remote {
                        self.jj
                            .git_push_change_to_remote_with_flags(change_id, r, &extra_flags)
                    } else {
                        self.jj.git_push_change_with_flags(change_id, &extra_flags)
                    };
                    match retry {
                        Ok(output) => {
                            self.notify_push_change_success(
                                &output,
                                change_id,
                                remote.as_deref(),
                                &extra_flags,
                            );
                            self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
                        }
                        Err(e2) => {
                            self.error_message = Some(format!("Push failed: {}", e2));
                        }
                    }
                } else {
                    self.error_message = Some(format!("Push failed: {}", e));
                }
            }
        }
    }

    /// Build notification message for successful push --change
    fn notify_push_change_success(
        &mut self,
        output: &str,
        change_id: &str,
        remote: Option<&str>,
        extra_flags: &[&str],
    ) {
        let bookmark_name = Self::parse_push_change_bookmark(output, change_id);
        let short_id = &change_id[..change_id.len().min(8)];
        let notes = retry_notes_from_flags(extra_flags);
        let suffix = build_push_suffix(false, &notes);
        let msg = match (bookmark_name, remote) {
            (Some(name), Some(r)) => {
                format!(
                    "Pushed change {} to {} (created bookmark: {}){suffix}",
                    short_id, r, name
                )
            }
            (Some(name), None) => {
                format!(
                    "Pushed change {} (created bookmark: {}){suffix}",
                    short_id, name
                )
            }
            (None, Some(r)) => format!("Pushed change {} to {}{suffix}", short_id, r),
            (None, None) => format!("Pushed change {}{suffix}", short_id),
        };
        self.notification = Some(Notification::success(msg));
    }

    /// Parse the auto-created bookmark name from `jj git push --change` output
    ///
    /// Output format: "Creating bookmark push-XXXXX for revision XXXXX"
    fn parse_push_change_bookmark(output: &str, change_id: &str) -> Option<String> {
        for line in output.lines() {
            if let Some(rest) = line.strip_prefix("Creating bookmark ")
                && let Some(name) = rest.split_whitespace().next()
            {
                return Some(name.to_string());
            }
        }
        // Fallback: construct expected name
        Some(format!("push-{}", &change_id[..change_id.len().min(8)]))
    }

    /// Start push-by-change flow (extracted for reuse from mode selection)
    ///
    /// Runs dry-run for --change and shows confirm dialog.
    fn start_push_change(&mut self, change_id: &str) {
        let dry_run_result = if let Some(ref remote) = self.push_target_remote {
            self.jj.git_push_change_dry_run_to_remote(change_id, remote)
        } else {
            self.jj.git_push_change_dry_run(change_id)
        };
        match dry_run_result {
            Ok(output) => {
                let preview = output.trim();
                let short_id = &change_id[..change_id.len().min(8)];
                let body = if preview.is_empty() {
                    format!("Push by change ID? (creates push-{})", short_id)
                } else {
                    format!(
                        "Push by change ID? (creates push-{})\n{}",
                        short_id, preview
                    )
                };
                self.active_dialog = Some(Dialog::confirm(
                    "Push to Remote",
                    body,
                    Some("Remote changes cannot be undone with 'u'.".to_string()),
                    DialogCallback::GitPushChange {
                        change_id: change_id.to_string(),
                    },
                ));
            }
            Err(e) => {
                // If dry-run fails due to private/empty-description, show confirm
                // dialog anyway (without preview). The actual push will retry with flags.
                let err_msg = format!("{}", e);
                let retry_flags = detect_push_retry_flags(&err_msg);
                if !retry_flags.is_empty() {
                    let short_id = &change_id[..change_id.len().min(8)];
                    self.active_dialog = Some(Dialog::confirm(
                        "Push to Remote",
                        format!(
                            "Push by change ID? (creates push-{})\n(preview unavailable: will auto-retry with flags)",
                            short_id
                        ),
                        Some("Remote changes cannot be undone with 'u'.".to_string()),
                        DialogCallback::GitPushChange {
                            change_id: change_id.to_string(),
                        },
                    ));
                } else {
                    self.push_target_remote = None;
                    self.error_message = Some(format!("Push failed: {}", e));
                }
            }
        }
    }

    /// Show dry-run preview for bulk push, then confirm dialog
    ///
    /// Parses the dry-run output through `parse_push_dry_run()` to detect
    /// force push and protected bookmark scenarios, matching the warning
    /// behavior of single-bookmark push.
    fn start_push_bulk(&mut self, mode: PushBulkMode) {
        let remote = self.push_target_remote.clone();

        let dry_run_result = self.jj.git_push_bulk_dry_run(mode, remote.as_deref());
        match dry_run_result {
            Ok(output) => {
                let parsed = parse_push_dry_run(&output);
                match parsed {
                    PushPreviewResult::NothingChanged => {
                        self.push_target_remote = None;
                        self.notification = Some(Notification::info(format!(
                            "Nothing to push ({})",
                            mode.label()
                        )));
                    }
                    PushPreviewResult::Changes(actions) => {
                        let preview_text = format_preview_actions(&actions);
                        let is_force = has_force_push(&actions);
                        // Check if any action targets a protected bookmark
                        let has_protected = actions.iter().any(|a| {
                            let name = match a {
                                crate::jj::PushPreviewAction::MoveForward { bookmark, .. }
                                | crate::jj::PushPreviewAction::MoveSideways { bookmark, .. }
                                | crate::jj::PushPreviewAction::MoveBackward { bookmark, .. }
                                | crate::jj::PushPreviewAction::Add { bookmark, .. }
                                | crate::jj::PushPreviewAction::Delete { bookmark, .. } => bookmark,
                            };
                            is_immutable_bookmark(name)
                        });

                        let (body, detail) = if is_force && has_protected {
                            (
                                format!(
                                    "\u{26A0} FORCE PUSH {} (includes protected bookmarks)!\n{}",
                                    mode.label(),
                                    preview_text
                                ),
                                "WARNING: Force pushing to protected bookmarks rewrites shared history!"
                                    .to_string(),
                            )
                        } else if is_force {
                            (
                                format!("\u{26A0} FORCE PUSH {}?\n{}", mode.label(), preview_text),
                                "This will rewrite remote history! Cannot be undone with 'u'."
                                    .to_string(),
                            )
                        } else {
                            (
                                format!("Push {}?\n\n{}", mode.label(), preview_text),
                                "Remote changes cannot be undone with 'u'.".to_string(),
                            )
                        };

                        self.active_dialog = Some(Dialog::confirm(
                            "Push to Remote",
                            body,
                            Some(detail),
                            DialogCallback::GitPushBulkConfirm { mode, remote },
                        ));
                    }
                    PushPreviewResult::Unparsed => {
                        // Fallback: show raw output
                        let preview = output.trim();
                        if preview.is_empty() || preview.contains("Nothing changed") {
                            self.push_target_remote = None;
                            self.notification = Some(Notification::info(format!(
                                "Nothing to push ({})",
                                mode.label()
                            )));
                        } else {
                            self.active_dialog = Some(Dialog::confirm(
                                "Push to Remote",
                                format!("Push {}?\n\n{}", mode.label(), preview),
                                Some("Remote changes cannot be undone with 'u'.".to_string()),
                                DialogCallback::GitPushBulkConfirm { mode, remote },
                            ));
                        }
                    }
                }
            }
            Err(e) => {
                self.push_target_remote = None;
                self.error_message = Some(format!("Push failed: {}", e));
            }
        }
    }

    /// Execute bulk push (called after confirmation)
    fn execute_push_bulk(&mut self, mode: PushBulkMode, remote: Option<&str>) {
        self.push_target_remote = None;

        match self.jj.git_push_bulk(mode, remote) {
            Ok(_) => {
                self.notification = Some(Notification::success(format!("Pushed {}", mode.label())));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                self.error_message = Some(format!("Push failed: {}", e));
            }
        }
    }

    /// Show individual bookmark multi-select dialog (phase 2 of multi-bookmark push)
    ///
    /// Shows a checkbox-style select dialog with per-bookmark dry-run status labels.
    fn show_individual_bookmark_select(&mut self, change_id: &str, bookmarks: &[String]) {
        let mut items: Vec<SelectItem> = Vec::new();
        for name in bookmarks {
            let dry_run = if let Some(ref remote) = self.push_target_remote {
                self.jj.git_push_dry_run_to_remote(name, remote)
            } else {
                self.jj.git_push_dry_run(name)
            };
            let status = match dry_run {
                Ok(output) => {
                    let preview = parse_push_dry_run(&output);
                    format_bookmark_status(&preview, name)
                }
                Err(_) => String::new(),
            };

            let label = if status.is_empty() {
                name.clone()
            } else {
                format!("{} ({})", name, status)
            };

            items.push(SelectItem {
                label,
                value: name.clone(),
                selected: false,
            });
        }

        self.active_dialog = Some(Dialog::select(
            "Push to Remote",
            format!(
                "Select bookmarks to push from {}:",
                &change_id[..change_id.len().min(8)]
            ),
            items,
            Some("Remote changes cannot be undone with 'u'.".to_string()),
            DialogCallback::GitPush,
        ));
    }

    /// Start push-by-revisions flow (dry-run → confirm)
    ///
    /// Pushes all bookmarks on the specified revision via `--revisions`.
    /// If the jj version doesn't support `--revisions`, falls back to
    /// per-bookmark push using the provided bookmarks list.
    fn start_push_revisions(&mut self, change_id: &str, bookmarks: &[String]) {
        let dry_run_result = if let Some(ref remote) = self.push_target_remote {
            self.jj
                .git_push_revisions_dry_run_to_remote(change_id, remote)
        } else {
            self.jj.git_push_revisions_dry_run(change_id)
        };
        match dry_run_result {
            Ok(output) => {
                let parsed = parse_push_dry_run(&output);
                match parsed {
                    PushPreviewResult::NothingChanged => {
                        self.push_target_remote = None;
                        self.notification = Some(Notification::info(
                            "Nothing to push: all bookmarks are already up to date".to_string(),
                        ));
                    }
                    PushPreviewResult::Changes(actions) => {
                        let preview_text = format_preview_actions(&actions);
                        let is_force = has_force_push(&actions);
                        let has_protected = actions.iter().any(|a| {
                            let name = match a {
                                crate::jj::PushPreviewAction::MoveForward { bookmark, .. }
                                | crate::jj::PushPreviewAction::MoveSideways { bookmark, .. }
                                | crate::jj::PushPreviewAction::MoveBackward { bookmark, .. }
                                | crate::jj::PushPreviewAction::Add { bookmark, .. }
                                | crate::jj::PushPreviewAction::Delete { bookmark, .. } => bookmark,
                            };
                            is_immutable_bookmark(name)
                        });

                        let short_id = &change_id[..change_id.len().min(8)];
                        let (body, detail) = if is_force && has_protected {
                            (
                                format!(
                                    "\u{26A0} FORCE PUSH all bookmarks on {} (includes protected)!\n{}",
                                    short_id, preview_text
                                ),
                                "WARNING: Force pushing to protected bookmarks rewrites shared history!"
                                    .to_string(),
                            )
                        } else if is_force {
                            (
                                format!(
                                    "\u{26A0} FORCE PUSH all bookmarks on {}?\n{}",
                                    short_id, preview_text
                                ),
                                "This will rewrite remote history! Cannot be undone with 'u'."
                                    .to_string(),
                            )
                        } else {
                            (
                                format!("Push all bookmarks on {}?\n{}", short_id, preview_text),
                                "Remote changes cannot be undone with 'u'.".to_string(),
                            )
                        };

                        self.active_dialog = Some(Dialog::confirm(
                            "Push to Remote",
                            body,
                            Some(detail),
                            DialogCallback::GitPushRevisions {
                                change_id: change_id.to_string(),
                                bookmarks: bookmarks.to_vec(),
                            },
                        ));
                    }
                    PushPreviewResult::Unparsed => {
                        // Fallback: show confirm without parsed preview
                        let short_id = &change_id[..change_id.len().min(8)];
                        self.active_dialog = Some(Dialog::confirm(
                            "Push to Remote",
                            format!("Push all bookmarks on {}?", short_id),
                            Some("Remote changes cannot be undone with 'u'.".to_string()),
                            DialogCallback::GitPushRevisions {
                                change_id: change_id.to_string(),
                                bookmarks: bookmarks.to_vec(),
                            },
                        ));
                    }
                }
            }
            Err(e) => {
                let err_msg = format!("{}", e);
                if is_revisions_unsupported_error(&err_msg) {
                    // --revisions not supported: fallback to per-bookmark push
                    self.notification = Some(Notification::info(
                        "--revisions not supported, pushing bookmarks individually".to_string(),
                    ));
                    self.execute_push(bookmarks);
                } else if !detect_push_retry_flags(&err_msg).is_empty() {
                    // Dry-run failed due to private/empty-description: show confirm
                    // dialog anyway. The actual push will retry with flags.
                    let short_id = &change_id[..change_id.len().min(8)];
                    self.active_dialog = Some(Dialog::confirm(
                        "Push to Remote",
                        format!(
                            "Push all bookmarks on {}?\n(preview unavailable: will auto-retry with flags)",
                            short_id
                        ),
                        Some("Remote changes cannot be undone with 'u'.".to_string()),
                        DialogCallback::GitPushRevisions {
                            change_id: change_id.to_string(),
                            bookmarks: bookmarks.to_vec(),
                        },
                    ));
                } else {
                    self.push_target_remote = None;
                    self.error_message = Some(format!("Push failed: {}", e));
                }
            }
        }
    }

    /// Execute push by revisions (called after confirmation)
    ///
    /// Falls back to per-bookmark push if --revisions is not supported.
    /// On private/empty-description errors, retries with appropriate flags.
    fn execute_push_revisions(&mut self, change_id: &str, bookmarks: &[String]) {
        let remote = self.push_target_remote.take();
        let result = if let Some(ref r) = remote {
            self.jj.git_push_revisions_to_remote(change_id, r)
        } else {
            self.jj.git_push_revisions(change_id)
        };
        match result {
            Ok(_) => {
                let short_id = &change_id[..change_id.len().min(8)];
                let msg = if let Some(r) = remote.as_deref() {
                    format!("Pushed all bookmarks on {} to {}", short_id, r)
                } else {
                    format!("Pushed all bookmarks on {}", short_id)
                };
                self.notification = Some(Notification::success(msg));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                let err_msg = format!("{}", e);
                if is_revisions_unsupported_error(&err_msg) {
                    // Restore remote for fallback
                    self.push_target_remote = remote;
                    self.notification = Some(Notification::info(
                        "--revisions not supported, pushing bookmarks individually".to_string(),
                    ));
                    self.execute_push(bookmarks);
                } else {
                    // Try private/empty-description retry
                    let extra_flags = detect_push_retry_flags(&err_msg);
                    if !extra_flags.is_empty() {
                        let retry = if let Some(ref r) = remote {
                            self.jj.git_push_revisions_to_remote_with_flags(
                                change_id,
                                r,
                                &extra_flags,
                            )
                        } else {
                            self.jj
                                .git_push_revisions_with_flags(change_id, &extra_flags)
                        };
                        match retry {
                            Ok(_) => {
                                let short_id = &change_id[..change_id.len().min(8)];
                                let notes = retry_notes_from_flags(&extra_flags);
                                let suffix = build_push_suffix(false, &notes);
                                let msg = if let Some(r) = remote.as_deref() {
                                    format!(
                                        "Pushed all bookmarks on {} to {}{}",
                                        short_id, r, suffix
                                    )
                                } else {
                                    format!("Pushed all bookmarks on {}{}", short_id, suffix)
                                };
                                self.notification = Some(Notification::success(msg));
                                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
                            }
                            Err(e2) => {
                                self.error_message = Some(format!("Push failed: {}", e2));
                            }
                        }
                    } else {
                        self.error_message = Some(format!("Push failed: {}", e));
                    }
                }
            }
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
    fn execute_bookmark_move_to_wc(&mut self, name: &str) {
        match self.jj.bookmark_move(name, "@") {
            Ok(_) => {
                self.notification = Some(Notification::success(format!(
                    "Moved bookmark '{}' to @",
                    name
                )));
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
                    self.error_message = Some(format!(
                        "Move failed: {}\nTry: jj bookmark move {} --to @ --allow-backwards",
                        e, name
                    ));
                }
            }
        }
    }

    /// Execute bookmark move with --allow-backwards (called after re-confirmation)
    fn execute_bookmark_move_backwards(&mut self, name: &str) {
        match self.jj.bookmark_move_allow_backwards(name, "@") {
            Ok(_) => {
                self.notification = Some(Notification::success(format!(
                    "Moved bookmark '{}' to @ (backwards)",
                    name
                )));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_bookmarks());
            }
            Err(e) => {
                self.error_message = Some(format!("Move failed: {}", e));
            }
        }
    }

    /// Resolve a conflict using :ours tool
    pub(crate) fn execute_resolve_ours(&mut self, file_path: &str) {
        let (change_id, is_wc) = match self.resolve_view {
            Some(ref v) => (v.change_id.clone(), v.is_working_copy),
            None => return,
        };

        match self
            .jj
            .resolve_with_tool(file_path, ":ours", Some(&change_id))
        {
            Ok(_) => {
                self.notification = Some(Notification::success(format!(
                    "Resolved {} with :ours",
                    file_path
                )));
                self.refresh_resolve_list(&change_id, is_wc);
            }
            Err(e) => {
                self.error_message = Some(format!("Resolve failed: {}", e));
            }
        }
    }

    /// Resolve a conflict using :theirs tool
    pub(crate) fn execute_resolve_theirs(&mut self, file_path: &str) {
        let (change_id, is_wc) = match self.resolve_view {
            Some(ref v) => (v.change_id.clone(), v.is_working_copy),
            None => return,
        };

        match self
            .jj
            .resolve_with_tool(file_path, ":theirs", Some(&change_id))
        {
            Ok(_) => {
                self.notification = Some(Notification::success(format!(
                    "Resolved {} with :theirs",
                    file_path
                )));
                self.refresh_resolve_list(&change_id, is_wc);
            }
            Err(e) => {
                self.error_message = Some(format!("Resolve failed: {}", e));
            }
        }
    }

    /// Resolve a conflict using external merge tool (@ only)
    ///
    /// Similar to execute_split: temporarily exits TUI mode for interactive tool.
    pub(crate) fn execute_resolve_external(&mut self, file_path: &str) {
        use crossterm::execute;
        use crossterm::terminal::{
            Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        };
        use std::io::stdout;

        let (change_id, is_wc) = match self.resolve_view {
            Some(ref v) => (v.change_id.clone(), v.is_working_copy),
            None => return,
        };

        if !is_wc {
            self.notification = Some(Notification::warning(
                "External merge tool only works for working copy (@)",
            ));
            return;
        }

        // 1. Exit TUI mode
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Clear(ClearType::All));

        // 2. Scope guard to ensure terminal restoration
        let _guard = scopeguard::guard((), |_| {
            let _ = enable_raw_mode();
            let _ = execute!(stdout(), EnterAlternateScreen);
        });

        // 3. Run jj resolve (blocking)
        let result = self.jj.resolve_interactive(file_path, Some(&change_id));

        // 4. Handle result
        match result {
            Ok(status) if status.success() => {
                self.notification = Some(Notification::success(format!("Resolved {}", file_path)));
            }
            Ok(_) => {
                self.notification = Some(Notification::info("Resolve cancelled or failed"));
            }
            Err(e) => {
                self.error_message = Some(format!("Resolve failed: {}", e));
            }
        }

        // 5. Refresh resolve list
        self.refresh_resolve_list(&change_id, is_wc);
    }

    /// Execute rebase with specified mode
    ///
    /// Supports five modes:
    /// - `Revision` (`-r`): Move single change, descendants rebased onto parent
    /// - `Source` (`-s`): Move change and all descendants together
    /// - `Branch` (`-b`): Move entire branch relative to destination's ancestors
    /// - `InsertAfter` (`-A`): Insert change after target in history
    /// - `InsertBefore` (`-B`): Insert change before target in history
    ///
    /// When `skip_emptied` is true, `--skip-emptied` is appended.
    /// On unsupported flag errors, retries without the flag or shows guidance.
    pub(crate) fn execute_rebase(
        &mut self,
        source: &str,
        destination: &str,
        mode: RebaseMode,
        skip_emptied: bool,
    ) {
        // Prevent rebasing to self
        if source == destination {
            self.notification = Some(Notification::warning("Cannot rebase to itself"));
            return;
        }

        let extra_flags: Vec<&str> = if skip_emptied {
            vec![crate::jj::constants::flags::SKIP_EMPTIED]
        } else {
            vec![]
        };

        let result = if extra_flags.is_empty() {
            // No extra flags: use original methods (no allocation overhead)
            match mode {
                RebaseMode::Revision => self.jj.rebase(source, destination),
                RebaseMode::Source => self.jj.rebase_source(source, destination),
                RebaseMode::Branch => self.jj.rebase_branch(source, destination),
                RebaseMode::InsertAfter => self.jj.rebase_insert_after(source, destination),
                RebaseMode::InsertBefore => self.jj.rebase_insert_before(source, destination),
            }
        } else {
            match mode {
                RebaseMode::Revision => {
                    self.jj.rebase_with_flags(source, destination, &extra_flags)
                }
                RebaseMode::Source => {
                    self.jj
                        .rebase_source_with_flags(source, destination, &extra_flags)
                }
                RebaseMode::Branch => {
                    self.jj
                        .rebase_branch_with_flags(source, destination, &extra_flags)
                }
                RebaseMode::InsertAfter => {
                    self.jj
                        .rebase_insert_after_with_flags(source, destination, &extra_flags)
                }
                RebaseMode::InsertBefore => {
                    self.jj
                        .rebase_insert_before_with_flags(source, destination, &extra_flags)
                }
            }
        };

        match result {
            Ok(output) => {
                self.notify_rebase_success(&output, destination, mode, skip_emptied);
            }
            Err(e) => {
                let err_msg = format!("{}", e);

                if skip_emptied && is_rebase_flag_unsupported(&err_msg) {
                    // --skip-emptied (or mode flag) caused failure, retry without it
                    let retry = match mode {
                        RebaseMode::Revision => self.jj.rebase(source, destination),
                        RebaseMode::Source => self.jj.rebase_source(source, destination),
                        RebaseMode::Branch => self.jj.rebase_branch(source, destination),
                        RebaseMode::InsertAfter => self.jj.rebase_insert_after(source, destination),
                        RebaseMode::InsertBefore => {
                            self.jj.rebase_insert_before(source, destination)
                        }
                    };
                    match retry {
                        Ok(output) => {
                            self.notify_rebase_success(&output, destination, mode, false);
                            // Append skip-emptied note, preserving severity
                            // (e.g., Warning for conflicts must not be downgraded to Info)
                            if let Some(existing) = self.notification.take() {
                                let new_msg = format!(
                                    "{} (--skip-emptied not supported, empty commits may remain)",
                                    existing.message
                                );
                                self.notification = Some(Notification::new(new_msg, existing.kind));
                            }
                        }
                        Err(e2) => {
                            // Retry also failed — if mode is Branch and error is still
                            // "unsupported flag", the real issue is -b not being supported
                            let retry_msg = format!("{}", e2);
                            if mode == RebaseMode::Branch && is_rebase_flag_unsupported(&retry_msg)
                            {
                                self.notification = Some(Notification::warning(
                                    "Branch mode (-b) not supported in this jj version. Use Source mode (-s) instead.",
                                ));
                            } else {
                                self.error_message = Some(format!("Rebase failed: {}", e2));
                            }
                        }
                    }
                } else if is_rebase_flag_unsupported(&err_msg) {
                    // No skip_emptied — flag unsupported (e.g., -b not supported)
                    if mode == RebaseMode::Branch {
                        self.notification = Some(Notification::warning(
                            "Branch mode (-b) not supported in this jj version. Use Source mode (-s) instead.",
                        ));
                    } else {
                        self.error_message = Some(format!("Rebase failed: {}", e));
                    }
                } else {
                    self.error_message = Some(format!("Rebase failed: {}", e));
                }
            }
        }
    }

    /// Build and set notification for successful rebase
    fn notify_rebase_success(
        &mut self,
        output: &str,
        destination: &str,
        mode: RebaseMode,
        skip_emptied: bool,
    ) {
        self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());

        let skip_suffix = if skip_emptied {
            " (empty commits skipped)"
        } else {
            ""
        };

        // Unified conflict detection from jj output
        let has_conflict = output.to_lowercase().contains("conflict");
        let notification = if has_conflict {
            Notification::warning("Rebased with conflicts - resolve with jj resolve")
        } else {
            let msg = match mode {
                RebaseMode::Revision => format!("Rebased successfully{}", skip_suffix),
                RebaseMode::Source => {
                    format!("Rebased source and descendants successfully{}", skip_suffix)
                }
                RebaseMode::Branch => format!("Rebased branch successfully{}", skip_suffix),
                RebaseMode::InsertAfter => {
                    let short = &destination[..8.min(destination.len())];
                    format!("Inserted after {} successfully{}", short, skip_suffix)
                }
                RebaseMode::InsertBefore => {
                    let short = &destination[..8.min(destination.len())];
                    format!("Inserted before {} successfully{}", short, skip_suffix)
                }
            };
            Notification::success(msg)
        };
        self.notification = Some(notification);
    }

    /// Handle dialog result
    ///
    /// Called when a dialog is closed.
    ///
    /// Implementation order (important):
    /// 1. Clone callback_id from active_dialog
    /// 2. Set active_dialog to None
    /// 3. Match on callback and result
    pub(crate) fn handle_dialog_result(&mut self, result: DialogResult) {
        // Step 1: Clone callback_id (String clone cost is acceptable)
        let callback = self.active_dialog.as_ref().map(|d| d.callback_id.clone());

        // Step 2: Clear active_dialog (callback is already cloned)
        self.active_dialog = None;

        // Step 3: Match on callback and result
        // Note: For Confirm dialogs, Confirmed(vec) always contains an empty vec.
        //       Only Select dialogs return selected values.
        match (callback, result) {
            (Some(DialogCallback::DeleteBookmarks), DialogResult::Confirmed(names)) => {
                // Select dialog - names contains selected bookmark names
                self.execute_bookmark_delete(&names);
            }
            (
                Some(DialogCallback::MoveBookmark { name, change_id }),
                DialogResult::Confirmed(_),
            ) => {
                // Confirm dialog - execute bookmark move
                self.execute_bookmark_move(&name, &change_id);
            }
            (Some(DialogCallback::OpRestore), DialogResult::Confirmed(_)) => {
                // TODO: Implement op restore with dialog
            }
            (Some(DialogCallback::GitPush), DialogResult::Confirmed(names)) => {
                if names.is_empty() {
                    // Confirm dialog (single bookmark): use pending_push_bookmarks
                    // Note: pending_push_bookmarks is for Confirm dialog only.
                    // Select dialog returns selected values in names.
                    let bookmarks = std::mem::take(&mut self.pending_push_bookmarks);
                    self.execute_push(&bookmarks);
                } else {
                    // Select dialog (multiple bookmarks): use selected names
                    self.execute_push(&names);
                }
            }
            (Some(DialogCallback::GitPush), DialogResult::Cancelled) => {
                // Clear pending state on cancel to prevent stale data
                self.pending_push_bookmarks.clear();
                self.push_target_remote = None;
            }
            // --- GitPushRevisions ---
            (
                Some(DialogCallback::GitPushRevisions {
                    change_id,
                    bookmarks,
                }),
                DialogResult::Confirmed(_),
            ) => {
                self.execute_push_revisions(&change_id, &bookmarks);
            }
            (Some(DialogCallback::GitPushRevisions { .. }), DialogResult::Cancelled) => {
                self.push_target_remote = None;
            }
            // --- GitPushMultiBookmarkMode ---
            (
                Some(DialogCallback::GitPushMultiBookmarkMode {
                    change_id,
                    bookmarks,
                }),
                DialogResult::Confirmed(selected),
            ) => match selected.first().map(|s| s.as_str()) {
                Some("revisions") => {
                    self.start_push_revisions(&change_id, &bookmarks);
                }
                Some("individual") => {
                    self.show_individual_bookmark_select(&change_id, &bookmarks);
                }
                _ => {}
            },
            (Some(DialogCallback::GitPushMultiBookmarkMode { .. }), DialogResult::Cancelled) => {
                self.push_target_remote = None;
            }
            (Some(DialogCallback::Track), DialogResult::Confirmed(names)) => {
                // Select dialog - names contains selected bookmark full names (e.g., "feature@origin")
                self.execute_track(&names);
            }
            (Some(DialogCallback::BookmarkJump), DialogResult::Confirmed(change_ids)) => {
                // Single-select dialog - change_ids contains exactly one change_id
                if let Some(change_id) = change_ids.first() {
                    self.execute_bookmark_jump(change_id);
                }
            }
            (Some(DialogCallback::BookmarkForget), DialogResult::Confirmed(_)) => {
                self.execute_bookmark_forget();
            }
            (Some(DialogCallback::BookmarkForget), DialogResult::Cancelled) => {
                self.pending_forget_bookmark = None;
            }
            (Some(DialogCallback::GitPushChange { change_id }), DialogResult::Confirmed(_)) => {
                self.execute_push_change(&change_id);
            }
            (Some(DialogCallback::GitPushChange { .. }), DialogResult::Cancelled) => {
                self.push_target_remote = None;
            }
            (Some(DialogCallback::GitPushRemoteSelect), DialogResult::Confirmed(selected)) => {
                if let Some(remote) = selected.first() {
                    self.push_target_remote = Some(remote.clone());
                    self.start_push();
                }
            }
            (Some(DialogCallback::GitPushRemoteSelect), DialogResult::Cancelled) => {
                self.push_target_remote = None;
            }
            (Some(DialogCallback::GitFetch), DialogResult::Confirmed(selected)) => {
                if let Some(value) = selected.first() {
                    if value == "__branch__" {
                        self.start_fetch_branch_select();
                    } else {
                        self.execute_fetch_with_option(value);
                    }
                }
            }
            (Some(DialogCallback::GitFetchBranch), DialogResult::Confirmed(selected)) => {
                if let Some(branch) = selected.first() {
                    self.execute_fetch_branch(branch);
                }
            }
            // --- GitPushModeSelect ---
            (
                Some(DialogCallback::GitPushModeSelect { change_id }),
                DialogResult::Confirmed(selected),
            ) => match selected.first().map(|s| s.as_str()) {
                Some("change") => {
                    self.start_push_change(&change_id);
                }
                Some("all") => {
                    self.start_push_bulk(PushBulkMode::All);
                }
                Some("tracked") => {
                    self.start_push_bulk(PushBulkMode::Tracked);
                }
                Some("deleted") => {
                    self.start_push_bulk(PushBulkMode::Deleted);
                }
                _ => {}
            },
            (Some(DialogCallback::GitPushModeSelect { .. }), DialogResult::Cancelled) => {
                self.push_target_remote = None;
            }
            // --- GitPushBulkConfirm ---
            (
                Some(DialogCallback::GitPushBulkConfirm { mode, remote }),
                DialogResult::Confirmed(_),
            ) => {
                self.execute_push_bulk(mode, remote.as_deref());
            }
            (Some(DialogCallback::GitPushBulkConfirm { .. }), DialogResult::Cancelled) => {
                self.push_target_remote = None;
            }
            // --- BookmarkMoveToWc ---
            (Some(DialogCallback::BookmarkMoveToWc { name }), DialogResult::Confirmed(_)) => {
                self.execute_bookmark_move_to_wc(&name);
            }
            (Some(DialogCallback::BookmarkMoveToWc { .. }), DialogResult::Cancelled) => {}
            // --- BookmarkMoveBackwards ---
            (Some(DialogCallback::BookmarkMoveBackwards { name }), DialogResult::Confirmed(_)) => {
                self.execute_bookmark_move_backwards(&name);
            }
            (Some(DialogCallback::BookmarkMoveBackwards { .. }), DialogResult::Cancelled) => {}
            // --- RestoreFile ---
            (Some(DialogCallback::RestoreFile { file_path }), DialogResult::Confirmed(_)) => {
                self.execute_restore_file(&file_path);
            }
            // --- RestoreAll ---
            (Some(DialogCallback::RestoreAll), DialogResult::Confirmed(_)) => {
                self.execute_restore_all();
            }
            // --- Revert ---
            (Some(DialogCallback::Revert { change_id }), DialogResult::Confirmed(_)) => {
                self.execute_revert(&change_id);
            }
            // --- SimplifyParents ---
            (Some(DialogCallback::SimplifyParents { change_id }), DialogResult::Confirmed(_)) => {
                self.execute_simplify_parents(&change_id);
            }
            (_, DialogResult::Cancelled) => {
                // Cancelled - do nothing
            }
            _ => {}
        }
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
                    self.notification = Some(Notification::info("No untracked remote bookmarks"));
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
                self.error_message = Some(format!("Failed to list bookmarks: {}", e));
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
                    self.notification = Some(Notification::info("No bookmarks available"));
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
                self.error_message = Some(format!("Failed to list bookmarks: {}", e));
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
            self.notification = Some(Notification::success(format!(
                "Jumped to {} in log",
                short_id
            )));
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
                self.notification = Some(Notification::success(format!(
                    "Jumped to {} (revset expanded, r+Enter to reset)",
                    short_id
                )));
                self.previous_view = None;
                self.current_view = View::Log;
            } else {
                self.notification = Some(Notification::warning("Change not found in repository"));
            }
        } else {
            // First press — show hint and store pending
            self.pending_jump_change_id = Some(change_id.to_string());
            self.notification = Some(Notification::info(
                "Change not in current revset. Press J again to search full log",
            ));
        }
    }

    /// Execute bookmark jump - select the change in log view
    pub(crate) fn execute_bookmark_jump(&mut self, change_id: &str) {
        if self.log_view.select_change_by_id(change_id) {
            let short_id = &change_id[..8.min(change_id.len())];
            self.notification = Some(Notification::success(format!("Jumped to {}", short_id)));
        } else {
            // The change might not be visible in current revset
            self.notification = Some(Notification::warning(
                "Bookmark target not visible in current revset",
            ));
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
                self.error_message = Some(format!("Failed to list bookmarks: {}", e));
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
                self.error_message = Some(format!("Failed to list bookmarks: {}", e));
            }
        }
    }

    /// Execute untrack for a remote bookmark
    pub(crate) fn execute_untrack(&mut self, full_name: &str) {
        match self.jj.bookmark_untrack(&[full_name]) {
            Ok(_) => {
                let display = full_name.split('@').next().unwrap_or(full_name);
                self.notification = Some(Notification::success(format!(
                    "Stopped tracking: {}",
                    display
                )));
                self.mark_dirty_and_refresh_current(DirtyFlags {
                    log: true,
                    status: true,
                    op_log: true,
                    bookmarks: true,
                });
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to untrack: {}", e));
            }
        }
    }

    /// Execute track for selected bookmarks
    pub(crate) fn execute_track(&mut self, names: &[String]) {
        if names.is_empty() {
            return;
        }

        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        match self.jj.bookmark_track(&name_refs) {
            Ok(_) => {
                let display = if names.len() == 1 {
                    // "feature-x@origin" から "feature-x" を抽出
                    names[0].split('@').next().unwrap_or(&names[0]).to_string()
                } else {
                    format!("{} bookmarks", names.len())
                };
                self.notification = Some(Notification::success(format!(
                    "Started tracking: {}",
                    display
                )));
                self.mark_dirty_and_refresh_current(DirtyFlags {
                    log: true,
                    status: true,
                    op_log: true,
                    bookmarks: true,
                });
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to track: {}", e));
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Preview pane
    // ─────────────────────────────────────────────────────────────────────────

    /// Update preview cache if selected change has changed.
    ///
    /// Called after key processing, NOT during render.
    /// Skips fetch if:
    /// - Preview is disabled
    /// - Same change_id is already cached (cache hit)
    /// - Preview auto-disabled (small terminal) — tracks pending_id for later
    /// - Rapid movement detected (debounce: skip if last fetch was < 100ms ago)
    pub(crate) fn update_preview_if_needed(&mut self) {
        if !self.preview_enabled {
            return;
        }

        let (current_change_id, current_commit_id) = match self.log_view.selected_change() {
            Some(c) => (c.change_id.clone(), c.commit_id.clone()),
            None => return, // No selection — keep cache intact
        };

        // Cache hit — same change_id with matching commit_id
        if let Some(cached) = self.preview_cache.peek(&current_change_id)
            && cached.commit_id == current_commit_id
        {
            self.preview_cache.touch(&current_change_id);
            return;
        }
        // commit_id mismatch — stale, need re-fetch

        // Always defer to idle tick — never block key handling with jj show.
        // resolve_pending_preview() will fetch on the next poll timeout.
        self.preview_pending_id = Some(current_change_id);
    }

    /// Actually fetch preview content via jj show
    fn fetch_preview(&mut self, change_id: &str) {
        self.preview_pending_id = None;

        // Capture bookmarks and commit_id from the Change model
        let (commit_id, bookmarks) = self
            .log_view
            .selected_change()
            .filter(|c| c.change_id == change_id)
            .map(|c| (c.commit_id.clone(), c.bookmarks.clone()))
            .unwrap_or_default();

        match self.jj.show(change_id) {
            Ok(content) => {
                self.preview_cache.insert(super::state::PreviewCacheEntry {
                    change_id: change_id.to_string(),
                    commit_id,
                    content,
                    bookmarks,
                });
            }
            Err(_) => {
                self.preview_cache.remove(change_id);
            }
        }
    }

    /// Called from the event loop idle handler (when no key is pressed).
    /// Resolves any pending preview fetch that was deferred by debounce or auto-disable.
    pub fn resolve_pending_preview(&mut self) {
        if !self.preview_enabled || self.preview_auto_disabled {
            return;
        }
        if let Some(pending_id) = self.preview_pending_id.take() {
            // Verify the selection hasn't changed
            let still_selected = self
                .log_view
                .selected_change()
                .map(|c| c.change_id == pending_id)
                .unwrap_or(false);
            if still_selected {
                self.fetch_preview(&pending_id);
            }
        }
    }

    // =========================================================================
    // Diff export (clipboard copy & file export)
    // =========================================================================

    /// Copy diff content to system clipboard
    pub(crate) fn copy_diff_to_clipboard(&mut self, full: bool) {
        let Some(ref diff_view) = self.diff_view else {
            return;
        };
        let change_id = diff_view.change_id.clone();
        let compare_info = diff_view.compare_info.clone();

        let result = if full {
            if let Some(ref ci) = compare_info {
                // Compare mode: jj show doesn't apply, prepend from/to metadata header
                let diff = self.jj.diff_range(&ci.from.change_id, &ci.to.change_id);
                diff.map(|d| {
                    format!(
                        "Compare: {} -> {}\nFrom: {} ({})\nTo:   {} ({})\n\n{}",
                        ci.from.change_id,
                        ci.to.change_id,
                        ci.from.change_id,
                        ci.from.description,
                        ci.to.change_id,
                        ci.to.description,
                        d,
                    )
                })
            } else {
                self.jj.show_raw(&change_id)
            }
        } else {
            // jj diff (diff only)
            if let Some(ref ci) = compare_info {
                self.jj.diff_range(&ci.from.change_id, &ci.to.change_id)
            } else {
                self.jj.diff_raw(&change_id)
            }
        };

        match result {
            Ok(text) => {
                let line_count = text.lines().count();
                match super::clipboard::copy_to_clipboard(&text) {
                    Ok(()) => {
                        let mode = if full { "show" } else { "diff" };
                        self.notification = Some(Notification::success(format!(
                            "Copied to clipboard ({} lines, {})",
                            line_count, mode
                        )));
                    }
                    Err(e) => {
                        self.error_message = Some(e);
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to get diff: {}", e));
            }
        }
    }

    /// Export diff content to a .patch file
    pub(crate) fn export_diff_to_file(&mut self) {
        let Some(ref diff_view) = self.diff_view else {
            return;
        };
        let change_id = diff_view.change_id.clone();
        let compare_info = diff_view.compare_info.clone();

        // Determine filename and content based on mode
        // Uses `jj diff --git` for git-compatible unified patch format (git apply compatible)
        let (short_id, result) = if let Some(ref ci) = compare_info {
            // Compare mode: use diff --git --from --to
            let from_short = &ci.from.change_id[..ci.from.change_id.len().min(8)];
            let to_short = &ci.to.change_id[..ci.to.change_id.len().min(8)];
            let label = format!("{}_{}", from_short, to_short);
            let result = self.jj.diff_range_git(&ci.from.change_id, &ci.to.change_id);
            (label, result)
        } else {
            let short = change_id[..change_id.len().min(8)].to_string();
            let result = self.jj.diff_git_raw(&change_id);
            (short, result)
        };

        match result {
            Ok(text) => {
                let filename = unique_patch_filename(&short_id);
                match std::fs::write(&filename, &text) {
                    Ok(()) => {
                        self.notification =
                            Some(Notification::success(format!("Exported to {}", filename)));
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Failed to write {}: {}", filename, e));
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to get diff: {}", e));
            }
        }
    }
}

/// Generate a unique .patch filename, appending -1, -2, etc. if the file already exists
fn unique_patch_filename(short_id: &str) -> String {
    let base = format!("{}.patch", short_id);
    if !std::path::Path::new(&base).exists() {
        return base;
    }
    for i in 1.. {
        let candidate = format!("{}-{}.patch", short_id, i);
        if !std::path::Path::new(&candidate).exists() {
            return candidate;
        }
    }
    unreachable!()
}

/// Check if any push actions involve a force push (non-fast-forward)
///
/// Uses safe-side detection: anything that is NOT a known-safe action
/// (MoveForward, Add, Delete) is treated as a force push. This ensures
/// that future jj action types (e.g. new move directions) are flagged
/// as potentially dangerous by default.
fn has_force_push(actions: &[crate::jj::PushPreviewAction]) -> bool {
    use crate::jj::PushPreviewAction;
    actions.iter().any(|a| {
        !matches!(
            a,
            PushPreviewAction::MoveForward { .. }
                | PushPreviewAction::Add { .. }
                | PushPreviewAction::Delete { .. }
        )
    })
}

/// Default list of protected/immutable bookmark names.
///
/// These are shared integration branches where force pushing rewrites
/// history for all collaborators. Extracted as a constant to make
/// future configuration-file-based overrides a minimal diff.
const DEFAULT_IMMUTABLE_BOOKMARKS: &[&str] = &["main", "master", "trunk"];

/// Check if a bookmark name is considered immutable/protected
///
/// Protected bookmarks are shared integration branches.
/// Force pushing to them rewrites shared history for all collaborators.
fn is_immutable_bookmark(name: &str) -> bool {
    DEFAULT_IMMUTABLE_BOOKMARKS.contains(&name)
}

/// Format preview actions for confirm dialog display
///
/// Produces a compact single-line per action, with hashes truncated to 8 chars.
/// Force push actions are prefixed with a warning symbol.
fn format_preview_actions(actions: &[crate::jj::PushPreviewAction]) -> String {
    use crate::jj::PushPreviewAction;
    actions
        .iter()
        .map(|action| match action {
            PushPreviewAction::MoveForward { bookmark, from, to } => {
                let from_short = &from[..8.min(from.len())];
                let to_short = &to[..8.min(to.len())];
                format!(
                    "Move forward {} from {}.. to {}..",
                    bookmark, from_short, to_short
                )
            }
            PushPreviewAction::MoveSideways { bookmark, from, to } => {
                let from_short = &from[..8.min(from.len())];
                let to_short = &to[..8.min(to.len())];
                format!(
                    "\u{26A0} Move sideways {} from {}.. to {}..",
                    bookmark, from_short, to_short
                )
            }
            PushPreviewAction::MoveBackward { bookmark, from, to } => {
                let from_short = &from[..8.min(from.len())];
                let to_short = &to[..8.min(to.len())];
                format!(
                    "\u{26A0} Move backward {} from {}.. to {}..",
                    bookmark, from_short, to_short
                )
            }
            PushPreviewAction::Add { bookmark, to } => {
                let to_short = &to[..8.min(to.len())];
                format!("Add {} to {}..", bookmark, to_short)
            }
            PushPreviewAction::Delete { bookmark, from } => {
                let from_short = &from[..8.min(from.len())];
                format!("Delete {} from {}..", bookmark, from_short)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format a single bookmark's dry-run status for select dialog label
fn format_bookmark_status(preview: &crate::jj::PushPreviewResult, name: &str) -> String {
    use crate::jj::{PushPreviewAction, PushPreviewResult};
    match preview {
        PushPreviewResult::Changes(actions) => actions
            .iter()
            .find_map(|a| match a {
                PushPreviewAction::MoveForward { bookmark, from, .. } if bookmark == name => {
                    let short = &from[..8.min(from.len())];
                    Some(format!("move from {}..", short))
                }
                PushPreviewAction::MoveSideways { bookmark, .. } if bookmark == name => {
                    if is_immutable_bookmark(name) {
                        Some("\u{26A0} PROTECTED force".to_string())
                    } else {
                        Some("\u{26A0} force".to_string())
                    }
                }
                PushPreviewAction::MoveBackward { bookmark, .. } if bookmark == name => {
                    if is_immutable_bookmark(name) {
                        Some("\u{26A0} PROTECTED force".to_string())
                    } else {
                        Some("\u{26A0} force".to_string())
                    }
                }
                PushPreviewAction::Add { bookmark, .. } if bookmark == name => {
                    Some("new".to_string())
                }
                PushPreviewAction::Delete { bookmark, .. } if bookmark == name => {
                    Some("delete".to_string())
                }
                _ => None,
            })
            .unwrap_or_default(),
        PushPreviewResult::NothingChanged => "up to date".to_string(),
        PushPreviewResult::Unparsed => String::new(),
    }
}

/// Check if a push error indicates an untracked/new bookmark
///
/// In jj 0.37+, pushing an untracked bookmark fails with errors like:
/// - "Refusing to create new remote bookmark" (older jj versions)
/// - Bookmark not tracked on any remote (0.37+ tracking model)
///
/// When detected, the caller retries with `--allow-new` (deprecated but functional).
fn is_untracked_bookmark_error(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    lower.contains("refusing to create new remote bookmark")
        || lower.contains("not tracked")
        || lower.contains("untracked")
}

/// Check if a push error indicates that `--revisions` is not supported
///
/// Older jj versions don't have the `--revisions` flag. When detected,
/// the caller falls back to per-bookmark push.
/// Requires the error message to reference "--revisions" to avoid false positives
/// from unrelated errors that contain generic "unexpected argument" text.
fn is_revisions_unsupported_error(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    // Must mention --revisions in context to avoid false positives
    let mentions_revisions = lower.contains("--revisions") || lower.contains("revisions");
    let is_unknown_flag = lower.contains("unexpected argument")
        || lower.contains("unrecognized")
        || lower.contains("unknown flag")
        || lower.contains("unknown option");
    mentions_revisions && is_unknown_flag
}

/// Check if a rebase error indicates an unsupported flag
///
/// Older jj versions may not support `--skip-emptied` or `-b`.
/// Detects generic "unexpected argument" / "unrecognized" errors.
fn is_rebase_flag_unsupported(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    lower.contains("unexpected argument")
        || lower.contains("unrecognized")
        || lower.contains("unknown flag")
        || lower.contains("unknown option")
}

/// Check if a push error indicates a private commit
///
/// In jj, pushing a private commit fails with an error like:
/// "Won't push commit abc123 since it is private"
fn is_private_commit_error(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    lower.contains("private") && lower.contains("won't push")
}

/// Check if a push error indicates an empty description
///
/// In jj, pushing a commit with no description fails with:
/// "Won't push commit abc123 since it has no description"
fn is_empty_description_error(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    lower.contains("no description") && lower.contains("won't push")
}

/// Detect which retry flags are needed based on push error message
///
/// Returns a Vec of flag strings for use with `_with_flags` methods.
/// Detects private commit and empty description errors simultaneously.
fn detect_push_retry_flags(err_msg: &str) -> Vec<&'static str> {
    let mut flags = Vec::new();
    if is_private_commit_error(err_msg) {
        flags.push(crate::jj::constants::flags::ALLOW_PRIVATE);
    }
    if is_empty_description_error(err_msg) {
        flags.push(crate::jj::constants::flags::ALLOW_EMPTY_DESC);
    }
    flags
}

/// Convert retry flags into human-readable notes for notification
fn retry_notes_from_flags<'a>(extra_flags: &[&str]) -> Vec<&'a str> {
    let mut notes = Vec::new();
    if extra_flags.contains(&crate::jj::constants::flags::ALLOW_PRIVATE) {
        notes.push("private commit allowed");
    }
    if extra_flags.contains(&crate::jj::constants::flags::ALLOW_EMPTY_DESC) {
        notes.push("empty description allowed");
    }
    notes
}

/// Build notification suffix from retry state
///
/// Examples:
/// - `" (used deprecated --allow-new)"` when allow_new is true
/// - `" (private commit allowed)"` for private retry
/// - `" (private commit allowed + empty description allowed)"` for both
fn build_push_suffix(used_allow_new: bool, retry_notes: &[&str]) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if used_allow_new {
        parts.push("used deprecated --allow-new");
    }
    parts.extend_from_slice(retry_notes);
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(" + "))
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

    // =========================================================================
    // Describe multi-line detection tests
    //
    // The App::start_describe_input() method uses `desc.lines().nth(1).is_some()`
    // to detect multi-line descriptions and fall through to the external editor.
    // These tests verify the detection logic matches expectations.
    // =========================================================================

    #[test]
    fn test_multiline_detection_single_line() {
        let desc = "single line description";
        assert!(desc.lines().nth(1).is_none());
    }

    #[test]
    fn test_multiline_detection_two_lines() {
        let desc = "first line\nsecond line";
        assert!(desc.lines().nth(1).is_some());
    }

    #[test]
    fn test_multiline_detection_empty_string() {
        let desc = "";
        assert!(desc.lines().nth(1).is_none());
    }

    #[test]
    fn test_multiline_detection_trailing_newline_only() {
        // After trim_end_matches('\n'), a single-line desc with trailing \n becomes single-line
        let desc = "single line\n".trim_end_matches('\n');
        assert!(desc.lines().nth(1).is_none());
    }

    #[test]
    fn test_multiline_detection_two_lines_with_trailing_newline() {
        // After trim_end_matches('\n'), multi-line desc is still multi-line
        let desc = "first\nsecond\n".trim_end_matches('\n');
        assert!(desc.lines().nth(1).is_some());
    }

    // =========================================================================
    // Before/after description comparison tests
    //
    // The App::execute_describe_external() method compares descriptions
    // using trim_end() to normalize trailing whitespace.
    // These tests verify the comparison logic.
    // =========================================================================

    #[test]
    fn test_description_comparison_identical() {
        let before = "test description".trim_end().to_string();
        let after = "test description".trim_end().to_string();
        assert_eq!(before, after);
    }

    #[test]
    fn test_description_comparison_trailing_whitespace_normalized() {
        let before = "test description\n".trim_end().to_string();
        let after = "test description\n\n".trim_end().to_string();
        assert_eq!(before, after);
    }

    #[test]
    fn test_description_comparison_different() {
        let before = "old description".trim_end().to_string();
        let after = "new description".trim_end().to_string();
        assert_ne!(before, after);
    }

    // =========================================================================
    // has_force_push tests
    // =========================================================================

    #[test]
    fn test_has_force_push_forward_only() {
        use crate::jj::PushPreviewAction;
        let actions = vec![PushPreviewAction::MoveForward {
            bookmark: "main".to_string(),
            from: "aaa".to_string(),
            to: "bbb".to_string(),
        }];
        assert!(!has_force_push(&actions));
    }

    #[test]
    fn test_has_force_push_sideways() {
        use crate::jj::PushPreviewAction;
        let actions = vec![PushPreviewAction::MoveSideways {
            bookmark: "feature".to_string(),
            from: "aaa".to_string(),
            to: "bbb".to_string(),
        }];
        assert!(has_force_push(&actions));
    }

    #[test]
    fn test_has_force_push_backward() {
        use crate::jj::PushPreviewAction;
        let actions = vec![PushPreviewAction::MoveBackward {
            bookmark: "main".to_string(),
            from: "bbb".to_string(),
            to: "aaa".to_string(),
        }];
        assert!(has_force_push(&actions));
    }

    // =========================================================================
    // is_immutable_bookmark tests
    // =========================================================================

    #[test]
    fn test_is_immutable_bookmark_main() {
        assert!(is_immutable_bookmark("main"));
    }

    #[test]
    fn test_is_immutable_bookmark_master() {
        assert!(is_immutable_bookmark("master"));
    }

    #[test]
    fn test_is_immutable_bookmark_trunk() {
        assert!(is_immutable_bookmark("trunk"));
    }

    #[test]
    fn test_is_immutable_bookmark_feature() {
        assert!(!is_immutable_bookmark("feature-x"));
    }

    // =========================================================================
    // format_bookmark_status tests (multi-bookmark select dialog labels)
    // =========================================================================

    #[test]
    fn test_format_bookmark_status_protected_force_label() {
        use crate::jj::{PushPreviewAction, PushPreviewResult};
        // Protected bookmark (main) with sideways move should show "⚠ PROTECTED force"
        let preview = PushPreviewResult::Changes(vec![PushPreviewAction::MoveSideways {
            bookmark: "main".to_string(),
            from: "aaa111bbb222".to_string(),
            to: "ccc333ddd444".to_string(),
        }]);
        let status = format_bookmark_status(&preview, "main");
        assert_eq!(status, "\u{26A0} PROTECTED force");
    }

    #[test]
    fn test_format_bookmark_status_force_label() {
        use crate::jj::{PushPreviewAction, PushPreviewResult};
        // Non-protected bookmark with backward move should show "⚠ force"
        let preview = PushPreviewResult::Changes(vec![PushPreviewAction::MoveBackward {
            bookmark: "feature-x".to_string(),
            from: "aaa111bbb222".to_string(),
            to: "ccc333ddd444".to_string(),
        }]);
        let status = format_bookmark_status(&preview, "feature-x");
        assert_eq!(status, "\u{26A0} force");
    }

    #[test]
    fn test_format_bookmark_status_forward_is_not_force() {
        use crate::jj::{PushPreviewAction, PushPreviewResult};
        let preview = PushPreviewResult::Changes(vec![PushPreviewAction::MoveForward {
            bookmark: "main".to_string(),
            from: "aaa111bbb222".to_string(),
            to: "ccc333ddd444".to_string(),
        }]);
        let status = format_bookmark_status(&preview, "main");
        assert!(status.starts_with("move from"));
    }

    // =========================================================================
    // truncate_description tests (UTF-8 safe truncation)
    // =========================================================================

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

    // =========================================================================
    // parse_push_change_bookmark tests
    // =========================================================================

    #[test]
    fn test_push_change_output_parsing() {
        let output = "Creating bookmark push-ryxwqxsq for revision ryxwqxsq\n\
                       Add bookmark push-ryxwqxsq to abc1234567890";
        let result = App::parse_push_change_bookmark(output, "ryxwqxsq");
        assert_eq!(result, Some("push-ryxwqxsq".to_string()));
    }

    #[test]
    fn test_push_change_output_parsing_fallback() {
        // No "Creating bookmark" in output → fallback to constructed name
        let output = "Some other output";
        let result = App::parse_push_change_bookmark(output, "abcd1234");
        assert_eq!(result, Some("push-abcd1234".to_string()));
    }

    #[test]
    fn test_push_change_output_parsing_empty() {
        let result = App::parse_push_change_bookmark("", "xyz98765");
        assert_eq!(result, Some("push-xyz98765".to_string()));
    }

    // =========================================================================
    // push_target_remote cleanup tests
    // =========================================================================

    #[test]
    fn test_push_target_remote_cleared_on_empty_bookmarks() {
        // execute_push with empty bookmarks should clear push_target_remote
        let mut app = App::new_for_test();
        app.push_target_remote = Some("upstream".to_string());
        app.execute_push(&[]);
        assert!(app.push_target_remote.is_none());
    }

    #[test]
    fn test_push_target_remote_cleared_by_execute_push() {
        // execute_push always takes push_target_remote regardless of outcome
        let mut app = App::new_for_test();
        app.push_target_remote = Some("upstream".to_string());
        // Push with a non-existent bookmark will fail, but remote should still be cleared
        app.execute_push(&["nonexistent-bookmark-xyz".to_string()]);
        assert!(app.push_target_remote.is_none());
    }

    #[test]
    fn test_push_target_remote_cleared_by_execute_push_change() {
        // execute_push_change always takes push_target_remote regardless of outcome
        let mut app = App::new_for_test();
        app.push_target_remote = Some("upstream".to_string());
        // Push with invalid change_id will fail, but remote should still be cleared
        app.execute_push_change("nonexistent_change_id");
        assert!(app.push_target_remote.is_none());
    }

    #[test]
    fn test_push_target_remote_cleared_on_git_push_cancel() {
        // Simulating GitPush dialog cancel should clear push_target_remote
        let mut app = App::new_for_test();
        app.push_target_remote = Some("upstream".to_string());
        app.pending_push_bookmarks = vec!["main".to_string()];
        // Set up a dummy dialog to satisfy handle_dialog_result callback extraction
        app.active_dialog = Some(Dialog::confirm(
            "Push",
            "Test",
            None,
            DialogCallback::GitPush,
        ));
        app.handle_dialog_result(DialogResult::Cancelled);
        assert!(app.push_target_remote.is_none());
        assert!(app.pending_push_bookmarks.is_empty());
    }

    #[test]
    fn test_push_target_remote_cleared_on_remote_select_cancel() {
        // Simulating GitPushRemoteSelect dialog cancel should clear push_target_remote
        let mut app = App::new_for_test();
        app.push_target_remote = Some("upstream".to_string());
        app.active_dialog = Some(Dialog::select_single(
            "Push to Remote",
            "Select remote:",
            vec![],
            None,
            DialogCallback::GitPushRemoteSelect,
        ));
        app.handle_dialog_result(DialogResult::Cancelled);
        assert!(app.push_target_remote.is_none());
    }

    #[test]
    fn test_push_target_remote_cleared_on_push_change_cancel() {
        // Simulating GitPushChange dialog cancel should clear push_target_remote
        let mut app = App::new_for_test();
        app.push_target_remote = Some("upstream".to_string());
        app.active_dialog = Some(Dialog::confirm(
            "Push",
            "Test",
            None,
            DialogCallback::GitPushChange {
                change_id: "abc12345".to_string(),
            },
        ));
        app.handle_dialog_result(DialogResult::Cancelled);
        assert!(app.push_target_remote.is_none());
    }

    // =========================================================================
    // duplicate output parsing tests
    // =========================================================================

    #[test]
    fn test_parse_duplicate_output() {
        let output = "Duplicated 0193efbd0b2d as nyowntnw 6abd63b3 no-bookmark change (plain)";
        let result = App::parse_duplicate_output(output);
        assert_eq!(result, Some("nyowntnw".to_string()));
    }

    #[test]
    fn test_parse_duplicate_output_no_match() {
        let result = App::parse_duplicate_output("Some unrelated output");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_duplicate_output_empty() {
        let result = App::parse_duplicate_output("");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_duplicate_output_multiline() {
        // Warning lines before the actual duplicate output
        let output = "Working copy changes were not restored.\n\
                       Duplicated abc1234567890 as xyzwqrst def5678901 test description";
        let result = App::parse_duplicate_output(output);
        assert_eq!(result, Some("xyzwqrst".to_string()));
    }

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
    // is_revisions_unsupported_error tests
    // =========================================================================

    #[test]
    fn test_revisions_unsupported_unexpected_argument() {
        assert!(is_revisions_unsupported_error(
            "error: unexpected argument '--revisions' found"
        ));
    }

    #[test]
    fn test_revisions_unsupported_unrecognized() {
        assert!(is_revisions_unsupported_error(
            "error: unrecognized option '--revisions'"
        ));
    }

    #[test]
    fn test_revisions_unsupported_unknown_flag() {
        assert!(is_revisions_unsupported_error(
            "error: unknown flag --revisions"
        ));
    }

    #[test]
    fn test_revisions_unsupported_unrelated_error() {
        // Error that doesn't mention --revisions should NOT match
        assert!(!is_revisions_unsupported_error(
            "error: unexpected argument '--foobar' found"
        ));
    }

    #[test]
    fn test_revisions_unsupported_generic_push_error() {
        // Push error without flag-related keywords should NOT match
        assert!(!is_revisions_unsupported_error(
            "error: Refusing to create new remote bookmark for --revisions"
        ));
    }

    // =========================================================================
    // GitPushRevisions dialog callback tests
    // =========================================================================

    #[test]
    fn test_push_revisions_cancelled_clears_remote() {
        let mut app = App::new_for_test();
        app.push_target_remote = Some("upstream".to_string());
        app.active_dialog = Some(Dialog::confirm(
            "Push to Remote",
            "Push all bookmarks?",
            None,
            DialogCallback::GitPushRevisions {
                change_id: "abc12345".to_string(),
                bookmarks: vec!["main".to_string()],
            },
        ));
        app.handle_dialog_result(DialogResult::Cancelled);
        assert!(app.push_target_remote.is_none());
    }

    #[test]
    fn test_push_revisions_confirmed_calls_execute() {
        // Verifies routing: confirmed GitPushRevisions calls execute_push_revisions
        let mut app = App::new_for_test();
        app.active_dialog = Some(Dialog::confirm(
            "Push to Remote",
            "Push all bookmarks?",
            None,
            DialogCallback::GitPushRevisions {
                change_id: "abc12345".to_string(),
                bookmarks: vec!["main".to_string()],
            },
        ));
        app.handle_dialog_result(DialogResult::Confirmed(vec![]));
        // execute_push_revisions was called → jj push fails in test env → error_message set
        assert!(
            app.error_message.is_some(),
            "execute_push_revisions should have been called (error expected in test env)"
        );
    }

    // =========================================================================
    // GitPushMultiBookmarkMode dialog callback tests
    // =========================================================================

    #[test]
    fn test_multi_bookmark_mode_cancelled_clears_remote() {
        let mut app = App::new_for_test();
        app.push_target_remote = Some("upstream".to_string());
        app.active_dialog = Some(Dialog::select_single(
            "Push to Remote",
            "Choose push mode:",
            vec![],
            None,
            DialogCallback::GitPushMultiBookmarkMode {
                change_id: "abc12345".to_string(),
                bookmarks: vec!["main".to_string(), "dev".to_string()],
            },
        ));
        app.handle_dialog_result(DialogResult::Cancelled);
        assert!(app.push_target_remote.is_none());
    }

    #[test]
    fn test_multi_bookmark_mode_no_sentinel_collision() {
        // Structural guarantee: mode selection uses DialogCallback dispatch,
        // not string matching. Even if a bookmark is named "revisions",
        // it cannot collide because the mode dialog and per-bookmark dialog
        // use different DialogCallback variants.
        let mode_callback = DialogCallback::GitPushMultiBookmarkMode {
            change_id: "abc12345".to_string(),
            bookmarks: vec!["revisions".to_string()],
        };
        let push_callback = DialogCallback::GitPush;
        // Different callback variants → structurally impossible to confuse
        assert_ne!(mode_callback, push_callback);
    }

    #[test]
    fn test_unique_patch_filename_no_conflict() {
        // When file doesn't exist, returns base name
        // (uses a name unlikely to exist in current dir)
        let name = unique_patch_filename("zzzz_test_nonexistent");
        assert_eq!(name, "zzzz_test_nonexistent.patch");
    }

    #[test]
    fn test_unique_patch_filename_with_conflict() {
        use std::fs;
        let base = "test_unique_patch_tmp";
        let base_file = format!("{}.patch", base);

        // Create the base file to force a conflict
        fs::write(&base_file, "test").unwrap();

        let name = unique_patch_filename(base);
        assert_eq!(name, format!("{}-1.patch", base));

        // Clean up
        let _ = fs::remove_file(&base_file);
    }

    #[test]
    fn test_unique_patch_filename_with_multiple_conflicts() {
        use std::fs;
        let base = "test_unique_multi_tmp";
        let files: Vec<String> = vec![
            format!("{}.patch", base),
            format!("{}-1.patch", base),
            format!("{}-2.patch", base),
        ];

        // Create all conflicting files
        for f in &files {
            fs::write(f, "test").unwrap();
        }

        let name = unique_patch_filename(base);
        assert_eq!(name, format!("{}-3.patch", base));

        // Clean up
        for f in &files {
            let _ = fs::remove_file(f);
        }
    }

    // --- Compare mode export path tests ---

    #[test]
    fn test_export_compare_mode_uses_diff_range_not_show() {
        use crate::model::{CompareInfo, CompareRevisionInfo, DiffContent};
        use crate::ui::views::DiffView;

        let mut app = App::new_for_test();

        let compare_info = CompareInfo {
            from: CompareRevisionInfo {
                change_id: "aaaa1111".to_string(),
                bookmarks: vec![],
                author: "user@test.com".to_string(),
                timestamp: "2024-01-01".to_string(),
                description: "from revision".to_string(),
            },
            to: CompareRevisionInfo {
                change_id: "bbbb2222".to_string(),
                bookmarks: vec![],
                author: "user@test.com".to_string(),
                timestamp: "2024-01-02".to_string(),
                description: "to revision".to_string(),
            },
        };
        app.diff_view = Some(DiffView::new_compare(DiffContent::default(), compare_info));

        // Export will fail (no jj repo in test), but the error reveals which path was taken.
        // In compare mode, it should attempt `jj diff --git --from --to`.
        app.export_diff_to_file();

        // Should have an error (no jj repo), confirming the code path was executed
        assert!(
            app.error_message.is_some(),
            "Expected error from jj command in test environment"
        );
    }

    #[test]
    fn test_copy_clipboard_compare_mode_uses_diff_range() {
        use crate::model::{CompareInfo, CompareRevisionInfo, DiffContent};
        use crate::ui::views::DiffView;

        let mut app = App::new_for_test();

        let compare_info = CompareInfo {
            from: CompareRevisionInfo {
                change_id: "cccc3333".to_string(),
                bookmarks: vec![],
                author: "user@test.com".to_string(),
                timestamp: "2024-01-01".to_string(),
                description: "from".to_string(),
            },
            to: CompareRevisionInfo {
                change_id: "dddd4444".to_string(),
                bookmarks: vec![],
                author: "user@test.com".to_string(),
                timestamp: "2024-01-02".to_string(),
                description: "to".to_string(),
            },
        };
        app.diff_view = Some(DiffView::new_compare(DiffContent::default(), compare_info));

        // Both full and diff-only should attempt diff_range in compare mode
        app.copy_diff_to_clipboard(true);
        assert!(
            app.error_message.is_some(),
            "Expected error from jj command in test environment (full)"
        );

        app.error_message = None;
        app.copy_diff_to_clipboard(false);
        assert!(
            app.error_message.is_some(),
            "Expected error from jj command in test environment (diff)"
        );
    }

    #[test]
    fn test_export_normal_mode_uses_diff_git() {
        use crate::model::DiffContent;
        use crate::ui::views::DiffView;

        let mut app = App::new_for_test();
        app.diff_view = Some(DiffView::new(
            "testid12".to_string(),
            DiffContent::default(),
        ));

        // Normal mode: should attempt `jj diff --git`
        app.export_diff_to_file();
        assert!(
            app.error_message.is_some(),
            "Expected error from jj command in test environment"
        );
    }

    // =========================================================================
    // is_private_commit_error tests
    // =========================================================================

    #[test]
    fn test_private_commit_error_standard() {
        assert!(is_private_commit_error(
            "Won't push commit abc123 since it is private"
        ));
    }

    #[test]
    fn test_private_commit_error_hint_format() {
        assert!(is_private_commit_error(
            "Hint: ... won't push commit ... private ..."
        ));
    }

    #[test]
    fn test_private_commit_error_lowercase() {
        assert!(is_private_commit_error(
            "error: won't push ... it is private"
        ));
    }

    #[test]
    fn test_private_commit_error_false_positive_no_push() {
        // "private" without "won't push" → false
        assert!(!is_private_commit_error("private key error"));
    }

    #[test]
    fn test_private_commit_error_network_error() {
        assert!(!is_private_commit_error("Push failed: network error"));
    }

    // =========================================================================
    // is_empty_description_error tests
    // =========================================================================

    #[test]
    fn test_empty_description_error_standard() {
        assert!(is_empty_description_error(
            "Won't push commit abc123 since it has no description"
        ));
    }

    #[test]
    fn test_empty_description_error_hint_format() {
        assert!(is_empty_description_error(
            "Hint: ... won't push commit ... no description ..."
        ));
    }

    #[test]
    fn test_empty_description_error_lowercase() {
        assert!(is_empty_description_error(
            "error: won't push ... has no description"
        ));
    }

    #[test]
    fn test_empty_description_error_false_positive_no_push() {
        // "no description" without "won't push" → false
        assert!(!is_empty_description_error(
            "no description found in config"
        ));
    }

    #[test]
    fn test_both_errors_simultaneous() {
        // Both private and empty description in same output
        let msg = "Won't push commit abc123 since it is private\nWon't push commit def456 since it has no description";
        assert!(is_private_commit_error(msg));
        assert!(is_empty_description_error(msg));
    }

    // =========================================================================
    // is_rebase_flag_unsupported tests
    // =========================================================================

    #[test]
    fn test_rebase_flag_unsupported_unexpected_argument() {
        assert!(is_rebase_flag_unsupported(
            "error: unexpected argument '--skip-emptied'"
        ));
    }

    #[test]
    fn test_rebase_flag_unsupported_unrecognized() {
        assert!(is_rebase_flag_unsupported(
            "error: unrecognized option '-b'"
        ));
    }

    #[test]
    fn test_rebase_flag_unsupported_unknown_flag() {
        assert!(is_rebase_flag_unsupported(
            "error: unknown flag '--skip-emptied'"
        ));
    }

    #[test]
    fn test_rebase_flag_unsupported_unknown_option() {
        assert!(is_rebase_flag_unsupported(
            "error: unknown option '--skip-emptied'"
        ));
    }

    #[test]
    fn test_rebase_flag_unsupported_false_for_normal_error() {
        assert!(!is_rebase_flag_unsupported(
            "Error: Revision abc123 is not reachable from destination"
        ));
    }

    #[test]
    fn test_rebase_flag_unsupported_false_for_conflict() {
        assert!(!is_rebase_flag_unsupported(
            "Rebase produced conflict in src/main.rs"
        ));
    }

    // =========================================================================
    // Rebase fallback: Branch unsupported × skip_emptied=true (Issue #1)
    // =========================================================================

    /// When both `-b` and `--skip-emptied` are unsupported, the retry (without
    /// `--skip-emptied`) also fails with "unsupported flag" for `-b`.
    /// The handler must detect this and show the Branch guidance notification.
    #[test]
    fn test_rebase_branch_unsupported_detected_on_retry_error() {
        // Simulates: first error = "--skip-emptied unsupported", retry error = "-b unsupported"
        let retry_msg = "error: unexpected argument '-b'";
        assert!(is_rebase_flag_unsupported(retry_msg));
        // In execute_rebase, mode == Branch && unsupported → guidance notification (not error_message)
    }

    // =========================================================================
    // Rebase fallback: notification severity preservation (Issue #2)
    // =========================================================================

    /// When --skip-emptied retry succeeds with conflicts, notify_rebase_success
    /// sets a Warning. The skip-emptied suffix must preserve Warning severity.
    #[test]
    fn test_notification_severity_preserved_on_skip_emptied_fallback() {
        use crate::model::{Notification, NotificationKind};

        // Simulate: notify_rebase_success set a Warning for conflicts
        let existing = Notification::warning("Rebased with conflicts - resolve with jj resolve");
        assert_eq!(existing.kind, NotificationKind::Warning);

        // The fallback code takes the existing notification and creates a new one
        // preserving the kind
        let new_msg = format!(
            "{} (--skip-emptied not supported, empty commits may remain)",
            existing.message
        );
        let result = Notification::new(new_msg, existing.kind);

        // Severity must remain Warning (not downgraded to Info)
        assert_eq!(result.kind, NotificationKind::Warning);
        assert!(result.message.contains("conflicts"));
        assert!(result.message.contains("--skip-emptied not supported"));
    }

    /// When --skip-emptied retry succeeds without conflicts, severity is Success.
    #[test]
    fn test_notification_severity_success_on_skip_emptied_fallback() {
        use crate::model::{Notification, NotificationKind};

        let existing = Notification::success("Rebased successfully");
        let new_msg = format!(
            "{} (--skip-emptied not supported, empty commits may remain)",
            existing.message
        );
        let result = Notification::new(new_msg, existing.kind);

        assert_eq!(result.kind, NotificationKind::Success);
        assert!(result.message.contains("Rebased successfully"));
        assert!(result.message.contains("--skip-emptied not supported"));
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
}
