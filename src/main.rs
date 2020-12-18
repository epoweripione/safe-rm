// Copyright (C) 2020 Francois Marier
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

#![forbid(unsafe_code)]

use glob::glob;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufRead};
use std::path::{self, Path, PathBuf};
use std::process;

const GLOBAL_CONFIG: &str = "/etc/safe-rm.conf";
const LOCAL_GLOBAL_CONFIG: &str = "/usr/local/etc/safe-rm.conf";
const USER_CONFIG: &str = ".config/safe-rm";
const LEGACY_USER_CONFIG: &str = ".safe-rm";

const REAL_RM: &str = "/bin/rm";

const DEFAULT_PATHS: &[&str] = &[
    "/bin",
    "/boot",
    "/dev",
    "/etc",
    "/home",
    "/initrd",
    "/lib",
    "/lib32",
    "/lib64",
    "/proc",
    "/root",
    "/sbin",
    "/sys",
    "/usr",
    "/usr/bin",
    "/usr/include",
    "/usr/lib",
    "/usr/local",
    "/usr/local/bin",
    "/usr/local/include",
    "/usr/local/sbin",
    "/usr/local/share",
    "/usr/sbin",
    "/usr/share",
    "/usr/src",
    "/var",
];

const MAX_GLOB_EXPANSION: usize = 256;

fn read_config<P: AsRef<Path>>(filename: P) -> Option<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if !filename.as_ref().exists() {
        return Some(paths);
    }
    let f = File::open(&filename).ok().or_else(|| {
        println!(
            "safe-rm: Could not open configuration file: {}",
            filename.as_ref().display()
        );
        None
    })?;

    let reader = io::BufReader::new(f);
    for line_result in reader.lines() {
        if let Some(line_paths) = parse_line(filename.as_ref().display(), line_result) {
            paths.extend(line_paths.into_iter());
        }
    }
    Some(paths)
}

#[test]
fn test_read_config() {
    use std::io::Write;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    {
        use std::os::unix::fs::PermissionsExt;

        let file_path = dir.path().join("oneline");
        writeln!(File::create(&file_path).unwrap(), "/home").unwrap();
        let paths = read_config(&file_path).unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths, vec![PathBuf::from("/home")]);

        // Make the file unreadable and check for an error.
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o200); // not readable by anyone
        fs::set_permissions(&file_path, perms).unwrap();
        assert!(read_config(&file_path).is_none());

        // Missing file
        let paths = read_config(dir.path().join("missing")).unwrap();
        assert_eq!(paths.len(), 0);
    }
    {
        let file_path = dir.path().join("empty");
        File::create(&file_path).unwrap();
        assert_eq!(read_config(&file_path).unwrap().len(), 0);
    }
}

fn parse_line(filename: path::Display, line_result: io::Result<String>) -> Option<Vec<PathBuf>> {
    let line = line_result.ok().or_else(|| {
        println!("safe-rm: Ignoring unreadable line in {}.", filename);
        None
    })?;
    let entries = glob(&line).ok().or_else(|| {
        println!(
            "safe-rm: Invalid glob pattern \"{}\" found in {} and ignored.",
            line, filename
        );
        None
    })?;

    let mut paths = Vec::new();

    let mut count = 0;
    for entry in entries {
        match entry {
            Ok(path) => {
                count += 1;
                if count > MAX_GLOB_EXPANSION {
                    println!(
                        "safe-rm: Glob \"{}\" found in {} expands to more than {} paths. Ignoring the rest.",
                        line, filename, MAX_GLOB_EXPANSION
                    );
                    return Some(paths);
                }
                paths.push(path);
            }
            Err(_) => println!(
                "safe-rm: Ignored unreadable path while expanding glob \"{}\" from {}.",
                line, filename
            ),
        }
    }

    Some(paths)
}

#[test]
fn test_parse_line() {
    let filename = Path::new("/");

    // Invalid lines
    assert_eq!(
        parse_line(filename.display(), Ok("/�".to_string()))
            .unwrap()
            .len(),
        0
    );
    assert!(parse_line(
        filename.display(),
        Err(io::Error::new(io::ErrorKind::Other, ""))
    )
    .is_none());
    assert!(parse_line(filename.display(), Ok("/usr/***/bin".to_string())).is_none());

    // Valid lines
    assert_eq!(
        parse_line(filename.display(), Ok("/".to_string())).unwrap(),
        vec![PathBuf::from("/")]
    );
    assert_eq!(
        parse_line(filename.display(), Ok("/tmp/".to_string())).unwrap(),
        vec![PathBuf::from("/tmp")]
    );
    assert_eq!(
        parse_line(filename.display(), Ok("/**".to_string()))
            .unwrap()
            .len(),
        MAX_GLOB_EXPANSION
    );
}

fn symlink_canonicalize(path: &Path) -> Option<PathBuf> {
    // Relative paths need to be prefixed by "./" to have a parent dir.
    let mut explicit_path = path.to_path_buf();
    if explicit_path.is_relative() {
        explicit_path = Path::new(".").join(path);
    }

    // Convert from relative to absolute path but don't follow the symlink.
    // We do this by:
    // 1. splitting directory and base file name
    // 2. canonicalizing the directory
    // 3. recombining directory and file name
    let parent: Option<PathBuf> = match explicit_path.parent() {
        Some(dir) => match dir.canonicalize() {
            Ok(normalized_parent) => Some(normalized_parent),
            Err(_) => None,
        },
        None => Some(PathBuf::from("/")),
    };
    return match parent {
        Some(dir) => match path.file_name() {
            Some(file_name) => Some(dir.join(file_name)),
            None => match dir.parent() {
                // file_name == ".."
                Some(parent_dir) => Some(parent_dir.to_path_buf()),
                None => Some(PathBuf::from("/")), // Stop at the root.
            },
        },
        None => None,
    };
}

#[test]
fn test_symlink_canonicalize() {
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/bin")),
        Some(PathBuf::from("/usr/bin"))
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/bin/../bin/sh")),
        Some(PathBuf::from("/usr/bin/sh"))
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/")),
        Some(PathBuf::from("/usr"))
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/.")),
        Some(PathBuf::from("/usr"))
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/bin/./.././local")),
        Some(PathBuf::from("/usr/local"))
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/..")),
        Some(PathBuf::from("/"))
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/")),
        Some(PathBuf::from("/"))
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/..")),
        Some(PathBuf::from("/"))
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/bin")),
        Some(PathBuf::from("/usr/bin"))
    );

    // Relative path
    assert!(symlink_canonicalize(Path::new("Cargo.toml"))
        .unwrap()
        .is_absolute());

    // Non-existent path
    assert_eq!(
        symlink_canonicalize(Path::new("/non/existent/path/to/file")),
        None
    );
}

fn normalize_path(arg: &str) -> OsString {
    let path = Path::new(arg);

    // Handle symlinks.
    if let Ok(metadata) = path.symlink_metadata() {
        if metadata.file_type().is_symlink() {
            return match symlink_canonicalize(&path) {
                Some(normalized_path) => normalized_path.into_os_string(),
                None => OsString::from(arg),
            };
        }
    }

    // Handle normal files.
    match path.canonicalize() {
        Ok(normalized_pathname) => normalized_pathname.into_os_string(),
        Err(_) => OsString::from(arg),
    }
}

#[test]
fn test_normalize_path() {
    assert_eq!(normalize_path("/"), "/");
    assert_eq!(normalize_path("/../."), "/");
    assert_eq!(normalize_path("/usr"), "/usr");
    assert_eq!(normalize_path("/usr/"), "/usr");
    assert_eq!(normalize_path("/home/../usr"), "/usr");
    assert_eq!(normalize_path(""), "");
    assert_eq!(normalize_path("foo"), "foo");
    assert_eq!(normalize_path("/tmp/�/"), "/tmp/�/");
}

fn filter_arguments(
    args: impl Iterator<Item = String>,
    protected_paths: &[PathBuf],
) -> Vec<String> {
    let mut filtered_args = Vec::new();
    for arg in args {
        if protected_paths.contains(&PathBuf::from(normalize_path(&arg))) {
            println!("safe-rm: Skipping {}.", arg);
        } else {
            filtered_args.push(arg);
        }
    }
    filtered_args
}

#[test]
fn test_filter_arguments() {
    // Simple cases
    assert_eq!(
        filter_arguments(
            vec!["/safe".to_string()].into_iter(),
            &vec![PathBuf::from("/safe")]
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        filter_arguments(
            vec!["/safe".to_string(), "/unsafe".to_string()].into_iter(),
            &vec![PathBuf::from("/safe")]
        ),
        vec!["/unsafe".to_string()]
    );

    // Degenerate cases
    assert_eq!(
        filter_arguments(Vec::<String>::new().into_iter(), &Vec::<PathBuf>::new()),
        Vec::<String>::new()
    );
    assert_eq!(
        filter_arguments(
            vec!["/safe".to_string(), "/unsafe".to_string()].into_iter(),
            &Vec::<PathBuf>::new()
        ),
        vec!["/safe".to_string(), "/unsafe".to_string()]
    );
    assert_eq!(
        filter_arguments(
            Vec::<String>::new().into_iter(),
            &vec![PathBuf::from("/safe")]
        ),
        Vec::<String>::new()
    );

    // Relative path
    assert_eq!(
        filter_arguments(
            vec!["/../".to_string(), "/unsafe".to_string()].into_iter(),
            &vec![PathBuf::from("/")]
        ),
        vec!["/unsafe".to_string()]
    );

    // Symlink tests
    {
        use std::os::unix::fs;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let empty_file = dir.path().join("empty").to_str().unwrap().to_string();
        File::create(&empty_file).unwrap();

        // Normal symlinks should not be protected.
        let unprotected_symlink = dir
            .path()
            .join("unprotected_symlink")
            .to_str()
            .unwrap()
            .to_string();
        fs::symlink(&empty_file, &unprotected_symlink).unwrap();

        // A symlink explicitly listed in a config file should be protected.
        let protected_symlink = dir
            .path()
            .join("protected_symlink")
            .to_str()
            .unwrap()
            .to_string();
        fs::symlink(&empty_file, &protected_symlink).unwrap();

        // A symlink to a protected file should not be protected itself.
        let symlink_to_protected_file = dir.path().join("usr").to_str().unwrap().to_string();
        fs::symlink("/usr", &symlink_to_protected_file).unwrap();

        assert_eq!(
            filter_arguments(
                vec![
                    empty_file.clone(),
                    unprotected_symlink.clone(),
                    protected_symlink.clone(),
                    symlink_to_protected_file.clone()
                ]
                .into_iter(),
                &vec![PathBuf::from("/usr"), PathBuf::from(&protected_symlink)]
            ),
            vec![empty_file, unprotected_symlink, symlink_to_protected_file]
        );
    }
}

fn read_config_files(globals: &[&str], locals: &[&str]) -> Vec<PathBuf> {
    let mut protected_paths = Vec::new();

    for config_file in globals {
        if let Some(paths) = read_config(config_file) {
            protected_paths.extend(paths.into_iter());
        }
    }
    if let Ok(value) = std::env::var("HOME") {
        let home_dir = Path::new(&value);
        for config_file in locals {
            if let Some(paths) = read_config(&home_dir.join(Path::new(config_file))) {
                protected_paths.extend(paths.into_iter());
            }
        }
    }

    if protected_paths.is_empty() {
        for path in DEFAULT_PATHS {
            protected_paths.push(PathBuf::from(path));
        }
    }
    protected_paths.sort();
    protected_paths.dedup();

    protected_paths
}

#[test]
fn test_read_config_files() {
    use std::io::Write;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let file_path1 = dir.path().join("home");
    writeln!(File::create(&file_path1).unwrap(), "/home").unwrap();
    let file_path2 = dir.path().join("tmp");
    writeln!(File::create(&file_path2).unwrap(), "/tmp").unwrap();

    // Empty config
    assert_eq!(read_config_files(&[], &[]).len(), DEFAULT_PATHS.len());

    // Sorted
    assert_eq!(
        read_config_files(
            &[file_path2.to_str().unwrap(), file_path1.to_str().unwrap()],
            &[]
        ),
        vec![PathBuf::from("/home"), PathBuf::from("/tmp")]
    );

    // Duplicate lines
    assert_eq!(
        read_config_files(
            &[file_path1.to_str().unwrap(), file_path1.to_str().unwrap()],
            &[]
        ),
        vec![PathBuf::from("/home")]
    );
}

fn run(
    rm_binary: &str,
    args: impl Iterator<Item = String>,
    globals: &[&str],
    locals: &[&str],
) -> i32 {
    let protected_paths = read_config_files(globals, locals);
    let filtered_args = filter_arguments(args, &protected_paths);

    // Run the real rm command, returning with the same error code.
    match process::Command::new(rm_binary)
        .args(&filtered_args)
        .status()
    {
        Ok(status) => status.code().unwrap_or(1),
        Err(_) => {
            println!("safe-rm: Failed to run the {} command.", REAL_RM);
            1
        }
    }
}

#[test]
fn test_run() {
    use std::io::Write;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let empty_file = dir.path().join("empty").to_str().unwrap().to_string();
    File::create(&empty_file).unwrap();
    let missing_file = dir.path().join("missing").to_str().unwrap().to_string();
    let file1 = dir.path().join("file1").to_str().unwrap().to_string();
    let file2 = dir.path().join("file2").to_str().unwrap().to_string();
    File::create(&file1).unwrap();
    File::create(&file2).unwrap();

    // Trying to delete a directory without "-r" should fail.
    assert_eq!(
        run(
            REAL_RM,
            vec![dir.path().to_str().unwrap().to_string()].into_iter(),
            &[],
            &[]
        ),
        1
    );

    // One file to delete, one directory to ignore.
    assert_eq!(Path::new(&empty_file).exists(), true);
    assert_eq!(
        run(
            REAL_RM,
            vec![empty_file.clone(), "/usr".to_string()].into_iter(),
            &[],
            &[]
        ),
        0
    );
    assert_eq!(Path::new(&empty_file).exists(), false);

    // When the real rm can't be found, run() fails.
    File::create(&empty_file).unwrap();
    assert_eq!(Path::new(&empty_file).exists(), true);
    assert_eq!(
        run(
            &missing_file,
            vec![empty_file.clone()].into_iter(),
            &[],
            &[]
        ),
        1
    );
    assert_eq!(Path::new(&empty_file).exists(), true);

    // Trying to delete a missing file should fail.
    assert_eq!(run(REAL_RM, vec![missing_file].into_iter(), &[], &[]), 1);

    // The "--help" option should work.
    assert_eq!(
        run(REAL_RM, vec!["--help".to_string()].into_iter(), &[], &[]),
        0
    );

    // The contents of a directory can be protected using a wildcard.
    let config_file = dir.path().join("config").to_str().unwrap().to_string();
    writeln!(
        File::create(&config_file).unwrap(),
        "{}",
        dir.path().join("*").to_str().unwrap()
    )
    .unwrap();
    assert_eq!(
        run(
            REAL_RM,
            vec![file1.clone(), file2.clone()].into_iter(),
            &[&config_file],
            &[]
        ),
        1
    );
    assert_eq!(Path::new(&file1).exists(), true);
    assert_eq!(Path::new(&file2).exists(), true);
}

fn ensure_real_rm_is_callable() -> io::Result<()> {
    // Make sure we're not calling ourselves recursively.
    if fs::canonicalize(REAL_RM)? == fs::canonicalize(std::env::current_exe()?)? {
        println!("safe-rm: Cannot find the real \"rm\" binary.");
        process::exit(1);
    }
    Ok(())
}

#[test]
fn test_ensure_real_rm_is_callable() {
    assert!(ensure_real_rm_is_callable().is_ok());
}

fn main() {
    if let Err(e) = ensure_real_rm_is_callable() {
        println!(
            "safe-rm: Cannot check that the real \"rm\" binary is callable: {}",
            e
        );
    }
    process::exit(run(
        REAL_RM,
        std::env::args().skip(1),
        &[GLOBAL_CONFIG, LOCAL_GLOBAL_CONFIG],
        &[USER_CONFIG, LEGACY_USER_CONFIG],
    ));
}
