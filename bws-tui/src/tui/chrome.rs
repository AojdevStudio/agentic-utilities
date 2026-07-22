use super::*;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

pub(super) fn layout(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .horizontal_margin(1)
    .split(area)
}

pub(super) fn draw_header(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let header = Layout::horizontal([Constraint::Min(20), Constraint::Length(28)]).split(area);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "◆ hush",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        header[0],
    );
    let stats = format!(
        "{} projects · {} secrets",
        app.projects.len(),
        app.secrets.len()
    );
    frame.render_widget(Paragraph::new(Span::styled(stats, DIM)), header[1]);
}

pub(super) fn hints(mode: &Mode) -> &'static [(&'static str, &'static str)] {
    match mode {
        Mode::Menu => &[("↑↓", "move"), ("Enter", "select"), ("Esc", "quit")],
        Mode::AddProject => &[("↑↓", "move"), ("Enter", "select"), ("Esc", "back")],
        Mode::AddKey => &[("Enter", "next"), ("Esc", "back")],
        Mode::AddValue => &[("Enter", "create"), ("Esc", "back")],
        Mode::Search => &[
            ("type", "filter"),
            ("↑↓", "move"),
            ("Enter", "actions"),
            ("Esc", "back"),
        ],
        Mode::ActionMenu => &[("↑↓", "choose"), ("Enter", "select"), ("Esc", "close")],
        Mode::Edit => &[("Tab", "next field"), ("Enter", "save"), ("Esc", "cancel")],
        Mode::ConfirmDelete => &[("y", "confirm delete"), ("any key", "cancel")],
    }
}

pub(super) fn draw_footer(
    frame: &mut ratatui::Frame,
    area: Rect,
    app: &App,
    hints: &[(&str, &str)],
) {
    let status = if app.status.is_empty() {
        Line::from("")
    } else {
        let color = if app.status_err {
            Color::Red
        } else {
            Color::Green
        };
        Line::from(Span::styled(
            format!("  {}", app.status),
            Style::default().fg(color),
        ))
    };
    frame.render_widget(Paragraph::new(vec![status, key_hints(hints)]), area);
}

pub(super) fn masked(value: &str) -> String {
    "•".repeat(value.chars().count())
}

pub(super) fn panel(title: impl Into<Line<'static>>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(DIM)
        .title(title)
        .title_style(Style::default().fg(ACCENT))
}

pub(super) fn key_hints(pairs: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, (cap, description)) in pairs.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", DIM));
        }
        spans.push(Span::styled(
            format!(" {cap} "),
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ));
        spans.push(Span::styled(format!(" {description}"), DIM));
    }
    Line::from(spans)
}

pub(super) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(height),
        Constraint::Min(0),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(width),
        Constraint::Min(0),
    ])
    .split(vertical[1])[1]
}
