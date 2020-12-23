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

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs::{self, File};
    use std::io;
    use std::path::{Path, PathBuf};

    #[test]
    fn read_config() {
        use super::super::read_config;

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
            assert!(paths.is_empty());
        }
        {
            let file_path = dir.path().join("empty");
            File::create(&file_path).unwrap();
            assert!(read_config(&file_path).unwrap().is_empty());
        }
    }

    #[test]
    fn parse_line() {
        use super::super::parse_line;
        use super::super::MAX_GLOB_EXPANSION;

        let filename = Path::new("/");

        // Invalid lines
        assert!(parse_line(filename.display(), Ok("/�".to_string()))
            .unwrap()
            .is_empty());
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

    #[test]
    fn symlink_canonicalize() {
        use super::super::symlink_canonicalize;

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

    #[test]
    fn normalize_path() {
        use super::super::normalize_path;

        assert_eq!(normalize_path(&OsString::from("/".to_string())), "/");
        assert_eq!(normalize_path(&OsString::from("/../.".to_string())), "/");
        assert_eq!(normalize_path(&OsString::from("/usr".to_string())), "/usr");
        assert_eq!(normalize_path(&OsString::from("/usr/".to_string())), "/usr");
        assert_eq!(
            normalize_path(&OsString::from("/home/../usr".to_string())),
            "/usr"
        );
        assert_eq!(normalize_path(&OsString::from("".to_string())), "");
        assert_eq!(normalize_path(&OsString::from("foo".to_string())), "foo");
        assert_eq!(
            normalize_path(&OsString::from("/tmp/�/".to_string())),
            "/tmp/�/"
        );
    }

    #[test]
    fn filter_arguments() {
        use super::super::filter_arguments;

        // Simple cases
        assert_eq!(
            filter_arguments(
                vec![OsString::from("/safe".to_string())].into_iter(),
                &vec![PathBuf::from("/safe")]
            ),
            Vec::<OsString>::new()
        );
        assert_eq!(
            filter_arguments(
                vec![
                    OsString::from("/safe".to_string()),
                    OsString::from("/unsafe".to_string())
                ]
                .into_iter(),
                &vec![PathBuf::from("/safe")]
            ),
            vec![OsString::from("/unsafe".to_string())]
        );

        // Degenerate cases
        assert_eq!(
            filter_arguments(Vec::<OsString>::new().into_iter(), &Vec::<PathBuf>::new()),
            Vec::<OsString>::new()
        );
        assert_eq!(
            filter_arguments(
                vec![
                    OsString::from("/safe".to_string()),
                    OsString::from("/unsafe".to_string())
                ]
                .into_iter(),
                &Vec::<PathBuf>::new()
            ),
            vec![
                OsString::from("/safe".to_string()),
                OsString::from("/unsafe".to_string())
            ]
        );
        assert_eq!(
            filter_arguments(
                Vec::<OsString>::new().into_iter(),
                &vec![PathBuf::from("/safe")]
            ),
            Vec::<OsString>::new()
        );

        // Relative path
        assert_eq!(
            filter_arguments(
                vec![
                    OsString::from("/../".to_string()),
                    OsString::from("/unsafe".to_string())
                ]
                .into_iter(),
                &vec![PathBuf::from("/")]
            ),
            vec![OsString::from("/unsafe".to_string())]
        );

        // Symlink tests
        {
            use std::os::unix::fs;
            use tempfile::tempdir;

            let dir = tempdir().unwrap();
            let empty_file = dir.path().join("empty");
            File::create(&empty_file).unwrap();

            // Normal symlinks should not be protected.
            let unprotected_symlink = dir.path().join("unprotected_symlink");
            fs::symlink(&empty_file, &unprotected_symlink).unwrap();

            // A symlink explicitly listed in a config file should be protected.
            let protected_symlink = dir.path().join("protected_symlink");
            fs::symlink(&empty_file, &protected_symlink).unwrap();

            // A symlink to a protected file should not be protected itself.
            let symlink_to_protected_file = dir.path().join("usr");
            fs::symlink("/usr", &symlink_to_protected_file).unwrap();

            assert_eq!(
                filter_arguments(
                    vec![
                        OsString::from(&empty_file),
                        OsString::from(&unprotected_symlink),
                        OsString::from(&protected_symlink),
                        OsString::from(&symlink_to_protected_file),
                    ]
                    .into_iter(),
                    &vec![PathBuf::from("/usr"), PathBuf::from(&protected_symlink)]
                ),
                vec![empty_file, unprotected_symlink, symlink_to_protected_file]
            );
        }
    }

    #[test]
    fn read_config_files() {
        use super::super::read_config_files;
        use super::super::DEFAULT_PATHS;

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

    #[test]
    fn run() {
        use super::super::run;
        use super::super::REAL_RM;

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
                vec![OsString::from(dir.path())].into_iter(),
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
                vec![
                    OsString::from(&empty_file),
                    OsString::from("/usr".to_string())
                ]
                .into_iter(),
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
                vec![OsString::from(&empty_file)].into_iter(),
                &[],
                &[]
            ),
            1
        );
        assert_eq!(Path::new(&empty_file).exists(), true);

        // Trying to delete a missing file should fail.
        assert_eq!(
            run(
                REAL_RM,
                vec![OsString::from(&missing_file)].into_iter(),
                &[],
                &[]
            ),
            1
        );

        // The "--help" option should work.
        assert_eq!(
            run(
                REAL_RM,
                vec![OsString::from("--help".to_string())].into_iter(),
                &[],
                &[]
            ),
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
                vec![OsString::from(&file1), OsString::from(&file2)].into_iter(),
                &[&config_file],
                &[]
            ),
            1
        );
        assert_eq!(Path::new(&file1).exists(), true);
        assert_eq!(Path::new(&file2).exists(), true);
    }

    #[test]
    fn ensure_real_rm_is_callable() {
        use super::super::ensure_real_rm_is_callable;

        assert!(ensure_real_rm_is_callable().is_ok());
    }
}
