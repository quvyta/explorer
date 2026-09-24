//! The command line: `qexp [PATH]`, `--version` and `--help`.
//!
//! This is qexp's contract with qdesk and with anyone who types it: a folder opens there, a file
//! opens its folder with the file selected, nothing opens the home folder, and a path that does
//! not exist is a one-line message and exit code 2 before any screen is drawn.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

/// Where the screen opens: a folder, and a name in it to select.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    /// The folder shown, with every link resolved.
    pub folder: PathBuf,
    /// The name of the entry the cursor starts on, for a file given on the command line.
    pub select: Option<String>,
}

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// Open the screen here.
    Screen(Start),
    /// Print the version and leave.
    Version,
    /// Print how to use qexp and leave.
    Help,
    /// A path that does not exist.
    Missing(PathBuf),
    /// An option qexp does not know, or more than one path.
    Unknown(String),
}

/// Reads the arguments after the program name. `home` is where nothing opens, and relative paths
/// are read from `cwd`.
#[must_use]
pub fn parse(args: impl IntoIterator<Item = OsString>, home: &Path, cwd: &Path) -> Invocation {
    let mut path = None;
    let mut only_paths = false;
    for arg in args {
        let text = arg.to_string_lossy();
        if !only_paths && text.starts_with('-') && text.len() > 1 {
            match text.as_ref() {
                "--version" | "-V" => return Invocation::Version,
                "--help" | "-h" => return Invocation::Help,
                "--" => only_paths = true,
                other => return Invocation::Unknown(other.to_owned()),
            }
            continue;
        }
        if path.is_some() {
            return Invocation::Unknown(text.into_owned());
        }
        path = Some(PathBuf::from(arg));
    }
    let Some(path) = path else { return start_at(home).map_or_else(Invocation::Missing, Invocation::Screen) };
    let path = if path.is_absolute() { path } else { cwd.join(path) };
    start_at(&path).map_or_else(Invocation::Missing, Invocation::Screen)
}

/// Where the screen opens for `path`, or the path back when nothing is there.
fn start_at(path: &Path) -> Result<Start, PathBuf> {
    let resolved = fs::canonicalize(path).map_err(|_| path.to_path_buf())?;
    if resolved.is_dir() {
        return Ok(Start { folder: resolved, select: None });
    }
    // A file's own name is kept, not its link's target: the person asked for the name they gave,
    // in the folder they gave it in.
    let given = path.file_name().map(|name| name.to_string_lossy().into_owned());
    let folder = path.parent().and_then(|parent| fs::canonicalize(parent).ok());
    match folder {
        Some(folder) => Ok(Start { folder, select: given }),
        None => Err(path.to_path_buf()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("qexp-cli-{name}-{}", std::process::id()));
        fs::create_dir_all(root.join("home/notes")).expect("folders");
        fs::write(root.join("home/notes/todo.md"), "milk").expect("file");
        fs::canonicalize(root).expect("root")
    }

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_folder_opens_there_a_file_opens_its_folder_selected_and_nothing_opens_home() {
        let root = scratch("paths");
        let home = root.join("home");
        let notes = home.join("notes");
        assert_eq!(
            parse(args(&[notes.to_str().unwrap()]), &home, &root),
            Invocation::Screen(Start { folder: notes.clone(), select: None })
        );
        assert_eq!(
            parse(args(&["notes/todo.md"]), &home, &home),
            Invocation::Screen(Start { folder: notes, select: Some("todo.md".into()) }),
            "a relative path is read from where qexp was started"
        );
        assert_eq!(parse(args(&[]), &home, &root), Invocation::Screen(Start { folder: home.clone(), select: None }));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_path_that_is_not_there_is_missing_and_options_are_read() {
        let root = scratch("missing");
        let home = root.join("home");
        assert_eq!(parse(args(&["gone"]), &home, &root), Invocation::Missing(root.join("gone")));
        assert_eq!(parse(args(&["--version"]), &home, &root), Invocation::Version);
        assert_eq!(parse(args(&["-h"]), &home, &root), Invocation::Help);
        assert_eq!(parse(args(&["--frobnicate"]), &home, &root), Invocation::Unknown("--frobnicate".into()));
        assert_eq!(parse(args(&["home", "home"]), &home, &root), Invocation::Unknown("home".into()));
        assert!(matches!(parse(args(&["--", "home"]), &home, &root), Invocation::Screen(_)));
        let _ = fs::remove_dir_all(&root);
    }
}
