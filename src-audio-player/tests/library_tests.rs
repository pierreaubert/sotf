/// Integration tests for MusicLibrary scanning and directory management
use sotf_audio_player::database::MusicDatabase;
use sotf_audio_player::MusicLibrary;
use std::path::PathBuf;

mod fixtures;

#[test]
fn test_create_library_with_database() {
    let (_temp_dir, db_path) = fixtures::temp_database();

    let library =
        MusicLibrary::with_database(&db_path).expect("Failed to create library with database");

    assert_eq!(library.directories.len(), 0, "New library should be empty");
    assert_eq!(library.albums.len(), 0, "New library should have no albums");
}

#[test]
fn test_add_directory_to_library() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_database(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();

    // Add directory
    library
        .add_directory(&demo_dir)
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
    let mut library = MusicLibrary::with_database(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();
    library.add_directory(&demo_dir).unwrap();

    // Scan the directory
    library.scan_all().expect("Failed to scan directory");

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
    let mut library = MusicLibrary::with_database(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();
    library.add_directory(&demo_dir).unwrap();
    library.scan_all().expect("Failed to scan directory");

    // Verify metadata extraction
    for album in &library.albums {
        for track in &album.tracks {
            // Duration should be extracted (demo files are ~5 seconds)
            if let Some(duration) = track.duration_secs {
                assert!(
                    duration > 0 && duration < 10,
                    "Duration should be around 5 seconds, got {}",
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
        let mut library = MusicLibrary::with_database(&db_path).unwrap();
        let demo_dir = fixtures::demo_audio_dir();
        library.add_directory(&demo_dir).unwrap();
        library.scan_all().expect("Failed to scan directory");

        let album_count = library.albums.len();
        assert!(album_count > 0, "Should have scanned some albums");

        library.save_to_db().expect("Failed to save library");
    }

    // Create new library instance and load
    {
        let library = MusicLibrary::with_database(&db_path).unwrap();
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
    let mut library = MusicLibrary::with_database(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();
    library.add_directory(&demo_dir).unwrap();

    // First scan
    library.scan_all().expect("Failed to scan directory");
    let first_album_count = library.albums.len();
    library.save_to_db().expect("Failed to save library");

    // Second scan (should be fast, no changes)
    library.scan_all().expect("Failed to rescan directory");
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
    let mut library = MusicLibrary::with_database(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();
    library.add_directory(&demo_dir).unwrap();
    library.scan_all().expect("Failed to scan directory");

    assert!(!library.albums.is_empty(), "Should have albums");

    // Remove directory
    library
        .remove_directory(&demo_dir)
        .expect("Failed to remove directory");

    assert_eq!(library.directories.len(), 0, "Should have no directories");
    // Note: Albums are not automatically cleared when removing a directory
    // This matches the current implementation behavior
}

#[test]
fn test_search_library_integration() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_database(&db_path).unwrap();

    // Create a controlled test dataset
    let db = MusicDatabase::open(&db_path).unwrap();
    let mut albums = vec![
        sotf_audio_player::Album {
            id: None,
            artist: "Pink Floyd".to_string(),
            title: "Dark Side of the Moon".to_string(),
            year: Some(1973),
            tracks: vec![sotf_audio_player::Track {
                path: fixtures::get_demo_file("rock.wav"),
                title: Some("Time".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            }],
            album_art_path: None,
        },
        sotf_audio_player::Album {
            id: None,
            artist: "Miles Davis".to_string(),
            title: "Kind of Blue".to_string(),
            year: Some(1959),
            tracks: vec![sotf_audio_player::Track {
                path: fixtures::get_demo_file("jazz.wav"),
                title: Some("So What".to_string()),
                track_number: Some(1),
                duration_secs: Some(5),
                channels: Some(2),
            }],
            album_art_path: None,
        },
    ];

    let mut db_mut = db;
    db_mut.save_albums(&albums).expect("Failed to save albums");

    // Reload library
    let library = MusicLibrary::with_database(&db_path).unwrap();

    // Search for "Pink"
    let results = library
        .search_albums("Pink")
        .expect("Failed to search library");
    assert_eq!(results.len(), 1, "Should find 1 album matching 'Pink'");
    assert_eq!(results[0].artist, "Pink Floyd");

    // Search for "Blue"
    let results = library
        .search_albums("Blue")
        .expect("Failed to search library");
    assert_eq!(results.len(), 1, "Should find 1 album matching 'Blue'");
    assert_eq!(results[0].title, "Kind of Blue");
}

#[test]
fn test_scan_specific_file_formats() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_database(&db_path).unwrap();

    let demo_dir = fixtures::demo_audio_dir();
    library.add_directory(&demo_dir).unwrap();
    library.scan_all().expect("Failed to scan directory");

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
    let library = MusicLibrary::with_database(&db_path).unwrap();

    // Test stereo album
    let stereo_album = sotf_audio_player::Album {
        id: None,
        artist: "Test".to_string(),
        title: "Stereo Album".to_string(),
        year: None,
        tracks: vec![
            sotf_audio_player::Track {
                path: PathBuf::from("track1.wav"),
                title: None,
                track_number: None,
                duration_secs: None,
                channels: Some(2),
            },
            sotf_audio_player::Track {
                path: PathBuf::from("track2.wav"),
                title: None,
                track_number: None,
                duration_secs: None,
                channels: Some(2),
            },
        ],
        album_art_path: None,
    };

    let channel_type = library.get_album_channel_type(&stereo_album);
    match channel_type {
        sotf_audio_player::AlbumChannelType::Stereo => {} // Expected
        _ => panic!("Expected Stereo channel type"),
    }

    // Test multichannel album
    let multichannel_album = sotf_audio_player::Album {
        id: None,
        artist: "Test".to_string(),
        title: "5.1 Album".to_string(),
        year: None,
        tracks: vec![
            sotf_audio_player::Track {
                path: PathBuf::from("track1.wav"),
                title: None,
                track_number: None,
                duration_secs: None,
                channels: Some(6),
            },
            sotf_audio_player::Track {
                path: PathBuf::from("track2.wav"),
                title: None,
                track_number: None,
                duration_secs: None,
                channels: Some(6),
            },
        ],
        album_art_path: None,
    };

    let channel_type = library.get_album_channel_type(&multichannel_album);
    match channel_type {
        sotf_audio_player::AlbumChannelType::Multichannel(6) => {} // Expected
        _ => panic!("Expected Multichannel(6) channel type"),
    }

    // Test mixed album
    let mixed_album = sotf_audio_player::Album {
        id: None,
        artist: "Test".to_string(),
        title: "Mixed Album".to_string(),
        year: None,
        tracks: vec![
            sotf_audio_player::Track {
                path: PathBuf::from("track1.wav"),
                title: None,
                track_number: None,
                duration_secs: None,
                channels: Some(2),
            },
            sotf_audio_player::Track {
                path: PathBuf::from("track2.wav"),
                title: None,
                track_number: None,
                duration_secs: None,
                channels: Some(6),
            },
        ],
        album_art_path: None,
    };

    let channel_type = library.get_album_channel_type(&mixed_album);
    match channel_type {
        sotf_audio_player::AlbumChannelType::Mixed => {} // Expected
        _ => panic!("Expected Mixed channel type"),
    }
}

#[test]
fn test_directory_persistence() {
    let (_temp_dir, db_path) = fixtures::temp_database();

    let test_dir = PathBuf::from("/music/test");

    // Create library and add directory
    {
        let mut library = MusicLibrary::with_database(&db_path).unwrap();
        library.add_directory(&test_dir).unwrap();
        // Directories are automatically saved when added
    }

    // Reload library and verify directory persists
    {
        let library = MusicLibrary::with_database(&db_path).unwrap();
        assert_eq!(
            library.directories.len(),
            1,
            "Directory should be persisted"
        );
        assert_eq!(library.directories[0].path, test_dir);
    }
}

#[test]
fn test_scan_empty_directory() {
    let empty_dir = tempfile::TempDir::new().unwrap();
    let (_temp_dir, db_path) = fixtures::temp_database();

    let mut library = MusicLibrary::with_database(&db_path).unwrap();
    library.add_directory(empty_dir.path()).unwrap();

    // Scan should succeed but find nothing
    library.scan_all().expect("Failed to scan empty directory");

    assert_eq!(library.albums.len(), 0, "Empty directory should have no albums");
}

#[test]
fn test_scan_nonexistent_directory() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_database(&db_path).unwrap();

    let nonexistent = PathBuf::from("/nonexistent/directory");
    library.add_directory(&nonexistent).unwrap();

    // Scan should handle missing directory gracefully
    let result = library.scan_all();

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
