//! What an archive is called: which endings mark one, the name it unpacks under, and the first
//! name in a folder nothing already has.

use std::fs;
use std::path::{Path, PathBuf};

use super::{Compressor, Format};

/// Every ending that marks an archive. When several fit a name the longest wins, so `.tar.gz` is
/// a tar and not a lone gzip file, and a package's `.pkg.tar.zst` is dropped whole from its stem.
const ENDINGS: &[(&str, Format)] = &[
    (".zip", Format::Zip),
    (".jar", Format::Zip),
    (".apk", Format::Zip),
    (".cbz", Format::Zip),
    (".war", Format::Zip),
    (".ear", Format::Zip),
    (".xpi", Format::Zip),
    (".tar", Format::Tar),
    (".pkg.tar.zst", Format::Tar),
    (".tar.gz", Format::Tar),
    (".tgz", Format::Tar),
    (".tar.bz2", Format::Tar),
    (".tbz", Format::Tar),
    (".tbz2", Format::Tar),
    (".tar.xz", Format::Tar),
    (".txz", Format::Tar),
    (".tar.zst", Format::Tar),
    (".tzst", Format::Tar),
    (".tar.lz", Format::Tar),
    (".tar.lzma", Format::Tar),
    (".tar.lz4", Format::Tar),
    (".tar.z", Format::Tar),
    (".7z", Format::SevenZip),
    (".rar", Format::Rar),
    (".cbr", Format::Rar),
    (".iso", Format::Disk),
    (".cpio", Format::Disk),
    (".deb", Format::Disk),
    (".rpm", Format::Disk),
    (".gz", Format::Single(Compressor::Gzip)),
    (".bz2", Format::Single(Compressor::Bzip2)),
    (".xz", Format::Single(Compressor::Xz)),
    (".zst", Format::Single(Compressor::Zstd)),
    (".lz4", Format::Single(Compressor::Lz4)),
    (".lzma", Format::Single(Compressor::Lzma)),
    (".lz", Format::Single(Compressor::Lzip)),
];

/// The longest archive ending of `name`, ignoring case, with the format it marks.
fn ending(name: &str) -> Option<(&'static str, Format)> {
    ENDINGS
        .iter()
        .filter(|(ending, _)| {
            name.len() >= ending.len()
                && name.as_bytes()[name.len() - ending.len()..].eq_ignore_ascii_case(ending.as_bytes())
        })
        .max_by_key(|(ending, _)| ending.len())
        .copied()
}

/// What kind of archive a file is, from its name alone, ignoring case.
///
/// Zip: `zip jar apk cbz war ear xpi`. Tar: `tar`, and a tar squeezed by any compressor (`tar.gz
/// tgz tar.bz2 tbz tbz2 tar.xz txz tar.zst tzst tar.lz tar.lzma tar.lz4 tar.z`), which includes a
/// package's `pkg.tar.zst`. `7z`. Rar: `rar cbr`. Disk: `iso cpio deb rpm`. A lone compressed
/// file: `gz bz2 xz zst lz4 lzma lz`. `None` for anything else.
pub fn format_of(name: &str) -> Option<Format> {
    ending(name).map(|(_, format)| format)
}

/// The name without its archive ending: `yedek.tar.gz` gives `yedek`, `notes.txt.gz` gives
/// `notes.txt` and `a.zip` gives `a`.
///
/// Never empty: a name that is nothing but an ending, such as `.zip`, is returned whole, since an
/// empty name could not be given to anything. A name with no archive ending is returned as it is.
pub fn stem(name: &str) -> &str {
    match ending(name) {
        // The ending is ASCII and matched the name's last bytes, so the cut falls between
        // characters.
        Some((ending, _)) if name.len() > ending.len() => &name[..name.len() - ending.len()],
        _ => name,
    }
}

/// The `n`th choice of name for `wanted`: the name itself first, then `stem 2.ext`, `stem 3.ext`
/// and on. A folder's name is never split, since a dot in a folder's name is not an extension; nor
/// is a file whose only dot is its first, such as `.bashrc`, which is hidden rather than extended.
pub(super) fn numbered(wanted: &str, n: usize, is_folder: bool) -> String {
    if n <= 1 {
        return wanted.to_owned();
    }
    match wanted.rfind('.').filter(|&dot| !is_folder && dot > 0) {
        Some(dot) => format!("{} {n}{}", &wanted[..dot], &wanted[dot..]),
        None => format!("{wanted} {n}"),
    }
}

/// The first name for `wanted` that nothing in `folder` has: `wanted`, then `stem 2.ext`, `stem
/// 3.ext` and on. A folder's name is numbered at its end (`yedek 2`), since a dot in a folder's
/// name is not an extension.
///
/// A name counts as taken by anything at all, a broken link included. This only looks; a name
/// found free may be taken before it is used, which is why extracting claims its name by making it.
pub fn fresh(folder: &Path, wanted: &str, is_folder: bool) -> PathBuf {
    let mut n = 1;
    loop {
        let path = folder.join(numbered(wanted, n, is_folder));
        if fs::symlink_metadata(&path).is_err() {
            return path;
        }
        n += 1;
    }
}
