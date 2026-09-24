//! The favourites on the screen: the person's own folders below the places, added from a
//! folder's menu, taken out and moved from their own menu or with alt+shift and the arrows, and
//! gone to like a place. The list on disk is `crate::favourites`'s.

use std::path::{Path, PathBuf};

use qframe::prelude::*;
use qframe::widgets::{ContextItem, Toast};

use super::{Explorer, Msg};
use crate::favourites as store;

/// Which way a favourite moves in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Towards {
    /// One place nearer the top.
    Up,
    /// One place nearer the bottom.
    Down,
}

/// One favourite as the sidebar draws it.
#[derive(Debug, Clone)]
pub(super) struct Favourite {
    /// The folder.
    pub(super) path: PathBuf,
    /// Whether the folder was not there when last looked at; the row is drawn faint.
    pub(super) missing: bool,
}

impl Favourite {
    /// The favourite of `path`, looked at now.
    fn new(path: PathBuf) -> Self {
        let missing = !path.is_dir();
        Self { path, missing }
    }

    /// The key of its row in the sidebar: its path, which is in the list only once.
    pub(super) fn key(&self) -> String {
        key_of(&self.path)
    }

    /// The name the sidebar shows: the folder's own.
    pub(super) fn name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(|| self.path.display().to_string(), |name| name.to_string_lossy().into_owned())
    }
}

/// The sidebar key of the favourite at `path`.
pub(super) fn key_of(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The entries of a favourite row's menu: taking it out and moving it, the moves that cannot
/// happen at the ends shown but not open to choose.
pub(super) fn row_menu(paths: &[PathBuf], key: &str) -> Vec<ContextItem<Msg>> {
    let Some(index) = paths.iter().position(|path| key_of(path) == key) else { return Vec::new() };
    let path = &paths[index];
    vec![
        ContextItem::new(t!("explorer.favourites.up"), Msg::MoveFavourite(Some(path.clone()), Towards::Up))
            .shortcut("alt+shift+↑")
            .disabled(index == 0),
        ContextItem::new(t!("explorer.favourites.down"), Msg::MoveFavourite(Some(path.clone()), Towards::Down))
            .shortcut("alt+shift+↓")
            .disabled(index + 1 == paths.len()),
        ContextItem::gap(),
        ContextItem::new(t!("explorer.favourites.remove"), Msg::RemoveFavourite(path.clone())),
    ]
}

/// The entry a folder's own menu adds: "Add to favourites", or, for a folder that is one already,
/// "Remove from favourites", so the same folder is never added twice.
pub(super) fn folder_item(paths: &[PathBuf], path: &Path) -> Option<ContextItem<Msg>> {
    if paths.iter().any(|favourite| favourite == path) {
        return Some(ContextItem::new(t!("explorer.favourites.remove"), Msg::RemoveFavourite(path.to_path_buf())));
    }
    store::storable(path)
        .then(|| ContextItem::new(t!("explorer.favourites.add"), Msg::AddFavourite(path.to_path_buf())))
}

impl Explorer {
    /// Reads the list at start, taking in the desktop's GTK bookmarks when qexp has never kept one.
    pub(super) fn load_favourites(&mut self) -> Command<Msg> {
        let Some(file) = self.machine.favourites.clone() else { return Command::none() };
        let gtk = self.machine.xdg.config_home.as_ref().map(|config| config.join("gtk-3.0").join("bookmarks"));
        let loaded = store::load(&file, gtk.as_deref());
        self.favourites = loaded.paths.into_iter().map(Favourite::new).collect();
        loaded.error.map_or_else(Command::none, |error| Self::not_saved(&error))
    }

    /// The paths of the favourites, in their order.
    pub(super) fn favourite_paths(&self) -> Vec<PathBuf> {
        self.favourites.iter().map(|favourite| favourite.path.clone()).collect()
    }

    /// Writes the list. It is a few lines, written whole and at once rather than in the
    /// background: two quick changes then land in the order they were made.
    fn save_favourites(&self) -> Command<Msg> {
        let Some(file) = &self.machine.favourites else { return Command::none() };
        match store::write(file, &self.favourite_paths()) {
            Ok(()) => Command::none(),
            Err(error) => Self::not_saved(&error),
        }
    }

    /// The note for a list that could not be written.
    fn not_saved(error: &std::io::Error) -> Command<Msg> {
        Command::toast(Toast::warning(t!("explorer.favourites.not-saved")).body(error.to_string()).key("favourites"))
    }

    /// Adds the folder at `path` at the end of the list, unless it is there already.
    pub(super) fn add_favourite(&mut self, path: PathBuf) -> Command<Msg> {
        if !store::storable(&path) || self.favourites.iter().any(|favourite| favourite.path == path) {
            return Command::none();
        }
        self.favourites.push(Favourite::new(path));
        self.save_favourites()
    }

    /// Takes the favourite at `path` out of the list; the folder itself stays where it is.
    pub(super) fn remove_favourite(&mut self, path: &Path) -> Command<Msg> {
        let before = self.favourites.len();
        self.favourites.retain(|favourite| favourite.path != path);
        if self.favourites.len() == before {
            return Command::none();
        }
        if self.favourite_cursor.as_deref() == Some(key_of(path).as_str()) {
            self.favourite_cursor = None;
        }
        self.save_favourites()
    }

    /// Moves a favourite one place: the one at `path`, or the one the keyboard rests on in the
    /// sidebar. The keyboard stays on it, so pressing again moves it further.
    pub(super) fn move_favourite(&mut self, path: Option<PathBuf>, towards: Towards) -> Command<Msg> {
        let key = match path {
            Some(path) => key_of(&path),
            None => match self.favourite_cursor.clone().or_else(|| self.current_favourite()) {
                Some(key) => key,
                None => return Command::none(),
            },
        };
        let Some(from) = self.favourites.iter().position(|favourite| favourite.key() == key) else {
            return Command::none();
        };
        let to = match towards {
            Towards::Up => from.checked_sub(1),
            Towards::Down => Some(from + 1).filter(|to| *to < self.favourites.len()),
        };
        let Some(to) = to else { return Command::none() };
        self.favourites.swap(from, to);
        self.favourite_cursor = Some(key);
        self.save_favourites()
    }

    /// Goes to the entry `index` of the sidebar: the places first, then the favourites. A
    /// favourite whose folder is gone says so and stays where the person is.
    pub(super) fn sidebar_entry(&mut self, index: usize) -> Command<Msg> {
        self.sidebar_open = false;
        if let Some(place) = self.places.get(index) {
            let path = place.path.clone();
            return self.go_to_path(&path, None);
        }
        let Some(favourite) = index.checked_sub(self.places.len()).and_then(|at| self.favourites.get_mut(at)) else {
            return Command::none();
        };
        favourite.missing = !favourite.path.is_dir();
        let path = favourite.path.clone();
        if favourite.missing {
            let toast = Toast::warning(t!("explorer.favourites.gone", path = self.shown(&path)))
                .action(t!("explorer.favourites.remove"), Msg::RemoveFavourite(path))
                .key("vanished");
            return Command::toast(toast);
        }
        self.go_to_path(&path, None)
    }

    /// The key of the favourite the folder shown is in, when a favourite holds it more closely
    /// than any place.
    pub(super) fn current_favourite(&self) -> Option<String> {
        match self.current_entry()? {
            (index, _) if index >= self.places.len() => {
                self.favourites.get(index - self.places.len()).map(Favourite::key)
            }
            _ => None,
        }
    }

    /// The sidebar entry the folder shown is in, by its place in the sidebar: the deepest place or
    /// favourite that holds it, a place before a favourite of the same folder.
    pub(super) fn current_entry(&self) -> Option<(usize, String)> {
        let folder = self.files.folder();
        let places = self.places.iter().map(|place| (&place.path, false));
        let favourites = self.favourites.iter().map(|favourite| (&favourite.path, favourite.missing));
        let depth = |key: &String| if key.is_empty() { 0 } else { key.split('/').count() };
        places
            .chain(favourites)
            .enumerate()
            .filter(|(_, (_, missing))| !missing)
            .filter_map(|(index, (path, _))| self.key_of(path).map(|key| (index, key)))
            .filter(|(_, key)| super::nav::within(folder, key))
            .fold(None, |best: Option<(usize, String)>, (index, key)| match &best {
                Some((_, held)) if depth(held) >= depth(&key) => best,
                _ => Some((index, key)),
            })
    }
}
