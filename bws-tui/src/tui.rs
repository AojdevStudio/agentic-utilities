mod actions;
mod chrome;
mod events;
mod forms;
mod ops;
mod render;
mod secrets;
mod terminal;

use crate::broker::meta::{ProjectMeta, SecretMeta};
use crate::human_cmd::connect_daemon;
use anyhow::Result;

const ACCENT: ratatui::style::Color = ratatui::style::Color::Cyan;
const DIM: ratatui::style::Style = ratatui::style::Style::new().fg(ratatui::style::Color::DarkGray);
const MENU: [&str; 2] = ["Add a secret", "Search / manage secrets"];

/// One entry in the secret action menu. The menu order is [`ACTIONS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Copy,
    Reveal,
    Edit,
    Delete,
    Cancel,
}

const ACTIONS: [(Action, &str); 5] = [
    (Action::Copy, "Copy value to clipboard"),
    (Action::Reveal, "Reveal value"),
    (Action::Edit, "Edit secret"),
    (Action::Delete, "Delete secret"),
    (Action::Cancel, "Cancel"),
];

#[derive(Debug, PartialEq)]
enum Mode {
    Menu,
    AddProject,
    AddKey,
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

/// TUI state. Metadata only: the daemon owns every value.
struct App {
    mode: Mode,
    projects: Vec<ProjectMeta>,
    secrets: Vec<SecretMeta>,
    filtered: Vec<usize>, // indices into secrets
    menu_idx: usize,
    sel: usize, // selected row in the current list
    input: String,
    filter: String,
    edit_field: EditField,
    edit_key: String,
    edit_note: String,
    /// An edit keeps the stored value unless the user asks to replace it.
    replace_value: bool,
    status: String,
    status_err: bool,
    action_idx: usize,
}

impl App {
    fn new(projects: Vec<ProjectMeta>) -> Self {
        Self {
            mode: Mode::Menu,
            projects,
            secrets: Vec::new(),
            filtered: Vec::new(),
            menu_idx: 0,
            sel: 0,
            input: String::new(),
            filter: String::new(),
            edit_field: EditField::Key,
            edit_key: String::new(),
            edit_note: String::new(),
            replace_value: false,
            status: String::new(),
            status_err: false,
            action_idx: 0,
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

    fn selected_secret(&self) -> Option<&SecretMeta> {
        self.filtered.get(self.sel).map(|&i| &self.secrets[i])
    }

    fn refilter(&mut self) {
        self.filtered = (0..self.secrets.len())
            .filter(|&i| fuzzy_match(&self.secrets[i].key, &self.filter))
            .collect();
        self.sel = self.sel.min(self.filtered.len().saturating_sub(1));
    }

    fn reload_secrets(&mut self) -> bool {
        match connect_daemon().and_then(|mut client| client.catalog(None)) {
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

    fn open_add_with(&mut self, projects: Result<Vec<ProjectMeta>>) {
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

/// Without a daemon the TUI does not start. The client has no vault access.
pub fn run() -> Result<()> {
    let mut app = App::new(connect_daemon()?.projects()?);
    terminal::run(&mut app)
}

#[cfg(test)]
mod tests;
