//! Bookmark integration tests.
//!
//! Tests for bookmark create, set, delete, and list operations.

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;

#[test]
fn test_bookmark_create() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .bookmark_create("feature", &change_id)
        .expect("bookmark_create should succeed");

    assert!(
        repo.bookmark_exists("feature"),
        "bookmark 'feature' should exist"
    );
}

#[test]
fn test_bookmark_create_duplicate_fails() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .bookmark_create("main", &change_id)
        .expect("first bookmark_create should succeed");

    // Second creation should fail
    let result = executor.bookmark_create("main", &change_id);
    assert!(result.is_err(), "duplicate bookmark creation should fail");
}

#[test]
fn test_bookmark_set_moves_existing() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.jj(&["new", "-m", "first"]);
    let first_id = repo.current_change_id();
    repo.jj(&["bookmark", "create", "mybranch", "-r", &first_id]);

    repo.jj(&["new", "-m", "second"]);
    let second_id = repo.current_change_id();

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .bookmark_set("mybranch", &second_id)
        .expect("bookmark_set should succeed");

    // bookmark now points to second
    let bookmark_rev = repo
        .jj(&[
            "log",
            "-r",
            "mybranch",
            "--no-graph",
            "-T",
            "change_id.short(8)",
        ])
        .trim()
        .to_string();
    assert_eq!(
        bookmark_rev, second_id,
        "bookmark should point to second revision"
    );
}

#[test]
fn test_bookmark_delete() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();
    repo.jj(&["bookmark", "create", "to-delete", "-r", &change_id]);

    assert!(
        repo.bookmark_exists("to-delete"),
        "bookmark should exist before delete"
    );

    let executor = JjExecutor::with_repo_path(repo.path());
    executor
        .bookmark_delete(&["to-delete"])
        .expect("bookmark_delete should succeed");

    assert!(
        !repo.bookmark_exists("to-delete"),
        "bookmark should not exist after delete"
    );
}

#[test]
fn test_bookmark_list_all() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();
    repo.jj(&["bookmark", "create", "main", "-r", &change_id]);
    repo.jj(&["bookmark", "create", "develop", "-r", &change_id]);

    let executor = JjExecutor::with_repo_path(repo.path());
    let bookmarks = executor
        .bookmark_list_all()
        .expect("bookmark_list_all should succeed");

    let names: Vec<_> = bookmarks.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"main"), "should contain 'main'");
    assert!(names.contains(&"develop"), "should contain 'develop'");
}

#[test]
fn test_bookmark_set_creates_if_not_exists() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let change_id = repo.current_change_id();

    let executor = JjExecutor::with_repo_path(repo.path());
    // bookmark_set with --allow-backwards should work on new bookmarks too
    executor
        .bookmark_set("new-branch", &change_id)
        .expect("bookmark_set on new bookmark should succeed");

    assert!(
        repo.bookmark_exists("new-branch"),
        "new bookmark should be created"
    );
}

#[test]
fn test_bookmarks_to_advance_finds_ancestor() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.jj(&["describe", "-m", "base"]);
    let base = repo.current_change_id();

    repo.jj(&["bookmark", "create", "main", "-r", &base]);
    repo.jj(&["new", "-m", "ahead"]);

    let executor = JjExecutor::with_repo_path(repo.path());
    let candidates = executor
        .bookmarks_to_advance()
        .expect("bookmarks_to_advance should succeed");
    assert_eq!(
        candidates,
        vec!["main".to_string()],
        "bookmarks_to_advance should return 'main' as an ancestor bookmark"
    );

    // Perform the advance as execute_bookmark_advance would
    repo.jj(&["bookmark", "advance", "exact:main", "--to", "@"]);

    // Verify main now points to @
    let at = repo.current_change_id();
    let main_rev = repo
        .jj(&[
            "log",
            "-r",
            "main",
            "--no-graph",
            "-T",
            "change_id.short(8)",
        ])
        .trim()
        .to_string();
    assert_eq!(main_rev, at, "main should point to @ after advance");
}

#[test]
fn test_bookmarks_to_advance_empty_when_at_wc() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let id = repo.current_change_id();

    // Create bookmark AT @, not as ancestor
    repo.jj(&["bookmark", "create", "main", "-r", &id]);

    let executor = JjExecutor::with_repo_path(repo.path());
    let candidates = executor
        .bookmarks_to_advance()
        .expect("bookmarks_to_advance should succeed");
    assert!(
        candidates.is_empty(),
        "bookmarks_to_advance should be empty when the only bookmark is already at @, got: {:?}",
        candidates
    );
}

#[test]
fn test_advance_exact_does_not_blast_sibling() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.jj(&["describe", "-m", "base"]);
    let base = repo.current_change_id();

    // Create a bookmark whose name contains a glob metacharacter ('*'), plus a
    // sibling that a BARE glob would also match. Verified empirically against
    // jj 0.41: `jj bookmark advance foo*` (bare) advances BOTH `foo*` and `foo1`
    // ("Advanced 2 bookmarks"); only `exact:foo*` confines it to `foo*`.
    // (jj allows '*' in local bookmark names; git export warns but the local
    // bookmark exists and jj exits 0, so repo.jj does not panic.)
    repo.jj(&["bookmark", "create", "foo*", "-r", &base]);
    repo.jj(&["bookmark", "create", "foo1", "-r", &base]);
    repo.jj(&["new", "-m", "ahead"]);
    let at = repo.current_change_id();

    // Advance ONLY `foo*` using the exact: prefix.
    repo.jj(&["bookmark", "advance", "exact:foo*", "--to", "@"]);

    // `foo*` advanced to @ — reference it via an exact bookmark revset, since
    // `foo*` is not a literal name in revset syntax.
    let foo_star_rev = repo
        .jj(&[
            "log",
            "-r",
            "bookmarks(exact:\"foo*\")",
            "--no-graph",
            "-T",
            "change_id.short(8)",
        ])
        .trim()
        .to_string();
    // `foo1` (the sibling a bare glob would catch) must stay at base.
    let foo1_rev = repo
        .jj(&[
            "log",
            "-r",
            "foo1",
            "--no-graph",
            "-T",
            "change_id.short(8)",
        ])
        .trim()
        .to_string();

    assert_eq!(foo_star_rev, at, "foo* should have advanced to @");
    assert_eq!(
        foo1_rev, base,
        "foo1 must NOT move — exact:foo* prevents glob blast onto sibling names"
    );
}
