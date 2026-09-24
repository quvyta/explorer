//! Which program a file opens with when it is opened with Enter or a double click, and how.
//!
//! The programs, the kinds and the default come from the framework's `qframe::desktop`; what is
//! qexp's own is the order it tries them in and the terminal editor after them. The answer is
//! worked out on a background thread, since it reads the desktop's databases and the first bytes
//! of the file; it comes back as a [`Launch`]. The screen starts a desktop program with the
//! framework's `DesktopApp::launch` and the editor with a `Handoff`, both in the file's folder, so a
//! test records the start instead of running it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use qframe::desktop::{DesktopApp, Openers};

/// What opening a file comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launch {
    /// A program of the desktop, started the way its kind wants: handed the terminal, or beside
    /// qexp when it has windows of its own.
    App(DesktopApp),
    /// The terminal editor, handed the terminal until it ends. The command, program first.
    Editor(Vec<OsString>),
    /// The path is a link to a folder, which is gone into rather than opened.
    Folder(PathBuf),
    /// Nothing opens the file here; its kind, for the message.
    Nothing(String),
}

/// How the file at `path` is opened on a machine with `openers`, where `graphical` says whether a
/// graphical session is there to show windows and `editor` is the terminal editor's command
/// (`$VISUAL`, `$EDITOR` or `less`).
///
/// The kind's default program wins, then the other programs of the kind in their order. Without a
/// graphical session, graphical programs are as good as not installed: starting one would only
/// fail out of sight. With no program left, a text file opens in the terminal editor.
#[must_use]
pub fn decide(openers: &Openers, path: &Path, graphical: bool, editor: &[String]) -> Launch {
    if path.is_dir() {
        return Launch::Folder(std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()));
    }
    let choices = openers.for_file(path);
    let usable = |app: &&DesktopApp| app.can_start(graphical);
    let default = choices.default.and_then(|at| choices.apps.get(at));
    // A program whose command line gives nothing for the file could never be started.
    let chosen = default
        .filter(usable)
        .into_iter()
        .chain(choices.apps.iter().filter(usable))
        .find(|app| app.command(path).is_some());
    if let Some(app) = chosen {
        return Launch::App(app.clone());
    }
    if is_text(openers, &choices.mime) && !editor.is_empty() {
        let mut command: Vec<OsString> = editor.iter().map(OsString::from).collect();
        command.push(path.as_os_str().to_owned());
        return Launch::Editor(command);
    }
    Launch::Nothing(choices.mime)
}

/// Whether files of `mime` are text a terminal editor can open.
fn is_text(openers: &Openers, mime: &str) -> bool {
    mime.starts_with("text/") || openers.mime.ancestors(mime).iter().any(|above| above == "text/plain")
}

/// The terminal editor of an environment read by `var`: `$VISUAL`, then `$EDITOR`, then `less`,
/// each split into its words so `nvim -p` is a program and its argument.
#[must_use]
pub fn editor(var: impl Fn(&str) -> Option<String>) -> Vec<String> {
    ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(var)
        .map(|value| value.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
        .find(|words| !words.is_empty())
        .unwrap_or_else(|| vec!["less".to_owned()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_editor_is_visual_then_editor_then_less() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| pairs.iter().find(|(key, _)| *key == name).map(|(_, value)| (*value).to_owned())
        };
        assert_eq!(editor(env(&[("VISUAL", "nvim -p"), ("EDITOR", "vi")])), ["nvim", "-p"]);
        assert_eq!(editor(env(&[("VISUAL", "  "), ("EDITOR", "vi")])), ["vi"]);
        assert_eq!(editor(env(&[])), ["less"]);
    }
}
