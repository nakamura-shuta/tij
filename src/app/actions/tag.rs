//! Tag operations (create, delete, list, track/untrack, push)

use crate::app::state::{App, DirtyFlags, View};
use crate::jj::constants::{commands, flags};
use crate::ui::components::{Dialog, DialogCallback, SelectItem};

impl App {
    /// Open the tag view
    pub(crate) fn open_tag_view(&mut self) {
        // Always navigate (even on jj failure) so errors are visible from inside
        // the tag view rather than trapping the user in Log.
        self.refresh_tag_view();
        self.go_to_view(View::Tag);
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
            DialogCallback::TagCreate { revision } => {
                if let Some(name) = values.first()
                    && !name.is_empty()
                {
                    self.execute_tag_create(name, &revision);
                }
            }
            DialogCallback::TagDelete { name } => {
                self.execute_tag_delete(&name);
            }
            DialogCallback::TagPush { name } => {
                self.execute_tag_push(&name);
            }
            _ => {}
        }
    }

    /// Execute tag creation on the given revision
    fn execute_tag_create(&mut self, name: &str, revision: &str) {
        match self.run_and_record("Tag create", &["tag", "set", name, "-r", revision]) {
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

    /// Start tracking a remote tag (`jj tag track <TAG@REMOTE>`)
    ///
    /// `full_name` is passed verbatim: `TAG@REMOTE` resolves to a remote tag
    /// exactly, so no `exact:` prefix is needed (unlike `git push --tag`).
    /// Undoable with `u`, hence no confirmation dialog (mirrors Bookmark View).
    pub(crate) fn execute_tag_track(&mut self, full_name: &str) {
        let display = display_name(full_name);
        match self.run_and_record(
            "Tag track",
            &[commands::TAG, commands::TAG_TRACK, full_name],
        ) {
            Ok(_) => {
                self.notify_success(format!("Started tracking: {display}"));
                self.refresh_tag_view();
                self.mark_dirty_and_refresh_current(DirtyFlags::log());
            }
            Err(e) => {
                self.set_error(format!("Failed to track: {e}"));
            }
        }
    }

    /// Stop tracking a remote tag (`jj tag untrack <TAG@REMOTE>`)
    pub(crate) fn execute_tag_untrack(&mut self, full_name: &str) {
        let display = display_name(full_name);
        match self.run_and_record(
            "Tag untrack",
            &[commands::TAG, commands::TAG_UNTRACK, full_name],
        ) {
            Ok(_) => {
                self.notify_success(format!("Stopped tracking: {display}"));
                self.refresh_tag_view();
                self.mark_dirty_and_refresh_current(DirtyFlags::log());
            }
            Err(e) => {
                self.set_error(format!("Failed to untrack: {e}"));
            }
        }
    }

    /// Start the tag push flow (remote selection if needed, then confirmation)
    ///
    /// Reuses the bookmark push machinery: on a multi-remote repo the shared
    /// `GitPushRemoteSelect` dialog runs first and this method is re-entered
    /// with `push_target_remote` populated (`pending_push_tag` tells the
    /// dialog handler to come back here rather than to `start_push`).
    pub(crate) fn start_tag_push(&mut self, name: String) {
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
                    // Re-entry marker for the shared remote-select handler.
                    self.pending_push_tag = Some(name);
                    return;
                }
                _ => {
                    // Single remote or lookup failure: let jj apply its default.
                }
            }
        }

        let message = tag_push_confirm_message(&name, self.push_target_remote.as_deref());
        self.active_dialog = Some(Dialog::confirm(
            "Push Tag",
            message,
            Some("Remote changes cannot be undone with 'u'.".to_string()),
            DialogCallback::TagPush { name },
        ));
    }

    /// Execute `jj git push [--remote <r>] --tag exact:<name>`
    ///
    /// Uses `push_target_remote` if set (consumed via `take()` at the top to
    /// guarantee cleanup on all exit paths). Mirrors `execute_push`.
    pub(crate) fn execute_tag_push(&mut self, name: &str) {
        // Take at the top → nothing leaks into the next push on any path.
        let remote = self.push_target_remote.take();
        self.pending_push_tag = None;

        // `--tag <TAG>` is glob-interpreted by jj; `exact:` pins it to one tag.
        let pattern = format!("exact:{name}");
        let mut args: Vec<&str> = vec![commands::GIT, commands::GIT_PUSH];
        if let Some(ref r) = remote {
            args.push(flags::REMOTE);
            args.push(r);
        }
        args.push(flags::TAG_FLAG);
        args.push(&pattern);

        match self.run_and_record("Tag push", &args) {
            Ok(_) => {
                self.notify_success(format!("Tag '{name}' pushed"));
                // Required: push auto-tracks a previously untracked tag, so the
                // Remote (tracked) group only gains the row after a re-list.
                self.refresh_tag_view();
                self.mark_dirty_and_refresh_current(DirtyFlags::log());
            }
            Err(e) => {
                self.set_error(format!("Failed to push tag: {e}"));
            }
        }
    }
}

/// Strip the `@remote` suffix for user-facing notifications
fn display_name(full_name: &str) -> &str {
    full_name.split('@').next().unwrap_or(full_name)
}

/// Confirmation wording for a tag push
///
/// Only names a remote when tij itself made the user pick one. jj resolves the
/// default from the `git.push` setting first (which tij does not read), so even
/// a single-remote repo may not push where that one remote points.
fn tag_push_confirm_message(name: &str, remote: Option<&str>) -> String {
    match remote {
        Some(r) => format!("Push tag '{name}' to {r}?"),
        None => format!("Push tag '{name}' to the default remote?"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jj::JjExecutor;
    use crate::ui::components::{DialogKind, DialogResult};
    use std::path::PathBuf;

    /// Repository path that cannot exist. Pointing the executor at it makes
    /// every jj invocation fail before it can touch a repo or a network, so
    /// `execute_tag_push` tests never run a real `git push` on the developer's
    /// machine while still recording the exact argv.
    const NO_REPO: &str = "/nonexistent-tij-tag-push-test-repo";

    /// App whose jj invocations are inert (see [`NO_REPO`]).
    fn app_without_repo() -> App {
        let mut app = App::new_for_test();
        app.jj = JjExecutor::with_repo_path(PathBuf::from(NO_REPO));
        app
    }

    /// Expected argv prefix for [`app_without_repo`] (`-R <path>` then `--color=never`).
    fn argv(rest: &[&str]) -> Vec<String> {
        let mut v = vec![
            "-R".to_string(),
            NO_REPO.to_string(),
            "--color=never".into(),
        ];
        v.extend(rest.iter().map(|s| s.to_string()));
        v
    }

    /// argv of the last recorded command (jj prepends `--color=never`).
    fn last_args(app: &App) -> Vec<String> {
        app.command_history
            .records()
            .back()
            .expect("command_history should have a record")
            .args
            .clone()
    }

    fn confirm_message(app: &App) -> String {
        match &app.active_dialog.as_ref().expect("dialog expected").kind {
            DialogKind::Confirm { message, .. } => message.clone(),
            other => panic!("expected a Confirm dialog, got: {other:?}"),
        }
    }

    // --- track / untrack argv ---

    #[test]
    fn execute_tag_track_passes_the_full_name_verbatim() {
        let mut app = app_without_repo();
        app.execute_tag_track("v1.0@origin");
        assert_eq!(
            last_args(&app),
            argv(&["tag", "track", "v1.0@origin"]),
            "TAG@REMOTE resolves exactly; no exact: prefix, no --remote"
        );
    }

    #[test]
    fn execute_tag_untrack_passes_the_full_name_verbatim() {
        let mut app = app_without_repo();
        app.execute_tag_untrack("v1.0@origin");
        assert_eq!(last_args(&app), argv(&["tag", "untrack", "v1.0@origin"]));
    }

    #[test]
    fn display_name_strips_the_remote_suffix() {
        assert_eq!(display_name("v1.0@origin"), "v1.0");
        assert_eq!(display_name("v1.0"), "v1.0");
    }

    // --- push argv ---

    #[test]
    fn execute_tag_push_uses_an_exact_pattern() {
        // `--tag <TAG>` is glob-interpreted by jj: a bare `v1.*` pushed two tags
        // in practice. A bare name here would silently push siblings.
        let mut app = app_without_repo();
        app.execute_tag_push("v1.0");
        assert_eq!(
            last_args(&app),
            argv(&["git", "push", "--tag", "exact:v1.0"])
        );
    }

    #[test]
    fn execute_tag_push_adds_remote_only_when_one_was_selected() {
        let mut app = app_without_repo();
        app.push_target_remote = Some("upstream".to_string());
        app.execute_tag_push("v1.0");
        assert_eq!(
            last_args(&app),
            argv(&["git", "push", "--remote", "upstream", "--tag", "exact:v1.0"])
        );

        // Without a selection the flag is absent → jj applies its own default.
        let mut app = app_without_repo();
        app.push_target_remote = None;
        app.execute_tag_push("v1.0");
        assert_eq!(
            last_args(&app),
            argv(&["git", "push", "--tag", "exact:v1.0"]),
            "no remote selected → no --remote flag"
        );
    }

    // --- confirmation wording ---

    #[test]
    fn tag_push_confirm_message_names_only_a_chosen_remote() {
        assert_eq!(
            tag_push_confirm_message("v1.0", Some("upstream")),
            "Push tag 'v1.0' to upstream?"
        );
        // tij does not resolve `git.push`, so it must not claim "origin".
        let default = tag_push_confirm_message("v1.0", None);
        assert_eq!(default, "Push tag 'v1.0' to the default remote?");
        assert!(!default.contains("origin"), "must not guess a remote name");
    }

    #[test]
    fn start_tag_push_with_selected_remote_names_it_in_the_confirmation() {
        // push_target_remote set → the multi-remote lookup is skipped entirely,
        // so this never shells out to jj.
        let mut app = App::new_for_test();
        app.push_target_remote = Some("upstream".to_string());
        app.start_tag_push("v1.0".to_string());

        assert_eq!(confirm_message(&app), "Push tag 'v1.0' to upstream?");
        assert_eq!(
            app.active_dialog.as_ref().unwrap().callback_id,
            DialogCallback::TagPush {
                name: "v1.0".to_string()
            }
        );
    }

    #[test]
    fn start_tag_push_without_selected_remote_does_not_name_one() {
        // The remote lookup fails here (inert executor), which is the same
        // fall-through the single-remote case takes: no dialog, jj's default.
        let mut app = app_without_repo();
        app.push_target_remote = None;
        app.start_tag_push("v1.0".to_string());

        assert_eq!(
            app.active_dialog.as_ref().unwrap().callback_id,
            DialogCallback::TagPush {
                name: "v1.0".to_string()
            }
        );
        assert_eq!(
            confirm_message(&app),
            "Push tag 'v1.0' to the default remote?"
        );
    }

    // --- dialog routing ---

    #[test]
    fn tag_push_dialog_confirmed_reaches_execute_tag_push() {
        let mut app = app_without_repo();
        app.active_dialog = Some(Dialog::confirm(
            "Push Tag",
            "Push tag 'v1.0'?",
            None,
            DialogCallback::TagPush {
                name: "v1.0".to_string(),
            },
        ));
        app.handle_dialog_result(DialogResult::Confirmed(vec![]));
        assert_eq!(
            last_args(&app),
            argv(&["git", "push", "--tag", "exact:v1.0"]),
            "confirmed TagPush must dispatch to execute_tag_push"
        );
    }

    #[test]
    fn tag_push_callback_round_trips() {
        let cb = DialogCallback::TagPush {
            name: "v1.0".to_string(),
        };
        assert_eq!(cb.clone(), cb);
        assert_ne!(
            cb,
            DialogCallback::TagDelete {
                name: "v1.0".to_string()
            },
            "push must not be confusable with delete"
        );
    }
}
