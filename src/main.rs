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

mod main_test;

use glob::glob;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, BufRead};
use std::path::{self, Path, PathBuf};
use std::process;

use serde_derive::Deserialize;
use std::io::prelude::*;

const GLOBAL_CONFIG: &str = "/etc/safe-rm.conf";
const LOCAL_GLOBAL_CONFIG: &str = "/usr/local/etc/safe-rm.conf";
const USER_CONFIG: &str = ".config/safe-rm";
const LEGACY_USER_CONFIG: &str = ".safe-rm";

const REAL_RM: &str = "/bin/rm";

const SAFE_RM_CONFIG: &str = "/etc/safe-rm.toml";

#[derive(Debug, Deserialize)]
struct Config {
    rm_binary: Option<String>,
}

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
        // Not all config files are expected to be present.
        // If they're missing, we silently skip them.
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

    for entry in entries {
        match entry {
            Ok(path) => {
                if paths.len() >= MAX_GLOB_EXPANSION {
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

fn normalize_path(arg: &OsStr) -> OsString {
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

fn filter_arguments(
    args: impl Iterator<Item = OsString>,
    protected_paths: &[PathBuf],
) -> Vec<OsString> {
    let mut filtered_args = Vec::new();
    for arg in args {
        if protected_paths.contains(&PathBuf::from(normalize_path(&arg))) {
            println!("safe-rm: Skipping {}.", arg.to_string_lossy());
        } else {
            filtered_args.push(arg);
        }
    }
    filtered_args
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

// fn run(
//     rm_binary: &str,
//     args: impl Iterator<Item = OsString>,
//     globals: &[&str],
//     locals: &[&str],
// ) -> i32 {
//     let protected_paths = read_config_files(globals, locals);
//     let filtered_args = filter_arguments(args, &protected_paths);

//     // Run the real rm command, returning with the same error code.
//     match process::Command::new(rm_binary)
//         .args(&filtered_args)
//         .status()
//     {
//         Ok(status) => status.code().unwrap_or(1),
//         Err(_) => {
//             println!("safe-rm: Failed to run the {} command.", REAL_RM);
//             1
//         }
//     }
// }

// fn ensure_real_rm_is_callable() -> io::Result<()> {
//     // Make sure we're not calling ourselves recursively.
//     if fs::canonicalize(REAL_RM)? == fs::canonicalize(std::env::current_exe()?)? {
//         println!("safe-rm: Cannot find the real \"rm\" binary.");
//         process::exit(1);
//     }
//     Ok(())
// }

fn run_binary(
    rm_binary: String,
    args: impl Iterator<Item = OsString>,
    globals: &[&str],
    locals: &[&str],
) -> i32 {
    let protected_paths = read_config_files(globals, locals);
    let filtered_args = filter_arguments(args, &protected_paths);

    // Run the real rm command, returning with the same error code.
    match process::Command::new(&rm_binary)
        .args(&filtered_args)
        .status()
    {
        Ok(status) => status.code().unwrap_or(1),
        Err(_) => {
            
            println!("safe-rm: Failed to run the {} command.", &rm_binary);
            1
        }
    }
}

fn ensure_real_rm_binary_is_callable(real_rm: &mut String) -> io::Result<()> {
    // Make sure we're not calling ourselves recursively.
    if fs::canonicalize(&real_rm)? == fs::canonicalize(std::env::current_exe()?)? {
        println!("safe-rm: Cannot find the real \"{}\" binary.", &real_rm);
        process::exit(1);
    }
    Ok(())
}

fn main() {
    // if let Err(e) = ensure_real_rm_is_callable() {
    //     println!(
    //         "safe-rm: Cannot check that the real \"rm\" binary is callable: {}",
    //         e
    //     );
    // }
    // process::exit(run(
    //     REAL_RM,
    //     std::env::args_os().skip(1),
    //     &[GLOBAL_CONFIG, LOCAL_GLOBAL_CONFIG],
    //     &[USER_CONFIG, LEGACY_USER_CONFIG],
    // ));

    let mut real_rm_binary: String = "".to_string();

    // For security reasons the real `rm` binary maybe renamed, e.g.: `/bin/rm.real`
    // Get real `rm` binary from `/etc/safe-rm.toml`
    // e.g.: rm_binary = "/bin/rm.real"
    let mut toml_content = String::new();
    if Path::new(SAFE_RM_CONFIG).exists() {
        match File::open(SAFE_RM_CONFIG) {
            Ok(mut file) => {
                file.read_to_string(&mut toml_content).unwrap();
            },
            Err(error) => {
                println!("Error opening file {}: {}", SAFE_RM_CONFIG, error);
            },
        }
    }

    if ! toml_content.is_empty() {
        let config: Config = toml::from_str(&toml_content).unwrap();
        let toml_real_rm = config.rm_binary.unwrap();
        if ! toml_real_rm.is_empty() {
            real_rm_binary = toml_real_rm;
        }
    }

    // Get real `rm` binary from enviroment variable `SAFE_RM_REAL_RM_BINARY`
    // e.g.: export SAFE_RM_REAL_RM="/bin/rm.real"
    if real_rm_binary.is_empty() {
        if let Ok(value) = std::env::var("SAFE_RM_REAL_RM") {
            let path  = normalize_path(Path::new(&value).as_os_str());
            real_rm_binary = path.to_str().unwrap().to_string();
        }
    }

    if real_rm_binary.is_empty() {
        real_rm_binary = String::from(REAL_RM);
    }

    if let Err(e) = ensure_real_rm_binary_is_callable(&mut real_rm_binary) {
        println!(
            "safe-rm: Cannot check that the real \"{}\" binary is callable: {}",
            real_rm_binary,
            e
        );
    }

    // println!("{}", real_rm_binary);

    process::exit(run_binary(
        real_rm_binary,
        std::env::args_os().skip(1),
        &[GLOBAL_CONFIG, LOCAL_GLOBAL_CONFIG],
        &[USER_CONFIG, LEGACY_USER_CONFIG],
    ));
}
