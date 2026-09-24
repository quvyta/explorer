//! Temporary folders for the tests, and the machine's real archive tools found by name. Every test
//! works in a folder of its own that is removed when the test ends, and the tools are looked for
//! in the standard folders rather than in the environment the tests happen to run in.

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// The folders the tools are looked for in.
const SYSTEM_BIN: &[&str] = &["/usr/local/bin", "/usr/bin", "/bin"];

/// A temporary folder of a test's own.
pub(super) struct Scratch {
    root: PathBuf,
}

impl Scratch {
    pub(super) fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("qexp-archive-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("the temporary folder can be made");
        Self { root }
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Writes a file below the folder, making its folders.
    pub(super) fn write(&self, rel: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.path(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("the folder can be made");
        }
        fs::write(&path, contents).expect("the file can be written");
        path
    }

    /// Writes a shell script that may be run.
    pub(super) fn script(&self, rel: &str, body: &str) -> PathBuf {
        let path = self.write(rel, format!("#!/bin/sh\n{body}\n"));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("mode can be set");
        path
    }

    /// The names in a folder below this one, sorted.
    pub(super) fn listing(&self, rel: &str) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.path(rel))
            .expect("the folder can be read")
            .map(|entry| entry.expect("the entry can be read").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// A folder `rel` holding links to just the real tools named, as a search path; `None`, with
    /// the reason printed, when one is not installed.
    pub(super) fn only(&self, rel: &str, tools: &[&str]) -> Option<OsString> {
        let folder = self.path(rel);
        fs::create_dir_all(&folder).expect("the folder can be made");
        for name in tools {
            symlink(tool(name)?, folder.join(name)).expect("the link can be made");
        }
        Some(folder.into_os_string())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The real tool `name`; `None`, with the reason printed, when it is not installed.
pub(super) fn tool(name: &str) -> Option<PathBuf> {
    let found = SYSTEM_BIN.iter().map(|folder| Path::new(folder).join(name)).find(|path| path.is_file());
    if found.is_none() {
        eprintln!("skipped: {name} is not installed");
    }
    found
}

/// The folders holding the real tools named, as a search path; `None`, with the reason printed,
/// when one is not installed.
pub(super) fn path_of(tools: &[&str]) -> Option<OsString> {
    let mut folders: Vec<PathBuf> = Vec::new();
    for name in tools {
        let folder = tool(name)?.parent().expect("a tool is in a folder").to_path_buf();
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    Some(std::env::join_paths(folders).expect("the folders can be joined"))
}

/// Runs the real tool `name` in `cwd` and panics unless it succeeds: it builds a test's archive.
pub(super) fn make(name: &str, args: &[&str], cwd: &Path) {
    let program = tool(name).expect("the tool that builds the archive is installed");
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .status()
        .expect("the tool can be started");
    assert!(status.success(), "{name} {args:?} failed");
}

/// A tar archive written by hand, holding `members` (name and contents) as plain files. Written
/// here rather than by a tool so that it can hold names no tool would put in an archive.
pub(super) fn tar(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (name, contents) in members {
        let mut header = [0u8; 512];
        header[..name.len()].copy_from_slice(name.as_bytes());
        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0001750\0");
        header[116..124].copy_from_slice(b"0001750\0");
        header[124..136].copy_from_slice(format!("{:011o}\0", contents.len()).as_bytes());
        header[136..148].copy_from_slice(b"15000000000\0");
        header[156] = b'0';
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        // The checksum is taken with its own field counted as spaces.
        header[148..156].copy_from_slice(b"        ");
        let sum: u32 = header.iter().map(|&byte| u32::from(byte)).sum();
        header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(contents);
        bytes.resize(bytes.len().div_ceil(512) * 512, 0);
    }
    bytes.resize(bytes.len() + 1024, 0);
    bytes
}

/// The table of the CRC-32 that zip uses, for [`unlucky_zip`].
fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (at, slot) in (0u32..).zip(table.iter_mut()) {
        *slot = (0..8).fold(at, |crc, _| if crc & 1 == 1 { 0xEDB8_8320 ^ (crc >> 1) } else { crc >> 1 });
    }
    table
}

/// The three keys of zip's old password encryption, as the format describes them.
struct ZipKeys([u32; 3], [u32; 256]);

impl ZipKeys {
    fn new(password: &str) -> Self {
        let mut keys = Self([0x1234_5678, 0x2345_6789, 0x3456_7890], crc_table());
        password.bytes().for_each(|byte| keys.update(byte));
        keys
    }

    fn crc(&self, crc: u32, byte: u8) -> u32 {
        self.1[((crc ^ u32::from(byte)) & 0xFF) as usize] ^ (crc >> 8)
    }

    fn update(&mut self, plain: u8) {
        self.0[0] = self.crc(self.0[0], plain);
        self.0[1] = (self.0[1].wrapping_add(self.0[0] & 0xFF)).wrapping_mul(134_775_813).wrapping_add(1);
        self.0[2] = self.crc(self.0[2], (self.0[1] >> 24) as u8);
    }

    fn stream(&self) -> u8 {
        let temp = (self.0[2] | 2) & 0xFFFF;
        ((temp * (temp ^ 1)) >> 8) as u8
    }

    fn encrypt(&mut self, plain: u8) -> u8 {
        let cipher = plain ^ self.stream();
        self.update(plain);
        cipher
    }

    fn decrypt(&mut self, cipher: u8) -> u8 {
        let plain = cipher ^ self.stream();
        self.update(plain);
        plain
    }
}

/// A zip holding one stored file, `name` with `contents`, encrypted with `password` the old way,
/// whose check byte a wrong password `unlucky` passes.
///
/// The old encryption checks a password against one byte of the entry's header, so one wrong
/// password in 256 gets through and fails only later, in decompression, with no word about a
/// password. A tool's own archive is that unlucky by chance; this one is every time, because its
/// header bytes are picked until it is.
pub(super) fn unlucky_zip(name: &str, contents: &[u8], password: &str, unlucky: &str) -> Vec<u8> {
    let table = crc_table();
    let crc = !contents.iter().fold(!0u32, |crc, &byte| table[((crc ^ u32::from(byte)) & 0xFF) as usize] ^ (crc >> 8));
    let check = (crc >> 24) as u8;
    let (header, mut keys) = (0u32..)
        .find_map(|seed| {
            let mut plain: Vec<u8> = seed.to_le_bytes().iter().copied().cycle().take(11).collect();
            plain.push(check);
            let mut keys = ZipKeys::new(password);
            let header: Vec<u8> = plain.iter().map(|&byte| keys.encrypt(byte)).collect();
            let mut wrong = ZipKeys::new(unlucky);
            let passes = header.iter().map(|&byte| wrong.decrypt(byte)).last() == Some(check);
            passes.then_some((header, keys))
        })
        .expect("one header in about 256 lets the wrong password through");
    let data: Vec<u8> = header.into_iter().chain(contents.iter().map(|&byte| keys.encrypt(byte))).collect();

    let size = |n: usize| u32::try_from(n).expect("a small file");
    let name_len = u16::try_from(name.len()).expect("a short name");
    // Version 2.0, encrypted, stored, 1980-01-01 00:00.
    let common = |bytes: &mut Vec<u8>| {
        for part in [&20u16.to_le_bytes()[..], &1u16.to_le_bytes(), &0u16.to_le_bytes(), &0u16.to_le_bytes()] {
            bytes.extend_from_slice(part);
        }
        bytes.extend_from_slice(&0x21u16.to_le_bytes());
        bytes.extend_from_slice(&crc.to_le_bytes());
        bytes.extend_from_slice(&size(data.len()).to_le_bytes());
        bytes.extend_from_slice(&size(contents.len()).to_le_bytes());
        bytes.extend_from_slice(&name_len.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
    };
    let mut zip = b"PK\x03\x04".to_vec();
    common(&mut zip);
    zip.extend_from_slice(name.as_bytes());
    zip.extend_from_slice(&data);
    let directory = zip.len();
    zip.extend_from_slice(b"PK\x01\x02");
    zip.extend_from_slice(&20u16.to_le_bytes());
    common(&mut zip);
    // No comment, disk 0, no attributes, the entry at the very start.
    zip.extend_from_slice(&[0u8; 2 + 2 + 2 + 4 + 4]);
    zip.extend_from_slice(name.as_bytes());
    let directory_size = zip.len() - directory;
    zip.extend_from_slice(b"PK\x05\x06");
    zip.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
    zip.extend_from_slice(&size(directory_size).to_le_bytes());
    zip.extend_from_slice(&size(directory).to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());
    zip
}
