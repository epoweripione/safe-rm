use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;

fn read_config<P>(filename: P) -> Result<Vec<String>, io::Error>
where P: AsRef<Path> {  // TODO: figure out what this line does exactly
    let mut excluded_paths = Vec::new();

    let f = File::open(filename)?;
    let reader = io::BufReader::new(f);
    for line_result in reader.lines() {
        let line = line_result?;  // TODO: warn invalid lines instead
        excluded_paths.push(line);
    }
    Ok(excluded_paths)
}

fn normalize_path(pathname: &str) -> String {
    let mut normalized_pathname = pathname.to_string();
    // TODO: Normalize pathname.
    // TODO: Convert to an absolute path (e.g. remove "..").
    // TODO: Trim trailing slashes.
    normalized_pathname
}

#[test]
fn test_normalize_path() {
    assert_eq!(normalize_path(""), "");
    assert_eq!(normalize_path("foo"), "foo");
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
        // TODO: provide some default protected paths
    }
    protected_paths.sort();
    protected_paths.dedup();
    println!("{:#?}", protected_paths);

    let mut filtered_args = Vec::new();
    for pathname in std::env::args().skip(1) {
        let normalized_pathname = normalize_path(&pathname);
        println!("{} -> {}", pathname, normalized_pathname);
        // TODO: Check against protected_paths.
        filtered_args.push(pathname);
    }

    // TODO: Run the real rm command.
    println!("{:#?}", filtered_args);
}
