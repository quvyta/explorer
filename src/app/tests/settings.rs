//! `explorer.conf`: each setting read at start changes what the screen does, each change on the
//! settings page or by its key is written, and a default is never written. The Quvyta-wide update
//! notice asks once at start, and nothing while it is off.

use super::*;

/// Where the switch of the row `label` is: at the right end of the rows, where the view picker of
/// the first row ends too. A switch is drawn in colour alone, so its place is read off the row that
/// has words there.
fn switch_of(h: &Harness<Explorer>, label: &str) -> (i32, i32) {
    let (_, y) = h.find(label).unwrap_or_else(|| panic!("no {label}:\n{}", h.screen()));
    let picker = h.screen().lines().position(|line| line.contains(" View ")).expect("the view row");
    let (x, _) = find_in_row(h, "grid", picker).expect("the picker");
    (x + 3, y)
}

fn conf(scratch: &Scratch) -> String {
    fs::read_to_string(scratch.path("config/explorer.conf")).unwrap_or_default()
}

#[test]
fn starting_with_the_defaults_writes_nothing() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    press(&mut h, "ctrl+,");
    press(&mut h, "esc");
    assert!(!scratch.path("config/explorer.conf").exists());
}

#[test]
fn the_view_in_the_file_is_the_view_at_start() {
    let scratch = Scratch::new();
    scratch.write("config/explorer.conf", "view = \"grid\"\n");
    let h = open(&scratch);
    assert!(!h.screen().contains("Size"), "a grid has no columns:\n{}", h.screen());
    assert!(h.screen().contains("notes"));
    scratch.write("config/explorer.conf", "view = \"sideways\"\n");
    let h = open(&scratch);
    assert!(h.screen().contains("Size"), "a view qexp does not know is the list:\n{}", h.screen());
}

#[test]
fn a_tree_left_by_an_older_qexp_opens_as_the_list_and_stays_in_the_file() {
    let scratch = Scratch::new();
    // 0.1.0 and 0.1.1 wrote this when the tree was chosen.
    scratch.write("config/explorer.conf", "view = \"tree\"\n");
    let h = open(&scratch);
    assert!(h.screen().contains("Size"), "the list, with its columns:\n{}", h.screen());
    assert!(!h.screen().contains(&format!("{} /", glyph(&h, "folder"))), "no tree from the root:\n{}", h.screen());
    assert!(!h.screen().contains("tree"), "and no word of it:\n{}", h.screen());
    assert_eq!(conf(&scratch), "view = \"tree\"\n", "starting alone rewrites nothing");
}

#[test]
fn the_view_picked_on_the_settings_page_is_drawn_and_written() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    click_icon(&mut h, "settings");
    let row = h.screen().lines().position(|line| line.contains(" View ")).expect("the view row");
    assert!(!h.screen().lines().nth(row).unwrap_or_default().contains("tree"), "{}", h.screen());
    let (x, y) = find_in_row(&h, "grid", row).expect("grid on the view row");
    h.click(x, y);
    settle(&mut h);
    assert!(conf(&scratch).contains("view = \"grid\""), "{}", conf(&scratch));
    press(&mut h, "esc");
    assert!(!h.screen().contains("Size"), "back at the folder, the grid has no columns:\n{}", h.screen());
    assert!(h.screen().contains("notes"), "{}", h.screen());
}

#[test]
fn hidden_entries_in_the_file_are_shown_at_start() {
    let scratch = Scratch::new();
    scratch.write("config/explorer.conf", "show-hidden = true\n");
    let h = open(&scratch);
    assert!(h.screen().contains(".hidden"), "{}", h.screen());
}

#[test]
fn the_view_chosen_is_written_and_the_default_is_taken_out_again() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    press(&mut h, "ctrl+2");
    assert!(conf(&scratch).contains("view = \"grid\""), "{}", conf(&scratch));
    assert!(open(&scratch).screen().contains("notes") && !open(&scratch).screen().contains("Size"));
    press(&mut h, "ctrl+1");
    assert!(!conf(&scratch).contains("view"), "the list is the default:\n{}", conf(&scratch));
}

#[test]
fn hidden_entries_turned_on_by_key_are_written() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    press(&mut h, "alt+.");
    assert!(conf(&scratch).contains("show-hidden = true"), "{}", conf(&scratch));
    assert!(open(&scratch).screen().contains(".hidden"), "the next start shows them");
    press(&mut h, "alt+.");
    assert!(!conf(&scratch).contains("show-hidden"), "{}", conf(&scratch));
}

/// The colour the icon of `build.sh` is drawn in.
fn icon_colour(h: &Harness<Explorer>) -> String {
    let (x, y) = h.find(" build.sh").unwrap_or_else(|| panic!("no build.sh:\n{}", h.screen()));
    format!("{:?}", h.buffer()[(u16::try_from(x - 1).expect("x"), u16::try_from(y).expect("y"))].fg)
}

#[test]
fn the_settings_page_turns_hidden_entries_and_coloured_icons_on() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    let plain = icon_colour(&h);
    click_icon(&mut h, "settings");
    for row in ["Show hidden files", "Colour icons by kind", "View", "Language", "Say when an update is out"] {
        assert!(h.screen().contains(row), "{row}:\n{}", h.screen());
    }
    let (x, y) = switch_of(&h, "Show hidden files");
    h.click(x, y);
    settle(&mut h);
    let (x, y) = switch_of(&h, "Colour icons by kind");
    h.click(x, y);
    settle(&mut h);
    let written = conf(&scratch);
    assert!(written.contains("show-hidden = true") && written.contains("colour-icons = true"), "{written}");
    press(&mut h, "esc");
    assert!(h.screen().contains(".hidden"), "back at the folder, hidden entries show:\n{}", h.screen());
    assert_ne!(icon_colour(&h), plain, "and the script's icon takes the colour of its kind at once");
}

#[test]
fn the_folder_keys_rest_while_the_settings_page_is_open() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    press(&mut h, "ctrl+,");
    press(&mut h, "backspace");
    assert_eq!(h.app().folder(), scratch.path("home"), "backspace does not go up behind the page");
    press(&mut h, "ctrl+,");
    press(&mut h, "backspace");
    assert_eq!(h.app().folder(), scratch.path(""), "the page closed, the key goes up");
}

#[test]
fn a_newer_version_is_said_once_at_start() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    let asked = h.update_checks().to_vec();
    assert_eq!(asked.len(), 1);
    assert_eq!((asked[0].package(), asked[0].current()), ("quvyta-explorer", env!("CARGO_PKG_VERSION")));
    h.set_latest_version(Some("9.9.9")).advance(MOMENT);
    assert!(h.screen().contains("quvyta-explorer 9.9.9 is out"), "{}", h.screen());
}

#[test]
fn with_the_quvyta_wide_switch_off_nothing_is_asked() {
    let scratch = Scratch::new();
    scratch.write("config/quvyta.conf", "language = \"en\"\nicons = \"unicode\"\nupdate-notice = false\n");
    let mut h = open(&scratch);
    assert!(h.update_checks().is_empty());
    h.set_latest_version(Some("9.9.9")).advance(MOMENT);
    assert!(!h.screen().contains("is out"), "{}", h.screen());
}

#[test]
fn turning_the_notice_off_on_the_settings_page_asks_nothing_at_the_next_start() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    click_icon(&mut h, "settings");
    let (x, y) = switch_of(&h, "Say when an update is out");
    h.click(x, y);
    settle(&mut h);
    assert!(!qframe::storage::Family::QUVYTA.update_notice_in(&scratch.path("config")), "the shared switch is off");
    assert!(open(&scratch).update_checks().is_empty());
}

#[test]
fn without_the_folders_nothing_is_asked() {
    let scratch = Scratch::new();
    let mut machine = scratch.machine();
    machine.updates = None;
    let h = open_at(machine, Start { folder: scratch.path("home"), select: None });
    assert!(h.update_checks().is_empty());
}

#[test]
fn a_theme_another_quvyta_application_gives_qexp_while_it_is_open_is_where_the_next_pick_goes() {
    use qframe::storage::{Ecosystem, Scope, Shared};
    let scratch = Scratch::new();
    let folder = scratch.path("config");
    scratch.write("config/quvyta.conf", "language = \"en\"\ntheme = \"nordic\"\nicons = \"unicode\"\n");
    let opening = Opening::new(scratch.machine(), Start { folder: scratch.path("home"), select: None });
    let mut h = Harness::member_in(opening.explorer, Ecosystem::QUVYTA, &folder, super::super::APP, 100, 30);
    h.set_reduced_motion(true);
    settle(&mut h);
    click_icon(&mut h, "settings");
    assert!(h.screen().contains("Nordic"), "{}", h.screen());
    // The launcher, which lists every member, gives qexp a theme of its own.
    Ecosystem::QUVYTA.set_in(&folder, super::super::APP, Shared::Theme, "iris", Scope::App).expect("saved");
    h.poll_preferences();
    settle(&mut h);
    assert_eq!(h.env().theme().id(), "iris", "the screen follows");
    assert!(h.screen().contains("Iris"), "and the page says so:\n{}", h.screen());
    // qexp now keeps its own theme, so the next one picked here stays with qexp.
    h.click_text("Iris");
    settle(&mut h);
    h.click_text("Amber");
    settle(&mut h);
    assert!(conf(&scratch).contains("theme = \"amber\""), "{}", conf(&scratch));
    let shared = fs::read_to_string(folder.join("quvyta.conf")).expect("quvyta.conf");
    assert!(shared.contains("theme = \"nordic\""), "the other applications keep theirs:\n{shared}");
}
