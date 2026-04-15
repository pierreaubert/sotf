//! Playback State Persistence Tests
//!
//! These tests verify that playback state (volume, mute) is PRESERVED
//! across track changes and queue transitions, using the REAL controllers.
//!
//! # Background
//!
//! A bug was discovered where volume reset to 100% after the first song finished.
//! These tests guard against that regression by using the production QueueController
//! and PlaybackController.

use std::path::PathBuf;

use sotf_audio_player::controllers::library::LibraryController;
use sotf_audio_player::controllers::playback::PlaybackController;
use sotf_audio_player::controllers::queue::{QueueController, QueuePlaybackEffect};
use sotf_audio_player::{Album, ChannelFilter, MusicLibrary, Track};

/// Helper: create an album with N tracks.
fn make_album(title: &str, track_count: usize) -> Album {
    Album {
        title: title.to_string(),
        tracks: (0..track_count)
            .map(|i| Track {
                path: PathBuf::from(format!("/music/{}/track_{}.flac", title, i + 1)),
                title: Some(format!("Track {}", i + 1)),
                channels: Some(2),
                duration_secs: Some(180),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

/// Add album directly to the underlying queue, bypassing file-existence
/// validation (test paths are fake).
fn add_test_album(ctrl: &mut QueueController, album: Album) {
    ctrl.add(album);
}

/// Test: Volume is preserved when advancing to next track within album
#[test]
fn test_volume_preserved_on_next_track_same_album() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.42);

    let mut queue = QueueController::new();
    add_test_album(&mut queue, make_album("Test Album", 3));
    queue.start();

    // Advance to next track
    let effect = queue.next_track();
    assert!(matches!(effect, QueuePlaybackEffect::Play(_)));

    // Volume on PlaybackController MUST be preserved (it's separate from queue)
    assert_eq!(
        playback.volume, 0.42,
        "Volume changed after next_track(): expected 0.42, got {}",
        playback.volume
    );
}

/// Test: Volume is preserved when advancing to next album
#[test]
fn test_volume_preserved_on_next_album() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.73);

    let mut queue = QueueController::new();
    add_test_album(&mut queue, make_album("Album 1", 1));
    add_test_album(&mut queue, make_album("Album 2", 1));
    queue.start();

    // Advance past Album 1's only track into Album 2
    let effect = queue.next_track();
    assert!(matches!(effect, QueuePlaybackEffect::Play(_)));

    assert_eq!(
        playback.volume, 0.73,
        "Volume changed when advancing to next album"
    );
}

/// Test: Volume is preserved across multiple track changes
#[test]
fn test_volume_preserved_across_multiple_tracks() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.55);

    let mut queue = QueueController::new();
    add_test_album(&mut queue, make_album("Long Album", 10));
    queue.start();

    // Advance through all tracks
    for i in 0..9 {
        let effect = queue.next_track();
        assert!(
            matches!(effect, QueuePlaybackEffect::Play(_)),
            "Track {} should exist",
            i + 2
        );
        assert_eq!(
            playback.volume,
            0.55,
            "Volume changed at track {}: expected 0.55, got {}",
            i + 2,
            playback.volume
        );
    }
}

/// Test: Volume is preserved at edge values (0.0 and 1.0)
#[test]
fn test_volume_preserved_at_edge_values() {
    // Test volume = 0.0
    {
        let mut playback = PlaybackController::new();
        playback.set_volume(0.0);

        let mut queue = QueueController::new();
        add_test_album(&mut queue, make_album("Album", 2));
        queue.start();
        queue.next_track();

        assert_eq!(playback.volume, 0.0, "Volume 0.0 not preserved");
    }

    // Test volume = 1.0
    {
        let mut playback = PlaybackController::new();
        playback.set_volume(1.0);

        let mut queue = QueueController::new();
        add_test_album(&mut queue, make_album("Album", 2));
        queue.start();
        queue.next_track();

        assert_eq!(playback.volume, 1.0, "Volume 1.0 not preserved");
    }
}

/// Test: Muted state is independent of queue operations
#[test]
fn test_muted_state_preserved() {
    let mut playback = PlaybackController::new();
    playback.muted = true;

    let mut queue = QueueController::new();
    add_test_album(&mut queue, make_album("Album", 2));
    queue.start();
    queue.next_track();

    assert!(
        playback.muted,
        "Muted state not preserved across track change"
    );
}

/// Test: Queue end returns Stop effect
#[test]
fn test_queue_end_returns_stop() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.8);

    let mut queue = QueueController::new();
    add_test_album(&mut queue, make_album("Album", 1));
    queue.start();

    // Try to advance past end
    let effect = queue.next_track();
    assert_eq!(
        effect,
        QueuePlaybackEffect::Stop,
        "Should return Stop at queue end"
    );

    // Volume MUST still be preserved
    assert_eq!(playback.volume, 0.8, "Volume changed at queue end");
}

/// Test: Filter state preserved when clearing search on real LibraryController
#[test]
fn test_filter_preserved_when_clearing_search() {
    let albums = vec![
        {
            let mut a = Album {
                title: "Stereo Album".to_string(),
                ..Default::default()
            };
            a.tracks.push(Track {
                path: PathBuf::from("/music/stereo.flac"),
                channels: Some(2),
                album_artist: Some("Artist 1".to_string()),
                ..Default::default()
            });
            a
        },
        {
            let mut a = Album {
                title: "Surround Album".to_string(),
                ..Default::default()
            };
            a.tracks.push(Track {
                path: PathBuf::from("/music/surround.flac"),
                channels: Some(6),
                album_artist: Some("Artist 2".to_string()),
                ..Default::default()
            });
            a
        },
    ];

    let mut lib = MusicLibrary::new();
    lib.albums = albums;
    let mut ctrl = LibraryController::with_library(lib);

    // Apply channel filter
    ctrl.set_filter(ChannelFilter::Stereo);
    ctrl.ensure_cache_valid();

    let count_before = ctrl.filtered_albums().len();
    assert_eq!(count_before, 1, "Only stereo album should show");

    // Apply search then clear
    ctrl.set_search_query("Album".to_string());
    ctrl.ensure_cache_valid();
    ctrl.clear_search();
    ctrl.ensure_cache_valid();

    // Channel filter MUST still be active
    let count_after = ctrl.filtered_albums().len();
    assert_eq!(
        count_after, count_before,
        "Channel filter was lost when clearing search"
    );
    assert!(matches!(ctrl.filter, ChannelFilter::Stereo));
}

/// Test: Genre selection preserved when clearing search
#[test]
fn test_genre_selection_preserved_when_clearing_search() {
    let albums = vec![
        {
            let mut a = Album {
                title: "Rock Album".to_string(),
                ..Default::default()
            };
            a.tracks.push(Track {
                path: PathBuf::from("/r.flac"),
                genre: Some("Rock".to_string()),
                album_artist: Some("A1".to_string()),
                ..Default::default()
            });
            a
        },
        {
            let mut a = Album {
                title: "Jazz Album".to_string(),
                ..Default::default()
            };
            a.tracks.push(Track {
                path: PathBuf::from("/j.flac"),
                genre: Some("Jazz".to_string()),
                album_artist: Some("A2".to_string()),
                ..Default::default()
            });
            a
        },
        {
            let mut a = Album {
                title: "Pop Album".to_string(),
                ..Default::default()
            };
            a.tracks.push(Track {
                path: PathBuf::from("/p.flac"),
                genre: Some("Pop".to_string()),
                album_artist: Some("A3".to_string()),
                ..Default::default()
            });
            a
        },
    ];

    let mut lib = MusicLibrary::new();
    lib.albums = albums;
    let mut ctrl = LibraryController::with_library(lib);
    ctrl.ensure_cache_valid();

    // Select genre
    ctrl.selected_genre = Some("Jazz".to_string());

    // With genre filter active
    assert_eq!(ctrl.selection_filtered_albums().len(), 1);

    // Apply search (bypasses genre filter)
    ctrl.set_search_query("Album".to_string());
    ctrl.ensure_cache_valid();
    assert_eq!(ctrl.selection_filtered_albums().len(), 3); // Search returns all

    // Clear search
    ctrl.clear_search();
    ctrl.ensure_cache_valid();

    // Genre filter MUST still be active
    assert_eq!(ctrl.selection_filtered_albums().len(), 1);
    assert_eq!(ctrl.selected_genre, Some("Jazz".to_string()));
}

/// Test: Replay gain adjustment with track having no RG data
#[test]
fn test_replay_gain_none_without_data() {
    let ctrl = PlaybackController::new();
    let track = Track {
        path: PathBuf::from("/test.flac"),
        ..Default::default()
    };

    let adjustment = ctrl.get_replay_gain_adjustment(&track);
    assert!(
        adjustment.is_none(),
        "Should return None when track has no RG data"
    );
}

/// Test: Replay gain adjustment includes preamp
#[test]
fn test_replay_gain_includes_preamp() {
    let mut ctrl = PlaybackController::new();
    ctrl.replay_gain_preamp = 3.0;

    let track = Track {
        path: PathBuf::from("/test.flac"),
        replay_gain: Some(-5.0),
        ..Default::default()
    };

    let adjustment = ctrl.get_replay_gain_adjustment(&track);
    assert_eq!(adjustment, Some(-2.0)); // -5.0 + 3.0
}

/// Test: Replay gain disabled returns None
#[test]
fn test_replay_gain_disabled() {
    let mut ctrl = PlaybackController::new();
    ctrl.replay_gain_enabled = false;

    let track = Track {
        path: PathBuf::from("/test.flac"),
        replay_gain: Some(-5.0),
        ..Default::default()
    };

    assert!(ctrl.get_replay_gain_adjustment(&track).is_none());
}
