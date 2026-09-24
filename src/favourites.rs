//! The person's favourites: folders they added to the sidebar themselves, kept in qexp's data
//! folder one absolute path per line.
//!
//! The list is the person's record, not a setting, so it lives in the Quvyta data folder rather
//! than next to `explorer.conf`. It is read at start and written whole after every change, through
//! a temporary file and a rename, so a crash never leaves half a list. A line that is not an
//! absolute path is skipped rather than refused: one broken line must not cost the rest.
//!
//! The first time qexp starts, before its list has ever been written, the local folders of the
//! desktop's GTK bookmarks are taken in once, so what a person added in Nautilus or Thunar is
//! there already. qexp never writes that file; after the first start the two lists live apart.

use std::ffi::OsString;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use qframe::storage::atomic_write;

/// The name of the list's file in qexp's data folder.
pub const FILE: &str = "favorites";

/// The list as it was read: `None` when the file has never been written, so the one-time import
/// knows it may run.
///
/// A file that is there but cannot be read counts as written: importing over it would replace a
/// list the person made with the desktop's.
#[must_use]
pub fn read(file: &Path) -> Option<Vec<PathBuf>> {
    match std::fs::read(file) {
        Ok(bytes) => Some(parse(&bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(_) => Some(Vec::new()),
    }
}

/// The paths of the lines of `bytes` that are absolute paths, each once, in their order.
fn parse(bytes: &[u8]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.contains(&0) {
            continue;
        }
        let path = PathBuf::from(OsString::from_vec(line.to_vec()));
        push_new(&mut paths, path);
    }
    paths
}

/// Adds `path` to `paths` when it is absolute and not there yet; true when it was added.
pub fn push_new(paths: &mut Vec<PathBuf>, path: PathBuf) -> bool {
    if !path.is_absolute() || paths.contains(&path) {
        return false;
    }
    paths.push(path);
    true
}

/// Whether `path` can be kept on a line of its own: a name with a line break in it cannot.
#[must_use]
pub fn storable(path: &Path) -> bool {
    path.is_absolute() && !path.as_os_str().as_bytes().iter().any(|byte| matches!(byte, b'\n' | b'\r' | 0))
}

/// Writes `paths` to `file`, one per line, making its folder when it is not there yet.
///
/// # Errors
///
/// Returns the error of making the folder or of writing the file.
pub fn write(file: &Path, paths: &[PathBuf]) -> io::Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(path.as_os_str().as_bytes());
        bytes.push(b'\n');
    }
    atomic_write(file, &bytes)
}

/// The local folders of the GTK bookmarks file at `file` that exist, each once, in their order;
/// `None` when there is no such file.
///
/// A line is a URI and, after a space, an optional label, which is left out: the sidebar names a
/// favourite after its folder. Only `file://` URIs of this machine are folders qexp can show; a
/// network place (`sftp://`, `smb://`) is left to the programs that can reach it.
#[must_use]
pub fn gtk_bookmarks(file: &Path) -> Option<Vec<PathBuf>> {
    let bytes = std::fs::read(file).ok()?;
    let mut paths = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let uri = line.split(|byte| *byte == b' ').next().unwrap_or_default();
        let Some(path) = local_path(uri) else { continue };
        if path.is_dir() {
            push_new(&mut paths, std::fs::canonicalize(&path).unwrap_or(path));
        }
    }
    Some(paths)
}

/// The path of a `file://` URI on this machine, percent-decoded.
fn local_path(uri: &[u8]) -> Option<PathBuf> {
    let rest = uri.strip_prefix(b"file://")?;
    // `file:///x` has an empty host; `file://localhost/x` names this machine; any other host is
    // somewhere else.
    let rest = rest.strip_prefix(b"localhost").unwrap_or(rest);
    if !rest.starts_with(b"/") {
        return None;
    }
    let decoded = percent_decode(rest);
    if decoded.contains(&0) {
        return None;
    }
    Some(PathBuf::from(OsString::from_vec(decoded)))
}

/// `bytes` with every `%` and two hexadecimal digits turned into the byte they name; a `%` without
/// them stays as it is.
fn percent_decode(bytes: &[u8]) -> Vec<u8> {
    let hex = |byte: u8| char::from(byte).to_digit(16).and_then(|digit| u8::try_from(digit).ok());
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && let (Some(high), Some(low)) =
                (bytes.get(at + 1).copied().and_then(hex), bytes.get(at + 2).copied().and_then(hex))
        {
            out.push(high * 16 + low);
            at += 3;
            continue;
        }
        out.push(bytes[at]);
        at += 1;
    }
    out
}

/// What starting with the list came to.
#[derive(Debug, Default)]
pub struct Loaded {
    /// The favourites, in the sidebar's order.
    pub paths: Vec<PathBuf>,
    /// Why the list taken in from the GTK bookmarks could not be written, when it could not.
    pub error: Option<io::Error>,
}

/// The list at start: the file at `file`, or, when it has never been written, the GTK bookmarks at
/// `gtk` taken in once and written as the first list.
///
/// Without a GTK file nothing is written, so the import can still happen on a later start once
/// the desktop has made one.
#[must_use]
pub fn load(file: &Path, gtk: Option<&Path>) -> Loaded {
    if let Some(paths) = read(file) {
        return Loaded { paths, error: None };
    }
    let Some(paths) = gtk.and_then(gtk_bookmarks) else { return Loaded::default() };
    let error = write(file, &paths).err();
    Loaded { paths, error }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_lines_and_repeats_are_skipped_and_the_rest_kept_in_order() {
        let paths = parse(b"/a/b\n\nrelative/path\n/a/c\r\n/a/b\n/with\0nul\n");
        assert_eq!(paths, [PathBuf::from("/a/b"), PathBuf::from("/a/c")]);
    }

    #[test]
    fn percent_signs_are_decoded_and_a_lone_one_is_kept() {
        assert_eq!(percent_decode(b"/My%20Files/100%/%c3%9c"), "/My Files/100%/Ü".as_bytes());
    }

    #[test]
    fn only_uris_of_this_machine_are_folders() {
        assert_eq!(local_path(b"file:///home/a%20b"), Some(PathBuf::from("/home/a b")));
        assert_eq!(local_path(b"file://localhost/srv"), Some(PathBuf::from("/srv")));
        assert_eq!(local_path(b"file://nas/share"), None);
        assert_eq!(local_path(b"sftp://host/home"), None);
    }

    #[test]
    fn a_name_with_a_line_break_cannot_be_kept_on_a_line() {
        assert!(storable(Path::new("/a/b c")));
        assert!(!storable(Path::new("/a/b\nc")));
        assert!(!storable(Path::new("relative")));
    }
}
