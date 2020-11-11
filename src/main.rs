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

const GLOBAL_CONFIG: &str = "/etc/safe-rm.conf";
const LOCAL_GLOBAL_CONFIG: &str = "/usr/local/etc/safe-rm.conf";
const USER_CONFIG: &str = ".config/safe-rm";
const LEGACY_USER_CONFIG: &str = ".safe-rm";

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

fn read_config<P>(filename: P, paths: &mut Vec<String>) where P: AsRef<Path> {  // TODO: figure out what this line does exactly
    match File::open(filename) {
        Ok(f) => {
            let reader = io::BufReader::new(f);
            for line_result in reader.lines() {
                match line_result {
                    Ok(line) => {
                        paths.push(line);
                    },
                    Err(_) => {
                        // TODO: warn about invalid line
                    }
                }
            }
        },
        Err(_) => ()
    }
}

fn normalize_path(pathname: &str) -> String {
    match fs::canonicalize(pathname) {
        Ok(normalized_pathname) => {
            normalized_pathname.to_str().unwrap().to_string()  // TODO: remove unwrap
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
        println!("{} -> {}", pathname, normalized_pathname);
        if !protected_paths.contains(&normalized_pathname) {
            filtered_args.push(pathname);
        }
    }

    // TODO: Run the real rm command.
    println!("{:#?}", filtered_args);
}
