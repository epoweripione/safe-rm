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

use std::fs;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
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

fn read_config<P: AsRef<Path>>(filename: P, paths: &mut Vec<String>) {
    match File::open(filename) {
        Ok(f) => {
            let reader = io::BufReader::new(f);
            for line_result in reader.lines() {
                match line_result {
                    Ok(line) => {
                        paths.push(line);
                    },
                    Err(_) => {
                        // TODO: make this error message work
                        //println!("Invalid line found in {} and ignored.", filename.display());
                        println!("Invalid line found and ignored.");  // TODO: remove this line
                    }
                }
            }
        },
        Err(_) => {
            // TODO: make this error message work
            //println!("Could not open configuration file: {}", filename.display());
            ()
        }
    }
}

fn normalize_path(pathname: &str) -> String {
    match fs::canonicalize(pathname) {
        Ok(normalized_pathname) => {
            match normalized_pathname.to_str() {
                Some(normalized_pathname_str) => {
                    normalized_pathname_str.to_string()
                },
                None => pathname.to_string()
            }
        },
        Err(_) => pathname.to_string()
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

fn main() {
    let mut protected_paths = Vec::new();

    read_config(GLOBAL_CONFIG, &mut protected_paths);
    read_config(LOCAL_GLOBAL_CONFIG, &mut protected_paths);
    match std::env::var("HOME") {
        Ok(value) => {
            let home_dir = Path::new(&value);
            read_config(&home_dir.join(Path::new(USER_CONFIG)), &mut protected_paths);
            read_config(&home_dir.join(Path::new(LEGACY_USER_CONFIG)), &mut protected_paths);
        },
        Err(_) => ()
    }

    if protected_paths.is_empty() {
        for path in DEFAULT_PATHS {
            protected_paths.push(path.to_string());
        }
    }
    protected_paths.sort();
    protected_paths.dedup();
    println!("{:#?}", protected_paths);  // TODO: remove this line

    let mut filtered_args = Vec::new();
    for pathname in std::env::args().skip(1) {
        let normalized_pathname = normalize_path(&pathname);
        println!("{} -> {}", pathname, normalized_pathname); // TODO: remove this line
        if protected_paths.contains(&normalized_pathname) {  // TODO: skip symlinks
            println!("safe-rm: skipping {}", pathname);
        } else {
            filtered_args.push(pathname);
        }
    }

    // Make sure we're not calling ourselves recursively.
    if fs::canonicalize(REAL_RM).unwrap() == fs::canonicalize(std::env::current_exe().unwrap()).unwrap() {
        println!("safe-rm cannot find the real \"rm\" binary");
        process::exit(1);
    }

    println!("{} {:#?}", REAL_RM, filtered_args);  // TODO: remove this line
    // TODO: Run the real rm command, returning with the same error code
}
