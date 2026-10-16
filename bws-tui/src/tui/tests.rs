use super::ops::{add_operation, edit_operation};
use super::*;
use crate::broker::operation::{Operation, SecretSelector, ValueChange, ValueSource};
use crate::tui::terminal::is_interactive;
use anyhow::anyhow;
use ratatui::{backend::TestBackend, Terminal};

fn project(id: &str, name: &str) -> ProjectMeta {
    ProjectMeta {
        id: id.into(),
        name: name.into(),
    }
}

fn app_with_secret() -> App {
    let mut app = App::new(vec![project("p1", "API Keys")]);
    app.secrets = vec![SecretMeta {
        id: "s1".into(),
        key: "TOKEN".into(),
        project: Some("API Keys".into()),
        project_id: Some("p1".into()),
        note: "scope: test".into(),
    }];
    app.refilter();
    app
}

#[test]
fn a_metadata_edit_keeps_the_stored_value() {
    let mut app = app_with_secret();
    actions::edit_action(&mut app);
    assert_eq!(app.mode, Mode::Edit);
    assert_eq!(
        edit_operation(&app),
        None,
        "an untouched form changes nothing"
    );
    app.edit_note = "rotated".into();
    assert_eq!(
        edit_operation(&app),
        Some(Operation::Edit {
            secret: SecretSelector::Id { id: "s1".into() },
            new_key: None,
            note: Some("rotated".into()),
            value: ValueChange::Keep,
        })
    );
}

#[test]
fn replacing_the_value_is_an_explicit_choice() {
    let mut app = app_with_secret();
    actions::edit_action(&mut app);
    app.replace_value = true;
    assert_eq!(
        edit_operation(&app),
        Some(Operation::Edit {
            secret: SecretSelector::Id { id: "s1".into() },
            new_key: None,
            note: None,
            value: ValueChange::Replace(ValueSource::TtyPrompt),
        })
    );
}

#[test]
fn add_sends_the_chosen_project_and_lets_the_daemon_prompt() {
    let mut app = App::new(vec![project("p1", "API Keys"), project("p2", "Logins")]);
    app.sel = 1;
    app.input = "  NEW_TOKEN ".into();
    assert_eq!(
        add_operation(&app),
        Some(Operation::Add {
            project: "p2".into(),
            key: "NEW_TOKEN".into(),
            note: None,
            value: ValueSource::TtyPrompt,
        })
    );
}

#[test]
fn search_and_actions_render_metadata() {
    let mut app = app_with_secret();
    app.mode = Mode::ActionMenu;
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal.draw(|f| render::draw(f, &app)).unwrap();
    let screen: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(screen.contains("TOKEN"));
    for (_, label) in ACTIONS {
        assert!(screen.contains(label), "missing action {label}");
    }
}

#[test]
fn add_refreshes_projects_before_opening_the_picker() {
    let mut app = App::new(vec![project("stale", "Stale")]);
    app.open_add_with(Ok(vec![project("live", "Live")]));
    assert_eq!(app.projects[0].id, "live");
    assert_eq!(app.mode, Mode::AddProject);

    app.mode = Mode::Menu;
    app.open_add_with(Ok(vec![]));
    assert_eq!(app.mode, Mode::Menu);
    assert!(app.status_err);
}

#[test]
fn edit_fields_cycle_without_invalid_numeric_states() {
    assert_eq!(EditField::Key.next(), EditField::Value);
    assert_eq!(EditField::Value.next(), EditField::Note);
    assert_eq!(EditField::Note.next(), EditField::Key);
    assert_eq!(EditField::Key.previous(), EditField::Note);
}

#[test]
/// Cover both one-sided terminal sessions and the fully headless rejection.
fn tui_gate_accepts_a_tty_on_either_side() {
    assert!(is_interactive(true, false));
    assert!(is_interactive(false, true));
    assert!(is_interactive(true, true));
    assert!(!is_interactive(false, false));
}

#[test]
fn terminal_cleanup_reports_every_failure() {
    let error = terminal::finish_terminal(
        Err(anyhow!("event failed")),
        Err(anyhow!("raw failed")),
        Err(anyhow!("screen failed")),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("event failed"));
    assert!(error.contains("raw failed"));
    assert!(error.contains("screen failed"));
}
