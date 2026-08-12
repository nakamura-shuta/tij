//! Snapshot tests for the Bookmark View (3 groups, columns, per-row hints).
//!
//! Like the Tag View tests these render the whole `App` — the status-bar
//! hints (`[t] Track`, `[m] Move`, …) come from `App::render_bookmark_view`,
//! not from `BookmarkView::render`, and they are what varies per row.
//!
//! `App::new_for_test()` runs no subprocess, so nothing here needs jj.

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::app::{App, View};
use tij::model::{Bookmark, BookmarkInfo, ChangeId, CommitId};

// ── Fixtures ──────────────────────────────────────────────────────────────

fn local_bookmark(name: &str, change_id: Option<&str>, description: Option<&str>) -> BookmarkInfo {
    BookmarkInfo {
        bookmark: Bookmark {
            name: name.to_string(),
            remote: None,
            is_tracked: false,
        },
        change_id: change_id.map(|s| ChangeId::new(s.to_string())),
        commit_id: change_id.map(|_| CommitId::new("abcd1234".to_string())),
        description: description.map(str::to_string),
    }
}

fn remote_bookmark(name: &str, remote: &str, is_tracked: bool) -> BookmarkInfo {
    BookmarkInfo {
        bookmark: Bookmark {
            name: name.to_string(),
            remote: Some(remote.to_string()),
            is_tracked,
        },
        change_id: None,
        commit_id: None,
        description: None,
    }
}

/// Six bookmarks covering all three groups, plus a local row with no target
/// (the blank change_id column branch, which also drops `[Enter] Jump`).
fn sample_bookmarks() -> Vec<BookmarkInfo> {
    vec![
        local_bookmark("main", Some("kxryzmor"), Some("feat: add parser")),
        local_bookmark("feature-x", Some("vwxyzabc"), Some("wip: refactor")),
        local_bookmark("orphan", None, None),
        remote_bookmark("main", "origin", true),
        remote_bookmark("feature-y", "origin", false),
        remote_bookmark("main", "upstream", false),
    ]
}

// ── Harness ───────────────────────────────────────────────────────────────

fn bookmark_app(bookmarks: Vec<BookmarkInfo>) -> App {
    let mut app = App::new_for_test();
    app.current_view = View::Bookmark;
    app.bookmark_view.set_bookmarks(bookmarks);
    app
}

/// 80x16: 13 rows for the view (9 content rows fit) + a 3-row status bar,
/// which is what the Bookmark View hints wrap to at this width.
fn render(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    terminal.backend().to_string()
}

/// Same frame, with per-cell styles — the only way to pin the group colors
/// (white / dark gray / yellow).
fn render_with_styles(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    format!("{:?}", terminal.backend())
}

/// Move the selection onto the row named `full_name` (headers are skipped by
/// `select_next`, so this walks bookmark rows only).
fn select_bookmark(app: &mut App, full_name: &str) {
    app.bookmark_view.select_first();
    for _ in 0..64 {
        let current = app
            .bookmark_view
            .selected_bookmark()
            .map(|info| info.bookmark.full_name());
        if current.as_deref() == Some(full_name) {
            return;
        }
        app.bookmark_view.select_next();
    }
    panic!("no bookmark row named {full_name}");
}

// ── Tests ─────────────────────────────────────────────────────────────────

/// The flagship layout snapshot: three group headers, remote rows that stop
/// after the name column, and local-row hints (Delete / Rename / Forget /
/// Move).
///
/// It also pins the change_id column width. `format!("  {:<10}", change_id)`
/// only pads when `ChangeId`'s `Display` honours the width spec (`f.pad`);
/// the v0.11.1 bug was `f.write_str`, which silently dropped the padding and
/// let the description slide 2 cells left into the change_id column.
#[test]
fn test_bookmark_view_three_groups() {
    let mut app = bookmark_app(sample_bookmarks());
    let out = render(&mut app);

    assert!(
        out.contains("vwxyzabc  wip: refactor"),
        "change_id must be padded to 10 cells so description starts its own \
         column (regression: ChangeId::fmt using write_str):\n{out}"
    );
    assert_snapshot!(out);
}

/// Colors: local white, tracked remote dark gray, untracked remote yellow.
/// Invisible in the plain-text snapshot above.
#[test]
fn test_bookmark_view_three_groups_styles() {
    let mut app = bookmark_app(sample_bookmarks());
    assert_snapshot!(render_with_styles(&mut app));
}

/// A local row with no change_id: everything local stays available except
/// `[Enter] Jump`, which has no target.
#[test]
fn test_bookmark_view_local_without_change_id() {
    let mut app = bookmark_app(sample_bookmarks());
    select_bookmark(&mut app, "orphan");
    let out = render(&mut app);

    assert!(!out.contains("[Enter] Jump"), "nothing to jump to:\n{out}");
    assert!(out.contains("[d] Delete"), "got:\n{out}");
    assert_snapshot!(out);
}

/// A tracked remote row: `[T] Untrack` only — no Delete / Rename / Forget /
/// Move, all of which act on a local bookmark.
#[test]
fn test_bookmark_view_tracked_remote_selected() {
    let mut app = bookmark_app(sample_bookmarks());
    select_bookmark(&mut app, "main@origin");
    let out = render(&mut app);

    assert!(out.contains("[T] Untrack"), "got:\n{out}");
    assert!(!out.contains("[d] Delete"), "d is local-only:\n{out}");
    assert!(!out.contains("[m] Move"), "m is local-only:\n{out}");
    assert_snapshot!(out);
}

/// An untracked remote row: `[t] Track` only.
#[test]
fn test_bookmark_view_untracked_remote_selected() {
    let mut app = bookmark_app(sample_bookmarks());
    select_bookmark(&mut app, "feature-y@origin");
    let out = render(&mut app);

    assert!(out.contains("[t] Track"), "got:\n{out}");
    assert!(!out.contains("[d] Delete"), "d is local-only:\n{out}");
    assert_snapshot!(out);
}

/// No bookmarks: the placeholder message, and no row-specific hints.
#[test]
fn test_bookmark_view_empty() {
    let mut app = bookmark_app(vec![]);
    let out = render(&mut app);

    assert!(out.contains("No bookmarks found"), "got:\n{out}");
    assert_snapshot!(out);
}
