//! Command palette: searchable list of low-frequency Log View commands.
//!
//! Each command carries a [`PaletteDispatch`] describing how it is invoked
//! (Phase 48-C):
//!
//! - Low-frequency commands (compare, interdiff, duplicate, …) dispatch by an
//!   action-id ([`PaletteDispatch::Command`] → `LogView::command_action`).
//!   These commands have NO single-key binding any more — the palette is the
//!   only way to reach them — so dispatch must not depend on a key.
//! - View-opener commands (evolog, command-history, tag-view, workspace-view)
//!   still keep their single keys, so they dispatch by [`PaletteDispatch::Key`]
//!   → `LogView::handle_key`, reusing the existing key logic verbatim.

use crossterm::event::KeyCode;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::keys;
use crate::ui::views::LogCommand;

/// How a palette entry is dispatched (Phase 48-C).
#[derive(Debug, Clone, Copy)]
pub enum PaletteDispatch {
    /// Low-frequency command with no single key — dispatched via
    /// `LogView::command_action`.
    Command(LogCommand),
    /// View-opener that keeps its single key — dispatched via
    /// `LogView::handle_key`.
    Key(KeyCode),
}

/// A palette entry: display name, description, and how it dispatches.
#[derive(Debug, Clone, Copy)]
pub struct PaletteCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub dispatch: PaletteDispatch,
}

/// All commands available from the palette (Log View, low-frequency ops).
pub fn palette_commands() -> &'static [PaletteCommand] {
    &[
        PaletteCommand {
            name: "simplify-parents",
            description: "Remove redundant parent edges",
            dispatch: PaletteDispatch::Command(LogCommand::SimplifyParents),
        },
        PaletteCommand {
            name: "parallelize",
            description: "Convert linear chain to siblings",
            dispatch: PaletteDispatch::Command(LogCommand::Parallelize),
        },
        PaletteCommand {
            name: "fix",
            description: "Apply configured code formatters",
            dispatch: PaletteDispatch::Command(LogCommand::Fix),
        },
        PaletteCommand {
            name: "arrange",
            description: "Interactively arrange the commit graph",
            dispatch: PaletteDispatch::Command(LogCommand::Arrange),
        },
        PaletteCommand {
            name: "metaedit",
            description: "Edit author, change-id, timestamp",
            dispatch: PaletteDispatch::Command(LogCommand::Metaedit),
        },
        PaletteCommand {
            name: "bisect",
            description: "Find bad revision (binary search)",
            dispatch: PaletteDispatch::Command(LogCommand::Bisect),
        },
        PaletteCommand {
            name: "interdiff",
            description: "Patch diff between two revisions",
            dispatch: PaletteDispatch::Command(LogCommand::Interdiff),
        },
        PaletteCommand {
            name: "duplicate",
            description: "Duplicate change",
            dispatch: PaletteDispatch::Command(LogCommand::Duplicate),
        },
        PaletteCommand {
            name: "diffedit",
            description: "Edit diff in external editor",
            dispatch: PaletteDispatch::Command(LogCommand::DiffEdit),
        },
        // View openers (Key dispatch — single keys retained)
        PaletteCommand {
            name: "evolog",
            description: "Evolution log (change history)",
            dispatch: PaletteDispatch::Key(keys::EVOLOG),
        },
        PaletteCommand {
            name: "revert",
            description: "Create reverse-diff commit",
            dispatch: PaletteDispatch::Command(LogCommand::Revert),
        },
        PaletteCommand {
            name: "command-history",
            description: "Show executed jj commands",
            dispatch: PaletteDispatch::Key(keys::COMMAND_HISTORY),
        },
        PaletteCommand {
            name: "compare",
            description: "Compare two revisions",
            dispatch: PaletteDispatch::Command(LogCommand::Compare),
        },
        PaletteCommand {
            name: "show-stack-diff",
            description: "Show all diffs in stack (selected to @)",
            dispatch: PaletteDispatch::Command(LogCommand::ShowStackDiff),
        },
        PaletteCommand {
            name: "tag-view",
            description: "Open Tag View",
            dispatch: PaletteDispatch::Key(keys::TAG_VIEW),
        },
        PaletteCommand {
            name: "workspace-view",
            description: "Open Workspace View",
            dispatch: PaletteDispatch::Key(keys::WORKSPACE_VIEW),
        },
    ]
}

/// Filter commands by case-insensitive substring match on name or description.
/// An empty query returns all commands.
pub fn filter_commands<'a>(commands: &'a [PaletteCommand], query: &str) -> Vec<&'a PaletteCommand> {
    if query.is_empty() {
        return commands.iter().collect();
    }
    let q = query.to_lowercase();
    commands
        .iter()
        .filter(|c| c.name.to_lowercase().contains(&q) || c.description.to_lowercase().contains(&q))
        .collect()
}

/// Render the command palette overlay: centered box with an input line and the
/// filtered command list (highlighting `selected`).
pub fn render_palette(frame: &mut Frame, input: &str, selected: usize) {
    let area = centered_rect(frame.area(), 60, 60);
    frame.render_widget(Clear, area);

    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);

    // Input line
    frame.render_widget(
        Paragraph::new(format!(": {input}")).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Command Palette "),
        ),
        chunks[0],
    );

    // Filtered command list. Use a ListState so the selected row stays within
    // the visible window (ratatui scrolls the list to keep `selected` in view).
    let filtered = filter_commands(palette_commands(), input);
    if filtered.is_empty() {
        frame.render_widget(
            List::new(vec![ListItem::new("  (no match)")])
                .block(Block::default().borders(Borders::ALL)),
            chunks[1],
        );
        return;
    }

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|c| ListItem::new(format!("{:<18} {}", c.name, c.description)))
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    // Clamp in case the filter shrank the list below `selected`.
    state.select(Some(selected.min(filtered.len() - 1)));
    frame.render_stateful_widget(list, chunks[1], &mut state);
}

/// Centered rect as a percentage of `area`.
fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(v[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_non_empty_and_has_metaedit() {
        let cmds = palette_commands();
        assert!(!cmds.is_empty());
        assert!(cmds.iter().any(|c| c.name == "metaedit"));
    }

    #[test]
    fn registry_has_show_stack_diff() {
        let cmds = palette_commands();
        let cmd = cmds
            .iter()
            .find(|c| c.name == "show-stack-diff")
            .expect("show-stack-diff registered");
        assert!(matches!(
            cmd.dispatch,
            PaletteDispatch::Command(LogCommand::ShowStackDiff)
        ));
    }

    #[test]
    fn filter_matches_name_case_insensitive() {
        let cmds = palette_commands();
        let out = filter_commands(cmds, "META");
        assert!(out.iter().any(|c| c.name == "metaedit"));
        assert!(!out.iter().any(|c| c.name == "compare"));
    }

    #[test]
    fn filter_matches_description() {
        let cmds = palette_commands();
        // "formatter" appears in fix's description
        let out = filter_commands(cmds, "formatter");
        assert!(out.iter().any(|c| c.name == "fix"));
    }

    #[test]
    fn empty_query_returns_all() {
        let cmds = palette_commands();
        assert_eq!(filter_commands(cmds, "").len(), cmds.len());
    }

    #[test]
    fn no_match_returns_empty() {
        let cmds = palette_commands();
        assert!(filter_commands(cmds, "zzzznotacommand").is_empty());
    }
}
