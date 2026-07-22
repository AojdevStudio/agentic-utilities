use super::*;

pub(super) fn copy_action(app: &mut App) {
    if let Some(s) = app.selected_secret() {
        let key = s.key.clone();
        let value = s.value.clone();
        match copy_with_autoclear(app, value) {
            Ok(()) => app.set_ok(format!("✓ copied “{key}” — clears in 30s (keep app open)")),
            Err(error) => app.set_err(&error),
        }
    }
    app.mode = Mode::Search;
}

pub(super) fn edit_action(app: &mut App) {
    if let Some(s) = app.selected_secret().cloned() {
        app.edit_key = s.key;
        app.edit_value = s.value;
        app.edit_note = s.note;
        app.edit_field = EditField::Key;
        app.mode = Mode::Edit;
    }
}

pub(super) fn run_action(app: &mut App) {
    match app.action_idx {
        0 => copy_action(app),
        1 => app.revealed = true,
        2 => edit_action(app),
        3 => app.mode = Mode::ConfirmDelete,
        _ => app.mode = Mode::Search,
    }
}

pub(super) fn edit_buf(app: &mut App) -> &mut String {
    match app.edit_field {
        EditField::Key => &mut app.edit_key,
        EditField::Value => &mut app.edit_value,
        EditField::Note => &mut app.edit_note,
    }
}
