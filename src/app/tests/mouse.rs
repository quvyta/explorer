//! The mouse as in a desktop file explorer: a click selects and opens nothing, a double click
//! opens, and a file dragged onto a folder moves there. Every test starts from a press on the
//! screen; nothing is started, the harness records what would be.

use super::*;

#[test]
fn a_click_on_a_file_selects_it_and_opens_nothing() {
    let scratch = Scratch::new();
    let mut h = open_at(scratch.machine(), Start { folder: scratch.path("home/notes"), select: None });
    click(&mut h, "todo.md");
    assert_eq!(h.app().selected(), Some(scratch.path("home/notes/todo.md")), "the click selects:\n{}", h.screen());
    assert!(h.handoffs().is_empty() && h.opens().is_empty(), "a click starts no program");
    assert!(!h.screen().contains("Open with"), "nor asks which program:\n{}", h.screen());
}

#[test]
fn a_double_click_on_a_file_opens_it_once() {
    let scratch = Scratch::new();
    scratch.write(
        "data/applications/vim.desktop",
        "[Desktop Entry]\nType=Application\nName=Vim\nExec=vim %F\nTerminal=true\nMimeType=text/plain;\n",
    );
    let mut h = open_at(scratch.machine(), Start { folder: scratch.path("home/notes"), select: None });
    double_click(&mut h, "todo.md");
    assert_eq!(h.handoffs().len(), 1, "one double click, one program:\n{}", h.screen());
}

#[test]
fn a_click_on_a_folder_selects_it_and_stays_where_it_is() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    click(&mut h, "notes");
    assert_eq!(h.app().folder(), scratch.path("home"), "a click does not go in:\n{}", h.screen());
    assert_eq!(h.app().selected(), Some(scratch.path("home/notes")));
    double_click(&mut h, "notes");
    assert_eq!(h.app().folder(), scratch.path("home/notes"), "a double click does:\n{}", h.screen());
}

#[test]
fn a_file_dragged_onto_a_folder_moves_there() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    let from = h.find("build.sh").expect("the file");
    let to = h.find("notes").expect("the folder");
    h.drag(from, to);
    settle(&mut h);
    assert!(scratch.path("home/notes/build.sh").exists(), "moved into the folder:\n{}", h.screen());
    assert!(!scratch.path("home/build.sh").exists(), "and gone from where it was");
}
