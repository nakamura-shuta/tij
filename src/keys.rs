//! Keybinding definitions for Tij
//!
//! All keybindings are defined here for easy modification and future config file support.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;

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

/// Squash change into parent (Log View, uppercase)
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

/// Show file annotation/blame (Status/Diff View)
pub const ANNOTATE: KeyCode = KeyCode::Char('a');

/// Open resolve list view for conflicts (Log View, uppercase)
pub const RESOLVE_LIST: KeyCode = KeyCode::Char('X');

/// Fetch from remote (Log View, uppercase for remote ops)
pub const FETCH: KeyCode = KeyCode::Char('F');

/// Push to remote (Log View, uppercase for remote ops)
pub const PUSH: KeyCode = KeyCode::Char('P');

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
        description: "Edit description",
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
        description: "Squash into parent",
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
        description: "Rebase change",
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

/// Log view status bar hints
pub const LOG_VIEW_HINTS: &[KeyHint] = &[
    KeyHint {
        key: "?",
        label: "Help",
        color: Color::Cyan,
    },
    KeyHint {
        key: "d",
        label: "Describe",
        color: Color::Green,
    },
    KeyHint {
        key: "e",
        label: "Edit",
        color: Color::Yellow,
    },
    KeyHint {
        key: "c",
        label: "New",
        color: Color::Magenta,
    },
    KeyHint {
        key: "S",
        label: "Squash",
        color: Color::Red,
    },
    KeyHint {
        key: "A",
        label: "Abandon",
        color: Color::Red,
    },
    KeyHint {
        key: "x",
        label: "Split",
        color: Color::Yellow,
    },
    KeyHint {
        key: "b",
        label: "Bookmark",
        color: Color::Cyan,
    },
    KeyHint {
        key: "D",
        label: "Del Bkm",
        color: Color::Red,
    },
    KeyHint {
        key: "R",
        label: "Rebase",
        color: Color::Yellow,
    },
    KeyHint {
        key: "B",
        label: "Absorb",
        color: Color::Magenta,
    },
    KeyHint {
        key: "X",
        label: "Resolve",
        color: Color::Red,
    },
    KeyHint {
        key: "F",
        label: "Fetch",
        color: Color::Blue,
    },
    KeyHint {
        key: "P",
        label: "Push",
        color: Color::Blue,
    },
    KeyHint {
        key: "o",
        label: "Ops",
        color: Color::Blue,
    },
    KeyHint {
        key: "u",
        label: "Undo",
        color: Color::Green,
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
