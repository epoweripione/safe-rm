use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;

fn read_config<P>(filename: P) -> Result<(), io::Error>
where P: AsRef<Path>, {
    let f = File::open(filename)?;
    let reader = io::BufReader::new(f);
    for line_result in reader.lines() {
        let line = line_result?;
        println!("{}", line);
    }
    Ok(())  // TODO: return a vector of exclusions
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
    read_config("/etc/safe-rm.conf");
    read_config("/usr/local/etc/safe-rm.conf");
    // TODO: read_config("~/.safe-rm");
    // TODO: read_config("~/.config/safe-rm");

    // TODO: use default protected paths if none were configured

    for pathname in std::env::args().skip(1) {
        let normalized_pathname = normalize_path(&pathname);
        // TODO: Check against the exclusions.
        println!("{} -> {}", pathname, normalized_pathname);
    }
}
