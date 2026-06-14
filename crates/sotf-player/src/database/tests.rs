use super::misc::backup_existing_database;
use super::misc::prune_old_backups;
use super::misc::sha256_of_file;
use super::music_database::MusicDatabase;
use std::path::{Path, PathBuf};

fn fresh_config_test_dir(name: &str) -> std::path::PathBuf {
    let test_dir = crate::config::test_config_dir().join(name);
    std::fs::remove_dir_all(&test_dir).ok();
    std::fs::create_dir_all(&test_dir).unwrap();
    test_dir
}

#[test]
fn test_backup_existing_database_creates_backup_file() {
    let test_dir = fresh_config_test_dir("test_backup");

    let db_path = test_dir.join("music.db");
    std::fs::write(&db_path, b"test").unwrap();

    backup_existing_database(&db_path).unwrap();

    let mut backups = Vec::new();
    for entry in std::fs::read_dir(&test_dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.starts_with("music-") && name.ends_with(".sqlite") {
            backups.push(name);
        }
    }

    assert_eq!(backups.len(), 1);

    // Cleanup
    std::fs::remove_dir_all(&test_dir).ok();
}

#[test]
fn test_prune_old_backups_limits_to_three_per_day() {
    let test_dir = fresh_config_test_dir("test_prune");

    // Create 5 backups for the same day
    let ts = [
        "20250101-010000",
        "20250101-020000",
        "20250101-030000",
        "20250101-040000",
        "20250101-050000",
    ];

    for t in &ts {
        let path = test_dir.join(format!("music-{}.sqlite", t));
        std::fs::write(path, b"test").unwrap();
    }

    prune_old_backups(&test_dir).unwrap();

    let mut remaining = Vec::new();
    for entry in std::fs::read_dir(&test_dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.starts_with("music-") && name.ends_with(".sqlite") {
            remaining.push(name);
        }
    }

    // Only 3 backups for that day should remain
    assert_eq!(remaining.len(), 3);

    // Cleanup
    std::fs::remove_dir_all(&test_dir).ok();
}

#[test]
fn test_sha256_of_file() {
    let dir = std::env::temp_dir().join("sotf_test_sha256");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.bin");
    std::fs::write(&path, b"hello world").unwrap();

    let hash = sha256_of_file(&path).unwrap();
    // SHA-256 of "hello world" is
    // b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
    assert_eq!(
        hash,
        [
            0xb9, 0x4d, 0x27, 0xb9, 0x93, 0x4d, 0x3e, 0x08, 0xa5, 0x2e, 0x52, 0xd7, 0xda, 0x7d,
            0xab, 0xfa, 0xc4, 0x84, 0xef, 0xe3, 0x7a, 0x53, 0x80, 0xee, 0x90, 0x88, 0xf7, 0xac,
            0xe2, 0xef, 0xcd, 0xe9
        ]
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_backup_skips_duplicate() {
    let test_dir = fresh_config_test_dir("test_backup_dedup");

    let db_path = test_dir.join("music.db");
    std::fs::write(&db_path, b"unchanged content").unwrap();

    // First backup should create a file
    backup_existing_database(&db_path).unwrap();
    let count_after_first = count_backups(&test_dir);
    assert_eq!(count_after_first, 1);

    // Second backup with same content should NOT create another file
    // Sleep 1 second so timestamp would differ
    std::thread::sleep(std::time::Duration::from_secs(1));
    backup_existing_database(&db_path).unwrap();
    let count_after_second = count_backups(&test_dir);
    assert_eq!(count_after_second, 1, "duplicate backup should be skipped");

    // Modify the database — next backup SHOULD create a new file
    std::fs::write(&db_path, b"modified content").unwrap();
    std::thread::sleep(std::time::Duration::from_secs(1));
    backup_existing_database(&db_path).unwrap();
    let count_after_third = count_backups(&test_dir);
    assert_eq!(
        count_after_third, 2,
        "changed database should produce a new backup"
    );

    // Cleanup
    std::fs::remove_dir_all(&test_dir).ok();
}

#[test]
fn test_remove_directory_cleans_up_albums() {
    let dir = std::env::temp_dir().join("sotf_test_remove_dir_cleanup");
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.sqlite");

    // Clean up any previous run
    let _ = std::fs::remove_file(&db_path);

    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Create an album with one track in /music/dir1
    let albums = vec![crate::library::Album {
        title: "Test Album".to_string(),
        tracks: vec![crate::library::Track {
            path: PathBuf::from("/music/dir1/track1.flac"),
            title: Some("Track 1".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    }];

    db.save_albums(&albums).unwrap();

    // Verify album exists
    let loaded = db.load_library().unwrap();
    assert_eq!(loaded.len(), 1, "should have 1 album after save");
    assert_eq!(loaded[0].tracks.len(), 1);

    // Remove the directory
    let removed = db
        .remove_tracks_from_directory(Path::new("/music/dir1"))
        .unwrap();
    assert_eq!(removed, 1, "should have removed 1 track");

    // Verify database is empty
    let loaded = db.load_library().unwrap();
    assert_eq!(
        loaded.len(),
        0,
        "should have 0 albums after removing the only directory"
    );

    // Cleanup
    std::fs::remove_dir_all(&dir).ok();
}

fn count_backups(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().into_string().unwrap_or_default();
            name.starts_with("music-") && name.ends_with(".sqlite")
        })
        .count()
}
