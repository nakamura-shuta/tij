//! View navigation (opening views with data loading)

use crate::jj::parser::{Parser, parse_evolog};
use crate::model::{ChangeId, CommitId, CompareInfo, CompareRevisionInfo, Notification};
use crate::ui::views::{BlameView, DiffView, EvologView, ResolveView};

use super::state::{App, View};

impl App {
    /// Open diff view for a specific change
    pub(crate) fn open_diff(&mut self, revision: &str) {
        match self.jj.show(revision) {
            Ok(content) => {
                self.diff_view = Some(DiffView::new(revision.to_string(), content));
                self.go_to_view(View::Diff);
                self.error_message = None;
            }
            Err(e) => {
                self.set_error(format!("Failed to load diff: {}", e));
            }
        }
    }

    /// Open diff view in stack mode for the stack containing `revision`
    ///
    /// `revision` is a change_id for the working copy, commit_id otherwise
    /// (OpenDiff convention — see `LogAction::ShowStackDiff`).
    ///
    /// Conservative revset: `(<rev>::@) | <rev>` — the full stack when the
    /// selected change is an ancestor of @ (or @ itself), otherwise just the
    /// selected change. One jj call, no pre-checks needed.
    pub(crate) fn open_stack_diff(&mut self, revision: &str) {
        let revset = format!("({0}::@) | {0}", revision);
        self.open_stack_diff_revset(&revset);
    }

    /// Maximum number of changes a stack diff will load (matches the
    /// project-wide `--limit 200` query convention)
    const STACK_DIFF_MAX_CHANGES: usize = 200;

    /// Open diff view in stack mode for an explicit revset (also used by refresh)
    pub(crate) fn open_stack_diff_revset(&mut self, revset: &str) {
        // Guard: selecting an old (e.g. near-root) change would make the
        // revset cover hundreds of revisions — check the count first.
        match self
            .jj
            .count_revisions_capped(revset, Self::STACK_DIFF_MAX_CHANGES)
        {
            Ok(n) if n > Self::STACK_DIFF_MAX_CHANGES => {
                self.set_error(format!(
                    "Stack too large: more than {} changes",
                    Self::STACK_DIFF_MAX_CHANGES
                ));
                return;
            }
            Ok(_) => {}
            Err(e) => {
                self.set_error(format!("Failed to load stack diff: {}", e));
                return;
            }
        }

        match self.jj.show_stack(revset) {
            Ok(content) => {
                self.diff_view = Some(DiffView::new_stack(revset.to_string(), content));
                self.go_to_view(View::Diff);
                self.error_message = None;
            }
            Err(e) => {
                self.set_error(format!("Failed to load stack diff: {}", e));
            }
        }
    }

    /// Open diff view for a specific change and jump to a file
    pub(crate) fn open_diff_at_file(&mut self, revision: &str, file_path: &str) {
        match self.jj.show(revision) {
            Ok(content) => {
                let mut diff_view = DiffView::new(revision.to_string(), content);
                // Jump to the specified file
                diff_view.jump_to_file(file_path);
                self.diff_view = Some(diff_view);
                self.go_to_view(View::Diff);
                self.error_message = None;
            }
            Err(e) => {
                self.set_error(format!("Failed to load diff: {}", e));
            }
        }
    }

    /// Open blame view for a specific file
    ///
    /// Optionally accepts a revision to annotate. If None, uses the working copy.
    pub(crate) fn open_blame(&mut self, file_path: &str, revision: Option<&str>) {
        match self.jj.file_annotate(file_path, revision) {
            Ok(content) => {
                let mut blame_view = BlameView::new();
                blame_view.set_content(content, revision.map(|s| s.to_string()));
                self.blame_view = Some(blame_view);
                self.go_to_view(View::Blame);
                self.error_message = None;
            }
            Err(e) => {
                self.set_error(format!("Failed to load blame: {}", e));
            }
        }
    }

    /// Open compare diff view between two revisions
    pub(crate) fn open_compare_diff(&mut self, from: &str, to: &str) {
        // Get diff output
        let diff_output = match self.jj.diff_range(from, to) {
            Ok(output) => output,
            Err(e) => {
                self.set_error(format!("Failed to load diff: {}", e));
                return;
            }
        };

        // Fetch metadata for both revisions in parallel (independent reads)
        let (from_result, to_result) = std::thread::scope(|s| {
            let from_handle = s.spawn(|| self.jj.get_change_info(from));
            let to_handle = s.spawn(|| self.jj.get_change_info(to));
            (from_handle.join().unwrap(), to_handle.join().unwrap())
        });

        let from_info = match from_result {
            Ok((change_id, bookmarks, author, timestamp, description)) => CompareRevisionInfo {
                change_id: ChangeId::new(change_id),
                commit_id: CommitId::new(from.to_string()),
                bookmarks,
                author,
                timestamp,
                description,
            },
            Err(e) => {
                self.set_error(format!("Failed to load from revision: {}", e));
                return;
            }
        };

        let to_info = match to_result {
            Ok((change_id, bookmarks, author, timestamp, description)) => CompareRevisionInfo {
                change_id: ChangeId::new(change_id),
                commit_id: CommitId::new(to.to_string()),
                bookmarks,
                author,
                timestamp,
                description,
            },
            Err(e) => {
                self.set_error(format!("Failed to load to revision: {}", e));
                return;
            }
        };

        // Parse diff body
        let content = Parser::parse_diff_body(&diff_output);

        let compare_info = CompareInfo {
            from: from_info,
            to: to_info,
        };

        let diff_view = DiffView::new_compare(content, compare_info);
        self.diff_view = Some(diff_view);
        self.go_to_view(View::Diff);
        self.error_message = None;
    }

    /// Open interdiff view between two revisions
    pub(crate) fn open_interdiff(&mut self, from: &str, to: &str) {
        // Get interdiff output
        let diff_output = match self.jj.interdiff(from, to) {
            Ok(output) => output,
            Err(e) => {
                self.set_error(format!("Failed to load interdiff: {}", e));
                return;
            }
        };

        // Fetch metadata for both revisions in parallel (independent reads)
        let (from_result, to_result) = std::thread::scope(|s| {
            let from_handle = s.spawn(|| self.jj.get_change_info(from));
            let to_handle = s.spawn(|| self.jj.get_change_info(to));
            (from_handle.join().unwrap(), to_handle.join().unwrap())
        });

        let from_info = match from_result {
            Ok((change_id, bookmarks, author, timestamp, description)) => CompareRevisionInfo {
                change_id: ChangeId::new(change_id),
                commit_id: CommitId::new(from.to_string()),
                bookmarks,
                author,
                timestamp,
                description,
            },
            Err(e) => {
                self.set_error(format!("Failed to load from revision: {}", e));
                return;
            }
        };

        let to_info = match to_result {
            Ok((change_id, bookmarks, author, timestamp, description)) => CompareRevisionInfo {
                change_id: ChangeId::new(change_id),
                commit_id: CommitId::new(to.to_string()),
                bookmarks,
                author,
                timestamp,
                description,
            },
            Err(e) => {
                self.set_error(format!("Failed to load to revision: {}", e));
                return;
            }
        };

        // Parse diff body (interdiff output has same format as diff)
        let content = Parser::parse_diff_body(&diff_output);

        let compare_info = CompareInfo {
            from: from_info,
            to: to_info,
        };

        let diff_view = DiffView::new_interdiff(content, compare_info);
        self.diff_view = Some(diff_view);
        self.go_to_view(View::Diff);
        self.error_message = None;
    }

    /// Open operation history view
    pub(crate) fn open_operation_history(&mut self) {
        self.go_to_view(View::Operation);
    }

    /// Open evolution log view for a change
    pub(crate) fn open_evolog(&mut self, revision: &str) {
        match self.jj.evolog(revision) {
            Ok(output) => {
                let entries = parse_evolog(&output);
                if entries.is_empty() {
                    self.notification =
                        Some(Notification::info("No evolution history for this change"));
                } else {
                    self.evolog_view = Some(EvologView::new(revision.to_string(), entries));
                    self.go_to_view(View::Evolog);
                }
            }
            Err(e) => {
                self.set_error(format!("Failed to load evolog: {}", e));
            }
        }
    }

    /// Open resolve view for a change
    ///
    /// Runs `jj resolve --list` and opens the Resolve List View if conflicts exist.
    pub(crate) fn open_resolve_view(&mut self, revision: &str, is_working_copy: bool) {
        match self.jj.resolve_list(Some(revision)) {
            Ok(files) => {
                if files.is_empty() {
                    self.notify_info("No conflicts in this change");
                } else {
                    self.resolve_view = Some(ResolveView::new(
                        revision.to_string(),
                        is_working_copy,
                        files,
                    ));
                    self.go_to_view(View::Resolve);
                }
            }
            Err(e) => {
                self.set_error(format!("Failed to list conflicts: {}", e));
            }
        }
    }
}
