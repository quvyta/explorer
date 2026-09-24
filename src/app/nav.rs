//! Moving between folders: going to a place or a typed path, back and forward through the history,
//! and up, with the file manager's own `Enter` and `Leave`.

use std::path::{Path, PathBuf};

use qframe::prelude::*;
use qframe::widgets::{FileManagerMsg, FileManagerState, Toast, child_key, is_within, name_of, parent_key};

/// Whether `key` is `folder` or below it; everything is below the root, whose key is empty.
pub(super) fn within(key: &str, folder: &str) -> bool {
    folder == ROOT || is_within(key, folder)
}

use super::{Explorer, Goal, Location, Msg};
use crate::cli::Start;

/// The key of the root folder.
const ROOT: &str = FileManagerState::ROOT;

/// The keys of the folders above `key`, from the top down, the root and `key` itself left out.
fn above(key: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut at = parent_key(key);
    while at != ROOT {
        keys.push(at.to_owned());
        at = parent_key(at);
    }
    keys.reverse();
    keys
}

impl Explorer {
    /// The file manager's key of `path`, or `None` for a path outside its root.
    pub(super) fn key_of(&self, path: &Path) -> Option<String> {
        let rest = path.strip_prefix(&self.machine.root).ok()?;
        let parts: Vec<String> = rest.iter().map(|part| part.to_string_lossy().into_owned()).collect();
        Some(parts.join("/"))
    }

    /// `path` as a person reads it: under the home folder it starts with `~`.
    pub(super) fn shown(&self, path: &Path) -> String {
        match path.strip_prefix(&self.machine.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        }
    }

    /// Goes to the folder at `path`, with the cursor on `select`, as a new step of the history.
    pub(super) fn go_to_path(&mut self, path: &Path, select: Option<String>) -> Command<Msg> {
        match self.key_of(path) {
            Some(key) => self.go(key, select, true),
            None => Command::toast(Toast::warning(t!("explorer.go.outside", path = self.shown(path))).key("vanished")),
        }
    }

    /// Goes to the folder `key`: the folders above it are opened and read, and once they are the
    /// manager steps into it. With `record`, where the person was goes onto the back history and
    /// the forward history is let go, as in a browser.
    pub(super) fn go(&mut self, key: String, select: Option<String>, record: bool) -> Command<Msg> {
        let current = self.files.folder().to_owned();
        if record && key != current {
            self.back.push(current);
            self.forward.clear();
        }
        let mut commands = Vec::new();
        for folder in above(&key) {
            if !(self.files.is_open(&folder) && self.files.children(&folder).is_some()) {
                commands.push(self.files.update(FileManagerMsg::Expand(folder, true), Msg::Files));
            }
        }
        self.goal = Some(Goal { key, select, reread: false });
        commands.push(self.settle_goal());
        Command::batch(commands)
    }

    /// Steps into the folder of the goal once every folder above it has been read. A folder that
    /// is no longer there, or that may not be looked into, leaves the person in the nearest one
    /// above it that is, with a note saying why.
    pub(super) fn settle_goal(&mut self) -> Command<Msg> {
        let Some(goal) = self.goal.clone() else { return Command::none() };
        let mut at = ROOT.to_owned();
        for part in goal.key.split('/').filter(|part| !part.is_empty()) {
            let Some(entries) = self.files.children(&at) else { return Command::none() };
            if !entries.iter().any(|entry| entry.folder && entry.name == part) {
                // A folder read again has not answered yet; its old entries say nothing.
                if self.files.is_loading(&at) {
                    return Command::none();
                }
                // A listing may be older than the folder asked for; it is read once more before the
                // folder is taken to be gone.
                if !goal.reread && self.files.folder_error(&at).is_none() {
                    self.goal = Some(Goal { reread: true, ..goal });
                    return self.files.update(FileManagerMsg::Refresh, Msg::Files);
                }
                let missing = self.files.path(&goal.key);
                let words = if self.files.folder_error(&at).is_some() {
                    t!("explorer.go.unreadable", path = self.shown(&self.files.path(&at)))
                } else {
                    t!("explorer.go.vanished", path = self.shown(&missing))
                };
                let enter = self.arrive(at, None);
                return Command::batch([enter, Command::toast(Toast::warning(words).key("vanished"))]);
            }
            at = child_key(&at, part);
        }
        self.arrive(goal.key, goal.select)
    }

    /// Shows the folder `key`, which the manager knows to be a folder, with the cursor on `select`.
    fn arrive(&mut self, key: String, select: Option<String>) -> Command<Msg> {
        self.goal = None;
        let command = self.files.update(FileManagerMsg::Enter(key.clone()), Msg::Files);
        if let Some(name) = select {
            self.files.select(&child_key(&key, &name));
        }
        self.here = key;
        Command::batch([command, Command::focus(super::view::FILES)])
    }

    /// Hands a message to the file manager, keeping the history in step with the folders its rows
    /// step into and out of, and noticing when the folder shown was taken away.
    pub(super) fn files_message(&mut self, message: FileManagerMsg) -> Command<Msg> {
        let stepping = matches!(message, FileManagerMsg::Enter(_) | FileManagerMsg::Leave);
        if stepping {
            self.back.push(self.files.folder().to_owned());
            self.forward.clear();
        }
        let command = self.files.update(message, Msg::Files);
        let now = self.files.folder().to_owned();
        let mut notice = Command::none();
        if now != self.here {
            // The manager leaves a folder another program deleted for the one above it; qexp says
            // so, since the person did not ask to go anywhere.
            if !stepping && self.goal.is_none() && within(&self.here, &now) {
                let gone = self.shown(&self.files.path(&self.here));
                notice = Command::toast(Toast::warning(t!("explorer.go.vanished", path = gone)).key("vanished"));
            }
            self.here = now;
        }
        Command::batch([command, notice, self.settle_goal()])
    }

    /// Back through the history. Going back to a folder above the one left puts the cursor on the
    /// way down again.
    pub(super) fn back(&mut self) -> Command<Msg> {
        let Some(key) = self.back.pop() else { return Command::none() };
        let current = self.files.folder().to_owned();
        let select = Self::way_down(&key, &current);
        self.forward.push(current);
        self.go(key, select, false)
    }

    /// Forward through the history.
    pub(super) fn forward(&mut self) -> Command<Msg> {
        let Some(key) = self.forward.pop() else { return Command::none() };
        let current = self.files.folder().to_owned();
        let select = Self::way_down(&key, &current);
        self.back.push(current);
        self.go(key, select, false)
    }

    /// The name of the folder of `key` that leads down to `from`, when `from` is below `key`.
    fn way_down(key: &str, from: &str) -> Option<String> {
        if from == key || !within(from, key) {
            return None;
        }
        let below = if key == ROOT { from } else { &from[key.len() + 1..] };
        below.split('/').next().map(str::to_owned)
    }

    /// Up to the folder above, with the cursor on the folder that was left.
    pub(super) fn up(&mut self) -> Command<Msg> {
        let current = self.files.folder().to_owned();
        if current == ROOT {
            return Command::none();
        }
        self.go(parent_key(&current).to_owned(), Some(name_of(&current).to_owned()), true)
    }

    /// The parts of the path bar, each with the key it goes to: the home folder and what is below
    /// it, or the root and what is below it.
    pub(super) fn crumbs(&self) -> Vec<(String, String)> {
        let folder = self.files.folder();
        let home = self.key_of(&self.machine.home).filter(|home| within(folder, home));
        let (mut crumbs, start) = match home {
            Some(home) => (vec![(t!("explorer.place.home"), home.clone())], home),
            None => (vec![(t!("explorer.place.root"), ROOT.to_owned())], ROOT.to_owned()),
        };
        let below = if folder == start {
            ""
        } else if start == ROOT {
            folder
        } else {
            &folder[start.len() + 1..]
        };
        let mut at = start;
        for part in below.split('/').filter(|part| !part.is_empty()) {
            at = child_key(&at, part);
            crumbs.push((part.to_owned(), at.clone()));
        }
        crumbs
    }

    /// A part of the path bar was clicked.
    pub(super) fn crumb(&mut self, index: usize) -> Command<Msg> {
        let Some((_, key)) = self.crumbs().into_iter().nth(index) else { return Command::none() };
        let current = self.files.folder().to_owned();
        let select = Self::way_down(&key, &current);
        self.go(key, select, true)
    }

    /// Turns the path bar into a text field holding the folder's path, or back.
    pub(super) fn location(&mut self, open: bool) -> Command<Msg> {
        if !open {
            self.location = None;
            return Command::focus(super::view::FILES);
        }
        let value = self.folder().display().to_string();
        self.location = Some(Location { value, error: None });
        Command::focus(super::view::LOCATION)
    }

    /// Works out where the typed path leads, off the drawing thread: `~` is the home folder, and a
    /// relative path starts at the folder shown.
    pub(super) fn check_location(&mut self, value: &str) -> Command<Msg> {
        let typed = value.trim();
        let path = match typed.strip_prefix('~') {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => {
                self.machine.home.join(rest.trim_start_matches('/'))
            }
            _ if typed.starts_with('/') => PathBuf::from(typed),
            _ => self.folder().join(typed),
        };
        let root = self.machine.root.clone();
        Command::perform(move || Msg::LocationChecked(resolve(&path, &root)))
    }

    /// The typed path was worked out: it is gone to, or the field says why not and nothing else
    /// changes.
    pub(super) fn location_checked(&mut self, result: Option<Start>) -> Command<Msg> {
        match result {
            Some(start) => {
                self.location = None;
                self.go_to_path(&start.folder, start.select)
            }
            None => {
                if let Some(location) = &mut self.location {
                    location.error = Some(t!("explorer.go.not-there", path = location.value.trim()));
                }
                Command::none()
            }
        }
    }
}

/// Where the path `path` leads under `root`: a folder, or a file's folder with the file selected;
/// `None` when nothing is there.
fn resolve(path: &Path, root: &Path) -> Option<Start> {
    let resolved = std::fs::canonicalize(path).ok().filter(|resolved| resolved.starts_with(root))?;
    if resolved.is_dir() {
        return Some(Start { folder: resolved, select: None });
    }
    let name = resolved.file_name().map(|name| name.to_string_lossy().into_owned());
    let folder = resolved.parent().map_or_else(|| root.to_path_buf(), Path::to_path_buf);
    Some(Start { folder, select: name })
}
