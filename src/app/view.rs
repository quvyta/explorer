//! Drawing the screen: the top strip, the places, the folder and the keys.

use qframe::prelude::*;
use qframe::widgets::{
    Breadcrumb, ContextItem, EmptyState, Field, FileManager, FileManagerState, FileView, HelpLayer, IconButton, Menu,
    MenuGroup, MenuItem, Segmented, TextInput,
};

use super::nav::within;
use super::settings::{VIEWS, view_index};
use super::{Explorer, Msg};

/// The name of the path field, which takes the keyboard when it opens.
pub(super) const LOCATION: &str = "location";

/// The name of the file manager's rows, which take the keyboard back whenever qexp shows another
/// folder or closes what stood over them, so ↑ and ↓ move through the folder at once.
pub(super) const FILES: &str = "files";

/// Below this width the places leave the side for a layer opened with `ctrl+b`.
const SIDEBAR_BELOW: u16 = 90;

/// Below this width the screen keeps only the folder and a one-line path.
const BARE_BELOW: u16 = 48;

/// Below this size nothing useful fits.
const SMALLEST: Size = Size { width: 20, height: 6 };

/// The width of the places.
const SIDEBAR_WIDTH: u16 = 22;

impl Explorer {
    /// The whole screen at the size it is drawn in.
    pub(super) fn screen(&self, ui: &mut View<'_, Msg>) {
        let size = ui.size();
        if size.width < SMALLEST.width || size.height < SMALLEST.height {
            ui.add(EmptyState::new(t!("explorer.too-small"))).fill();
            return;
        }
        if size.width < BARE_BELOW {
            AppShell::new().header(|ui| self.path_bar(ui)).body(|ui| self.body(ui)).show(ui);
        } else {
            let collapsed = size.width < SIDEBAR_BELOW;
            AppShell::new()
                .sidebar_width(SIDEBAR_WIDTH)
                .collapse_below(SIDEBAR_BELOW)
                .sidebar_open(collapsed && self.sidebar_open)
                .header(|ui| self.top_strip(ui, collapsed))
                .sidebar(|ui| self.sidebar(ui))
                .body(|ui| self.body(ui))
                .footer(|ui| self.footer(ui))
                .show(ui);
        }
        self.open_with_dialog(ui);
        self.picker_dialog(ui);
        if self.help_open {
            ui.add(self.help());
        }
    }

    /// The top strip: back, forward and up, the path, and the view picker.
    fn top_strip(&self, ui: &mut View<'_, Msg>, collapsed: bool) {
        let at_root = self.files.folder() == FileManagerState::ROOT;
        ui.row(|ui| {
            if collapsed {
                ui.add(
                    IconButton::new("edge-right")
                        .tooltip(t!("explorer.places"))
                        .on_press(Msg::Places(!self.sidebar_open)),
                )
                .id("places-button");
            }
            ui.add(
                IconButton::new("arrow-left")
                    .tooltip(t!("explorer.back"))
                    .disabled(self.back.is_empty())
                    .on_press(Msg::Back),
            )
            .id("back");
            ui.add(
                IconButton::new("arrow-right")
                    .tooltip(t!("explorer.forward"))
                    .disabled(self.forward.is_empty())
                    .on_press(Msg::Forward),
            )
            .id("forward");
            ui.add(IconButton::new("arrow-up").tooltip(t!("explorer.up")).disabled(at_root).on_press(Msg::Up)).id("up");
            self.path_or_field(ui);
            if self.location.is_none() {
                ui.add(IconButton::new("prompt").tooltip(t!("explorer.go.type")).on_press(Msg::Location(true)))
                    .id("type-path");
            }
            let views = VIEWS.map(|(_, name)| t!(&format!("explorer.view.{name}")));
            ui.add(
                Segmented::new(views)
                    .selected(view_index(self.view))
                    .on_select(|index| Msg::View(VIEWS.get(index).map_or(FileView::List, |(view, _)| *view))),
            )
            .id("views");
            ui.add(
                IconButton::new("settings")
                    .tooltip(t!("explorer.settings.title"))
                    .on_press(Msg::Settings(!self.settings_open)),
            )
            .id("settings-button");
        })
        .gap(1)
        .padding(Padding::symmetric(0, 1))
        .fill_width();
    }

    /// The path alone, for a narrow screen.
    fn path_bar(&self, ui: &mut View<'_, Msg>) {
        ui.row(|ui| self.path_or_field(ui)).padding(Padding::symmetric(0, 1)).fill_width();
    }

    /// The path, or the field it turns into with `ctrl+l`.
    fn path_or_field(&self, ui: &mut View<'_, Msg>) {
        match &self.location {
            Some(location) => {
                ui.add_with(Field::new(t!("explorer.go.label")).label_width(8).error(location.error.clone()), |ui| {
                    ui.add(
                        TextInput::new(location.value.clone())
                            .invalid(location.error.is_some())
                            .on_change(Msg::LocationTyped)
                            .on_submit(Msg::LocationGo),
                    )
                    .fill_width()
                    .id(LOCATION);
                })
                .fill_width();
            }
            None => {
                let labels: Vec<String> = self.crumbs().into_iter().map(|(label, _)| label).collect();
                // A folder the person may not look into is shown but not open to them; its path
                // still leads back out.
                let unreadable = self.files.folder_error(self.files.folder()).is_some();
                ui.add(Breadcrumb::new(labels).faint(unreadable).on_select(Msg::Crumb)).fill_width().id("path");
            }
        }
    }

    /// The places, the one the folder is in raised with the accent pillar.
    fn sidebar(&self, ui: &mut View<'_, Msg>) {
        let items = self
            .places
            .iter()
            .enumerate()
            .map(|(index, place)| {
                MenuItem::new(format!("place-{index}"), t!(&format!("explorer.place.{}", place.kind.key())))
                    .icon(place.kind.icon(), None)
            })
            .collect::<Vec<_>>();
        let current = self.current_place().map(|index| format!("place-{index}"));
        ui.add(
            Menu::new([MenuGroup::new("places", items).title(t!("explorer.places"))])
                .selected(current.as_deref())
                .on_select(|key| {
                    Msg::Place(key.strip_prefix("place-").and_then(|n| n.parse().ok()).unwrap_or(usize::MAX))
                }),
        )
        .fill()
        .id("places");
    }

    /// The place the folder shown is in: the deepest place that holds it.
    fn current_place(&self) -> Option<usize> {
        let folder = self.files.folder();
        self.places
            .iter()
            .enumerate()
            .filter_map(|(index, place)| self.key_of(&place.path).map(|key| (index, key)))
            .filter(|(_, key)| within(folder, key))
            .max_by_key(|(_, key)| if key.is_empty() { 0 } else { key.split('/').count() })
            .map(|(index, _)| index)
    }

    /// The folder, or the settings page in its place.
    fn body(&self, ui: &mut View<'_, Msg>) {
        if self.settings_open {
            self.settings_page(ui);
            return;
        }
        let menu = self.menu_items();
        FileManager::new(&self.files, Msg::Files)
            .id(FILES)
            .view(self.view)
            .root_label("/")
            .on_open(|path| Msg::Open(path.to_path_buf()))
            // Every row's icon follows its kind; the colours, which the framework leaves out in
            // sixteen colours and in ASCII, are the person's setting.
            .kind_icons(true)
            .kind_tones(self.colour_icons)
            .user_folders(&self.user_folders)
            .menu_items(menu)
            .show(ui)
            .fill();
    }

    /// qexp's own items on a row's menu: "Open with…" on a file, the extracting items on an
    /// archive, and "Set as wallpaper" on a picture when qdesk is installed. The menu is built
    /// after the view, so what it needs is taken along.
    fn menu_items(&self) -> impl Fn(&str, &[String]) -> Vec<ContextItem<Msg>> + 'static {
        let folders = self.files.folder_keys();
        let root = self.machine.root.clone();
        let path_var = self.machine.path_var.clone();
        move |key, targets| {
            let alone = targets.len() <= 1;
            if !alone || key == FileManagerState::ROOT || folders.contains(key) {
                return Vec::new();
            }
            let path = key.split('/').filter(|part| !part.is_empty()).fold(root.clone(), |path, part| path.join(part));
            let mut items = vec![ContextItem::new(t!("explorer.open-with.item"), Msg::OpenWith(path.clone()))];
            items.extend(super::extract::menu_items(&path, path_var.as_deref()));
            items.extend(super::wallpaper::menu_items(&path, path_var.as_deref()));
            items
        }
    }

    /// The keys at the bottom, the ones that matter most first.
    fn footer(&self, ui: &mut View<'_, Msg>) {
        // An extraction that takes a while has the footer to itself, with its cancel button.
        if self.extraction_status(ui) {
            return;
        }
        let hints = if self.settings_open {
            KeyHints::new()
                .hint("↑↓", t!("explorer.hints.move"))
                .hint("enter", t!("explorer.hints.change"))
                .action(Scope::App, "cancel")
        } else {
            KeyHints::new()
                .hint("↑↓", t!("explorer.hints.move"))
                .hint("enter", t!("explorer.hints.open"))
                .action(Scope::App, "up")
                .action(Scope::App, "back")
                .action(Scope::App, "hidden")
                .action(Scope::App, "location")
                .action(Scope::Global, "help")
        };
        ui.add(hints.action_right(Scope::Global, "quit")).fill_width();
    }

    /// The key overview of `?`.
    fn help(&self) -> HelpLayer<Msg> {
        HelpLayer::new(Msg::Help(false))
            .hint("↑↓", t!("explorer.hints.move"))
            .hint("enter", t!("explorer.hints.open"))
            .hint("space", t!("explorer.hints.select"))
            .hint("menu", t!("explorer.hints.menu"))
    }
}
