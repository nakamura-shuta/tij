//! Tag View rendering

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{DisplayRow, TagFilter, TagView};
use crate::model::{Notification, TagInfo};
use crate::ui::text::{display_width, fit_display_width};
use crate::ui::{components, navigation, theme};

/// Width of the name column in terminal cells (fits `v0.11.0@origin`-style names)
const NAME_WIDTH: usize = 30;

/// Marker appended to a conflicted tag name
const CONFLICT_MARKER: &str = " !";

impl TagView {
    /// Render the tag view with optional notification in title bar
    pub fn render(&self, frame: &mut Frame, area: Rect, notification: Option<&Notification>) {
        let title = Line::from(format!(" {} ", self.title_text()))
            .bold()
            .cyan()
            .centered();

        let title_width = title.width();
        let available_for_notif = area.width.saturating_sub(title_width as u16 + 4) as usize;
        let notif_line = notification
            .filter(|n| !n.is_expired())
            .map(|n| components::build_notification_title(n, Some(available_for_notif)))
            .filter(|line| !line.spans.is_empty());

        let block = components::bordered_block_with_notification(title, notif_line);

        if self.display_rows.is_empty() {
            let paragraph = Paragraph::new(self.empty_message()).block(block);
            frame.render_widget(paragraph, area);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize;
        if inner_height == 0 {
            return;
        }

        let scroll_offset =
            navigation::adjust_scroll(self.selected, self.scroll_offset, inner_height);

        let mut lines: Vec<Line> = Vec::new();
        for (idx, row) in self.display_rows.iter().enumerate().skip(scroll_offset) {
            if lines.len() >= inner_height {
                break;
            }
            let is_selected = idx == self.selected;
            let line = match row {
                DisplayRow::Header(text) => build_header_line(text),
                DisplayRow::Tag(tag_idx) => build_tag_line(&self.tags[*tag_idx], is_selected),
            };
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }

    /// Title text: `Tags (6)` in All mode, `Tags (1/6, tracked)` when filtered
    fn title_text(&self) -> String {
        match self.filter {
            TagFilter::All => format!("Tags ({})", self.tag_count()),
            filter => format!(
                "Tags ({}/{}, {})",
                self.tag_count(),
                self.total_count(),
                filter.label()
            ),
        }
    }

    /// Distinguish "no tags at all" from "the filter hid everything"
    fn empty_message(&self) -> &'static str {
        if self.total_count() == 0 {
            "No tags found"
        } else {
            "No tags match the current filter (F to change)"
        }
    }
}

fn build_header_line(text: &str) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!("  {}", text),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )])
}

fn build_tag_line(tag: &TagInfo, is_selected: bool) -> Line<'static> {
    let is_local = tag.is_local();
    let name_color = if tag.is_untracked_remote() {
        Color::Yellow
    } else if is_local {
        Color::Green
    } else {
        Color::DarkGray
    };

    let mut spans = vec![Span::raw("  ")];
    spans.extend(build_name_column(tag, Style::default().fg(name_color)));

    // Remote rows have no local target, so they stop after the name column.
    if is_local {
        if let Some(ref change_id) = tag.change_id {
            spans.push(Span::styled(
                format!("  {:<10}", change_id),
                Style::default().fg(Color::Yellow),
            ));
        } else {
            spans.push(Span::raw(format!("{:12}", "")));
        }

        let desc = tag.description.as_deref().unwrap_or("(no description)");
        spans.push(Span::styled(
            desc.to_string(),
            Style::default().fg(Color::White),
        ));
    }

    let mut line = Line::from(spans);
    if is_selected {
        line = line.style(
            Style::default()
                .fg(theme::selection::FG)
                .bg(theme::selection::BG)
                .add_modifier(Modifier::BOLD),
        );
    }
    line
}

/// Name column: full name plus a red ` !` when conflicted, always `NAME_WIDTH` cells.
///
/// The marker needs its own span to be red, so the padding is emitted after it
/// instead of relying on `fit_display_width` alone — otherwise the marker would
/// float at the far edge of the column and the trailing columns would shift.
fn build_name_column(tag: &TagInfo, style: Style) -> Vec<Span<'static>> {
    let name = tag.full_name();
    if !tag.conflict {
        return vec![Span::styled(fit_display_width(&name, NAME_WIDTH), style)];
    }

    let marker_width = display_width(CONFLICT_MARKER);
    let budget = NAME_WIDTH.saturating_sub(marker_width);
    // Truncated/padded to `budget`, then re-trimmed so the marker sits next to
    // the name; tag names cannot contain spaces, so nothing real is lost.
    let fitted = fit_display_width(&name, budget);
    let name_text = fitted.trim_end().to_string();
    let pad = NAME_WIDTH.saturating_sub(display_width(&name_text) + marker_width);

    let mut spans = vec![
        Span::styled(name_text, style),
        Span::styled(CONFLICT_MARKER, Style::default().fg(Color::Red)),
    ];
    if pad > 0 {
        spans.push(Span::raw(" ".repeat(pad)));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChangeId, CommitId};

    fn local(name: &str) -> TagInfo {
        TagInfo {
            name: name.to_string(),
            remote: None,
            present: true,
            tracked: false,
            conflict: false,
            change_id: Some(ChangeId::new("mzslzzzz".to_string())),
            commit_id: Some(CommitId::new("abcd1234".to_string())),
            description: Some("release".to_string()),
        }
    }

    fn remote(name: &str, remote: &str, tracked: bool) -> TagInfo {
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

    /// Flatten a rendered line to plain text for assertions.
    fn text(line: &Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn view_with(tags: Vec<TagInfo>) -> TagView {
        let mut view = TagView::new();
        view.set_tags(tags);
        view
    }

    #[test]
    fn title_shows_plain_count_in_all_mode() {
        let view = view_with(vec![local("v1.0"), remote("v1.0", "origin", true)]);
        assert_eq!(view.title_text(), "Tags (2)");
    }

    #[test]
    fn title_shows_filtered_over_total() {
        let mut view = view_with(vec![
            local("v1.0"),
            remote("v1.0", "origin", true),
            remote("v0.9", "origin", false),
        ]);
        view.cycle_filter(); // Tracked
        assert_eq!(view.title_text(), "Tags (1/3, tracked)");
        view.cycle_filter(); // Conflicted
        assert_eq!(view.title_text(), "Tags (0/3, conflicted)");
    }

    #[test]
    fn empty_message_distinguishes_no_tags_from_filtered_out() {
        let empty = view_with(vec![]);
        assert_eq!(empty.empty_message(), "No tags found");

        let mut filtered = view_with(vec![local("v1.0")]);
        filtered.cycle_filter(); // Tracked → hides the only (local) tag
        assert!(filtered.display_rows.is_empty());
        assert_eq!(
            filtered.empty_message(),
            "No tags match the current filter (F to change)"
        );
    }

    #[test]
    fn remote_row_shows_full_name_and_stops_after_it() {
        let line = build_tag_line(&remote("v1.0", "origin", false), false);
        let rendered = text(&line);
        assert!(rendered.contains("v1.0@origin"), "got: {rendered}");
        // 2-space indent + 30-cell name column, nothing else
        assert_eq!(rendered.trim_end(), "  v1.0@origin");
        assert_eq!(display_width(&rendered), 2 + NAME_WIDTH);
    }

    #[test]
    fn local_row_shows_change_id_and_description() {
        let line = build_tag_line(&local("v1.0"), false);
        let rendered = text(&line);
        assert!(rendered.contains("mzslzzzz"), "got: {rendered}");
        assert!(rendered.ends_with("release"), "got: {rendered}");
        // change_id occupies exactly 12 cells (2 spaces + 10-wide field)
        let after_name = &rendered[2 + NAME_WIDTH..];
        assert_eq!(&after_name[..12], "  mzslzzzz  ");
    }

    #[test]
    fn name_colors_split_local_tracked_untracked() {
        let color = |t: &TagInfo| build_tag_line(t, false).spans[1].style.fg;
        assert_eq!(color(&local("v1.0")), Some(Color::Green));
        assert_eq!(
            color(&remote("v1.0", "origin", true)),
            Some(Color::DarkGray)
        );
        assert_eq!(color(&remote("v0.9", "origin", false)), Some(Color::Yellow));
    }

    #[test]
    fn conflict_marker_is_red_and_keeps_column_width() {
        let mut tag = local("v1.0");
        tag.conflict = true;
        let line = build_tag_line(&tag, false);
        let rendered = text(&line);

        assert!(rendered.starts_with("  v1.0 !"), "got: {rendered}");
        let marker = line
            .spans
            .iter()
            .find(|s| s.content.as_ref() == CONFLICT_MARKER)
            .expect("conflict marker span");
        assert_eq!(marker.style.fg, Some(Color::Red));
        // Column alignment is unchanged: change_id still starts at the same cell
        let plain = text(&build_tag_line(&local("v1.0"), false));
        assert_eq!(
            rendered.find("mzslzzzz"),
            plain.find("mzslzzzz"),
            "conflict marker must not shift the change_id column"
        );
    }

    #[test]
    fn long_conflicted_name_still_fits_the_column() {
        let mut tag = remote(&"v".repeat(40), "origin", false);
        tag.conflict = true;
        let rendered = text(&build_tag_line(&tag, false));
        assert_eq!(display_width(&rendered), 2 + NAME_WIDTH);
        assert!(rendered.ends_with(CONFLICT_MARKER), "got: {rendered}");
    }

    #[test]
    fn header_line_is_indented_and_dim() {
        let line = build_header_line("── Local ──");
        assert_eq!(text(&line), "  ── Local ──");
        assert_eq!(line.spans[0].style.fg, Some(Color::DarkGray));
    }
}
