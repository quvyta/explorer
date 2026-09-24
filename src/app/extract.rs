//! Extracting an archive from its menu: beside itself, or into a folder the person picks.
//!
//! The work runs as a background task and never holds up drawing. The task waits on its own sleep
//! between looks at the tool, so cancelling it wakes it at once and the tool is stopped; in the
//! tests the harness's clock drives those sleeps. A quick extraction shows nothing until it is
//! done: the footer names it only once it has run for [`SHOW_AFTER`].

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use qframe::prelude::*;
use qframe::runtime::{Task, TaskEvent, TaskId};
use qframe::widgets::{
    ContextItem, FileBrowser, FileManagerMsg, FilePicker, FilePickerMsg, Modal, PickMode, Toast, child_key,
};

use super::{Explorer, Msg};
use crate::archive::{self, ExtractError};

/// How often the task looks whether the tool is done.
const LOOK: Duration = Duration::from_millis(50);

/// How long an extraction runs before the footer names it; a quicker one never flashes there.
const SHOW_AFTER: Duration = Duration::from_millis(300);

/// The width of the folder picker, in cells.
const PICKER_WIDTH: u16 = 70;

/// The height of the folder picker's own rows, in cells.
const PICKER_ROWS: u16 = 16;

/// An extraction under way.
#[derive(Debug, Clone)]
pub(super) struct Extraction {
    id: TaskId,
    /// The archive's name, for the footer.
    name: String,
    /// Whether it has run long enough to be shown.
    shown: bool,
}

/// The folder picker of "Extract to…", while it is open.
pub(super) struct Picking {
    archive: PathBuf,
    browser: FileBrowser,
}

/// A file's name as it is shown.
fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// The items for the archive at `path` on its row's menu, or none when it is no archive.
///
/// Without a tool for it both items are shown faint, saying which tool is needed; qexp never
/// installs one of its own accord.
pub(super) fn menu_items(path: &Path, path_var: Option<&std::ffi::OsStr>) -> Vec<ContextItem<Msg>> {
    let Some(format) = archive::format_of(&name_of(path)) else { return Vec::new() };
    let here = path.parent().map_or_else(|| path.to_path_buf(), Path::to_path_buf);
    let missing = archive::plan(format, path, &here, path_var).err();
    let items = [
        ContextItem::new(t!("explorer.archive.here"), Msg::Extract(path.to_path_buf(), here.clone())),
        ContextItem::new(t!("explorer.archive.to"), Msg::ExtractTo(path.to_path_buf())),
    ];
    match missing {
        None => items.into(),
        Some(missing) => {
            items.map(|item| item.disabled(true).detail(t!("explorer.archive.needs", tool = missing.wanted))).into()
        }
    }
}

impl Explorer {
    /// Starts extracting `archive` into `into` in the background.
    pub(super) fn extract(&mut self, archive: PathBuf, into: PathBuf) -> Command<Msg> {
        let name = name_of(&archive);
        let path_var: Option<OsString> = self.machine.path_var.clone();
        let label = t!("explorer.archive.running", name = name.as_str());
        let result_name = name.clone();
        let task = Task::new(label, move |cx| {
            let id = cx.id();
            let mut waited = Duration::ZERO;
            let mut told = false;
            let result = archive::extract_watched(&archive, &into, path_var.as_deref(), &mut || {
                let going = cx.sleep(LOOK);
                waited += LOOK;
                if going && !told && waited >= SHOW_AFTER {
                    told = true;
                    cx.send(Msg::ExtractSlow(id));
                }
                going
            });
            Ok(Msg::Extracted(id, result_name, result))
        })
        .on_event(Msg::ExtractEvent);
        self.extractions.push(Extraction { id: task.id(), name, shown: false });
        Command::task(task)
    }

    /// The extraction `id` has run long enough to be named in the footer.
    pub(super) fn extraction_slow(&mut self, id: TaskId) {
        if let Some(extraction) = self.extractions.iter_mut().find(|extraction| extraction.id == id) {
            extraction.shown = true;
        }
    }

    /// An extraction ended, one way or another; a cancelled one ends here without a result.
    pub(super) fn extraction_event(&mut self, event: &TaskEvent) {
        if let TaskEvent::Finished { id, .. } = event {
            self.extractions.retain(|extraction| extraction.id != *id);
        }
    }

    /// What an extraction came to: the new folder is shown and selected, or the reason it failed.
    pub(super) fn extracted(&mut self, id: TaskId, name: &str, result: Result<PathBuf, ExtractError>) -> Command<Msg> {
        self.extractions.retain(|extraction| extraction.id != id);
        let reason = match result {
            Ok(made) => {
                let toast = Toast::success(t!("explorer.archive.done", name = name)).key("extract");
                return Command::batch([self.reveal(&made), Command::toast(toast)]);
            }
            Err(ExtractError::Cancelled) => return Command::none(),
            Err(ExtractError::Encrypted) => t!("explorer.archive.encrypted"),
            Err(ExtractError::Missing(missing)) => {
                t!("explorer.archive.missing", tool = missing.wanted, package = missing.package)
            }
            Err(ExtractError::NotAnArchive) => t!("explorer.archive.not-archive"),
            Err(ExtractError::Failed(reason)) => reason,
        };
        Command::toast(Toast::danger(t!("explorer.archive.failed", name = name)).body(reason).key("extract"))
    }

    /// Shows the folder `made` is in, read again, with the cursor on it.
    fn reveal(&mut self, made: &Path) -> Command<Msg> {
        let (Some(parent), Some(name)) = (made.parent(), made.file_name()) else { return Command::none() };
        let Some(key) = self.key_of(parent) else { return Command::none() };
        let name = name.to_string_lossy().into_owned();
        let refresh = self.files.update(FileManagerMsg::Refresh, Msg::Files);
        if key == self.files.folder() {
            self.files.select(&child_key(&key, &name));
            // The keyboard stays where the person left it: the menu that started the extraction
            // gave it back to the rows, and a person who has since clicked into the path field
            // keeps typing there.
            return refresh;
        }
        Command::batch([refresh, self.go(key, Some(name), true)])
    }

    /// The cancel button of the footer: the extraction `id` stops and leaves nothing behind.
    pub(super) fn cancel_extraction(&mut self, id: TaskId) -> Command<Msg> {
        Command::cancel_task(id)
    }

    /// Opens the folder picker for extracting `archive`, at the archive's own folder.
    pub(super) fn pick_folder(&mut self, archive: PathBuf) -> Command<Msg> {
        let folder = archive.parent().map_or_else(|| self.machine.root.clone(), Path::to_path_buf);
        let mut browser = FileBrowser::new(folder.clone(), PickMode::Folders);
        let open = browser.open(folder, Msg::Picker);
        self.picking = Some(Picking { archive, browser });
        open
    }

    /// A message of the folder picker: a folder chosen starts the extraction into it.
    pub(super) fn picker_message(&mut self, message: FilePickerMsg) -> Command<Msg> {
        if let FilePickerMsg::Chosen(folder) = message {
            let Some(picking) = self.picking.take() else { return Command::none() };
            return self.extract(picking.archive, folder);
        }
        let Some(picking) = &mut self.picking else { return Command::none() };
        picking.browser.update(message, Msg::Picker)
    }

    /// The folder picker, while it is open.
    pub(super) fn picker_dialog(&self, ui: &mut View<'_, Msg>) {
        let Some(picking) = &self.picking else { return };
        let title = t!("explorer.archive.pick-title", name = name_of(&picking.archive).as_str());
        let modal = Modal::new().title(title).width(PICKER_WIDTH).on_close(Msg::PickerClose);
        ui.add_with(modal, |ui| {
            FilePicker::new(&picking.browser, Msg::Picker).show(ui).height(Length::Cells(PICKER_ROWS)).fill_width();
        });
    }

    /// The footer's note on the extraction running longest, with its cancel button, once one has
    /// run long enough to be shown.
    pub(super) fn extraction_status(&self, ui: &mut View<'_, Msg>) -> bool {
        let Some(extraction) = self.extractions.iter().find(|extraction| extraction.shown) else { return false };
        ui.row(|ui| {
            ui.add(Text::new(t!("explorer.archive.running", name = extraction.name.as_str())).no_wrap());
            ui.spacer();
            ui.add(Button::new(t!("explorer.archive.cancel")).on_press(Msg::ExtractCancel(extraction.id)))
                .id("cancel-extract");
        })
        .gap(2)
        .padding(Padding::symmetric(0, 1))
        .fill_width();
        true
    }
}
