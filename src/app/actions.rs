//! jj operations (actions that modify repository state)

use crate::jj::{PushPreviewResult, parse_push_dry_run};
use crate::model::Notification;
use crate::ui::components::{Dialog, DialogCallback, DialogResult, SelectItem};
use crate::ui::views::RebaseMode;

use super::state::{App, View};

impl App {
    /// Execute undo operation
    pub(crate) fn execute_undo(&mut self) {
        match self.jj.undo() {
            Ok(_) => {
                self.notification = Some(Notification::success("Undo complete"));
                // Refresh log to show updated state
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
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
        let revset = self.log_view.current_revset.clone();
        self.refresh_log(revset.as_deref());
        self.refresh_status();
    }

    /// Execute describe operation
    pub(crate) fn execute_describe(&mut self, change_id: &str, message: &str) {
        match self.jj.describe(change_id, message) {
            Ok(_) => {
                self.notification = Some(Notification::success("Description updated"));
                // Refresh log and status to show updated description
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();
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
                // Refresh log to show @ marker moved
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
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
                // Refresh log to show new change
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();
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
                // Refresh log to show new change
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();
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
                // Refresh status view to show clean state
                self.refresh_status();
                // Also refresh log view
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
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
        let revset = self.log_view.current_revset.clone();
        self.refresh_log(revset.as_deref());
        self.refresh_status();
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
                // Refresh log to show updated state
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                // Also refresh status view (abandon may affect working copy)
                self.refresh_status();
            }
            Err(e) => {
                self.error_message = Some(format!("Abandon failed: {}", e));
            }
        }
    }

    /// Execute redo operation
    ///
    /// Only works if the last operation was an undo.
    pub(crate) fn execute_redo(&mut self) {
        // First, check if last operation was an undo and get the target
        match self.jj.get_redo_target() {
            Ok(Some(op_id)) => {
                match self.jj.redo(&op_id) {
                    Ok(_) => {
                        self.notification = Some(Notification::success("Redo complete"));
                        // Refresh log to show updated state
                        let revset = self.log_view.current_revset.clone();
                        self.refresh_log(revset.as_deref());
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Redo failed: {}", e));
                    }
                }
            }
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
                // Refresh log and status after restore
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();
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
        let revset = self.log_view.current_revset.clone();
        self.refresh_log(revset.as_deref());
        self.refresh_status();
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
                // Refresh log to show bookmark
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
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
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
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
                // Refresh log
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to delete bookmarks: {}", e));
            }
        }
    }

    /// Execute absorb: move working copy changes into ancestor commits
    ///
    /// Each hunk is moved to the closest mutable ancestor where the
    /// corresponding lines were modified last.
    pub(crate) fn execute_absorb(&mut self) {
        match self.jj.absorb() {
            Ok(output) => {
                // Refresh both log and status
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();

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

    /// Execute git fetch
    pub(crate) fn execute_fetch(&mut self) {
        match self.jj.git_fetch() {
            Ok(output) => {
                // Refresh all views after fetch
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();

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

    /// Start push flow with dry-run preview
    ///
    /// Runs `jj git push --dry-run` to preview what will be pushed,
    /// then shows a confirmation/selection dialog with the preview.
    /// If dry-run fails (untracked bookmark, etc.), falls back to dialog without preview.
    pub(crate) fn start_push(&mut self) {
        let (change_id, bookmarks) = match self.log_view.selected_change() {
            Some(change) => (change.change_id.clone(), change.bookmarks.clone()),
            None => return,
        };

        if bookmarks.is_empty() {
            self.notification = Some(Notification::info("No bookmarks to push on this change"));
            return;
        }

        if bookmarks.len() == 1 {
            let name = &bookmarks[0];

            // Run dry-run to preview push
            match self.jj.git_push_dry_run(name) {
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
            // Multiple bookmarks: run dry-run for each to get status labels
            let mut items: Vec<SelectItem> = Vec::new();
            for name in &bookmarks {
                let status = match self.jj.git_push_dry_run(name) {
                    Ok(output) => {
                        let preview = parse_push_dry_run(&output);
                        format_bookmark_status(&preview, name)
                    }
                    Err(_) => String::new(), // dry-run failed → no label
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
                format!("Select bookmarks to push from {}:", &change_id[..8]),
                items,
                Some("Remote changes cannot be undone with 'u'.".to_string()),
                DialogCallback::GitPush,
            ));
        }
    }

    /// Execute git push for the specified bookmarks
    ///
    /// If `jj git push --bookmark` fails for an untracked/new bookmark,
    /// retries with `--allow-new` (deprecated in jj 0.37+ but functional).
    /// On success via --allow-new, adds a hint about configuring auto-track.
    pub(crate) fn execute_push(&mut self, bookmark_names: &[String]) {
        if bookmark_names.is_empty() {
            return;
        }

        let mut successes = Vec::new();
        let mut errors = Vec::new();
        let mut used_allow_new = false;

        for name in bookmark_names {
            match self.jj.git_push_bookmark(name) {
                Ok(_) => {
                    successes.push(name.clone());
                }
                Err(e) => {
                    let err_msg = format!("{}", e);
                    // Auto-retry with --allow-new if the bookmark is new on remote.
                    // --allow-new is deprecated in jj 0.37+ but still functional.
                    if is_untracked_bookmark_error(&err_msg) {
                        match self.jj.git_push_bookmark_allow_new(name) {
                            Ok(_) => {
                                successes.push(name.clone());
                                used_allow_new = true;
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

        // Show result
        if !successes.is_empty() {
            let names = successes.join(", ");
            let msg = if used_allow_new {
                // Hint about auto-track config when --allow-new was needed
                format!("Pushed bookmark: {} (used deprecated --allow-new)", names)
            } else {
                format!("Pushed bookmark: {}", names)
            };
            self.notification = Some(Notification::success(msg));
        }
        if !errors.is_empty() {
            let msg = errors.join("; ");
            self.error_message = Some(format!("Push failed: {}", msg));
        }

        // Always clear pending state after execution (prevent stale data)
        self.pending_push_bookmarks.clear();

        // Refresh after push (log + status)
        let revset = self.log_view.current_revset.clone();
        self.refresh_log(revset.as_deref());
        self.refresh_status();
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
    /// Supports four modes:
    /// - `Revision` (`-r`): Move single change, descendants rebased onto parent
    /// - `Source` (`-s`): Move change and all descendants together
    /// - `InsertAfter` (`-A`): Insert change after target in history
    /// - `InsertBefore` (`-B`): Insert change before target in history
    pub(crate) fn execute_rebase(&mut self, source: &str, destination: &str, mode: RebaseMode) {
        // Prevent rebasing to self
        if source == destination {
            self.notification = Some(Notification::warning("Cannot rebase to itself"));
            return;
        }

        let result = match mode {
            RebaseMode::Revision => self.jj.rebase(source, destination),
            RebaseMode::Source => self.jj.rebase_source(source, destination),
            RebaseMode::InsertAfter => self.jj.rebase_insert_after(source, destination),
            RebaseMode::InsertBefore => self.jj.rebase_insert_before(source, destination),
        };

        match result {
            Ok(output) => {
                // Refresh both log and status
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();

                // Unified conflict detection from jj output
                let has_conflict = output.to_lowercase().contains("conflict");
                let notification = if has_conflict {
                    Notification::warning("Rebased with conflicts - resolve with jj resolve")
                } else {
                    let msg = match mode {
                        RebaseMode::Revision => "Rebased successfully".to_string(),
                        RebaseMode::Source => {
                            "Rebased source and descendants successfully".to_string()
                        }
                        RebaseMode::InsertAfter => {
                            let short = &destination[..8.min(destination.len())];
                            format!("Inserted after {} successfully", short)
                        }
                        RebaseMode::InsertBefore => {
                            let short = &destination[..8.min(destination.len())];
                            format!("Inserted before {} successfully", short)
                        }
                    };
                    Notification::success(msg)
                };
                self.notification = Some(notification);
            }
            Err(e) => {
                self.error_message = Some(format!("Rebase failed: {}", e));
            }
        }
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
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
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
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.refresh_status();
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

        let current_id = match self.log_view.selected_change() {
            Some(c) => c.change_id.clone(),
            None => {
                self.preview_cache = None;
                return;
            }
        };

        // Cache hit — same change already fetched
        if let Some(ref cache) = self.preview_cache
            && cache.change_id == current_id
        {
            return;
        }

        // Always defer to idle tick — never block key handling with jj show.
        // resolve_pending_preview() will fetch on the next poll timeout.
        self.preview_pending_id = Some(current_id);
    }

    /// Actually fetch preview content via jj show
    fn fetch_preview(&mut self, change_id: &str) {
        self.preview_pending_id = None;

        // Capture bookmarks at fetch time from the Change model (not jj show)
        // to ensure consistency between content and bookmarks in the cache
        let bookmarks = self
            .log_view
            .selected_change()
            .filter(|c| c.change_id == change_id)
            .map(|c| c.bookmarks.clone())
            .unwrap_or_default();

        match self.jj.show(change_id) {
            Ok(content) => {
                self.preview_cache = Some(super::state::PreviewCache {
                    change_id: change_id.to_string(),
                    content,
                    bookmarks,
                });
            }
            Err(_) => {
                self.preview_cache = None;
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
}
