use super::*;
use anyhow::anyhow;

#[test]
fn clipboard_clear_only_targets_the_value_hush_copied() {
    let copied = SecretString::from("secret-value".to_string());
    assert!(clipboard_holds_copied_value("secret-value", &copied));
    assert!(!clipboard_holds_copied_value("newer-user-copy", &copied));
}

#[test]
fn clipboard_clear_retries_after_a_transient_failure() {
    let now = Instant::now();
    let mut app = App::new(vec![]);
    app.clipboard_value = Some(SecretString::from("secret-value".to_string()));
    app.clipboard_clear_at = Some(now);

    app.clear_clipboard_if_due_with(now, |_| Err(anyhow!("clipboard busy")));
    assert!(app.clipboard_value.is_some());
    assert!(app.clipboard_clear_at.is_some_and(|retry| retry > now));

    let retry_at = app.clipboard_clear_at.unwrap();
    app.clear_clipboard_if_due_with(retry_at, |_| Ok(true));
    assert!(app.clipboard_value.is_none());
    assert!(app.clipboard_clear_at.is_none());
}

#[test]
fn add_refreshes_projects_before_opening_the_picker() {
    let mut app = App::new(vec![Project {
        id: "stale".into(),
        name: "Stale".into(),
    }]);
    app.open_add_with(Ok(vec![Project {
        id: "live".into(),
        name: "Live".into(),
    }]));
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
