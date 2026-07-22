use super::{events, App};
use anyhow::{anyhow, Context, Result};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Write};

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
    let mut guard = TerminalGuard::default();
    guard.enable_raw()?;
    let mut output = io::stdout();
    guard.enter_alternate(&mut output)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(output))?;
    let event = events::event_loop(&mut terminal, app);
    let (raw, screen) = guard.restore(terminal.backend_mut());
    finish_terminal(event, raw, screen)
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
