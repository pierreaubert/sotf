/// Integration tests for ReplayGain scanning functionality
use sotf_audio_player::MusicLibrary;
use sotf_audio_player::database::MusicDatabase;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

mod fixtures;

#[test]
fn test_replay_gain_scanner_creation() {
    use sotf_audio_player::ReplayGainScanner;

    let (_temp_dir, db_path) = fixtures::temp_database();
    let _db = Arc::new(MusicDatabase::open_for_testing(&db_path).expect("Failed to open database"));

    // Create scanner (new API: num_threads, db_path)
    let scanner = ReplayGainScanner::new(2, db_path.clone(), Arc::new(AtomicBool::new(false)));

    // Scanner should start successfully
    drop(scanner);
}

#[test]
fn test_get_tracks_without_replay_gain_empty() {
    let (_temp_dir, db_path) = fixtures::temp_database();
    let db = MusicDatabase::open_for_testing(&db_path).unwrap();

    let tracks = db
        .get_tracks_without_replay_gain()
        .expect("Failed to get tracks");

    assert_eq!(tracks.len(), 0, "Empty database should have no tracks");
}

#[test]
fn test_get_tracks_without_replay_gain_after_scan() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let mut library = MusicLibrary::with_custom_database_for_testing(&db_path).unwrap();

    // Scan demo files
    let demo_dir = fixtures::demo_audio_dir();
    library.add_directory(demo_dir.clone()).unwrap();
    library.scan().expect("Failed to scan directory");
    // Saving happens automatically during scan

    // All tracks should need ReplayGain
    let db = MusicDatabase::open_for_testing(&db_path).unwrap();
    let tracks = db
        .get_tracks_without_replay_gain()
        .expect("Failed to get tracks");

    let scanned_track_count = library.albums.iter().map(|a| a.tracks.len()).sum::<usize>();

    assert_eq!(
        tracks.len(),
        scanned_track_count,
        "All scanned tracks should need ReplayGain initially"
    );
}

#[test]
fn test_update_replay_gain() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();

    // Add a single track
    let classical_file = fixtures::get_demo_file("classical.wav");
    let album = sotf_audio_player::Album {
        id: None,
        title: "Test Album".to_string(),
        year: Some(2024),
        tracks: vec![sotf_audio_player::Track {
            path: classical_file.clone(),
            title: Some("Test Track".to_string()),
            artist: Some("Test Artist".to_string()),
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
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
        is_favorite: false,
    };

    let db = MusicDatabase::open_for_testing(&db_path).unwrap();
    let mut db_mut = db;
    db_mut.save_albums(&[album]).expect("Failed to save album");

    // Initially needs ReplayGain
    let tracks = db_mut
        .get_tracks_without_replay_gain()
        .expect("Failed to get tracks");
    assert_eq!(tracks.len(), 1);

    // Update ReplayGain
    db_mut
        .update_replay_gain(&classical_file, -8.5, 0.88)
        .expect("Failed to update ReplayGain");

    // Should no longer need ReplayGain
    let tracks = db_mut
        .get_tracks_without_replay_gain()
        .expect("Failed to get tracks");
    assert_eq!(tracks.len(), 0, "Track should have ReplayGain now");
}

#[test]
fn test_replay_gain_values_persistence() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let classical_file = fixtures::get_demo_file("classical.wav");

    // Save track and update ReplayGain
    {
        let album = sotf_audio_player::Album {
            id: None,
            title: "Test Album".to_string(),
            year: Some(2024),
            tracks: vec![sotf_audio_player::Track {
                path: classical_file.clone(),
                title: Some("Test Track".to_string()),
                artist: Some("Test Artist".to_string()),
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
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
        };

        let db = MusicDatabase::open_for_testing(&db_path).unwrap();
        let mut db_mut = db;
        db_mut.save_albums(&[album]).expect("Failed to save album");

        db_mut
            .update_replay_gain(&classical_file, -6.2, 0.91)
            .expect("Failed to update ReplayGain");
    }

    // Reload database and verify persistence
    {
        let db = MusicDatabase::open_for_testing(&db_path).unwrap();
        let tracks = db
            .get_tracks_without_replay_gain()
            .expect("Failed to get tracks");

        assert_eq!(
            tracks.len(),
            0,
            "ReplayGain values should persist across database reloads"
        );
    }
}

#[test]
fn test_partial_replay_gain_scanning() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let file1 = fixtures::get_demo_file("classical.wav");
    let file2 = fixtures::get_demo_file("jazz.wav");
    let file3 = fixtures::get_demo_file("rock.wav");

    let albums = vec![
        sotf_audio_player::Album {
            id: None,
            title: "Album 1".to_string(),
            year: Some(2024),
            tracks: vec![sotf_audio_player::Track {
                path: file1.clone(),
                title: Some("Track 1".to_string()),
                artist: Some("Artist 1".to_string()),
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
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
        },
        sotf_audio_player::Album {
            id: None,
            title: "Album 2".to_string(),
            year: Some(2024),
            tracks: vec![sotf_audio_player::Track {
                path: file2.clone(),
                title: Some("Track 2".to_string()),
                artist: Some("Artist 2".to_string()),
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
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
        },
        sotf_audio_player::Album {
            id: None,
            title: "Album 3".to_string(),
            year: Some(2024),
            tracks: vec![sotf_audio_player::Track {
                path: file3.clone(),
                title: Some("Track 3".to_string()),
                artist: Some("Artist 3".to_string()),
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
            }],
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
        },
    ];

    let db = MusicDatabase::open_for_testing(&db_path).unwrap();
    let mut db_mut = db;
    db_mut.save_albums(&albums).expect("Failed to save albums");

    // All 3 tracks need ReplayGain
    let tracks = db_mut
        .get_tracks_without_replay_gain()
        .expect("Failed to get tracks");
    assert_eq!(tracks.len(), 3);

    // Scan only one track
    db_mut
        .update_replay_gain(&file1, -7.0, 0.85)
        .expect("Failed to update ReplayGain");

    // Now only 2 tracks need ReplayGain
    let tracks = db_mut
        .get_tracks_without_replay_gain()
        .expect("Failed to get tracks");
    assert_eq!(tracks.len(), 2);
    assert!(tracks.contains(&file2));
    assert!(tracks.contains(&file3));
    assert!(!tracks.contains(&file1));
}

#[test]
fn test_replay_gain_range_values() {
    fixtures::ensure_demo_files_exist();

    let (_temp_dir, db_path) = fixtures::temp_database();
    let test_file = fixtures::get_demo_file("classical.wav");

    let album = sotf_audio_player::Album {
        id: None,
        title: "Test".to_string(),
        year: None,
        tracks: vec![sotf_audio_player::Track {
            path: test_file.clone(),
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
            is_favorite: false,
            play_count: 0,
            source: None,
        }],
        album_art_path: None,
        album_art_thumbnail: None,
        play_count: 0,
        edition: None,
        dynamic_range: None,
        is_favorite: false,
    };

    let db = MusicDatabase::open_for_testing(&db_path).unwrap();
    let mut db_mut = db;
    db_mut.save_albums(&[album]).expect("Failed to save album");

    // Test extreme but valid values
    let test_cases = vec![
        (-20.0, 0.1),   // Quiet track, low peak
        (0.0, 1.0),     // No gain needed, full peak
        (-10.5, 0.707), // Typical values
        (5.0, 0.95),    // Positive gain (rare but valid)
    ];

    for (gain, peak) in test_cases {
        db_mut
            .update_replay_gain(&test_file, gain, peak)
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to update ReplayGain with gain={}, peak={}",
                    gain, peak
                )
            });

        // Verify it's no longer needed
        let tracks = db_mut
            .get_tracks_without_replay_gain()
            .expect("Failed to get tracks");
        assert_eq!(tracks.len(), 0, "Track should have ReplayGain");
    }
}

#[test]
fn test_real_replay_gain_scanning() {
    fixtures::ensure_demo_files_exist();

    // Analyze 2 files, reading only ~0.5 MB each (131072 f32 samples)
    let max_samples = 131_072;
    let files = ["classical.wav", "jazz.wav"];

    for name in &files {
        let path = fixtures::get_demo_file(name);
        let info = sotf_audio::replaygain::analyze_file_limited(&path, Some(max_samples))
            .unwrap_or_else(|e| panic!("Failed to analyze {}: {}", name, e));

        // ReplayGain should be in a plausible range (-30..+10 dB)
        assert!(
            (-30.0..=10.0).contains(&info.gain),
            "{}: gain {:.2} dB out of expected range",
            name,
            info.gain
        );
        // Peak should be positive and at most 1.0 for well-formed audio
        assert!(
            info.peak > 0.0 && info.peak <= 1.5,
            "{}: peak {:.4} out of expected range",
            name,
            info.peak
        );
    }
}
