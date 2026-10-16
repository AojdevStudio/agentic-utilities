use super::ops::{perform, selected, Term};
use super::*;
use crate::broker::operation::{Operation, SecretSelector};

pub(super) fn edit_action(app: &mut App) {
    if let Some(s) = app.selected_secret().cloned() {
        app.edit_key = s.key;
        app.edit_note = s.note;
        app.replace_value = false;
        app.edit_field = EditField::Key;
        app.mode = Mode::Edit;
    }
}

/// Copy and reveal hand the terminal to the daemon for approval and disclosure.
pub(super) fn run_action(term: &mut Term, app: &mut App, action: Action) {
    match action {
        Action::Copy => disclose(term, app, |secret| Operation::Copy { secret }),
        Action::Reveal => disclose(term, app, |secret| Operation::Reveal { secret }),
        Action::Edit => edit_action(app),
        Action::Delete => app.mode = Mode::ConfirmDelete,
        Action::Cancel => app.mode = Mode::Search,
    }
}

fn disclose(term: &mut Term, app: &mut App, build: fn(SecretSelector) -> Operation) {
    match selected(app, build) {
        Some(op) => perform(term, app, op),
        None => app.mode = Mode::Search,
    }
}

pub(super) fn edit_buf(app: &mut App) -> Option<&mut String> {
    match app.edit_field {
        EditField::Key => Some(&mut app.edit_key),
        EditField::Value => None,
        EditField::Note => Some(&mut app.edit_note),
    }
}
