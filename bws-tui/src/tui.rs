use crate::bws::{self, Project, Secret};
use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::Terminal;
use secrecy::SecretString;
use std::io::{self, Stdout};
use std::thread;
use std::time::Duration;

const ACCENT: Color = Color::Cyan;
const DIM: Style = Style::new().fg(Color::DarkGray);
const MENU: [&str; 2] = ["Add a secret", "Search / manage secrets"];
const ACTIONS: [&str; 5] = [
    "Copy value to clipboard",
    "Reveal value",
    "Edit secret",
    "Delete secret",
    "Cancel",
];

#[derive(PartialEq)]
enum Mode {
    Menu,
    AddProject,
    AddKey,
    AddValue,
    Search,
    ActionMenu,
    Edit,
    ConfirmDelete,
}

struct App {
    mode: Mode,
    projects: Vec<Project>,
    secrets: Vec<Secret>,
    filtered: Vec<usize>, // indices into secrets
    menu_idx: usize,
    sel: usize, // selected row in the current list
    input: String,
    value_buf: String, // masked entry
    filter: String,
    edit_field: usize, // 0 key, 1 value, 2 note
    edit_key: String,
    edit_value: String,
    edit_note: String,
    status: String,
    status_err: bool,
    action_idx: usize,
    revealed: bool,
}

impl App {
    fn set_ok(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_err = false;
    }

    fn set_err(&mut self, e: &anyhow::Error) {
        self.status = format!("{e:#}");
        self.status_err = true;
    }

    fn project_name(&self, id: &str) -> &str {
        self.projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.as_str())
            .unwrap_or("?")
    }

    fn selected_secret(&self) -> Option<&Secret> {
        self.filtered.get(self.sel).map(|&i| &self.secrets[i])
    }

    fn refilter(&mut self) {
        self.filtered = (0..self.secrets.len())
            .filter(|&i| fuzzy_match(&self.secrets[i].key, &self.filter))
            .collect();
        self.sel = self.sel.min(self.filtered.len().saturating_sub(1));
    }

    fn reload_secrets(&mut self) {
        match bws::list_secrets(None) {
            Ok(s) => {
                self.secrets = s;
                self.refilter();
            }
            Err(e) => self.set_err(&e),
        }
    }
}

fn fuzzy_match(hay: &str, needle: &str) -> bool {
    let mut n = needle.chars().flat_map(char::to_lowercase);
    let mut cur = n.next();
    for c in hay.chars().flat_map(char::to_lowercase) {
        if Some(c) == cur {
            cur = n.next();
        }
    }
    cur.is_none()
}

fn copy_with_autoclear(value: String) {
    if let Ok(mut cb) = Clipboard::new() {
        let _ = cb.set_text(value.clone());
    }
    // ponytail: the clear thread dies if the process exits first — clipboard is
    // only guaranteed cleared while bws-tui stays open. Daemonize if that hurts.
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(30));
        if let Ok(mut cb) = Clipboard::new() {
            if cb.get_text().map(|t| t == value).unwrap_or(false) {
                let _ = cb.set_text(String::new());
            }
        }
    });
}

pub fn run() -> Result<()> {
    let projects = bws::list_projects()?;
    let mut app = App {
        mode: Mode::Menu,
        projects,
        secrets: Vec::new(),
        filtered: Vec::new(),
        menu_idx: 0,
        sel: 0,
        input: String::new(),
        value_buf: String::new(),
        filter: String::new(),
        edit_field: 0,
        edit_key: String::new(),
        edit_value: String::new(),
        edit_note: String::new(),
        status: String::new(),
        status_err: false,
        action_idx: 0,
        revealed: false,
    };

    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(out))?;
    let result = event_loop(&mut term, &mut app);
    let _ = disable_raw_mode();
    let _ = execute!(term.backend_mut(), LeaveAlternateScreen);
    result
}

fn event_loop(term: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        term.draw(|f| draw(f, app))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        app.status.clear();
        match app.mode {
            Mode::Menu => match key.code {
                KeyCode::Esc => return Ok(()),
                KeyCode::Up => app.menu_idx = app.menu_idx.saturating_sub(1),
                KeyCode::Down => app.menu_idx = (app.menu_idx + 1).min(MENU.len() - 1),
                KeyCode::Enter => match app.menu_idx {
                    0 => {
                        app.sel = 0;
                        app.mode = Mode::AddProject;
                    }
                    _ => {
                        app.set_ok("loading secrets…");
                        term.draw(|f| draw(f, app))?;
                        app.reload_secrets();
                        app.status.clear();
                        app.mode = Mode::Search;
                    }
                },
                _ => {}
            },
            Mode::AddProject => match key.code {
                KeyCode::Esc => app.mode = Mode::Menu,
                KeyCode::Up => app.sel = app.sel.saturating_sub(1),
                KeyCode::Down => app.sel = (app.sel + 1).min(app.projects.len().saturating_sub(1)),
                KeyCode::Enter => {
                    app.input.clear();
                    app.mode = Mode::AddKey;
                }
                _ => {}
            },
            Mode::AddKey => match key.code {
                KeyCode::Esc => app.mode = Mode::AddProject,
                KeyCode::Enter => {
                    if app.input.trim().is_empty() {
                        app.set_ok("give the secret a name first");
                    } else {
                        app.value_buf.clear();
                        app.mode = Mode::AddValue;
                    }
                }
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Char(c) => app.input.push(c),
                _ => {}
            },
            Mode::AddValue => match key.code {
                KeyCode::Esc => app.mode = Mode::AddKey,
                KeyCode::Enter => {
                    if app.value_buf.is_empty() {
                        app.set_ok("paste the secret value first");
                    } else {
                        let project_id = app.projects[app.sel].id.clone();
                        let key_name = app.input.trim().to_string();
                        match bws::create_secret(
                            &key_name,
                            &SecretString::from(app.value_buf.clone()),
                            &project_id,
                            None,
                        ) {
                            Ok(_) => {
                                app.set_ok(format!("✓ created “{key_name}”"));
                                app.secrets.clear(); // force reload next search
                                app.mode = Mode::Menu;
                            }
                            Err(e) => app.set_err(&e),
                        }
                        app.value_buf.clear();
                    }
                }
                KeyCode::Backspace => {
                    app.value_buf.pop();
                }
                KeyCode::Char(c) => app.value_buf.push(c),
                _ => {}
            },
            Mode::Search => match key.code {
                KeyCode::Esc => app.mode = Mode::Menu,
                KeyCode::Up => app.sel = app.sel.saturating_sub(1),
                KeyCode::Down => app.sel = (app.sel + 1).min(app.filtered.len().saturating_sub(1)),
                KeyCode::Backspace => {
                    app.filter.pop();
                    app.refilter();
                }
                KeyCode::Enter => {
                    if app.selected_secret().is_some() {
                        app.action_idx = 0;
                        app.revealed = false;
                        app.mode = Mode::ActionMenu;
                    }
                }
                KeyCode::Char(c) => {
                    app.filter.push(c);
                    app.refilter();
                }
                _ => {}
            },
            Mode::ActionMenu => match key.code {
                KeyCode::Esc => {
                    if app.revealed {
                        app.revealed = false;
                    } else {
                        app.mode = Mode::Search;
                    }
                }
                KeyCode::Up => {
                    if !app.revealed {
                        app.action_idx = app.action_idx.saturating_sub(1);
                    }
                }
                KeyCode::Down => {
                    if !app.revealed {
                        app.action_idx = (app.action_idx + 1).min(ACTIONS.len() - 1);
                    }
                }
                KeyCode::Enter => run_action(app),
                KeyCode::Char('c') => copy_action(app),
                KeyCode::Char('r') => app.revealed = !app.revealed,
                KeyCode::Char('e') => edit_action(app),
                KeyCode::Char('d') => app.mode = Mode::ConfirmDelete,
                _ => {}
            },
            Mode::Edit => match key.code {
                KeyCode::Esc => app.mode = Mode::Search,
                KeyCode::Tab | KeyCode::Down => app.edit_field = (app.edit_field + 1) % 3,
                KeyCode::BackTab | KeyCode::Up => app.edit_field = (app.edit_field + 2) % 3,
                KeyCode::Backspace => {
                    edit_buf(app).pop();
                }
                KeyCode::Enter => {
                    let Some(orig) = app.selected_secret().cloned() else {
                        app.mode = Mode::Search;
                        continue;
                    };
                    let key_chg = (app.edit_key != orig.key).then_some(app.edit_key.as_str());
                    let val_chg = (app.edit_value != orig.value)
                        .then(|| SecretString::from(app.edit_value.clone()));
                    let note_chg = (app.edit_note != orig.note).then_some(app.edit_note.as_str());
                    if key_chg.is_none() && val_chg.is_none() && note_chg.is_none() {
                        app.set_ok("no changes");
                        app.mode = Mode::Search;
                    } else {
                        match bws::edit_secret(&orig.id, key_chg, val_chg.as_ref(), note_chg) {
                            Ok(u) => {
                                app.set_ok(format!("✓ updated “{}”", u.key));
                                app.reload_secrets();
                                app.mode = Mode::Search;
                            }
                            Err(e) => app.set_err(&e),
                        }
                    }
                }
                KeyCode::Char(c) => edit_buf(app).push(c),
                _ => {}
            },
            Mode::ConfirmDelete => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if let Some(s) = app.selected_secret().cloned() {
                        match bws::delete_secret(&s.id) {
                            Ok(()) => {
                                app.set_ok(format!("✓ deleted “{}”", s.key));
                                app.reload_secrets();
                            }
                            Err(e) => app.set_err(&e),
                        }
                    }
                    app.mode = Mode::Search;
                }
                _ => app.mode = Mode::Search,
            },
        }
    }
}

fn copy_action(app: &mut App) {
    if let Some(s) = app.selected_secret() {
        let key = s.key.clone();
        copy_with_autoclear(s.value.clone());
        app.set_ok(format!("✓ copied “{key}” — clears in 30s (keep app open)"));
    }
    app.mode = Mode::Search;
}

fn edit_action(app: &mut App) {
    if let Some(s) = app.selected_secret().cloned() {
        app.edit_key = s.key;
        app.edit_value = s.value;
        app.edit_note = s.note;
        app.edit_field = 0;
        app.mode = Mode::Edit;
    }
}

fn run_action(app: &mut App) {
    match app.action_idx {
        0 => copy_action(app),
        1 => app.revealed = true,
        2 => edit_action(app),
        3 => app.mode = Mode::ConfirmDelete,
        _ => app.mode = Mode::Search,
    }
}

fn edit_buf(app: &mut App) -> &mut String {
    match app.edit_field {
        0 => &mut app.edit_key,
        1 => &mut app.edit_value,
        _ => &mut app.edit_note,
    }
}

fn masked(s: &str) -> String {
    "•".repeat(s.chars().count())
}

fn panel(title: impl Into<Line<'static>>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(DIM)
        .title(title)
        .title_style(Style::default().fg(ACCENT))
}

fn key_hints(pairs: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (cap, desc)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", DIM));
        }
        spans.push(Span::styled(
            format!(" {cap} "),
            Style::default().fg(Color::Black).bg(Color::DarkGray),
        ));
        spans.push(Span::styled(format!(" {desc}"), DIM));
    }
    Line::from(spans)
}

fn draw(f: &mut ratatui::Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .horizontal_margin(1)
    .split(area);

    // header
    let header = Layout::horizontal([Constraint::Min(20), Constraint::Length(28)]).split(chunks[0]);
    f.render_widget(
        Paragraph::new(Span::styled(
            "◆ bws-tui",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        header[0],
    );
    let stats = format!(
        "{} projects · {} secrets",
        app.projects.len(),
        app.secrets.len()
    );
    f.render_widget(Paragraph::new(Span::styled(stats, DIM)), header[1]);

    let hints: &[(&str, &str)] = match app.mode {
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
    };

    match app.mode {
        Mode::Menu => {
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
            f.render_stateful_widget(list, chunks[1], &mut state);
        }
        Mode::AddProject => {
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
            f.render_stateful_widget(list, chunks[1], &mut state);
        }
        Mode::AddKey => {
            let block = panel(format!(
                " step 2/3 — name this secret (e.g. STRIPE_API_KEY) · project: {} ",
                app.projects[app.sel].name
            ));
            let inner = block.inner(chunks[1]);
            f.render_widget(block, chunks[1]);
            let label = "key name: ";
            f.render_widget(Paragraph::new(format!("{label}{}", app.input)), inner);
            f.set_cursor_position(Position::new(
                inner.x + (label.len() + app.input.len()) as u16,
                inner.y,
            ));
        }
        Mode::AddValue => {
            let block = panel(format!(
                " step 3/3 — paste the secret value for “{}” (hidden while you type) ",
                app.input.trim()
            ));
            let inner = block.inner(chunks[1]);
            f.render_widget(block, chunks[1]);
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
        Mode::Search => {
            let rows =
                Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(chunks[1]);
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
        Mode::ActionMenu => {
            let items: Vec<ListItem> = app
                .filtered
                .iter()
                .map(|&i| ListItem::new(format!("  {}", app.secrets[i].key)))
                .collect();
            f.render_widget(List::new(items).block(panel(" secrets ")), chunks[1]);
            if let Some(s) = app.selected_secret() {
                if app.revealed {
                    let dialog = centered(chunks[1], 60, 9);
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
                    let dialog = centered(chunks[1], 40, 9);
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
        Mode::Edit => {
            let fields: [(&str, String); 3] = [
                ("key name", app.edit_key.clone()),
                ("value", masked(&app.edit_value)),
                ("note", app.edit_note.clone()),
            ];
            let lines: Vec<Line> = fields
                .iter()
                .enumerate()
                .map(|(i, (name, val))| {
                    if i == app.edit_field {
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
                chunks[1],
            );
        }
        Mode::ConfirmDelete => {
            let items: Vec<ListItem> = app
                .filtered
                .iter()
                .map(|&i| ListItem::new(format!("  {}", app.secrets[i].key)))
                .collect();
            f.render_widget(List::new(items).block(panel(" secrets ")), chunks[1]);
            if let Some(s) = app.selected_secret() {
                let dialog = centered(chunks[1], 50, 7);
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
    }

    // footer: status (colored) + key hints
    let status_line = if app.status.is_empty() {
        Line::from("")
    } else {
        Line::from(Span::styled(
            format!("  {}", app.status),
            Style::default().fg(if app.status_err {
                Color::Red
            } else {
                Color::Green
            }),
        ))
    };
    f.render_widget(
        Paragraph::new(vec![status_line, key_hints(hints)]),
        chunks[2],
    );
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let v = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(h),
        Constraint::Min(0),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(w),
        Constraint::Min(0),
    ])
    .split(v[1])[1]
}
