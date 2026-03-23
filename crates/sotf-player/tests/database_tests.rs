/// Integration tests for MusicDatabase
use sotf_audio_player::database::MusicDatabase;
use sotf_audio_player::{Album, Track};
use std::path::PathBuf;

mod fixtures;

/// Helper function to create a test track with minimal fields
fn test_track(path: PathBuf, title: &str, artist: &str) -> Track {
    Track {
        path,
        title: Some(title.to_string()),
        artist: Some(artist.to_string()),
        track_number: Some(1),
        duration_secs: Some(5),
        channels: Some(2),
        sample_rate: Some(44100),
        bit_depth: Some(16),
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
        edition: None,
        is_favorite: false,
        play_count: 0,
        source: None,
        uuid: None,
    }
}

/// Helper function to create a test track with extended metadata
fn test_track_with_metadata(
    path: PathBuf,
    title: &str,
    artist: &str,
    track_number: u32,
    genre: Option<&str>,
    composer: Option<&str>,
    conductor: Option<&str>,
    performer: Option<&str>,
    ensemble: Option<&str>,
    album_artist: Option<&str>,
    disc_number: Option<u32>,
) -> Track {
    Track {
        path,
        title: Some(title.to_string()),
        artist: Some(artist.to_string()),
        track_number: Some(track_number),
        duration_secs: Some(5),
        channels: Some(2),
        sample_rate: Some(44100),
        bit_depth: Some(16),
        replay_gain: None,
        replay_peak: None,
        album_gain: None,
        album_peak: None,
        waveform: None,
        genre: genre.map(|s| s.to_string()),
        composer: composer.map(|s| s.to_string()),
        disc_number,
        conductor: conductor.map(|s| s.to_string()),
        performer: performer.map(|s| s.to_string()),
        isrc: None,
        album_artist: album_artist.map(|s| s.to_string()),
        ensemble: ensemble.map(|s| s.to_string()),
        edition: None,
        is_favorite: false,
        play_count: 0,
        source: None,
        uuid: None,
    }
}

/// Helper function to create a test album
fn test_album(title: &str, year: Option<u32>, tracks: Vec<Track>) -> Album {
    Album {
        id: None,
        title: title.to_string(),
        year,
        tracks,
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
        is_favorite: false,
        uuid: None,
    }
}

#[test]
fn test_database_creation() {
    let (_temp_dir, db_path) = fixtures::temp_database();

    // Create database
    let db = MusicDatabase::open_for_testing(&db_path).expect("Failed to open database");

    // Database file should now exist
    assert!(db_path.exists(), "Database file should be created");
    drop(db);
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
    let album = test_album(
        "Test Album",
        Some(2024),
        vec![test_track(demo_file.clone(), "Test Track", "Test Artist")],
    );

    // Save album
    db.save_albums(&[album]).expect("Failed to save album");

    // Load library
    let loaded = db.load_library().expect("Failed to load library");
    assert_eq!(loaded.len(), 1, "Should have 1 album");

    let loaded_album = &loaded[0];
    assert_eq!(loaded_album.artist(), "Test Artist");
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
        test_album(
            "Album 1",
            Some(2020),
            vec![test_track(
                fixtures::get_demo_file("classical.wav"),
                "Track 1",
                "Artist 1",
            )],
        ),
        test_album(
            "Album 2",
            Some(2021),
            vec![test_track(
                fixtures::get_demo_file("jazz.wav"),
                "Track 2",
                "Artist 2",
            )],
        ),
    ];

    // Save albums
    db.save_albums(&albums).expect("Failed to save albums");

    // Load library
    let loaded = db.load_library().expect("Failed to load library");
    assert_eq!(loaded.len(), 2, "Should have 2 albums");

    // Verify both albums are present (order may vary)
    let artists: Vec<_> = loaded.iter().map(|a| a.artist()).collect();
    assert!(artists.contains(&"Artist 1".to_string()));
    assert!(artists.contains(&"Artist 2".to_string()));
}

#[test]
fn test_album_with_multiple_tracks() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Create album with multiple tracks
    let mut track1 = test_track(
        fixtures::get_demo_file("classical.wav"),
        "Track 1",
        "Multi Track Artist",
    );
    track1.track_number = Some(1);

    let mut track2 = test_track(
        fixtures::get_demo_file("jazz.wav"),
        "Track 2",
        "Multi Track Artist",
    );
    track2.track_number = Some(2);

    let mut track3 = test_track(
        fixtures::get_demo_file("rock.wav"),
        "Track 3",
        "Multi Track Artist",
    );
    track3.track_number = Some(3);

    let album = test_album(
        "Multi Track Album",
        Some(2023),
        vec![track1, track2, track3],
    );

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
        test_album(
            "The Wall",
            Some(1979),
            vec![test_track(
                fixtures::get_demo_file("rock.wav"),
                "Another Brick in the Wall",
                "Pink Floyd",
            )],
        ),
        test_album(
            "Kind of Blue",
            Some(1959),
            vec![test_track(
                fixtures::get_demo_file("jazz.wav"),
                "So What",
                "Miles Davis",
            )],
        ),
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
    assert_eq!(found_album.artist(), "Pink Floyd");
}

#[test]
fn test_search_library_by_album_title() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let album = test_album(
        "Dark Side of the Moon",
        Some(1973),
        vec![test_track(
            fixtures::get_demo_file("rock.wav"),
            "Time",
            "Test Artist",
        )],
    );

    db.save_albums(&[album]).expect("Failed to save album");

    // Search for album title
    let results = db.search_library("Moon").expect("Failed to search library");
    assert_eq!(results.len(), 1, "Should find 1 album");
}

#[test]
fn test_search_library_by_track_title() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let album = test_album(
        "Test Album",
        Some(2024),
        vec![test_track(
            fixtures::get_demo_file("classical.wav"),
            "Bohemian Rhapsody",
            "Test Artist",
        )],
    );

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

    let album = test_album(
        "Abbey Road",
        Some(1969),
        vec![test_track(
            fixtures::get_demo_file("rock.wav"),
            "Come Together",
            "The Beatles",
        )],
    );

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

    let album = test_album(
        "The Wall",
        Some(1979),
        vec![test_track(
            fixtures::get_demo_file("rock.wav"),
            "Another Brick",
            "Pink Floyd",
        )],
    );

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
        test_album(
            "Album 1",
            Some(2024),
            vec![test_track(classical_path.clone(), "Track 1", "Artist 1")],
        ),
        test_album(
            "Album 2",
            Some(2024),
            vec![test_track(jazz_path.clone(), "Track 2", "Artist 2")],
        ),
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
    assert_eq!(loaded[0].artist(), "Artist 1");
}

#[test]
fn test_replay_gain_storage() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let demo_file = fixtures::get_demo_file("classical.wav");
    let album = test_album(
        "Test Album",
        Some(2024),
        vec![test_track(demo_file.clone(), "Test Track", "Test Artist")],
    );

    db.save_albums(&[album]).expect("Failed to save album");

    // Update replay gain
    db.update_replay_gain(&demo_file, -5.5, 0.95)
        .expect("Failed to update replay gain");

    // Load and verify
    let loaded = db.load_library().expect("Failed to load library");
    assert_eq!(loaded.len(), 1);
}

#[test]
fn test_get_tracks_without_replay_gain() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let file1 = fixtures::get_demo_file("classical.wav");
    let file2 = fixtures::get_demo_file("jazz.wav");

    let albums = vec![
        test_album(
            "Album 1",
            Some(2024),
            vec![test_track(file1.clone(), "Track 1", "Artist 1")],
        ),
        test_album(
            "Album 2",
            Some(2024),
            vec![test_track(file2.clone(), "Track 2", "Artist 2")],
        ),
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
    let album = test_album(
        "Original Title",
        Some(2020),
        vec![test_track(demo_file.clone(), "Track 1", "Original Artist")],
    );

    db.save_albums(&[album]).expect("Failed to save album");

    // Load and get the ID
    let loaded = db.load_library().expect("Failed to load library");
    let album_id = loaded[0].id;

    // Update the album (same title, so it should update existing record)
    let mut updated_track1 = test_track(demo_file.clone(), "Updated Track 1", "Original Artist");
    updated_track1.track_number = Some(1);

    let mut new_track2 = test_track(
        fixtures::get_demo_file("jazz.wav"),
        "New Track 2",
        "Original Artist",
    );
    new_track2.track_number = Some(2);

    let updated_album = Album {
        id: album_id,
        title: "Original Title".to_string(),
        year: Some(2024), // Changed year
        tracks: vec![updated_track1, new_track2],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
        is_favorite: false,
        uuid: None,
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
    let album = test_album(
        "Classical Album",
        Some(2024),
        vec![test_track_with_metadata(
            fixtures::get_demo_file("classical.wav"),
            "Symphony No. 1",
            "Classical Artist",
            1,
            Some("Classical"),
            None,
            None,
            None,
            None,
            None,
            None,
        )],
    );

    db.save_albums(&[album]).expect("Failed to save album");

    // Verify genre was added to normalized table
    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 1, "Should have 1 genre");
    assert_eq!(genres[0].1, "Classical");

    // Verify track-genre relationship
    let track_path = fixtures::get_demo_file("classical.wav");
    let track_genre = db
        .get_track_genre(&track_path)
        .expect("Failed to get track genre");
    assert_eq!(track_genre, Some("Classical".to_string()));
}

#[test]
fn test_normalized_composer_table() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let album = test_album(
        "Beethoven Symphonies",
        Some(2024),
        vec![test_track_with_metadata(
            fixtures::get_demo_file("classical.wav"),
            "Symphony No. 5",
            "Orchestra",
            1,
            None,
            Some("Ludwig van Beethoven"),
            None,
            None,
            None,
            None,
            None,
        )],
    );

    db.save_albums(&[album]).expect("Failed to save album");

    let composers = db.get_all_composers().expect("Failed to get composers");
    assert_eq!(composers.len(), 1, "Should have 1 composer");
    assert_eq!(composers[0].1, "Ludwig van Beethoven");

    let tracks = db
        .get_tracks_by_composer(composers[0].0)
        .expect("Failed to get tracks by composer");
    assert_eq!(tracks.len(), 1);
}

#[test]
fn test_normalized_conductor_table() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let album = test_album(
        "Mahler Symphonies",
        Some(2024),
        vec![test_track_with_metadata(
            fixtures::get_demo_file("classical.wav"),
            "Symphony No. 2",
            "Berlin Philharmonic",
            1,
            None,
            None,
            Some("Herbert von Karajan"),
            None,
            None,
            None,
            None,
        )],
    );

    db.save_albums(&[album]).expect("Failed to save album");

    let conductors = db.get_all_conductors().expect("Failed to get conductors");
    assert_eq!(conductors.len(), 1);
    assert_eq!(conductors[0].1, "Herbert von Karajan");

    let tracks = db
        .get_tracks_by_conductor(conductors[0].0)
        .expect("Failed to get tracks");
    assert_eq!(tracks.len(), 1);
}

#[test]
fn test_normalized_performer_table() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let album = test_album(
        "Kind of Blue",
        Some(1959),
        vec![test_track_with_metadata(
            fixtures::get_demo_file("jazz.wav"),
            "So What",
            "Jazz Album",
            1,
            None,
            None,
            None,
            Some("Miles Davis"),
            None,
            None,
            None,
        )],
    );

    db.save_albums(&[album]).expect("Failed to save album");

    let performers = db.get_all_performers().expect("Failed to get performers");
    assert_eq!(performers.len(), 1);
    assert_eq!(performers[0].1, "Miles Davis");

    let tracks = db
        .get_tracks_by_performer(performers[0].0)
        .expect("Failed to get tracks");
    assert_eq!(tracks.len(), 1);
}

#[test]
fn test_normalized_ensemble_table() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let album = test_album(
        "String Quartets",
        Some(2024),
        vec![test_track_with_metadata(
            fixtures::get_demo_file("classical.wav"),
            "Quartet Op. 18",
            "Chamber Music",
            1,
            None,
            None,
            None,
            None,
            Some("Emerson String Quartet"),
            None,
            None,
        )],
    );

    db.save_albums(&[album]).expect("Failed to save album");

    let ensembles = db.get_all_ensembles().expect("Failed to get ensembles");
    assert_eq!(ensembles.len(), 1);
    assert_eq!(ensembles[0].1, "Emerson String Quartet");

    let tracks = db
        .get_tracks_by_ensemble(ensembles[0].0)
        .expect("Failed to get tracks");
    assert_eq!(tracks.len(), 1);
}

#[test]
fn test_normalized_tables_multiple_tracks_same_metadata() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Two albums with the same genre
    let albums = vec![
        test_album(
            "Rock Album 1",
            Some(2024),
            vec![test_track_with_metadata(
                fixtures::get_demo_file("rock.wav"),
                "Rock Song 1",
                "Rock Artist 1",
                1,
                Some("Rock"),
                None,
                None,
                None,
                None,
                None,
                None,
            )],
        ),
        test_album(
            "Rock Album 2",
            Some(2024),
            vec![test_track_with_metadata(
                fixtures::get_demo_file("jazz.wav"),
                "Rock Song 2",
                "Rock Artist 2",
                1,
                Some("Rock"),
                None,
                None,
                None,
                None,
                None,
                None,
            )],
        ),
    ];

    db.save_albums(&albums).expect("Failed to save albums");

    // Should only have one genre entry (normalized)
    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(genres.len(), 1, "Should have exactly 1 genre entry");
    assert_eq!(genres[0].1, "Rock");

    // Both tracks should be linked to this genre
    let genre_id = genres[0].0;
    let tracks = db
        .get_tracks_by_genre(genre_id)
        .expect("Failed to get tracks by genre");
    assert_eq!(
        tracks.len(),
        2,
        "Both tracks should be linked to the Rock genre"
    );

    // Get albums by genre
    let album_ids = db
        .get_albums_by_genre(genre_id)
        .expect("Failed to get albums by genre");
    assert_eq!(album_ids.len(), 2, "Both albums should be in Rock genre");
}

#[test]
fn test_genre_splitting_by_comma() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Track with multiple genres separated by comma
    let album = test_album(
        "Multi-Genre Album",
        Some(2024),
        vec![test_track_with_metadata(
            fixtures::get_demo_file("rock.wav"),
            "Multi-Genre Song",
            "Multi-Genre Artist",
            1,
            Some("Rock, Pop, Electronic"),
            None,
            None,
            None,
            None,
            None,
            None,
        )],
    );

    db.save_albums(&[album]).expect("Failed to save album");

    // Should have 3 separate genre entries
    let genres = db.get_all_genres().expect("Failed to get genres");
    assert_eq!(
        genres.len(),
        3,
        "Should have 3 genres from comma-separated list"
    );

    let genre_names: Vec<&str> = genres.iter().map(|(_, name)| name.as_str()).collect();
    assert!(genre_names.contains(&"Rock"));
    assert!(genre_names.contains(&"Pop"));
    assert!(genre_names.contains(&"Electronic"));

    // Track should be linked to all 3 genres
    let track_path = fixtures::get_demo_file("rock.wav");
    let track_genres = db
        .get_track_genres(&track_path)
        .expect("Failed to get track genres");
    assert_eq!(track_genres.len(), 3, "Track should be linked to 3 genres");
}

#[test]
fn test_normalized_tables_track_with_all_metadata() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    // Create album with all metadata fields
    let album = test_album(
        "Complete Beethoven",
        Some(2024),
        vec![test_track_with_metadata(
            fixtures::get_demo_file("classical.wav"),
            "Symphony No. 9",
            "Vienna Philharmonic",
            1,
            Some("Classical"),
            Some("Ludwig van Beethoven"),
            Some("Herbert von Karajan"),
            Some("Jessye Norman"),
            Some("Vienna State Opera Chorus"),
            Some("Vienna Philharmonic"),
            Some(1),
        )],
    );

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
        test_album(
            "Mozart Vol 1",
            Some(2024),
            vec![test_track_with_metadata(
                fixtures::get_demo_file("classical.wav"),
                "Symphony No. 40",
                "Orchestra 1",
                1,
                None,
                Some("Wolfgang Amadeus Mozart"),
                None,
                None,
                None,
                None,
                None,
            )],
        ),
        test_album(
            "Mozart Vol 2",
            Some(2024),
            vec![test_track_with_metadata(
                fixtures::get_demo_file("jazz.wav"),
                "Symphony No. 41",
                "Orchestra 2",
                1,
                None,
                Some("Wolfgang Amadeus Mozart"),
                None,
                None,
                None,
                None,
                None,
            )],
        ),
    ];

    db.save_albums(&albums).expect("Failed to save albums");

    // Get the composer
    let composers = db.get_all_composers().expect("Failed to get composers");
    assert_eq!(composers.len(), 1);

    // Get albums by composer
    let album_ids = db
        .get_albums_by_composer(composers[0].0)
        .expect("Failed to get albums");
    assert_eq!(album_ids.len(), 2, "Should find 2 albums by Mozart");
}
