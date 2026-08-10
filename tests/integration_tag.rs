//! Tag integration tests (jj 0.44 tag tracking).
//!
//! Covers the remote-tag half of `Executor::tag_list()`: the `--all-remotes`
//! rows, the `tracked` flag, `jj tag track` / `untrack`, and the
//! `jj git push --tag exact:<name>` argv that `App::execute_tag_push` builds.
//!
//! These tests shell out to a real `jj` (and to `git` for inspecting the bare
//! remote). They live in `tests/` on purpose — `release.yml` runs
//! `cargo test --locked --lib` on a machine without jj, so nothing here may
//! move into the library test suite.

#[path = "common/mod.rs"]
mod common;

use std::process::Command;

use common::{RemoteRepo, TestRepo};
use tij::jj::JjExecutor;
use tij::model::TagInfo;

// ── Helpers (file-local; the shared helpers live in tests/common) ──────────

/// Tag names that actually exist in the bare remote repository, sorted.
///
/// Reads the remote directly rather than trusting the pushing repo's view of
/// it, so a push that overshoots is visible.
fn remote_tag_names(remote: &RemoteRepo) -> Vec<String> {
    let output = Command::new("git")
        .args(["tag", "--list"])
        .current_dir(remote.url())
        .output()
        .expect("failed to run `git tag --list` in the bare remote");
    assert!(
        output.status.success(),
        "`git tag --list` failed in the bare remote: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut names: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    names.sort();
    names
}

/// A repo wired to `remote` with one described commit, ready to be tagged.
///
/// The description matters: `jj git push` refuses to push a commit that has
/// none, and every test here pushes a tag.
fn repo_with_commit(remote: &RemoteRepo) -> TestRepo {
    let repo = TestRepo::with_remote(remote);
    repo.write_file("README.md", "# tag test");
    repo.jj(&["describe", "-m", "tagged commit"]);
    repo
}

/// Find the `tag_list()` row for `name` at `remote` (`None` = local row).
fn find_row<'a>(tags: &'a [TagInfo], name: &str, remote: Option<&str>) -> Option<&'a TagInfo> {
    tags.iter()
        .find(|t| t.name == name && t.remote.as_deref() == remote)
}

/// Same as [`find_row`], but panics with the full row dump when missing.
fn row<'a>(tags: &'a [TagInfo], name: &str, remote: Option<&str>) -> &'a TagInfo {
    find_row(tags, name, remote)
        .unwrap_or_else(|| panic!("no tag row for {name} @ {remote:?}; rows were {tags:#?}"))
}

// ── tag_list: local + remote rows ─────────────────────────────────────────

/// `jj git fetch` materialises a remote tag, and `tag_list()` surfaces both
/// the local row and the `@origin` row (this is what `--all-remotes` buys).
///
/// Note: `tag_list()` also returns rows with `remote == Some("git")` — jj's
/// internal git-backend remote — even in a non-colocated repo. Filtering those
/// out is Tag View's job, not the parser's, so this test does not assert on
/// them.
#[test]
fn test_tag_list_returns_local_and_remote_rows_after_fetch() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();

    // Publisher: tag a commit and push the tag.
    let publisher = repo_with_commit(&remote);
    publisher.jj(&["tag", "set", "v1.0", "-r", "@"]);
    publisher.jj(&["git", "push", "--tag", "exact:v1.0"]);

    // Consumer: fetch picks up the tag as `v1.0@origin`.
    let consumer = TestRepo::with_remote(&remote);
    consumer.jj(&["git", "fetch"]);

    let tags = JjExecutor::with_repo_path(consumer.path())
        .tag_list()
        .expect("tag_list should succeed");

    let local = row(&tags, "v1.0", None);
    assert!(local.present, "the local row should be present");

    let remote_row = row(&tags, "v1.0", Some("origin"));
    assert!(remote_row.present, "the v1.0@origin row should be present");
    assert_eq!(
        remote_row.full_name(),
        "v1.0@origin",
        "remote rows render as name@remote"
    );
}

/// `tracked` is populated per row: local rows are always `false`, and a fetched
/// remote tag comes back tracked (jj auto-tracks on fetch).
#[test]
fn test_tag_list_reports_tracked_flag_for_local_and_remote_rows() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();
    let repo = repo_with_commit(&remote);
    repo.jj(&["tag", "set", "v1.0", "-r", "@"]);
    repo.jj(&["git", "push", "--tag", "exact:v1.0"]);

    let tags = JjExecutor::with_repo_path(repo.path())
        .tag_list()
        .expect("tag_list should succeed");

    let local = row(&tags, "v1.0", None);
    assert!(
        !local.tracked,
        "local rows never carry tracked=true, got {local:?}"
    );
    assert!(!local.is_tracked_remote() && !local.is_untracked_remote());
    assert!(
        !local.conflict,
        "an unconflicted tag must report conflict=false"
    );
    assert!(
        local.change_id.is_some() && local.commit_id.is_some(),
        "a present local tag resolves its target, got {local:?}"
    );
    assert_eq!(
        local.description.as_deref(),
        Some("tagged commit"),
        "the local row carries the target's first description line"
    );

    let remote_row = row(&tags, "v1.0", Some("origin"));
    assert!(
        remote_row.tracked,
        "a pushed tag is tracked on the remote, got {remote_row:?}"
    );
    assert!(remote_row.is_tracked_remote());
}

// ── track / untrack round trip ────────────────────────────────────────────

/// `jj tag untrack <name>@<remote>` flips the remote row to untracked — the
/// only state in which Tag View offers `t` (Track).
#[test]
fn test_tag_untrack_turns_remote_row_into_untracked() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();
    let repo = repo_with_commit(&remote);
    repo.jj(&["tag", "set", "v1.0", "-r", "@"]);
    repo.jj(&["git", "push", "--tag", "exact:v1.0"]);

    // `TAG@REMOTE` resolves exactly, so no `exact:` prefix — this is the
    // argv shape `App::execute_tag_track/untrack` passes verbatim.
    repo.jj(&["tag", "untrack", "v1.0@origin"]);

    let tags = JjExecutor::with_repo_path(repo.path())
        .tag_list()
        .expect("tag_list should succeed");

    let remote_row = row(&tags, "v1.0", Some("origin"));
    assert!(
        remote_row.is_untracked_remote(),
        "v1.0@origin should be untracked after `jj tag untrack`, got {remote_row:?}"
    );

    // The local tag is untouched by untracking.
    let local = row(&tags, "v1.0", None);
    assert!(local.present, "untrack must not delete the local tag");
}

/// `jj tag track <name>@<remote>` puts an untracked remote row back into the
/// tracked group.
#[test]
fn test_tag_track_restores_tracked_remote_row() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();
    let repo = repo_with_commit(&remote);
    repo.jj(&["tag", "set", "v1.0", "-r", "@"]);
    repo.jj(&["git", "push", "--tag", "exact:v1.0"]);
    repo.jj(&["tag", "untrack", "v1.0@origin"]);

    let executor = JjExecutor::with_repo_path(repo.path());
    let before = executor.tag_list().expect("tag_list should succeed");
    assert!(
        row(&before, "v1.0", Some("origin")).is_untracked_remote(),
        "precondition: v1.0@origin is untracked"
    );

    repo.jj(&["tag", "track", "v1.0@origin"]);

    let after = executor.tag_list().expect("tag_list should succeed");
    let remote_row = row(&after, "v1.0", Some("origin"));
    assert!(
        remote_row.is_tracked_remote(),
        "v1.0@origin should be tracked again after `jj tag track`, got {remote_row:?}"
    );
}

// ── push ──────────────────────────────────────────────────────────────────

/// The exact argv `App::execute_tag_push` builds adds exactly one tag to the
/// remote, leaving the sibling tag local-only.
#[test]
fn test_tag_push_exact_adds_exactly_one_tag_to_the_remote() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();
    let repo = repo_with_commit(&remote);
    repo.jj(&["tag", "set", "v1.0", "-r", "@"]);
    repo.jj(&["tag", "set", "v1.1", "-r", "@"]);

    assert!(
        remote_tag_names(&remote).is_empty(),
        "precondition: the bare remote starts with no tags"
    );

    // Mirrors `execute_tag_push` with `push_target_remote == None`.
    repo.jj(&["git", "push", "--tag", "exact:v1.0"]);

    assert_eq!(
        remote_tag_names(&remote),
        vec!["v1.0".to_string()],
        "pushing `exact:v1.0` must add exactly one tag to the remote"
    );
}

/// Glob regression: `--tag <TAG>` is glob-interpreted by jj, so a bare pattern
/// pushes every tag it matches. `exact:` is the only thing confining a tag push
/// to the selected tag.
///
/// The two halves are run as an A/B from identical starting states (same tag
/// names, same jj, one bare remote each):
///
/// * A — `exact:v1.0` (tij's argv): only `v1.0` lands.
/// * B — bare `v1.*`: **both** `v1.0` and `v1.1` land. This is the failure mode,
///   reproduced live rather than quoted from the docs; if a future jj stopped
///   glob-expanding bare patterns, this half fails and the `exact:` requirement
///   can be revisited.
///
/// Two repos rather than one repo with two remotes: once a tag tracks any
/// remote, jj refuses to create it on a second one ("Refusing to create new
/// remote tag ..."), so a single repo cannot run both halves.
///
/// A tag name cannot itself contain a glob metacharacter (git refnames ban `*`,
/// `?` and `[`), so dropping `exact:` from `exact:v1.0` cannot be made to blast
/// `v1.1` directly. The A/B is the closest live proof: it pins that glob
/// expansion is active on this jj and that `exact:` is what disables it.
#[test]
fn test_tag_push_exact_does_not_blast_sibling_tag() {
    skip_if_no_jj!();

    // A: tij's argv — `exact:` confines the push to the selected tag.
    let exact_remote = RemoteRepo::new_bare();
    let exact_repo = repo_with_commit(&exact_remote);
    exact_repo.jj(&["tag", "set", "v1.0", "-r", "@"]);
    exact_repo.jj(&["tag", "set", "v1.1", "-r", "@"]);
    exact_repo.jj(&["git", "push", "--tag", "exact:v1.0"]);

    assert_eq!(
        remote_tag_names(&exact_remote),
        vec!["v1.0".to_string()],
        "`exact:v1.0` must push only v1.0; v1.1 must not reach the remote"
    );

    // ...and jj agrees: no v1.1@origin tracking row was created.
    let tags = JjExecutor::with_repo_path(exact_repo.path())
        .tag_list()
        .expect("tag_list should succeed");
    assert!(
        find_row(&tags, "v1.1", Some("origin")).is_none(),
        "v1.1 must have no origin row after an exact: push of v1.0, rows were {tags:#?}"
    );

    // B: the same setup with a bare pattern takes the sibling along.
    let glob_remote = RemoteRepo::new_bare();
    let glob_repo = repo_with_commit(&glob_remote);
    glob_repo.jj(&["tag", "set", "v1.0", "-r", "@"]);
    glob_repo.jj(&["tag", "set", "v1.1", "-r", "@"]);
    glob_repo.jj(&["git", "push", "--tag", "v1.*"]);

    assert_eq!(
        remote_tag_names(&glob_remote),
        vec!["v1.0".to_string(), "v1.1".to_string()],
        "a bare `--tag` pattern is glob-expanded by jj — this is why tij sends `exact:`"
    );
}

/// Pushing a tag auto-creates a **tracked** `<name>@<remote>` row that did not
/// exist before. This is why `execute_tag_push` must call `refresh_tag_view()`:
/// without a re-list, the new Remote (tracked) row never appears.
#[test]
fn test_tag_push_auto_creates_tracked_remote_row() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();
    let repo = repo_with_commit(&remote);
    repo.jj(&["tag", "set", "v1.0", "-r", "@"]);

    let executor = JjExecutor::with_repo_path(repo.path());
    let before = executor.tag_list().expect("tag_list should succeed");
    assert!(
        find_row(&before, "v1.0", Some("origin")).is_none(),
        "precondition: no origin row exists before the push, rows were {before:#?}"
    );

    repo.jj(&["git", "push", "--tag", "exact:v1.0"]);

    let after = executor.tag_list().expect("tag_list should succeed");
    let remote_row = row(&after, "v1.0", Some("origin"));
    assert!(
        remote_row.tracked,
        "push must auto-track the new remote tag, got {remote_row:?}"
    );
}

// ── absent rows ───────────────────────────────────────────────────────────

/// A row with `present == false` must survive parsing.
///
/// Recipe (the one that actually reproduces it): push the tag, then delete the
/// **local** tag while the remote tag stays. Deleting on the remote and
/// fetching does not work — tracking removes the local tag too.
///
/// jj emits no target for such a row and does not fail: without `try(..., "")`
/// in the template it embeds the literal string `<Error: No Commit available>`,
/// which would be mis-parsed as a change_id. Asserting all three target fields
/// are `None` pins both the fixed 8-column parse and the `try()` wrapper.
#[test]
fn test_tag_list_keeps_absent_local_row_after_local_tag_delete() {
    skip_if_no_jj!();
    let remote = RemoteRepo::new_bare();
    let repo = repo_with_commit(&remote);
    repo.jj(&["tag", "set", "v1.0", "-r", "@"]);
    repo.jj(&["git", "push", "--tag", "exact:v1.0"]);

    repo.jj(&["tag", "delete", "v1.0"]);

    let tags = JjExecutor::with_repo_path(repo.path())
        .tag_list()
        .expect("tag_list should succeed");

    let local = row(&tags, "v1.0", None);
    assert!(
        !local.present,
        "the local row must be returned with present=false, got {local:?}"
    );
    assert!(!local.tracked && !local.conflict, "got {local:?}");
    assert_eq!(local.change_id, None, "absent rows resolve no change_id");
    assert_eq!(local.commit_id, None, "absent rows resolve no commit_id");
    assert_eq!(
        local.description, None,
        "absent rows resolve no description"
    );
    assert!(
        !local.is_jumpable(),
        "an absent row has no target to jump to"
    );

    // The remote tag itself is untouched by a local delete.
    let remote_row = row(&tags, "v1.0", Some("origin"));
    assert!(remote_row.present, "the remote tag still exists");
    assert_eq!(
        remote_tag_names(&remote),
        vec!["v1.0".to_string()],
        "deleting the local tag must not touch the remote"
    );
}
