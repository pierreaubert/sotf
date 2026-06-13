//! Temporary database helpers for integration tests.

use std::path::PathBuf;
use tempfile::TempDir;

/// Create a temporary directory containing an empty SQLite database file.
///
/// Returns the `TempDir` (so it stays alive for the duration of the test)
/// and the path to the database file.
pub fn temp_sqlite_db() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    (temp_dir, db_path)
}

/// Create a temporary directory and copy the given file names into it.
pub fn temp_files(file_names: &[&str]) -> (TempDir, Vec<PathBuf>) {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let paths: Vec<PathBuf> = file_names
        .iter()
        .map(|name| {
            let path = temp_dir.path().join(name);
            std::fs::write(&path, b"").expect("failed to create temp file");
            path
        })
        .collect();
    (temp_dir, paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_sqlite_db_creates_file() {
        let (_dir, path) = temp_sqlite_db();
        assert!(path.ends_with("test.db"));
        assert!(!path.exists());
    }

    #[test]
    fn temp_files_creates_expected_paths() {
        let (dir, paths) = temp_files(&["a.txt", "b.txt"]);
        assert_eq!(paths.len(), 2);
        for path in &paths {
            assert!(path.exists());
            assert!(path.starts_with(dir.path()));
        }
    }
}
