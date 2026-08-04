use super::misc::backup_existing_database;
use super::misc::prune_old_backups;
use super::misc::sha256_of_file;
use super::music_database::MusicDatabase;
use crate::federation_config::{FederationSourceEntry, SourceConnectionConfig};
use sotf_audio::decoder::AudioSource;
use sotf_federation::{ProviderAlbum, ProviderTrack};
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
fn test_prune_old_backups_keeps_at_least_three_versions() {
    let test_dir = fresh_config_test_dir("test_prune");

    // Create 5 backups with identical sizes and no age buckets.
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
fn test_prune_old_backups_keeps_previous_weekly_and_monthly_distinct_sizes() {
    let test_dir = fresh_config_test_dir("test_prune_retention_slots");

    let backups = [
        ("20250201-010000", b"old".as_slice()),
        ("20250301-010000", b"monthly".as_slice()),
        ("20250324-010000", b"week-size".as_slice()),
        ("20250331-010000", b"latest".as_slice()),
        // Same size as the newest backup; this should be removed instead of
        // creating a fourth retained version.
        ("20250331-020000", b"other".as_slice()),
    ];

    for (timestamp, contents) in backups {
        std::fs::write(test_dir.join(format!("music-{timestamp}.sqlite")), contents).unwrap();
    }

    prune_old_backups(&test_dir).unwrap();

    let mut remaining: Vec<_> = std::fs::read_dir(&test_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().into_string().unwrap())
        .filter(|name| name.starts_with("music-") && name.ends_with(".sqlite"))
        .collect();
    remaining.sort();

    assert_eq!(
        remaining,
        vec![
            "music-20250301-010000.sqlite",
            "music-20250324-010000.sqlite",
            "music-20250331-020000.sqlite",
        ]
    );

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

#[test]
fn federation_merge_attaches_remote_source_to_matching_local_track_and_unmerges_cleanly() {
    let dir = std::env::temp_dir().join("sotf_test_federation_smart_merge");
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.sqlite");
    let mut db = MusicDatabase::open_for_testing(&db_path).unwrap();

    db.save_albums(&[crate::library::Album {
        title: "Kind of Blue".to_string(),
        tracks: vec![crate::library::Track {
            path: PathBuf::from("/music/kind-of-blue/01-so-what.flac"),
            title: Some("So What".to_string()),
            artist: Some("Miles Davis".to_string()),
            album_artist: Some("Miles Davis".to_string()),
            track_number: Some(1),
            disc_number: Some(1),
            ..Default::default()
        }],
        ..Default::default()
    }])
    .unwrap();

    let source = FederationSourceEntry {
        source_id: "peer:studio".to_string(),
        display_name: "Studio".to_string(),
        priority: 10,
        is_enabled: true,
        connection: SourceConnectionConfig::Peer {
            host: "studio.local".to_string(),
            port: 8732,
            accepted_fingerprint: None,
            auth_token: None,
        },
        is_available: Some(true),
    };
    db.save_federation_source(&source).unwrap();

    let remote_album = ProviderAlbum {
        external_id: "album-remote".to_string(),
        title: "Kind of Blue".to_string(),
        artist: "Miles Davis".to_string(),
        year: Some(1959),
        album_art_url: None,
        tracks: vec![ProviderTrack {
            external_id: "track-remote".to_string(),
            title: "So What".to_string(),
            artist: Some("Miles Davis".to_string()),
            album_artist: Some("Miles Davis".to_string()),
            track_number: Some(1),
            disc_number: Some(1),
            duration_secs: Some(545.0),
            genre: None,
            composer: None,
            channels: Some(2),
            sample_rate: Some(44_100),
            bit_depth: Some(16),
            audio_source: AudioSource::Url {
                url: "http://studio.local:8732/api/v1/media/track-remote?token=redacted"
                    .to_string(),
                format_hint: Some("flac".to_string()),
                seekable: true,
            },
        }],
    };

    let album_id = db
        .merge_federation_album(&source.source_id, &remote_album)
        .unwrap();
    db.merge_federation_track(&source.source_id, album_id, &remote_album.tracks[0])
        .unwrap();

    let loaded = db.load_library().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].tracks.len(), 1);
    assert_eq!(
        loaded[0].tracks[0].path,
        PathBuf::from("/music/kind-of-blue/01-so-what.flac")
    );
    assert!(loaded[0].tracks[0].source.is_none());

    let (removed_tracks, removed_albums) = db.unmerge_federation_source(&source.source_id).unwrap();
    assert_eq!(removed_tracks, 0);
    assert_eq!(removed_albums, 0);

    let loaded = db.load_library().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].tracks.len(), 1);
    assert_eq!(
        loaded[0].tracks[0].audio_source(),
        AudioSource::File(PathBuf::from("/music/kind-of-blue/01-so-what.flac"))
    );

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
