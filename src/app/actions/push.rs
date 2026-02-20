//! Git push operations

use crate::jj::{PushBulkMode, PushPreviewResult, parse_push_dry_run};
use crate::ui::components::{Dialog, DialogCallback, SelectItem};

use crate::app::state::{App, DirtyFlags};

impl App {
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
                            self.notify_info(format!(
                                "Nothing to push: {} is already up to date",
                                name
                            ));
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
            self.notify_success(msg);
        }
        if !errors.is_empty() {
            let msg = errors.join("; ");
            self.set_error(format!("Push failed: {}", msg));
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
                            self.set_error(format!("Push failed: {}", e2));
                        }
                    }
                } else {
                    self.set_error(format!("Push failed: {}", e));
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
        self.notify_success(msg);
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
    pub(super) fn start_push_change(&mut self, change_id: &str) {
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
                    self.set_error(format!("Push failed: {}", e));
                }
            }
        }
    }

    /// Show dry-run preview for bulk push, then confirm dialog
    ///
    /// Parses the dry-run output through `parse_push_dry_run()` to detect
    /// force push and protected bookmark scenarios, matching the warning
    /// behavior of single-bookmark push.
    pub(super) fn start_push_bulk(&mut self, mode: PushBulkMode) {
        let remote = self.push_target_remote.clone();

        let dry_run_result = self.jj.git_push_bulk_dry_run(mode, remote.as_deref());
        match dry_run_result {
            Ok(output) => {
                let parsed = parse_push_dry_run(&output);
                match parsed {
                    PushPreviewResult::NothingChanged => {
                        self.push_target_remote = None;
                        self.notify_info(format!("Nothing to push ({})", mode.label()));
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
                            self.notify_info(format!("Nothing to push ({})", mode.label()));
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
                self.set_error(format!("Push failed: {}", e));
            }
        }
    }

    /// Execute bulk push (called after confirmation)
    pub(super) fn execute_push_bulk(&mut self, mode: PushBulkMode, remote: Option<&str>) {
        self.push_target_remote = None;

        match self.jj.git_push_bulk(mode, remote) {
            Ok(_) => {
                self.notify_success(format!("Pushed {}", mode.label()));
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                self.set_error(format!("Push failed: {}", e));
            }
        }
    }

    /// Show individual bookmark multi-select dialog (phase 2 of multi-bookmark push)
    ///
    /// Shows a checkbox-style select dialog with per-bookmark dry-run status labels.
    pub(super) fn show_individual_bookmark_select(
        &mut self,
        change_id: &str,
        bookmarks: &[String],
    ) {
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
    pub(super) fn start_push_revisions(&mut self, change_id: &str, bookmarks: &[String]) {
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
                        self.notify_info(
                            "Nothing to push: all bookmarks are already up to date".to_string(),
                        );
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
                    self.notify_info(
                        "--revisions not supported, pushing bookmarks individually".to_string(),
                    );
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
                    self.set_error(format!("Push failed: {}", e));
                }
            }
        }
    }

    /// Execute push by revisions (called after confirmation)
    ///
    /// Falls back to per-bookmark push if --revisions is not supported.
    /// On private/empty-description errors, retries with appropriate flags.
    pub(super) fn execute_push_revisions(&mut self, change_id: &str, bookmarks: &[String]) {
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
                self.notify_success(msg);
                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
            }
            Err(e) => {
                let err_msg = format!("{}", e);
                if is_revisions_unsupported_error(&err_msg) {
                    // Restore remote for fallback
                    self.push_target_remote = remote;
                    self.notify_info(
                        "--revisions not supported, pushing bookmarks individually".to_string(),
                    );
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
                                self.notify_success(msg);
                                self.mark_dirty_and_refresh_current(DirtyFlags::log_and_status());
                            }
                            Err(e2) => {
                                self.set_error(format!("Push failed: {}", e2));
                            }
                        }
                    } else {
                        self.set_error(format!("Push failed: {}", e));
                    }
                }
            }
        }
    }
}

// ── Free functions ───────────────────────────────────────────────────────

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::components::{Dialog, DialogCallback, DialogResult};

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
}
