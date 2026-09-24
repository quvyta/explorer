//! The actions on a row: deleting for good only after a question, the "Open with" dialog, and
//! extracting an archive from its menu. Every test starts where a person acts, with a key or a
//! click, and reads what is drawn, the files in the scratch folder and what the harness recorded.

use qframe::event::{MouseButton, MouseKind};

use super::*;

/// The screen opened with the cursor on `name` in `folder` of the scratch machine.
fn at(machine: Machine, scratch: &Scratch, folder: &str, name: &str) -> Harness<Explorer> {
    open_at(machine, Start { folder: scratch.path(folder), select: Some(name.to_owned()) })
}

/// Every file below `folder`, at any depth.
fn files_below(folder: &std::path::Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(folder).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_below(&path));
        } else {
            found.push(path);
        }
    }
    found
}

#[test]
fn shift_delete_asks_first_and_delete_moves_to_the_trash() {
    let scratch = Scratch::new();
    let mut h = at(scratch.machine(), &scratch, "home/notes", "todo.md");
    press(&mut h, "shift+delete");
    assert!(h.screen().contains("Delete todo.md?"), "a question is asked:\n{}", h.screen());
    assert!(scratch.path("home/notes/todo.md").exists(), "nothing is deleted before the answer");

    press(&mut h, "esc");
    assert!(!h.screen().contains("Delete todo.md?"), "{}", h.screen());
    assert!(scratch.path("home/notes/todo.md").exists(), "Esc keeps the file");

    press(&mut h, "delete");
    assert!(!scratch.path("home/notes/todo.md").exists(), "{}", h.screen());
    let trashed = files_below(&scratch.path("trash"));
    assert!(
        trashed.iter().any(|path| fs::read_to_string(path).is_ok_and(|text| text == "milk")),
        "the file is in the test's trash: {trashed:?}"
    );
}

// --- Open with ---

/// A desktop of the scratch machine: a PDF viewer with windows as the default for PDFs, a terminal
/// editor for plain text, which Markdown is a kind of, and Markdown described in words.
fn desktop(scratch: &Scratch) {
    scratch.write("system/mime/globs2", "50:application/pdf:*.pdf\n50:text/markdown:*.md\n");
    scratch.write("system/mime/subclasses", "text/markdown text/plain\n");
    scratch.write(
        "system/mime/text/markdown.xml",
        "<mime-type type=\"text/markdown\">\n  <comment>Markdown document</comment>\n  \
         <comment xml:lang=\"tr\">Markdown belgesi</comment>\n</mime-type>\n",
    );
    scratch.write(
        "data/applications/viewer.desktop",
        "[Desktop Entry]\nType=Application\nName=Viewer\nExec=viewer --page 1 %f\nMimeType=application/pdf;\n",
    );
    scratch.write(
        "data/applications/reader.desktop",
        "[Desktop Entry]\nType=Application\nName=Reader\nExec=reader %f\nMimeType=application/pdf;\n",
    );
    scratch.write(
        "data/applications/vim.desktop",
        "[Desktop Entry]\nType=Application\nName=Vim\nExec=vim %F\nTerminal=true\nMimeType=text/plain;\n",
    );
    scratch.write("home/.config/mimeapps.list", "[Default Applications]\napplication/pdf=viewer.desktop\n");
}

/// Right-clicks the row showing `name` and clicks `item` on the menu that opens.
pub(super) fn menu_item(h: &mut Harness<Explorer>, name: &str, item: &str) {
    let (x, y) = h.find(name).unwrap_or_else(|| panic!("no {name} row:\n{}", h.screen()));
    h.mouse(MouseKind::Down(MouseButton::Right), x, y);
    h.mouse(MouseKind::Up(MouseButton::Right), x, y);
    settle(h);
    click(h, item);
}

#[test]
fn ctrl_enter_lists_the_programs_of_the_kind_and_the_terminal_editor() {
    let scratch = Scratch::new();
    desktop(&scratch);
    let mut h = at(scratch.machine(), &scratch, "home/notes", "todo.md");
    press(&mut h, "ctrl+enter");
    let screen = h.screen();
    assert!(screen.contains("Markdown document · text/markdown"), "the kind in words and its name:\n{screen}");
    assert!(line_with(&h, "Vim").contains("default"), "{screen}");
    assert!(line_with(&h, "Terminal editor").contains("nvim"), "the editor says which program it is:\n{screen}");
    assert!(h.handoffs().is_empty() && h.opens().is_empty(), "asking opens nothing");

    click(&mut h, "Terminal editor");
    let handoffs = h.handoffs();
    assert_eq!(handoffs.len(), 1, "{}", h.screen());
    assert_eq!(handoffs[0].program, "nvim");
    assert_eq!(handoffs[0].args, [scratch.path("home/notes/todo.md").into_os_string()]);
    assert_eq!(handoffs[0].dir, Some(scratch.path("home/notes")), "the editor starts beside the file");
    assert!(!h.screen().contains("Terminal editor"), "the dialog closes:\n{}", h.screen());
}

#[test]
fn the_kind_is_described_in_the_language_of_the_screen() {
    let scratch = Scratch::new();
    desktop(&scratch);
    let mut h = at(scratch.machine(), &scratch, "home/notes", "todo.md");
    h.set_locale("tr");
    press(&mut h, "ctrl+enter");
    assert!(h.screen().contains("Markdown belgesi · text/markdown"), "{}", h.screen());
    assert!(h.screen().contains("Terminal düzenleyicisi"), "{}", h.screen());
}

#[test]
fn the_menu_opens_the_dialog_and_enter_starts_the_default_program() {
    let scratch = Scratch::new();
    desktop(&scratch);
    let mut machine = scratch.machine();
    machine.graphical = true;
    let mut h = open_at(machine, Start { folder: scratch.path("home/Documents"), select: None });
    menu_item(&mut h, "report.pdf", "Open with…");
    let screen = h.screen();
    let viewer = screen.find("Viewer").expect("the default program is listed");
    let reader = screen.find("Reader").expect("the other program is listed");
    assert!(viewer < reader, "the default comes first:\n{screen}");
    assert!(line_with(&h, "Viewer").contains("default"), "{screen}");

    press(&mut h, "enter");
    let opens = h.opens();
    assert_eq!(opens.len(), 1, "{}", h.screen());
    assert_eq!(opens[0].program, "viewer");
    assert_eq!(opens[0].dir, Some(scratch.path("home/Documents")), "the program starts beside the file");
    assert!(h.handoffs().is_empty());
}

#[test]
fn without_a_graphical_desktop_its_programs_are_faint_and_cannot_be_chosen() {
    let scratch = Scratch::new();
    desktop(&scratch);
    let mut h = at(scratch.machine(), &scratch, "home/Documents", "report.pdf");
    press(&mut h, "ctrl+enter");
    assert!(line_with(&h, "Viewer").contains("no graphical desktop"), "{}", h.screen());
    assert!(line_with(&h, "Reader").contains("no graphical desktop"), "{}", h.screen());
    click(&mut h, "Viewer");
    assert!(h.opens().is_empty() && h.handoffs().is_empty(), "nothing is started");
    assert!(h.screen().contains("Viewer"), "the dialog stays open:\n{}", h.screen());
}

#[test]
fn esc_and_the_close_mark_close_the_dialog_without_opening_anything() {
    let scratch = Scratch::new();
    desktop(&scratch);
    let mut h = at(scratch.machine(), &scratch, "home/notes", "todo.md");
    press(&mut h, "ctrl+enter");
    assert!(h.screen().contains("Terminal editor"), "{}", h.screen());
    press(&mut h, "esc");
    assert!(!h.screen().contains("Terminal editor"), "Esc closes it:\n{}", h.screen());

    press(&mut h, "ctrl+enter");
    assert!(h.screen().contains("Terminal editor"), "{}", h.screen());
    click(&mut h, "×");
    assert!(!h.screen().contains("Terminal editor"), "the close mark closes it:\n{}", h.screen());
    assert!(h.handoffs().is_empty() && h.opens().is_empty());
}

#[test]
fn ctrl_enter_on_a_folder_opens_no_dialog() {
    let scratch = Scratch::new();
    desktop(&scratch);
    let mut h = at(scratch.machine(), &scratch, "home", "notes");
    press(&mut h, "ctrl+enter");
    assert!(!h.screen().contains("Terminal editor"), "{}", h.screen());
}

// --- Archives ---

/// The folders the real tools are looked for in, as `src/archive/fixture.rs` does.
const SYSTEM_BIN: &[&str] = &["/usr/local/bin", "/usr/bin", "/bin"];

/// The real tool `name`; `None`, with the reason printed, when it is not installed.
fn tool(name: &str) -> Option<PathBuf> {
    let found = SYSTEM_BIN.iter().map(|folder| std::path::Path::new(folder).join(name)).find(|path| path.is_file());
    if found.is_none() {
        eprintln!("skipped: {name} is not installed");
    }
    found
}

/// Puts links to the real tools named into the scratch machine's search path, the only one its
/// screen looks in; `false`, with the reason printed, when one is not installed.
fn with_tools(scratch: &Scratch, names: &[&str]) -> bool {
    for name in names {
        let Some(real) = tool(name) else { return false };
        std::os::unix::fs::symlink(real, scratch.path(&format!("bin/{name}"))).expect("link");
    }
    true
}

/// A real `yedek.tar.gz` in the home folder holding `a.txt` and `inner/b.txt`, made with the real
/// `bsdtar`.
fn backup(scratch: &Scratch) {
    scratch.write("source/a.txt", "first");
    scratch.write("source/inner/b.txt", "second");
    let made = std::process::Command::new(tool("bsdtar").expect("bsdtar"))
        .args(["-czf", "home/yedek.tar.gz", "-C", "source", "."])
        .current_dir(scratch.path(""))
        .stdin(std::process::Stdio::null())
        .status()
        .expect("bsdtar runs");
    assert!(made.success());
    fs::remove_dir_all(scratch.path("source")).expect("the source goes");
}

/// A tool standing in for `bsdtar` in the scratch search path, running `body` as a shell script,
/// with the system's own folders after it so the script finds `sleep`.
fn fake_bsdtar(scratch: &Scratch, body: &str) -> Machine {
    let path = scratch.path("bin/bsdtar");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("mode");
    let mut machine = scratch.machine();
    let folders = std::iter::once(scratch.path("bin")).chain(SYSTEM_BIN.iter().map(PathBuf::from));
    machine.path_var = Some(std::env::join_paths(folders).expect("search path"));
    machine
}

/// Lets the clock run in short steps, with the real clock beside it for the tools, until `done`;
/// fails with the screen after a generous bound.
pub(super) fn wait_until(h: &mut Harness<Explorer>, what: &str, done: impl Fn(&Harness<Explorer>) -> bool) {
    let started = std::time::Instant::now();
    while !done(h) {
        assert!(started.elapsed() < Duration::from_secs(30), "{what} never happened:\n{}", h.screen());
        std::thread::sleep(Duration::from_millis(10));
        h.advance(Duration::from_millis(100));
    }
}

/// The names in the scratch folder `relative`, sorted.
fn listing(scratch: &Scratch, relative: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(scratch.path(relative))
        .expect("folder")
        .map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn extract_here_makes_a_new_folder_beside_the_archive_and_selects_it() {
    let scratch = Scratch::new();
    if !with_tools(&scratch, &["bsdtar"]) {
        return;
    }
    backup(&scratch);
    let mut h = open(&scratch);
    menu_item(&mut h, "yedek.tar.gz", "Extract here");
    wait_until(&mut h, "the toast", |h| h.screen().contains("yedek.tar.gz extracted"));
    assert_eq!(fs::read_to_string(scratch.path("home/yedek/a.txt")).expect("extracted"), "first");
    assert_eq!(fs::read_to_string(scratch.path("home/yedek/inner/b.txt")).expect("extracted"), "second");
    settle(&mut h);
    assert_eq!(h.app().selected(), Some(scratch.path("home/yedek")), "{}", h.screen());
    assert!(line_with(&h, "build.sh").contains("10 B"), "the folder read again shows its details:\n{}", h.screen());
    assert!(!listing(&scratch, "home").iter().any(|name| name.starts_with(".yedek")), "no temporary folder is left");
}

#[test]
fn extract_to_asks_for_a_folder_and_makes_the_new_folder_there() {
    let scratch = Scratch::new();
    if !with_tools(&scratch, &["bsdtar"]) {
        return;
    }
    backup(&scratch);
    let mut h = open(&scratch);
    menu_item(&mut h, "yedek.tar.gz", "Extract to…");
    assert!(h.screen().contains("Extract yedek.tar.gz to"), "the picker opens:\n{}", h.screen());
    // The sidebar and the folder behind the dialog show Documents too; the picker's row is the
    // first one below its way to the parent folder.
    let screen = h.screen();
    let (x, y) = screen
        .lines()
        .enumerate()
        .skip_while(|(_, line)| !line.contains("Parent folder"))
        .find_map(|(row, line)| {
            let at = line.find("Documents")?;
            Some((i32::try_from(line[..at].chars().count()).ok()?, i32::try_from(row).ok()?))
        })
        .unwrap_or_else(|| panic!("no Documents row in the picker:\n{screen}"));
    h.click(x, y);
    settle(&mut h);
    click(&mut h, "Choose folder");
    wait_until(&mut h, "the toast", |h| h.screen().contains("yedek.tar.gz extracted"));
    assert_eq!(fs::read_to_string(scratch.path("home/Documents/yedek/a.txt")).expect("extracted"), "first");
    assert!(!scratch.path("home/yedek").exists(), "nothing is made beside the archive");
    settle(&mut h);
    assert_eq!(h.app().folder(), scratch.path("home/Documents"), "{}", h.screen());
    assert_eq!(h.app().selected(), Some(scratch.path("home/Documents/yedek")), "{}", h.screen());
}

#[test]
fn without_a_tool_the_archive_items_are_faint_and_name_the_tool() {
    let scratch = Scratch::new();
    scratch.write("home/yedek.tar.gz", "not read at all");
    let mut h = open(&scratch);
    menu_item(&mut h, "yedek.tar.gz", "Extract here");
    assert!(line_with(&h, "Extract to…").contains("bsdtar needed"), "{}", h.screen());
    settle(&mut h);
    assert_eq!(listing(&scratch, "home").iter().filter(|name| name.contains("yedek")).count(), 1, "nothing is made");
    assert!(!h.screen().contains("could not be extracted"), "{}", h.screen());
}

#[test]
fn a_slow_extraction_shows_in_the_footer_and_its_cancel_button_stops_it() {
    let scratch = Scratch::new();
    if tool("sleep").is_none() {
        return;
    }
    let machine = fake_bsdtar(&scratch, "for last; do :; done\n: > \"$last/half\"\nexec sleep 60");
    scratch.write("home/yedek.tar.gz", "whatever is inside");
    let mut h = open_at(machine, Start { folder: scratch.path("home"), select: None });
    let (x, y) = h.find("yedek.tar.gz").expect("the archive row");
    h.mouse(MouseKind::Down(MouseButton::Right), x, y);
    h.mouse(MouseKind::Up(MouseButton::Right), x, y);
    h.click_text("Extract here");
    h.advance(Duration::from_millis(100));
    assert!(!h.screen().contains("is being extracted"), "a quick look shows nothing yet:\n{}", h.screen());

    let half = || {
        fs::read_dir(scratch.path("home"))
            .expect("home")
            .filter_map(Result::ok)
            .any(|entry| entry.path().join("half").exists())
    };
    wait_until(&mut h, "the tool writing", |_| half());
    wait_until(&mut h, "the footer", |h| h.screen().contains("yedek.tar.gz is being extracted"));
    click(&mut h, "Cancel");
    wait_until(&mut h, "the footer to clear", |h| !h.screen().contains("is being extracted"));
    assert_eq!(
        listing(&scratch, "home").iter().filter(|name| name.contains("yedek")).collect::<Vec<_>>(),
        ["yedek.tar.gz"],
        "the half-written folder is gone"
    );
    assert!(!h.screen().contains("could not be extracted"), "a cancel is no failure:\n{}", h.screen());
}

#[test]
fn an_encrypted_archive_says_it_is_not_supported_yet() {
    let scratch = Scratch::new();
    let machine = fake_bsdtar(&scratch, "echo 'Incorrect passphrase' >&2\nexit 1");
    scratch.write("home/gizli.zip", "whatever is inside");
    let mut h = open_at(machine, Start { folder: scratch.path("home"), select: None });
    menu_item(&mut h, "gizli.zip", "Extract here");
    wait_until(&mut h, "the toast", |h| h.screen().contains("gizli.zip could not be extracted"));
    assert!(h.screen().contains("the archive is encrypted"), "{}", h.screen());
    assert_eq!(listing(&scratch, "home").iter().filter(|name| name.contains("gizli")).count(), 1);
}
