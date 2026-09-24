//! qexp's settings: the view, hidden entries and coloured icons, kept in `explorer.conf`, and the
//! page that shows them with the framework's shared appearance rows and the Quvyta-wide update
//! notice.
//!
//! A value is written only while it differs from the default: a file of defaults would pin them,
//! and a later change of default would never reach the person.

use qframe::prelude::*;
use qframe::storage::Setting;
use qframe::widgets::{
    FileManagerMsg, FileView, ScrollView, Segmented, SettingRow, SettingsList, Switch, Text as Words,
};

use super::{Explorer, Msg};

/// The key of the view: `list` or `grid`. A `tree` left by 0.1.0 or 0.1.1 is read as the list and
/// stays in the file until the person picks a view.
pub(super) const VIEW: &str = "view";

/// The key of whether hidden entries are shown.
pub(super) const HIDDEN: &str = "show-hidden";

/// The key of whether icons take the colour of their kind.
pub(super) const COLOUR_ICONS: &str = "colour-icons";

/// The views in the order the picker shows them, with the names the file keeps them under.
/// The framework's tree view is left out: the person found it unneeded next to the list.
pub(super) const VIEWS: [(FileView, &str); 2] = [(FileView::List, "list"), (FileView::Icons, "grid")];

/// The widest the rows grow: beyond it a label and its control drift too far apart.
const SECTION: u16 = 76;

/// The view a file calls `name`.
pub(super) fn view_named(name: &str) -> Option<FileView> {
    VIEWS.iter().find(|(_, named)| *named == name).map(|(view, _)| *view)
}

/// The picker's place of `view`.
pub(super) fn view_index(view: FileView) -> usize {
    VIEWS.iter().position(|(shown, _)| *shown == view).unwrap_or_default()
}

impl Explorer {
    /// Keeps `value` under `key`, or nothing while it is `default`, and writes the file off the
    /// drawing thread.
    fn store<T: Setting + PartialEq>(&mut self, key: &str, value: T, default: &T) -> Command<Msg> {
        let changed = if value == *default { self.settings.remove(key) } else { self.settings.set(key, value) };
        if !changed {
            return Command::none();
        }
        self.settings.save_command(Msg::Saved)
    }

    /// Draws the folder in `view` from now on.
    pub(super) fn set_view(&mut self, view: FileView) -> Command<Msg> {
        self.view = view;
        let name = VIEWS.iter().find(|(shown, _)| *shown == view).map_or("list", |(_, name)| *name);
        // A click on the view picker leaves the keyboard there; it goes back to the rows.
        Command::batch([self.store(VIEW, name.to_owned(), &"list".to_owned()), Command::focus(super::view::FILES)])
    }

    /// Shows hidden entries, or not, from now on.
    pub(super) fn set_hidden(&mut self, shown: bool) -> Command<Msg> {
        let apply = self.files.update(FileManagerMsg::ShowHidden(shown), Msg::Files);
        Command::batch([apply, self.store(HIDDEN, shown, &false)])
    }

    /// Colours icons by kind, or not, from now on.
    pub(super) fn set_colour_icons(&mut self, on: bool) -> Command<Msg> {
        self.colour_icons = on;
        self.store(COLOUR_ICONS, on, &false)
    }

    /// The settings page, in the place of the folder.
    pub(super) fn settings_page(&self, ui: &mut View<'_, Msg>) {
        let page =
            |ui: &mut View<'_, Msg>| {
                ui.column(|ui| {
                    ui.add(Words::new(t!("explorer.settings.title")).role("title"));
                    SettingsList::show(ui, |list| {
                        list.heading(t!("explorer.settings.folders"));
                        let views = VIEWS.map(|(_, name)| t!(&format!("explorer.view.{name}")));
                        list.row(SettingRow::new(t!("explorer.settings.view")), |ui| {
                            ui.add(Segmented::new(views).selected(view_index(self.view)).on_select(|index| {
                                Msg::View(VIEWS.get(index).map_or(FileView::List, |(view, _)| *view))
                            }));
                        });
                        let hidden = self.files.shows_hidden();
                        list.row(
                            SettingRow::new(t!("explorer.settings.hidden"))
                                .description(t!("explorer.settings.hidden-text")),
                            |ui| {
                                ui.add(Switch::new(hidden).on_toggle(Msg::Hidden));
                            },
                        );
                        let colour = self.colour_icons;
                        list.row(
                            SettingRow::new(t!("explorer.settings.colour-icons"))
                                .description(t!("explorer.settings.colour-icons-text")),
                            |ui| {
                                ui.add(Switch::new(colour).on_toggle(Msg::ColourIcons));
                            },
                        );
                        self.appearance.section(list, Msg::Appearance);
                        // The switch is Quvyta-wide; without the folders that keep it nothing is asked,
                        // and a switch there would change nothing.
                        if self.machine.updates.is_some() {
                            self.appearance.updates(list, Msg::Appearance);
                        }
                    })
                    .width(Length::Cells(SECTION))
                    .id("settings-rows");
                })
                .padding(Padding { top: 1, right: 2, bottom: 1, left: 2 })
                .fill_width();
            };
        ui.add_with(ScrollView::new(), page).fill().id("settings");
    }
}
