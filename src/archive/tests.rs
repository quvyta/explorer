use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use super::fixture::{Scratch, make, path_of, tar, tool, unlucky_zip};
use super::*;

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

// --- Names ---

#[test]
fn every_archive_ending_is_known_whatever_its_case() {
    let cases: &[(&str, Format)] = &[
        ("a.zip", Format::Zip),
        ("a.jar", Format::Zip),
        ("a.apk", Format::Zip),
        ("a.cbz", Format::Zip),
        ("a.war", Format::Zip),
        ("a.ear", Format::Zip),
        ("a.xpi", Format::Zip),
        ("a.tar", Format::Tar),
        ("a.tar.gz", Format::Tar),
        ("a.tgz", Format::Tar),
        ("a.tar.bz2", Format::Tar),
        ("a.tbz", Format::Tar),
        ("a.tbz2", Format::Tar),
        ("a.tar.xz", Format::Tar),
        ("a.txz", Format::Tar),
        ("a.tar.zst", Format::Tar),
        ("a.tzst", Format::Tar),
        ("a.tar.lz", Format::Tar),
        ("a.tar.lzma", Format::Tar),
        ("a.tar.lz4", Format::Tar),
        ("a.tar.z", Format::Tar),
        ("a.tar.Z", Format::Tar),
        ("yay-12.0-1-x86_64.pkg.tar.zst", Format::Tar),
        ("a.7z", Format::SevenZip),
        ("a.rar", Format::Rar),
        ("a.cbr", Format::Rar),
        ("a.iso", Format::Disk),
        ("a.cpio", Format::Disk),
        ("a.deb", Format::Disk),
        ("a.rpm", Format::Disk),
        ("a.gz", Format::Single(Compressor::Gzip)),
        ("a.bz2", Format::Single(Compressor::Bzip2)),
        ("a.xz", Format::Single(Compressor::Xz)),
        ("a.zst", Format::Single(Compressor::Zstd)),
        ("a.lz4", Format::Single(Compressor::Lz4)),
        ("a.lzma", Format::Single(Compressor::Lzma)),
        ("a.lz", Format::Single(Compressor::Lzip)),
        ("YEDEK.TAR.GZ", Format::Tar),
        ("Foto.Zip", Format::Zip),
        ("notes.txt.GZ", Format::Single(Compressor::Gzip)),
        ("şarkılar ğü.7Z", Format::SevenZip),
    ];
    for (name, format) in cases {
        assert_eq!(format_of(name), Some(*format), "{name}");
    }
}

#[test]
fn other_names_are_not_archives() {
    for name in ["", "a.txt", "zip", "tar", "a.gzip", "a.tar.gz.bak", "a.zipx", "a.z", "a.tar.", "archive"] {
        assert_eq!(format_of(name), None, "{name}");
    }
}

#[test]
fn the_stem_drops_the_whole_archive_ending() {
    let cases = [
        ("yedek.tar.gz", "yedek"),
        ("yedek.TGZ", "yedek"),
        ("notes.txt.gz", "notes.txt"),
        ("a.zip", "a"),
        ("site.v2.tar.zst", "site.v2"),
        ("yay-12.0-1-x86_64.pkg.tar.zst", "yay-12.0-1-x86_64"),
        ("film.rar", "film"),
        ("debian.iso", "debian"),
        ("Yedek dosyası ğüş.tar.xz", "Yedek dosyası ğüş"),
        ("plain.txt", "plain.txt"),
    ];
    for (name, stem_of) in cases {
        assert_eq!(stem(name), stem_of, "{name}");
    }
}

#[test]
fn a_name_that_is_only_an_ending_keeps_its_name() {
    for name in [".zip", ".tar.gz", ".gz"] {
        assert_eq!(stem(name), name);
    }
}

#[test]
fn a_free_name_is_used_as_it_is() {
    let scratch = Scratch::new();
    assert_eq!(fresh(scratch.root(), "yedek", true), scratch.path("yedek"));
    assert_eq!(fresh(scratch.root(), "notes.txt", false), scratch.path("notes.txt"));
}

#[test]
fn a_taken_folder_name_is_numbered_at_its_end() {
    let scratch = Scratch::new();
    fs::create_dir(scratch.path("yedek")).unwrap();
    assert_eq!(fresh(scratch.root(), "yedek", true), scratch.path("yedek 2"));
    scratch.write("yedek 2", "a file takes the name as much as a folder");
    assert_eq!(fresh(scratch.root(), "yedek", true), scratch.path("yedek 3"));
    fs::create_dir(scratch.path("site.v2")).unwrap();
    assert_eq!(fresh(scratch.root(), "site.v2", true), scratch.path("site.v2 2"));
}

#[test]
fn a_taken_file_name_is_numbered_before_its_extension() {
    let scratch = Scratch::new();
    scratch.write("notes.txt", "");
    assert_eq!(fresh(scratch.root(), "notes.txt", false), scratch.path("notes 2.txt"));
    scratch.write("notes 2.txt", "");
    assert_eq!(fresh(scratch.root(), "notes.txt", false), scratch.path("notes 3.txt"));
    scratch.write(".bashrc", "");
    assert_eq!(fresh(scratch.root(), ".bashrc", false), scratch.path(".bashrc 2"));
    scratch.write("README", "");
    assert_eq!(fresh(scratch.root(), "README", false), scratch.path("README 2"));
}

#[test]
fn a_broken_link_takes_its_name() {
    let scratch = Scratch::new();
    symlink(scratch.path("nowhere"), scratch.path("yedek")).unwrap();
    assert_eq!(fresh(scratch.root(), "yedek", true), scratch.path("yedek 2"));
}

// --- Tools ---

/// A folder of do-nothing programs with the names given, as a search path.
fn fake_tools(scratch: &Scratch, names: &[&str]) -> OsString {
    for name in names {
        scratch.script(&format!("bin/{name}"), "exit 0");
    }
    fs::create_dir_all(scratch.path("bin")).unwrap();
    scratch.path("bin").into_os_string()
}

fn program_for(format: Format, names: &[&str]) -> Result<String, Missing> {
    let scratch = Scratch::new();
    let path_var = fake_tools(&scratch, names);
    plan(format, Path::new("/a/b.x"), Path::new("/a/.t"), Some(&path_var))
        .map(|plan| plan.program.file_name().unwrap().to_string_lossy().into_owned())
}

#[test]
fn the_first_installed_tool_is_taken_in_order() {
    let all = ["bsdtar", "tar", "unzip", "7z", "7zz", "7za", "unrar"];
    for format in [Format::Zip, Format::Tar, Format::SevenZip, Format::Rar, Format::Disk] {
        assert_eq!(program_for(format, &all).unwrap(), "bsdtar", "{format:?}");
    }
    assert_eq!(program_for(Format::Zip, &["unzip", "tar", "7z"]).unwrap(), "unzip");
    assert_eq!(program_for(Format::Tar, &["tar", "unzip", "7z"]).unwrap(), "tar");
    assert_eq!(program_for(Format::SevenZip, &["7z", "7zz", "7za"]).unwrap(), "7z");
    assert_eq!(program_for(Format::SevenZip, &["7zz", "7za"]).unwrap(), "7zz");
    assert_eq!(program_for(Format::SevenZip, &["7za", "unrar"]).unwrap(), "7za");
    assert_eq!(program_for(Format::Rar, &["unrar", "7z"]).unwrap(), "unrar");
    assert_eq!(program_for(Format::Rar, &["7z", "7za"]).unwrap(), "7z");
    let singles = [
        (Compressor::Gzip, "gzip"),
        (Compressor::Bzip2, "bzip2"),
        (Compressor::Xz, "xz"),
        (Compressor::Zstd, "zstd"),
        (Compressor::Lz4, "lz4"),
        (Compressor::Lzma, "xz"),
        (Compressor::Lzip, "lzip"),
    ];
    for (compressor, program) in singles {
        let names = ["bsdtar", "gzip", "bzip2", "xz", "zstd", "lz4", "lzip"];
        assert_eq!(program_for(Format::Single(compressor), &names).unwrap(), program, "{compressor:?}");
    }
}

#[test]
fn with_no_tool_the_first_one_and_its_package_are_named() {
    let cases = [
        (Format::Zip, "bsdtar", "libarchive"),
        (Format::Tar, "bsdtar", "libarchive"),
        (Format::SevenZip, "bsdtar", "libarchive"),
        (Format::Rar, "bsdtar", "libarchive"),
        (Format::Disk, "bsdtar", "libarchive"),
        (Format::Single(Compressor::Gzip), "gzip", "gzip"),
        (Format::Single(Compressor::Bzip2), "bzip2", "bzip2"),
        (Format::Single(Compressor::Xz), "xz", "xz"),
        (Format::Single(Compressor::Zstd), "zstd", "zstd"),
        (Format::Single(Compressor::Lz4), "lz4", "lz4"),
        (Format::Single(Compressor::Lzma), "xz", "xz"),
        (Format::Single(Compressor::Lzip), "lzip", "lzip"),
    ];
    for (format, wanted, package) in cases {
        assert_eq!(program_for(format, &[]), Err(Missing { wanted, package }), "{format:?}");
        // Another format's tool does not stand in.
        assert!(program_for(format, &["cat"]).is_err());
    }
    assert_eq!(
        plan(Format::Zip, Path::new("a.zip"), Path::new("t"), None),
        Err(Missing { wanted: "bsdtar", package: "libarchive" })
    );
}

#[test]
fn only_a_regular_file_that_may_be_run_counts_as_a_tool() {
    let scratch = Scratch::new();
    scratch.write("bin/bsdtar", "not allowed to run");
    fs::create_dir_all(scratch.path("bin/unzip")).unwrap();
    let path_var = scratch.path("bin").into_os_string();
    let missing = plan(Format::Zip, Path::new("/a.zip"), Path::new("/t"), Some(&path_var));
    assert_eq!(missing, Err(Missing { wanted: "bsdtar", package: "libarchive" }));
}

#[test]
fn folders_are_searched_in_order_and_relative_ones_are_skipped() {
    let scratch = Scratch::new();
    scratch.script("first/unzip", "exit 0");
    scratch.script("second/bsdtar", "exit 0");
    scratch.script("second/unzip", "exit 0");
    let path_var = std::env::join_paths([scratch.path("first"), scratch.path("second")]).unwrap();
    let found = plan(Format::Zip, Path::new("/a.zip"), Path::new("/t"), Some(&path_var)).unwrap();
    assert_eq!(found.program, scratch.path("second/bsdtar"), "the better tool wins over an earlier folder");

    // The same folder, named relative to where the tests run, is not searched.
    let here = std::env::current_dir().unwrap();
    let mut relative = PathBuf::new();
    for _ in here.components().skip(1) {
        relative.push("..");
    }
    relative.push(scratch.path("second").strip_prefix("/").unwrap());
    assert!(relative.is_relative() && relative.join("bsdtar").is_file());
    let missing = plan(Format::Zip, Path::new("/a.zip"), Path::new("/t"), Some(relative.as_os_str()));
    assert_eq!(missing, Err(Missing { wanted: "bsdtar", package: "libarchive" }));
}

#[test]
fn each_tool_is_given_its_own_arguments() {
    let scratch = Scratch::new();
    let archive = Path::new("/d/yedek.x");
    let into = Path::new("/d/.t");
    let cases: &[(Format, &str, &[&str], Output)] = &[
        (
            Format::Zip,
            "bsdtar",
            &["--passphrase", "qexp-no-password", "-x", "-f", "/d/yedek.x", "-C", "/d/.t"],
            Output::Folder,
        ),
        (Format::Tar, "tar", &["-x", "-f", "/d/yedek.x", "-C", "/d/.t"], Output::Folder),
        (Format::Zip, "unzip", &["-P", "qexp-no-password", "/d/yedek.x", "-d", "/d/.t"], Output::Folder),
        (Format::SevenZip, "7z", &["x", "-y", "-pqexp-no-password", "-o/d/.t", "/d/yedek.x"], Output::Folder),
        (Format::SevenZip, "7zz", &["x", "-y", "-pqexp-no-password", "-o/d/.t", "/d/yedek.x"], Output::Folder),
        (Format::SevenZip, "7za", &["x", "-y", "-pqexp-no-password", "-o/d/.t", "/d/yedek.x"], Output::Folder),
        (Format::Rar, "unrar", &["x", "-idq", "-p-", "/d/yedek.x", "/d/.t/"], Output::Folder),
        (Format::Single(Compressor::Gzip), "gzip", &["-dc", "/d/yedek.x"], Output::Stdout),
        (Format::Single(Compressor::Lzma), "xz", &["-dc", "/d/yedek.x"], Output::Stdout),
        (Format::Single(Compressor::Lzip), "lzip", &["-dc", "/d/yedek.x"], Output::Stdout),
    ];
    for (format, program, args, output) in cases {
        let path_var = fake_tools(&scratch, &[program]);
        let found = plan(*format, archive, into, Some(&path_var)).unwrap();
        assert_eq!(found, Plan { program: scratch.path("bin").join(program), args: os(args), output: *output });
        fs::remove_file(scratch.path("bin").join(program)).unwrap();
    }
}

#[test]
fn a_relative_path_cannot_pass_for_an_option() {
    let scratch = Scratch::new();
    let path_var = fake_tools(&scratch, &["unzip", "gzip"]);
    let zip = plan(Format::Zip, Path::new("-o.zip"), Path::new("-t"), Some(&path_var)).unwrap();
    assert_eq!(zip.args, os(&["-P", "qexp-no-password", "./-o.zip", "-d", "./-t"]));
    let gz = plan(Format::Single(Compressor::Gzip), Path::new("-f.gz"), Path::new("t"), Some(&path_var)).unwrap();
    assert_eq!(gz.args, os(&["-dc", "./-f.gz"]));
}

// --- Extracting ---

/// Builds `src/a.txt` and `src/inner/b.txt` below the scratch folder.
fn sources(scratch: &Scratch) {
    scratch.write("src/a.txt", "first");
    scratch.write("src/inner/b.txt", "second");
}

fn assert_holds_sources(folder: &Path) {
    assert_eq!(fs::read_to_string(folder.join("a.txt")).unwrap(), "first");
    assert_eq!(fs::read_to_string(folder.join("inner/b.txt")).unwrap(), "second");
}

/// A zip of the sources at `out/<name>`, or `None` when bsdtar, which builds it, is missing.
fn zip(scratch: &Scratch, name: &str) -> Option<PathBuf> {
    tool("bsdtar")?;
    sources(scratch);
    fs::create_dir_all(scratch.path("out")).unwrap();
    let archive = scratch.path("out").join(name);
    make("bsdtar", &["-a", "-cf", archive.to_str().unwrap(), "-C", "src", "."], scratch.root());
    Some(archive)
}

#[test]
fn a_zip_is_extracted_into_a_new_folder_named_after_it() {
    let scratch = Scratch::new();
    let (Some(archive), Some(path_var)) = (zip(&scratch, "yedek.zip"), path_of(&["bsdtar"])) else { return };
    let made = extract_here(&archive, Some(&path_var)).unwrap();
    assert_eq!(made, scratch.path("out/yedek"));
    assert_holds_sources(&made);
    assert_eq!(scratch.listing("out"), ["yedek", "yedek.zip"], "no temporary folder is left behind");
}

#[test]
fn a_zip_is_extracted_by_unzip_when_it_is_the_only_tool() {
    let scratch = Scratch::new();
    let (Some(archive), Some(path_var)) = (zip(&scratch, "yedek.zip"), scratch.only("bin", &["unzip"])) else {
        return;
    };
    let made = extract_here(&archive, Some(&path_var)).unwrap();
    assert_eq!(made, scratch.path("out/yedek"));
    assert_holds_sources(&made);
    assert_eq!(scratch.listing("out"), ["yedek", "yedek.zip"]);
}

/// A gzipped tar of the sources at `out/<name>`, or `None` when tar or gzip is missing.
fn tar_gz(scratch: &Scratch, name: &str) -> Option<PathBuf> {
    tool("tar")?;
    tool("gzip")?;
    sources(scratch);
    fs::create_dir_all(scratch.path("out")).unwrap();
    let archive = scratch.path("out").join(name);
    make("tar", &["-czf", archive.to_str().unwrap(), "-C", "src", "."], scratch.root());
    Some(archive)
}

#[test]
fn a_tar_gz_is_extracted_into_a_new_folder_named_after_it() {
    let scratch = Scratch::new();
    let (Some(archive), Some(path_var)) = (tar_gz(&scratch, "yedek.tar.gz"), path_of(&["bsdtar"])) else { return };
    let made = extract_here(&archive, Some(&path_var)).unwrap();
    assert_eq!(made, scratch.path("out/yedek"));
    assert_holds_sources(&made);
    assert_eq!(scratch.listing("out"), ["yedek", "yedek.tar.gz"]);
}

#[test]
fn a_tar_gz_is_extracted_by_gnu_tar_when_there_is_no_bsdtar() {
    let scratch = Scratch::new();
    let (Some(archive), Some(path_var)) = (tar_gz(&scratch, "yedek.tgz"), scratch.only("bin", &["tar", "gzip"])) else {
        return;
    };
    let made = extract_here(&archive, Some(&path_var)).unwrap();
    assert_eq!(made, scratch.path("out/yedek"));
    assert_holds_sources(&made);
    assert_eq!(scratch.listing("out"), ["yedek", "yedek.tgz"]);
}

#[test]
fn a_second_extraction_gets_the_next_number_and_the_first_is_untouched() {
    let scratch = Scratch::new();
    let (Some(archive), Some(path_var)) = (tar_gz(&scratch, "yedek.tar.gz"), path_of(&["bsdtar"])) else { return };
    fs::create_dir(scratch.path("out/yedek")).unwrap();
    scratch.write("out/yedek/mine.txt", "kept");
    assert_eq!(extract_here(&archive, Some(&path_var)).unwrap(), scratch.path("out/yedek 2"));
    assert_eq!(extract_here(&archive, Some(&path_var)).unwrap(), scratch.path("out/yedek 3"));
    assert_eq!(scratch.listing("out/yedek"), ["mine.txt"]);
    assert_eq!(fs::read_to_string(scratch.path("out/yedek/mine.txt")).unwrap(), "kept");
    assert_holds_sources(&scratch.path("out/yedek 2"));
    assert_holds_sources(&scratch.path("out/yedek 3"));
    assert_eq!(scratch.listing("out"), ["yedek", "yedek 2", "yedek 3", "yedek.tar.gz"]);
}

#[test]
fn a_lone_gz_becomes_a_file_and_never_replaces_the_original() {
    let scratch = Scratch::new();
    let Some(path_var) = path_of(&["gzip"]) else { return };
    scratch.write("out/notes.txt", "the notes");
    make("gzip", &["-k", "notes.txt"], &scratch.path("out"));
    let archive = scratch.path("out/notes.txt.gz");
    // gzip kept the original beside the archive, so the result takes the next name.
    assert_eq!(extract_here(&archive, Some(&path_var)).unwrap(), scratch.path("out/notes 2.txt"));
    fs::remove_file(scratch.path("out/notes.txt")).unwrap();
    assert_eq!(extract_here(&archive, Some(&path_var)).unwrap(), scratch.path("out/notes.txt"));
    for name in ["notes.txt", "notes 2.txt"] {
        assert_eq!(fs::read_to_string(scratch.path("out").join(name)).unwrap(), "the notes");
    }
    assert_eq!(scratch.listing("out"), ["notes 2.txt", "notes.txt", "notes.txt.gz"]);
}

#[test]
fn every_compressor_unpacks_its_lone_file() {
    let cases: &[(&str, &[&str], &str, &str)] = &[
        ("gzip", &["-k", "f.txt"], "f.txt.gz", "gzip"),
        ("bzip2", &["-k", "f.txt"], "f.txt.bz2", "bzip2"),
        ("xz", &["-k", "f.txt"], "f.txt.xz", "xz"),
        ("xz", &["--format=lzma", "-k", "f.txt"], "f.txt.lzma", "xz"),
        ("zstd", &["-q", "f.txt"], "f.txt.zst", "zstd"),
        ("lz4", &["-q", "f.txt", "f.txt.lz4"], "f.txt.lz4", "lz4"),
        ("lzip", &["-k", "f.txt"], "f.txt.lz", "lzip"),
    ];
    for (maker, args, name, reader) in cases {
        let scratch = Scratch::new();
        let Some(path_var) = path_of(&[maker, reader]) else { continue };
        scratch.write("f.txt", "squeezed");
        make(maker, args, scratch.root());
        fs::remove_file(scratch.path("f.txt")).unwrap();
        let made = extract_here(&scratch.path(name), Some(&path_var)).unwrap();
        assert_eq!(made, scratch.path("f.txt"), "{name}");
        assert_eq!(fs::read_to_string(&made).unwrap(), "squeezed", "{name}");
        assert_eq!(scratch.listing(""), ["f.txt", *name]);
    }
}

#[test]
fn unicode_and_spaces_in_the_name_are_kept() {
    let scratch = Scratch::new();
    let (Some(archive), Some(path_var)) = (tar_gz(&scratch, "Yedek dosyası ğüş.tar.gz"), path_of(&["bsdtar"]))
    else {
        return;
    };
    let made = extract_here(&archive, Some(&path_var)).unwrap();
    assert_eq!(made, scratch.path("out/Yedek dosyası ğüş"));
    assert_holds_sources(&made);

    let Some(path_var) = path_of(&["gzip"]) else { return };
    scratch.write("out/şiir -n ve ş.txt", "dize");
    make("gzip", &["-k", "--", "şiir -n ve ş.txt"], &scratch.path("out"));
    let made = extract_here(&scratch.path("out/şiir -n ve ş.txt.gz"), Some(&path_var)).unwrap();
    assert_eq!(made, scratch.path("out/şiir -n ve ş 2.txt"));
    assert_eq!(fs::read_to_string(made).unwrap(), "dize");
}

#[test]
fn a_very_long_name_still_extracts() {
    let scratch = Scratch::new();
    let Some(path_var) = path_of(&["gzip"]) else { return };
    let name = "ğ".repeat(123) + "a";
    assert_eq!(name.len(), 247);
    scratch.write(&format!("out/{name}"), "long");
    make("gzip", &[&name], &scratch.path("out"));
    let made = extract_here(&scratch.path(&format!("out/{name}.gz")), Some(&path_var)).unwrap();
    assert_eq!(made, scratch.path("out").join(&name));
    assert_eq!(fs::read_to_string(made).unwrap(), "long");
}

#[test]
fn extracting_into_another_folder_makes_the_new_folder_there() {
    let scratch = Scratch::new();
    let (Some(archive), Some(path_var)) = (zip(&scratch, "yedek.zip"), path_of(&["bsdtar"])) else { return };
    fs::create_dir(scratch.path("elsewhere")).unwrap();
    let made = extract_into(&archive, &scratch.path("elsewhere"), Some(&path_var)).unwrap();
    assert_eq!(made, scratch.path("elsewhere/yedek"));
    assert_holds_sources(&made);
    assert_eq!(scratch.listing("out"), ["yedek.zip"]);
    assert_eq!(scratch.listing("elsewhere"), ["yedek"]);
}

#[test]
fn a_corrupt_archive_fails_and_leaves_nothing_behind() {
    let scratch = Scratch::new();
    let Some(path_var) = path_of(&["bsdtar", "gzip"]) else { return };
    let mut broken = Vec::new();
    broken.extend_from_slice(&[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3]);
    broken.extend(std::iter::repeat_n(0xa5, 300));
    let tgz = scratch.write("out/bozuk.tar.gz", &broken);
    let gz = scratch.write("out/bozuk.txt.gz", "not squeezed at all");
    let Err(ExtractError::Failed(said)) = extract_here(&tgz, Some(&path_var)) else {
        panic!("a corrupt tar.gz is reported as a failure");
    };
    assert!(!said.is_empty());
    let Err(ExtractError::Failed(said)) = extract_here(&gz, Some(&path_var)) else {
        panic!("a corrupt gz is reported as a failure");
    };
    assert!(said.contains("not in gzip format"), "{said}");
    assert_eq!(scratch.listing("out"), ["bozuk.tar.gz", "bozuk.txt.gz"]);
}

#[test]
fn a_missing_tool_is_named_and_nothing_is_left_behind() {
    let scratch = Scratch::new();
    let archive = scratch.write("out/yedek.zip", "whatever is inside");
    fs::create_dir(scratch.path("empty")).unwrap();
    let path_var = scratch.path("empty").into_os_string();
    assert_eq!(
        extract_here(&archive, Some(&path_var)),
        Err(ExtractError::Missing(Missing { wanted: "bsdtar", package: "libarchive" }))
    );
    assert_eq!(scratch.listing("out"), ["yedek.zip"]);
}

#[test]
fn a_name_no_archive_has_is_refused() {
    let scratch = Scratch::new();
    let file = scratch.write("notes.txt", "plain");
    let path_var = path_of(&[]).unwrap();
    assert_eq!(extract_here(&file, Some(&path_var)), Err(ExtractError::NotAnArchive));
    assert_eq!(scratch.listing(""), ["notes.txt"]);
}

#[test]
fn an_encrypted_zip_is_reported_as_encrypted_without_waiting_for_a_password() {
    let scratch = Scratch::new();
    if tool("bsdtar").is_none() {
        return;
    }
    sources(&scratch);
    fs::create_dir(scratch.path("out")).unwrap();
    make(
        "bsdtar",
        &[
            "-a",
            "-cf",
            "out/gizli.zip",
            "--options",
            "zip:encryption=zipcrypt",
            "--passphrase",
            "secret",
            "-C",
            "src",
            ".",
        ],
        scratch.root(),
    );
    let archive = scratch.path("out/gizli.zip");
    for tools in [&["bsdtar"][..], &["unzip"][..]] {
        let Some(path_var) = scratch.only(&format!("bin-{}", tools[0]), tools) else { continue };
        assert_eq!(extract_here(&archive, Some(&path_var)), Err(ExtractError::Encrypted), "{tools:?}");
        assert_eq!(scratch.listing("out"), ["gizli.zip"], "{tools:?}");
    }
}

#[test]
fn an_encrypted_zip_whose_check_byte_lets_the_dummy_password_through_is_still_encrypted() {
    // One encrypted zip in 256 fails this way: the tool gets past the password check with qexp's
    // dummy password and then fails to decompress, saying nothing of a password.
    let scratch = Scratch::new();
    let zip =
        unlucky_zip("a.txt", b"a secret note, long enough to be more than a few bytes", "secret", tools::NO_PASSWORD);
    let archive = scratch.write("out/gizli.zip", zip);
    let mut tried = 0;
    for tools in [&["bsdtar"][..], &["unzip"][..]] {
        let Some(path_var) = scratch.only(&format!("bin-{}", tools[0]), tools) else { continue };
        tried += 1;
        assert_eq!(extract_here(&archive, Some(&path_var)), Err(ExtractError::Encrypted), "{tools:?}");
        assert_eq!(scratch.listing("out"), ["gizli.zip"], "{tools:?}");
    }
    if tried == 0 {
        eprintln!("neither bsdtar nor unzip is installed; only the reading of the zip is checked");
    }
    assert!(zip_is_encrypted(&archive));
    let plain = scratch.write("plain.zip", unlucky_zip("a.txt", b"x", "secret", "other"));
    let mut bytes = fs::read(&plain).unwrap();
    // The same archive with the encrypted flag cleared in both headers reads as not encrypted.
    bytes[6] = 0;
    let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    bytes[central + 8] = 0;
    fs::write(&plain, bytes).unwrap();
    assert!(!zip_is_encrypted(&plain));
    assert!(!zip_is_encrypted(&scratch.write("short.zip", "PK")));
}

#[test]
fn members_reaching_outside_the_archive_are_never_written_there() {
    for tools in [&["bsdtar"][..], &["tar"][..]] {
        let scratch = Scratch::new();
        let Some(path_var) = scratch.only("bin", tools) else { continue };
        let absolute = scratch.path("absolute");
        let archive = tar(&[
            ("fine.txt", b"inside"),
            ("../escaped", b"one level up"),
            ("../../escaped-further", b"two levels up"),
            (absolute.to_str().unwrap(), b"anywhere"),
        ]);
        let evil = scratch.write("deep/out/evil.tar", &archive);
        let result = extract_here(&evil, Some(&path_var));
        match result {
            Ok(made) => {
                assert_eq!(made, scratch.path("deep/out/evil"));
                assert_eq!(fs::read_to_string(made.join("fine.txt")).unwrap(), "inside");
                assert_eq!(scratch.listing("deep/out"), ["evil", "evil.tar"], "{tools:?}");
            }
            Err(error) => {
                assert!(matches!(error, ExtractError::Failed(_)), "{tools:?}: {error:?}");
                assert_eq!(scratch.listing("deep/out"), ["evil.tar"], "{tools:?}");
            }
        }
        assert_eq!(scratch.listing("deep"), ["out"], "{tools:?}");
        assert_eq!(scratch.listing(""), ["bin", "deep"], "{tools:?}");
    }
}

#[test]
fn the_tool_sees_only_its_search_path_and_the_c_locale_and_reads_nothing() {
    let scratch = Scratch::new();
    let Some(system) = path_of(&["env", "readlink"]) else { return };
    scratch.script("bin/gzip", "env\nreadlink /proc/$$/fd/0");
    let path_var =
        std::env::join_paths(std::iter::once(scratch.path("bin")).chain(std::env::split_paths(&system))).unwrap();
    let archive = scratch.write("out/report.txt.gz", "");
    let made = extract_here(&archive, Some(&path_var)).unwrap();
    let seen = fs::read_to_string(made).unwrap();
    let lines: Vec<&str> = seen.lines().collect();
    assert!(lines.contains(&"LC_ALL=C"), "{seen}");
    assert!(lines.contains(&format!("PATH={}", path_var.to_string_lossy()).as_str()), "{seen}");
    assert!(std::env::var_os("HOME").is_some(), "the test process has a home the tool must not see");
    assert!(!lines.iter().any(|line| line.starts_with("HOME=")), "{seen}");
    assert_eq!(lines.last(), Some(&"/dev/null"), "{seen}");
}

#[test]
fn a_tool_still_being_written_is_waited_for_a_moment_rather_than_failing() {
    // Another test's program, forked while this one's tool was being written, can hold the file
    // open for writing for a moment; so can a package upgrade replacing the tool. Starting it then
    // fails with "text file busy", which must not become a failed extraction.
    let scratch = Scratch::new();
    let Some(system) = path_of(&["cat"]) else { return };
    let tool = scratch.script("bin/gzip", "exec cat \"$2\"");
    let path_var =
        std::env::join_paths(std::iter::once(scratch.path("bin")).chain(std::env::split_paths(&system))).unwrap();
    let archive = scratch.write("out/notes.txt.gz", "inside");
    let writer = fs::OpenOptions::new().append(true).open(&tool).unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        drop(writer);
    });
    let made = extract_here(&archive, Some(&path_var));
    release.join().unwrap();
    assert_eq!(fs::read_to_string(made.unwrap()).unwrap(), "inside");
}

// --- Stopping ---

#[test]
fn cancelling_a_running_extraction_stops_the_tool_and_leaves_nothing_behind() {
    let scratch = Scratch::new();
    let Some(system) = path_of(&["sleep"]) else { return };
    // A tool that starts writing and then takes far longer than the test waits: the last argument
    // is the folder it was given.
    scratch.script("bin/bsdtar", "for last; do :; done\n: > \"$last/half\"\nexec sleep 60");
    let path_var =
        std::env::join_paths(std::iter::once(scratch.path("bin")).chain(std::env::split_paths(&system))).unwrap();
    let archive = scratch.write("out/yedek.tar.gz", "whatever is inside");
    let started = std::time::Instant::now();
    let bound = std::time::Duration::from_secs(20);
    let written = || {
        fs::read_dir(scratch.path("out")).unwrap().filter_map(Result::ok).any(|entry| {
            entry.file_name().to_string_lossy().starts_with(".yedek.") && entry.path().join("half").exists()
        })
    };
    let mut carry_on = || {
        assert!(started.elapsed() < bound, "the tool never began writing");
        std::thread::sleep(std::time::Duration::from_millis(10));
        !written()
    };
    let result = extract_watched(&archive, &scratch.path("out"), Some(&path_var), &mut carry_on);
    assert_eq!(result, Err(ExtractError::Cancelled));
    assert!(started.elapsed() < bound, "the tool was stopped rather than waited for");
    assert_eq!(scratch.listing("out"), ["yedek.tar.gz"], "the half-written folder is gone");
}
