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

use crate::jj::JjError;
use crate::model::ConflictFile;
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
                // Schedule preview update so it loads on next idle tick.
                // Without this, dialog-based operations (fix, abandon, etc.)
                // leave "No preview available" until the user presses j/k.
                self.update_preview_if_needed();
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

    /// Re-load the current view's data after a repo-mutating op (undo/redo).
    ///
    /// `mark_dirty_and_refresh_current(DirtyFlags::all())` already re-loads the
    /// dirty-flag-backed views (Log/Status/Operation/Bookmark). Tag/Workspace/
    /// Resolve have no dirty flag, so undo/redo — which are repo-wide — must
    /// reload them explicitly to show the post-op state without leaving the view
    /// (Phase 48-D). The `set_*` helpers reset selection to 0, so the cursor
    /// always stays in bounds. This must run AFTER the dirty refresh and only
    /// covers views the dirty path misses (no double jj invocation).
    pub(crate) fn refresh_current_view_after_op(&mut self) {
        match self.current_view {
            View::Tag => self.refresh_tag_view(),
            View::Workspace => self.refresh_workspace_view(),
            View::Resolve => {
                // ResolveView stores its own revision/is_working_copy, so we can
                // re-run the conflict listing for the same target.
                if let Some(ref resolve_view) = self.resolve_view {
                    let revision = resolve_view.revision.clone();
                    let is_wc = resolve_view.is_working_copy;
                    self.refresh_resolve_list(&revision, is_wc);
                }
            }
            // Log/Status/Operation/Bookmark are handled by the dirty refresh;
            // read-only views never reach undo/redo.
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
                // Detect truncation: if selectable (non-graph-only) count equals
                // the limit, results were likely truncated by --limit
                let limit: usize = crate::jj::constants::DEFAULT_LOG_LIMIT
                    .parse()
                    .unwrap_or(200);
                let selectable_count = changes.iter().filter(|c| !c.is_graph_only).count();
                let truncated = selectable_count >= limit;

                self.log_view.set_changes(changes);
                self.log_view.truncated = truncated;
                // Validate cache against new change list: evict stale entries,
                // update bookmarks for entries whose commit_id still matches
                self.preview_cache.validate(&self.log_view.changes);
                self.log_view.current_revset = revset.map(|s| s.to_string());
                // Re-match Agent Trace badges against the new change list
                // (no I/O — the trace file itself reloads on Ctrl+L only)
                self.apply_trace_badges();
                self.error_message = None;
            }
            Err(e) => {
                self.set_error(format!("jj error: {}", e));
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
                self.set_error(format!("jj status error: {}", e));
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
                self.set_error(format!("jj op log error: {}", e));
            }
        }
    }

    /// Refresh the resolve list for the current resolve view
    pub(crate) fn refresh_resolve_list(&mut self, revision: &str, is_working_copy: bool) {
        let result = self.jj.resolve_list(Some(revision));
        self.apply_resolve_list_refresh(result, revision, is_working_copy);
    }

    /// Apply a `resolve --list` result to the refresh path.
    ///
    /// Split out of `refresh_resolve_list` so the "last conflict just got
    /// resolved" branch can be exercised without spawning jj.
    pub(crate) fn apply_resolve_list_refresh(
        &mut self,
        result: Result<Vec<ConflictFile>, JjError>,
        revision: &str,
        is_working_copy: bool,
    ) {
        match result {
            Ok(files) => {
                if files.is_empty() {
                    // All resolved - go back (simple message for Log View title bar)
                    self.notify_success("All conflicts resolved!");
                    self.resolve_view = None;
                    self.go_back();
                    // Refresh log to update conflict indicators
                    let revset = self.log_view.current_revset.clone();
                    self.refresh_log(revset.as_deref());
                } else if let Some(ref mut view) = self.resolve_view {
                    view.set_files(files);
                } else {
                    self.resolve_view = Some(ResolveView::new(
                        revision.to_string(),
                        is_working_copy,
                        files,
                    ));
                }
            }
            // "No conflicts" is normalized to `Ok(vec![])` by
            // `JjExecutor::resolve_list`, so everything reaching here is a
            // real failure. (Handling it in both arms invited fixing only one.)
            Err(e) => {
                self.set_error(format!("Failed to refresh conflicts: {}", e));
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
                // Explicit refresh also re-reads the Agent Trace sidecar
                self.reload_traces();
                self.dirty.log = false;
                self.update_preview_if_needed();
                self.notify_info("Refreshed");
            }
            View::Status => {
                self.refresh_status();
                self.dirty.status = false;
                self.notify_info("Refreshed");
            }
            View::Operation => {
                self.refresh_operation_log();
                self.dirty.op_log = false;
                self.notify_info("Refreshed");
            }
            View::Diff => {
                // Only refresh if diff_view is loaded
                if let Some(ref diff_view) = self.diff_view {
                    use crate::model::DiffMode;
                    match diff_view.mode {
                        DiffMode::Compare => {
                            let ci = diff_view.compare_info.as_ref().unwrap();
                            let from = ci.from.commit_id.to_string();
                            let to = ci.to.commit_id.to_string();
                            self.open_compare_diff(&from, &to);
                        }
                        DiffMode::Interdiff => {
                            let ci = diff_view.compare_info.as_ref().unwrap();
                            let from = ci.from.commit_id.to_string();
                            let to = ci.to.commit_id.to_string();
                            self.open_interdiff(&from, &to);
                        }
                        DiffMode::Single => {
                            let revision = diff_view.revision.clone();
                            self.open_diff(&revision);
                        }
                        DiffMode::Stack => {
                            let revset = diff_view.revision.clone();
                            self.open_stack_diff_revset(&revset);
                        }
                    }
                    self.notify_info("Refreshed");
                }
                // If diff_view is None, do nothing (no notification)
            }
            View::Resolve => {
                // Refresh resolve list
                if let Some(ref resolve_view) = self.resolve_view {
                    let revision = resolve_view.revision.clone();
                    let is_wc = resolve_view.is_working_copy;
                    self.refresh_resolve_list(&revision, is_wc);
                    self.notify_info("Refreshed");
                }
            }
            View::Bookmark => {
                self.refresh_bookmark_view();
                self.dirty.bookmarks = false;
                self.notify_info("Refreshed");
            }
            View::Blame => {
                // Only refresh if blame_view is loaded
                if let Some(ref blame_view) = self.blame_view {
                    let file_path = blame_view.file_path().to_string();
                    let revision = blame_view.revision().map(|s| s.to_string());
                    self.open_blame(&file_path, revision.as_deref());
                    self.notify_info("Refreshed");
                }
            }
            View::Evolog => {
                // Refresh evolog view
                if let Some(ref evolog_view) = self.evolog_view {
                    let revision = evolog_view.revision.clone();
                    self.open_evolog(&revision);
                    // Only show "Refreshed" if open_evolog didn't set an error/notification
                    if self.error_message.is_none() && self.notification.is_none() {
                        self.notify_info("Refreshed");
                    }
                }
            }
            View::Tag => {
                self.refresh_tag_view();
                self.notify_info("Refreshed");
            }
            View::Workspace => {
                self.refresh_workspace_view();
                self.notify_info("Refreshed");
            }
            View::CommandHistory => {
                // Command history is in-memory data, no external refresh needed
            }
            View::TraceDetail => {
                // Trace Detail holds a snapshot taken at open time; the trace
                // sidecar reloads only on Log's Ctrl+L. Re-open from Log to
                // refresh. No-op here.
            }
            View::Help => {
                // Help is static content, no refresh needed, no notification
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jj::JjExecutor;
    use crate::model::NotificationKind;

    fn conflict(path: &str) -> ConflictFile {
        ConflictFile {
            path: path.to_string(),
            description: "2-sided conflict".to_string(),
        }
    }

    /// An App sitting in the Resolve View with one conflict listed.
    ///
    /// The executor points at a path that is not a jj repo so the trailing
    /// `refresh_log` in the "all resolved" branch fails fast instead of
    /// reading whatever repository the test runner happens to sit in (and it
    /// is equally harmless when jj is not installed at all, as in CI's
    /// `cargo test --lib`).
    fn app_in_resolve_view() -> App {
        let mut app = App::new_for_test();
        app.jj = JjExecutor::with_repo_path(std::env::temp_dir().join("tij-not-a-repo"));
        app.resolve_view = Some(ResolveView::new(
            "abc12345".to_string(),
            true,
            vec![conflict("src/main.rs")],
        ));
        app.current_view = View::Resolve;
        app.view_stack = vec![View::Log];
        app
    }

    /// Resolving the last conflict still reports success and drops back to
    /// Log. This used to run through an `Err` branch that string-matched
    /// "No conflicts"; `resolve_list` now normalizes that to `Ok(empty)` and
    /// the workaround is gone — this pins the behaviour across that move.
    #[test]
    fn refresh_with_no_conflicts_reports_all_conflicts_resolved() {
        let mut app = app_in_resolve_view();

        app.apply_resolve_list_refresh(Ok(Vec::new()), "abc12345", true);

        let notification = app.notification.clone().expect("expected a notification");
        assert_eq!(notification.message, "All conflicts resolved!");
        assert_eq!(notification.kind, NotificationKind::Success);
        assert!(app.resolve_view.is_none());
        assert_eq!(app.current_view, View::Log);
        // The conflict listing itself must not raise a banner. (The log
        // refresh that follows may fail without a repo — unrelated.)
        assert!(
            !app.error_message
                .unwrap_or_default()
                .contains("Failed to refresh conflicts"),
        );
    }

    #[test]
    fn refresh_with_remaining_conflicts_updates_the_view_in_place() {
        let mut app = app_in_resolve_view();

        app.apply_resolve_list_refresh(
            Ok(vec![conflict("src/main.rs"), conflict("src/lib.rs")]),
            "abc12345",
            true,
        );

        assert_eq!(app.current_view, View::Resolve);
        let view = app.resolve_view.expect("view should stay open");
        assert_eq!(view.files().len(), 2);
        assert!(app.notification.is_none());
        assert!(app.error_message.is_none());
    }

    /// A real failure must surface as an error and leave the view open — it
    /// must never be mistaken for "all conflicts resolved" (which would close
    /// the view and claim success on a change that is still conflicted).
    #[test]
    fn refresh_with_a_real_error_keeps_the_view_and_shows_the_banner() {
        let mut app = app_in_resolve_view();

        app.apply_resolve_list_refresh(
            Err(JjError::CommandFailed {
                stderr: "Error: Revision `nosuch` doesn't exist".to_string(),
                exit_code: 1,
            }),
            "abc12345",
            true,
        );

        let error = app.error_message.expect("expected an error banner");
        assert!(error.starts_with("Failed to refresh conflicts:"), "{error}");
        assert!(app.notification.is_none(), "must not claim success");
        assert!(app.resolve_view.is_some(), "view must stay open");
        assert_eq!(app.current_view, View::Resolve);
    }
}
