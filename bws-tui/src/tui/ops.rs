//! Daemon operations from the TUI. Each one hands the terminal to the daemon:
//! the TUI leaves raw mode and the alternate screen, the daemon prompts and
//! discloses on the terminal, and the TUI redraws after.

use super::*;
use crate::broker::client_human::open_terminal;
use crate::broker::operation::{
    Operation, Outcome, Refusal, SecretSelector, ValueChange, ValueSource,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::Stdout;
use std::os::fd::AsFd;

pub(super) type Term = Terminal<CrosstermBackend<Stdout>>;

/// The add for the chosen project and typed key. The daemon prompts for the value.
pub(super) fn add_operation(app: &App) -> Option<Operation> {
    let project = app.projects.get(app.sel)?;
    Some(Operation::Add {
        project: project.id.clone(),
        key: app.input.trim().to_owned(),
        note: None,
        value: ValueSource::TtyPrompt,
    })
}

/// The edit the form describes, or `None` when nothing changed. The stored
/// value is kept unless the user chose to replace it.
pub(super) fn edit_operation(app: &App) -> Option<Operation> {
    let current = app.selected_secret()?;
    let new_key = (app.edit_key != current.key).then(|| app.edit_key.clone());
    let note = (app.edit_note != current.note).then(|| app.edit_note.clone());
    let value = if app.replace_value {
        ValueChange::Replace(ValueSource::TtyPrompt)
    } else {
        ValueChange::Keep
    };
    if new_key.is_none() && note.is_none() && value == ValueChange::Keep {
        return None;
    }
    Some(Operation::Edit {
        secret: SecretSelector::Id {
            id: current.id.clone(),
        },
        new_key,
        note,
        value,
    })
}

/// An operation on the selected secret, by id.
pub(super) fn selected(app: &App, build: fn(SecretSelector) -> Operation) -> Option<Operation> {
    let id = app.selected_secret()?.id.clone();
    Some(build(SecretSelector::Id { id }))
}

/// Run one operation with the terminal suspended, then report it in the status line.
pub(super) fn perform(term: &mut Term, app: &mut App, op: Operation) {
    match terminal::suspended(term, || send(op)).and_then(|sent| sent) {
        Ok(Ok(outcome)) => report(app, outcome),
        Ok(Err(refusal)) => app.set_error_message(refusal.to_string()),
        Err(error) => app.set_err(&error),
    }
}

/// Every TUI operation passes exactly the terminal. Values are typed into the daemon's prompt.
fn send(op: Operation) -> anyhow::Result<Result<Outcome, Refusal>> {
    let tty = open_terminal()?;
    connect_daemon()?.operate(op, &[tty.as_fd()])
}

fn report(app: &mut App, outcome: Outcome) {
    match outcome {
        Outcome::Added { meta } => {
            app.set_ok(format!("✓ created “{}”", meta.key));
            app.secrets.clear();
            app.mode = Mode::Menu;
        }
        Outcome::Edited { meta } => {
            app.set_ok(format!("✓ updated “{}”", meta.key));
            app.reload_secrets();
            app.mode = Mode::Search;
        }
        Outcome::Deleted { key, .. } => {
            app.set_ok(format!("✓ deleted “{key}”"));
            app.reload_secrets();
            app.mode = Mode::Search;
        }
        Outcome::Revealed { key } => {
            app.set_ok(format!("✓ revealed “{key}” on the terminal"));
            app.mode = Mode::Search;
        }
        Outcome::CopySent { key } => {
            app.set_ok(format!(
                "sent “{key}” to the terminal clipboard (OSC 52). Delivery is not confirmed. Clear it manually"
            ));
            app.mode = Mode::Search;
        }
    }
}
