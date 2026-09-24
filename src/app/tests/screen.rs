//! What the screen shows: an icon for every kind of file, the views, hidden entries and the keys.

use qframe::color::ColorDepth;
use qframe::icons::GlyphMode;

use super::*;

/// The glyph drawn just before `name` on its row.
fn icon_before(h: &Harness<Explorer>, name: &str) -> String {
    let line = h.screen().lines().find(|line| line.contains(&format!(" {name}"))).map(str::to_owned);
    let line = line.unwrap_or_else(|| panic!("no row {name}:\n{}", h.screen()));
    let before: Vec<char> = line[..line.find(&format!(" {name}")).expect("the name")].chars().collect();
    before.last().map(char::to_string).unwrap_or_default()
}

#[test]
fn every_row_carries_the_icon_of_its_kind() {
    let scratch = Scratch::new();
    scratch.write("home/notes/main.rs", "fn main() {}");
    scratch.write("home/notes/photo.png", "png");
    scratch.write("home/notes/backup.tar.gz", "gz");
    let h = open_at(scratch.machine(), Start { folder: scratch.path("home/notes"), select: None });
    let family = |key: &str| glyph(&h, key);
    assert_eq!(icon_before(&h, "todo.md"), family("file-markdown"), "{}", h.screen());
    assert_eq!(icon_before(&h, "main.rs"), family("file-rust"));
    assert_eq!(icon_before(&h, "photo.png"), family("file-image"));
    assert_eq!(icon_before(&h, "backup.tar.gz"), family("file-archive"));
    assert_eq!(icon_before(&h, "deep"), family("folder"));
    // In Unicode the families are what tell the rows apart: text, code, a picture, an archive.
    let shapes: std::collections::BTreeSet<String> =
        ["todo.md", "main.rs", "photo.png", "backup.tar.gz"].iter().map(|name| icon_before(&h, name)).collect();
    assert_eq!(shapes.len(), 4, "four families, four shapes:\n{}", h.screen());
}

#[test]
fn a_program_is_known_by_its_run_bit_and_the_home_folders_by_their_places() {
    let scratch = Scratch::new();
    scratch.write("home/install", "#!/bin/sh");
    fs::set_permissions(scratch.path("home/install"), fs::Permissions::from_mode(0o755)).expect("mode");
    // A folder named in the person's language is known only from `user-dirs.dirs` of this home.
    fs::create_dir(scratch.path("home/Filmler")).expect("folder");
    let dirs = fs::read_to_string(scratch.path("home/.config/user-dirs.dirs")).expect("user dirs");
    scratch.write("home/.config/user-dirs.dirs", &format!("{dirs}XDG_VIDEOS_DIR=\"$HOME/Filmler\"\n"));
    let mut h = open(&scratch);
    settle(&mut h);
    assert_eq!(icon_before(&h, "install"), glyph(&h, "file-executable"), "{}", h.screen());
    h.set_glyph_mode(GlyphMode::Nerd);
    assert_eq!(icon_before(&h, "Documents"), glyph(&h, "folder-documents"), "{}", h.screen());
    assert_eq!(icon_before(&h, "Pictures"), glyph(&h, "folder-pictures"));
    assert_eq!(icon_before(&h, "Filmler"), glyph(&h, "folder-videos"), "{}", h.screen());
    assert_ne!(glyph(&h, "folder-documents"), glyph(&h, "folder"), "a Nerd Font tells a place from a folder");
}

#[test]
fn hidden_entries_come_and_go_with_ctrl_h_and_alt_dot() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    assert!(!h.screen().contains(".hidden"));
    press(&mut h, "ctrl+h");
    assert!(h.screen().contains(".hidden"), "{}", h.screen());
    press(&mut h, "alt+.");
    assert!(!h.screen().contains(".hidden"), "{}", h.screen());
}

#[test]
fn the_views_change_from_the_picker_and_from_ctrl_and_a_number() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    assert!(h.screen().contains("Size"), "the list has its columns:\n{}", h.screen());
    click(&mut h, "grid");
    assert!(!h.screen().contains("Size"), "the grid has none:\n{}", h.screen());
    assert!(h.screen().contains("notes"), "{}", h.screen());
    press(&mut h, "ctrl+3");
    // The tree shows the way down from the root, which is a row of its own.
    let root = format!("{} /", glyph(&h, "folder"));
    assert!(h.screen().contains(&root), "{}", h.screen());
    assert!(!h.screen().contains("Size"), "{}", h.screen());
    press(&mut h, "ctrl+1");
    assert!(h.screen().contains("Size"), "{}", h.screen());
}

#[test]
fn colour_follows_the_kind_only_where_the_terminal_can_show_it() {
    let scratch = Scratch::new();
    scratch.write("config/explorer.conf", "colour-icons = true\n");
    let coloured = open(&scratch);
    fs::remove_file(scratch.path("config/explorer.conf")).expect("back to the default");
    let plain = open(&scratch);
    let cell = |h: &Harness<Explorer>| {
        let (x, y) = h.find(" build.sh").expect("the row");
        format!("{:?}", h.buffer()[(u16::try_from(x - 1).expect("x"), u16::try_from(y).expect("y"))].fg)
    };
    assert_ne!(cell(&coloured), cell(&plain), "the setting colours the icon");

    let mut coloured16 = coloured;
    let mut plain16 = plain;
    coloured16.set_depth(ColorDepth::Ansi16);
    plain16.set_depth(ColorDepth::Ansi16);
    assert_eq!(cell(&coloured16), cell(&plain16), "sixteen colours cannot keep the families apart");
}

#[test]
fn colour_is_off_in_ascii() {
    let scratch = Scratch::new();
    scratch.write("config/explorer.conf", "colour-icons = true\n");
    let mut coloured = open(&scratch);
    fs::remove_file(scratch.path("config/explorer.conf")).expect("back to the default");
    let mut plain = open(&scratch);
    coloured.set_glyph_mode(GlyphMode::Ascii);
    plain.set_glyph_mode(GlyphMode::Ascii);
    let cell = |h: &Harness<Explorer>| {
        let (x, y) = h.find(" build.sh").expect("the row");
        format!("{:?}", h.buffer()[(u16::try_from(x - 1).expect("x"), u16::try_from(y).expect("y"))].fg)
    };
    assert_eq!(cell(&coloured), cell(&plain));
}

#[test]
fn question_mark_lists_the_keys() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    press(&mut h, "?");
    assert!(h.screen().contains("select several"), "the screen's own keys come first:\n{}", h.screen());
    // Typing filters the list, which is longer than the layer.
    h.type_text("view");
    for label in ["list view", "grid view", "tree view"] {
        assert!(h.screen().contains(label), "{label}:\n{}", h.screen());
    }
    press(&mut h, "esc");
    assert!(!h.screen().contains("list view"), "{}", h.screen());
}

#[test]
fn delete_moves_the_entry_under_the_cursor_to_the_trash() {
    let scratch = Scratch::new();
    let mut h = open_at(scratch.machine(), Start { folder: scratch.path("home"), select: Some("data.bin".into()) });
    press(&mut h, "delete");
    assert!(!scratch.path("home/data.bin").exists(), "{}", h.screen());
    assert!(scratch.path("trash/files/data.bin").exists(), "into the machine's own trash, the test's");
    assert!(!h.screen().contains("data.bin"), "{}", h.screen());
}

#[test]
fn f2_asks_for_a_new_name() {
    let scratch = Scratch::new();
    let mut h = open_at(scratch.machine(), Start { folder: scratch.path("home"), select: Some("data.bin".into()) });
    press(&mut h, "f2");
    assert!(h.screen().contains("Rename"), "{}", h.screen());
}

#[test]
fn a_program_carries_the_program_icon_in_the_grid_and_the_tree_too() {
    let scratch = Scratch::new();
    scratch.write("home/install", "#!/bin/sh");
    fs::set_permissions(scratch.path("home/install"), fs::Permissions::from_mode(0o755)).expect("mode");
    scratch.write("home/draft", "plain");
    // Each view is the one qexp starts in: neither reads a row's permissions for its columns, as
    // the list does, so the run bit has to come with the folder's entries.
    for view in ["grid", "tree"] {
        scratch.write("config/explorer.conf", &format!("view = \"{view}\"\n"));
        let h = open(&scratch);
        assert_eq!(icon_before(&h, "install"), glyph(&h, "file-executable"), "{view}:\n{}", h.screen());
        assert_eq!(icon_before(&h, "draft"), glyph(&h, "file"), "{view}: no run bit, no program");
    }
}
