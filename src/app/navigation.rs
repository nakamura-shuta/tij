//! View navigation (opening views with data loading)

use crate::jj::parser::{Parser, parse_evolog};
use crate::model::{CompareInfo, CompareRevisionInfo, Notification};
use crate::ui::views::{BlameView, DiffView, EvologView, ResolveView};

use super::state::{App, View};

impl App {
    /// Open diff view for a specific change
    pub(crate) fn open_diff(&mut self, change_id: &str) {
        match self.jj.show(change_id) {
            Ok(content) => {
                self.diff_view = Some(DiffView::new(change_id.to_string(), content));
                self.go_to_view(View::Diff);
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to load diff: {}", e));
            }
        }
    }

    /// Open diff view for a specific change and jump to a file
    pub(crate) fn open_diff_at_file(&mut self, change_id: &str, file_path: &str) {
        match self.jj.show(change_id) {
            Ok(content) => {
                let mut diff_view = DiffView::new(change_id.to_string(), content);
                // Jump to the specified file
                diff_view.jump_to_file(file_path);
                self.diff_view = Some(diff_view);
                self.go_to_view(View::Diff);
                self.error_message = None;
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to load diff: {}", e));
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
                self.error_message = Some(format!("Failed to load blame: {}", e));
            }
        }
    }

    /// Open compare diff view between two revisions
    pub(crate) fn open_compare_diff(&mut self, from: &str, to: &str) {
        // Get diff output
        let diff_output = match self.jj.diff_range(from, to) {
            Ok(output) => output,
            Err(e) => {
                self.error_message = Some(format!("Failed to load diff: {}", e));
                return;
            }
        };

        // Get metadata for both revisions
        let from_info = match self.jj.get_change_info(from) {
            Ok((change_id, bookmarks, author, timestamp, description)) => CompareRevisionInfo {
                change_id,
                bookmarks,
                author,
                timestamp,
                description,
            },
            Err(e) => {
                self.error_message = Some(format!("Failed to load from revision: {}", e));
                return;
            }
        };

        let to_info = match self.jj.get_change_info(to) {
            Ok((change_id, bookmarks, author, timestamp, description)) => CompareRevisionInfo {
                change_id,
                bookmarks,
                author,
                timestamp,
                description,
            },
            Err(e) => {
                self.error_message = Some(format!("Failed to load to revision: {}", e));
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

    /// Open operation history view
    pub(crate) fn open_operation_history(&mut self) {
        self.go_to_view(View::Operation);
    }

    /// Open evolution log view for a change
    pub(crate) fn open_evolog(&mut self, change_id: &str) {
        match self.jj.evolog(change_id) {
            Ok(output) => {
                let entries = parse_evolog(&output);
                if entries.is_empty() {
                    self.notification =
                        Some(Notification::info("No evolution history for this change"));
                } else {
                    self.evolog_view = Some(EvologView::new(change_id.to_string(), entries));
                    self.go_to_view(View::Evolog);
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to load evolog: {}", e));
            }
        }
    }

    /// Open resolve view for a change
    ///
    /// Runs `jj resolve --list` and opens the Resolve List View if conflicts exist.
    pub(crate) fn open_resolve_view(&mut self, change_id: &str, is_working_copy: bool) {
        match self.jj.resolve_list(Some(change_id)) {
            Ok(files) => {
                if files.is_empty() {
                    self.notification = Some(Notification::info("No conflicts in this change"));
                } else {
                    self.resolve_view = Some(ResolveView::new(
                        change_id.to_string(),
                        is_working_copy,
                        files,
                    ));
                    self.go_to_view(View::Resolve);
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to list conflicts: {}", e));
            }
        }
    }
}
