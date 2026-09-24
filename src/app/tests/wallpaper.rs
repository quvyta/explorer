//! "Set as wallpaper" on a picture's menu. qdesk is a script in the scratch machine's search path
//! standing in for it; every test starts with a right click on a row, as a person does.

use qframe::event::{Event, MouseButton, MouseEvent, MouseKind};
use qframe::keymap::Modifiers;

use super::menu::{menu_item, wait_until};
use super::*;

/// The item's words, the same as qdesk's own menu.
const ITEM: &str = "Set as wallpaper";

/// A picture in the scratch home with a space in its name and its ending in capitals.
const PICTURE: &str = "home/Pictures/harbour view.JPG";

/// Puts a `qdesk` running `body` as a shell script into the scratch machine's search path.
fn fake_qdesk(scratch: &Scratch, body: &str) {
    let path = scratch.path("bin/qdesk");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("mode");
}

/// A `qdesk` that writes each argument it is given on a line of `args` in the scratch folder and
/// exits 0, as the real one does once it has kept the picture.
fn recording_qdesk(scratch: &Scratch) {
    let args = scratch.path("args");
    fake_qdesk(scratch, &format!("for arg; do printf '%s\\n' \"$arg\"; done > '{}'", args.display()));
}

/// The screen opened in the folder `folder` of the scratch machine.
fn in_folder(scratch: &Scratch, folder: &str) -> Harness<Explorer> {
    open_at(scratch.machine(), Start { folder: scratch.path(folder), select: None })
}

/// Right-clicks the row showing `name` and leaves its menu open.
fn right_click(h: &mut Harness<Explorer>, name: &str) {
    let (x, y) = h.find(name).unwrap_or_else(|| panic!("no {name} row:\n{}", h.screen()));
    h.mouse(MouseKind::Down(MouseButton::Right), x, y);
    h.mouse(MouseKind::Up(MouseButton::Right), x, y);
    settle(h);
}

/// Clicks the row showing `name` with ctrl held, adding it to what is chosen.
fn ctrl_click(h: &mut Harness<Explorer>, name: &str) {
    let (x, y) = h.find(name).unwrap_or_else(|| panic!("no {name} row:\n{}", h.screen()));
    let mods = Modifiers { ctrl: true, ..Modifiers::default() };
    for kind in [MouseKind::Down(MouseButton::Left), MouseKind::Up(MouseButton::Left)] {
        h.events(&[Event::Mouse(MouseEvent { kind, x, y, mods })]);
    }
    settle(h);
}

#[test]
fn the_item_runs_qdesk_wallpaper_with_the_picture_and_says_it_is_set() {
    let scratch = Scratch::new();
    scratch.write(PICTURE, "not read by qexp");
    recording_qdesk(&scratch);
    let mut h = in_folder(&scratch, "home/Pictures");
    menu_item(&mut h, "harbour view.JPG", ITEM);
    wait_until(&mut h, "the toast", |h| h.screen().contains("Wallpaper set"));
    let args = fs::read_to_string(scratch.path("args")).expect("qdesk was run");
    let picture = scratch.path(PICTURE);
    assert_eq!(args, format!("wallpaper\n{}\n", picture.display()), "the picture is one absolute argument");
    assert!(picture.exists(), "the picture itself is left as it was");
}

#[test]
fn a_picture_qdesk_refuses_shows_its_reason() {
    let scratch = Scratch::new();
    scratch.write(PICTURE, "not a picture at all");
    fake_qdesk(
        &scratch,
        "echo 'qdesk wallpaper: harbour view.JPG cannot be the wallpaper: unknown picture format' >&2\nexit 2",
    );
    let mut h = in_folder(&scratch, "home/Pictures");
    menu_item(&mut h, "harbour view.JPG", ITEM);
    wait_until(&mut h, "the toast", |h| h.screen().contains("harbour view.JPG is not the wallpaper"));
    assert!(h.screen().contains("unknown picture format"), "qdesk's own line is the body:\n{}", h.screen());
    assert!(!h.screen().contains("Wallpaper set"), "{}", h.screen());
}

#[test]
fn a_qdesk_that_fails_without_a_word_is_named_by_its_status() {
    let scratch = Scratch::new();
    scratch.write(PICTURE, "whatever");
    fake_qdesk(&scratch, "exit 1");
    let mut h = in_folder(&scratch, "home/Pictures");
    menu_item(&mut h, "harbour view.JPG", ITEM);
    wait_until(&mut h, "the toast", |h| h.screen().contains("harbour view.JPG is not the wallpaper"));
    assert!(h.screen().contains("qdesk ended with status 1"), "{}", h.screen());
}

#[test]
fn a_qdesk_that_never_answers_is_stopped() {
    let scratch = Scratch::new();
    scratch.write(PICTURE, "whatever");
    let started = scratch.path("started");
    fake_qdesk(&scratch, &format!(": > '{}'\nexec sleep 600", started.display()));
    let mut h = in_folder(&scratch, "home/Pictures");
    menu_item(&mut h, "harbour view.JPG", ITEM);
    wait_until(&mut h, "qdesk to start", |_| started.exists());
    assert!(h.screen().contains("harbour view.JPG"), "drawing goes on while qdesk runs:\n{}", h.screen());
    wait_until(&mut h, "the toast", |h| h.screen().contains("harbour view.JPG is not the wallpaper"));
    assert!(h.screen().contains("did not answer within 30 seconds"), "{}", h.screen());
}

#[test]
fn without_qdesk_there_is_no_item() {
    let scratch = Scratch::new();
    scratch.write(PICTURE, "whatever");
    let mut h = in_folder(&scratch, "home/Pictures");
    right_click(&mut h, "harbour view.JPG");
    assert!(h.screen().contains("Open with…"), "the menu is open:\n{}", h.screen());
    assert!(!h.screen().contains(ITEM), "{}", h.screen());
}

#[test]
fn a_qdesk_that_may_not_be_run_is_no_qdesk() {
    let scratch = Scratch::new();
    scratch.write(PICTURE, "whatever");
    recording_qdesk(&scratch);
    fs::set_permissions(scratch.path("bin/qdesk"), fs::Permissions::from_mode(0o644)).expect("mode");
    let mut h = in_folder(&scratch, "home/Pictures");
    right_click(&mut h, "harbour view.JPG");
    assert!(h.screen().contains("Open with…"), "the menu is open:\n{}", h.screen());
    assert!(!h.screen().contains(ITEM), "{}", h.screen());
}

#[test]
fn a_file_that_is_no_picture_has_no_item() {
    let scratch = Scratch::new();
    recording_qdesk(&scratch);
    let mut h = in_folder(&scratch, "home/notes");
    right_click(&mut h, "todo.md");
    assert!(h.screen().contains("Open with…"), "the menu is open:\n{}", h.screen());
    assert!(!h.screen().contains(ITEM), "{}", h.screen());
}

#[test]
fn a_folder_has_no_item_even_with_a_picture_name() {
    let scratch = Scratch::new();
    recording_qdesk(&scratch);
    fs::create_dir_all(scratch.path("home/album.png")).expect("folder");
    let mut h = in_folder(&scratch, "home");
    right_click(&mut h, "album.png");
    assert!(h.screen().contains("Rename"), "the menu is open:\n{}", h.screen());
    assert!(!h.screen().contains(ITEM), "{}", h.screen());
}

#[test]
fn several_pictures_chosen_have_no_item() {
    let scratch = Scratch::new();
    recording_qdesk(&scratch);
    scratch.write("home/Pictures/a.png", "a");
    scratch.write("home/Pictures/b.png", "b");
    let mut h = in_folder(&scratch, "home/Pictures");
    ctrl_click(&mut h, "a.png");
    ctrl_click(&mut h, "b.png");
    right_click(&mut h, "b.png");
    assert!(h.screen().contains("Copy 2 entries"), "the menu acts on both:\n{}", h.screen());
    assert!(!h.screen().contains(ITEM), "{}", h.screen());
    assert!(!scratch.path("args").exists());
}
