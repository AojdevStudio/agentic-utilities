use super::{actions::*, render::draw, *};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io::Stdout, time::Duration};

pub(super) fn event_loop(
    term: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        app.clear_clipboard_if_due();
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
                    0 => app.open_add_with(bws::list_projects()),
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
                KeyCode::Tab | KeyCode::Down => app.edit_field = app.edit_field.next(),
                KeyCode::BackTab | KeyCode::Up => app.edit_field = app.edit_field.previous(),
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
