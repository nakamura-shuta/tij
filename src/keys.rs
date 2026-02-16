//! Keybinding definitions for Tij
//!
//! All keybindings are defined here for easy modification and future config file support.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;

use crate::app::View;
use crate::ui::views::InputMode;

// =============================================================================
// Key detection helpers (for modifier keys)
// =============================================================================

/// Check if key is Ctrl+L (refresh)
/// Note: Accept both 'l' and 'L' for terminal compatibility
pub fn is_refresh_key(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('l') | KeyCode::Char('L'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

// =============================================================================
// Global keys (available in all views)
// =============================================================================

/// Quit application or go back
pub const QUIT: KeyCode = KeyCode::Char('q');

/// Show help
pub const HELP: KeyCode = KeyCode::Char('?');

/// Switch between views
pub const TAB: KeyCode = KeyCode::Tab;

/// Alternative quit
pub const ESC: KeyCode = KeyCode::Esc;

// =============================================================================
// Navigation keys
// =============================================================================

/// Move cursor up (vim style)
pub const MOVE_UP: KeyCode = KeyCode::Char('k');

/// Move cursor up (arrow key)
pub const MOVE_UP_ARROW: KeyCode = KeyCode::Up;

/// Move cursor down (vim style)
pub const MOVE_DOWN: KeyCode = KeyCode::Char('j');

/// Move cursor down (arrow key)
pub const MOVE_DOWN_ARROW: KeyCode = KeyCode::Down;

/// Go to top
pub const GO_TOP: KeyCode = KeyCode::Char('g');

/// Go to bottom
pub const GO_BOTTOM: KeyCode = KeyCode::Char('G');

/// Check if key is move up (k or ↑)
pub fn is_move_up(code: KeyCode) -> bool {
    matches!(code, MOVE_UP | MOVE_UP_ARROW)
}

/// Check if key is move down (j or ↓)
pub fn is_move_down(code: KeyCode) -> bool {
    matches!(code, MOVE_DOWN | MOVE_DOWN_ARROW)
}

// =============================================================================
// Input keys (used in input modes)
// =============================================================================

/// Submit input (Enter in input mode)
pub const SUBMIT: KeyCode = KeyCode::Enter;

// =============================================================================
// Log View keys
// =============================================================================

/// Open diff view for selected commit
pub const OPEN_DIFF: KeyCode = KeyCode::Enter;

/// Edit change description
pub const DESCRIBE: KeyCode = KeyCode::Char('d');

/// Edit (set working-copy to selected change)
pub const EDIT: KeyCode = KeyCode::Char('e');

/// Create new change
pub const NEW_CHANGE: KeyCode = KeyCode::Char('c');

/// Commit changes (Status View, uppercase)
pub const COMMIT: KeyCode = KeyCode::Char('C');

/// Squash change (select destination, Log View, uppercase)
pub const SQUASH: KeyCode = KeyCode::Char('S');

/// Abandon change (Log View, uppercase)
pub const ABANDON: KeyCode = KeyCode::Char('A');

/// Split change (Log View, opens external diff editor)
pub const SPLIT: KeyCode = KeyCode::Char('x');

/// Create bookmark (Log View)
pub const BOOKMARK: KeyCode = KeyCode::Char('b');

/// Delete bookmark (Log View)
pub const BOOKMARK_DELETE: KeyCode = KeyCode::Char('D');

/// Rebase change (Log View, uppercase)
pub const REBASE: KeyCode = KeyCode::Char('R');

/// Absorb changes into ancestors (Log View, uppercase)
pub const ABSORB: KeyCode = KeyCode::Char('B');

/// Create new change from selected revision (Log View, uppercase)
/// Note: This is different from COMMIT ('C' in Status View)
pub const NEW_FROM: KeyCode = KeyCode::Char('C');

/// Show file annotation/blame (Status/Diff View)
pub const ANNOTATE: KeyCode = KeyCode::Char('a');

/// Open resolve list view for conflicts (Log View, uppercase)
pub const RESOLVE_LIST: KeyCode = KeyCode::Char('X');

/// Fetch from remote (Log View, uppercase for remote ops)
pub const FETCH: KeyCode = KeyCode::Char('F');

/// Push to remote (Log View, uppercase for remote ops)
pub const PUSH: KeyCode = KeyCode::Char('P');

/// Track remote bookmarks (Log View, uppercase for remote ops)
pub const TRACK: KeyCode = KeyCode::Char('T');

/// Jump to bookmark (Log View)
pub const BOOKMARK_JUMP: KeyCode = KeyCode::Char('\'');

/// Compare two revisions (Log View)
pub const COMPARE: KeyCode = KeyCode::Char('=');

/// Open Bookmark View (Log View)
pub const BOOKMARK_VIEW: KeyCode = KeyCode::Char('M');

/// Toggle preview pane (Log View)
pub const PREVIEW: KeyCode = KeyCode::Char('p');

/// Untrack remote bookmark (Bookmark View)
pub const BOOKMARK_UNTRACK: KeyCode = KeyCode::Char('U');

/// Rename bookmark (Bookmark View)
pub const BOOKMARK_RENAME: KeyCode = KeyCode::Char('r');

/// Forget bookmark (Bookmark View)
pub const BOOKMARK_FORGET: KeyCode = KeyCode::Char('f');

/// Move @ to next child (Log View)
pub const NEXT_CHANGE: KeyCode = KeyCode::Char(']');

/// Move @ to previous parent (Log View)
pub const PREV_CHANGE: KeyCode = KeyCode::Char('[');

/// Toggle reversed display order (Log View)
pub const LOG_REVERSE: KeyCode = KeyCode::Char('V');

/// Jump to change in Log View (Blame View)
pub const JUMP_TO_LOG: KeyCode = KeyCode::Char('J');

/// Jump to first conflict file (Status View)
pub const JUMP_CONFLICT: KeyCode = KeyCode::Char('f');

/// Open text search input (for n/N navigation)
pub const SEARCH_INPUT: KeyCode = KeyCode::Char('/');

/// Open revset input (for jj filtering)
pub const REVSET_INPUT: KeyCode = KeyCode::Char('r');

/// Next search result
pub const SEARCH_NEXT: KeyCode = KeyCode::Char('n');

/// Previous search result
pub const SEARCH_PREV: KeyCode = KeyCode::Char('N');

// =============================================================================
// Diff View keys
// =============================================================================

/// Next file in diff
pub const NEXT_FILE: KeyCode = KeyCode::Char(']');

/// Previous file in diff
pub const PREV_FILE: KeyCode = KeyCode::Char('[');

/// Half page down
pub const HALF_PAGE_DOWN: KeyCode = KeyCode::Char('d');

/// Half page up
pub const HALF_PAGE_UP: KeyCode = KeyCode::Char('u');

// =============================================================================
// Undo/Redo keys
// =============================================================================

/// Undo last operation
pub const UNDO: KeyCode = KeyCode::Char('u');

// Note: Redo is Ctrl+R, handled via KeyModifiers in input.rs

// =============================================================================
// View switching keys
// =============================================================================

/// Go to status view
pub const STATUS_VIEW: KeyCode = KeyCode::Char('s');

/// Open operation history view
pub const OPERATION_HISTORY: KeyCode = KeyCode::Char('o');

// =============================================================================
// Help text generation
// =============================================================================

/// Key binding entry for help display
pub struct KeyBindEntry {
    pub key: &'static str,
    pub description: &'static str,
}

/// Global key bindings for help display
pub const GLOBAL_KEYS: &[KeyBindEntry] = &[
    KeyBindEntry {
        key: "q",
        description: "Quit / Back",
    },
    KeyBindEntry {
        key: "?",
        description: "Help",
    },
    KeyBindEntry {
        key: "Tab",
        description: "Switch view",
    },
    KeyBindEntry {
        key: "Esc",
        description: "Back to previous",
    },
    KeyBindEntry {
        key: "Ctrl+l",
        description: "Refresh",
    },
];

/// Navigation key bindings for help display
pub const NAV_KEYS: &[KeyBindEntry] = &[
    KeyBindEntry {
        key: "j/k",
        description: "Move down/up",
    },
    KeyBindEntry {
        key: "g/G",
        description: "Go to top/bottom",
    },
];

/// Log view key bindings for help display
pub const LOG_KEYS: &[KeyBindEntry] = &[
    KeyBindEntry {
        key: "Enter",
        description: "Show diff",
    },
    KeyBindEntry {
        key: "d",
        description: "Describe (1-line quick edit; opens editor for multi-line)",
    },
    KeyBindEntry {
        key: "Ctrl+e",
        description: "Describe in external editor (full text)",
    },
    KeyBindEntry {
        key: "e",
        description: "Edit change",
    },
    KeyBindEntry {
        key: "c",
        description: "Create new change",
    },
    KeyBindEntry {
        key: "C",
        description: "New from selected (Log)",
    },
    KeyBindEntry {
        key: "/",
        description: "Search in list",
    },
    KeyBindEntry {
        key: "r",
        description: "Revset filter",
    },
    KeyBindEntry {
        key: "n/N",
        description: "Next/prev search",
    },
    KeyBindEntry {
        key: "s",
        description: "Status view",
    },
    KeyBindEntry {
        key: "o",
        description: "Operation history",
    },
    KeyBindEntry {
        key: "u",
        description: "Undo",
    },
    KeyBindEntry {
        key: "Ctrl+r",
        description: "Redo",
    },
    KeyBindEntry {
        key: "S",
        description: "Squash (select target)",
    },
    KeyBindEntry {
        key: "A",
        description: "Abandon change",
    },
    KeyBindEntry {
        key: "x",
        description: "Split change",
    },
    KeyBindEntry {
        key: "b",
        description: "Create bookmark",
    },
    KeyBindEntry {
        key: "D",
        description: "Delete bookmark",
    },
    KeyBindEntry {
        key: "R",
        description: "Rebase (r/s/A/B)",
    },
    KeyBindEntry {
        key: "B",
        description: "Absorb changes",
    },
    KeyBindEntry {
        key: "X",
        description: "Resolve conflicts",
    },
    KeyBindEntry {
        key: "F",
        description: "Git fetch",
    },
    KeyBindEntry {
        key: "P",
        description: "Git push",
    },
    KeyBindEntry {
        key: "T",
        description: "Track remote bookmarks",
    },
    KeyBindEntry {
        key: "'",
        description: "Jump to bookmark",
    },
    KeyBindEntry {
        key: "=",
        description: "Compare revisions",
    },
    KeyBindEntry {
        key: "M",
        description: "Bookmark view",
    },
    KeyBindEntry {
        key: "p",
        description: "Toggle preview pane",
    },
    KeyBindEntry {
        key: "]/[",
        description: "Move @ to next/prev",
    },
    KeyBindEntry {
        key: "V",
        description: "Toggle reversed order",
    },
];

/// Input mode key bindings (describe, search, revset, bookmark)
pub const INPUT_KEYS: &[KeyBindEntry] = &[
    KeyBindEntry {
        key: "Enter",
        description: "Submit input",
    },
    KeyBindEntry {
        key: "Esc",
        description: "Cancel input",
    },
    KeyBindEntry {
        key: "Backspace",
        description: "Delete character",
    },
];

/// Diff view key bindings for help display
pub const DIFF_KEYS: &[KeyBindEntry] = &[
    KeyBindEntry {
        key: "j/k",
        description: "Scroll down/up",
    },
    KeyBindEntry {
        key: "d/u",
        description: "Half page down/up",
    },
    KeyBindEntry {
        key: "g/G",
        description: "Go to top/bottom",
    },
    KeyBindEntry {
        key: "]/[",
        description: "Next/prev file",
    },
    KeyBindEntry {
        key: "a",
        description: "Show file blame",
    },
    KeyBindEntry {
        key: "q",
        description: "Back to log",
    },
];

// =============================================================================
// Status bar hints
// =============================================================================

/// Key hint for status bar display (colored badges)
#[derive(Clone, Copy)]
pub struct KeyHint {
    pub key: &'static str,
    pub label: &'static str,
    pub color: Color,
}

// Individual KeyHint constants (used by builder functions)
pub const HINT_HELP: KeyHint = KeyHint {
    key: "?",
    label: "Help",
    color: Color::Cyan,
};
pub const HINT_DESC: KeyHint = KeyHint {
    key: "d",
    label: "Desc",
    color: Color::Green,
};
pub const HINT_EDITOR: KeyHint = KeyHint {
    key: "^E",
    label: "Editor",
    color: Color::Green,
};
pub const HINT_EDIT: KeyHint = KeyHint {
    key: "e",
    label: "Edit",
    color: Color::Yellow,
};
pub const HINT_NEW: KeyHint = KeyHint {
    key: "c",
    label: "New",
    color: Color::Magenta,
};
pub const HINT_NEW_AT: KeyHint = KeyHint {
    key: "C",
    label: "New@",
    color: Color::Magenta,
};
pub const HINT_SQUASH: KeyHint = KeyHint {
    key: "S",
    label: "Squash",
    color: Color::Red,
};
pub const HINT_ABANDON: KeyHint = KeyHint {
    key: "A",
    label: "Abandon",
    color: Color::Red,
};
pub const HINT_SPLIT: KeyHint = KeyHint {
    key: "x",
    label: "Split",
    color: Color::Yellow,
};
pub const HINT_BOOKMARK: KeyHint = KeyHint {
    key: "b",
    label: "Bookmark",
    color: Color::Cyan,
};
pub const HINT_DEL_BKM: KeyHint = KeyHint {
    key: "D",
    label: "Del Bkm",
    color: Color::Red,
};
pub const HINT_REBASE: KeyHint = KeyHint {
    key: "R",
    label: "Rebase",
    color: Color::Yellow,
};
pub const HINT_ABSORB: KeyHint = KeyHint {
    key: "B",
    label: "Absorb",
    color: Color::Magenta,
};
pub const HINT_RESOLVE: KeyHint = KeyHint {
    key: "X",
    label: "Resolve",
    color: Color::Red,
};
pub const HINT_FETCH: KeyHint = KeyHint {
    key: "F",
    label: "Fetch",
    color: Color::Blue,
};
pub const HINT_PUSH: KeyHint = KeyHint {
    key: "P",
    label: "Push",
    color: Color::Blue,
};
pub const HINT_TRACK: KeyHint = KeyHint {
    key: "T",
    label: "Track",
    color: Color::Cyan,
};
pub const HINT_JUMP: KeyHint = KeyHint {
    key: "'",
    label: "Jump",
    color: Color::Green,
};
pub const HINT_COMPARE: KeyHint = KeyHint {
    key: "=",
    label: "Compare",
    color: Color::Yellow,
};
pub const HINT_OPS: KeyHint = KeyHint {
    key: "o",
    label: "Ops",
    color: Color::Blue,
};
pub const HINT_UNDO: KeyHint = KeyHint {
    key: "u",
    label: "Undo",
    color: Color::Green,
};
pub const HINT_REFRESH: KeyHint = KeyHint {
    key: "^L",
    label: "Refresh",
    color: Color::Blue,
};
pub const HINT_SWITCH: KeyHint = KeyHint {
    key: "Tab",
    label: "Switch",
    color: Color::Blue,
};
pub const HINT_QUIT: KeyHint = KeyHint {
    key: "q",
    label: "Quit",
    color: Color::Red,
};
pub const HINT_BACK: KeyHint = KeyHint {
    key: "q",
    label: "Back",
    color: Color::Red,
};
// Navigation/action hints for special modes
pub const HINT_NAV: KeyHint = KeyHint {
    key: "j/k",
    label: "Navigate",
    color: Color::Blue,
};
pub const HINT_SQUASH_CONFIRM: KeyHint = KeyHint {
    key: "Enter",
    label: "Squash",
    color: Color::Green,
};
pub const HINT_CANCEL: KeyHint = KeyHint {
    key: "Esc",
    label: "Cancel",
    color: Color::Red,
};
pub const HINT_SUBMIT: KeyHint = KeyHint {
    key: "Enter",
    label: "Confirm",
    color: Color::Green,
};
pub const HINT_CANCEL_ESC: KeyHint = KeyHint {
    key: "Esc",
    label: "Cancel",
    color: Color::Red,
};
// Dialog hints
pub const HINT_YES: KeyHint = KeyHint {
    key: "y/Enter",
    label: "Yes",
    color: Color::Green,
};
pub const HINT_NO: KeyHint = KeyHint {
    key: "n/Esc",
    label: "No",
    color: Color::Red,
};
pub const HINT_TOGGLE: KeyHint = KeyHint {
    key: "Space",
    label: "Toggle",
    color: Color::Yellow,
};
pub const HINT_CONFIRM: KeyHint = KeyHint {
    key: "Enter",
    label: "Confirm",
    color: Color::Green,
};
pub const HINT_DIALOG_CANCEL: KeyHint = KeyHint {
    key: "Esc",
    label: "Cancel",
    color: Color::Red,
};
pub const HINT_SELECT: KeyHint = KeyHint {
    key: "Enter",
    label: "Select",
    color: Color::Green,
};
// Resolve view hints
pub const HINT_RESOLVE_ENTER: KeyHint = KeyHint {
    key: "Enter",
    label: "Resolve",
    color: Color::Green,
};
pub const HINT_OURS: KeyHint = KeyHint {
    key: "o",
    label: "Ours",
    color: Color::Cyan,
};
pub const HINT_THEIRS: KeyHint = KeyHint {
    key: "t",
    label: "Theirs",
    color: Color::Cyan,
};
pub const HINT_DIFF: KeyHint = KeyHint {
    key: "d",
    label: "Diff",
    color: Color::Magenta,
};
// Bookmark view hints
pub const HINT_BOOKMARK_VIEW: KeyHint = KeyHint {
    key: "M",
    label: "Bookmarks",
    color: Color::Cyan,
};
pub const HINT_JUMP_ENTER: KeyHint = KeyHint {
    key: "Enter",
    label: "Jump",
    color: Color::Green,
};
pub const HINT_UNTRACK: KeyHint = KeyHint {
    key: "U",
    label: "Untrack",
    color: Color::Yellow,
};
pub const HINT_LOG_JUMP: KeyHint = KeyHint {
    key: "J",
    label: "Log Jump",
    color: Color::Yellow,
};
pub const HINT_PREVIEW: KeyHint = KeyHint {
    key: "p",
    label: "Preview",
    color: Color::Blue,
};
pub const HINT_REVERSE: KeyHint = KeyHint {
    key: "V",
    label: "Reverse",
    color: Color::Yellow,
};
pub const HINT_RENAME: KeyHint = KeyHint {
    key: "r",
    label: "Rename",
    color: Color::Yellow,
};
pub const HINT_FORGET: KeyHint = KeyHint {
    key: "f",
    label: "Forget",
    color: Color::Red,
};

// =============================================================================
// HintContext + DialogHintKind
// =============================================================================

/// Bookmark kind for context-dependent Bookmark View hints
#[derive(Clone, Copy)]
pub enum BookmarkKind {
    /// Local bookmark with change_id (jumpable)
    LocalJumpable,
    /// Local bookmark without change_id
    LocalNoChange,
    /// Tracked remote bookmark
    TrackedRemote,
    /// Untracked remote bookmark
    UntrackedRemote,
}

/// Context for dynamic hint selection
#[derive(Default)]
pub struct HintContext {
    /// Selected change has bookmarks
    pub has_bookmarks: bool,
    /// Selected change has conflicts
    pub has_conflicts: bool,
    /// Selected change is the working copy (@)
    pub is_working_copy: bool,
    /// Active dialog kind (overrides view hints)
    pub dialog: Option<DialogHintKind>,
    /// Selected bookmark kind (Bookmark View only)
    pub selected_bookmark_kind: Option<BookmarkKind>,
}

/// Dialog kind for hint selection
pub enum DialogHintKind {
    /// y/n confirmation
    Confirm,
    /// Multi-select with Space toggle
    Select,
    /// Single-select (Enter immediately confirms)
    SingleSelect,
}

// =============================================================================
// Unified dispatch
// =============================================================================

/// Get the appropriate hints for the current context.
///
/// Priority: dialog > view × input_mode.
/// Diff, Blame, and Help views use dedicated rendering and should not call this.
pub fn current_hints(view: View, input_mode: InputMode, ctx: &HintContext) -> Vec<KeyHint> {
    // Priority 1: dialog overrides everything
    if let Some(ref kind) = ctx.dialog {
        return dialog_hints(kind);
    }
    // Priority 2: view × input_mode
    match view {
        View::Log => log_hints(input_mode, ctx),
        View::Resolve => resolve_hints(ctx),
        View::Bookmark => bookmark_view_hints(ctx),
        View::Status => STATUS_VIEW_HINTS.to_vec(),
        View::Operation => OPERATION_VIEW_HINTS.to_vec(),
        // Diff, Blame use prefix-based rendering; Help has no status bar.
        // Return empty as a safety fallback.
        _ => vec![],
    }
}

fn dialog_hints(kind: &DialogHintKind) -> Vec<KeyHint> {
    match kind {
        DialogHintKind::Confirm => vec![HINT_YES, HINT_NO],
        DialogHintKind::Select => vec![HINT_NAV, HINT_TOGGLE, HINT_CONFIRM, HINT_DIALOG_CANCEL],
        DialogHintKind::SingleSelect => vec![HINT_NAV, HINT_SELECT, HINT_DIALOG_CANCEL],
    }
}

fn log_hints(input_mode: InputMode, ctx: &HintContext) -> Vec<KeyHint> {
    match input_mode {
        InputMode::Normal => log_normal_hints(ctx),
        InputMode::SquashSelect => vec![HINT_NAV, HINT_SQUASH_CONFIRM, HINT_CANCEL],
        InputMode::RebaseModeSelect => REBASE_MODE_SELECT_HINTS.to_vec(),
        InputMode::RebaseSelect => REBASE_SELECT_HINTS.to_vec(),
        InputMode::CompareSelect => COMPARE_SELECT_HINTS.to_vec(),
        InputMode::SearchInput
        | InputMode::RevsetInput
        | InputMode::DescribeInput
        | InputMode::BookmarkInput => vec![HINT_SUBMIT, HINT_CANCEL_ESC],
    }
}

fn log_normal_hints(ctx: &HintContext) -> Vec<KeyHint> {
    let mut h = vec![
        HINT_HELP,
        HINT_DESC,
        HINT_EDITOR,
        HINT_EDIT,
        HINT_NEW,
        HINT_NEW_AT,
        HINT_SQUASH,
        HINT_ABANDON,
        HINT_SPLIT,
        HINT_BOOKMARK,
        HINT_REBASE,
        HINT_ABSORB,
    ];
    if ctx.has_conflicts {
        h.push(HINT_RESOLVE);
    }
    if ctx.has_bookmarks {
        h.push(HINT_DEL_BKM);
        h.push(HINT_PUSH);
    }
    h.extend([
        HINT_FETCH,
        HINT_TRACK,
        HINT_JUMP,
        HINT_COMPARE,
        HINT_BOOKMARK_VIEW,
        HINT_PREVIEW,
        HINT_REVERSE,
        HINT_OPS,
        HINT_UNDO,
        HINT_REFRESH,
        HINT_SWITCH,
        HINT_QUIT,
    ]);
    h
}

fn resolve_hints(ctx: &HintContext) -> Vec<KeyHint> {
    let mut h = Vec::new();
    if ctx.is_working_copy {
        h.push(HINT_RESOLVE_ENTER);
    }
    h.extend([HINT_OURS, HINT_THEIRS, HINT_DIFF, HINT_BACK]);
    h
}

fn bookmark_view_hints(ctx: &HintContext) -> Vec<KeyHint> {
    let mut h = Vec::new();
    match ctx.selected_bookmark_kind {
        Some(BookmarkKind::LocalJumpable) => {
            h.push(HINT_JUMP_ENTER);
            h.push(HINT_DEL_BKM);
            h.push(HINT_RENAME);
            h.push(HINT_FORGET);
        }
        Some(BookmarkKind::LocalNoChange) => {
            h.push(HINT_DEL_BKM);
            h.push(HINT_RENAME);
            h.push(HINT_FORGET);
        }
        Some(BookmarkKind::TrackedRemote) => {
            h.push(HINT_UNTRACK);
        }
        Some(BookmarkKind::UntrackedRemote) => {
            h.push(HINT_TRACK);
        }
        None => {}
    }
    h.extend([HINT_UNDO, HINT_REFRESH, HINT_BACK]);
    h
}

/// CompareSelect mode status bar hints
pub const COMPARE_SELECT_HINTS: &[KeyHint] = &[
    KeyHint {
        key: "j/k",
        label: "Navigate",
        color: Color::Blue,
    },
    KeyHint {
        key: "Enter",
        label: "Compare",
        color: Color::Green,
    },
    KeyHint {
        key: "Esc",
        label: "Cancel",
        color: Color::Red,
    },
];

/// RebaseModeSelect mode status bar hints
pub const REBASE_MODE_SELECT_HINTS: &[KeyHint] = &[
    KeyHint {
        key: "r",
        label: "Revision",
        color: Color::Yellow,
    },
    KeyHint {
        key: "s",
        label: "Source",
        color: Color::Magenta,
    },
    KeyHint {
        key: "A",
        label: "After",
        color: Color::Cyan,
    },
    KeyHint {
        key: "B",
        label: "Before",
        color: Color::Green,
    },
    KeyHint {
        key: "Esc",
        label: "Cancel",
        color: Color::Red,
    },
];

/// RebaseSelect mode status bar hints
pub const REBASE_SELECT_HINTS: &[KeyHint] = &[
    KeyHint {
        key: "j/k",
        label: "Navigate",
        color: Color::Blue,
    },
    KeyHint {
        key: "Enter",
        label: "Rebase",
        color: Color::Green,
    },
    KeyHint {
        key: "Esc",
        label: "Cancel",
        color: Color::Red,
    },
];

/// Diff view status bar hints
pub const DIFF_VIEW_HINTS: &[KeyHint] = &[
    KeyHint {
        key: "j/k",
        label: "Scroll",
        color: Color::Cyan,
    },
    KeyHint {
        key: "]/[",
        label: "File",
        color: Color::Magenta,
    },
    KeyHint {
        key: "a",
        label: "Blame",
        color: Color::Magenta,
    },
    KeyHint {
        key: "^L",
        label: "Refresh",
        color: Color::Blue,
    },
    KeyHint {
        key: "q",
        label: "Back",
        color: Color::Red,
    },
];

/// Status view status bar hints
pub const STATUS_VIEW_HINTS: &[KeyHint] = &[
    KeyHint {
        key: "?",
        label: "Help",
        color: Color::Cyan,
    },
    KeyHint {
        key: "Enter",
        label: "Diff",
        color: Color::Green,
    },
    KeyHint {
        key: "a",
        label: "Blame",
        color: Color::Magenta,
    },
    KeyHint {
        key: "C",
        label: "Commit",
        color: Color::Yellow,
    },
    KeyHint {
        key: "f",
        label: "Conflict",
        color: Color::Red,
    },
    KeyHint {
        key: "^L",
        label: "Refresh",
        color: Color::Blue,
    },
    KeyHint {
        key: "Tab",
        label: "Switch",
        color: Color::Blue,
    },
    KeyHint {
        key: "q",
        label: "Quit",
        color: Color::Red,
    },
];

/// Status view key bindings for help display
pub const STATUS_KEYS: &[KeyBindEntry] = &[
    KeyBindEntry {
        key: "j/k",
        description: "Move down/up",
    },
    KeyBindEntry {
        key: "g/G",
        description: "Go to top/bottom",
    },
    KeyBindEntry {
        key: "Enter",
        description: "Show file diff",
    },
    KeyBindEntry {
        key: "a",
        description: "Show file blame",
    },
    KeyBindEntry {
        key: "C",
        description: "Commit changes",
    },
    KeyBindEntry {
        key: "f",
        description: "Jump to conflict",
    },
    KeyBindEntry {
        key: "Tab",
        description: "Switch to log",
    },
    KeyBindEntry {
        key: "q",
        description: "Quit",
    },
];

/// Operation history view key bindings for help display
pub const OPERATION_KEYS: &[KeyBindEntry] = &[
    KeyBindEntry {
        key: "j/k",
        description: "Move down/up",
    },
    KeyBindEntry {
        key: "g/G",
        description: "Go to top/bottom",
    },
    KeyBindEntry {
        key: "Enter",
        description: "Restore operation",
    },
    KeyBindEntry {
        key: "q",
        description: "Back to log",
    },
];

/// Bookmark view key bindings for help display
pub const BOOKMARK_KEYS: &[KeyBindEntry] = &[
    KeyBindEntry {
        key: "j/k",
        description: "Move down/up",
    },
    KeyBindEntry {
        key: "g/G",
        description: "Go to top/bottom",
    },
    KeyBindEntry {
        key: "Enter",
        description: "Jump to bookmark in log",
    },
    KeyBindEntry {
        key: "T",
        description: "Track remote bookmark",
    },
    KeyBindEntry {
        key: "U",
        description: "Untrack remote bookmark",
    },
    KeyBindEntry {
        key: "D",
        description: "Delete local bookmark",
    },
    KeyBindEntry {
        key: "r",
        description: "Rename bookmark",
    },
    KeyBindEntry {
        key: "f",
        description: "Forget bookmark (remove tracking)",
    },
    KeyBindEntry {
        key: "u",
        description: "Undo",
    },
    KeyBindEntry {
        key: "q",
        description: "Back to log",
    },
];

/// Blame view key bindings for help display
#[allow(dead_code)] // planned for Help View integration
pub const BLAME_KEYS: &[KeyBindEntry] = &[
    KeyBindEntry {
        key: "j/k",
        description: "Move down/up",
    },
    KeyBindEntry {
        key: "g/G",
        description: "Go to top/bottom",
    },
    KeyBindEntry {
        key: "Enter",
        description: "Show diff",
    },
    KeyBindEntry {
        key: "J",
        description: "Jump to change in log",
    },
    KeyBindEntry {
        key: "q",
        description: "Back",
    },
];

/// Resolve view key bindings for help display
#[allow(dead_code)] // planned for Help View integration
pub const RESOLVE_KEYS: &[KeyBindEntry] = &[
    KeyBindEntry {
        key: "j/k",
        description: "Move down/up",
    },
    KeyBindEntry {
        key: "Enter",
        description: "Resolve (external tool, @ only)",
    },
    KeyBindEntry {
        key: "o",
        description: "Resolve with :ours",
    },
    KeyBindEntry {
        key: "t",
        description: "Resolve with :theirs",
    },
    KeyBindEntry {
        key: "d",
        description: "Show diff",
    },
    KeyBindEntry {
        key: "q",
        description: "Back to log",
    },
];

/// Operation history view status bar hints
pub const OPERATION_VIEW_HINTS: &[KeyHint] = &[
    KeyHint {
        key: "j/k",
        label: "Move",
        color: Color::Cyan,
    },
    KeyHint {
        key: "Enter",
        label: "Restore",
        color: Color::Green,
    },
    KeyHint {
        key: "^L",
        label: "Refresh",
        color: Color::Blue,
    },
    KeyHint {
        key: "q",
        label: "Back",
        color: Color::Red,
    },
];

/// Blame view status bar hints
pub const BLAME_VIEW_HINTS: &[KeyHint] = &[
    KeyHint {
        key: "j/k",
        label: "Move",
        color: Color::Cyan,
    },
    KeyHint {
        key: "Enter",
        label: "Diff",
        color: Color::Green,
    },
    KeyHint {
        key: "J",
        label: "Log Jump",
        color: Color::Yellow,
    },
    KeyHint {
        key: "^L",
        label: "Refresh",
        color: Color::Blue,
    },
    KeyHint {
        key: "q",
        label: "Back",
        color: Color::Red,
    },
];

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Log Normal: context-dependent hints ---

    #[test]
    fn log_normal_with_bookmarks_includes_push_and_del() {
        let ctx = HintContext {
            has_bookmarks: true,
            ..HintContext::default()
        };
        let hints = current_hints(View::Log, InputMode::Normal, &ctx);
        assert!(hints.iter().any(|h| h.key == "P"), "Push hint missing");
        assert!(hints.iter().any(|h| h.key == "D"), "Del Bkm hint missing");
    }

    #[test]
    fn log_normal_without_bookmarks_excludes_push_and_del() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Log, InputMode::Normal, &ctx);
        assert!(
            !hints.iter().any(|h| h.key == "P"),
            "Push hint should not appear"
        );
        assert!(
            !hints.iter().any(|h| h.key == "D"),
            "Del Bkm hint should not appear"
        );
    }

    #[test]
    fn log_normal_with_conflicts_includes_resolve() {
        let ctx = HintContext {
            has_conflicts: true,
            ..HintContext::default()
        };
        let hints = current_hints(View::Log, InputMode::Normal, &ctx);
        assert!(hints.iter().any(|h| h.key == "X"), "Resolve hint missing");
    }

    #[test]
    fn log_normal_without_conflicts_excludes_resolve() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Log, InputMode::Normal, &ctx);
        assert!(
            !hints.iter().any(|h| h.key == "X"),
            "Resolve hint should not appear"
        );
    }

    #[test]
    fn log_normal_always_includes_core_hints() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Log, InputMode::Normal, &ctx);
        assert!(hints.iter().any(|h| h.key == "?"), "Help hint missing");
        assert!(hints.iter().any(|h| h.key == "d"), "Desc hint missing");
        assert!(hints.iter().any(|h| h.key == "q"), "Quit hint missing");
        assert!(hints.iter().any(|h| h.key == "e"), "Edit hint missing");
        assert!(hints.iter().any(|h| h.key == "c"), "New hint missing");
        assert!(hints.iter().any(|h| h.key == "S"), "Squash hint missing");
        assert!(hints.iter().any(|h| h.key == "F"), "Fetch hint missing");
    }

    // --- Log View: InputMode-specific hint counts ---

    #[test]
    fn log_squash_select_hints() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Log, InputMode::SquashSelect, &ctx);
        assert_eq!(hints.len(), 3);
        assert!(hints.iter().any(|h| h.label == "Navigate"));
        assert!(hints.iter().any(|h| h.label == "Squash"));
        assert!(hints.iter().any(|h| h.label == "Cancel"));
    }

    #[test]
    fn log_search_input_hints() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Log, InputMode::SearchInput, &ctx);
        assert_eq!(hints.len(), 2);
    }

    #[test]
    fn log_revset_input_hints() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Log, InputMode::RevsetInput, &ctx);
        assert_eq!(hints.len(), 2);
    }

    #[test]
    fn log_describe_input_hints() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Log, InputMode::DescribeInput, &ctx);
        assert_eq!(hints.len(), 2);
    }

    #[test]
    fn log_bookmark_input_hints() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Log, InputMode::BookmarkInput, &ctx);
        assert_eq!(hints.len(), 2);
    }

    // --- Dialog hints ---

    #[test]
    fn dialog_confirm_hints() {
        let ctx = HintContext {
            dialog: Some(DialogHintKind::Confirm),
            ..HintContext::default()
        };
        let hints = current_hints(View::Log, InputMode::Normal, &ctx);
        assert_eq!(hints.len(), 2);
        assert!(hints.iter().any(|h| h.label == "Yes"));
        assert!(hints.iter().any(|h| h.label == "No"));
    }

    #[test]
    fn dialog_select_hints() {
        let ctx = HintContext {
            dialog: Some(DialogHintKind::Select),
            ..HintContext::default()
        };
        let hints = current_hints(View::Log, InputMode::Normal, &ctx);
        assert_eq!(hints.len(), 4);
        assert!(hints.iter().any(|h| h.label == "Navigate"));
        assert!(hints.iter().any(|h| h.label == "Toggle"));
        assert!(hints.iter().any(|h| h.label == "Confirm"));
        assert!(hints.iter().any(|h| h.label == "Cancel"));
    }

    #[test]
    fn dialog_single_select_hints() {
        let ctx = HintContext {
            dialog: Some(DialogHintKind::SingleSelect),
            ..HintContext::default()
        };
        let hints = current_hints(View::Log, InputMode::Normal, &ctx);
        assert_eq!(hints.len(), 3);
        assert!(hints.iter().any(|h| h.label == "Navigate"));
        assert!(hints.iter().any(|h| h.label == "Select"));
        assert!(hints.iter().any(|h| h.label == "Cancel"));
    }

    // --- Dialog overrides ---

    #[test]
    fn dialog_overrides_log_normal() {
        let ctx = HintContext {
            has_bookmarks: true,
            dialog: Some(DialogHintKind::Confirm),
            ..HintContext::default()
        };
        let hints = current_hints(View::Log, InputMode::Normal, &ctx);
        assert_eq!(hints.len(), 2);
        assert!(hints.iter().any(|h| h.label == "Yes"));
        assert!(
            !hints.iter().any(|h| h.key == "P"),
            "Log hints should be suppressed"
        );
    }

    #[test]
    fn dialog_overrides_squash_select() {
        let ctx = HintContext {
            dialog: Some(DialogHintKind::Select),
            ..HintContext::default()
        };
        let hints = current_hints(View::Log, InputMode::SquashSelect, &ctx);
        assert_eq!(hints.len(), 4);
        assert!(hints.iter().any(|h| h.label == "Toggle"));
    }

    #[test]
    fn dialog_overrides_status_view() {
        let ctx = HintContext {
            dialog: Some(DialogHintKind::Confirm),
            ..HintContext::default()
        };
        let hints = current_hints(View::Status, InputMode::Normal, &ctx);
        assert_eq!(hints.len(), 2);
        assert!(hints.iter().any(|h| h.label == "Yes"));
    }

    #[test]
    fn no_dialog_returns_view_hints() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Log, InputMode::Normal, &ctx);
        assert!(hints.len() > 10, "Should return full Log Normal hints");
    }

    // --- Resolve view ---

    #[test]
    fn resolve_working_copy_includes_enter() {
        let ctx = HintContext {
            is_working_copy: true,
            ..HintContext::default()
        };
        let hints = current_hints(View::Resolve, InputMode::Normal, &ctx);
        assert!(
            hints
                .iter()
                .any(|h| h.key == "Enter" && h.label == "Resolve"),
            "Resolve Enter hint missing for working copy"
        );
    }

    #[test]
    fn resolve_non_working_copy_excludes_enter() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Resolve, InputMode::Normal, &ctx);
        assert!(
            !hints.iter().any(|h| h.key == "Enter"),
            "Enter hint should not appear for non-working-copy"
        );
    }

    #[test]
    fn resolve_always_includes_ours_theirs_diff_back() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Resolve, InputMode::Normal, &ctx);
        assert!(hints.iter().any(|h| h.key == "o"), "Ours hint missing");
        assert!(hints.iter().any(|h| h.key == "t"), "Theirs hint missing");
        assert!(hints.iter().any(|h| h.key == "d"), "Diff hint missing");
        assert!(hints.iter().any(|h| h.key == "q"), "Back hint missing");
    }

    // --- Safety fallback for Diff/Help ---

    #[test]
    fn diff_view_returns_empty() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Diff, InputMode::Normal, &ctx);
        assert!(hints.is_empty());
    }

    #[test]
    fn help_view_returns_empty() {
        let ctx = HintContext::default();
        let hints = current_hints(View::Help, InputMode::Normal, &ctx);
        assert!(hints.is_empty());
    }
}
