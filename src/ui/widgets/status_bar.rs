//! Status bar widget

use ratatui::{Frame, prelude::*, text::Line, widgets::Paragraph};

use crate::ui::views::DiffView;

/// Render the status bar at the bottom of the screen
pub fn render_status_bar(frame: &mut Frame) {
    let area = frame.area();
    if area.height < 2 {
        return;
    }

    let status_area = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };

    let status = Line::from(vec![
        Span::styled(
            " [?] Help ",
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        Span::raw(" "),
        Span::styled(
            " [/] Search ",
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ),
        Span::raw(" "),
        Span::styled(
            " [r] Revset ",
            Style::default().fg(Color::Black).bg(Color::Magenta),
        ),
        Span::raw(" "),
        Span::styled(
            " [Tab] Switch ",
            Style::default().fg(Color::Black).bg(Color::Blue),
        ),
        Span::raw(" "),
        Span::styled(
            " [q] Quit ",
            Style::default().fg(Color::Black).bg(Color::Red),
        ),
    ]);

    frame.render_widget(Paragraph::new(status), status_area);
}

/// Render the status bar for diff view
pub fn render_diff_status_bar(frame: &mut Frame, diff_view: &DiffView) {
    let area = frame.area();
    if area.height < 2 {
        return;
    }

    let status_area = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };

    // Build context info (current file)
    let context = diff_view.current_context();

    let status = Line::from(vec![
        Span::styled(
            format!(" {} ", diff_view.change_id),
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ),
        Span::raw(" "),
        Span::styled(format!(" {} ", context), Style::default().fg(Color::Cyan)),
        Span::raw(" "),
        Span::styled(
            " [j/k] Scroll ",
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        Span::raw(" "),
        Span::styled(
            " ]/[ Next/Prev File ",
            Style::default().fg(Color::Black).bg(Color::Magenta),
        ),
        Span::raw(" "),
        Span::styled(
            " [q] Back ",
            Style::default().fg(Color::Black).bg(Color::Red),
        ),
    ]);

    frame.render_widget(Paragraph::new(status), status_area);
}
