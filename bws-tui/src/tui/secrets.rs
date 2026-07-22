use super::{chrome::*, *};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};

pub(super) fn draw_search(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
    let filter_block = panel(" search ");
    let filter_inner = filter_block.inner(rows[0]);
    f.render_widget(filter_block, rows[0]);
    f.render_widget(Paragraph::new(app.filter.to_string()), filter_inner);
    f.set_cursor_position(Position::new(
        filter_inner.x + app.filter.len() as u16,
        filter_inner.y,
    ));

    let title = format!(" secrets — {}/{} ", app.filtered.len(), app.secrets.len());
    let list_block = panel(title);
    if app.filtered.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled("  no secrets match", DIM)).block(list_block),
            rows[1],
        );
    } else {
        let items: Vec<ListItem> = app
            .filtered
            .iter()
            .map(|&i| {
                let s = &app.secrets[i];
                let badge = s
                    .project_id
                    .as_deref()
                    .map(|pid| app.project_name(pid))
                    .unwrap_or("no project");
                ListItem::new(Line::from(vec![
                    Span::raw(format!("  {}", s.key)),
                    Span::styled(format!("  {badge}"), DIM),
                ]))
            })
            .collect();
        let list = List::new(items)
            .block(list_block)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▸");
        let mut state = ListState::default();
        state.select(Some(app.sel));
        f.render_stateful_widget(list, rows[1], &mut state);
    }
}

pub(super) fn draw_action_menu(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| ListItem::new(format!("  {}", app.secrets[i].key)))
        .collect();
    f.render_widget(List::new(items).block(panel(" secrets ")), area);
    if let Some(s) = app.selected_secret() {
        if app.revealed {
            let dialog = centered(area, 60, 9);
            f.render_widget(Clear, dialog);
            let p = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    s.value.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                key_hints(&[("Esc", "hide")]),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title(format!(" “{}” ", s.key))
                    .title_style(Style::default().fg(ACCENT)),
            )
            .wrap(Wrap { trim: false });
            f.render_widget(p, dialog);
        } else {
            let dialog = centered(area, 40, 9);
            f.render_widget(Clear, dialog);
            let lines: Vec<Line> = ACTIONS
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    // semantic: green=safe action, yellow=caution (value visible),
                    // cyan=modify, red=destructive, dim=cancel
                    let color = match i {
                        0 => Color::Green,
                        1 => Color::Yellow,
                        2 => ACCENT,
                        3 => Color::Red,
                        _ => Color::DarkGray,
                    };
                    let style = if i == app.action_idx {
                        Style::default().fg(color).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(color)
                    };
                    let marker = if i == app.action_idx { "▸ " } else { "  " };
                    Line::from(Span::styled(format!("{marker}{a}"), style))
                })
                .collect();
            let p = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(format!(" “{}” ", s.key))
                    .title_style(Style::default().fg(ACCENT)),
            );
            f.render_widget(p, dialog);
        }
    }
}

pub(super) fn draw_confirm_delete(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| ListItem::new(format!("  {}", app.secrets[i].key)))
        .collect();
    f.render_widget(List::new(items).block(panel(" secrets ")), area);
    if let Some(s) = app.selected_secret() {
        let dialog = centered(area, 50, 7);
        f.render_widget(Clear, dialog);
        let p = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("Delete "),
                Span::styled(
                    format!("“{}”", s.key),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" permanently?"),
            ]),
            Line::from(""),
            key_hints(&[("y", "delete forever"), ("any other key", "cancel")]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Red))
                .title(" confirm ")
                .title_style(Style::default().fg(Color::Red)),
        )
        .wrap(Wrap { trim: false });
        f.render_widget(p, dialog);
    }
}
