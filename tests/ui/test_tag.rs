//! Snapshot tests for the Tag View (3 groups, columns, conflict marker,
//! filter title, per-row hints).
//!
//! These render the whole `App`, not `TagView::render` alone: the status-bar
//! hints (`[t] Track`, `[F] Filter: …`) are drawn by `App::render_tag_view`,
//! so a View-only snapshot would miss exactly the part that varies with the
//! selected row. What is captured here is the screen the user actually sees.
//!
//! `App::new_for_test()` runs no subprocess, so nothing here needs jj.

use insta::assert_snapshot;
use ratatui::{Terminal, backend::TestBackend};

use tij::app::{App, View};
use tij::model::{ChangeId, CommitId, TagInfo};

// ── Fixtures ──────────────────────────────────────────────────────────────

fn local_tag(name: &str, change_id: Option<&str>, description: Option<&str>) -> TagInfo {
    TagInfo {
        name: name.to_string(),
        remote: None,
        present: true,
        tracked: false,
        conflict: false,
        change_id: change_id.map(|s| ChangeId::new(s.to_string())),
        commit_id: change_id.map(|_| CommitId::new("abcd1234".to_string())),
        description: description.map(str::to_string),
    }
}

fn remote_tag(name: &str, remote: &str, tracked: bool) -> TagInfo {
    TagInfo {
        name: name.to_string(),
        remote: Some(remote.to_string()),
        present: true,
        tracked,
        conflict: false,
        change_id: None,
        commit_id: None,
        description: None,
    }
}

/// Six tags covering all three groups, plus a conflicted local row and a
/// local row with no target (the blank change_id column branch).
fn sample_tags() -> Vec<TagInfo> {
    let mut conflicted = local_tag("v0.12.0", Some("qpvuntsm"), Some("release: v0.12.0"));
    conflicted.conflict = true;
    vec![
        local_tag("v0.11.0", Some("mzslzzzz"), Some("release: v0.11.0")),
        conflicted,
        local_tag("v0.9.0", None, None),
        remote_tag("v0.11.0", "origin", true),
        remote_tag("v0.12.0", "origin", false),
        remote_tag("v0.10.1", "upstream", false),
    ]
}

// ── Harness ───────────────────────────────────────────────────────────────

fn tag_app(tags: Vec<TagInfo>) -> App {
    let mut app = App::new_for_test();
    app.current_view = View::Tag;
    app.tag_view.set_tags(tags);
    app
}

/// 80x16: 13 rows for the view (9 content rows fit) + a 3-row status bar,
/// which is what the Tag View hints wrap to at this width.
fn render(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    terminal.backend().to_string()
}

/// Same frame, with per-cell styles — the only way to pin the group colors
/// (green / dark gray / yellow) and the red conflict marker.
fn render_with_styles(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    format!("{:?}", terminal.backend())
}

/// Move the selection onto the row named `full_name` (headers are skipped by
/// `select_next`, so this walks tag rows only).
fn select_tag(app: &mut App, full_name: &str) {
    app.tag_view.select_first();
    for _ in 0..64 {
        if app
            .tag_view
            .selected_tag()
            .map(TagInfo::full_name)
            .as_deref()
            == Some(full_name)
        {
            return;
        }
        app.tag_view.select_next();
    }
    panic!("no tag row named {full_name}");
}

// ── Tests ─────────────────────────────────────────────────────────────────

/// The flagship layout snapshot: three group headers, remote rows that stop
/// after the name column, the ` !` conflict marker, and Local-row hints
/// (`[d] Delete` / `[P] Push`).
///
/// It also pins the change_id column width. `format!("  {:<10}", change_id)`
/// only pads when `ChangeId`'s `Display` honours the width spec (`f.pad`);
/// the v0.11.1 bug was `f.write_str`, which silently dropped the padding and
/// let the description slide 2 cells left into the change_id column.
#[test]
fn test_tag_view_three_groups() {
    let mut app = tag_app(sample_tags());
    let out = render(&mut app);

    assert!(
        out.contains("mzslzzzz  release: v0.11.0"),
        "change_id must be padded to 10 cells so description starts its own \
         column (regression: ChangeId::fmt using write_str):\n{out}"
    );
    assert_snapshot!(out);
}

/// Colors: Local green, tracked remote dark gray, untracked remote yellow,
/// conflict marker red. Invisible in the plain-text snapshot above.
#[test]
fn test_tag_view_three_groups_styles() {
    let mut app = tag_app(sample_tags());
    assert_snapshot!(render_with_styles(&mut app));
}

/// A tracked remote row: `[T] Untrack` appears, and neither `[d] Delete`
/// (local-only: `jj tag delete` takes a bare name) nor `[P] Push` does.
#[test]
fn test_tag_view_tracked_remote_selected() {
    let mut app = tag_app(sample_tags());
    select_tag(&mut app, "v0.11.0@origin");
    let out = render(&mut app);

    assert!(out.contains("[T] Untrack"), "got:\n{out}");
    assert!(!out.contains("[d] Delete"), "d is local-only:\n{out}");
    assert!(!out.contains("[P] Push"), "P is local-only:\n{out}");
    assert_snapshot!(out);
}

/// An untracked remote row: `[t] Track` appears, `[d] Delete` still does not.
#[test]
fn test_tag_view_untracked_remote_selected() {
    let mut app = tag_app(sample_tags());
    select_tag(&mut app, "v0.12.0@origin");
    let out = render(&mut app);

    assert!(out.contains("[t] Track"), "got:\n{out}");
    assert!(!out.contains("[d] Delete"), "d is local-only:\n{out}");
    assert_snapshot!(out);
}

/// Tracked filter: title becomes `Tags (1/6, tracked)` and the hint label
/// follows the mode.
#[test]
fn test_tag_view_filter_tracked() {
    let mut app = tag_app(sample_tags());
    app.tag_view.cycle_filter(); // All → Tracked
    let out = render(&mut app);

    assert!(out.contains("Tags (1/6, tracked)"), "got:\n{out}");
    assert!(out.contains("[F] Filter: Tracked"), "got:\n{out}");
    assert_snapshot!(out);
}

/// Conflicted filter: only the conflicted local row survives, and it keeps
/// its marker.
///
/// The snapshot's second hint row ends in `[q] B` — that is real, not a
/// snapshot artifact: `build_content` splits the hints once and then lets the
/// second row run past the frame, and `Filter: Conflicted` is the longest
/// filter label. Pre-existing behaviour, recorded here rather than papered
/// over.
#[test]
fn test_tag_view_filter_conflicted() {
    let mut app = tag_app(sample_tags());
    app.tag_view.cycle_filter(); // All → Tracked
    app.tag_view.cycle_filter(); // Tracked → Conflicted
    let out = render(&mut app);

    assert!(out.contains("Tags (1/6, conflicted)"), "got:\n{out}");
    assert!(out.contains("[F] Filter: Conflicted"), "got:\n{out}");
    assert_snapshot!(out);
}

/// No tags at all. Distinct from the filtered-out case below — the message
/// must not tell the user to change a filter that is not hiding anything.
#[test]
fn test_tag_view_empty_no_tags() {
    let mut app = tag_app(vec![]);
    let out = render(&mut app);

    assert!(out.contains("No tags found"), "got:\n{out}");
    assert_snapshot!(out);
}

/// Tags exist but the filter hides them all: the message points at `F`, and
/// the row-specific hints disappear with the selection.
#[test]
fn test_tag_view_empty_filtered_out() {
    let mut app = tag_app(vec![
        local_tag("v0.11.0", Some("mzslzzzz"), Some("release: v0.11.0")),
        remote_tag("v0.11.0", "origin", true),
    ]);
    app.tag_view.cycle_filter(); // All → Tracked
    app.tag_view.cycle_filter(); // Tracked → Conflicted (nothing is conflicted)
    let out = render(&mut app);

    assert!(
        out.contains("No tags match the current filter"),
        "got:\n{out}"
    );
    assert_snapshot!(out);
}
