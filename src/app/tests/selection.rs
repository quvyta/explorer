//! Selecting several entries from the keys and dropping on the way up, as in a desktop file
//! explorer. The file manager qexp's screen is built on does the work; these tests show it reaches
//! a person on qexp's own screen, where qexp's keys and dialogs sit around it.

use super::*;

/// A folder of four files, `home/batch`, shown in the list with nothing selected. The files sort
/// as four, one, three, two.
fn batch(scratch: &Scratch) -> Harness<Explorer> {
    for name in ["one.txt", "two.txt", "three.txt", "four.txt"] {
        scratch.write(&format!("home/batch/{name}"), name);
    }
    open_at(scratch.machine(), Start { folder: scratch.path("home/batch"), select: None })
}

/// What qexp's file manager holds as chosen, as paths.
fn chosen(h: &Harness<Explorer>) -> Vec<PathBuf> {
    let files = &h.app().files;
    files.chosen().iter().map(|key| files.path(key)).collect()
}

/// `names` inside `home/batch`, in the order the screen shows them.
fn in_batch(scratch: &Scratch, names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(|name| scratch.path(&format!("home/batch/{name}"))).collect()
}

#[test]
fn shift_with_the_down_arrow_twice_selects_three_files_and_ctrl_c_copies_all_three() {
    let scratch = Scratch::new();
    let mut h = batch(&scratch);
    click(&mut h, "four.txt");
    press(&mut h, "shift+down");
    press(&mut h, "shift+down");
    assert_eq!(chosen(&h), in_batch(&scratch, &["four.txt", "one.txt", "three.txt"]), "{}", h.screen());
    // The selection the paste carries is the one on screen, not only the entry under the cursor.
    press(&mut h, "ctrl+c");
    press(&mut h, "alt+2");
    assert_eq!(h.app().folder(), scratch.path("home/Documents"), "{}", h.screen());
    press(&mut h, "ctrl+v");
    for name in ["four.txt", "one.txt", "three.txt"] {
        let copy = fs::read_to_string(scratch.path(&format!("home/Documents/{name}"))).ok();
        assert_eq!(copy.as_deref(), Some(name), "{name} copied:\n{}", h.screen());
        assert!(scratch.path(&format!("home/batch/{name}")).exists(), "a copy leaves {name} where it was");
    }
    assert!(!scratch.path("home/Documents/two.txt").exists(), "the file left out stays out");
}

#[test]
fn ctrl_a_selects_every_entry_but_the_folder_itself_in_the_list_and_the_icons() {
    for view in ["ctrl+1", "ctrl+2"] {
        let scratch = Scratch::new();
        let mut h = batch(&scratch);
        press(&mut h, view);
        click(&mut h, "one.txt");
        press(&mut h, "ctrl+a");
        // The folder's own row is where the person is, not an entry to carry away.
        let every = in_batch(&scratch, &["four.txt", "one.txt", "three.txt", "two.txt"]);
        assert_eq!(chosen(&h), every, "{view}:\n{}", h.screen());
        assert_eq!(h.app().selected(), Some(scratch.path("home/batch/one.txt")), "{view}: the cursor stays");
        assert!(h.handoffs().is_empty() && h.opens().is_empty(), "{view}: selecting opens nothing");
    }
}

#[test]
fn esc_after_selecting_several_keeps_only_the_entry_under_the_cursor() {
    let scratch = Scratch::new();
    let mut h = batch(&scratch);
    click(&mut h, "four.txt");
    press(&mut h, "shift+down");
    press(&mut h, "shift+down");
    assert_eq!(chosen(&h).len(), 3, "{}", h.screen());
    press(&mut h, "esc");
    assert_eq!(chosen(&h), in_batch(&scratch, &["three.txt"]), "{}", h.screen());
    assert_eq!(h.app().folder(), scratch.path("home/batch"), "esc goes nowhere");
}

#[test]
fn esc_still_closes_the_path_field_before_it_touches_the_selection() {
    let scratch = Scratch::new();
    let mut h = batch(&scratch);
    click(&mut h, "four.txt");
    press(&mut h, "shift+down");
    press(&mut h, "ctrl+l");
    assert!(h.app().location.is_some(), "the path field is open:\n{}", h.screen());
    press(&mut h, "esc");
    assert!(h.app().location.is_none(), "esc closes it:\n{}", h.screen());
    assert_eq!(chosen(&h).len(), 2, "and leaves the selection alone:\n{}", h.screen());
}

#[test]
fn a_file_dropped_on_the_shown_folders_own_row_moves_to_the_folder_above_in_the_list_and_the_icons() {
    for view in ["ctrl+1", "ctrl+2"] {
        let scratch = Scratch::new();
        let mut h = batch(&scratch);
        press(&mut h, view);
        // The own row is drawn with the folder's glyph; the path in the top strip has none.
        let own = format!("{} batch", glyph(&h, "folder"));
        let from = h.find("one.txt").expect("the file");
        let to = h.find(&own).unwrap_or_else(|| panic!("{view}: no own row:\n{}", h.screen()));
        assert!(to.1 > 0, "{view}: the own row, not the path");
        h.drag(from, to);
        settle(&mut h);
        assert!(scratch.path("home/one.txt").is_file(), "{view}: moved up:\n{}", h.screen());
        assert!(!scratch.path("home/batch/one.txt").exists(), "{view}: and gone from where it was");
        assert!(scratch.path("home/batch/two.txt").exists(), "{view}: only the dragged file moved");
    }
}
