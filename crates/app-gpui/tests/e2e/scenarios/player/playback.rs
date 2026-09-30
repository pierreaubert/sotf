//! Playback scenarios for E2E testing.
//!
//! Tests for verifying audio playback functionality.

use gpui::TestAppContext;
use std::path::PathBuf;

/// Get the path to the workspace-level test audio directory.
///
/// Resolved from this package's own `CARGO_MANIFEST_DIR`: `sotf-testkit`
/// lives in the `sotf-daw` workspace, so a testkit-relative lookup would
/// point at the wrong workspace.
fn test_audio_dir() -> PathBuf {
    if let Ok(root) = std::env::var("SOTF_TEST_DATA_ROOT") {
        PathBuf::from(root).join("audio")
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("data_tests")
            .join("audio")
    }
}

/// Get the path to a specific test audio file.
fn test_audio_file(name: &str) -> PathBuf {
    test_audio_dir().join(format!("{}.flac", name))
}

/// Test loading a test audio file.
#[gpui::test]
async fn test_test_audio_files_exist(_cx: &mut TestAppContext) {
    let test_files = ["piano", "rock", "classical", "jazz", "edm"];
    for name in test_files {
        let path = test_audio_file(name);
        assert!(path.exists(), "Test audio file '{}' should exist", name);
    }
}

/// Test audio file paths are valid.
#[gpui::test]
async fn test_audio_file_paths(_cx: &mut TestAppContext) {
    let piano = test_audio_file("piano");
    let rock = test_audio_file("rock");

    assert!(piano.exists());
    assert!(rock.exists());

    // Files should have .flac extension
    assert_eq!(piano.extension().unwrap(), "flac");
    assert_eq!(rock.extension().unwrap(), "flac");
}

/// Test that test audio directory exists.
#[gpui::test]
async fn test_test_audio_directory_exists(_cx: &mut TestAppContext) {
    let dir = test_audio_dir();
    assert!(dir.exists(), "Test audio directory should exist");
    assert!(dir.is_dir(), "Test audio path should be a directory");
}
