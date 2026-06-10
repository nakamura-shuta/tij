//! Command History View for displaying executed jj commands

mod input;
mod render;

use crate::model::{CommandHistory, CommandKind};
use crate::ui::navigation;

/// Action returned by the Command History View after handling input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandHistoryAction {
    /// No action needed
    None,
    /// Go back to previous view
    Back,
    /// Toggle detail expansion for the selected record
    ToggleDetail(usize),
    /// Copy a shell-pasteable command line to the clipboard (`y`)
    CopyCommand(String),
}

/// Which records the view shows (`f` cycles; command transparency P1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryFilter {
    /// Everything tij executed
    #[default]
    All,
    /// Write + Interactive — the classic "what did I do" history
    Mutations,
    /// Read invocations only — what tij runs behind the scenes
    Reads,
}

impl HistoryFilter {
    fn next(self) -> Self {
        match self {
            HistoryFilter::All => HistoryFilter::Mutations,
            HistoryFilter::Mutations => HistoryFilter::Reads,
            HistoryFilter::Reads => HistoryFilter::All,
        }
    }

    fn matches(self, kind: CommandKind) -> bool {
        match self {
            HistoryFilter::All => true,
            HistoryFilter::Mutations => {
                matches!(kind, CommandKind::Write | CommandKind::Interactive)
            }
            HistoryFilter::Reads => kind == CommandKind::Read,
        }
    }

    /// Title label (`Command History [Write] (N)`)
    pub fn label(self) -> &'static str {
        match self {
            HistoryFilter::All => "All",
            HistoryFilter::Mutations => "Write",
            HistoryFilter::Reads => "Read",
        }
    }
}

/// Command History View state
///
/// `visible` (raw indices into the history, filtered) is the single source
/// of truth: selection, scroll, Enter detail, `y` copy, and rendering all go
/// through it — mirroring the A2 log-filter design — so a filtered view can
/// never operate on a hidden raw index.
#[derive(Debug)]
pub struct CommandHistoryView {
    /// Selected position within `visible`
    selected: usize,
    /// Scroll offset (rows within `visible`)
    scroll_offset: usize,
    /// Position within `visible` whose detail is expanded
    expanded_index: Option<usize>,
    /// Active kind filter
    filter: HistoryFilter,
    /// Raw record indices matching `filter`, in history order
    visible: Vec<usize>,
}

impl Default for CommandHistoryView {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHistoryView {
    /// Create a new Command History View
    pub fn new() -> Self {
        Self {
            selected: 0,
            scroll_offset: 0,
            expanded_index: None,
            filter: HistoryFilter::All,
            visible: Vec::new(),
        }
    }

    /// Rebuild `visible` from the records and clamp the selection. Called
    /// before key handling and rendering so the view always operates on the
    /// current filtered set.
    pub fn sync(&mut self, history: &CommandHistory) {
        self.visible = history
            .records()
            .iter()
            .enumerate()
            .filter(|(_, r)| self.filter.matches(r.kind))
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.visible.len() {
            self.selected = self.visible.len().saturating_sub(1);
            self.expanded_index = None;
        }
    }

    /// Number of records under the current filter
    pub fn visible_len(&self) -> usize {
        self.visible.len()
    }

    /// Raw history index of the visible row at `pos`
    pub(super) fn raw_index(&self, pos: usize) -> Option<usize> {
        self.visible.get(pos).copied()
    }

    /// Current filter (for the title)
    pub(super) fn filter(&self) -> HistoryFilter {
        self.filter
    }

    /// Cycle the kind filter (`f`)
    pub fn cycle_filter(&mut self) {
        self.filter = self.filter.next();
        self.selected = 0;
        self.scroll_offset = 0;
        self.expanded_index = None;
    }

    /// Move selection to next record
    pub fn select_next(&mut self, total: usize) {
        let max = total.saturating_sub(1);
        self.selected = navigation::select_next(self.selected, max);
        self.expanded_index = None;
    }

    /// Move selection to previous record
    pub fn select_prev(&mut self) {
        self.selected = navigation::select_prev(self.selected);
        self.expanded_index = None;
    }

    /// Go to first record
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
        self.expanded_index = None;
    }

    /// Go to last record
    pub fn select_last(&mut self, total: usize) {
        if total > 0 {
            self.selected = total - 1;
        }
        self.expanded_index = None;
    }

    /// Toggle detail for the currently selected record
    pub fn toggle_detail(&mut self) {
        if self.expanded_index == Some(self.selected) {
            self.expanded_index = None;
        } else {
            self.expanded_index = Some(self.selected);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CommandRecord, CommandStatus};
    use crossterm::event::{KeyCode, KeyEvent};
    use std::time::SystemTime;

    fn record(operation: &str, kind: CommandKind, args: &[&str]) -> CommandRecord {
        CommandRecord {
            operation: operation.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            kind,
            repeat: 1,
            timestamp: SystemTime::UNIX_EPOCH,
            duration_ms: 5,
            status: CommandStatus::Success,
            error: None,
        }
    }

    /// write, read, write, read, interactive
    fn mixed_history() -> CommandHistory {
        let mut h = CommandHistory::new();
        h.push(record("New change", CommandKind::Write, &["new"]));
        h.push(record(
            "log (read)",
            CommandKind::Read,
            &["--color=never", "log"],
        ));
        h.push(record(
            "Describe",
            CommandKind::Write,
            &["describe", "-r", "abc"],
        ));
        h.push(record(
            "show (read)",
            CommandKind::Read,
            &["--color=never", "show"],
        ));
        h.push(record(
            "Split",
            CommandKind::Interactive,
            &["split", "-r", "abc"],
        ));
        h
    }

    #[test]
    fn test_new_command_history_view() {
        let view = CommandHistoryView::new();
        assert_eq!(view.selected, 0);
        assert_eq!(view.scroll_offset, 0);
        assert!(view.expanded_index.is_none());
        assert_eq!(view.filter, HistoryFilter::All);
    }

    #[test]
    fn test_navigation() {
        let mut view = CommandHistoryView::new();
        let h = mixed_history();
        view.handle_key(KeyEvent::from(KeyCode::Char('j')), &h);
        assert_eq!(view.selected, 1);
        view.handle_key(KeyEvent::from(KeyCode::Char('j')), &h);
        assert_eq!(view.selected, 2);
        view.handle_key(KeyEvent::from(KeyCode::Char('k')), &h);
        assert_eq!(view.selected, 1);
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_select_first_last_g_G() {
        let mut view = CommandHistoryView::new();
        let h = mixed_history();
        view.handle_key(KeyEvent::from(KeyCode::Char('G')), &h);
        assert_eq!(view.selected, 4);
        view.handle_key(KeyEvent::from(KeyCode::Char('g')), &h);
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn test_handle_key_back() {
        let mut view = CommandHistoryView::new();
        let h = CommandHistory::new();
        assert_eq!(
            view.handle_key(KeyEvent::from(KeyCode::Char('q')), &h),
            CommandHistoryAction::Back
        );
        assert_eq!(
            view.handle_key(KeyEvent::from(KeyCode::Esc), &h),
            CommandHistoryAction::Back
        );
    }

    #[test]
    fn test_handle_key_enter_toggle() {
        let mut view = CommandHistoryView::new();
        let h = mixed_history();
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter), &h);
        assert_eq!(action, CommandHistoryAction::ToggleDetail(0));
        assert_eq!(view.expanded_index, Some(0));
    }

    #[test]
    fn test_handle_key_enter_empty() {
        let mut view = CommandHistoryView::new();
        let h = CommandHistory::new();
        let action = view.handle_key(KeyEvent::from(KeyCode::Enter), &h);
        assert_eq!(action, CommandHistoryAction::None);
    }

    #[test]
    fn test_move_closes_detail() {
        let mut view = CommandHistoryView::new();
        let h = mixed_history();
        view.handle_key(KeyEvent::from(KeyCode::Enter), &h);
        assert_eq!(view.expanded_index, Some(0));
        view.handle_key(KeyEvent::from(KeyCode::Char('j')), &h);
        assert!(view.expanded_index.is_none());
    }

    #[test]
    fn test_empty_history_no_panic() {
        let mut view = CommandHistoryView::new();
        let h = CommandHistory::new();
        for key in ['j', 'k', 'g', 'G', 'f', 'y'] {
            view.handle_key(KeyEvent::from(KeyCode::Char(key)), &h);
        }
        view.handle_key(KeyEvent::from(KeyCode::Enter), &h);
        assert_eq!(view.selected, 0);
    }

    // ---- filter (f) + visible_indices: the single source of truth ----

    #[test]
    fn filter_cycles_and_visible_follows() {
        let mut view = CommandHistoryView::new();
        let h = mixed_history();
        view.sync(&h);
        assert_eq!(view.visible_len(), 5, "All shows everything");

        view.handle_key(KeyEvent::from(KeyCode::Char('f')), &h);
        assert_eq!(view.filter, HistoryFilter::Mutations);
        assert_eq!(view.visible_len(), 3, "writes + interactive");

        view.handle_key(KeyEvent::from(KeyCode::Char('f')), &h);
        assert_eq!(view.filter, HistoryFilter::Reads);
        assert_eq!(view.visible_len(), 2);

        view.handle_key(KeyEvent::from(KeyCode::Char('f')), &h);
        assert_eq!(view.filter, HistoryFilter::All);
        assert_eq!(view.visible_len(), 5);
    }

    #[test]
    fn copy_targets_the_filtered_row_not_raw_index() {
        // Regression: under a filter, `y` must copy the row the user SEES.
        // With Reads filter on, visible row 1 is the raw index 3 ("show").
        let mut view = CommandHistoryView::new();
        let h = mixed_history();
        view.handle_key(KeyEvent::from(KeyCode::Char('f')), &h); // Mutations
        view.handle_key(KeyEvent::from(KeyCode::Char('f')), &h); // Reads
        view.handle_key(KeyEvent::from(KeyCode::Char('j')), &h); // select 2nd read
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('y')), &h);
        assert_eq!(
            action,
            CommandHistoryAction::CopyCommand("jj --color=never show".to_string())
        );
    }

    #[test]
    fn selection_clamps_when_filter_shrinks_visible() {
        let mut view = CommandHistoryView::new();
        let h = mixed_history();
        view.handle_key(KeyEvent::from(KeyCode::Char('G')), &h); // selected = 4
        view.handle_key(KeyEvent::from(KeyCode::Char('f')), &h); // Mutations (3 rows)
        assert!(view.selected < view.visible_len());
    }

    #[test]
    fn copy_emits_shell_pasteable_line() {
        let mut view = CommandHistoryView::new();
        let mut h = CommandHistory::new();
        h.push(record(
            "log (read)",
            CommandKind::Read,
            &["--color=never", "log", "-r", "all()"],
        ));
        let action = view.handle_key(KeyEvent::from(KeyCode::Char('y')), &h);
        assert_eq!(
            action,
            CommandHistoryAction::CopyCommand("jj --color=never log -r 'all()'".to_string())
        );
    }
}
