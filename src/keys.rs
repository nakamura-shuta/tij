//! Keybinding definitions for Tij
//!
//! All keybindings are defined here for easy modification and future config file support.

use crossterm::event::KeyCode;

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

/// Move cursor up
pub const MOVE_UP: KeyCode = KeyCode::Char('k');

/// Move cursor down
pub const MOVE_DOWN: KeyCode = KeyCode::Char('j');

/// Go to top
pub const GO_TOP: KeyCode = KeyCode::Char('g');

/// Go to bottom
pub const GO_BOTTOM: KeyCode = KeyCode::Char('G');

// =============================================================================
// Log View keys
// =============================================================================

/// Open diff view for selected commit
pub const OPEN_DIFF: KeyCode = KeyCode::Enter;

/// Open revset input
pub const REVSET_INPUT: KeyCode = KeyCode::Char('/');

/// Next search result
pub const SEARCH_NEXT: KeyCode = KeyCode::Char('n');

/// Previous search result
pub const SEARCH_PREV: KeyCode = KeyCode::Char('N');

// =============================================================================
// View switching keys
// =============================================================================

/// Go to status view
pub const STATUS_VIEW: KeyCode = KeyCode::Char('s');

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
        key: "/",
        description: "Revset input",
    },
    KeyBindEntry {
        key: "n/N",
        description: "Next/prev result",
    },
    KeyBindEntry {
        key: "s",
        description: "Status view",
    },
];
