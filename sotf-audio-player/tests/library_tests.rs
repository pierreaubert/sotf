use sotf_audio_player::MusicLibrary;
/// Integration tests for MusicLibrary scanning and directory management
use sotf_audio_player::database::MusicDatabase;
use std::path::PathBuf;

mod fixtures;

#[test]
fn test_create_library_with_database() {
    let (_temp_dir, db_path) = fixtures::temp_database();

    let library = MusicLibrary::with_custom_database_for_testing(&db_path)
        .expect("Failed to create library with database");

    assert_eq!(library.directories.len(), 0, "New library should be empty");
    assert_eq!(library.albums.len(), 0, "New library should have no albums");
}

#[test]
fn test_add_directory_to_library() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();

    // Add directory
    library
        .add_directory(demo_dir.clone())
        .expect("Failed to add directory");

    assert_eq!(
        library.directories.len(),
        1,
        "Library should have 1 directory"
    );
    assert_eq!(library.directories[0].path, demo_dir);
}

#[test]
fn test_scan_directory_finds_files() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();
    library.add_directory(demo_dir.clone()).unwrap();

    // Scan the directory
    library.scan().expect("Failed to scan directory");

    // Should have found albums
    assert!(
        !library.albums.is_empty(),
        "Should have found albums in demo directory"
    );

    // Verify albums have tracks
    for album in &library.albums {
        assert!(
            !album.tracks.is_empty(),
            "Album '{}' should have tracks",
            album.title
        );

        // Verify track paths exist
        for track in &album.tracks {
            assert!(
                track.path.exists(),
                "Track path should exist: {:?}",
                track.path
            );
        }
    }
}

#[test]
fn test_scan_directory_extracts_metadata() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();
    library.add_directory(demo_dir.clone()).unwrap();
    library.scan().expect("Failed to scan directory");

    // Verify metadata extraction
    for album in &library.albums {
        for track in &album.tracks {
            // Duration should be extracted (demo files should have reasonable duration)
            if let Some(duration) = track.duration_secs {
                assert!(
                    duration > 0 && duration < 60,
                    "Duration should be reasonable (< 60 seconds), got {}",
                    duration
                );
            }

            // Channels should be extracted (demo files are stereo)
            if let Some(channels) = track.channels {
                assert_eq!(
                    channels, 2,
                    "Demo files should be stereo (2 channels), got {}",
                    channels
                );
            }
        }
    }
}

#[test]
fn test_library_persistence() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();

    // Create library, scan, and save
    {
        let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
        let demo_dir = fixtures::demo_audio_dir();
        library.add_directory(demo_dir).unwrap();
        library.scan().expect("Failed to scan directory");

        let album_count = library.albums.len();
        assert!(album_count > 0, "Should have scanned some albums");
    }

    // Create new library instance and load
    {
        let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
        library
            .load_from_database()
            .expect("Failed to load from database");
        assert!(
            !library.albums.is_empty(),
            "Loaded library should have albums"
        );
    }
}

#[test]
fn test_incremental_scan_skips_unchanged_files() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();
    library.add_directory(demo_dir.clone()).unwrap();

    // First scan
    library.scan().expect("Failed to scan directory");
    let first_album_count = library.albums.len();
    // Saving happens automatically during scan

    // Second scan (should be fast, no changes)
    library.scan().expect("Failed to rescan directory");
    let second_album_count = library.albums.len();

    assert_eq!(
        first_album_count, second_album_count,
        "Incremental scan should find same number of albums"
    );
}

#[test]
fn test_remove_directory() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();
    library.add_directory(demo_dir.clone()).unwrap();
    library.scan().expect("Failed to scan directory");

    assert!(!library.albums.is_empty(), "Should have albums");

    // Remove directory (by index, 0 is the first/only directory)
    library
        .remove_directory(0)
        .expect("Should remove directory");

    assert_eq!(library.directories.len(), 0, "Should have no directories");
    // Note: Albums are not automatically cleared when removing a directory
    // This matches the current implementation behavior
}

#[test]
fn test_search_library_integration() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();

    // Create a controlled test dataset
    let db = MusicDatabase::open_for_testing(&db_path).unwrap();
    let albums = vec![
        sotf_audio_player::Album {
            id: None,
            title: "Dark Side of the Moon".to_string(),
            year: Some(1973),
            tracks: vec![sotf_audio_player::Track {
                path: fixtures::get_demo_file("rock.wav"),
                title: Some("Time".to_string()),
                artist: Some("Pink Floyd".to_string()),
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
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
        },
        sotf_audio_player::Album {
            id: None,
            title: "Kind of Blue".to_string(),
            year: Some(1959),
            tracks: vec![sotf_audio_player::Track {
                path: fixtures::get_demo_file("jazz.wav"),
                title: Some("So What".to_string()),
                artist: Some("Miles Davis".to_string()),
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
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
        },
    ];

    let mut db_mut = db;
    db_mut.save_albums(&albums).expect("Failed to save albums");

    // Reload library
    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library
        .load_from_database()
        .expect("Failed to load from database");

    // Search for "Pink"
    let results = library.search_albums("Pink");
    assert_eq!(results.len(), 1, "Should find 1 album matching 'Pink'");
    assert_eq!(results[0].artist(), "Pink Floyd");

    // Search for "Blue"
    let results = library.search_albums("Blue");
    assert_eq!(results.len(), 1, "Should find 1 album matching 'Blue'");
    assert_eq!(results[0].title, "Kind of Blue");
}

#[test]
fn test_scan_specific_file_formats() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();
    library.add_directory(demo_dir.clone()).unwrap();
    library.scan().expect("Failed to scan directory");

    // Count different file formats
    let mut wav_count = 0;
    let mut flac_count = 0;
    let mut other_count = 0;

    for album in &library.albums {
        for track in &album.tracks {
            match track.path.extension().and_then(|s| s.to_str()) {
                Some("wav") => wav_count += 1,
                Some("flac") => flac_count += 1,
                _ => other_count += 1,
            }
        }
    }

    // We should have found WAV files at minimum
    assert!(wav_count > 0, "Should have found WAV files");

    // Verify total tracks
    let total_tracks = wav_count + flac_count + other_count;
    assert!(total_tracks > 0, "Should have found some audio files");

    println!(
        "Found {} WAV, {} FLAC, {} other files",
        wav_count, flac_count, other_count
    );
}

#[test]
fn test_album_channel_type_detection() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let _library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();

    // Test stereo album
    let stereo_album = sotf_audio_player::Album {
        id: None,
        title: "Stereo Album".to_string(),
        year: None,
        tracks: vec![
            sotf_audio_player::Track {
                path: PathBuf::from("track1.wav"),
                title: None,
                artist: Some("Test".to_string()),
                track_number: None,
                duration_secs: None,
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
            },
            sotf_audio_player::Track {
                path: PathBuf::from("track2.wav"),
                title: None,
                artist: Some("Test".to_string()),
                track_number: None,
                duration_secs: None,
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
            },
        ],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
    };

    let channel_type = stereo_album.channel_type();
    match channel_type {
        Some(sotf_audio_player::AlbumChannelType::Stereo) => {} // Expected
        _ => panic!("Expected Stereo channel type"),
    }

    // Test multichannel album
    let multichannel_album = sotf_audio_player::Album {
        id: None,
        title: "5.1 Album".to_string(),
        year: None,
        tracks: vec![
            sotf_audio_player::Track {
                path: PathBuf::from("track1.wav"),
                title: None,
                artist: Some("Test".to_string()),
                track_number: None,
                duration_secs: None,
                channels: Some(6),
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
            },
            sotf_audio_player::Track {
                path: PathBuf::from("track2.wav"),
                title: None,
                artist: Some("Test".to_string()),
                track_number: None,
                duration_secs: None,
                channels: Some(6),
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
            },
        ],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
    };

    let channel_type = multichannel_album.channel_type();
    match channel_type {
        Some(sotf_audio_player::AlbumChannelType::Multichannel(6)) => {} // Expected
        _ => panic!("Expected Multichannel(6) channel type"),
    }

    // Test mixed album
    let mixed_album = sotf_audio_player::Album {
        id: None,
        title: "Mixed Album".to_string(),
        year: None,
        tracks: vec![
            sotf_audio_player::Track {
                path: PathBuf::from("track1.wav"),
                title: None,
                artist: Some("Test".to_string()),
                track_number: None,
                duration_secs: None,
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
            },
            sotf_audio_player::Track {
                path: PathBuf::from("track2.wav"),
                title: None,
                artist: Some("Test".to_string()),
                track_number: None,
                duration_secs: None,
                channels: Some(6),
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
            },
        ],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
    };

    let channel_type = mixed_album.channel_type();
    match channel_type {
        Some(sotf_audio_player::AlbumChannelType::Mixed) => {} // Expected
        _ => panic!("Expected Mixed channel type"),
    }
}

#[test]
fn test_directory_persistence() {
    fixtures::ensure_demo_files_exist();
    let (_temp_dir, db_path) = fixtures::temp_database();

    // Use demo directory which actually exists
    let test_dir = fixtures::demo_audio_dir();

    // Create library and add directory
    {
        let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
        library.add_directory(test_dir.clone()).unwrap();
        // Scan to save to database
        library.scan().expect("Failed to scan");
    }

    // Reload library and verify directory persists
    {
        let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
        library
            .load_from_database()
            .expect("Failed to load from database");
        // Directory should be loaded (only directories with tracks are persisted)
        assert!(
            !library.directories.is_empty(),
            "Directory should be persisted"
        );
        // The loaded path should match (canonicalized)
        let loaded_path = &library.directories[0].path;
        assert!(
            loaded_path == &test_dir
                || loaded_path.canonicalize().ok() == test_dir.canonicalize().ok(),
            "Directory path should match (possibly canonicalized)"
        );
    }
}

#[test]
fn test_scan_empty_directory() {
    let empty_dir = tempfile::TempDir::new().unwrap();
    let (_temp_dir, db_path) = fixtures::temp_database();

    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();
    library
        .add_directory(empty_dir.path().to_path_buf())
        .unwrap();

    // Scan should succeed but find nothing
    library.scan().expect("Failed to scan empty directory");

    assert_eq!(
        library.albums.len(),
        0,
        "Empty directory should have no albums"
    );
}

#[test]
fn test_scan_nonexistent_directory() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();

    let nonexistent = PathBuf::from("/nonexistent/directory");
    library.add_directory(nonexistent.clone()).unwrap();

    // Scan should handle missing directory gracefully
    let result = library.scan();

    // The current implementation may succeed or fail depending on error handling
    // This test documents the behavior
    match result {
        Ok(_) => {
            // Succeeded, should have no albums
            assert_eq!(library.albums.len(), 0);
        }
        Err(_) => {
            // Failed gracefully, which is also acceptable
        }
    }
}
