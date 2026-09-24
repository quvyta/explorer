//! The places of the sidebar: the home folder, the person's own folders the desktop names in
//! `user-dirs.dirs`, and the root of the file system.
//!
//! The desktop already knows where a person keeps their pictures and downloads, and in which
//! language those folders are named: `xdg-user-dirs` writes it to `user-dirs.dirs` the first time
//! a session starts. The framework's `storage::user_dir_in` reads that file, so a Turkish desktop's
//! `~/Resimler` is found as the pictures folder. What stays here is which of them the sidebar
//! lists, in which order, and their names in qexp's languages.

use std::fs;
use std::path::{Path, PathBuf};

use qframe::storage::{UserDir, user_dir_in};

/// What a place is, which decides its name and its icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceKind {
    /// The home folder.
    Home,
    /// The desktop folder.
    Desktop,
    /// The documents folder.
    Documents,
    /// The downloads folder.
    Downloads,
    /// The pictures folder.
    Pictures,
    /// The music folder.
    Music,
    /// The videos folder.
    Videos,
    /// The root of the file system.
    Root,
}

impl PlaceKind {
    /// The part of the language keys (`explorer.place.<key>`) that names the place.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Desktop => "desktop",
            Self::Documents => "documents",
            Self::Downloads => "downloads",
            Self::Pictures => "pictures",
            Self::Music => "music",
            Self::Videos => "videos",
            Self::Root => "root",
        }
    }

    /// The icon key the place is drawn with, from the framework's icon set.
    #[must_use]
    pub fn icon(self) -> &'static str {
        match self {
            Self::Home => "folder-home",
            Self::Desktop => "folder-desktop",
            Self::Documents => "folder-documents",
            Self::Downloads => "folder-downloads",
            Self::Pictures => "folder-pictures",
            Self::Music => "folder-music",
            Self::Videos => "folder-videos",
            Self::Root => "file-disk",
        }
    }
}

/// One entry of the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// What it is.
    pub kind: PlaceKind,
    /// Where it is, with every link resolved.
    pub path: PathBuf,
}

/// The folders of `user-dirs.dirs`, in the order the sidebar lists them.
const USER_DIRS: [(UserDir, PlaceKind); 6] = [
    (UserDir::Desktop, PlaceKind::Desktop),
    (UserDir::Documents, PlaceKind::Documents),
    (UserDir::Downloads, PlaceKind::Downloads),
    (UserDir::Pictures, PlaceKind::Pictures),
    (UserDir::Music, PlaceKind::Music),
    (UserDir::Videos, PlaceKind::Videos),
];

/// The places of a person whose home folder is `home` and whose `user-dirs.dirs` is in
/// `config_home` (`home/.config` when that is not known): home, the user folders that exist, and
/// `root`.
///
/// A folder the file does not name is looked for under its English name, as every other program
/// does. A user folder that is the home folder itself is left out: `xdg-user-dirs` points a folder
/// it has given up on at `$HOME`, and a second "Home" under another name would only confuse.
#[must_use]
pub fn places(home: &Path, config_home: Option<&Path>, root: &Path) -> Vec<Place> {
    let config = config_home.map_or_else(|| home.join(".config"), Path::to_path_buf);
    let mut places = vec![Place { kind: PlaceKind::Home, path: home.to_path_buf() }];
    for (which, kind) in USER_DIRS {
        let Ok(path) = fs::canonicalize(user_dir_in(which, home, &config)) else { continue };
        if path != home && path.is_dir() {
            places.push(Place { kind, path });
        }
    }
    places.push(Place { kind: PlaceKind::Root, path: root.to_path_buf() });
    places
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_s_own_names_are_read_and_only_folders_that_exist_are_listed() {
        let scratch = std::env::temp_dir().join(format!("qexp-places-{}", std::process::id()));
        let home = scratch.join("home");
        let config = home.join(".config");
        for folder in ["Masaüstü", "Resimler", "İndirilenler"] {
            fs::create_dir_all(home.join(folder)).expect("folder");
        }
        fs::create_dir_all(&config).expect("config");
        let file = "# written by xdg-user-dirs-update\nXDG_DESKTOP_DIR=\"$HOME/Masaüstü\"\n\
                    XDG_DOWNLOAD_DIR=\"$HOME/İndirilenler\"\nXDG_PICTURES_DIR=\"$HOME/Resimler\"\n\
                    XDG_MUSIC_DIR=\"$HOME/Müzik\"\nXDG_VIDEOS_DIR=\"$HOME/\"\nXDG_DOCUMENTS_DIR=Belgeler\n";
        fs::write(config.join("user-dirs.dirs"), file).expect("user dirs");
        let home = fs::canonicalize(&home).expect("home");

        let found = places(&home, Some(&config), Path::new("/"));
        let kinds: Vec<PlaceKind> = found.iter().map(|place| place.kind).collect();
        assert_eq!(
            kinds,
            [PlaceKind::Home, PlaceKind::Desktop, PlaceKind::Downloads, PlaceKind::Pictures, PlaceKind::Root],
            "music does not exist, videos is home itself and documents, not in a form that is read, \
             has no English folder either"
        );
        assert_eq!(found[1].path, home.join("Masaüstü"));
        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn without_the_file_the_english_folders_that_exist_are_listed() {
        let scratch = std::env::temp_dir().join(format!("qexp-places-english-{}", std::process::id()));
        let home = scratch.join("home");
        fs::create_dir_all(home.join("Downloads")).expect("folder");
        let home = fs::canonicalize(&home).expect("home");
        let found = places(&home, None, Path::new("/"));
        let kinds: Vec<PlaceKind> = found.iter().map(|place| place.kind).collect();
        assert_eq!(kinds, [PlaceKind::Home, PlaceKind::Downloads, PlaceKind::Root]);
        let _ = fs::remove_dir_all(&scratch);
    }

    #[test]
    fn without_the_file_there_are_home_and_root() {
        let found = places(Path::new("/nowhere/home"), None, Path::new("/"));
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].kind, found[1].kind), (PlaceKind::Home, PlaceKind::Root));
    }
}
