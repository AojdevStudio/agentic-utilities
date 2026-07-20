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
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;
use secrecy::SecretString;
use std::io::{self, Stdout};
use std::thread;
use std::time::Duration;

const MENU: [&str; 2] = ["Add secret", "Search / manage secrets"];

#[derive(PartialEq)]
enum Mode {
    Menu,
    AddProject,
    AddKey,
    AddValue,
    Search,
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
}

impl App {
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
            Err(e) => self.status = format!("{e:#}"),
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
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        app.status.clear();
        match app.mode {
            Mode::Menu => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Up | KeyCode::Char('k') => app.menu_idx = app.menu_idx.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    app.menu_idx = (app.menu_idx + 1).min(MENU.len() - 1)
                }
                KeyCode::Enter | KeyCode::Char('a') | KeyCode::Char('s') => {
                    let pick = if matches!(key.code, KeyCode::Enter) {
                        app.menu_idx
                    } else if key.code == KeyCode::Char('a') {
                        0
                    } else {
                        1
                    };
                    match pick {
                        0 => {
                            app.sel = 0;
                            app.mode = Mode::AddProject;
                        }
                        _ => {
                            if app.secrets.is_empty() {
                                app.status = "loading secrets…".into();
                                term.draw(|f| draw(f, app))?;
                                app.reload_secrets();
                            } else {
                                app.refilter();
                            }
                            app.status.clear();
                            app.mode = Mode::Search;
                        }
                    }
                }
                _ => {}
            },
            Mode::AddProject => match key.code {
                KeyCode::Esc => app.mode = Mode::Menu,
                KeyCode::Up | KeyCode::Char('k') => app.sel = app.sel.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    app.sel = (app.sel + 1).min(app.projects.len().saturating_sub(1))
                }
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
                        app.status = "key cannot be empty".into();
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
                        app.status = "value cannot be empty".into();
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
                                app.status = format!("created {key_name}");
                                app.secrets.clear(); // force reload next search
                                app.mode = Mode::Menu;
                            }
                            Err(e) => app.status = format!("{e:#}"),
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
                KeyCode::Down => {
                    app.sel = (app.sel + 1).min(app.filtered.len().saturating_sub(1))
                }
                KeyCode::Backspace => {
                    app.filter.pop();
                    app.refilter();
                }
                KeyCode::Char('r') if app.filter.is_empty() => app.reload_secrets(),
                KeyCode::Enter => {
                    if let Some(s) = app.selected_secret() {
                        let value = s.value.clone();
                        let _ = disable_raw_mode();
                        let _ = execute!(term.backend_mut(), LeaveAlternateScreen);
                        println!("{value}");
                        return Ok(());
                    }
                }
                KeyCode::Char('c') if app.filter.is_empty() => {
                    if let Some(s) = app.selected_secret() {
                        copy_with_autoclear(s.value.clone());
                        app.status = format!("copied {} — clears in 30s while app stays open", s.key);
                    }
                }
                KeyCode::Char('e') if app.filter.is_empty() => {
                    if let Some(s) = app.selected_secret().cloned() {
                        app.edit_key = s.key;
                        app.edit_value = s.value;
                        app.edit_note = s.note;
                        app.edit_field = 0;
                        app.mode = Mode::Edit;
                    }
                }
                KeyCode::Char('d') if app.filter.is_empty() => {
                    if app.selected_secret().is_some() {
                        app.mode = Mode::ConfirmDelete;
                    }
                }
                KeyCode::Char(c) => {
                    app.filter.push(c);
                    app.refilter();
                }
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
                        app.status = "no changes".into();
                        app.mode = Mode::Search;
                    } else {
                        match bws::edit_secret(&orig.id, key_chg, val_chg.as_ref(), note_chg) {
                            Ok(u) => {
                                app.status = format!("updated {}", u.key);
                                app.reload_secrets();
                                app.mode = Mode::Search;
                            }
                            Err(e) => app.status = format!("{e:#}"),
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
                                app.status = format!("deleted {}", s.key);
                                app.reload_secrets();
                            }
                            Err(e) => app.status = format!("{e:#}"),
                        }
                    }
                    app.mode = Mode::Search;
                }
                _ => app.mode = Mode::Search,
            },
        }
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

fn draw(f: &mut ratatui::Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(area);

    let title = Paragraph::new("bws-tui")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(title, chunks[0]);

    match app.mode {
        Mode::Menu => {
            let items: Vec<ListItem> = MENU
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let marker = if i == app.menu_idx { "> " } else { "  " };
                    ListItem::new(format!("{marker}{m}"))
                })
                .collect();
            f.render_widget(List::new(items), chunks[1]);
        }
        Mode::AddProject => {
            let items: Vec<ListItem> = app
                .projects
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let marker = if i == app.sel { "> " } else { "  " };
                    ListItem::new(format!("{marker}{}", p.name))
                })
                .collect();
            let list = List::new(items).block(Block::default().borders(Borders::TOP).title("add: pick project"));
            f.render_widget(list, chunks[1]);
        }
        Mode::AddKey => {
            let p = Paragraph::new(format!("key: {}", app.input)).block(
                Block::default()
                    .borders(Borders::TOP)
                    .title(format!("add to {}", app.projects[app.sel].name)),
            );
            f.render_widget(p, chunks[1]);
            f.set_cursor_position(Position::new(chunks[1].x + 6 + app.input.len() as u16, chunks[1].y + 1));
        }
        Mode::AddValue => {
            let p = Paragraph::new(format!("value: {}", masked(&app.value_buf))).block(
                Block::default().borders(Borders::TOP).title("value (masked)"),
            );
            f.render_widget(p, chunks[1]);
        }
        Mode::Search => {
            let rows = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).split(chunks[1]);
            let filter_line = Paragraph::new(format!("filter: {}", app.filter));
            f.render_widget(filter_line, rows[0]);
            let items: Vec<ListItem> = app
                .filtered
                .iter()
                .enumerate()
                .map(|(row, &i)| {
                    let s = &app.secrets[i];
                    let marker = if row == app.sel { "> " } else { "  " };
                    let badge = s
                        .project_id
                        .as_deref()
                        .map(|pid| app.project_name(pid))
                        .unwrap_or("no project");
                    ListItem::new(format!("{marker}{}  [{}]", s.key, badge))
                })
                .collect();
            let mut state = ListState::default();
            state.select(Some(app.sel));
            f.render_stateful_widget(List::new(items), rows[1], &mut state);
        }
        Mode::Edit => {
            let fields = [
                ("key", app.edit_key.clone(), false),
                ("value", masked(&app.edit_value), true),
                ("note", app.edit_note.clone(), false),
            ];
            let lines: Vec<Line> = fields
                .iter()
                .enumerate()
                .map(|(i, (name, val, is_masked))| {
                    let marker = if i == app.edit_field { "> " } else { "  " };
                    let style = if i == app.edit_field {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    };
                    let _ = is_masked;
                    Line::from(Span::styled(format!("{marker}{name}: {val}"), style))
                })
                .collect();
            let p = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::TOP)
                    .title("edit (Tab moves, Enter saves, Esc cancels)"),
            );
            f.render_widget(p, chunks[1]);
        }
        Mode::ConfirmDelete => {
            // dim background list, then modal
            let items: Vec<ListItem> = app
                .filtered
                .iter()
                .map(|&i| ListItem::new(app.secrets[i].key.clone()))
                .collect();
            f.render_widget(List::new(items), chunks[1]);
            if let Some(s) = app.selected_secret() {
                let dialog = centered(chunks[1], 46, 5);
                f.render_widget(Clear, dialog);
                let p = Paragraph::new(vec![
                    Line::from(format!("Delete '{}' permanently?", s.key)),
                    Line::from(""),
                    Line::from("[y] confirm    [any other key] cancel"),
                ])
                .block(Block::default().borders(Borders::ALL).title("confirm"))
                .wrap(Wrap { trim: false });
                f.render_widget(p, dialog);
            }
        }
    }

    let hints = match app.mode {
        Mode::Menu => "↑↓/jk move · Enter select · q quit",
        Mode::AddProject => "↑↓/jk move · Enter select · Esc back",
        Mode::AddKey => "type key · Enter next · Esc back",
        Mode::AddValue => "type value (masked) · Enter create · Esc back",
        Mode::Search => "type to filter · Enter print · c copy · e edit · d delete · r refresh · Esc back",
        Mode::Edit => "Tab/↑↓ field · Enter save · Esc cancel",
        Mode::ConfirmDelete => "y confirm · anything else cancels",
    };
    let bottom = if app.status.is_empty() {
        hints.to_string()
    } else {
        format!("{}  |  {}", app.status, hints)
    };
    f.render_widget(Paragraph::new(bottom).style(Style::default().fg(Color::DarkGray)), chunks[2]);
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let v = Layout::vertical([Constraint::Min(0), Constraint::Length(h), Constraint::Min(0)]).split(area);
    Layout::horizontal([Constraint::Min(0), Constraint::Length(w), Constraint::Min(0)]).split(v[1])[1]
}
