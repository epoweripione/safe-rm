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

use glob::glob;
use std::fs::{self, File};
use std::io::{self, BufRead};
use std::path::{self, Path};
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

fn read_config<P: AsRef<Path>>(filename: P, mut paths: &mut Vec<String>) -> bool {
    if !filename.as_ref().exists() {
        return true;
    }
    match File::open(&filename) {
        Ok(f) => {
            let reader = io::BufReader::new(f);
            for line_result in reader.lines() {
                parse_line(filename.as_ref().display(), line_result, &mut paths);
            }
            true
        }
        Err(_) => {
            println!(
                "safe-rm: Could not open configuration file: {}",
                filename.as_ref().display()
            );
            false
        }
    }
}

#[test]
fn test_read_config() {
    use std::io::Write;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    {
        use std::os::unix::fs::PermissionsExt;

        let mut paths = Vec::<String>::new();
        let file_path = dir.path().join("oneline");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "/home").unwrap();
        assert!(read_config(&file_path, &mut paths));
        assert_eq!(paths.len(), 1);
        assert_eq!(paths, vec!["/home".to_string()]);

        // Make the file unreadable and check for an error.
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o200); // not readable by anyone
        fs::set_permissions(&file_path, perms).unwrap();
        paths.clear();
        assert!(!read_config(file_path, &mut paths));
        assert_eq!(paths.len(), 0);
    }
    {
        let mut paths = Vec::<String>::new();
        let file_path = dir.path().join("empty");
        File::create(&file_path).unwrap();
        assert!(read_config(file_path, &mut paths));
        assert_eq!(paths.len(), 0);
    }
}

fn parse_line(filename: path::Display, line_result: io::Result<String>, paths: &mut Vec<String>) {
    match line_result {
        Ok(line) => match glob(&line) {
            Ok(entries) => {
                let mut count = 0;
                for entry in entries {
                    match entry {
                        Ok(path) => {
                            if let Some(path_str) = path.to_str() {
                                count += 1;
                                if count > MAX_GLOB_EXPANSION {
                                    println!(
                                    "safe-rm: Glob \"{}\" found in {} expands to more than {} paths. Ignoring the rest.",
                                    line, filename, MAX_GLOB_EXPANSION
                                );
                                    return;
                                }
                                paths.push(path_str.to_string());
                            }
                        }
                        Err(_) => println!(
                            "safe-rm: Ignored unreadable path while expanding glob \"{}\" from {}.",
                            line, filename
                        ),
                    }
                }
            }
            Err(_) => println!(
                "safe-rm: Invalid glob pattern \"{}\" found in {} and ignored.",
                line, filename
            ),
        },
        Err(_) => println!("safe-rm: Invalid line found in {} and ignored.", filename),
    }
}

#[test]
fn test_parse_line() {
    let filename = Path::new("/");
    {
        let mut paths = Vec::new();
        parse_line(filename.display(), Ok("/�".to_string()), &mut paths);
        assert_eq!(paths, Vec::<String>::new());
    }
    {
        let mut paths = Vec::new();
        parse_line(filename.display(), Ok("/".to_string()), &mut paths);
        assert_eq!(paths, vec!["/".to_string()]);
    }
    {
        let mut paths = Vec::new();
        parse_line(filename.display(), Ok("/tmp/".to_string()), &mut paths);
        assert_eq!(paths, vec!["/tmp".to_string()]);
    }
    {
        let mut paths = Vec::new();
        parse_line(
            filename.display(),
            Ok("/usr/***/bin".to_string()),
            &mut paths,
        );
        assert_eq!(paths, Vec::<String>::new());
    }
    {
        let mut paths = Vec::new();
        parse_line(filename.display(), Ok("/**".to_string()), &mut paths);
        assert_eq!(paths.len(), MAX_GLOB_EXPANSION);
    }
    {
        let mut paths = Vec::new();
        parse_line(
            filename.display(),
            Err(io::Error::new(io::ErrorKind::Other, "")),
            &mut paths,
        );
        assert_eq!(paths, Vec::<String>::new());
    }
}

fn symlink_canonicalize(path: &Path) -> Option<String> {
    // Relative paths need to be prefixed by "./" to have a parent dir.
    let mut explicit_path = path.to_path_buf();
    if let Some(first_char) = path.to_string_lossy().chars().next() {
        if first_char != '/' {
            explicit_path = Path::new(".").join(path);
        }
    }

    // Convert from relative to absolute path but don't follow the symlink.
    // We do this by:
    // 1. splitting directory and base file name
    // 2. canonicalizing the directory
    // 3. recombining directory and file name
    let parent: Option<path::PathBuf> = match explicit_path.parent() {
        Some(dir) => match dir.canonicalize() {
            Ok(normalized_parent) => Some(normalized_parent),
            Err(_) => None,
        },
        None => Some(Path::new("/").to_path_buf()),
    };
    return match parent {
        Some(dir) => match path.file_name() {
            Some(file_name) => match dir.join(file_name).to_str() {
                Some(pathname_str) => Some(pathname_str.to_string()),
                None => None,
            },
            None => match dir.parent() {
                // file_name == ".."
                Some(parent_dir) => match parent_dir.to_str() {
                    Some(parent_str) => Some(parent_str.to_string()),
                    None => None,
                },
                None => Some("/".to_string()), // Stop at the root.
            },
        },
        None => None,
    };
}

#[test]
fn test_symlink_canonicalize() {
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/bin")),
        Some("/usr/bin".to_string())
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/bin/../bin/sh")),
        Some("/usr/bin/sh".to_string())
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/")),
        Some("/usr".to_string())
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/.")),
        Some("/usr".to_string())
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/bin/./.././local")),
        Some("/usr/local".to_string())
    );
    assert_eq!(
        symlink_canonicalize(Path::new("/usr/..")),
        Some("/".to_string())
    );
    assert_eq!(symlink_canonicalize(Path::new("/")), Some("/".to_string()));
    assert_eq!(
        symlink_canonicalize(Path::new("/..")),
        Some("/".to_string())
    );
}

fn normalize_path(pathname: &str) -> String {
    let path = Path::new(pathname);

    // Handle symlinks.
    if let Ok(metadata) = path.symlink_metadata() {
        if metadata.file_type().is_symlink() {
            return match symlink_canonicalize(&path) {
                Some(normalized_path) => normalized_path,
                None => pathname.to_string(),
            };
        }
    }

    // Handle normal files.
    match path.canonicalize() {
        Ok(normalized_pathname) => match normalized_pathname.to_str() {
            Some(normalized_pathname_str) => normalized_pathname_str.to_string(),
            None => pathname.to_string(),
        },
        Err(_) => pathname.to_string(),
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

fn filter_pathnames(args: impl Iterator<Item = String>, protected_paths: &[String]) -> Vec<String> {
    let mut filtered_args = Vec::new();
    for pathname in args {
        let normalized_pathname = normalize_path(&pathname);
        if protected_paths.contains(&normalized_pathname) {
            println!("safe-rm: Skipping {}.", pathname);
        } else {
            filtered_args.push(pathname);
        }
    }
    filtered_args
}

#[test]
fn test_filter_pathnames() {
    // Simple cases
    assert_eq!(
        filter_pathnames(
            vec!["/safe".to_string()].into_iter(),
            &vec!["/safe".to_string()]
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        filter_pathnames(
            vec!["/safe".to_string(), "/unsafe".to_string()].into_iter(),
            &vec!["/safe".to_string()]
        ),
        vec!["/unsafe".to_string()]
    );

    // Degenerate cases
    assert_eq!(
        filter_pathnames(Vec::<String>::new().into_iter(), &Vec::<String>::new()),
        Vec::<String>::new()
    );
    assert_eq!(
        filter_pathnames(
            vec!["/safe".to_string(), "/unsafe".to_string()].into_iter(),
            &Vec::<String>::new()
        ),
        vec!["/safe".to_string(), "/unsafe".to_string()]
    );
    assert_eq!(
        filter_pathnames(Vec::<String>::new().into_iter(), &vec!["/safe".to_string()]),
        Vec::<String>::new()
    );

    // Relative path
    assert_eq!(
        filter_pathnames(
            vec!["/../".to_string(), "/unsafe".to_string()].into_iter(),
            &vec!["/".to_string()]
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
            filter_pathnames(
                vec![
                    empty_file.clone(),
                    unprotected_symlink.clone(),
                    protected_symlink.clone(),
                    symlink_to_protected_file.clone()
                ]
                .into_iter(),
                &vec!["/usr".to_string(), protected_symlink.clone()]
            ),
            vec![empty_file, unprotected_symlink, symlink_to_protected_file]
        );
    }
}

fn finalize_protected_paths(protected_paths: &mut Vec<String>) {
    if protected_paths.is_empty() {
        for path in DEFAULT_PATHS {
            protected_paths.push(path.to_string());
        }
    }
    protected_paths.sort();
    protected_paths.dedup();
}

#[test]
fn test_finalize_protected_paths() {
    {
        let mut paths = vec![];
        finalize_protected_paths(&mut paths);
        assert_eq!(paths, DEFAULT_PATHS);
    }
    {
        let mut paths = vec!["/two".to_string(), "/one".to_string()];
        finalize_protected_paths(&mut paths);
        assert_eq!(paths, vec!["/one".to_string(), "/two".to_string()]);
    }
    {
        let mut paths = vec!["/one".to_string(), "/one".to_string()];
        finalize_protected_paths(&mut paths);
        assert_eq!(paths, vec!["/one".to_string()]);
    }
}

fn read_config_files() -> Vec<String> {
    let mut protected_paths = Vec::new();
    read_config(GLOBAL_CONFIG, &mut protected_paths);
    read_config(LOCAL_GLOBAL_CONFIG, &mut protected_paths);
    if let Ok(value) = std::env::var("HOME") {
        let home_dir = Path::new(&value);
        read_config(&home_dir.join(Path::new(USER_CONFIG)), &mut protected_paths);
        read_config(
            &home_dir.join(Path::new(LEGACY_USER_CONFIG)),
            &mut protected_paths,
        );
    }
    protected_paths
}

fn run(args: impl Iterator<Item = String>) -> i32 {
    let mut protected_paths = read_config_files();
    finalize_protected_paths(&mut protected_paths);

    let filtered_args = filter_pathnames(args, &protected_paths);

    // Run the real rm command, returning with the same error code.
    match process::Command::new(REAL_RM).args(&filtered_args).status() {
        Ok(status) => match status.code() {
            Some(code) => code,
            None => 1,
        },
        Err(_) => {
            println!("safe-rm: Failed to run the {} command.", REAL_RM);
            1
        }
    }
}

#[test]
fn test_run() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let empty_file = dir.path().join("empty").to_str().unwrap().to_string();
    File::create(&empty_file).unwrap();
    let missing_file = dir.path().join("missing").to_str().unwrap().to_string();

    // Trying to delete a directory without "-r" should fail.
    assert_eq!(
        run(vec![dir.path().to_str().unwrap().to_string()].into_iter()),
        1
    );

    // One file to delete, one directory to ignore.
    assert_eq!(Path::new(&empty_file).exists(), true);
    assert_eq!(
        run(vec![empty_file.clone(), "/usr".to_string()].into_iter()),
        0
    );
    assert_eq!(Path::new(&empty_file).exists(), false);

    // Trying to delete a missing file should fail.
    assert_eq!(run(vec![missing_file].into_iter()), 1);

    // The "--help" option should work.
    assert_eq!(run(vec!["--help".to_string()].into_iter()), 0);
}

fn main() {
    // Make sure we're not calling ourselves recursively.
    if fs::canonicalize(REAL_RM).unwrap()
        == fs::canonicalize(std::env::current_exe().unwrap()).unwrap()
    {
        println!("safe-rm: Cannot find the real \"rm\" binary.");
        process::exit(1);
    }

    process::exit(run(std::env::args().skip(1)));
}
