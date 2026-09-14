use super::{events, App};
use anyhow::{anyhow, bail, Context, Result};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, IsTerminal, Write};

#[derive(Default)]
struct TerminalGuard {
    raw: bool,
    alternate: bool,
}

impl TerminalGuard {
    fn enable_raw(&mut self) -> Result<()> {
        self.raw = true;
        enable_raw_mode().context("failed to enable terminal input mode")
    }

    fn enter_alternate(&mut self, output: &mut impl Write) -> Result<()> {
        self.alternate = true;
        execute!(output, EnterAlternateScreen).context("failed to enter alternate terminal screen")
    }

    fn restore(&mut self, output: &mut impl Write) -> (Result<()>, Result<()>) {
        let raw = disable_raw_mode().context("failed to restore terminal input mode");
        if raw.is_ok() {
            self.raw = false;
        }
        let screen = execute!(output, LeaveAlternateScreen)
            .context("failed to leave alternate terminal screen");
        if screen.is_ok() {
            self.alternate = false;
        }
        (raw, screen)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.raw {
            let _ = disable_raw_mode();
        }
        if self.alternate {
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
        }
    }
}

pub(super) fn run(app: &mut App) -> Result<()> {
    if !is_interactive(io::stdin().is_terminal(), io::stdout().is_terminal()) {
        bail!(
            "hush's interactive TUI needs a terminal session (no TTY found). \
             Run `hush` from an interactive terminal, or use the script-friendly \
             subcommands: `hush list`, `hush get --key <KEY>`, `hush exec --key <KEY> -- <cmd>`"
        );
    }
    let mut guard = TerminalGuard::default();
    guard.enable_raw()?;
    let mut output = io::stdout();
    guard.enter_alternate(&mut output)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(output))?;
    let event = events::event_loop(&mut terminal, app);
    let (raw, screen) = guard.restore(terminal.backend_mut());
    finish_terminal(event, raw, screen)
}

/// The TUI needs either side attached to a terminal; crossterm opens /dev/tty
/// directly for input when stdin is piped, so stdout alone is enough.
pub(super) fn is_interactive(stdin_tty: bool, stdout_tty: bool) -> bool {
    stdin_tty || stdout_tty
}

pub(super) fn finish_terminal(
    event: Result<()>,
    raw: Result<()>,
    screen: Result<()>,
) -> Result<()> {
    let errors = [
        ("event loop", event),
        ("input mode", raw),
        ("screen", screen),
    ]
    .into_iter()
    .filter_map(|(label, result)| result.err().map(|error| format!("{label}: {error:#}")))
    .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(errors.join("; ")))
    }
}
