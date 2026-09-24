//! Favourites: added from a folder's menu, kept in the data folder across starts, gone to from the
//! sidebar and with alt and a number, taken out and moved from their own menu and with alt+shift
//! and the arrows; a gone folder drawn faint; and the desktop's GTK bookmarks taken in once.

use qframe::event::{MouseButton, MouseKind};

use super::menu::menu_item;
use super::*;

/// The favourites file of the scratch machine.
fn file(scratch: &Scratch) -> PathBuf {
    scratch.path("data/quvyta/explorer/favorites")
}

/// The favourites file's lines.
fn stored(scratch: &Scratch) -> Vec<String> {
    fs::read_to_string(file(scratch)).unwrap_or_default().lines().map(str::to_owned).collect()
}

/// The sidebar's lines: the first cells of every line of the screen below the top strip.
fn sidebar(h: &Harness<Explorer>) -> Vec<String> {
    h.screen().lines().skip(1).map(|line| line.chars().take(22).collect::<String>()).collect()
}

/// Where the favourite `name` is in the sidebar, below the "Favourites" heading.
fn in_sidebar(h: &Harness<Explorer>, name: &str) -> Option<(i32, i32)> {
    let lines = sidebar(h);
    let heading = lines.iter().position(|line| line.contains("Favourites"))?;
    lines.iter().enumerate().skip(heading + 1).find_map(|(row, line)| {
        let at = line.find(name)?;
        let x = i32::try_from(line[..at].chars().count()).ok()?;
        Some((x, i32::try_from(row + 1).ok()?))
    })
}

/// The favourites below the "Favourites" heading, top to bottom.
fn favourites_shown(h: &Harness<Explorer>) -> Vec<String> {
    let lines = sidebar(h);
    let Some(heading) = lines.iter().position(|line| line.contains("Favourites")) else { return Vec::new() };
    lines[heading + 1..]
        .iter()
        .map(|line| line.trim_matches(|c: char| c == ' ' || c == '▌').to_owned())
        .take_while(|line| !line.is_empty())
        // The icon comes first, then the folder's name.
        .map(|line| line.split_once(' ').map_or(line.clone(), |(_, name)| name.to_owned()))
        .collect()
}

/// Right-clicks `name` in the sidebar and clicks `item` on its menu.
fn sidebar_menu(h: &mut Harness<Explorer>, name: &str, item: &str) {
    let (x, y) = in_sidebar(h, name).unwrap_or_else(|| panic!("no {name} in the sidebar:\n{}", h.screen()));
    h.mouse(MouseKind::Down(MouseButton::Right), x, y);
    h.mouse(MouseKind::Up(MouseButton::Right), x, y);
    settle(h);
    click(h, item);
}

/// Clicks `name` in the sidebar.
fn sidebar_click(h: &mut Harness<Explorer>, name: &str) {
    let (x, y) = in_sidebar(h, name).unwrap_or_else(|| panic!("no {name} in the sidebar:\n{}", h.screen()));
    h.click(x, y);
    settle(h);
}

/// The scratch machine with `notes` and `notes/deep` added as favourites from their menus.
fn with_two(scratch: &Scratch) -> Harness<Explorer> {
    let mut h = open(scratch);
    menu_item(&mut h, "notes", "Add to favourites");
    double_click(&mut h, "notes");
    menu_item(&mut h, "deep", "Add to favourites");
    h
}

#[test]
fn a_folder_added_from_its_menu_is_in_the_sidebar_on_disk_and_after_a_restart() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    assert!(!h.screen().contains("Favourites"), "no heading without favourites:\n{}", h.screen());
    menu_item(&mut h, "notes", "Add to favourites");
    assert_eq!(favourites_shown(&h), ["notes"], "{}", h.screen());
    assert_eq!(stored(&scratch), [scratch.path("home/notes").display().to_string()]);

    // A new start on the same machine reads it back.
    let mut again = open(&scratch);
    assert_eq!(favourites_shown(&again), ["notes"], "{}", again.screen());
    sidebar_click(&mut again, "notes");
    assert_eq!(again.app().folder(), scratch.path("home/notes"), "a click goes there");
    let pillar = sidebar(&again).iter().any(|line| line.starts_with('▌') && line.contains("notes"));
    assert!(pillar, "the favourite the folder is in carries the pillar:\n{}", again.screen());
    let home = sidebar(&again).iter().any(|line| line.starts_with('▌') && line.contains("Home"));
    assert!(!home, "and not the place above it:\n{}", again.screen());
}

/// Right-clicks the row of `name` in the folder, right of the sidebar.
fn folder_menu(h: &mut Harness<Explorer>, name: &str) {
    let found = h.screen().lines().enumerate().skip(1).find_map(|(row, line)| {
        let rest: String = line.chars().skip(22).collect();
        let at = rest.find(name)?;
        Some((i32::try_from(22 + rest[..at].chars().count()).ok()?, i32::try_from(row).ok()?))
    });
    let (x, y) = found.unwrap_or_else(|| panic!("no {name} row in the folder:\n{}", h.screen()));
    h.mouse(MouseKind::Down(MouseButton::Right), x, y);
    h.mouse(MouseKind::Up(MouseButton::Right), x, y);
    settle(h);
}

#[test]
fn the_shown_folder_s_own_row_adds_it_too() {
    let scratch = Scratch::new();
    let mut h = open_at(scratch.machine(), Start { folder: scratch.path("home/notes"), select: None });
    folder_menu(&mut h, "notes");
    click(&mut h, "Add to favourites");
    assert_eq!(stored(&scratch), [scratch.path("home/notes").display().to_string()], "{}", h.screen());
}

#[test]
fn a_favourite_s_own_folder_offers_removing_it_never_adding_it_twice() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    menu_item(&mut h, "notes", "Add to favourites");
    folder_menu(&mut h, "notes");
    assert!(!h.screen().contains("Add to favourites"), "{}", h.screen());
    click(&mut h, "Remove from favourites");
    assert!(favourites_shown(&h).is_empty(), "{}", h.screen());
    assert!(stored(&scratch).is_empty());
    assert!(scratch.path("home/notes/todo.md").exists(), "the folder itself stays");
}

#[test]
fn a_favourite_is_removed_and_moved_from_its_menu() {
    let scratch = Scratch::new();
    let mut h = with_two(&scratch);
    assert_eq!(favourites_shown(&h), ["notes", "deep"], "{}", h.screen());
    sidebar_menu(&mut h, "deep", "Move up");
    assert_eq!(favourites_shown(&h), ["deep", "notes"], "{}", h.screen());
    let (deep, notes) = (scratch.path("home/notes/deep"), scratch.path("home/notes"));
    assert_eq!(stored(&scratch), [deep.display().to_string(), notes.display().to_string()]);
    sidebar_menu(&mut h, "deep", "Move down");
    assert_eq!(favourites_shown(&h), ["notes", "deep"], "{}", h.screen());
    sidebar_menu(&mut h, "notes", "Remove from favourites");
    assert_eq!(favourites_shown(&h), ["deep"], "{}", h.screen());
    assert_eq!(stored(&scratch), [deep.display().to_string()]);
}

#[test]
fn alt_shift_and_the_arrows_move_the_favourite_the_keyboard_is_on() {
    let scratch = Scratch::new();
    let mut h = with_two(&scratch);
    // The keyboard goes to the sidebar the way a person sends it: shift+tab back from the folder
    // until the favourites have it.
    for _ in 0..20 {
        if h.is_focused("favourites") {
            break;
        }
        press(&mut h, "shift+tab");
    }
    assert!(h.is_focused("favourites"), "{}", h.screen());
    press(&mut h, "down");
    press(&mut h, "down");
    press(&mut h, "alt+shift+up");
    assert_eq!(favourites_shown(&h), ["deep", "notes"], "{}", h.screen());
    press(&mut h, "alt+shift+up");
    assert_eq!(favourites_shown(&h), ["deep", "notes"], "the top one stays at the top");
    press(&mut h, "alt+shift+down");
    assert_eq!(favourites_shown(&h), ["notes", "deep"], "{}", h.screen());
    let (deep, notes) = (scratch.path("home/notes/deep"), scratch.path("home/notes"));
    assert_eq!(stored(&scratch), [notes.display().to_string(), deep.display().to_string()]);
}

#[test]
fn alt_and_a_number_counts_the_favourites_after_the_places() {
    let scratch = Scratch::new();
    let mut h = with_two(&scratch);
    // Home, Documents, Pictures and Root come first.
    press(&mut h, "alt+1");
    press(&mut h, "alt+6");
    assert_eq!(h.app().folder(), scratch.path("home/notes/deep"), "{}", h.screen());
    press(&mut h, "alt+5");
    assert_eq!(h.app().folder(), scratch.path("home/notes"), "{}", h.screen());
}

#[test]
fn a_favourite_whose_folder_is_gone_is_faint_and_a_click_says_so_and_stays() {
    let scratch = Scratch::new();
    let h = with_two(&scratch);
    drop(h);
    fs::remove_dir_all(scratch.path("home/notes/deep")).expect("remove");
    let mut h = open(&scratch);
    let colour = |h: &Harness<Explorer>, name: &str| {
        let (x, y) = in_sidebar(h, name).expect("in the sidebar");
        h.fg(u16::try_from(x).expect("x"), u16::try_from(y).expect("y"))
    };
    let muted = h.env().theme().color("muted");
    assert_eq!(colour(&h, "deep"), muted, "the gone one is faint:\n{}", h.screen());
    assert_ne!(colour(&h, "notes"), muted, "the other one is not");

    sidebar_click(&mut h, "deep");
    assert_eq!(h.app().folder(), scratch.path("home"), "qexp stays where it was");
    assert!(h.screen().contains("~/notes/deep is no"), "a note says so:\n{}", h.screen());
    assert_eq!(favourites_shown(&h), ["notes", "deep"], "it stays in the list until taken out");
}

// --- GTK bookmarks ---

/// A GTK bookmarks file with two local folders that exist, one of them with a space in its name
/// and a label, one that does not, a network place and a repeat.
fn gtk(scratch: &Scratch) -> String {
    fs::create_dir_all(scratch.path("home/My Work")).expect("folder");
    let home = scratch.path("home");
    let text = format!(
        "file://{home}/Documents\nfile://{home}/My%20Work Work stuff\nfile://{home}/Gone\n\
         sftp://server/home/ada\nfile://{home}/Documents\n",
        home = home.display()
    );
    scratch.write("home/.config/gtk-3.0/bookmarks", &text);
    text
}

#[test]
fn the_gtk_bookmarks_are_taken_in_once_and_never_written() {
    let scratch = Scratch::new();
    let text = gtk(&scratch);
    let h = open(&scratch);
    assert_eq!(favourites_shown(&h), ["Documents", "My Work"], "{}", h.screen());
    assert_eq!(
        stored(&scratch),
        [scratch.path("home/Documents").display().to_string(), scratch.path("home/My Work").display().to_string()]
    );
    let bookmarks = scratch.path("home/.config/gtk-3.0/bookmarks");
    assert_eq!(fs::read_to_string(&bookmarks).expect("bookmarks"), text, "the GTK file is left as it was");

    // Once qexp keeps its own list, a change on the desktop's side is not taken in again.
    fs::write(&bookmarks, format!("file://{}\n", scratch.path("home/Pictures").display())).expect("bookmarks");
    let again = open(&scratch);
    assert_eq!(favourites_shown(&again), ["Documents", "My Work"], "{}", again.screen());
}

#[test]
fn an_emptied_list_is_not_filled_from_the_gtk_bookmarks_again() {
    let scratch = Scratch::new();
    gtk(&scratch);
    let mut h = open(&scratch);
    sidebar_menu(&mut h, "Work", "Remove from favourites");
    sidebar_menu(&mut h, "Documents", "Remove from favourites");
    assert!(stored(&scratch).is_empty());
    let again = open(&scratch);
    assert!(favourites_shown(&again).is_empty(), "{}", again.screen());
}

#[test]
fn a_favourite_stands_in_the_same_column_as_the_places_above_it() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    menu_item(&mut h, "notes", "Add to favourites");
    let lines = sidebar(&h);
    // The column a row's name starts in, in cells: an arrow column or any other indent before the
    // favourites would push their names right of the places' names.
    let start = |needle: &str| {
        lines
            .iter()
            .find_map(|line| line.find(needle).map(|at| line[..at].chars().count()))
            .unwrap_or_else(|| panic!("no {needle} in the sidebar:\n{}", h.screen()))
    };
    assert_eq!(start("notes"), start("Documents"), "a favourite lines up with the places:\n{}", h.screen());
}
