use super::App;
use anyhow::{Context, Result};
use arboard::Clipboard;
use base64::{engine::general_purpose::STANDARD, Engine};
use secrecy::{ExposeSecret, SecretString};
use std::{
    ffi::OsStr,
    io::Write,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

/// Reports whether the clipboard still holds the value hush copied.
pub(super) fn clipboard_holds_copied_value(current: &str, copied: &SecretString) -> bool {
    current == copied.expose_secret()
}

/// Clears a native copy only when it has not been replaced.
pub(super) fn clear_clipboard_value(copied: &SecretString) -> Result<bool> {
    let mut clipboard = Clipboard::new().context("native clipboard is unavailable")?;
    let current = clipboard
        .get_text()
        .context("failed to read the clipboard")?;
    if !clipboard_holds_copied_value(&current, copied) {
        return Ok(false);
    }
    clipboard
        .set_text(String::new())
        .context("failed to clear the clipboard")?;
    Ok(true)
}

#[derive(Debug, PartialEq)]
pub(super) enum CopyMethod {
    Native,
    Terminal,
}

const MAX_OSC52_VALUE_BYTES: usize = 128 * 1024;

/// Detects a configured X11 or Wayland display.
pub(super) fn has_display_server(display: Option<&OsStr>, wayland: Option<&OsStr>) -> bool {
    display.is_some_and(|value| !value.is_empty()) || wayland.is_some_and(|value| !value.is_empty())
}

/// Skips arboard on headless Linux while keeping the native path elsewhere.
pub(super) fn should_try_native_clipboard() -> bool {
    !cfg!(target_os = "linux")
        || has_display_server(
            std::env::var_os("DISPLAY").as_deref(),
            std::env::var_os("WAYLAND_DISPLAY").as_deref(),
        )
}

/// Sends a bounded OSC 52 clipboard write through the active terminal output.
pub(super) fn write_osc52(value: &str, output: &mut impl Write) -> Result<()> {
    anyhow::ensure!(
        value.len() <= MAX_OSC52_VALUE_BYTES,
        "terminal clipboard OSC 52 value exceeds 128 KiB"
    );
    let encoded = Zeroizing::new(STANDARD.encode(value));
    output.write_all(b"\x1b]52;c;")?;
    output.write_all(encoded.as_bytes())?;
    output.write_all(b"\x07")?;
    output
        .flush()
        .context("failed to flush terminal clipboard")?;
    Ok(())
}

/// Uses the native clipboard first and falls back to OSC 52 when unavailable.
/// Only native copies schedule a conditional clear.
pub(super) fn copy_value(
    app: &mut App,
    value: String,
    output: &mut impl Write,
    try_native: bool,
) -> Result<CopyMethod> {
    let native_error = if try_native {
        match Clipboard::new()
            .context("native clipboard is unavailable")
            .and_then(|mut clipboard| {
                clipboard
                    .set_text(value.clone())
                    .context("failed to copy using native clipboard")
            }) {
            Ok(()) => {
                app.clipboard_value = Some(SecretString::from(value));
                app.clipboard_clear_at = Some(Instant::now() + Duration::from_secs(30));
                return Ok(CopyMethod::Native);
            }
            Err(error) => error,
        }
    } else {
        anyhow::anyhow!("native clipboard unavailable: no display server")
    };

    write_osc52(&value, output).map_err(|error| {
        anyhow::anyhow!("{native_error:#}; terminal clipboard OSC 52 failed: {error:#}")
    })?;
    // OSC 52 cannot read back the clipboard. Keep any earlier native clear pending.
    Ok(CopyMethod::Terminal)
}
