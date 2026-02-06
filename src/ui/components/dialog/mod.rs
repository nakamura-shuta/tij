//! Dialog components for confirmation and selection
//!
//! Provides reusable dialog components:
//! - Confirm dialog: Yes/No confirmation
//! - Select dialog: Checkbox selection for multiple items

mod confirm;
mod select;
#[cfg(test)]
mod tests;

use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};

use crate::keys;

/// Callback identifier for dialog results
///
/// Note: `Copy` is not implemented because some variants contain `String` data.
/// Use `clone()` when extracting from `active_dialog`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogCallback {
    /// Bookmark deletion (Select dialog)
    DeleteBookmarks,
    /// Bookmark move confirmation (Confirm dialog)
    MoveBookmark {
        /// Bookmark name to move
        name: String,
        /// Target change ID
        change_id: String,
    },
    /// Operation restore (future use)
    #[allow(dead_code)]
    OpRestore,
    /// Git push confirmation
    GitPush,
    /// Track remote bookmarks (Select dialog)
    Track,
}

/// Selection item for Select dialog
#[derive(Debug, Clone)]
pub struct SelectItem {
    /// Display label
    pub label: String,
    /// Internal value (returned on confirm)
    pub value: String,
    /// Whether this item is selected
    pub selected: bool,
}

/// Dialog kind and content
#[derive(Debug, Clone)]
pub enum DialogKind {
    /// Simple Yes/No confirmation
    Confirm {
        title: String,
        message: String,
        /// Optional detail text (warning, etc.)
        detail: Option<String>,
    },
    /// Checkbox selection (multiple items)
    Select {
        title: String,
        message: String,
        items: Vec<SelectItem>,
        /// Optional detail text (warning, etc.)
        detail: Option<String>,
    },
}

/// Dialog result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogResult {
    /// Confirmed with selected values (empty for Confirm dialog)
    Confirmed(Vec<String>),
    /// Cancelled
    Cancelled,
}

/// Dialog state
#[derive(Debug, Clone)]
pub struct Dialog {
    /// Dialog kind and content
    pub kind: DialogKind,
    /// Cursor position (for Select dialog)
    pub cursor: usize,
    /// Callback identifier
    pub callback_id: DialogCallback,
}

impl Dialog {
    /// Create a new Confirm dialog
    pub fn confirm(
        title: impl Into<String>,
        message: impl Into<String>,
        detail: Option<String>,
        callback_id: DialogCallback,
    ) -> Self {
        Self {
            kind: DialogKind::Confirm {
                title: title.into(),
                message: message.into(),
                detail,
            },
            cursor: 0,
            callback_id,
        }
    }

    /// Create a new Select dialog
    pub fn select(
        title: impl Into<String>,
        message: impl Into<String>,
        items: Vec<SelectItem>,
        detail: Option<String>,
        callback_id: DialogCallback,
    ) -> Self {
        Self {
            kind: DialogKind::Select {
                title: title.into(),
                message: message.into(),
                items,
                detail,
            },
            cursor: 0,
            callback_id,
        }
    }

    /// Handle key input, returns Some(result) when dialog should close
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<DialogResult> {
        match &self.kind {
            DialogKind::Confirm { .. } => self.handle_confirm_key(key),
            DialogKind::Select { .. } => self.handle_select_key(key),
        }
    }

    /// Render the dialog centered on screen
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        match &self.kind {
            DialogKind::Confirm {
                title,
                message,
                detail,
            } => self.render_confirm(frame, area, title, message, detail.as_deref()),
            DialogKind::Select {
                title,
                message,
                items,
                detail,
            } => self.render_select(frame, area, title, message, items, detail.as_deref()),
        }
    }
}

/// Calculate a centered rectangle within the given area
pub(super) fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical_margin = area.height.saturating_sub(height) / 2;
    let horizontal_margin = area.width.saturating_sub(width) / 2;

    let vertical_layout = Layout::vertical([
        Constraint::Length(vertical_margin),
        Constraint::Length(height),
        Constraint::Length(vertical_margin),
    ])
    .split(area);

    let horizontal_layout = Layout::horizontal([
        Constraint::Length(horizontal_margin),
        Constraint::Length(width),
        Constraint::Length(horizontal_margin),
    ])
    .split(vertical_layout[1]);

    horizontal_layout[1]
}
