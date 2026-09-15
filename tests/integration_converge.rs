//! Converge integration tests (jj 0.45 `jj converge` + divergent display).
//!
//! Pins the jj-side facts the converge feature relies on: the log template's
//! divergent offset field as parsed from real graph-mode output, the
//! `change_id(<id>)` target revset, the immutable pre-check query, and the
//! `revsets.converge` default / error behavior that the pre-check depends on.
//!
//! These tests shell out to a real `jj`. They live in `tests/` on purpose —
//! `release.yml` runs `cargo test --locked --lib` on a machine without jj, so
//! nothing here may move into the library test suite.

#[path = "common/mod.rs"]
mod common;

use std::collections::HashSet;
use std::process::Command;

use common::TestRepo;
use tij::jj::constants::DEFAULT_CONVERGE_REVSET;
use tij::jj::{JjExecutor, converge_target_revset};
use tij::model::Change;

// ── Helpers ───────────────────────────────────────────────────────────────

/// A repo whose "same desc" change is divergent (2 visible commits).
///
/// Recipe (measured against jj 0.45.1): rewrite the change from an older
/// operation with `--at-op`; jj merges the two concurrent operations on the
/// next command and both rewrites stay visible. The two sides differ in
/// content (g.txt vs g.txt + h.txt), so they are distinct commits even when
/// the timestamps collide.
fn divergent_repo() -> TestRepo {
    let repo = TestRepo::new();
    repo.write_file("f.txt", "base");
    repo.jj(&["describe", "-m", "base"]);
    repo.jj(&["new", "-m", "same desc"]);

    repo.write_file("g.txt", "one");
    repo.jj(&["log"]); // snapshot
    let op = repo
        .jj(&[
            "op",
            "log",
            "--no-graph",
            "-T",
            "id.short() ++ \"\\n\"",
            "--limit",
            "1",
        ])
        .trim()
        .to_string();

    repo.write_file("h.txt", "two");
    repo.jj(&["log"]); // snapshot
    repo.jj(&[
        "--at-op",
        &op,
        "--ignore-working-copy",
        "metaedit",
        "--update-author-timestamp",
    ]);
    // Integrate the concurrent operations so later (read-only) queries see
    // a settled view.
    repo.jj(&["log"]);
    repo
}

/// Log rows exactly as the Log View loads them (graph mode, so trailing empty
/// fields are trimmed — M18), with graph-only lines dropped.
fn log_rows(repo: &TestRepo) -> Vec<Change> {
    JjExecutor::with_repo_path(repo.path())
        .log_changes(Some("all()"), false)
        .expect("log_changes failed")
        .into_iter()
        .filter(|c| !c.is_graph_only)
        .collect()
}

/// The commits of the "same desc" change (the divergent one).
fn divergent_rows(repo: &TestRepo) -> Vec<Change> {
    log_rows(repo)
        .into_iter()
        .filter(|c| c.description == "same desc")
        .collect()
}

// ── Template / parser against real graph-mode output ──────────────────────

/// The two commits of the divergent change carry offsets {0, 1}; every other
/// row has none. Graph mode trims trailing TABs, so this also pins that the
/// parser copes with non-divergent rows missing the trailing fields.
#[test]
fn divergent_rows_parse_with_offsets() {
    skip_if_no_jj!();
    let repo = divergent_repo();
    let rows = log_rows(&repo);

    let divergent: Vec<&Change> = rows
        .iter()
        .filter(|c| c.description == "same desc")
        .collect();
    assert_eq!(divergent.len(), 2, "rows were {rows:#?}");
    assert_eq!(divergent[0].change_id, divergent[1].change_id);

    let offsets: HashSet<Option<u32>> = divergent.iter().map(|c| c.divergent_offset).collect();
    assert_eq!(offsets, HashSet::from([Some(0), Some(1)]));

    for c in rows.iter().filter(|c| c.description != "same desc") {
        assert_eq!(c.divergent_offset, None, "non-divergent row: {c:#?}");
    }
}

// ── converge argv against real jj ─────────────────────────────────────────

/// `converge_argv(Some(change_id(<8-char id>)))` converges the change.
/// `--no-interactive` is appended HERE ONLY (production is always
/// interactive); identical descriptions let jj resolve it automatically.
#[test]
fn converge_argv_target_resolves_divergence() {
    skip_if_no_jj!();
    let repo = divergent_repo();
    let change_id = divergent_rows(&repo)[0].change_id.to_string();
    assert_eq!(change_id.len(), 8);

    let jj = JjExecutor::with_repo_path(repo.path());
    let mut argv = jj.converge_argv(Some(&converge_target_revset(&change_id)));
    argv.push("--no-interactive".to_string());

    let output = Command::new("jj")
        .args(&argv)
        .current_dir(repo.path())
        .output()
        .expect("failed to run jj converge");
    assert!(
        output.status.success(),
        "jj {argv:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(repo.count_changes("divergent()"), 0);
}

// ── Pre-check queries ─────────────────────────────────────────────────────

/// Tagging one side makes it immutable; the `change_id(x) & immutable()`
/// count (the query `start_converge` uses) sees it. A tag avoids writing any
/// config (the default `immutable_heads()` includes `tags()`).
#[test]
fn immutable_precheck_counts_divergent_commits() {
    skip_if_no_jj!();
    let repo = divergent_repo();
    let rows = divergent_rows(&repo);
    assert_eq!(
        rows.len(),
        2,
        "expected a divergent pair, rows were {rows:#?}"
    );
    // Tag the side that is not the working copy
    let side = rows.iter().find(|c| !c.is_working_copy).unwrap_or(&rows[0]);
    let change_id = side.change_id.to_string();
    repo.jj(&["tag", "set", "imm-test", "-r", side.commit_id.as_str()]);

    let revset = format!("{} & immutable()", converge_target_revset(&change_id));
    let count = JjExecutor::with_repo_path(repo.path())
        .count_revisions_capped(&revset, 1)
        .expect("immutable pre-check query failed");
    assert!(count >= 1, "expected an immutable commit, got {count}");
}

/// Detects jj changing its `revsets.converge` default (read-only).
#[test]
fn revsets_converge_default_matches_constant() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    assert_eq!(
        JjExecutor::with_repo_path(repo.path()).config_get("revsets.converge"),
        Some(DEFAULT_CONVERGE_REVSET.to_string())
    );
}

/// `config get` returns a broken revset verbatim, and the count query fails
/// on it — so `config_get()` (which maps errors to None) is enough for a bad
/// `revsets.converge` to reach `PrecheckFailed`. If jj ever starts validating
/// at `config get` time, this test notices.
///
/// Note: `config set --repo` writes under `~/.config/jj/repos/<hash>/`
/// (outside the temp dir), same as `integration_trace.rs`.
#[test]
fn invalid_converge_revset_surfaces_in_count_query() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    repo.jj(&["config", "set", "--repo", "revsets.converge", "bogus(("]);

    let jj = JjExecutor::with_repo_path(repo.path());
    assert_eq!(
        jj.config_get("revsets.converge"),
        Some("bogus((".to_string())
    );
    assert!(jj.count_revisions_capped("bogus((", 1).is_err());
}
