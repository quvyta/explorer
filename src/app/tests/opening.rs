//! Opening a file with Enter or a double click: the kind's program, in the terminal or beside it,
//! the terminal editor for text nothing else opens, and the "Open with" dialog when nothing opens
//! it. Every opening is recorded by the harness; no program is started.

use std::ffi::OsString;

use qframe::runtime::HandoffOutcome;

use super::*;

/// A desktop of the scratch machine: a PDF viewer with windows as the default for PDFs, and a
/// terminal editor for plain text, which Markdown is a kind of.
fn desktop(scratch: &Scratch) {
    scratch.write("system/mime/globs2", "50:application/pdf:*.pdf\n50:text/markdown:*.md\n");
    scratch.write("system/mime/subclasses", "text/markdown text/plain\n");
    scratch.write(
        "data/applications/viewer.desktop",
        "[Desktop Entry]\nType=Application\nName=Viewer\nExec=viewer --page 1 %f\nMimeType=application/pdf;\n",
    );
    scratch.write(
        "data/applications/vim.desktop",
        "[Desktop Entry]\nType=Application\nName=Vim\nExec=vim %F\nTerminal=true\nMimeType=text/plain;\n",
    );
    scratch.write("home/.config/mimeapps.list", "[Default Applications]\napplication/pdf=viewer.desktop\n");
}

/// The screen opened with the cursor on `name` in `folder` of the home folder.
fn at(machine: Machine, scratch: &Scratch, folder: &str, name: &str) -> Harness<Explorer> {
    open_at(machine, Start { folder: scratch.path(folder), select: Some(name.to_owned()) })
}

fn words(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn enter_hands_the_terminal_to_a_terminal_program_of_the_kind() {
    let scratch = Scratch::new();
    desktop(&scratch);
    let mut h = at(scratch.machine(), &scratch, "home/notes", "todo.md");
    press(&mut h, "enter");
    let handoffs = h.handoffs();
    assert_eq!(handoffs.len(), 1, "{}", h.screen());
    let file = scratch.path("home/notes/todo.md");
    assert_eq!(handoffs[0].program, "vim");
    assert_eq!(handoffs[0].args, [file.into_os_string()]);
    assert_eq!(handoffs[0].dir, Some(scratch.path("home/notes")), "the program starts beside the file");
    assert!(h.opens().is_empty());
    assert_eq!(h.app().folder(), scratch.path("home/notes"), "qexp is where it was when the program ends");
}

#[test]
fn a_double_click_starts_a_graphical_program_beside_qexp_when_there_is_a_graphical_session() {
    let scratch = Scratch::new();
    desktop(&scratch);
    let mut machine = scratch.machine();
    machine.graphical = true;
    let mut h = open_at(machine, Start { folder: scratch.path("home/Documents"), select: None });
    let (x, y) = h.find("report.pdf").expect("the file");
    h.click(x, y);
    h.click(x, y);
    settle(&mut h);
    let opens = h.opens();
    assert_eq!(opens.len(), 1, "{}", h.screen());
    let file = scratch.path("home/Documents/report.pdf");
    assert_eq!(opens[0].program, "viewer");
    assert_eq!(opens[0].args, [OsString::from("--page"), OsString::from("1"), file.into_os_string()]);
    assert_eq!(opens[0].dir, Some(scratch.path("home/Documents")), "the program starts beside the file");
    assert!(h.handoffs().is_empty(), "the terminal stays with qexp");
}

#[test]
fn without_a_graphical_session_a_graphical_program_is_as_good_as_not_there() {
    let scratch = Scratch::new();
    desktop(&scratch);
    let mut h = at(scratch.machine(), &scratch, "home/Documents", "report.pdf");
    press(&mut h, "enter");
    assert!(h.opens().is_empty() && h.handoffs().is_empty(), "nothing is started");
    assert!(h.screen().contains("no graphical desktop"), "the dialog says why:\n{}", h.screen());
}

#[test]
fn text_nothing_else_opens_goes_to_the_terminal_editor() {
    let scratch = Scratch::new();
    desktop(&scratch);
    fs::remove_file(scratch.path("data/applications/vim.desktop")).expect("no terminal program");
    let mut h = at(scratch.machine(), &scratch, "home/notes", "todo.md");
    press(&mut h, "enter");
    let handoffs = h.handoffs();
    assert_eq!(handoffs.len(), 1, "{}", h.screen());
    assert_eq!(handoffs[0].program, "nvim", "the machine's editor");
    assert_eq!(handoffs[0].args, words(&[scratch.path("home/notes/todo.md").to_str().expect("path")]));
    assert_eq!(handoffs[0].dir, Some(scratch.path("home/notes")), "the editor starts beside the file");
}

#[test]
fn a_file_nothing_opens_asks_what_to_open_it_with_and_starts_nothing() {
    let scratch = Scratch::new();
    desktop(&scratch);
    let mut h = at(scratch.machine(), &scratch, "home", "data.bin");
    press(&mut h, "enter");
    assert!(h.opens().is_empty() && h.handoffs().is_empty());
    assert!(h.screen().contains("application/octet-stream"), "the dialog names the kind:\n{}", h.screen());
    assert!(h.screen().contains("Terminal editor"), "{}", h.screen());
}

#[test]
fn a_program_that_cannot_be_started_is_said() {
    let scratch = Scratch::new();
    desktop(&scratch);
    let mut h = at(scratch.machine(), &scratch, "home/notes", "todo.md");
    h.set_handoff_outcome(HandoffOutcome::Failed("vim: not found".into()));
    press(&mut h, "enter");
    assert!(h.screen().contains("todo.md could not be opened"), "{}", h.screen());
}

#[test]
fn a_link_to_a_folder_is_gone_into_rather_than_opened() {
    let scratch = Scratch::new();
    desktop(&scratch);
    std::os::unix::fs::symlink(scratch.path("home/notes"), scratch.path("home/shortcut")).expect("link");
    let mut h = at(scratch.machine(), &scratch, "home", "shortcut");
    press(&mut h, "enter");
    assert_eq!(h.app().folder(), scratch.path("home/notes"), "{}", h.screen());
    assert!(h.opens().is_empty() && h.handoffs().is_empty());
}
