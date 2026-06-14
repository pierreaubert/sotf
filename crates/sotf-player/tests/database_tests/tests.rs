/// Integration tests for MusicDatabase
use sotf_audio_player::database::MusicDatabase;
use std::path::PathBuf;

#[test]
fn test_database_creation() {
    let (_temp_dir, db_path) = super::fixtures::temp_database();

    // Create database
    let db = MusicDatabase::open_for_testing(&db_path).expect("Failed to open database");

    // Database file should now exist
    assert!(db_path.exists(), "Database file should be created");
    drop(db);
}

#[test]
fn test_save_and_load_empty_library() {
    let (_temp_dir, db_path) = super::fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Save empty library
    db.save_albums(&[]).expect("Failed to save empty library");

    // Load library
    let albums = db.load_library().expect("Failed to load library");
    assert_eq!(albums.len(), 0, "Empty library should have no albums");
}

#[test]
fn test_search_library_empty() {
    let (_temp_dir, db_path) = super::fixtures::temp_database();
    let db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let results = db.search_library("test").expect("Failed to search library");
    assert_eq!(results.len(), 0, "Empty library should return no results");
}

#[test]
fn test_record_and_get_scan_history() {
    let (_temp_dir, db_path) = super::fixtures::temp_database();
    let db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let scan_dir = PathBuf::from("/music/library");

    // Record a scan
    db.record_scan(&scan_dir, 10, 2)
        .expect("Failed to record scan");

    // Get scan history
    let history = db
        .get_scanned_directories()
        .expect("Failed to get scan history");
    assert_eq!(history.len(), 1, "Should have 1 scan record");

    let (dir, tracks, albums, _timestamp) = &history[0];
    assert_eq!(dir, &scan_dir);
    assert_eq!(*tracks, 10);
    assert_eq!(*albums, 2);
}

#[test]
fn test_federation_migrations_in_history() {
    let (_temp_dir, db_path) = super::fixtures::temp_database();
    let db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let history = db.get_migration_history().expect("Failed to get history");
    let versions: Vec<i64> = history.iter().map(|(v, _, _)| *v).collect();
    assert!(
        versions.contains(&19),
        "Migration 19 should be in history, got: {:?}",
        versions
    );
    assert!(
        versions.contains(&20),
        "Migration 20 should be in history, got: {:?}",
        versions
    );
}
