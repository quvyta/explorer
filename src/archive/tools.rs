//! Which installed program unpacks an archive, and the arguments it is given.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::programs::find;

use super::{Compressor, Format, Missing, Output, Plan};

/// Handed to every tool that would otherwise stop to ask for a password. The tools read a password
/// from the terminal itself, not from their input, so closing their input is not enough: without
/// one they would draw a prompt over the explorer and wait for keys that never come. Given this
/// one they fail on an encrypted archive at once, and say why.
pub(super) const NO_PASSWORD: &str = "qexp-no-password";

/// A tool that can unpack an archive: its program name and the package it comes in on Arch.
#[derive(Clone, Copy)]
struct Tool {
    program: &'static str,
    package: &'static str,
}

const BSDTAR: Tool = Tool { program: "bsdtar", package: "libarchive" };
const TAR: Tool = Tool { program: "tar", package: "tar" };
const UNZIP: Tool = Tool { program: "unzip", package: "unzip" };
const SEVEN_Z: Tool = Tool { program: "7z", package: "7zip" };
const SEVEN_ZZ: Tool = Tool { program: "7zz", package: "7zip" };
const SEVEN_ZA: Tool = Tool { program: "7za", package: "7zip" };
const UNRAR: Tool = Tool { program: "unrar", package: "unrar" };

/// The tools for a format, the most capable first. `bsdtar` leads wherever it can, since it reads
/// nearly every format alone and comes with the system's own package manager on Arch.
fn tools(format: Format) -> &'static [Tool] {
    match format {
        Format::Zip => &[BSDTAR, UNZIP],
        Format::Tar => &[BSDTAR, TAR],
        Format::SevenZip => &[BSDTAR, SEVEN_Z, SEVEN_ZZ, SEVEN_ZA],
        Format::Rar => &[BSDTAR, UNRAR, SEVEN_Z],
        Format::Disk => &[BSDTAR],
        Format::Single(Compressor::Gzip) => &[Tool { program: "gzip", package: "gzip" }],
        Format::Single(Compressor::Bzip2) => &[Tool { program: "bzip2", package: "bzip2" }],
        // xz reads the older lzma format as well as its own.
        Format::Single(Compressor::Xz | Compressor::Lzma) => &[Tool { program: "xz", package: "xz" }],
        Format::Single(Compressor::Zstd) => &[Tool { program: "zstd", package: "zstd" }],
        Format::Single(Compressor::Lz4) => &[Tool { program: "lz4", package: "lz4" }],
        Format::Single(Compressor::Lzip) => &[Tool { program: "lzip", package: "lzip" }],
    }
}

/// A path as an argument no tool can take for an option: one beginning with `-` would otherwise be
/// read as a switch.
fn arg(path: &Path) -> OsString {
    if path.is_absolute() { path.into() } else { Path::new(".").join(path).into() }
}

/// How to unpack the archive at `archive` into the folder `into` with the first installed tool,
/// searched in `path_var` (folders separated by colons, like `PATH`; a candidate must be a regular
/// file that may be run).
///
/// The tools, in order: zip `bsdtar`, `unzip`; tar `bsdtar`, `tar`; 7z `bsdtar`, `7z`, `7zz`, `7za`;
/// rar `bsdtar`, `unrar`, `7z`; a disk image or package `bsdtar`; a lone compressed file its own
/// compressor, `gzip`, `bzip2`, `xz` (also for lzma), `zstd`, `lz4` or `lzip`, writing the one file
/// to its output.
///
/// Nothing is ever given that would let a tool write outside `into`: `bsdtar` refuses `..` and
/// absolute names unless told otherwise with `-P`, and it never is.
///
/// # Errors
///
/// [`Missing`] names the first tool that would do and its package when none is installed, or when
/// there is no `path_var` to search.
pub fn plan(format: Format, archive: &Path, into: &Path, path_var: Option<&OsStr>) -> Result<Plan, Missing> {
    let candidates = tools(format);
    let found = path_var.and_then(|path_var| {
        candidates.iter().find_map(|tool| find(tool.program, path_var).map(|program| (tool.program, program)))
    });
    let Some((name, program)) = found else {
        let first = candidates[0];
        return Err(Missing { wanted: first.program, package: first.package });
    };
    let (archive, into) = (arg(archive), arg(into));
    let (args, output): (Vec<OsString>, _) = match (format, name) {
        (Format::Single(_), _) => (vec!["-dc".into(), archive], Output::Stdout),
        (_, "bsdtar") => (
            vec!["--passphrase".into(), NO_PASSWORD.into(), "-x".into(), "-f".into(), archive, "-C".into(), into],
            Output::Folder,
        ),
        (_, "tar") => (vec!["-x".into(), "-f".into(), archive, "-C".into(), into], Output::Folder),
        // The target folder is new and empty, so unzip is never told to overwrite; `-q` is left off
        // because it also hides the line saying a password was wrong.
        (_, "unzip") => (vec!["-P".into(), NO_PASSWORD.into(), archive, "-d".into(), into], Output::Folder),
        (_, "unrar") => {
            // unrar reads a destination without a trailing slash as a name inside the archive.
            let mut into = into;
            into.push("/");
            (vec!["x".into(), "-idq".into(), "-p-".into(), archive, into], Output::Folder)
        }
        _ => {
            let mut to = OsString::from("-o");
            to.push(into);
            (vec!["x".into(), "-y".into(), format!("-p{NO_PASSWORD}").into(), to, archive], Output::Folder)
        }
    };
    Ok(Plan { program, args, output })
}
