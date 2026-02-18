//! Data refresh operations (reload from jj)
//!
//! ## Concurrency safety
//!
//! All refresh methods (`refresh_log`, `refresh_status`, etc.) are independent
//! read-only jj commands that could theoretically run in parallel. However,
//! `mark_dirty_and_refresh_current()` only refreshes the **current view**,
//! so at most one jj command runs per call. Other views refresh lazily on
//! navigation via `go_to_view()`. This design (from Phase 17.1 DirtyFlags)
//! makes parallel refresh unnecessary for the current architecture.

use crate::model::Notification;
use crate::ui::views::ResolveView;

use super::state::{App, DirtyFlags, View};

impl App {
    /// Set dirty flags and immediately refresh only the current view if affected.
    ///
    /// Other views will be refreshed lazily when navigated to (via `go_to_view()`).
    /// This avoids spawning unnecessary jj subprocesses for views that aren't visible.
    pub(crate) fn mark_dirty_and_refresh_current(&mut self, affected: DirtyFlags) {
        // Clear entire preview cache when all flags are dirty (undo/redo/fetch/op_restore)
        // since we can't know what changed
        if affected == DirtyFlags::all() {
            self.preview_cache.clear();
        }

        // Merge affected flags into current dirty state
        self.dirty.log |= affected.log;
        self.dirty.status |= affected.status;
        self.dirty.op_log |= affected.op_log;
        self.dirty.bookmarks |= affected.bookmarks;

        // Refresh only the currently visible view if it's dirty
        match self.current_view {
            View::Log if self.dirty.log => {
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.dirty.log = false;
            }
            View::Status if self.dirty.status => {
                self.refresh_status();
                self.dirty.status = false;
            }
            View::Operation if self.dirty.op_log => {
                self.refresh_operation_log();
                self.dirty.op_log = false;
            }
            View::Bookmark if self.dirty.bookmarks => {
                self.refresh_bookmark_view();
                self.dirty.bookmarks = false;
            }
            _ => {}
        }
    }

    /// Refresh the log view with optional revset
    ///
    /// Also invalidates the preview cache, since repository state may have changed
    /// (e.g., after describe, edit, squash, rebase, etc.).
    pub fn refresh_log(&mut self, revset: Option<&str>) {
        self.preview_pending_id = None;

        let reversed = self.log_view.reversed;
        match self.jj.log_changes(revset, reversed) {
            Ok(changes) => {
                self.log_view.set_changes(changes);
                // Validate cache against new change list: evict stale entries,
                // update bookmarks for entries whose commit_id still matches
                self.preview_cache.validate(&self.log_view.changes);
                self.log_view.current_revset = revset.map(|s| s.to_string());
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("jj error: {}", e));
            }
        }
    }

    /// Refresh the status view
    pub fn refresh_status(&mut self) {
        match self.jj.status() {
            Ok(status) => {
                self.status_view.set_status(status);
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("jj status error: {}", e));
            }
        }
    }

    /// Refresh the operation history view
    pub fn refresh_operation_log(&mut self) {
        match self.jj.op_log(Some(50)) {
            Ok(operations) => {
                self.operation_view.set_operations(operations);
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("jj op log error: {}", e));
            }
        }
    }

    /// Refresh the resolve list for the current resolve view
    pub(crate) fn refresh_resolve_list(&mut self, change_id: &str, is_working_copy: bool) {
        match self.jj.resolve_list(Some(change_id)) {
            Ok(files) => {
                if files.is_empty() {
                    // All resolved - go back (simple message for Log View title bar)
                    self.notification = Some(Notification::success("All conflicts resolved!"));
                    self.resolve_view = None;
                    self.go_back();
                    // Refresh log to update conflict indicators
                    let revset = self.log_view.current_revset.clone();
                    self.refresh_log(revset.as_deref());
                } else if let Some(ref mut view) = self.resolve_view {
                    view.set_files(files);
                } else {
                    self.resolve_view = Some(ResolveView::new(
                        change_id.to_string(),
                        is_working_copy,
                        files,
                    ));
                }
            }
            Err(e) => {
                // "No conflicts found" means all conflicts were just resolved
                let err_msg = e.to_string();
                if err_msg.contains("No conflicts") {
                    // All resolved - simple message for Log View title bar
                    self.notification = Some(Notification::success("All conflicts resolved!"));
                    self.resolve_view = None;
                    self.go_back();
                    let revset = self.log_view.current_revset.clone();
                    self.refresh_log(revset.as_deref());
                } else {
                    self.error_message = Some(format!("Failed to refresh conflicts: {}", e));
                }
            }
        }
    }

    /// Execute refresh for current view (Ctrl+L)
    ///
    /// Force-refreshes the data for the current view and clears only that
    /// view's dirty flag. Other views' dirty flags are preserved so they
    /// still refresh when navigated to.
    ///
    /// Note: Selection position is NOT preserved after refresh.
    pub(crate) fn execute_refresh(&mut self) {
        match self.current_view {
            View::Log => {
                let revset = self.log_view.current_revset.clone();
                self.refresh_log(revset.as_deref());
                self.dirty.log = false;
                self.notification = Some(Notification::info("Refreshed"));
            }
            View::Status => {
                self.refresh_status();
                self.dirty.status = false;
                self.notification = Some(Notification::info("Refreshed"));
            }
            View::Operation => {
                self.refresh_operation_log();
                self.dirty.op_log = false;
                self.notification = Some(Notification::info("Refreshed"));
            }
            View::Diff => {
                // Only refresh if diff_view is loaded
                if let Some(ref diff_view) = self.diff_view {
                    if let Some(ref compare_info) = diff_view.compare_info {
                        // Compare mode: re-run diff --from --to
                        let from = compare_info.from.change_id.clone();
                        let to = compare_info.to.change_id.clone();
                        self.open_compare_diff(&from, &to);
                    } else {
                        // Normal mode: re-run jj show
                        let change_id = diff_view.change_id.clone();
                        self.open_diff(&change_id);
                    }
                    self.notification = Some(Notification::info("Refreshed"));
                }
                // If diff_view is None, do nothing (no notification)
            }
            View::Resolve => {
                // Refresh resolve list
                if let Some(ref resolve_view) = self.resolve_view {
                    let change_id = resolve_view.change_id.clone();
                    let is_wc = resolve_view.is_working_copy;
                    self.refresh_resolve_list(&change_id, is_wc);
                    self.notification = Some(Notification::info("Refreshed"));
                }
            }
            View::Bookmark => {
                self.refresh_bookmark_view();
                self.dirty.bookmarks = false;
                self.notification = Some(Notification::info("Refreshed"));
            }
            View::Blame => {
                // Only refresh if blame_view is loaded
                if let Some(ref blame_view) = self.blame_view {
                    let file_path = blame_view.file_path().to_string();
                    let revision = blame_view.revision().map(|s| s.to_string());
                    self.open_blame(&file_path, revision.as_deref());
                    self.notification = Some(Notification::info("Refreshed"));
                }
            }
            View::Evolog => {
                // Refresh evolog view
                if let Some(ref evolog_view) = self.evolog_view {
                    let change_id = evolog_view.change_id.clone();
                    self.open_evolog(&change_id);
                    // Only show "Refreshed" if open_evolog didn't set an error/notification
                    if self.error_message.is_none() && self.notification.is_none() {
                        self.notification = Some(Notification::info("Refreshed"));
                    }
                }
            }
            View::Help => {
                // Help is static content, no refresh needed, no notification
            }
        }
    }
}
