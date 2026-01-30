//! Status bar widget

use ratatui::{Frame, prelude::*, text::Line, widgets::Paragraph};

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
