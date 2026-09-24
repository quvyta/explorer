//! Screen tests. Every test builds a machine of its own inside a temporary folder: the root the
//! file manager shows, the home folder, the trash, the desktop's databases and the shared Quvyta
//! folder are all in it, and it is removed when the test ends. The harness records every program
//! a test asks to open and runs none, and answers the update question itself.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use qframe::desktop::XdgDirs;
use qframe::icons::GlyphMode;
use qframe::prelude::*;

use super::{Explorer, Following, Machine, Opening, UpdateFolders};
use crate::cli::Start;

mod clipboard;
mod favourites;
mod menu;
mod mouse;
mod navigation;
mod opening;
mod screen;
mod selection;
mod settings;
mod wallpaper;

/// Long enough for background work to come back and a toast to settle in.
pub(super) const MOMENT: Duration = Duration::from_millis(300);

/// A temporary folder that is removed when the test ends, whatever happened in it.
pub(super) struct Scratch {
    root: PathBuf,
}

impl Scratch {
    /// A new, empty machine: a home folder with a few entries, `user-dirs.dirs` naming two of them,
    /// and a shared Quvyta look in English.
    pub(super) fn new() -> Self {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let name = format!("qexp-app-{}-{}", std::process::id(), COUNT.fetch_add(1, Ordering::Relaxed));
        let root = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch folder");
        let root = fs::canonicalize(root).expect("scratch folder");
        let scratch = Self { root };
        for folder in ["home/Documents", "home/Pictures", "home/notes/deep", "home/.config", "etc", "config", "bin"] {
            fs::create_dir_all(scratch.path(folder)).expect("folder");
        }
        scratch.write("home/notes/todo.md", "milk");
        scratch.write("home/notes/deep/inner.txt", "inside");
        scratch.write("home/Documents/report.pdf", "%PDF-1.7");
        scratch.write("home/build.sh", "echo hello");
        fs::set_permissions(scratch.path("home/build.sh"), fs::Permissions::from_mode(0o755)).expect("mode");
        scratch.write("home/.hidden", "shh");
        fs::write(scratch.path("home/data.bin"), [0u8, 159, 146, 150, 0, 1]).expect("binary file");
        scratch.write(
            "home/.config/user-dirs.dirs",
            "XDG_DOCUMENTS_DIR=\"$HOME/Documents\"\nXDG_PICTURES_DIR=\"$HOME/Pictures\"\nXDG_MUSIC_DIR=\"$HOME/Music\"\n",
        );
        scratch.write("config/quvyta.conf", "language = \"en\"\nicons = \"unicode\"\n");
        scratch
    }

    /// The path of `relative` inside the scratch folder.
    pub(super) fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// Writes `text` to `relative`, making the folders on the way.
    pub(super) fn write(&self, relative: &str, text: &str) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("folder");
        }
        fs::write(path, text).expect("file");
    }

    /// The machine the screen runs on: everything inside the scratch folder, no graphical session.
    pub(super) fn machine(&self) -> Machine {
        Machine {
            root: self.root.clone(),
            home: self.path("home"),
            xdg: XdgDirs {
                data_home: Some(self.path("data")),
                data_dirs: vec![self.path("system")],
                config_home: Some(self.path("home/.config")),
                config_dirs: Vec::new(),
                desktops: Vec::new(),
            },
            lang: "en_GB.UTF-8".to_owned(),
            path_var: Some(self.path("bin").into_os_string()),
            graphical: false,
            editor: vec!["nvim".to_owned()],
            trash: Some(self.path("trash")),
            config: Some(self.path("config")),
            updates: Some(UpdateFolders { config: self.path("config"), state: self.path("state") }),
            following: Following::Off,
            favourites: Some(self.path("data/quvyta/explorer/favorites")),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The screen on `machine`, opened at `start`, in English with Unicode glyphs, 100 × 30.
pub(super) fn open_at(machine: Machine, start: Start) -> Harness<Explorer> {
    let opening = Opening::new(machine, start);
    let mut h = Harness::with_env(opening.explorer, crate::locales::env(), 100, 30);
    h.set_locale("en").set_glyph_mode(GlyphMode::Unicode).set_reduced_motion(true);
    settle(&mut h);
    h
}

/// The screen of `scratch`, opened at its home folder.
pub(super) fn open(scratch: &Scratch) -> Harness<Explorer> {
    open_at(scratch.machine(), Start { folder: scratch.path("home"), select: None })
}

/// Lets the chain of folder reads a step starts run to its end: the harness runs one round of
/// background work per frame.
pub(super) fn settle(h: &mut Harness<Explorer>) {
    for _ in 0..8 {
        h.advance(MOMENT);
    }
}

/// The glyph `key` is drawn with.
pub(super) fn glyph(h: &Harness<Explorer>, key: &str) -> String {
    h.env().icons().glyph(key).into_owned()
}

/// Clicks the icon button drawn with the glyph `key` in the top strip.
pub(super) fn click_icon(h: &mut Harness<Explorer>, key: &str) {
    let glyph = glyph(h, key);
    let (x, y) = find_in_row(h, &glyph, 0).unwrap_or_else(|| panic!("no {key} button:\n{}", h.screen()));
    h.click(x, y);
    settle(h);
}

/// Where `text` is on screen row `row`.
pub(super) fn find_in_row(h: &Harness<Explorer>, text: &str, row: usize) -> Option<(i32, i32)> {
    let line = h.screen().lines().nth(row)?.to_owned();
    let at = line.find(text)?;
    let x = line[..at].chars().count();
    Some((i32::try_from(x).ok()?, i32::try_from(row).ok()?))
}

/// The line of the screen `text` is first on.
pub(super) fn line_with(h: &Harness<Explorer>, text: &str) -> String {
    h.screen().lines().find(|line| line.contains(text)).map(str::to_owned).unwrap_or_default()
}

/// Presses `chord` and lets what it started finish.
pub(super) fn press(h: &mut Harness<Explorer>, chord: &str) {
    h.press(chord);
    settle(h);
}

/// Double-clicks the first place `text` is on screen, two presses with no time between them as a
/// person's double click arrives, and lets what it started finish.
pub(super) fn double_click(h: &mut Harness<Explorer>, text: &str) {
    let (x, y) = h.find(text).unwrap_or_else(|| panic!("no {text:?} on screen:\n{}", h.screen()));
    h.click(x, y).click(x, y);
    settle(h);
}

/// Clicks the first place `text` is on screen and lets what it started finish.
pub(super) fn click(h: &mut Harness<Explorer>, text: &str) {
    let (x, y) = h.find(text).unwrap_or_else(|| panic!("no {text:?} on screen:\n{}", h.screen()));
    h.click(x, y);
    settle(h);
}
