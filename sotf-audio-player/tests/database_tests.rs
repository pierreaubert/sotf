/// Integration tests for MusicDatabase
use sotf_audio_player::database::MusicDatabase;
use sotf_audio_player::{Album, Track};
use std::path::PathBuf;

mod fixtures;

#[test]
fn test_database_creation() {
    let (_temp_dir, db_path) = fixtures::temp_database();

    // Create database
    let db = MusicDatabase::open_for_testing(&db_path).expect("Failed to open database");

    // Database file should now exist
    assert!(db_path.exists(), "Database file should be created");
}

#[test]
fn test_save_and_load_empty_library() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Save empty library
    db.save_albums(&[]).expect("Failed to save empty library");

    // Load library
    let albums = db.load_library().expect("Failed to load library");
    assert_eq!(albums.len(), 0, "Empty library should have no albums");
}

#[test]
fn test_save_and_load_single_album() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
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
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
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
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
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
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            },
            Track {
                path: fixtures::get_demo_file("jazz.wav"),
                title: Some("Track 2".to_string()),
                track_number: Some(2),
                duration_secs: Some(5),
                channels: Some(2),
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            },
            Track {
                path: fixtures::get_demo_file("rock.wav"),
                title: Some("Track 3".to_string()),
                track_number: Some(3),
                duration_secs: Some(5),
                channels: Some(2),
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            },
        ],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
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
    let db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let results = db.search_library("test").expect("Failed to search library");
    assert_eq!(results.len(), 0, "Empty library should return no results");
}

#[test]
fn test_search_library_by_artist() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
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
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
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
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    // Search for album title
    let results = db.search_library("Moon").expect("Failed to search library");
    assert_eq!(results.len(), 1, "Should find 1 album");
}

#[test]
fn test_search_library_by_track_title() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
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
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
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
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
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
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
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
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
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
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
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
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
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
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
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
fn test_update_existing_album() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

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
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
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
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            },
            Track {
                path: fixtures::get_demo_file("jazz.wav"),
                title: Some("New Track 2".to_string()), // Added track
                track_number: Some(2),
                duration_secs: Some(5),
                channels: Some(2),
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            },
        ],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
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

// ==================== Normalized Metadata Table Tests ====================

#[test]
fn test_normalized_genre_table() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Create album with genre
    let album = Album {
        id: None,
        artist: "Classical Artist".to_string(),
        title: "Classical Album".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("classical.wav"),
            title: Some("Symphony No. 1".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: Some("Classical".to_string()),
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    // Verify genre was added to normalized table
    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 1, "Should have 1 genre");
    assert_eq!(genres[0].1, "Classical");

    // Verify track-genre relationship
    let track_path = fixtures::get_demo_file("classical.wav");
    let track_genre = db.get_track_genre(&track_path).expect("Failed to get track genre");
    assert_eq!(track_genre, Some("Classical".to_string()));
}

#[test]
fn test_normalized_composer_table() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Create album with composer
    let album = Album {
        id: None,
        artist: "Orchestra".to_string(),
        title: "Beethoven Symphonies".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("classical.wav"),
            title: Some("Symphony No. 5".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: Some("Ludwig van Beethoven".to_string()),
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    // Verify composer was added to normalized table
    let composers = db.get_all_composers().expect("Failed to get composers");
    assert_eq!(composers.len(), 1, "Should have 1 composer");
    assert_eq!(composers[0].1, "Ludwig van Beethoven");

    // Verify tracks by composer
    let composer_id = composers[0].0;
    let tracks = db.get_tracks_by_composer(composer_id).expect("Failed to get tracks by composer");
    assert_eq!(tracks.len(), 1);
}

#[test]
fn test_normalized_conductor_table() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let album = Album {
        id: None,
        artist: "Berlin Philharmonic".to_string(),
        title: "Mahler Symphonies".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("classical.wav"),
            title: Some("Symphony No. 2".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: Some("Herbert von Karajan".to_string()),
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    let conductors = db.get_all_conductors().expect("Failed to get conductors");
    assert_eq!(conductors.len(), 1);
    assert_eq!(conductors[0].1, "Herbert von Karajan");

    let tracks = db.get_tracks_by_conductor(conductors[0].0).expect("Failed to get tracks");
    assert_eq!(tracks.len(), 1);
}

#[test]
fn test_normalized_performer_table() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let album = Album {
        id: None,
        artist: "Jazz Album".to_string(),
        title: "Kind of Blue".to_string(),
        year: Some(1959),
        tracks: vec![Track {
            path: fixtures::get_demo_file("jazz.wav"),
            title: Some("So What".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: None,
            performer: Some("Miles Davis".to_string()),
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    let performers = db.get_all_performers().expect("Failed to get performers");
    assert_eq!(performers.len(), 1);
    assert_eq!(performers[0].1, "Miles Davis");

    let tracks = db.get_tracks_by_performer(performers[0].0).expect("Failed to get tracks");
    assert_eq!(tracks.len(), 1);
}

#[test]
fn test_normalized_ensemble_table() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let album = Album {
        id: None,
        artist: "Chamber Music".to_string(),
        title: "String Quartets".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("classical.wav"),
            title: Some("Quartet Op. 18".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: Some("Emerson String Quartet".to_string()),
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    let ensembles = db.get_all_ensembles().expect("Failed to get ensembles");
    assert_eq!(ensembles.len(), 1);
    assert_eq!(ensembles[0].1, "Emerson String Quartet");

    let tracks = db.get_tracks_by_ensemble(ensembles[0].0).expect("Failed to get tracks");
    assert_eq!(tracks.len(), 1);
}

#[test]
fn test_normalized_tables_multiple_tracks_same_metadata() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Two albums with the same genre
    let albums = vec![
        Album {
            id: None,
            artist: "Rock Artist 1".to_string(),
            title: "Rock Album 1".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: fixtures::get_demo_file("rock.wav"),
                title: Some("Rock Song 1".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: Some("Rock".to_string()),
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
        },
        Album {
            id: None,
            artist: "Rock Artist 2".to_string(),
            title: "Rock Album 2".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: fixtures::get_demo_file("jazz.wav"),
                title: Some("Rock Song 2".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: Some("Rock".to_string()),
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
        },
    ];

    db.save_albums(&albums).expect("Failed to save albums");

    // Should only have one genre entry (normalized)
    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 1, "Should have exactly 1 genre entry");
    assert_eq!(genres[0].1, "Rock");

    // Both tracks should be linked to this genre
    let genre_id = genres[0].0;
    let tracks = db.get_tracks_by_genre(genre_id).expect("Failed to get tracks by genre");
    assert_eq!(tracks.len(), 2, "Both tracks should be linked to the Rock genre");

    // Get albums by genre
    let album_ids = db.get_albums_by_genre(genre_id).expect("Failed to get albums by genre");
    assert_eq!(album_ids.len(), 2, "Both albums should be in Rock genre");
}

#[test]
fn test_normalized_tables_case_insensitive() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Create album with genre "Rock"
    let album1 = Album {
        id: None,
        artist: "Artist 1".to_string(),
        title: "Album 1".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("rock.wav"),
            title: Some("Song 1".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: Some("Rock".to_string()),
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album1]).expect("Failed to save album 1");

    // Create album with genre "rock" (lowercase)
    let album2 = Album {
        id: None,
        artist: "Artist 2".to_string(),
        title: "Album 2".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("jazz.wav"),
            title: Some("Song 2".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: Some("rock".to_string()),
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album2]).expect("Failed to save album 2");

    // Should have only one genre entry due to COLLATE NOCASE
    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 1, "Case-insensitive: should have 1 genre entry");
}

#[test]
fn test_normalized_tables_track_with_all_metadata() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Create album with all metadata fields
    let album = Album {
        id: None,
        artist: "Vienna Philharmonic".to_string(),
        title: "Complete Beethoven".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("classical.wav"),
            title: Some("Symphony No. 9".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: Some("Classical".to_string()),
            composer: Some("Ludwig van Beethoven".to_string()),
            disc_number: Some(1),
            conductor: Some("Herbert von Karajan".to_string()),
            performer: Some("Jessye Norman".to_string()),
            isrc: None,
            album_artist: Some("Vienna Philharmonic".to_string()),
            ensemble: Some("Vienna State Opera Chorus".to_string()),
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    // Verify all normalized tables have entries
    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 1);
    assert_eq!(genres[0].1, "Classical");

    let composers = db.get_all_composers().expect("Failed to get composers");
    assert_eq!(composers.len(), 1);
    assert_eq!(composers[0].1, "Ludwig van Beethoven");

    let conductors = db.get_all_conductors().expect("Failed to get conductors");
    assert_eq!(conductors.len(), 1);
    assert_eq!(conductors[0].1, "Herbert von Karajan");

    let performers = db.get_all_performers().expect("Failed to get performers");
    assert_eq!(performers.len(), 1);
    assert_eq!(performers[0].1, "Jessye Norman");

    let ensembles = db.get_all_ensembles().expect("Failed to get ensembles");
    assert_eq!(ensembles.len(), 1);
    assert_eq!(ensembles[0].1, "Vienna State Opera Chorus");
}

#[test]
fn test_albums_by_composer() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Create two albums with tracks by the same composer
    let albums = vec![
        Album {
            id: None,
            artist: "Orchestra 1".to_string(),
            title: "Mozart Vol 1".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: fixtures::get_demo_file("classical.wav"),
                title: Some("Symphony No. 40".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: Some("Wolfgang Amadeus Mozart".to_string()),
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
        },
        Album {
            id: None,
            artist: "Orchestra 2".to_string(),
            title: "Mozart Vol 2".to_string(),
            year: Some(2024),
            tracks: vec![Track {
                path: fixtures::get_demo_file("jazz.wav"),
                title: Some("Symphony No. 41".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: Some("Wolfgang Amadeus Mozart".to_string()),
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
        },
    ];

    db.save_albums(&albums).expect("Failed to save albums");

    // Get the composer
    let composers = db.get_all_composers().expect("Failed to get composers");
    assert_eq!(composers.len(), 1);

    // Get albums by composer
    let album_ids = db.get_albums_by_composer(composers[0].0).expect("Failed to get albums");
    assert_eq!(album_ids.len(), 2, "Should find 2 albums by Mozart");
}

#[test]
fn test_genre_splitting_by_comma() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Track with multiple genres separated by comma
    let album = Album {
        id: None,
        artist: "Multi-Genre Artist".to_string(),
        title: "Multi-Genre Album".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("rock.wav"),
            title: Some("Multi-Genre Song".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: Some("Rock, Pop, Electronic".to_string()),
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    // Should have 3 separate genre entries
    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 3, "Should have 3 genres from comma-separated list");

    let genre_names: Vec<&str> = genres.iter().map(|(_, name)| name.as_str()).collect();
    assert!(genre_names.contains(&"Rock"));
    assert!(genre_names.contains(&"Pop"));
    assert!(genre_names.contains(&"Electronic"));

    // Track should be linked to all 3 genres
    let track_path = fixtures::get_demo_file("rock.wav");
    let track_genres = db.get_track_genres(&track_path).expect("Failed to get track genres");
    assert_eq!(track_genres.len(), 3, "Track should be linked to 3 genres");
}

#[test]
fn test_genre_splitting_by_slash() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Track with genres separated by slash
    let album = Album {
        id: None,
        artist: "Jazz Artist".to_string(),
        title: "Jazz Album".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("jazz.wav"),
            title: Some("Jazz Song".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: Some("Jazz / Blues / Soul".to_string()),
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 3, "Should have 3 genres from slash-separated list");

    let genre_names: Vec<&str> = genres.iter().map(|(_, name)| name.as_str()).collect();
    assert!(genre_names.contains(&"Jazz"));
    assert!(genre_names.contains(&"Blues"));
    assert!(genre_names.contains(&"Soul"));
}

#[test]
fn test_genre_splitting_by_semicolon() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Track with genres separated by semicolon
    let album = Album {
        id: None,
        artist: "Classical Artist".to_string(),
        title: "Classical Album".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("classical.wav"),
            title: Some("Classical Song".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: Some("Classical; Baroque; Chamber Music".to_string()),
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 3, "Should have 3 genres from semicolon-separated list");

    let genre_names: Vec<&str> = genres.iter().map(|(_, name)| name.as_str()).collect();
    assert!(genre_names.contains(&"Classical"));
    assert!(genre_names.contains(&"Baroque"));
    assert!(genre_names.contains(&"Chamber Music"));
}

#[test]
fn test_composer_splitting_multiple_values() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Track with multiple composers (common for collaborations)
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
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: Some("Lennon/McCartney".to_string()),
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    let composers = db.get_all_composers().expect("Failed to get composers");
    assert_eq!(composers.len(), 2, "Should have 2 composers from slash-separated value");

    let composer_names: Vec<&str> = composers.iter().map(|(_, name)| name.as_str()).collect();
    assert!(composer_names.contains(&"Lennon"));
    assert!(composer_names.contains(&"McCartney"));
}

#[test]
fn test_performer_splitting_multiple_values() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Track with multiple performers
    let album = Album {
        id: None,
        artist: "Jazz Collaboration".to_string(),
        title: "Blue Train".to_string(),
        year: Some(1958),
        tracks: vec![Track {
            path: fixtures::get_demo_file("jazz.wav"),
            title: Some("Blue Train".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: None,
            composer: None,
            disc_number: None,
            conductor: None,
            performer: Some("John Coltrane, Lee Morgan, Curtis Fuller".to_string()),
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    let performers = db.get_all_performers().expect("Failed to get performers");
    assert_eq!(performers.len(), 3, "Should have 3 performers from comma-separated value");

    let performer_names: Vec<&str> = performers.iter().map(|(_, name)| name.as_str()).collect();
    assert!(performer_names.contains(&"John Coltrane"));
    assert!(performer_names.contains(&"Lee Morgan"));
    assert!(performer_names.contains(&"Curtis Fuller"));
}

#[test]
fn test_mixed_delimiter_splitting() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Track with mixed delimiters in genre
    let album = Album {
        id: None,
        artist: "Mixed Artist".to_string(),
        title: "Mixed Album".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("rock.wav"),
            title: Some("Mixed Song".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: Some("Rock/Metal, Punk; Hardcore".to_string()),
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 4, "Should have 4 genres from mixed-delimiter list");

    let genre_names: Vec<&str> = genres.iter().map(|(_, name)| name.as_str()).collect();
    assert!(genre_names.contains(&"Rock"));
    assert!(genre_names.contains(&"Metal"));
    assert!(genre_names.contains(&"Punk"));
    assert!(genre_names.contains(&"Hardcore"));
}

#[test]
fn test_genre_normalization_dots_and_underscores() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Track with genres using dots and underscores
    let album = Album {
        id: None,
        artist: "World Music Artist".to_string(),
        title: "World Music Album".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("rock.wav"),
            title: Some("World Song".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: Some("world.music, trip_hop, drum_and_bass".to_string()),
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 3, "Should have 3 normalized genres");

    let genre_names: Vec<&str> = genres.iter().map(|(_, name)| name.as_str()).collect();
    // "world.music" -> "World Music"
    assert!(genre_names.contains(&"World Music"), "world.music should become World Music");
    // "trip_hop" -> "Trip Hop"
    assert!(genre_names.contains(&"Trip Hop"), "trip_hop should become Trip Hop");
    // "drum_and_bass" -> "Drum And Bass"
    assert!(genre_names.contains(&"Drum And Bass"), "drum_and_bass should become Drum And Bass");
}

#[test]
fn test_genre_normalization_title_case() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Track with various capitalization styles
    let album = Album {
        id: None,
        artist: "Test Artist".to_string(),
        title: "Test Album".to_string(),
        year: Some(2024),
        tracks: vec![Track {
            path: fixtures::get_demo_file("rock.wav"),
            title: Some("Test Song".to_string()),
            track_number: Some(1),
            duration_secs: Some(5),
            channels: Some(2),
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: Some("ROCK, hip hop, ELECTRONIC MUSIC".to_string()),
            composer: None,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: None,
            ensemble: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
    };

    db.save_albums(&[album]).expect("Failed to save album");

    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 3, "Should have 3 title-cased genres");

    let genre_names: Vec<&str> = genres.iter().map(|(_, name)| name.as_str()).collect();
    // All should be title cased
    assert!(genre_names.contains(&"Rock"), "ROCK should become Rock");
    assert!(genre_names.contains(&"Hip Hop"), "hip hop should become Hip Hop");
    assert!(genre_names.contains(&"Electronic Music"), "ELECTRONIC MUSIC should become Electronic Music");
}
