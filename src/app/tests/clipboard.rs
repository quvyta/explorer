//! Cutting, copying and pasting files from the keys, as in a desktop file manager.

use super::*;

/// The screen opened in `~/Documents`, with the cursor walked onto `report.pdf` by the arrows.
fn on_the_report(scratch: &Scratch) -> Harness<Explorer> {
    let mut h = open_at(scratch.machine(), Start { folder: scratch.path("home/Documents"), select: None });
    // The folder's own row, then the file.
    press(&mut h, "down");
    press(&mut h, "down");
    assert_eq!(h.app().selected(), Some(scratch.path("home/Documents/report.pdf")), "{}", h.screen());
    h
}

#[test]
fn ctrl_c_and_ctrl_v_copy_a_file_into_another_folder() {
    let scratch = Scratch::new();
    let mut h = on_the_report(&scratch);
    press(&mut h, "ctrl+c");
    press(&mut h, "alt+1");
    assert_eq!(h.app().folder(), scratch.path("home"));
    press(&mut h, "ctrl+v");
    assert_eq!(fs::read_to_string(scratch.path("home/report.pdf")).ok().as_deref(), Some("%PDF-1.7"), "{}", h.screen());
    assert!(scratch.path("home/Documents/report.pdf").exists(), "a copy leaves the original");
    assert!(line_with(&h, "report.pdf").contains("report.pdf"), "the copy is on screen:\n{}", h.screen());
}

#[test]
fn ctrl_x_and_ctrl_v_move_a_file_into_another_folder() {
    let scratch = Scratch::new();
    let mut h = on_the_report(&scratch);
    press(&mut h, "ctrl+x");
    assert!(scratch.path("home/Documents/report.pdf").exists(), "cutting alone moves nothing");
    press(&mut h, "alt+3");
    assert_eq!(h.app().folder(), scratch.path("home/Pictures"));
    press(&mut h, "ctrl+v");
    assert!(scratch.path("home/Pictures/report.pdf").exists(), "{}", h.screen());
    assert!(!scratch.path("home/Documents/report.pdf").exists(), "a move leaves nothing behind");
}
