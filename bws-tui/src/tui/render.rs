use super::{chrome::*, forms, secrets, App, Mode};

pub(super) fn draw(frame: &mut ratatui::Frame, app: &App) {
    let chunks = layout(frame.area());
    draw_header(frame, chunks[0], app);
    let hints = hints(&app.mode);
    let f = frame;

    match app.mode {
        Mode::Menu => forms::draw_menu(f, chunks[1], app),
        Mode::AddProject => forms::draw_add_project(f, chunks[1], app),
        Mode::AddKey => forms::draw_add_key(f, chunks[1], app),
        Mode::AddValue => forms::draw_add_value(f, chunks[1], app),
        Mode::Search => secrets::draw_search(f, chunks[1], app),
        Mode::ActionMenu => secrets::draw_action_menu(f, chunks[1], app),
        Mode::Edit => forms::draw_edit(f, chunks[1], app),
        Mode::ConfirmDelete => secrets::draw_confirm_delete(f, chunks[1], app),
    }

    draw_footer(f, chunks[2], app, hints);
}
