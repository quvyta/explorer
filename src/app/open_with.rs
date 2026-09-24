//! The "Open with" dialog: every program that opens a file, the default one marked, and the
//! terminal editor last.
//!
//! It opens from a file's menu, with ctrl+enter, and when Enter finds no program for a file. What
//! it lists is worked out off the drawing thread, from the same databases [`super::launch`] reads,
//! with the kind described in the language the screen speaks when the dialog is asked for.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use qframe::desktop::{Choices, Openers};
use qframe::prelude::*;
use qframe::widgets::{List, ListItem, Modal};

use super::launch::Launch;
use super::{Explorer, Msg};

/// The dialog's width, in cells.
const WIDTH: u16 = 60;

/// The most rows of programs shown at once; a longer list scrolls.
const ROWS: u16 = 10;

/// The name of the list of programs.
pub(super) const PROGRAMS: &str = "open-with";

/// The dialog while it is open.
#[derive(Debug, Clone)]
pub struct OpenWith {
    path: PathBuf,
    mime: String,
    /// The kind in words, when the desktop describes it: "Markdown document".
    comment: Option<String>,
    rows: Vec<Row>,
    selected: usize,
}

/// One program the file can be opened with.
#[derive(Debug, Clone)]
struct Row {
    /// The program's name; `None` for the terminal editor, which is named while drawing.
    name: Option<String>,
    /// How choosing it opens the file.
    launch: Launch,
    default: bool,
    /// Whether it can be started here: a graphical program needs a graphical session.
    usable: bool,
}

impl OpenWith {
    /// The dialog for the file at `path`, whose kind and programs are `choices` and whose kind is
    /// called `comment` in words. `graphical` says whether windows can be shown; `editor` is the
    /// terminal editor's command.
    #[must_use]
    pub fn new(path: &Path, choices: Choices, comment: Option<String>, graphical: bool, editor: &[String]) -> Self {
        // The default first, then the others in the order the desktop gives them.
        let order =
            choices.default.into_iter().chain((0..choices.apps.len()).filter(|at| Some(*at) != choices.default));
        let mut rows: Vec<Row> = order
            .filter_map(|at| {
                let app = choices.apps.get(at)?;
                // A program whose command line is broken could never be started; it is left out.
                app.command(path)?;
                Some(Row {
                    name: Some(app.name.clone()),
                    launch: Launch::App(app.clone()),
                    default: Some(at) == choices.default,
                    usable: app.can_start(graphical),
                })
            })
            .collect();
        if !editor.is_empty() {
            let mut command: Vec<OsString> = editor.iter().map(OsString::from).collect();
            command.push(path.as_os_str().to_owned());
            rows.push(Row { name: None, launch: Launch::Editor(command), default: false, usable: true });
        }
        let selected = rows.iter().position(|row| row.usable).unwrap_or(0);
        Self { path: path.to_path_buf(), mime: choices.mime, comment, rows, selected }
    }

    /// The file the dialog is for.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How choosing row `index` opens the file, or `None` when that row cannot be chosen.
    fn launch(&self, index: usize) -> Option<Launch> {
        self.rows.get(index).filter(|row| row.usable).map(|row| row.launch.clone())
    }
}

impl Explorer {
    /// Opens the dialog for the file at `path`, reading what it lists off the drawing thread.
    pub(super) fn ask_open_with(&mut self, path: PathBuf) -> Command<Msg> {
        if path.is_dir() {
            return Command::none();
        }
        let openers = self.openers.clone();
        let (xdg, lang, path_var) =
            (self.machine.xdg.clone(), self.machine.lang.clone(), self.machine.path_var.clone());
        let (graphical, editor) = (self.machine.graphical, self.machine.editor.clone());
        // The screen's language, read here since the work below runs off the thread that knows it.
        let screen = qframe::i18n::active_code();
        Command::perform(move || {
            let openers = openers.unwrap_or_else(|| Arc::new(Openers::load(&xdg, &lang, path_var.as_deref())));
            let choices = openers.for_file(&path);
            let comment = openers.mime.comment(&choices.mime, &screen);
            let dialog = OpenWith::new(&path, choices, comment, graphical, &editor);
            Msg::OpenWithReady(openers, dialog)
        })
    }

    /// Row `index` of the dialog was chosen: the dialog closes and the file opens with it. A row
    /// that cannot be chosen keeps the dialog open.
    pub(super) fn choose_open_with(&mut self, index: usize) -> Command<Msg> {
        let Some(dialog) = &self.open_with else { return Command::none() };
        let Some(launch) = dialog.launch(index) else { return Command::none() };
        let path = dialog.path.clone();
        self.open_with = None;
        self.launch(&path, launch)
    }

    /// The dialog, while it is open.
    pub(super) fn open_with_dialog(&self, ui: &mut View<'_, Msg>) {
        let Some(dialog) = &self.open_with else { return };
        let name = dialog
            .path
            .file_name()
            .map_or_else(|| dialog.path.display().to_string(), |n| n.to_string_lossy().into_owned());
        let kind = match &dialog.comment {
            Some(comment) => format!("{comment} · {}", dialog.mime),
            None => dialog.mime.clone(),
        };
        let items: Vec<ListItem> = dialog
            .rows
            .iter()
            .map(|row| {
                let label = row.name.clone().unwrap_or_else(|| t!("explorer.open-with.editor"));
                let detail = if !row.usable {
                    Some(t!("explorer.open-with.no-desktop"))
                } else if row.default {
                    Some(t!("explorer.open-with.default"))
                } else if let Launch::Editor(command) = &row.launch {
                    command.first().map(|program| program.to_string_lossy().into_owned())
                } else {
                    None
                };
                let item = ListItem::new(label).faint(!row.usable);
                match detail {
                    Some(detail) => item.detail(detail),
                    None => item,
                }
            })
            .collect();
        let height = u16::try_from(items.len()).unwrap_or(ROWS).clamp(1, ROWS);
        let modal = Modal::new().title(name).width(WIDTH).on_close(Msg::OpenWithClose);
        ui.add_with(modal, |ui| {
            ui.add(Text::new(kind).role("secondary")).fill_width();
            ui.add(
                List::new(items)
                    .selected(Some(dialog.selected))
                    .on_select(Msg::OpenWithSelect)
                    .on_activate(Msg::OpenWithChoose),
            )
            .height(Length::Cells(height))
            .fill_width()
            .id(PROGRAMS);
        });
    }

    /// The cursor of the dialog moved to row `index`.
    pub(super) fn select_open_with(&mut self, index: usize) {
        if let Some(dialog) = &mut self.open_with {
            dialog.selected = index.min(dialog.rows.len().saturating_sub(1));
        }
    }
}
