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

use crate::jj::PushBulkMode;
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
    /// Jump to bookmark (Select dialog, single_select)
    BookmarkJump,
    /// Forget bookmark (Confirm dialog)
    BookmarkForget,
    /// Git fetch remote selection (Select dialog, single_select)
    GitFetch,
    /// Git fetch specific branch (Select dialog, single_select)
    GitFetchBranch,
    /// Git push by change ID (creates auto bookmark)
    GitPushChange {
        /// Change ID to push
        change_id: String,
    },
    /// Remote selection for push (Select dialog, single_select)
    GitPushRemoteSelect,
    /// Push mode selection when no bookmarks on selected change (Single Select)
    /// User chooses between --change, --all, --tracked, --deleted
    GitPushModeSelect { change_id: String },
    /// Bulk push confirmation after dry-run preview (Confirm dialog)
    GitPushBulkConfirm {
        mode: PushBulkMode,
        remote: Option<String>,
    },
    /// Bookmark move to @ confirmation
    BookmarkMoveToWc { name: String },
    /// Bookmark move with --allow-backwards confirmation
    BookmarkMoveBackwards { name: String },
    /// Restore a single file (Confirm dialog)
    RestoreFile { file_path: String },
    /// Restore all files (Confirm dialog)
    RestoreAll,
    /// Revert a change (Confirm dialog, creates reverse-diff commit)
    Revert { change_id: String },
    /// Git push by revision (all bookmarks on a change via --revisions)
    GitPushRevisions {
        change_id: String,
        /// Bookmarks for fallback if --revisions is unsupported
        bookmarks: Vec<String>,
    },
    /// Mode selection for multi-bookmark push (Single Select)
    /// User chooses between --revisions (all) or individual bookmark selection
    GitPushMultiBookmarkMode {
        change_id: String,
        bookmarks: Vec<String>,
    },
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
        /// Single select mode: Enter immediately confirms current item
        single_select: bool,
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

    /// Create a new Select dialog (multi-select with checkboxes)
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
                single_select: false,
            },
            cursor: 0,
            callback_id,
        }
    }

    /// Create a new single-select dialog (Enter immediately confirms current item)
    pub fn select_single(
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
                single_select: true,
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
                single_select,
            } => self.render_select(
                frame,
                area,
                title,
                message,
                items,
                detail.as_deref(),
                *single_select,
            ),
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
