//! Finding and starting the programs qexp hands work to: the archive tools, and qdesk for a
//! wallpaper. Both are looked for and started the same way, so both keep the same care.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Child;
use std::thread::JoinHandle;
use std::time::Duration;

/// The first `program` in the folders of `path_var` that may be run: a regular file with a mode
/// that lets it run. A relative folder is skipped: it would mean whatever folder the explorer
/// happens to be in, and an archive's own folder could then supply a program named `tar`.
pub(crate) fn find(program: &str, path_var: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path_var).filter(|folder| folder.is_absolute()).map(|folder| folder.join(program)).find(
        |candidate| fs::metadata(candidate).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0),
    )
}

/// Reads all of `pipe` on a thread of its own.
pub(crate) fn drain(mut pipe: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    })
}

/// How many times a program is tried while its file is still open for writing somewhere, and how
/// long to wait between tries: a package upgrade replacing it, or another process that has just
/// written it and whose descendants still hold it, lets go within moments.
const BUSY_TRIES: u32 = 20;
const BUSY_WAIT: Duration = Duration::from_millis(25);

/// The Linux error for starting a program whose file is open for writing ("text file busy").
const TEXT_FILE_BUSY: i32 = 26;

/// Starts a program, trying again for a moment while its file is busy being written. `start`
/// builds and starts the command afresh each time, since a started command gives up its pipes.
pub(crate) fn spawn_patiently(mut start: impl FnMut() -> io::Result<io::Result<Child>>) -> io::Result<Child> {
    let mut tries = 1;
    loop {
        match start()? {
            Err(error) if error.raw_os_error() == Some(TEXT_FILE_BUSY) && tries < BUSY_TRIES => {
                tries += 1;
                std::thread::sleep(BUSY_WAIT);
            }
            started => return started,
        }
    }
}
