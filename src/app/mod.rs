//! The screen: the top strip with the way back, forward and up, the path and the view picker; the
//! places on the left; the folder in the middle, drawn by the framework's `FileManager`; and the
//! keys at the bottom.
//!
//! qexp keeps one `FileManagerState` rooted at the root of the file system, so what is cut or
//! copied stays waiting while the person moves between folders: every path is under the same
//! root. Going somewhere opens the folders above the goal, reads them, and then steps into it with
//! the manager's own `Enter`; the tree view shows the way down to it for that reason.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use qframe::desktop::{Launched, Openers, XdgDirs, graphical_session};
use qframe::icons::UserFolders;
use qframe::prelude::*;
use qframe::runtime::{Handoff, HandoffOutcome, TaskEvent, TaskId, Update, UpdateCheck};
use qframe::storage::{Family, Preferences, Settings};
use qframe::widgets::{Appearance, AppearanceChange, FileManagerMsg, FileManagerState, FilePickerMsg, FileView, Toast};

use crate::archive::ExtractError;
use crate::cli::Start;
use crate::places::{self, Place};

mod extract;
pub mod launch;
mod nav;
mod open_with;
mod settings;
mod view;
pub mod wallpaper;

use launch::Launch;
use open_with::OpenWith;

/// qexp's name among the Quvyta apps: its settings file is `explorer.conf` in the shared Quvyta
/// folder.
pub const APP: &str = "explorer";

/// What qexp knows about the machine it runs on: where things are and what it may start.
///
/// Everything the screen reads from the environment is here, so a test hands it a machine made
/// of temporary folders and nothing reaches the person's own files, trash or desktop.
#[derive(Debug, Clone)]
pub struct Machine {
    /// The folder the file manager is rooted at: `/`, or a test's temporary folder.
    pub root: PathBuf,
    /// The home folder, with every link resolved.
    pub home: PathBuf,
    /// Where the desktop's own databases are read from: programs, kinds and `user-dirs.dirs`.
    pub xdg: XdgDirs,
    /// The person's language as in `LANG`, for the names of programs.
    pub lang: String,
    /// The program search path, to leave out programs that are not installed.
    pub path_var: Option<OsString>,
    /// Whether a graphical session is there to show windows (`DISPLAY` or `WAYLAND_DISPLAY`).
    pub graphical: bool,
    /// The terminal editor's command, for text files nothing else opens.
    pub editor: Vec<String>,
    /// Where deleted entries go: `None` for the person's own trash.
    pub trash: Option<PathBuf>,
    /// The shared Quvyta folder, which holds `explorer.conf` and the Quvyta-wide switches; `None`
    /// keeps every change in memory.
    pub config: Option<PathBuf>,
    /// Where the update notice is read and the last question remembered; `None` asks nothing.
    pub updates: Option<UpdateFolders>,
    /// Whether folders on screen are followed as other programs change them.
    pub following: Following,
}

impl Machine {
    /// This machine, as the environment describes it.
    #[must_use]
    pub fn here() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let home = var("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
        let home = std::fs::canonicalize(&home).unwrap_or(home);
        let family = Family::QUVYTA;
        Self {
            root: PathBuf::from("/"),
            home,
            xdg: XdgDirs::from_env(var),
            lang: var("LC_ALL").or_else(|| var("LC_MESSAGES")).or_else(|| var("LANG")).unwrap_or_default(),
            path_var: std::env::var_os("PATH"),
            graphical: graphical_session(var),
            editor: launch::editor(var),
            trash: None,
            config: family.config_dir(),
            updates: UpdateFolders::here(),
            following: Following::On,
        }
    }
}

/// Whether the folders on screen are followed as other programs change them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Following {
    /// Not followed: a folder is read when it is gone into and when the person asks.
    Off,
    /// Followed, each wait for a change as long as it takes: the running application.
    On,
    /// Followed, each wait lasting at most this long; a harness runs the wait on the spot, so a
    /// test that wants to see another program's change arrive gives a short bound.
    Within(std::time::Duration),
}

/// Where the Quvyta-wide update notice is kept and where qexp remembers when it last asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateFolders {
    /// The shared Quvyta configuration folder, whose shared file holds the switch.
    pub config: PathBuf,
    /// qexp's state folder under the Quvyta one, which remembers when the question was last asked.
    pub state: PathBuf,
}

impl UpdateFolders {
    /// This machine's folders, or `None` without a home folder, where nothing could remember the
    /// switch or the last question and so nothing is asked.
    #[must_use]
    pub fn here() -> Option<Self> {
        let family = Family::QUVYTA;
        family.config_dir().zip(family.state_dir(APP)).map(|(config, state)| Self { config, state })
    }
}

/// Where the path bar stands while it is a text field.
#[derive(Debug, Clone, Default)]
struct Location {
    /// What has been typed.
    value: String,
    /// Why the last path given could not be gone to.
    error: Option<String>,
}

/// A folder qexp is on its way to: the folders above it are being read.
#[derive(Debug, Clone)]
struct Goal {
    /// The folder's key in the file manager.
    key: String,
    /// The entry of it the cursor lands on.
    select: Option<String>,
    /// Whether the folders above were read again already, in case a listing was older than the
    /// folder asked for.
    reread: bool,
}

/// The application's state.
pub struct Explorer {
    machine: Machine,
    files: FileManagerState,
    places: Vec<Place>,
    /// The home and its folders by the names the desktop gives them, for their icons.
    user_folders: UserFolders,
    view: FileView,
    colour_icons: bool,
    settings: Settings,
    appearance: Appearance,
    back: Vec<String>,
    forward: Vec<String>,
    goal: Option<Goal>,
    /// The folder qexp last saw shown, to notice when it is taken away by another program.
    here: String,
    location: Option<Location>,
    sidebar_open: bool,
    settings_open: bool,
    help_open: bool,
    openers: Option<Arc<Openers>>,
    start: Option<Start>,
    /// The "Open with" dialog, while it is open.
    open_with: Option<OpenWith>,
    /// The extractions running, oldest first.
    extractions: Vec<extract::Extraction>,
    /// The folder picker of "Extract to…", while it is open.
    picking: Option<extract::Picking>,
}

/// What the screen starts with: qexp itself, its settings file and the shared Quvyta look the
/// runtime opens in.
pub struct Opening {
    /// The screen.
    pub explorer: Explorer,
    /// `explorer.conf`, for the runtime's saved look.
    pub settings: Settings,
    /// The shared Quvyta language, theme and icons, in force from the first frame.
    pub preferences: Preferences,
}

impl Opening {
    /// The screen on `machine`, opening at `start`.
    #[must_use]
    pub fn new(machine: Machine, start: Start) -> Self {
        let family = Family::QUVYTA;
        let i18n = crate::locales::i18n();
        let (settings, preferences) = match &machine.config {
            Some(folder) => (
                Settings::open(folder.join(format!("{APP}.conf"))).member_of(&family),
                family.preferences_in(folder, APP, &i18n),
            ),
            None => (Settings::in_memory(), family.preferences(APP, &i18n)),
        };
        let appearance = match &machine.config {
            Some(folder) => Appearance::new(family, APP, preferences.clone()).in_folder(folder),
            None => Appearance::new(family, APP, preferences.clone()).without_saving(),
        };
        let explorer = Explorer::new(machine, start, settings.clone(), appearance);
        Self { explorer, settings, preferences }
    }
}

impl Explorer {
    fn new(machine: Machine, start: Start, settings: Settings, appearance: Appearance) -> Self {
        let hidden = settings.get_or(settings::HIDDEN, false);
        let view = settings.get::<String>(settings::VIEW).and_then(|name| settings::view_named(&name));
        let colour_icons = settings.get_or(settings::COLOUR_ICONS, false);
        let files = FileManagerState::new(&machine.root).showing_hidden(hidden);
        let files = match machine.following {
            Following::Off => files,
            Following::On => files.following(true),
            Following::Within(bound) => files.following_within(bound),
        };
        let files = match &machine.trash {
            Some(folder) => files.trashing_in(folder),
            None => files.trashing(),
        };
        let config_home = machine.xdg.config_home.clone();
        let places = places::places(&machine.home, config_home.as_deref(), &machine.root);
        // The machine's own folders, never the process's: a test's home is in its scratch folder.
        let user_folders = match &config_home {
            Some(config) => UserFolders::read(&machine.home, config),
            None => UserFolders::english(&machine.home),
        };
        Self {
            machine,
            files,
            places,
            user_folders,
            view: view.unwrap_or(FileView::List),
            colour_icons,
            settings,
            appearance,
            back: Vec::new(),
            forward: Vec::new(),
            goal: None,
            here: FileManagerState::ROOT.to_owned(),
            location: None,
            sidebar_open: false,
            settings_open: false,
            help_open: false,
            openers: None,
            start: Some(start),
            open_with: None,
            extractions: Vec::new(),
            picking: None,
        }
    }

    /// The folder shown, as a path.
    #[must_use]
    pub fn folder(&self) -> PathBuf {
        self.files.path(self.files.folder())
    }

    /// The path of the entry the cursor is on, when it is on one.
    #[must_use]
    pub fn selected(&self) -> Option<PathBuf> {
        self.files.selected().map(|key| self.files.path(key))
    }

    /// The view the folder is drawn in.
    #[must_use]
    pub fn view(&self) -> FileView {
        self.view
    }

    /// The question for a newer version of qexp, when the Quvyta-wide update notice is on. A
    /// machine where it is off asks nothing at all.
    fn ask_for_update(&self) -> Command<Msg> {
        let Some(folders) = &self.machine.updates else { return Command::none() };
        if !Family::QUVYTA.update_notice_in(&folders.config) {
            return Command::none();
        }
        let check =
            UpdateCheck::new(Family::QUVYTA, APP, env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), Msg::NewVersion)
                .in_folders(folders.config.clone(), folders.state.clone());
        Command::check_for_update(check)
    }

    /// Opens the file at `path` with its program, working out which off the drawing thread.
    fn open(&mut self, path: PathBuf) -> Command<Msg> {
        let openers = self.openers.clone();
        let (xdg, lang, path_var) =
            (self.machine.xdg.clone(), self.machine.lang.clone(), self.machine.path_var.clone());
        let (graphical, editor) = (self.machine.graphical, self.machine.editor.clone());
        Command::perform(move || {
            // The databases are read the first time a file is opened, not at start: a person who
            // only looks around never waits for them.
            let openers = openers.unwrap_or_else(|| Arc::new(Openers::load(&xdg, &lang, path_var.as_deref())));
            let launch = launch::decide(&openers, &path, graphical, &editor);
            Msg::Decided(openers, path, launch)
        })
    }

    /// Carries out what opening `path` came to. Either way the program starts in the file's folder,
    /// as a desktop file manager starts it: an editor's `:e other.md` or a shell's `ls` looks beside
    /// the file, not wherever qexp itself was started.
    fn launch(&mut self, path: &Path, launch: Launch) -> Command<Msg> {
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        match launch {
            Launch::App(app) => {
                let failed = name.clone();
                app.launch(path, self.machine.graphical, move |ran| Msg::Ran(name, ran)).unwrap_or_else(|error| {
                    let toast =
                        Toast::danger(t!("explorer.open.failed", name = failed.as_str())).body(error.to_string());
                    Command::toast(toast.key("open"))
                })
            }
            Launch::Editor(command) => {
                let Some((program, args)) = command.split_first() else { return Command::none() };
                let folder = path.parent().map_or_else(|| self.machine.root.clone(), Path::to_path_buf);
                // The editor is qexp's own last resort, no program of the desktop, so it is handed
                // the terminal here rather than through `DesktopApp::launch`.
                let handoff = Handoff::new(program.clone(), move |outcome| {
                    Msg::Ran(
                        name,
                        match outcome {
                            HandoffOutcome::Finished { code } => Launched::Returned { code },
                            HandoffOutcome::Failed(reason) => Launched::Failed(reason),
                        },
                    )
                })
                .args(args.to_vec())
                .dir(folder);
                Command::handoff(handoff)
            }
            Launch::Folder(target) => self.go_to_path(&target, None),
            // Nothing opens it without being asked, so the person is asked.
            Launch::Nothing(_) => self.ask_open_with(path.to_path_buf()),
        }
    }
}

/// Everything that can happen on this screen.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Something for the file manager, from its rows or its own work.
    Files(FileManagerMsg),
    /// Back through the history.
    Back,
    /// Forward through the history.
    Forward,
    /// Up to the folder above.
    Up,
    /// A part of the path was clicked: the folder at that place in it.
    Crumb(usize),
    /// A place of the sidebar, by its place in the list.
    Place(usize),
    /// The folder is drawn in this view.
    View(FileView),
    /// Hidden entries are shown or not.
    Hidden(bool),
    /// Icons are coloured by the kind of file or not.
    ColourIcons(bool),
    /// The path bar becomes a text field, or goes back to the path.
    Location(bool),
    /// The text of the path field changed.
    LocationTyped(String),
    /// Enter in the path field.
    LocationGo(String),
    /// Where the typed path leads, or `None` when nothing is there.
    LocationChecked(Option<Start>),
    /// The sidebar opens over the folder on a narrow screen, or closes.
    Places(bool),
    /// The settings page opens or closes.
    Settings(bool),
    /// The key overview opens or closes.
    Help(bool),
    /// Esc: closes whatever is open on top.
    Cancel,
    /// A file was asked to be opened.
    Open(PathBuf),
    /// What opening a file comes to, with the databases read for it.
    Decided(Arc<Openers>, PathBuf, Launch),
    /// The program the file of this name was opened with ended, was started beside qexp, or could
    /// not be started.
    Ran(String, Launched),
    /// The "Open with" dialog is asked for the file at this path.
    OpenWith(PathBuf),
    /// What the dialog lists has been worked out, with the databases read for it.
    OpenWithReady(Arc<Openers>, OpenWith),
    /// The cursor of the dialog moved to this row.
    OpenWithSelect(usize),
    /// This row of the dialog was chosen.
    OpenWithChoose(usize),
    /// The dialog closes without opening anything.
    OpenWithClose,
    /// Extract the archive at the first path into the folder at the second.
    Extract(PathBuf, PathBuf),
    /// Ask for a folder to extract the archive at this path into.
    ExtractTo(PathBuf),
    /// Something for the folder picker of "Extract to…".
    Picker(FilePickerMsg),
    /// The folder picker closes without extracting anything.
    PickerClose,
    /// The extraction has run long enough to be named in the footer.
    ExtractSlow(TaskId),
    /// What an extraction came to, with the archive's name.
    Extracted(TaskId, String, Result<PathBuf, ExtractError>),
    /// An extraction's task started or ended.
    ExtractEvent(TaskEvent),
    /// The footer's cancel button: stop this extraction.
    ExtractCancel(TaskId),
    /// Set the picture at the second path as qdesk's wallpaper, with the qdesk at the first.
    Wallpaper(PathBuf, PathBuf),
    /// What `qdesk wallpaper` came to, with the picture's name.
    WallpaperDone(String, wallpaper::Outcome),
    /// F2: rename the entry under the cursor.
    Rename,
    /// Delete: move the selection to the trash.
    Trash,
    /// Shift+Delete: delete the selection for good, after a question.
    Delete,
    /// A change on the settings page's appearance rows.
    Appearance(AppearanceChange),
    /// `explorer.conf` was written, or why not.
    Saved(Result<(), String>),
    /// A newer version of qexp is out.
    NewVersion(Update),
}

impl std::fmt::Debug for Explorer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Explorer")
            .field("folder", &self.files.folder())
            .field("view", &self.view)
            .finish_non_exhaustive()
    }
}

impl App for Explorer {
    type Msg = Msg;

    fn init(&mut self) -> Command<Msg> {
        let load = self.files.load(Msg::Files);
        let start = match self.start.take() {
            Some(start) => self.go_to_path(&start.folder, start.select),
            None => Command::none(),
        };
        Command::batch([load, start, self.ask_for_update()])
    }

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Files(message) => return self.files_message(message),
            Msg::Back => return self.back(),
            Msg::Forward => return self.forward(),
            Msg::Up => return self.up(),
            Msg::Crumb(index) => return self.crumb(index),
            Msg::Place(index) => {
                self.sidebar_open = false;
                let Some(place) = self.places.get(index) else { return Command::none() };
                let path = place.path.clone();
                return self.go_to_path(&path, None);
            }
            Msg::View(view) => return self.set_view(view),
            Msg::Hidden(shown) => return self.set_hidden(shown),
            Msg::ColourIcons(on) => return self.set_colour_icons(on),
            Msg::Location(open) => return self.location(open),
            Msg::LocationTyped(value) => {
                if let Some(location) = &mut self.location {
                    location.value = value;
                    location.error = None;
                }
            }
            Msg::LocationGo(value) => return self.check_location(&value),
            Msg::LocationChecked(result) => return self.location_checked(result),
            Msg::Places(open) => self.sidebar_open = open,
            Msg::Settings(open) => {
                self.settings_open = open;
                if !open {
                    return Command::focus(view::FILES);
                }
            }
            Msg::Help(open) => self.help_open = open,
            Msg::Cancel => return self.cancel(),
            Msg::Open(path) => return self.open(path),
            Msg::Decided(openers, path, launch) => {
                self.openers = Some(openers);
                return self.launch(&path, launch);
            }
            // The terminal program may have changed what the folder holds.
            Msg::Ran(_, Launched::Returned { .. }) => return self.files.update(FileManagerMsg::Refresh, Msg::Files),
            Msg::Ran(_, Launched::Started) => {}
            Msg::Ran(name, Launched::Failed(reason)) => {
                let toast = Toast::danger(t!("explorer.open.failed", name = name.as_str())).body(reason).key("open");
                return Command::toast(toast);
            }
            Msg::OpenWith(path) => return self.ask_open_with(path),
            Msg::OpenWithReady(openers, dialog) => {
                self.openers = Some(openers);
                self.open_with = Some(dialog);
            }
            Msg::OpenWithSelect(index) => self.select_open_with(index),
            Msg::OpenWithChoose(index) => return self.choose_open_with(index),
            Msg::OpenWithClose => self.open_with = None,
            Msg::Extract(archive, into) => return self.extract(archive, into),
            Msg::ExtractTo(archive) => return self.pick_folder(archive),
            Msg::Picker(message) => return self.picker_message(message),
            Msg::PickerClose => self.picking = None,
            Msg::ExtractSlow(id) => self.extraction_slow(id),
            Msg::Extracted(id, name, result) => return self.extracted(id, &name, result),
            Msg::ExtractEvent(event) => self.extraction_event(&event),
            Msg::ExtractCancel(id) => return self.cancel_extraction(id),
            Msg::Wallpaper(qdesk, picture) => return self.set_wallpaper(qdesk, picture),
            Msg::WallpaperDone(name, outcome) => return Self::wallpaper_done(&name, outcome),
            Msg::Rename => return self.on_selected(FileManagerMsg::Rename),
            Msg::Trash => return self.on_selected(FileManagerMsg::Trash),
            Msg::Delete => return self.on_selected(FileManagerMsg::Delete),
            // The framework's rows write their own files, key by key.
            Msg::Appearance(change) => return self.appearance.update(change, &mut self.settings),
            Msg::Saved(Ok(())) => {}
            Msg::Saved(Err(reason)) => {
                return Command::toast(Toast::warning(t!("explorer.settings.not-saved")).body(reason).key("saved"));
            }
            Msg::NewVersion(update) => return Command::toast(update.toast()),
        }
        Command::none()
    }

    fn view(&self, ui: &mut View<'_, Msg>) {
        self.screen(ui);
    }

    fn action(&self, name: &str) -> Option<Msg> {
        // The settings page has no folder on screen for the folder's keys to act on.
        if self.settings_open {
            return match name {
                "settings" | "cancel" => Some(Msg::Settings(false)),
                "help" => Some(Msg::Help(true)),
                _ => None,
            };
        }
        let msg = match name {
            "back" => Msg::Back,
            "forward" => Msg::Forward,
            "up" => Msg::Up,
            "location" => Msg::Location(true),
            "places" => Msg::Places(!self.sidebar_open),
            "hidden" => Msg::Hidden(!self.files.shows_hidden()),
            "view-list" => Msg::View(FileView::List),
            "view-grid" => Msg::View(FileView::Icons),
            "view-tree" => Msg::View(FileView::Tree),
            "settings" => Msg::Settings(true),
            "help" => Msg::Help(true),
            "cancel" => Msg::Cancel,
            "rename" => Msg::Rename,
            "trash" => Msg::Trash,
            "delete" => Msg::Delete,
            "open-with" => Msg::OpenWith(self.selected_file()?),
            other => {
                let number: usize = other.strip_prefix("place-")?.parse().ok()?;
                Msg::Place(number.checked_sub(1)?)
            }
        };
        Some(msg)
    }
}

impl Explorer {
    /// Esc closes what is on top: the path field, the sidebar laid over the folder.
    fn cancel(&mut self) -> Command<Msg> {
        if self.location.is_some() {
            return self.location(false);
        }
        self.sidebar_open = false;
        Command::none()
    }

    /// The path of the file under the cursor, when the cursor is on a file rather than a folder.
    fn selected_file(&self) -> Option<PathBuf> {
        let key = self.files.selected().filter(|key| !self.files.is_folder(key))?;
        Some(self.files.path(key))
    }

    /// Sends `make(key)` for the entry under the cursor, when the cursor is on an entry rather
    /// than on the folder's own row.
    fn on_selected(&mut self, make: fn(String) -> FileManagerMsg) -> Command<Msg> {
        let Some(key) = self.files.selected().filter(|key| *key != self.files.folder()).map(str::to_owned) else {
            return Command::none();
        };
        self.files.update(make(key), Msg::Files)
    }
}

#[cfg(test)]
mod tests;
