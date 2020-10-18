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

fn read_config<P>(filename: P) -> Result<Vec<String>, io::Error>
where P: AsRef<Path> {  // TODO: figure out what this line does exactly
    let mut excluded_paths = Vec::new();

    let f = File::open(filename)?;
    let reader = io::BufReader::new(f);
    for line_result in reader.lines() {
        let line = line_result?;  // TODO: warn about invalid lines instead
        excluded_paths.push(line);
    }
    Ok(excluded_paths)
}

fn normalize_path(pathname: &str) -> String {
    // TODO: return original pathname in case of unwrap() errors
    let normalized_pathname = fs::canonicalize(pathname).unwrap();
    normalized_pathname.to_str().unwrap().to_string()
}

#[test]
fn test_normalize_path() {
    assert_eq!(normalize_path("/"), "/");
    assert_eq!(normalize_path("/../."), "/");
    assert_eq!(normalize_path("/usr"), "/usr");
    assert_eq!(normalize_path("/usr/"), "/usr");
    assert_eq!(normalize_path("/home/../usr"), "/usr");
    // TODO: re-enable these tests once unwrap() errors are handled
    //assert_eq!(normalize_path(""), "");
    //assert_eq!(normalize_path("foo"), "foo");
}

fn main() {
    let mut protected_paths = Vec::new();

    // System-wide config
    match read_config("/etc/safe-rm.conf") {
        Ok(mut paths) => protected_paths.append(&mut paths),
        Err(_) => ()
    };
    // Alternative system-wide config
    match read_config("/usr/local/etc/safe-rm.conf") {
        Ok(mut paths) => protected_paths.append(&mut paths),
        Err(_) => ()
    };
    match std::env::var("HOME") {
        Ok(value) => {
            let home_dir = Path::new(&value);
            // User config file
            match read_config(&home_dir.join(Path::new(".config/safe-rm"))) {
                Ok(mut paths) => protected_paths.append(&mut paths),
                Err(_) => ()
            };
            // Legacy user config file
            match read_config(&home_dir.join(Path::new(".safe-rm"))) {
                Ok(mut paths) => protected_paths.append(&mut paths),
                Err(_) => ()
            };
        },
        Err(_) => ()
    }

    if protected_paths.is_empty() {
        // TODO: move to a separate function
        protected_paths = vec![
            "/bin".to_string(),
            "/boot".to_string(),
            "/dev".to_string(),
            "/etc".to_string(),
            "/home".to_string(),
            "/initrd".to_string(),
            "/lib".to_string(),
            "/lib32".to_string(),
            "/lib64".to_string(),
            "/proc".to_string(),
            "/root".to_string(),
            "/sbin".to_string(),
            "/sys".to_string(),
            "/usr".to_string(),
            "/usr/bin".to_string(),
            "/usr/include".to_string(),
            "/usr/lib".to_string(),
            "/usr/local".to_string(),
            "/usr/local/bin".to_string(),
            "/usr/local/include".to_string(),
            "/usr/local/sbin".to_string(),
            "/usr/local/share".to_string(),
            "/usr/sbin".to_string(),
            "/usr/share".to_string(),
            "/usr/src".to_string(),
            "/var".to_string()
        ];
    }
    protected_paths.sort();
    protected_paths.dedup();
    println!("{:#?}", protected_paths);

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
