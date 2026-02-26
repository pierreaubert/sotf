use sotf_audio_player::MusicLibrary;
use std::fs;

mod fixtures;

#[test]
fn test_scan_corrupted_audio_file() {
    let (_temp_db_dir, db_path) = fixtures::temp_database();
    let music_dir = tempfile::TempDir::new().unwrap();
    let music_path = music_dir.path();

    // Create a "corrupted" audio file (just random data with a music extension)
    let corrupted_file = music_path.join("corrupted.mp3");
    fs::write(&corrupted_file, b"this is not a real mp3 file").unwrap();

    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library.add_directory(music_path.to_path_buf()).unwrap();

    // Scan should not crash and should gracefully skip the file
    library
        .scan()
        .expect("Scan should handle corrupted files gracefully");

    // Should have no albums since the only file was corrupted
    assert_eq!(library.albums.len(), 0, "Corrupted file should be skipped");
}

#[test]
fn test_scan_mixed_valid_and_corrupted() {
    fixtures::ensure_demo_files_exist();
    let (_temp_db_dir, db_path) = fixtures::temp_database();
    let music_dir = tempfile::TempDir::new().unwrap();
    let music_path = music_dir.path();

    // Valid file
    let demo_file = fixtures::get_demo_file("rock.wav");
    fs::copy(&demo_file, music_path.join("rock.wav")).unwrap();

    // Corrupted file
    let corrupted_file = music_path.join("bad.flac");
    fs::write(&corrupted_file, b"NOT A FLAC").unwrap();

    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library.add_directory(music_path.to_path_buf()).unwrap();

    library
        .scan()
        .expect("Scan should handle mixed valid/corrupted files");

    // Should have found 1 album (from the valid file)
    assert_eq!(library.albums.len(), 1, "Should have found the valid album");
}
