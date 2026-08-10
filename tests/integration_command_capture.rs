//! Command transparency P1 integration tests — real jj spawns.
//!
//! Lib tests must not depend on jj, so everything that actually executes a
//! jj process to verify the invocation capture lives here.

#[path = "common/mod.rs"]
mod common;

use common::TestRepo;
use tij::jj::JjExecutor;
use tij::model::CommandKind;

fn executor_for(repo: &TestRepo) -> JjExecutor {
    JjExecutor::with_repo_path(repo.path())
}

#[test]
fn run_captures_full_argv_kind_and_seq() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let jj = executor_for(&repo);

    // A read (log_raw goes through run_readonly_str) and a write (run()).
    jj.log_raw(None, false).expect("log");
    jj.run(&["new"]).expect("new");

    let invs = jj.take_invocations();
    assert_eq!(invs.len(), 2);

    let read = &invs[0];
    assert_eq!(read.kind, CommandKind::Read);
    assert!(read.success);
    // Full argv = the honest command line: -R <path>, --color=never, and the
    // readonly --no-integrate-operation prefix all present.
    assert_eq!(read.argv[0], "-R");
    assert!(read.argv.contains(&"--color=never".to_string()));
    assert!(read.argv.contains(&"--no-integrate-operation".to_string()));
    assert!(read.argv.contains(&"log".to_string()));
    // bare_args = what the caller passed (no -R / --color=never).
    assert!(!read.bare_args.contains(&"--color=never".to_string()));

    let write = &invs[1];
    assert_eq!(write.kind, CommandKind::Write);
    assert!(write.success);
    assert!(write.seq > read.seq, "seq strictly increases");

    // Drained — second take is empty.
    assert!(jj.take_invocations().is_empty());
}

#[test]
fn failed_run_is_captured_with_error() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let jj = executor_for(&repo);

    let result = jj.run(&["abandon", "-r", "zzzzzzzzzzzz"]);
    assert!(result.is_err());

    let invs = jj.take_invocations();
    assert_eq!(invs.len(), 1);
    assert!(!invs[0].success);
    assert!(invs[0].error.is_some(), "failure carries stderr");
}

/// Regression (real jj): the capture keeps the **whole** stderr.
///
/// `jj bookmark create <existing>` answers with two lines — `Error: …` and
/// `Hint: …`. The capture used to keep only the first, and because the error
/// banner is one row tall, the hint was then unreachable from anywhere in tij.
#[test]
fn failed_run_captures_full_multiline_stderr() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let jj = executor_for(&repo);
    repo.write_file("f.txt", "hello");
    repo.jj(&["describe", "-m", "base"]);
    repo.jj(&["bookmark", "create", "dup", "-r", "@"]);

    let result = jj.run(&["bookmark", "create", "dup", "-r", "@"]);
    assert!(result.is_err(), "creating an existing bookmark must fail");

    let invs = jj.take_invocations();
    let err = invs
        .last()
        .expect("invocation captured")
        .error
        .clone()
        .expect("failure carries stderr");

    assert!(err.contains("Error:"), "stderr head kept: {err:?}");
    assert!(
        err.contains("Hint:"),
        "the Hint line must not be truncated away: {err:?}"
    );
    assert!(
        err.lines().count() >= 2,
        "multi-line stderr stays multi-line: {err:?}"
    );
}

#[test]
fn cloned_executors_share_log_and_never_duplicate_seq() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let jj = executor_for(&repo);
    let jj2 = jj.clone();

    // Parallel reads through both clones (the Compare/Interdiff pattern —
    // requires JjExecutor: Sync, i.e. the Arc<Mutex> capture design).
    std::thread::scope(|s| {
        let a = s.spawn(|| jj.log_raw(None, false));
        let b = s.spawn(|| jj2.log_raw(None, false));
        a.join().unwrap().expect("log a");
        b.join().unwrap().expect("log b");
    });

    // Either clone drains the SAME shared log.
    let invs = jj2.take_invocations();
    assert_eq!(invs.len(), 2, "both invocations in the shared log");
    assert_ne!(invs[0].seq, invs[1].seq, "shared counter never duplicates");
    assert!(jj.take_invocations().is_empty());
}

#[test]
fn stderr_commands_are_captured_too() {
    skip_if_no_jj!();
    let repo = TestRepo::new();
    let jj = executor_for(&repo);
    repo.write_file("f.txt", "hello");
    repo.jj(&["describe", "-m", "base"]);

    // duplicate() goes through run_stderr (the former run() bypass).
    let rev = repo.current_change_id();
    jj.duplicate(&rev).expect("duplicate");

    let invs = jj.take_invocations();
    let dup = invs
        .iter()
        .find(|i| i.bare_args.first().map(String::as_str) == Some("duplicate"))
        .expect("duplicate captured");
    assert_eq!(dup.kind, CommandKind::Write);
    assert!(dup.success);
}
