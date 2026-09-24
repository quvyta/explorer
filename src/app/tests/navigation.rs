//! Moving between folders from the keys, the buttons, the path and the places.

use super::*;

/// The longest one wait of a followed folder lasts in these tests.
const WATCH: Duration = Duration::from_millis(20);

#[test]
fn it_opens_in_the_home_folder_with_the_places_that_exist() {
    let scratch = Scratch::new();
    let h = open(&scratch);
    assert_eq!(h.app().folder(), scratch.path("home"));
    let screen = h.screen();
    for place in ["Home", "Documents", "Pictures", "Root"] {
        assert!(screen.contains(place), "{place} is a place:\n{screen}");
    }
    assert!(!screen.contains("Music"), "a user folder that is not there is not a place:\n{screen}");
    assert!(line_with(&h, "notes").contains("notes"), "{screen}");
    assert!(!screen.contains(".hidden"), "hidden entries start hidden:\n{screen}");
}

#[test]
fn a_file_given_at_start_opens_its_folder_with_the_file_under_the_cursor() {
    let scratch = Scratch::new();
    let h = open_at(scratch.machine(), Start { folder: scratch.path("home/notes"), select: Some("todo.md".into()) });
    assert_eq!(h.app().folder(), scratch.path("home/notes"));
    assert_eq!(h.app().selected(), Some(scratch.path("home/notes/todo.md")));
}

#[test]
fn the_keys_reach_the_rows_from_the_start() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    // The folder's own row, then Documents, Pictures and notes.
    for _ in 0..4 {
        h.press("down");
    }
    assert_eq!(h.app().selected(), Some(scratch.path("home/notes")), "{}", h.screen());
    press(&mut h, "enter");
    assert_eq!(h.app().folder(), scratch.path("home/notes"), "{}", h.screen());
    assert!(h.screen().contains("todo.md"), "{}", h.screen());
}

#[test]
fn backspace_goes_up_and_leaves_the_cursor_on_the_folder_it_came_out_of() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    double_click(&mut h, "notes");
    assert_eq!(h.app().folder(), scratch.path("home/notes"), "a double click on a folder goes in:\n{}", h.screen());
    press(&mut h, "backspace");
    assert_eq!(h.app().folder(), scratch.path("home"));
    assert_eq!(h.app().selected(), Some(scratch.path("home/notes")));
    press(&mut h, "alt+up");
    assert_eq!(h.app().folder(), scratch.path(""), "alt+up goes up too");
    assert_eq!(h.app().selected(), Some(scratch.path("home")));
}

#[test]
fn the_up_button_goes_up_and_stays_put_at_the_root() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    click_icon(&mut h, "arrow-up");
    assert_eq!(h.app().folder(), scratch.path(""));
    click_icon(&mut h, "arrow-up");
    assert_eq!(h.app().folder(), scratch.path(""), "there is nothing above the root");
}

#[test]
fn back_and_forward_walk_the_history_like_a_browser() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    double_click(&mut h, "notes");
    double_click(&mut h, "deep");
    assert_eq!(h.app().folder(), scratch.path("home/notes/deep"));

    click_icon(&mut h, "arrow-left");
    assert_eq!(h.app().folder(), scratch.path("home/notes"));
    assert_eq!(h.app().selected(), Some(scratch.path("home/notes/deep")), "back to a folder above: on the way down");
    press(&mut h, "alt+left");
    assert_eq!(h.app().folder(), scratch.path("home"));

    click_icon(&mut h, "arrow-right");
    assert_eq!(h.app().folder(), scratch.path("home/notes"));
    press(&mut h, "alt+right");
    assert_eq!(h.app().folder(), scratch.path("home/notes/deep"));

    // Going somewhere new lets go of what lay ahead.
    press(&mut h, "alt+left");
    press(&mut h, "alt+left");
    assert_eq!(h.app().folder(), scratch.path("home"));
    click(&mut h, "Documents");
    press(&mut h, "alt+right");
    assert_eq!(h.app().folder(), scratch.path("home/Documents"), "nothing ahead after a new step");
}

#[test]
fn a_part_of_the_path_goes_to_its_folder() {
    let scratch = Scratch::new();
    let mut h = open_at(scratch.machine(), Start { folder: scratch.path("home/notes/deep"), select: None });
    let (x, y) = find_in_row(&h, "Home", 0).expect("the path starts at home");
    h.click(x, y);
    settle(&mut h);
    assert_eq!(h.app().folder(), scratch.path("home"));
    assert_eq!(h.app().selected(), Some(scratch.path("home/notes")), "on the way down");
}

#[test]
fn a_place_goes_there_by_click_and_by_alt_and_its_number() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    let (x, y) = h.find("Pictures").expect("the place");
    assert!(x < 22, "the first Pictures on screen is the place, in the sidebar");
    h.click(x, y);
    settle(&mut h);
    assert_eq!(h.app().folder(), scratch.path("home/Pictures"));
    press(&mut h, "alt+2");
    assert_eq!(h.app().folder(), scratch.path("home/Documents"), "the second place");
    press(&mut h, "alt+4");
    assert_eq!(h.app().folder(), scratch.path(""), "the fourth place is the root");
    press(&mut h, "alt+9");
    assert_eq!(h.app().folder(), scratch.path(""), "a place that is not there goes nowhere");
}

#[test]
fn the_place_the_folder_is_in_carries_the_pillar() {
    let scratch = Scratch::new();
    let mut h = open_at(scratch.machine(), Start { folder: scratch.path("home/Documents"), select: None });
    // The places are the lines' first cells; the pillar stands before the place's icon.
    let pillar = |h: &Harness<Explorer>, place: &str| {
        h.screen()
            .lines()
            .any(|line| line.starts_with('▌') && line.chars().take(22).collect::<String>().contains(place))
    };
    assert!(pillar(&h, "Documents"), "{}", h.screen());
    assert!(!pillar(&h, "Home"), "{}", h.screen());
    press(&mut h, "alt+up");
    assert!(pillar(&h, "Home"), "{}", h.screen());
    press(&mut h, "alt+up");
    assert!(pillar(&h, "Root"), "{}", h.screen());
}

#[test]
fn ctrl_l_goes_to_a_typed_path_and_a_wrong_one_is_said_under_the_field() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    press(&mut h, "ctrl+l");
    assert!(h.is_focused("location"), "{}", h.screen());
    press(&mut h, "ctrl+u");
    h.type_text("~/nowhere");
    press(&mut h, "enter");
    assert!(h.screen().contains("There is nothing at ~/nowhere"), "{}", h.screen());
    assert_eq!(h.app().folder(), scratch.path("home"), "the screen does not change");

    press(&mut h, "ctrl+u");
    h.type_text("~/notes/todo.md");
    press(&mut h, "enter");
    assert_eq!(h.app().folder(), scratch.path("home/notes"), "{}", h.screen());
    assert_eq!(h.app().selected(), Some(scratch.path("home/notes/todo.md")), "a file is selected in its folder");
    assert!(!h.screen().contains("Go to"), "the field is the path again:\n{}", h.screen());

    press(&mut h, "ctrl+l");
    press(&mut h, "ctrl+u");
    h.type_text("deep");
    press(&mut h, "enter");
    assert_eq!(h.app().folder(), scratch.path("home/notes/deep"), "a relative path starts at the folder shown");
}

#[test]
fn esc_turns_the_field_back_into_the_path_and_q_typed_there_does_not_quit() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    click_icon(&mut h, "prompt");
    assert!(h.screen().contains("Go to"), "the button opens the field too:\n{}", h.screen());
    h.type_text("q");
    assert!(!h.quit_requested(), "a q typed into the path is a letter");
    press(&mut h, "esc");
    assert!(!h.screen().contains("Go to"), "{}", h.screen());
    assert_eq!(h.app().folder(), scratch.path("home"));
    press(&mut h, "q");
    assert!(h.quit_requested(), "q quits from the rows");
}

#[test]
fn a_folder_that_went_away_leaves_the_person_in_the_nearest_one_with_a_note() {
    let scratch = Scratch::new();
    let mut machine = scratch.machine();
    machine.following = Following::Within(WATCH);
    let mut h = open_at(machine, Start { folder: scratch.path("home"), select: None });
    double_click(&mut h, "notes");
    double_click(&mut h, "deep");
    press(&mut h, "alt+left");
    press(&mut h, "alt+left");
    fs::remove_dir_all(scratch.path("home/notes")).expect("another program deletes it");
    press(&mut h, "alt+right");
    assert_eq!(h.app().folder(), scratch.path("home"), "{}", h.screen());
    assert!(h.screen().contains("~/notes is no"), "{}", h.screen());
}

#[test]
fn a_folder_deleted_while_it_is_shown_leaves_for_the_one_above_with_a_note() {
    let scratch = Scratch::new();
    let mut machine = scratch.machine();
    machine.following = Following::Within(WATCH);
    let mut h = open_at(machine, Start { folder: scratch.path("home"), select: None });
    double_click(&mut h, "notes");
    fs::remove_dir_all(scratch.path("home/notes")).expect("another program deletes it");
    settle(&mut h);
    assert_eq!(h.app().folder(), scratch.path("home"), "{}", h.screen());
    assert!(h.screen().contains("~/notes is no"), "{}", h.screen());
}

#[test]
fn below_ninety_columns_the_places_open_over_the_folder_with_ctrl_b() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    h.resize(80, 24);
    settle(&mut h);
    // Root is a place and nothing else on this screen.
    assert!(!h.screen().contains("Root"), "{}", h.screen());
    press(&mut h, "ctrl+b");
    assert!(h.screen().contains("Root"), "{}", h.screen());
    let (x, y) = h.find("Pictures").expect("the place");
    h.click(x, y);
    settle(&mut h);
    assert_eq!(h.app().folder(), scratch.path("home/Pictures"));
    assert!(!h.screen().contains("Root"), "choosing a place closes them:\n{}", h.screen());
    click_icon(&mut h, "edge-right");
    assert!(h.screen().contains("Root"), "the strip's button opens them too:\n{}", h.screen());
    press(&mut h, "esc");
    assert!(!h.screen().contains("Root"), "{}", h.screen());
}

#[test]
fn a_narrow_screen_keeps_the_folder_and_the_path_and_a_tiny_one_says_so() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    h.resize(40, 12);
    settle(&mut h);
    let screen = h.screen();
    assert!(screen.contains("Home") && screen.contains("notes"), "{screen}");
    assert!(!screen.contains(&glyph(&h, "arrow-left")), "no strip of buttons:\n{screen}");
    h.resize(18, 5);
    assert!(h.screen().contains("The terminal"), "{}", h.screen());
}

#[test]
fn a_folder_made_after_its_parent_was_read_is_still_found() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    fs::create_dir_all(scratch.path("home/later")).expect("another program makes it");
    press(&mut h, "ctrl+l");
    press(&mut h, "ctrl+u");
    h.type_text("~/later");
    press(&mut h, "enter");
    assert_eq!(h.app().folder(), scratch.path("home/later"), "{}", h.screen());
    assert!(!h.screen().contains("no longer"), "{}", h.screen());
}

#[test]
fn a_folder_that_may_not_be_read_says_so_and_its_path_is_faint() {
    let scratch = Scratch::new();
    let locked = scratch.path("home/notes");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("mode");
    let mut h = open(&scratch);
    double_click(&mut h, "notes");
    let screen = h.screen();
    let home = crumb_colour(&h, "Home");
    // Put back, so the scratch folder can be removed whatever the assertions say.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("mode");
    assert_eq!(h.app().folder(), scratch.path("home/notes"), "{screen}");
    assert!(!screen.contains("Empty") && !screen.contains("todo.md"), "{screen}");
    assert!(screen.contains("cannot be read"), "{screen}");

    let theme = h.env().theme();
    let (muted, dim) = (theme.color("muted"), theme.color("dim"));
    assert!(muted.is_some() && muted != dim, "the theme tells faint from plain");
    assert_eq!(home, muted, "the path to a folder that may not be read is faint:\n{screen}");
    let readable = open_at(scratch.machine(), Start { folder: scratch.path("home/notes"), select: None });
    assert_eq!(crumb_colour(&readable, "Home"), dim, "and plain to one that may:\n{}", readable.screen());
}

/// The colour the part `name` of the path is drawn in, on the top line.
fn crumb_colour(h: &Harness<Explorer>, name: &str) -> Option<qframe::color::Rgb> {
    let (x, y) = find_in_row(h, name, 0).unwrap_or_else(|| panic!("no {name} in the path:\n{}", h.screen()));
    h.fg(u16::try_from(x).expect("on screen"), u16::try_from(y).expect("on screen"))
}

/// The arrows move through the folder after each way of getting there that leaves the keyboard
/// somewhere else: a click on a place, on the back and forward buttons and on a part of the path.
#[test]
fn the_arrows_move_through_the_folder_after_a_click_that_went_there() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    let (x, y) = h.find("Documents").expect("the place");
    h.click(x, y);
    settle(&mut h);
    press(&mut h, "down");
    // The cursor starts on no row; the first ↓ puts it on the folder's own.
    assert_eq!(h.app().selected(), Some(scratch.path("home/Documents")), "after a place:\n{}", h.screen());

    press(&mut h, "alt+1");
    double_click(&mut h, "notes");
    click_icon(&mut h, "arrow-left");
    assert_eq!(h.app().selected(), Some(scratch.path("home/notes")));
    press(&mut h, "up");
    assert_eq!(h.app().selected(), Some(scratch.path("home/Pictures")), "after back:\n{}", h.screen());

    click_icon(&mut h, "arrow-right");
    press(&mut h, "down");
    assert_eq!(h.app().selected(), Some(scratch.path("home/notes")), "after forward:\n{}", h.screen());

    let (x, y) = find_in_row(&h, "Home", 0).expect("the path starts at home");
    h.click(x, y);
    settle(&mut h);
    press(&mut h, "up");
    assert_eq!(h.app().selected(), Some(scratch.path("home/Pictures")), "after the path:\n{}", h.screen());
}

#[test]
fn the_arrows_move_through_the_grid_and_the_tree_after_a_click_on_the_picker() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    click(&mut h, "grid");
    press(&mut h, "down");
    assert_eq!(h.app().selected(), Some(scratch.path("home")), "the grid:\n{}", h.screen());
    press(&mut h, "right");
    assert_eq!(h.app().selected(), Some(scratch.path("home/Documents")), "the grid:\n{}", h.screen());
    click(&mut h, "tree");
    press(&mut h, "down");
    assert_eq!(h.app().selected(), Some(scratch.path("home/Pictures")), "the tree:\n{}", h.screen());
}

#[test]
fn the_arrows_move_through_the_folder_after_the_path_field_and_the_settings_close() {
    let scratch = Scratch::new();
    let mut h = open(&scratch);
    press(&mut h, "ctrl+l");
    press(&mut h, "esc");
    press(&mut h, "down");
    assert_eq!(h.app().selected(), Some(scratch.path("home")), "after the path field:\n{}", h.screen());
    press(&mut h, "ctrl+,");
    press(&mut h, "esc");
    press(&mut h, "down");
    assert_eq!(h.app().selected(), Some(scratch.path("home/Documents")), "after the settings:\n{}", h.screen());
}
