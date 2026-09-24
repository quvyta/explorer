//! Archives: which files are archives, which installed tool unpacks them, and unpacking without
//! ever touching anything already there.
//!
//! No format is decoded here. The programs a system already trusts for the job (`bsdtar`, `tar`,
//! `unzip`, 7-Zip, `unrar` and the compressors) are run, so an archive opens here exactly when it
//! opens anywhere else on the machine, and the care those programs take over hostile archives
//! comes along with them.
//!
//! Unpacking never overwrites: the tool writes into a new hidden folder beside the target, on the
//! same file system, and only a finished result is moved to a name nothing had. A tool that fails
//! leaves nothing behind, not even half an archive.
//!
//! This module draws nothing and blocks while a tool runs; the caller runs it away from drawing,
//! and can stop it there with [`extract_watched`].

mod names;
mod tools;

#[cfg(test)]
mod fixture;
#[cfg(test)]
mod tests;

use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

pub use names::{format_of, fresh, stem};
pub use tools::plan;

use crate::programs::{drain, spawn_patiently};

/// The kinds of archive, by what reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// A zip file, and the formats built on it: `jar`, `apk`, `cbz`, `war`, `ear`, `xpi`.
    Zip,
    /// A tar file, bare or compressed.
    Tar,
    /// A 7-Zip archive.
    SevenZip,
    /// A RAR archive, and a comic book in one (`cbr`).
    Rar,
    /// A disk image or a package (`iso`, `cpio`, `deb`, `rpm`), which only `bsdtar` reads.
    Disk,
    /// A single compressed file, which unpacks to one file rather than a folder.
    Single(Compressor),
}

/// The compressors a lone compressed file may have gone through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compressor {
    /// `.gz`
    Gzip,
    /// `.bz2`
    Bzip2,
    /// `.xz`
    Xz,
    /// `.zst`
    Zstd,
    /// `.lz4`
    Lz4,
    /// `.lzma`, the format xz grew out of.
    Lzma,
    /// `.lz`
    Lzip,
}

/// Where a tool puts what it unpacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Into the folder it was given, as the archive's files.
    Folder,
    /// To its standard output, as the one file a lone compressed file holds.
    Stdout,
}

/// How to unpack one archive: the program, its arguments and where the result appears. Never run
/// through a shell, so no file name can change what is run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The program, with the folder it was found in.
    pub program: PathBuf,
    /// Its arguments, each one whole.
    pub args: Vec<OsString>,
    /// Where it puts what it unpacks.
    pub output: Output,
}

/// No installed tool can unpack an archive: what to install to have one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// The first tool that would do, such as `bsdtar`.
    pub wanted: &'static str,
    /// The Arch package it comes in, such as `libarchive` for `bsdtar`.
    pub package: &'static str,
}

/// Why an archive was not unpacked. Whatever the reason, nothing was left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractError {
    /// The file's name is not one an archive has.
    NotAnArchive,
    /// No installed tool unpacks it.
    Missing(Missing),
    /// It is protected with a password, which is not asked for.
    Encrypted,
    /// The tool failed, or the file system did: the tool's last line of complaint, or the error.
    Failed(String),
    /// The caller asked to stop, and the tool was stopped.
    Cancelled,
}

impl From<io::Error> for ExtractError {
    fn from(error: io::Error) -> Self {
        Self::Failed(error.to_string())
    }
}

/// Unpacks `archive` beside itself, in its own folder, and returns what was made.
///
/// An archive becomes a new folder named after its [`stem`] (`yedek.tar.gz` makes `yedek`, or
/// `yedek 2` when that is taken, then `yedek 3`); a lone compressed file becomes a new file
/// (`notes.txt.gz` makes `notes.txt`, or `notes 2.txt`). Nothing already there is ever replaced.
/// Blocks until the tool is done.
///
/// `path_var` is where the tools are looked for, like `PATH`; see [`plan`].
///
/// # Errors
///
/// See [`ExtractError`]; on any error nothing new is left beside the archive.
pub fn extract_here(archive: &Path, path_var: Option<&OsStr>) -> Result<PathBuf, ExtractError> {
    extract_into(archive, archive.parent().unwrap_or(Path::new("")), path_var)
}

/// Unpacks `archive` into `folder` instead of the archive's own folder, by the same rules as
/// [`extract_here`].
///
/// # Errors
///
/// See [`ExtractError`]; on any error nothing new is left in `folder`.
pub fn extract_into(archive: &Path, folder: &Path, path_var: Option<&OsStr>) -> Result<PathBuf, ExtractError> {
    extract_watched(archive, folder, path_var, &mut || {
        std::thread::sleep(LOOK);
        true
    })
}

/// How often [`extract_into`] looks whether the tool is done: often enough that a small archive
/// is not kept waiting, seldom enough to cost nothing.
const LOOK: Duration = Duration::from_millis(10);

/// Unpacks `archive` into `folder` as [`extract_into`] does, asking `carry_on` again and again
/// while the tool runs. `carry_on` waits a moment itself and answers whether to go on; on `false`
/// the tool is stopped and everything it wrote is removed.
///
/// This is how the explorer runs an extraction in the background and lets the person cancel it:
/// the wait is the background task's own sleep, which wakes when the task is cancelled.
///
/// # Errors
///
/// See [`ExtractError`]; [`ExtractError::Cancelled`] when `carry_on` said to stop. On any error
/// nothing new is left in `folder`.
pub fn extract_watched(
    archive: &Path,
    folder: &Path,
    path_var: Option<&OsStr>,
    carry_on: &mut dyn FnMut() -> bool,
) -> Result<PathBuf, ExtractError> {
    // A name that is not valid UTF-8 is read with its broken bytes replaced: the result is named a
    // little differently from the archive, but it is still a name nothing had.
    let name = archive.file_name().map(OsStr::to_string_lossy).ok_or(ExtractError::NotAnArchive)?;
    let format = format_of(&name).ok_or(ExtractError::NotAnArchive)?;
    let stem = stem(&name);
    let temp = Temp::new(folder, stem)?;
    let plan = plan(format, archive, temp.path(), path_var).map_err(ExtractError::Missing)?;

    // A lone compressed file comes out on the tool's output, straight into a file in the
    // temporary folder; its name there is fixed, since the stem could be `..`.
    let single = temp.path().join("content");
    let single_file = match plan.output {
        Output::Folder => None,
        Output::Stdout => Some(File::create_new(&single)?),
    };
    let mut child = spawn_patiently(|| {
        let stdout = match &single_file {
            None => Stdio::piped(),
            Some(file) => file.try_clone()?.into(),
        };
        Ok(Command::new(&plan.program)
            .args(&plan.args)
            // Only what the tool needs: options a person set for their own shell (`TAR_OPTIONS`,
            // `UNZIP`) could otherwise lift a tool's protections, and the C locale keeps its
            // complaints in words that can be recognised.
            .env_clear()
            .env("PATH", path_var.unwrap_or_default())
            .env("LC_ALL", "C")
            // A tool that wants to ask something finds nothing to read and fails rather than
            // waiting.
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(Stdio::piped())
            .spawn())
    })?;
    drop(single_file);
    // Both pipes are read while the tool runs: a tool that says a lot would otherwise fill one and
    // wait for it to be read, forever.
    let said_out = child.stdout.take().map(drain);
    let said_err = child.stderr.take().map(drain);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if !carry_on() {
            let _ = child.kill();
            let _ = child.wait();
            // The readers are left to end on their own: a program the tool started could still
            // hold the pipes open, and nothing it says matters any more. `temp` goes on return.
            return Err(ExtractError::Cancelled);
        }
    };
    if !status.success() {
        let said = [said_err, said_out].map(|reader| reader.and_then(|reader| reader.join().ok()).unwrap_or_default());
        let said = said.iter().map(|bytes| String::from_utf8_lossy(bytes)).collect::<Vec<_>>();
        let lower = said.iter().map(|text| text.to_lowercase()).collect::<String>();
        // What the archive says comes first: a tool can fail on an encrypted zip without a word
        // about a password. See `zip_is_encrypted`.
        let said_so = ["password", "passphrase", "encrypted"].iter().any(|word| lower.contains(word));
        if said_so || (format == Format::Zip && zip_is_encrypted(archive)) {
            return Err(ExtractError::Encrypted);
        }
        let complaint = said.iter().find_map(|text| last_line(text)).unwrap_or_else(|| status.to_string());
        return Err(ExtractError::Failed(complaint));
    }

    match plan.output {
        Output::Folder => settle(temp.path(), folder, stem, true),
        Output::Stdout => settle(&single, folder, stem, false),
    }
}

/// The largest zip central directory read to look for a password: a few hundred bytes an entry,
/// so tens of thousands of entries fit.
const LARGEST_DIRECTORY: u64 = 16 << 20;

/// Whether the zip at `archive` says in its central directory that an entry is protected with a
/// password (bit 0 of an entry's flags).
///
/// A tool's own words are not enough to tell. Zip's old encryption checks a password against a
/// single byte, so one wrong password in 256 passes the check, and the tool then fails to
/// decompress and says nothing of a password: `bsdtar` says "ZIP decompression failed". The flag is
/// what the archive itself says, the same on every run.
///
/// Only the plain end record is read; a zip64 archive, whose record points elsewhere, and anything
/// that does not read as a zip count as not encrypted, and the tool's words decide.
fn zip_is_encrypted(archive: &Path) -> bool {
    let read = || -> io::Result<bool> {
        // A named pipe that carries a zip's name would keep the reading waiting forever.
        if !fs::metadata(archive)?.is_file() {
            return Ok(false);
        }
        let mut file = File::open(archive)?;
        let len = file.metadata()?.len();
        // The end record is 22 bytes, followed by a comment of at most 64 KiB.
        let tail_len = len.min(22 + 0xFFFF);
        file.seek(SeekFrom::Start(len - tail_len))?;
        let mut tail = Vec::new();
        file.by_ref().take(tail_len).read_to_end(&mut tail)?;
        let Some(end) = tail.windows(4).rposition(|word| word == b"PK\x05\x06") else { return Ok(false) };
        let Some(record) = tail.get(end..end + 22) else { return Ok(false) };
        let word =
            |bytes: &[u8], at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        let (size, offset) = (u64::from(word(record, 12)), u64::from(word(record, 16)));
        if offset == u64::from(u32::MAX) || size > LARGEST_DIRECTORY || offset + size > len {
            return Ok(false);
        }
        file.seek(SeekFrom::Start(offset))?;
        let mut directory = Vec::new();
        file.take(size).read_to_end(&mut directory)?;
        let half = |at: usize| usize::from(u16::from_le_bytes([directory[at], directory[at + 1]]));
        let mut at = 0;
        while directory.get(at..at + 46).is_some_and(|entry| entry.starts_with(b"PK\x01\x02")) {
            if half(at + 8) & 1 == 1 {
                return Ok(true);
            }
            at += 46 + half(at + 28) + half(at + 30) + half(at + 32);
        }
        Ok(false)
    };
    read().unwrap_or(false)
}

/// The last line of a tool's complaint worth showing, skipping the closing line `bsdtar` adds after
/// every error, which says only that there was one.
fn last_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty() && !line.contains("Error exit delayed from previous errors"))
        .map(str::to_owned)
}

/// Moves `from` to the first free name for `wanted` in `folder` and returns it.
///
/// The name is claimed by making an empty folder or file under it, which fails if anything has it,
/// and only then is the result moved over that empty claim. Looking and then moving would let
/// something arriving in between be replaced.
fn settle(from: &Path, folder: &Path, wanted: &str, is_folder: bool) -> Result<PathBuf, ExtractError> {
    let mut n = 1;
    let target = loop {
        let target = folder.join(names::numbered(wanted, n, is_folder));
        let claimed = if is_folder { fs::create_dir(&target) } else { File::create_new(&target).map(drop) };
        match claimed {
            Ok(()) => break target,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => n += 1,
            Err(error) => return Err(error.into()),
        }
    };
    if let Err(error) = fs::rename(from, &target) {
        let _ = if is_folder { fs::remove_dir(&target) } else { fs::remove_file(&target) };
        return Err(error.into());
    }
    Ok(target)
}

/// Tells apart the temporary folders of several extractions running at once in one process.
static NEXT: AtomicUsize = AtomicUsize::new(0);

/// The longest part of the stem a temporary folder's name carries: enough to tell which archive it
/// belongs to, short enough that the whole name stays within a file system's limit.
const TEMP_STEM: usize = 64;

/// A new hidden folder a tool unpacks into, removed with everything in it when dropped. Once it
/// has been moved to its final name there is nothing left to remove.
struct Temp {
    path: PathBuf,
}

impl Temp {
    /// Makes `.<stem>.qexp-<process>-<n>` in `folder`: in the same folder as the result, so moving
    /// it there is a rename on one file system rather than a copy.
    fn new(folder: &Path, stem: &str) -> io::Result<Self> {
        let mut cut = stem.len().min(TEMP_STEM);
        while !stem.is_char_boundary(cut) {
            cut -= 1;
        }
        loop {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = folder.join(format!(".{}.qexp-{}-{n}", &stem[..cut], std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
