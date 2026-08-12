//! Tag View for displaying all tags

mod input;
mod render;

use crate::model::TagInfo;
use crate::ui::navigation;

/// Action returned by the Tag View after handling input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagAction {
    /// No action needed
    None,
    /// Jump to tag's commit in Log View (change_id)
    Jump(String),
    /// Create new tag (open input dialog)
    StartCreate,
    /// Delete selected tag (open confirm dialog)
    Delete(String),
    /// Start tracking an untracked remote tag (`<name>@<remote>`)
    Track(String),
    /// Stop tracking a tracked remote tag (`<name>@<remote>`)
    Untrack(String),
    /// Push a local tag to a remote (bare tag name; `exact:` is added downstream)
    Push(String),
    /// Cycle the display filter (all → tracked → conflicted)
    CycleFilter,
}

/// Display row type for rendering
#[derive(Debug, Clone)]
pub(super) enum DisplayRow {
    /// Group header (e.g., "── Local ──")
    Header(String),
    /// Tag entry (index into TagView.tags)
    Tag(usize),
}

/// Client-side display filter for the Tag View
///
/// The `--all-remotes` listing is a superset of every mode, so filtering
/// happens in-memory (no jj re-run) and switching is instant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TagFilter {
    /// Every row (after `@git` exclusion)
    #[default]
    All,
    /// Tracked remote rows only
    Tracked,
    /// Conflicted rows only
    Conflicted,
}

impl TagFilter {
    /// Next mode in the cycle: All → Tracked → Conflicted → All
    pub fn next(self) -> Self {
        match self {
            TagFilter::All => TagFilter::Tracked,
            TagFilter::Tracked => TagFilter::Conflicted,
            TagFilter::Conflicted => TagFilter::All,
        }
    }

    /// Lowercase label for the view title (`Tags (1/6, tracked)`)
    pub fn label(self) -> &'static str {
        match self {
            TagFilter::All => "all",
            TagFilter::Tracked => "tracked",
            TagFilter::Conflicted => "conflicted",
        }
    }

    /// Whether `tag` is visible under this mode
    fn matches(self, tag: &TagInfo) -> bool {
        match self {
            TagFilter::All => true,
            TagFilter::Tracked => tag.is_tracked_remote(),
            TagFilter::Conflicted => tag.conflict,
        }
    }
}

/// Tag View state
#[derive(Debug)]
pub struct TagView {
    /// All tags from jj, minus the internal `@git` rows (filter input)
    all_tags: Vec<TagInfo>,
    /// Tags currently displayed (filtered, grouped and sorted)
    tags: Vec<TagInfo>,
    /// Display rows (headers + tag indices)
    display_rows: Vec<DisplayRow>,
    /// Selected row index (within display_rows, only Tag rows are selectable)
    selected: usize,
    /// Scroll offset
    scroll_offset: usize,
    /// Current display filter
    filter: TagFilter,
}

impl Default for TagView {
    fn default() -> Self {
        Self::new()
    }
}

impl TagView {
    /// Create a new Tag View
    pub fn new() -> Self {
        Self {
            all_tags: Vec::new(),
            tags: Vec::new(),
            display_rows: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            filter: TagFilter::All,
        }
    }

    /// Set the tags to display, sorted and grouped
    pub fn set_tags(&mut self, mut tags: Vec<TagInfo>) {
        // Filter out @git remote entries (internal jj representation).
        // Without this every tag of a colocated repo shows up twice.
        tags.retain(|t| t.remote.as_deref() != Some("git"));
        self.all_tags = tags;
        self.rebuild_rows();
    }

    /// Re-apply the current filter and rebuild grouped display rows
    ///
    /// Resets the selection to the first selectable row.
    fn rebuild_rows(&mut self) {
        let mut tags: Vec<TagInfo> = self
            .all_tags
            .iter()
            .filter(|t| self.filter.matches(t))
            .cloned()
            .collect();

        // Sort: local first, then tracked remote, then untracked remote.
        // Within each group, sort alphabetically by full name.
        tags.sort_by(|a, b| {
            tag_group_order(a)
                .cmp(&tag_group_order(b))
                .then(a.full_name().cmp(&b.full_name()))
        });

        // Build display rows with headers
        let mut rows = Vec::new();
        let mut current_group = None;

        for (idx, tag) in tags.iter().enumerate() {
            let group = tag_group_order(tag);
            if current_group != Some(group) {
                current_group = Some(group);
                let header = match group {
                    0 => "── Local ──",
                    1 => "── Remote (tracked) ──",
                    2 => "── Remote (untracked) ──",
                    _ => "── Other ──",
                };
                rows.push(DisplayRow::Header(header.to_string()));
            }
            rows.push(DisplayRow::Tag(idx));
        }

        self.tags = tags;
        self.display_rows = rows;
        self.selected = self.first_tag_row().unwrap_or(0);
        self.scroll_offset = 0;
    }

    /// Get the currently selected tag
    pub fn selected_tag(&self) -> Option<&TagInfo> {
        if let Some(DisplayRow::Tag(idx)) = self.display_rows.get(self.selected) {
            self.tags.get(*idx)
        } else {
            None
        }
    }

    /// Number of tags currently displayed (excluding headers)
    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }

    /// Number of tags before the filter is applied (`@git` rows already excluded)
    pub fn total_count(&self) -> usize {
        self.all_tags.len()
    }

    /// Current display filter
    pub fn filter(&self) -> TagFilter {
        self.filter
    }

    /// Advance to the next filter mode and rebuild the rows
    pub fn cycle_filter(&mut self) {
        self.filter = self.filter.next();
        self.rebuild_rows();
    }

    /// Move selection to next tag row (skip headers)
    pub fn select_next(&mut self) {
        let max = self.display_rows.len().saturating_sub(1);
        let mut next = navigation::select_next(self.selected, max);
        while next <= max {
            if matches!(self.display_rows.get(next), Some(DisplayRow::Tag(_))) {
                break;
            }
            if next == max {
                return;
            }
            next = navigation::select_next(next, max);
        }
        self.selected = next;
    }

    /// Move selection to previous tag row (skip headers)
    pub fn select_prev(&mut self) {
        let mut prev = navigation::select_prev(self.selected);
        loop {
            if matches!(self.display_rows.get(prev), Some(DisplayRow::Tag(_))) {
                break;
            }
            if prev == 0 {
                return;
            }
            prev = navigation::select_prev(prev);
        }
        self.selected = prev;
    }

    /// Go to first tag row
    pub fn select_first(&mut self) {
        if let Some(first) = self.first_tag_row() {
            self.selected = first;
            self.scroll_offset = 0;
        }
    }

    /// Go to last tag row
    pub fn select_last(&mut self) {
        if let Some(last) = self.last_tag_row() {
            self.selected = last;
        }
    }

    fn first_tag_row(&self) -> Option<usize> {
        self.display_rows
            .iter()
            .position(|r| matches!(r, DisplayRow::Tag(_)))
    }

    fn last_tag_row(&self) -> Option<usize> {
        self.display_rows
            .iter()
            .rposition(|r| matches!(r, DisplayRow::Tag(_)))
    }
}

/// Return sort order: 0=local, 1=tracked remote, 2=untracked remote
fn tag_group_order(tag: &TagInfo) -> u8 {
    if tag.is_local() {
        0
    } else if tag.tracked {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChangeId, CommitId};
    use crossterm::event::{KeyCode, KeyEvent};

    fn make_tag(name: &str, change_id: Option<&str>, desc: Option<&str>) -> TagInfo {
        TagInfo {
            name: name.to_string(),
            remote: None,
            present: true,
            tracked: false,
            conflict: false,
            change_id: change_id.map(|s| ChangeId::new(s.to_string())),
            commit_id: Some(CommitId::new("abcd1234".to_string())),
            description: desc.map(|s| s.to_string()),
        }
    }

    fn make_remote_tag(name: &str, remote: &str, tracked: bool) -> TagInfo {
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

    /// Tags sort alphabetically by full name, so the display order of these
    /// three is: v0.4.10, v0.4.8, v0.4.9 ('1' < '8' < '9').
    fn create_test_tags() -> Vec<TagInfo> {
        vec![
            make_tag("v0.4.10", Some("mzslzzzz"), Some("fix: preview pane")),
            make_tag("v0.4.9", Some("swknqzvs"), Some("feat: highlight")),
            make_tag("v0.4.8", Some("qknsuxln"), Some("fix: notification")),
        ]
    }

    /// 2 local + 1 tracked remote + 2 untracked remote + 2 internal @git rows
    fn create_mixed_tags() -> Vec<TagInfo> {
        vec![
            make_tag("v1.0", Some("mzslzzzz"), Some("release v1.0")),
            make_tag("v0.9", Some("swknqzvs"), Some("release v0.9")),
            make_remote_tag("v1.0", "origin", true),
            make_remote_tag("v0.8", "origin", false),
            make_remote_tag("v0.7", "origin", false),
            make_remote_tag("v1.0", "git", true),
            make_remote_tag("v0.9", "git", true),
        ]
    }

    #[test]
    fn test_new_tag_view() {
        let view = TagView::new();
        assert!(view.tags.is_empty());
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_set_tags() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        assert_eq!(view.tag_count(), 3);
        // Row 0 is the "── Local ──" header, so the first tag row is 1
        assert_eq!(view.selected, 1);
    }

    #[test]
    fn test_selected_tag() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        let selected = view.selected_tag().unwrap();
        assert_eq!(selected.name, "v0.4.10");
    }

    #[test]
    fn test_navigation() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        assert_eq!(view.selected, 1);

        view.select_next();
        assert_eq!(view.selected, 2);
        assert_eq!(view.selected_tag().unwrap().name, "v0.4.8");

        view.select_next();
        assert_eq!(view.selected, 3);
        assert_eq!(view.selected_tag().unwrap().name, "v0.4.9");

        // At end, should not advance
        view.select_next();
        assert_eq!(view.selected, 3);

        view.select_prev();
        assert_eq!(view.selected, 2);
    }

    #[test]
    fn test_select_first_last() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());

        view.select_last();
        assert_eq!(view.selected_tag().unwrap().name, "v0.4.9");

        view.select_first();
        assert_eq!(view.selected_tag().unwrap().name, "v0.4.10");
    }

    #[test]
    fn test_empty_tags() {
        let mut view = TagView::new();
        view.set_tags(vec![]);
        assert_eq!(view.tag_count(), 0);
        assert!(view.selected_tag().is_none());
    }

    #[test]
    fn test_handle_key_enter_jumpable() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(action, TagAction::Jump(id) if id == "mzslzzzz"));
    }

    #[test]
    fn test_handle_key_enter_not_jumpable() {
        let mut view = TagView::new();
        view.set_tags(vec![TagInfo {
            name: "v0.1".into(),
            remote: None,
            present: true,
            tracked: false,
            conflict: false,
            change_id: None,
            commit_id: None,
            description: None,
        }]);
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter));
        assert!(matches!(action, TagAction::None));
    }

    #[test]
    fn test_handle_key_create() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('n')));
        assert!(matches!(action, TagAction::StartCreate));
    }

    #[test]
    fn test_handle_key_delete() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('d')));
        assert!(matches!(action, TagAction::Delete(name) if name == "v0.4.10"));
    }

    #[test]
    fn test_handle_key_delete_empty() {
        let mut view = TagView::new();
        view.set_tags(vec![]);
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('d')));
        assert!(matches!(action, TagAction::None));
    }

    #[test]
    fn test_handle_key_navigation_j_k() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('j')));
        assert!(matches!(action, TagAction::None));
        assert_eq!(view.selected, 2);

        let action = view.handle_key(KeyEvent::from(KeyCode::Char('k')));
        assert!(matches!(action, TagAction::None));
        assert_eq!(view.selected, 1);
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_handle_key_g_G() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());

        view.handle_key(KeyEvent::from(KeyCode::Char('G')));
        assert_eq!(view.selected, 3);

        view.handle_key(KeyEvent::from(KeyCode::Char('g')));
        assert_eq!(view.selected, 1);
    }

    #[test]
    fn test_set_tags_resets_selection() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        view.select_last();
        assert_eq!(view.selected, 3);

        // Setting new tags should reset selection to the first tag row
        view.set_tags(create_test_tags());
        assert_eq!(view.selected, 1);
    }

    // --- Grouping / @git exclusion ---

    #[test]
    fn set_tags_excludes_internal_git_remote_rows() {
        let mut view = TagView::new();
        view.set_tags(create_mixed_tags());
        // 7 input rows − 2 @git rows = 5
        assert_eq!(view.tag_count(), 5);
        assert_eq!(view.total_count(), 5);
        assert!(
            view.tags.iter().all(|t| t.remote.as_deref() != Some("git")),
            "no @git row may survive set_tags"
        );
    }

    #[test]
    fn set_tags_groups_with_headers_in_order() {
        let mut view = TagView::new();
        view.set_tags(create_mixed_tags());

        // 3 headers + 5 tags
        assert_eq!(view.display_rows.len(), 8);
        assert!(matches!(&view.display_rows[0], DisplayRow::Header(h) if h.contains("Local")));
        assert!(matches!(&view.display_rows[3], DisplayRow::Header(h) if h.contains("(tracked)")));
        assert!(
            matches!(&view.display_rows[5], DisplayRow::Header(h) if h.contains("(untracked)"))
        );

        let names: Vec<String> = view.tags.iter().map(|t| t.full_name()).collect();
        assert_eq!(
            names,
            vec!["v0.9", "v1.0", "v1.0@origin", "v0.7@origin", "v0.8@origin"]
        );
    }

    #[test]
    fn only_locals_produce_a_single_header() {
        let mut view = TagView::new();
        view.set_tags(create_test_tags());
        assert_eq!(view.display_rows.len(), 4); // 1 header + 3 tags
        assert!(matches!(&view.display_rows[0], DisplayRow::Header(h) if h.contains("Local")));
    }

    #[test]
    fn navigation_skips_headers() {
        let mut view = TagView::new();
        view.set_tags(create_mixed_tags());

        // rows: 0=H 1=v0.9 2=v1.0 3=H 4=v1.0@origin 5=H 6=v0.7@origin 7=v0.8@origin
        assert_eq!(view.selected, 1);
        view.select_next();
        assert_eq!(view.selected, 2);
        view.select_next();
        assert_eq!(view.selected, 4, "header at 3 must be skipped");
        view.select_next();
        assert_eq!(view.selected, 6, "header at 5 must be skipped");

        view.select_prev();
        assert_eq!(view.selected, 4, "header at 5 must be skipped going up");
        view.select_prev();
        assert_eq!(view.selected, 2, "header at 3 must be skipped going up");

        // A header is never selectable
        for _ in 0..10 {
            view.select_prev();
            assert!(view.selected_tag().is_some());
        }
        for _ in 0..10 {
            view.select_next();
            assert!(view.selected_tag().is_some());
        }
    }

    #[test]
    fn select_first_last_land_on_tag_rows() {
        let mut view = TagView::new();
        view.set_tags(create_mixed_tags());

        view.select_last();
        assert_eq!(view.selected, 7);
        assert_eq!(view.selected_tag().unwrap().full_name(), "v0.8@origin");

        view.select_first();
        assert_eq!(view.selected, 1);
        assert_eq!(view.selected_tag().unwrap().full_name(), "v0.9");
    }

    // --- Filter ---

    #[test]
    fn filter_cycles_all_tracked_conflicted() {
        assert_eq!(TagFilter::All.next(), TagFilter::Tracked);
        assert_eq!(TagFilter::Tracked.next(), TagFilter::Conflicted);
        assert_eq!(TagFilter::Conflicted.next(), TagFilter::All);

        let mut view = TagView::new();
        assert_eq!(view.filter(), TagFilter::All);
        view.cycle_filter();
        assert_eq!(view.filter(), TagFilter::Tracked);
        view.cycle_filter();
        assert_eq!(view.filter(), TagFilter::Conflicted);
        view.cycle_filter();
        assert_eq!(view.filter(), TagFilter::All);
    }

    #[test]
    fn filter_tracked_keeps_only_tracked_remote_rows() {
        let mut view = TagView::new();
        view.set_tags(create_mixed_tags());
        view.cycle_filter(); // Tracked

        assert_eq!(view.tag_count(), 1);
        assert_eq!(view.total_count(), 5, "total is the pre-filter count");
        assert_eq!(view.tags[0].full_name(), "v1.0@origin");
        // Only the "Remote (tracked)" header remains
        assert_eq!(view.display_rows.len(), 2);
        assert!(matches!(&view.display_rows[0], DisplayRow::Header(h) if h.contains("(tracked)")));
        assert_eq!(view.selected_tag().unwrap().full_name(), "v1.0@origin");
    }

    #[test]
    fn filter_conflicted_keeps_only_conflicted_rows() {
        let mut tags = create_mixed_tags();
        tags[1].conflict = true; // local v0.9
        let mut view = TagView::new();
        view.set_tags(tags);

        view.cycle_filter(); // Tracked
        view.cycle_filter(); // Conflicted
        assert_eq!(view.tag_count(), 1);
        assert_eq!(view.tags[0].full_name(), "v0.9");
        assert!(view.selected_tag().unwrap().conflict);
    }

    #[test]
    fn selected_tag_is_none_when_filter_matches_nothing() {
        let mut view = TagView::new();
        view.set_tags(create_mixed_tags()); // nothing is conflicted
        view.cycle_filter(); // Tracked
        view.cycle_filter(); // Conflicted

        assert_eq!(view.tag_count(), 0);
        assert_eq!(view.total_count(), 5);
        assert!(view.display_rows.is_empty(), "no header without rows");
        assert!(view.selected_tag().is_none());
        // Navigation on an empty filter result must stay a no-op
        view.select_next();
        view.select_prev();
        view.select_last();
        view.select_first();
        assert!(view.selected_tag().is_none());
    }

    #[test]
    fn cycle_filter_resets_selection_to_first_row() {
        let mut view = TagView::new();
        view.set_tags(create_mixed_tags());
        view.select_last();
        assert_eq!(view.selected, 7);

        view.cycle_filter(); // Tracked → single row
        assert_eq!(view.selected, 1);
        assert_eq!(view.selected_tag().unwrap().full_name(), "v1.0@origin");
    }

    // --- handle_key: track / untrack / push / filter ---

    /// Select the display row `idx` (rows include headers).
    fn select_row(view: &mut TagView, idx: usize) {
        view.selected = idx;
        assert!(
            view.selected_tag().is_some(),
            "row {idx} must be a selectable tag row"
        );
    }

    fn press(view: &mut TagView, c: char) -> TagAction {
        view.handle_key(KeyEvent::from(KeyCode::Char(c)))
    }

    /// rows: 0=H 1=v0.9 2=v1.0 3=H 4=v1.0@origin 5=H 6=v0.7@origin 7=v0.8@origin
    fn mixed_view() -> TagView {
        let mut view = TagView::new();
        view.set_tags(create_mixed_tags());
        view
    }

    #[test]
    fn delete_is_local_only() {
        // Regression: before remote rows were displayed, `d` was always on a
        // local tag. `jj tag delete` takes a bare name, so firing it from a
        // remote row would delete the local tag of the same name — a
        // destructive action on a different object than the cursor is on.
        let mut view = mixed_view();

        select_row(&mut view, 2); // local v1.0
        assert_eq!(press(&mut view, 'd'), TagAction::Delete("v1.0".to_string()));

        select_row(&mut view, 4); // v1.0@origin (tracked remote)
        assert_eq!(press(&mut view, 'd'), TagAction::None);

        select_row(&mut view, 6); // v0.7@origin (untracked remote)
        assert_eq!(press(&mut view, 'd'), TagAction::None);
    }

    #[test]
    fn local_row_pushes_and_ignores_track_untrack() {
        let mut view = mixed_view();
        select_row(&mut view, 1);
        assert_eq!(view.selected_tag().unwrap().full_name(), "v0.9");

        assert_eq!(press(&mut view, 't'), TagAction::None, "local: no track");
        assert_eq!(press(&mut view, 'T'), TagAction::None, "local: no untrack");
        assert_eq!(
            press(&mut view, 'P'),
            TagAction::Push("v0.9".to_string()),
            "push passes the bare name (exact: is added by the App)"
        );
    }

    #[test]
    fn tracked_remote_row_untracks_only() {
        let mut view = mixed_view();
        select_row(&mut view, 4);
        assert_eq!(view.selected_tag().unwrap().full_name(), "v1.0@origin");

        assert_eq!(
            press(&mut view, 't'),
            TagAction::None,
            "already tracked: no track"
        );
        assert_eq!(
            press(&mut view, 'T'),
            TagAction::Untrack("v1.0@origin".to_string())
        );
        assert_eq!(
            press(&mut view, 'P'),
            TagAction::None,
            "remote rows are not pushable"
        );
    }

    #[test]
    fn untracked_remote_row_tracks_only() {
        let mut view = mixed_view();
        select_row(&mut view, 6);
        assert_eq!(view.selected_tag().unwrap().full_name(), "v0.7@origin");

        assert_eq!(
            press(&mut view, 't'),
            TagAction::Track("v0.7@origin".to_string())
        );
        assert_eq!(
            press(&mut view, 'T'),
            TagAction::None,
            "not tracked: no untrack"
        );
        assert_eq!(
            press(&mut view, 'P'),
            TagAction::None,
            "remote rows are not pushable"
        );
    }

    #[test]
    fn no_selection_makes_track_untrack_push_noops() {
        let mut view = TagView::new();
        view.set_tags(vec![]);
        assert!(view.selected_tag().is_none());

        assert_eq!(press(&mut view, 't'), TagAction::None);
        assert_eq!(press(&mut view, 'T'), TagAction::None);
        assert_eq!(press(&mut view, 'P'), TagAction::None);
    }

    #[test]
    fn filter_key_works_on_every_selection_kind() {
        // F is unconditional: it never depends on the selected row.
        for row in [1, 4, 6] {
            let mut view = mixed_view();
            select_row(&mut view, row);
            assert_eq!(press(&mut view, 'F'), TagAction::CycleFilter, "row {row}");
        }
        let mut empty = TagView::new();
        empty.set_tags(vec![]);
        assert_eq!(
            press(&mut empty, 'F'),
            TagAction::CycleFilter,
            "no selection"
        );
    }

    #[test]
    fn set_tags_keeps_the_current_filter() {
        let mut view = TagView::new();
        view.set_tags(create_mixed_tags());
        view.cycle_filter(); // Tracked

        // A refresh (e.g. after track/untrack) must not silently reset the filter
        view.set_tags(create_mixed_tags());
        assert_eq!(view.filter(), TagFilter::Tracked);
        assert_eq!(view.tag_count(), 1);
    }
}
