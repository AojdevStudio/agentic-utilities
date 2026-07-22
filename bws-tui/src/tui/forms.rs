use super::{chrome::*, *};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

pub(super) fn draw_menu(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = MENU
        .iter()
        .map(|m| ListItem::new(format!("  {m}")))
        .collect();
    let list = List::new(items)
        .block(panel(" menu "))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸");
    let mut state = ListState::default();
    state.select(Some(app.menu_idx));
    f.render_stateful_widget(list, area, &mut state);
}

pub(super) fn draw_add_project(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .projects
        .iter()
        .map(|p| ListItem::new(format!("  {}", p.name)))
        .collect();
    let list = List::new(items)
        .block(panel(" step 1/3 — which project does it belong to? "))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸");
    let mut state = ListState::default();
    state.select(Some(app.sel));
    f.render_stateful_widget(list, area, &mut state);
}

pub(super) fn draw_add_key(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let block = panel(format!(
        " step 2/3 — name this secret (e.g. STRIPE_API_KEY) · project: {} ",
        app.projects[app.sel].name
    ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let label = "key name: ";
    f.render_widget(Paragraph::new(format!("{label}{}", app.input)), inner);
    f.set_cursor_position(Position::new(
        inner.x + (label.len() + app.input.len()) as u16,
        inner.y,
    ));
}

pub(super) fn draw_add_value(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let block = panel(format!(
        " step 3/3 — paste the secret value for “{}” (hidden while you type) ",
        app.input.trim()
    ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let label = "value: ";
    f.render_widget(
        Paragraph::new(format!("{label}{}", masked(&app.value_buf))),
        inner,
    );
    f.set_cursor_position(Position::new(
        inner.x + (label.len() + app.value_buf.chars().count()) as u16,
        inner.y,
    ));
}

pub(super) fn draw_edit(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let fields: [(EditField, &str, String); 3] = [
        (EditField::Key, "key name", app.edit_key.clone()),
        (EditField::Value, "value", masked(&app.edit_value)),
        (EditField::Note, "note", app.edit_note.clone()),
    ];
    let lines: Vec<Line> = fields
        .iter()
        .map(|(field, name, val)| {
            if *field == app.edit_field {
                Line::from(vec![
                    Span::styled("▸ ", Style::default().fg(ACCENT)),
                    Span::styled(format!("{name}: {val}"), Style::default().fg(ACCENT)),
                ])
            } else {
                Line::from(format!("  {name}: {val}"))
            }
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).block(panel(" edit — type to change, Enter saves ")),
        area,
    );
}
