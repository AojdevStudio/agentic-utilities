mod actions;
mod chrome;
mod events;
mod forms;
mod render;
mod secrets;
mod terminal;

use crate::bws::{self, Project, Secret};
use anyhow::{Context, Result};
use arboard::Clipboard;
use secrecy::{ExposeSecret, SecretString};
use std::time::{Duration, Instant};

const ACCENT: ratatui::style::Color = ratatui::style::Color::Cyan;
const DIM: ratatui::style::Style = ratatui::style::Style::new().fg(ratatui::style::Color::DarkGray);
const MENU: [&str; 2] = ["Add a secret", "Search / manage secrets"];
const ACTIONS: [&str; 5] = [
    "Copy value to clipboard",
    "Reveal value",
    "Edit secret",
    "Delete secret",
    "Cancel",
];

#[derive(Debug, PartialEq)]
enum Mode {
    Menu,
    AddProject,
    AddKey,
    AddValue,
    Search,
    ActionMenu,
    Edit,
    ConfirmDelete,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EditField {
    Key,
    Value,
    Note,
}

impl EditField {
    fn next(self) -> Self {
        match self {
            Self::Key => Self::Value,
            Self::Value => Self::Note,
            Self::Note => Self::Key,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Key => Self::Note,
            Self::Value => Self::Key,
            Self::Note => Self::Value,
        }
    }
}

struct App {
    mode: Mode,
    projects: Vec<Project>,
    secrets: Vec<Secret>,
    filtered: Vec<usize>, // indices into secrets
    menu_idx: usize,
    sel: usize, // selected row in the current list
    input: String,
    value_buf: String, // masked entry
    filter: String,
    edit_field: EditField,
    edit_key: String,
    edit_value: String,
    edit_note: String,
    status: String,
    status_err: bool,
    action_idx: usize,
    revealed: bool,
    clipboard_value: Option<SecretString>,
    clipboard_clear_at: Option<Instant>,
}

impl App {
    fn new(projects: Vec<Project>) -> Self {
        Self {
            mode: Mode::Menu,
            projects,
            secrets: Vec::new(),
            filtered: Vec::new(),
            menu_idx: 0,
            sel: 0,
            input: String::new(),
            value_buf: String::new(),
            filter: String::new(),
            edit_field: EditField::Key,
            edit_key: String::new(),
            edit_value: String::new(),
            edit_note: String::new(),
            status: String::new(),
            status_err: false,
            action_idx: 0,
            revealed: false,
            clipboard_value: None,
            clipboard_clear_at: None,
        }
    }

    fn set_ok(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_err = false;
    }

    fn set_err(&mut self, error: &anyhow::Error) {
        self.set_error_message(format!("{error:#}"));
    }

    fn set_error_message(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.status_err = true;
    }

    fn project_name(&self, id: &str) -> &str {
        self.projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.as_str())
            .unwrap_or("?")
    }

    fn selected_secret(&self) -> Option<&Secret> {
        self.filtered.get(self.sel).map(|&i| &self.secrets[i])
    }

    fn refilter(&mut self) {
        self.filtered = (0..self.secrets.len())
            .filter(|&i| fuzzy_match(&self.secrets[i].key, &self.filter))
            .collect();
        self.sel = self.sel.min(self.filtered.len().saturating_sub(1));
    }

    fn reload_secrets(&mut self) -> bool {
        match bws::list_secrets(None) {
            Ok(secrets) => {
                self.secrets = secrets;
                self.refilter();
                true
            }
            Err(error) => {
                self.set_err(&error);
                false
            }
        }
    }

    fn open_add_with(&mut self, projects: Result<Vec<Project>>) {
        match projects {
            Ok(projects) if !projects.is_empty() => {
                self.projects = projects;
                self.sel = 0;
                self.mode = Mode::AddProject;
            }
            Ok(_) => self.set_error_message("no accessible BWS projects"),
            Err(error) => self.set_err(&error),
        }
    }

    fn clear_clipboard_if_due(&mut self) {
        self.clear_clipboard_if_due_with(Instant::now(), clear_clipboard_value);
    }

    fn clear_clipboard_if_due_with(
        &mut self,
        now: Instant,
        clear: impl FnOnce(&SecretString) -> Result<bool>,
    ) {
        if self.clipboard_clear_at.is_none_or(|at| now < at) {
            return;
        }
        let Some(copied) = self.clipboard_value.as_ref() else {
            return;
        };
        match clear(copied) {
            Ok(cleared) => {
                self.clipboard_value = None;
                self.clipboard_clear_at = None;
                if cleared {
                    self.set_ok("✓ clipboard cleared");
                }
            }
            Err(error) => {
                self.clipboard_clear_at = Some(now + Duration::from_secs(1));
                self.set_err(&error);
            }
        }
    }
}

fn fuzzy_match(hay: &str, needle: &str) -> bool {
    let mut n = needle.chars().flat_map(char::to_lowercase);
    let mut cur = n.next();
    for c in hay.chars().flat_map(char::to_lowercase) {
        if Some(c) == cur {
            cur = n.next();
        }
    }
    cur.is_none()
}

fn clipboard_holds_copied_value(current: &str, copied: &SecretString) -> bool {
    current == copied.expose_secret()
}

fn clear_clipboard_value(copied: &SecretString) -> Result<bool> {
    let mut clipboard = Clipboard::new().context("macOS clipboard is unavailable")?;
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

fn copy_with_autoclear(app: &mut App, value: String) -> Result<()> {
    let mut clipboard = Clipboard::new().context("macOS clipboard is unavailable")?;
    clipboard
        .set_text(value.clone())
        .context("failed to copy the secret value")?;
    app.clipboard_value = Some(SecretString::from(value));
    app.clipboard_clear_at = Some(Instant::now() + Duration::from_secs(30));
    Ok(())
}

pub fn run() -> Result<()> {
    let mut app = App::new(bws::list_projects()?);
    terminal::run(&mut app)
}

#[cfg(test)]
mod tests;
