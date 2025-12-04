/// Shared test fixtures and utilities for src-audio-player integration tests
use std::path::PathBuf;
use tempfile::TempDir;

/// Returns the path to the demo audio files directory
pub fn demo_audio_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sotf-tauri")
        .join("public")
        .join("demo-audio")
}

/// Returns all WAV files in the demo audio directory
pub fn all_wav_files() -> Vec<PathBuf> {
    let dir = demo_audio_dir();
    std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "wav" {
                Some(path)
            } else {
                None
            }
        })
        .collect()
}

/// Returns all FLAC files in the demo audio directory
pub fn all_flac_files() -> Vec<PathBuf> {
    let dir = demo_audio_dir();
    std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "flac" {
                Some(path)
            } else {
                None
            }
        })
        .collect()
}

/// Returns a specific demo audio file by name
pub fn get_demo_file(name: &str) -> PathBuf {
    demo_audio_dir().join(name)
}

/// Creates a temporary database file for testing
pub fn temp_database() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_music.db");
    (temp_dir, db_path)
}

/// Verifies that the demo audio directory exists and contains files
pub fn ensure_demo_files_exist() {
    let dir = demo_audio_dir();
    assert!(
        dir.exists(),
        "Demo audio directory does not exist: {:?}",
        dir
    );

    let wav_files = all_wav_files();
    assert!(
        !wav_files.is_empty(),
        "No WAV files found in demo audio directory: {:?}",
        dir
    );
}

/// Copy demo files to a temporary directory for testing
pub fn copy_demo_files_to_temp(files: &[&str]) -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let demo_dir = demo_audio_dir();

    for file_name in files {
        let src = demo_dir.join(file_name);
        let dst = temp_dir.path().join(file_name);
        std::fs::copy(&src, &dst).unwrap_or_else(|e| {
            panic!("Failed to copy {} to temp dir: {}", file_name, e);
        });
    }

    temp_dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_audio_dir_exists() {
        let dir = demo_audio_dir();
        assert!(dir.exists(), "Demo audio directory should exist");
    }

    #[test]
    fn test_all_wav_files_returns_files() {
        let files = all_wav_files();
        assert!(!files.is_empty(), "Should find WAV files");
        for file in &files {
            assert!(file.exists(), "WAV file should exist: {:?}", file);
            assert_eq!(file.extension().unwrap(), "wav");
        }
    }

    #[test]
    fn test_get_demo_file() {
        let file = get_demo_file("classical.wav");
        assert!(file.ends_with("demo-audio/classical.wav"));
    }

    #[test]
    fn test_temp_database() {
        let (_temp_dir, db_path) = temp_database();
        assert!(db_path.ends_with("test_music.db"));
        assert!(!db_path.exists(), "Database should not exist yet");
    }

    #[test]
    fn test_copy_demo_files_to_temp() {
        let temp_dir = copy_demo_files_to_temp(&["classical.wav", "jazz.wav"]);
        let classical = temp_dir.path().join("classical.wav");
        let jazz = temp_dir.path().join("jazz.wav");

        assert!(classical.exists(), "classical.wav should be copied");
        assert!(jazz.exists(), "jazz.wav should be copied");
    }
}
