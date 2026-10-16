use super::ops::{add_operation, edit_operation, perform, selected, Term};
use super::{actions::*, render::draw, *};
use crate::broker::operation::Operation;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

pub(super) fn event_loop(term: &mut Term, app: &mut App) -> Result<()> {
    loop {
        term.draw(|f| draw(f, app))?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
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
                    0 => app.open_add_with(connect_daemon().and_then(|mut c| c.projects())),
                    _ => {
                        app.set_ok("loading secrets…");
                        term.draw(|f| draw(f, app))?;
                        if app.reload_secrets() {
                            app.status.clear();
                            app.mode = Mode::Search;
                        }
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
                KeyCode::Enter if app.input.trim().is_empty() => {
                    app.set_ok("give the secret a name first")
                }
                KeyCode::Enter => match add_operation(app) {
                    Some(op) => perform(term, app, op),
                    None => app.mode = Mode::Menu,
                },
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Char(c) => app.input.push(c),
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
                KeyCode::Esc => app.mode = Mode::Search,
                KeyCode::Up => app.action_idx = app.action_idx.saturating_sub(1),
                KeyCode::Down => app.action_idx = (app.action_idx + 1).min(ACTIONS.len() - 1),
                KeyCode::Enter => run_action(term, app, ACTIONS[app.action_idx].0),
                KeyCode::Char('c') => run_action(term, app, Action::Copy),
                KeyCode::Char('r') => run_action(term, app, Action::Reveal),
                KeyCode::Char('e') => run_action(term, app, Action::Edit),
                KeyCode::Char('d') => run_action(term, app, Action::Delete),
                _ => {}
            },
            Mode::Edit => match key.code {
                KeyCode::Esc => app.mode = Mode::Search,
                KeyCode::Tab | KeyCode::Down => app.edit_field = app.edit_field.next(),
                KeyCode::BackTab | KeyCode::Up => app.edit_field = app.edit_field.previous(),
                KeyCode::Backspace => {
                    if let Some(buf) = edit_buf(app) {
                        buf.pop();
                    }
                }
                KeyCode::Enter => match edit_operation(app) {
                    Some(op) => perform(term, app, op),
                    None => {
                        app.set_ok("no changes");
                        app.mode = Mode::Search;
                    }
                },
                KeyCode::Char(' ') if app.edit_field == EditField::Value => {
                    app.replace_value = !app.replace_value;
                }
                KeyCode::Char(c) => {
                    if let Some(buf) = edit_buf(app) {
                        buf.push(c);
                    }
                }
                _ => {}
            },
            Mode::ConfirmDelete => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    match selected(app, |secret| Operation::Delete { secret }) {
                        Some(op) => perform(term, app, op),
                        None => app.mode = Mode::Search,
                    }
                }
                _ => app.mode = Mode::Search,
            },
        }
    }
}
