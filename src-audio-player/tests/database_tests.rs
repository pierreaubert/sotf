/// Integration tests for MusicDatabase
use sotf_audio_player::database::MusicDatabase;
use sotf_audio_player::{Album, Track};
use std::path::PathBuf;

mod fixtures;

#[test]
fn test_database_creation() {
    let (_temp_dir, db_path) = fixtures::temp_database();

    // Create database
    let db = MusicDatabase::open(&db_path).expect("Failed to open database");

    // Database file should now exist
    assert!(db_path.exists(), "Database file should be created");
}

#[test]
fn test_save_and_load_empty_library() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    // Save empty library
    db.save_albums(&[]).expect("Failed to save empty library");

    // Load library
    let albums = db.load_library().expect("Failed to load library");
    assert_eq!(albums.len(), 0, "Empty library should have no albums");
}

#[test]
fn test_save_and_load_single_album() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    // Create test album
    let demo_file = fixtures::get_demo_file("classical.wav");
    let album = Album {
        id: None,
        artist: "Test Artist".to_string(),
        title: "Test Album".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: demo_file.clone(),
            title: Some("Test Track".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
        }],
        album_art_path: None,
    };

    // Save album
    db.save_albums(&[album]).expect("Failed to save album");

    // Load library
    let loaded = db.load_library().expect("Failed to load library");
    assert_eq!(loaded.len(), 1, "Should have 1 album");

    let loaded_album = &loaded[0];
    assert_eq!(loaded_album.artist, "Test Artist");
    assert_eq!(loaded_album.title, "Test Album");
    assert_eq!(loaded_album.year, Some(2024));
    assert_eq!(loaded_album.tracks.len(), 1);

    let track = &loaded_album.tracks[0];
    assert_eq!(track.path, demo_file);
    assert_eq!(track.title, Some("Test Track".to_string()));
    assert_eq!(track.track_number, Some(1));
    assert_eq!(track.channels, Some(2));
}

#[test]
fn test_save_multiple_albums() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    // Create multiple albums
    let albums = vec![
        Album {
            id: None,
            artist: "Artist 1".to_string(),
            title: "Album 1".to_string(),
            year: Some(2020),
            tracks: vec![Track {
                path: fixtures::get_demo_file("classical.wav"),
                title: Some("Track 1".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            }],
            album_art_path: None,
        },
        Album {
            id: None,
            artist: "Artist 2".to_string(),
            title: "Album 2".to_string(),
            year: Some(2021),
            tracks: vec![Track {
                path: fixtures::get_demo_file("jazz.wav"),
                title: Some("Track 2".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            }],
            album_art_path: None,
        },
    ];

    // Save albums
    db.save_albums(&albums).expect("Failed to save albums");

    // Load library
    let loaded = db.load_library().expect("Failed to load library");
    assert_eq!(loaded.len(), 2, "Should have 2 albums");

    // Verify both albums are present (order may vary)
    let artists: Vec<_> = loaded.iter().map(|a| a.artist.as_str()).collect();
    assert!(artists.contains(&"Artist 1"));
    assert!(artists.contains(&"Artist 2"));
}

#[test]
fn test_album_with_multiple_tracks() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    // Create album with multiple tracks
    let album = Album {
        id: None,
        artist: "Multi Track Artist".to_string(),
        title: "Multi Track Album".to_string(),
        year: Some(2023),
        tracks: vec![
            Track {
                path: fixtures::get_demo_file("classical.wav"),
                title: Some("Track 1".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            },
            Track {
                path: fixtures::get_demo_file("jazz.wav"),
                title: Some("Track 2".to_string()),
                track_number: Some(2),
                duration_secs: Some(5),
                channels: Some(2),
            },
            Track {
                path: fixtures::get_demo_file("rock.wav"),
                title: Some("Track 3".to_string()),
                track_number: Some(3),
                duration_secs: Some(5),
                channels: Some(2),
            },
        ],
        album_art_path: None,
    };

    // Save album
    db.save_albums(&[album]).expect("Failed to save album");

    // Load library
    let loaded = db.load_library().expect("Failed to load library");
    assert_eq!(loaded.len(), 1, "Should have 1 album");

    let loaded_album = &loaded[0];
    assert_eq!(loaded_album.tracks.len(), 3, "Should have 3 tracks");

    // Verify tracks are ordered by track number
    for (i, track) in loaded_album.tracks.iter().enumerate() {
        assert_eq!(track.track_number, Some((i + 1) as u32));
    }
}

#[test]
fn test_search_library_empty() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let db = MusicDatabase::open(&db_path).unwrap();

    let results = db.search_library("test").expect("Failed to search library");
    assert_eq!(results.len(), 0, "Empty library should return no results");
}

#[test]
fn test_search_library_by_artist() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    // Create albums with different artists
    let albums = vec![
        Album {
            id: None,
            artist: "Pink Floyd".to_string(),
            title: "The Wall".to_string(),
            year: Some(1979),
            tracks: vec![Track {
                path: fixtures::get_demo_file("rock.wav"),
                title: Some("Another Brick in the Wall".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            }],
            album_art_path: None,
        },
        Album {
            id: None,
            artist: "Miles Davis".to_string(),
            title: "Kind of Blue".to_string(),
            year: Some(1959),
            tracks: vec![Track {
                path: fixtures::get_demo_file("jazz.wav"),
                title: Some("So What".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            }],
            album_art_path: None,
        },
    ];

    db.save_albums(&albums).expect("Failed to save albums");

    // Search for Pink Floyd
    let results = db.search_library("Pink").expect("Failed to search library");
    assert_eq!(results.len(), 1, "Should find 1 album");

    // Verify we found the right album
    let all_albums = db.load_library().unwrap();
    let found_album = all_albums
        .iter()
        .find(|a| a.id == Some(results[0]))
        .unwrap();
    assert_eq!(found_album.artist, "Pink Floyd");
}

#[test]
fn test_search_library_by_album_title() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    let album = Album {
        id: None,
        artist: "Test Artist".to_string(),
        title: "Dark Side of the Moon".to_string(),
        year: Some(1973),
        tracks: vec![Track {
            path: fixtures::get_demo_file("rock.wav"),
            title: Some("Time".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
        }],
        album_art_path: None,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    // Search for album title
    let results = db.search_library("Moon").expect("Failed to search library");
    assert_eq!(results.len(), 1, "Should find 1 album");
}

#[test]
fn test_search_library_by_track_title() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    let album = Album {
        id: None,
        artist: "Test Artist".to_string(),
        title: "Test Album".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("classical.wav"),
            title: Some("Bohemian Rhapsody".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
        }],
        album_art_path: None,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    // Search for track title
    let results = db
        .search_library("Rhapsody")
        .expect("Failed to search library");
    assert_eq!(results.len(), 1, "Should find 1 album");
}

#[test]
fn test_search_library_case_insensitive() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    let album = Album {
        id: None,
        artist: "The Beatles".to_string(),
        title: "Abbey Road".to_string(),
        year: Some(1969),
        tracks: vec![Track {
            path: fixtures::get_demo_file("rock.wav"),
            title: Some("Come Together".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
        }],
        album_art_path: None,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    // Test various case combinations
    let test_queries = vec!["beatles", "BEATLES", "BeAtLeS", "abbey", "ABBEY"];

    for query in test_queries {
        let results = db.search_library(query).expect("Failed to search library");
        assert_eq!(
            results.len(),
            1,
            "Search should be case-insensitive for query: {}",
            query
        );
    }
}

#[test]
fn test_search_library_prefix_matching() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    let album = Album {
        id: None,
        artist: "Pink Floyd".to_string(),
        title: "The Wall".to_string(),
        year: Some(1979),
        tracks: vec![Track {
            path: fixtures::get_demo_file("rock.wav"),
            title: Some("Another Brick".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
        }],
        album_art_path: None,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    // Test prefix matching
    let test_queries = vec!["Pin", "Pink", "Pink Flo", "Flo"];

    for query in test_queries {
        let results = db.search_library(query).expect("Failed to search library");
        assert!(
            !results.is_empty(),
            "Prefix search should work for query: {}",
            query
        );
    }
}

#[test]
fn test_clean_missing_files() {
    let temp_dir = fixtures::copy_demo_files_to_temp(&["classical.wav", "jazz.wav"]);
    let (_db_temp, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    // Create albums pointing to temp directory files
    let classical_path = temp_dir.path().join("classical.wav");
    let jazz_path = temp_dir.path().join("jazz.wav");

    let albums = vec![
        Album {
            id: None,
            artist: "Artist 1".to_string(),
            title: "Album 1".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: classical_path.clone(),
                title: Some("Track 1".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            }],
            album_art_path: None,
        },
        Album {
            id: None,
            artist: "Artist 2".to_string(),
            title: "Album 2".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: jazz_path.clone(),
                title: Some("Track 2".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            }],
            album_art_path: None,
        },
    ];

    db.save_albums(&albums).expect("Failed to save albums");

    // Delete one file
    std::fs::remove_file(&jazz_path).expect("Failed to delete test file");

    // Clean missing files
    let removed = db
        .clean_missing_files()
        .expect("Failed to clean missing files");
    assert_eq!(removed, 1, "Should have removed 1 track");

    // Verify library now has only 1 album
    let loaded = db.load_library().expect("Failed to load library");
    assert_eq!(loaded.len(), 1, "Should have 1 album left");
    assert_eq!(loaded[0].artist, "Artist 1");
}

#[test]
fn test_replay_gain_storage() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    let demo_file = fixtures::get_demo_file("classical.wav");
    let album = Album {
        id: None,
        artist: "Test Artist".to_string(),
        title: "Test Album".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: demo_file.clone(),
            title: Some("Test Track".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
        }],
        album_art_path: None,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    // Update replay gain
    db.update_replay_gain(&demo_file, -5.5, 0.95)
        .expect("Failed to update replay gain");

    // Load and verify
    let loaded = db.load_library().expect("Failed to load library");
    assert_eq!(loaded.len(), 1);

    // Note: ReplayGain is not exposed in the Album/Track struct by default
    // This test verifies that the database operation succeeds
    // A more complete test would check the actual values via a database query
}

#[test]
fn test_get_tracks_without_replay_gain() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    // Create albums with and without replay gain
    let file1 = fixtures::get_demo_file("classical.wav");
    let file2 = fixtures::get_demo_file("jazz.wav");

    let albums = vec![
        Album {
            id: None,
            artist: "Artist 1".to_string(),
            title: "Album 1".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: file1.clone(),
                title: Some("Track 1".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            }],
            album_art_path: None,
        },
        Album {
            id: None,
            artist: "Artist 2".to_string(),
            title: "Album 2".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: file2.clone(),
                title: Some("Track 2".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            }],
            album_art_path: None,
        },
    ];

    db.save_albums(&albums).expect("Failed to save albums");

    // Initially, both tracks should need replay gain
    let tracks = db
        .get_tracks_without_replay_gain()
        .expect("Failed to get tracks without replay gain");
    assert_eq!(tracks.len(), 2, "Both tracks should need replay gain");

    // Add replay gain to one track
    db.update_replay_gain(&file1, -5.0, 0.9)
        .expect("Failed to update replay gain");

    // Now only one track should need replay gain
    let tracks = db
        .get_tracks_without_replay_gain()
        .expect("Failed to get tracks without replay gain");
    assert_eq!(tracks.len(), 1, "Only 1 track should need replay gain");
    assert_eq!(tracks[0], file2);
}

#[test]
fn test_record_and_get_scan_history() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let db = MusicDatabase::open(&db_path).unwrap();

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
fn test_update_existing_album() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open(&db_path).unwrap();

    // Create initial album
    let demo_file = fixtures::get_demo_file("classical.wav");
    let album = Album {
        id: None,
        artist: "Original Artist".to_string(),
        title: "Original Title".to_string(),
        year: Some(2020),
        tracks: vec![Track {
            path: demo_file.clone(),
            title: Some("Track 1".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
        }],
        album_art_path: None,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    // Load and get the ID
    let loaded = db.load_library().expect("Failed to load library");
    let album_id = loaded[0].id;

    // Update the album (same artist+title, so it should update existing record)
    let updated_album = Album {
        id: album_id,
        artist: "Original Artist".to_string(),
        title: "Original Title".to_string(),
        year: Some(2024), // Changed year
        tracks: vec![
            Track {
                path: demo_file.clone(),
                title: Some("Updated Track 1".to_string()), // Changed title
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            },
            Track {
                path: fixtures::get_demo_file("jazz.wav"),
                title: Some("New Track 2".to_string()), // Added track
                track_number: Some(2),
                duration_secs: Some(5),
                channels: Some(2),
            },
        ],
        album_art_path: None,
    };

    db.save_albums(&[updated_album])
        .expect("Failed to update album");

    // Verify update
    let loaded = db.load_library().expect("Failed to load library");
    assert_eq!(loaded.len(), 1, "Should still have 1 album");

    let album = &loaded[0];
    assert_eq!(album.year, Some(2024), "Year should be updated");
    assert_eq!(album.tracks.len(), 2, "Should have 2 tracks now");
}
