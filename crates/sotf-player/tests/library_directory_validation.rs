use sotf_audio_player::{LibraryDirectoryAccessError, validate_library_directory};

#[test]
fn distinguishes_common_recoverable_failures() {
    let temp = tempfile::TempDir::new().unwrap();
    assert_eq!(validate_library_directory(temp.path()), Ok(()));

    let missing = temp.path().join("missing");
    assert!(matches!(
        validate_library_directory(&missing),
        Err(LibraryDirectoryAccessError::NotFound(path)) if path == missing
    ));

    let file = temp.path().join("track.wav");
    std::fs::write(&file, b"not audio; validation only checks folder access").unwrap();
    assert!(matches!(
        validate_library_directory(&file),
        Err(LibraryDirectoryAccessError::NotDirectory(path)) if path == file
    ));
}
