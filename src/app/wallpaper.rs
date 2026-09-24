//! "Set as wallpaper" on a picture's menu, when qdesk is installed.
//!
//! qexp never reads or writes qdesk's settings: it runs `qdesk wallpaper <picture>`, which checks
//! that the picture decodes, keeps it in qdesk's own file, and exits 0; a picture it cannot take
//! is refused with one line on its error output and a status that is not 0. An open desktop sees
//! the change in its file and redraws its floor. So the two applications are bound only by that
//! one command, and qdesk may change how it keeps the setting without qexp knowing.
//!
//! The command runs as a background task and never holds up drawing. The task waits on its own
//! sleep between looks at qdesk, so in the tests the harness's clock drives it and a qdesk that
//! never answers is stopped after thirty seconds of that clock.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use std::time::Duration;

use qframe::prelude::*;
use qframe::runtime::Task;
use qframe::widgets::{ContextItem, Toast};

use super::{Explorer, Msg};
use crate::programs::{drain, find, spawn_patiently};

/// The names a picture qdesk draws ends in, compared without regard to case: the formats its
/// decoder reads, PNG, JPEG, GIF and WebP.
const PICTURES: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

/// How often the task looks whether qdesk is done.
const LOOK: Duration = Duration::from_millis(50);

/// How long qdesk is given. It decodes the picture once and writes one small file, which takes a
/// moment even for a large picture on a slow disk; one that has not answered by then is stuck and
/// is stopped, rather than left running unseen.
const BOUND: Duration = Duration::from_secs(30);

/// What `qdesk wallpaper` came to. The words are put together on the screen's side, in its
/// language; the task only says what happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It exited 0: the picture is the wallpaper.
    Set,
    /// It refused or failed with this last line of complaint.
    Said(String),
    /// It failed without a word, ending with this status.
    Ended(i32),
    /// It could not be started or looked at: the system's reason, or a signal it was ended by.
    Error(String),
    /// It did not answer within thirty seconds and was stopped.
    Stopped,
}

/// Whether `name` is the name of a picture qdesk can lay over its floor.
fn is_picture(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty() && PICTURES.iter().any(|known| known.eq_ignore_ascii_case(extension))
    })
}

/// A file's name as it is shown.
fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// The item for the file at `path` on its row's menu: "Set as wallpaper" on a picture when a
/// `qdesk` is found in `path_var`, otherwise none. Without qdesk the item would lead nowhere, so it
/// is left out rather than shown faint.
pub(super) fn menu_items(path: &Path, path_var: Option<&OsStr>) -> Vec<ContextItem<Msg>> {
    if !is_picture(&name_of(path)) {
        return Vec::new();
    }
    let Some(qdesk) = path_var.and_then(|path_var| find("qdesk", path_var)) else { return Vec::new() };
    vec![ContextItem::new(t!("explorer.wallpaper.item"), Msg::Wallpaper(qdesk, path.to_path_buf()))]
}

impl Explorer {
    /// Runs `qdesk wallpaper <picture>` with the qdesk at `qdesk`, in the background.
    pub(super) fn set_wallpaper(&mut self, qdesk: PathBuf, picture: PathBuf) -> Command<Msg> {
        let name = name_of(&picture);
        let label = t!("explorer.wallpaper.running", name = name.as_str());
        let task = Task::new(label, move |cx| {
            let outcome = run(&qdesk, &picture, &mut || cx.sleep(LOOK));
            Ok(Msg::WallpaperDone(name, outcome))
        });
        Command::task(task)
    }

    /// What `qdesk wallpaper` came to, said in the corner.
    pub(super) fn wallpaper_done(name: &str, outcome: Outcome) -> Command<Msg> {
        let reason = match outcome {
            Outcome::Set => return Command::toast(Toast::success(t!("explorer.wallpaper.done")).key("wallpaper")),
            Outcome::Said(line) | Outcome::Error(line) => line,
            Outcome::Ended(code) => t!("explorer.wallpaper.status", code = code.to_string().as_str()),
            Outcome::Stopped => t!("explorer.wallpaper.stopped", seconds = BOUND.as_secs().to_string().as_str()),
        };
        let toast = Toast::danger(t!("explorer.wallpaper.refused", name = name)).body(reason);
        Command::toast(toast.key("wallpaper"))
    }
}

/// Runs `<qdesk> wallpaper <picture>` and waits for it, calling `wait` between looks; `wait`
/// sleeps a moment itself and answers whether to go on. After [`BOUND`] of waiting, or when `wait`
/// says to stop, qdesk is stopped.
///
/// Never through a shell: the picture is one argument whatever its name holds, and it is absolute,
/// so a name beginning with `-` cannot be read as an option. qdesk keeps qexp's environment, since
/// it finds its settings through `HOME` and `XDG_CONFIG_HOME`; its input is closed, so it can
/// never wait for keys the explorer is reading.
fn run(qdesk: &Path, picture: &Path, wait: &mut dyn FnMut() -> bool) -> Outcome {
    let picture = std::path::absolute(picture).unwrap_or_else(|_| picture.to_path_buf());
    let started = spawn_patiently(|| {
        Ok(Process::new(qdesk)
            .arg("wallpaper")
            .arg(&picture)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn())
    });
    let mut child = match started {
        Ok(child) => child,
        Err(error) => return Outcome::Error(error.to_string()),
    };
    // Read while qdesk runs: one that said a great deal would otherwise fill the pipe and wait.
    let said = child.stderr.take().map(drain);
    let mut waited = Duration::ZERO;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => return Outcome::Error(error.to_string()),
        }
        if waited >= BOUND || !wait() {
            let _ = child.kill();
            let _ = child.wait();
            // The reader is left to end on its own: a program qdesk started could still hold the
            // pipe open.
            return Outcome::Stopped;
        }
        waited += LOOK;
    };
    if status.success() {
        return Outcome::Set;
    }
    let said = said.and_then(|reader| reader.join().ok()).unwrap_or_default();
    match (last_line(&String::from_utf8_lossy(&said)), status.code()) {
        (Some(line), _) => Outcome::Said(line),
        (None, Some(code)) => Outcome::Ended(code),
        (None, None) => Outcome::Error(status.to_string()),
    }
}

/// The last line of `text` that says something.
fn last_line(text: &str) -> Option<String> {
    text.lines().map(str::trim).rfind(|line| !line.is_empty()).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::is_picture;

    #[test]
    fn pictures_are_known_by_their_names_in_any_case() {
        for name in ["harbour.png", "Harbour.JPG", "a.jpeg", "k.Gif", "w.webp", "two.dots.png"] {
            assert!(is_picture(name), "{name}");
        }
        for name in ["todo.md", "png", ".png", "notes.png.txt", "picture", "archive.tar.gz"] {
            assert!(!is_picture(name), "{name}");
        }
    }
}
