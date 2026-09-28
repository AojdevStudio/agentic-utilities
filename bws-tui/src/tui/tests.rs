use super::*;
use anyhow::anyhow;
use std::ffi::OsStr;

#[cfg(target_os = "linux")]
#[test]
fn headless_copy_succeeds_without_a_display_server() {
    let mut app = App::new(vec![]);
    let mut output = Vec::new();
    let result = copy_value(
        &mut app,
        "dummy-clipboard-value".to_string(),
        &mut output,
        false,
    );
    assert_eq!(result.unwrap(), CopyMethod::Terminal);
    assert_eq!(output, b"\x1b]52;c;ZHVtbXktY2xpcGJvYXJkLXZhbHVl\x07");
    assert!(app.clipboard_clear_at.is_none());
}

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

#[test]
fn osc52_encodes_utf8_and_flushes() {
    #[derive(Default)]
    struct Output {
        bytes: Vec<u8>,
        flushes: usize,
    }

    impl std::io::Write for Output {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }

    let mut output = Output::default();
    write_osc52("café", &mut output).unwrap();
    assert_eq!(output.bytes, b"\x1b]52;c;Y2Fmw6k=\x07");
    assert_eq!(output.flushes, 1);
}

#[test]
fn display_detection_covers_x11_and_wayland() {
    assert!(!has_display_server(None, None));
    assert!(!has_display_server(Some(OsStr::new("")), None));
    assert!(has_display_server(Some(OsStr::new(":0")), None));
    assert!(has_display_server(None, Some(OsStr::new("wayland-0"))));
}

#[test]
fn terminal_copy_failure_names_both_clipboards() {
    struct FailingWriter;

    impl std::io::Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("terminal write failed"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut app = App::new(vec![]);
    let error = copy_value(
        &mut app,
        "dummy-clipboard-value".to_string(),
        &mut FailingWriter,
        false,
    )
    .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("native clipboard unavailable: no display server"));
    assert!(message.contains("terminal clipboard OSC 52 failed"));
    assert!(!message.contains("dummy-clipboard-value"));
}

#[test]
fn osc52_rejects_oversized_values_without_writing() {
    let mut output = Vec::new();
    let value = "x".repeat(128 * 1024 + 1);
    let error = write_osc52(&value, &mut output).unwrap_err();
    assert!(error.to_string().contains("exceeds 128 KiB"));
    assert!(output.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn failed_native_and_terminal_paths_are_both_reported() {
    if should_try_native_clipboard() {
        return;
    }

    struct FailingWriter;

    impl std::io::Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("terminal write failed"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut app = App::new(vec![]);
    let error = copy_value(
        &mut app,
        "dummy-clipboard-value".to_string(),
        &mut FailingWriter,
        true,
    )
    .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("native clipboard is unavailable"));
    assert!(message.contains("terminal clipboard OSC 52 failed"));
    assert!(!message.contains("dummy-clipboard-value"));
}

#[test]
fn terminal_fallback_preserves_an_earlier_native_clear() {
    let mut app = App::new(vec![]);
    let clear_at = Instant::now() + Duration::from_secs(30);
    app.clipboard_value = Some(SecretString::from("prior-copy".to_string()));
    app.clipboard_clear_at = Some(clear_at);

    let mut output = Vec::new();
    let method = copy_value(
        &mut app,
        "dummy-clipboard-value".to_string(),
        &mut output,
        false,
    )
    .unwrap();

    assert_eq!(method, CopyMethod::Terminal);
    assert_eq!(app.clipboard_clear_at, Some(clear_at));
    assert!(clipboard_holds_copied_value(
        "prior-copy",
        app.clipboard_value.as_ref().unwrap()
    ));
}
