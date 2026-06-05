//! Agent Trace Phase 1 integration tests.
//!
//! End-to-end: a real jj repo + a sidecar `.agent-trace/traces.jsonl` →
//! Log View AI badges ([AI] confirmed / [AI?] heuristic).

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::app::App;
use tij::jj::JjExecutor;

/// Full (untruncated) change ID of @
fn full_change_id(repo: &TestRepo) -> String {
    repo.jj(&["log", "-r", "@", "--no-graph", "-T", "change_id"])
        .trim()
        .to_string()
}

/// Standard repo setup: a source file plus a .gitignore covering trace paths.
///
/// The trace sidecar must be gitignored — otherwise writing it dirties the
/// working copy, jj snapshots a NEW commit_id for @, and any git-SHA-anchored
/// record taken beforehand stops matching (the exact false-negative mode
/// documented in the SoW §6.3; change IDs are immune, which is why the spec
/// recommends them for jj).
fn setup_repo() -> TestRepo {
    let repo = TestRepo::new();
    repo.write_file(".gitignore", ".agent-trace/\ncustom/\n");
    repo.write_file("src.rs", "fn main() {}");
    repo.jj(&["describe", "-m", "ai change"]);
    repo
}

/// Write a trace record JSONL into the repo (reference-implementation shape)
fn write_trace(repo: &TestRepo, rel_path: &str, vcs_type: &str, revision: &str) {
    let dir = repo.path().join(
        std::path::Path::new(rel_path)
            .parent()
            .unwrap_or(std::path::Path::new("")),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let record = format!(
        concat!(
            r#"{{"version":"1.0","timestamp":"2026-06-05T00:00:00Z","#,
            r#""vcs":{{"type":"{}","revision":"{}"}},"#,
            r#""tool":{{"name":"claude-code"}},"#,
            r#""files":[{{"path":"src/main.rs","conversations":[{{"#,
            r#""contributor":{{"type":"ai","model_id":"anthropic/claude-opus-4-8"}},"#,
            r#""ranges":[{{"start_line":1,"end_line":5}}]}}]}}]}}"#,
            "\n"
        ),
        vcs_type, revision
    );
    std::fs::write(repo.path().join(rel_path), record).unwrap();
}

fn app_for(repo: &TestRepo) -> App {
    let mut app = App::new_for_test();
    app.jj = JjExecutor::with_repo_path(repo.path());
    app.refresh_log(None);
    app.reload_traces();
    app
}

#[test]
fn jj_trace_record_shows_confirmed_badge() {
    skip_if_no_jj!();
    let repo = setup_repo();

    write_trace(
        &repo,
        ".agent-trace/traces.jsonl",
        "jj",
        &full_change_id(&repo),
    );

    let app = app_for(&repo);
    assert!(
        !app.log_view.ai_badges.confirmed.is_empty(),
        "jj-typed record should produce a confirmed badge"
    );
    assert!(app.log_view.ai_badges.heuristic.is_empty());

    // The badged commit is the @ row
    let wc = app
        .log_view
        .changes
        .iter()
        .find(|c| c.is_working_copy)
        .unwrap();
    assert!(
        app.log_view
            .ai_badges
            .confirmed
            .contains(wc.commit_id.as_str())
    );
}

#[test]
fn git_trace_record_shows_heuristic_badge() {
    skip_if_no_jj!();
    let repo = setup_repo();

    // Full commit SHA of @ (what a git-only writer would record at best)
    let sha = repo
        .jj(&["log", "-r", "@", "--no-graph", "-T", "commit_id"])
        .trim()
        .to_string();
    write_trace(&repo, ".agent-trace/traces.jsonl", "git", &sha);

    let app = app_for(&repo);
    assert!(
        !app.log_view.ai_badges.heuristic.is_empty(),
        "git-typed record should produce a heuristic badge"
    );
    assert!(app.log_view.ai_badges.confirmed.is_empty());
}

#[test]
fn no_trace_file_means_no_badges() {
    skip_if_no_jj!();
    let repo = setup_repo();

    let app = app_for(&repo);
    assert!(app.log_view.ai_badges.is_empty());
}

#[test]
fn config_path_override_is_respected() {
    skip_if_no_jj!();
    let repo = setup_repo();

    // Trace lives at a custom relative path, configured via jj config
    write_trace(
        &repo,
        "custom/my-traces.jsonl",
        "jj",
        &full_change_id(&repo),
    );
    repo.jj(&[
        "config",
        "set",
        "--repo",
        "tij.agent-trace.path",
        "custom/my-traces.jsonl",
    ]);

    let app = app_for(&repo);
    assert!(
        !app.log_view.ai_badges.confirmed.is_empty(),
        "trace at configured path should be loaded"
    );
}

#[test]
fn broken_lines_do_not_break_loading() {
    skip_if_no_jj!();
    let repo = setup_repo();

    let change_id = full_change_id(&repo);
    let dir = repo.path().join(".agent-trace");
    std::fs::create_dir_all(&dir).unwrap();
    let valid = format!(
        r#"{{"timestamp":"t","vcs":{{"type":"jj","revision":"{}"}},"tool":{{"name":"x"}},"files":[{{"path":"a.rs","conversations":[{{"contributor":{{"type":"ai"}},"ranges":[]}}]}}]}}"#,
        change_id
    );
    std::fs::write(
        dir.join("traces.jsonl"),
        format!("garbage\n{}\n{{broken json", valid),
    )
    .unwrap();

    let app = app_for(&repo);
    assert!(!app.log_view.ai_badges.confirmed.is_empty());
}
