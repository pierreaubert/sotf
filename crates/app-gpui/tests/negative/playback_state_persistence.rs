//! Playback State Persistence Tests
//!
//! These tests verify that playback state (volume, settings) is PRESERVED
//! across track changes and other transitions.
//!
//! # Background
//!
//! A bug was discovered where volume reset to 100% after the first song finished.
//! The volume value wasn't preserved across song changes because the EngineConfig
//! was being created with hardcoded `volume: 1.0` on every track load.

#[path = "../common/mod.rs"]
mod common;

use common::state_builder::{
    TestAlbum, TestChannelFilter, TestLibraryState, TestPlaybackState, TestQueueItem, TestTrack,
};

/// Test: Volume is preserved when advancing to next track within album
#[test]
fn test_volume_preserved_on_next_track_same_album() {
    let tracks = vec![
        TestTrack::new("track1.flac"),
        TestTrack::new("track2.flac"),
        TestTrack::new("track3.flac"),
    ];
    let album = TestAlbum::new("Test Album", "Test Artist").with_tracks(tracks);

    let mut state = TestPlaybackState::default()
        .with_volume(0.42)
        .with_queue(vec![TestQueueItem::from_album(album)]);

    state.is_playing = true;

    // Advance to next track
    let next = state.next_track();
    assert!(next.is_some());

    // Volume MUST be preserved
    assert_eq!(
        state.volume, 0.42,
        "Volume changed after next_track(): expected 0.42, got {}",
        state.volume
    );
}

/// Test: Volume is preserved when advancing to next album
#[test]
fn test_volume_preserved_on_next_album() {
    let album1 =
        TestAlbum::new("Album 1", "Artist 1").with_tracks(vec![TestTrack::new("a1_t1.flac")]);
    let album2 =
        TestAlbum::new("Album 2", "Artist 2").with_tracks(vec![TestTrack::new("a2_t1.flac")]);

    let mut state = TestPlaybackState::default()
        .with_volume(0.73)
        .with_queue(vec![
            TestQueueItem::from_album(album1),
            TestQueueItem::from_album(album2),
        ]);

    state.is_playing = true;

    // Advance to next album
    let next = state.next_track();
    assert!(next.is_some());

    // Volume MUST be preserved
    assert_eq!(
        state.volume, 0.73,
        "Volume changed when advancing to next album"
    );
}

/// Test: Volume is preserved across multiple track changes
#[test]
fn test_volume_preserved_across_multiple_tracks() {
    let tracks: Vec<TestTrack> = (1..=10)
        .map(|i| TestTrack::new(&format!("track{}.flac", i)))
        .collect();
    let album = TestAlbum::new("Long Album", "Artist").with_tracks(tracks);

    let mut state = TestPlaybackState::default()
        .with_volume(0.55)
        .with_queue(vec![TestQueueItem::from_album(album)]);

    state.is_playing = true;

    // Advance through all tracks
    for i in 0..9 {
        let next = state.next_track();
        assert!(next.is_some(), "Track {} should exist", i + 2);
        assert_eq!(
            state.volume,
            0.55,
            "Volume changed at track {}: expected 0.55, got {}",
            i + 2,
            state.volume
        );
    }
}

/// Test: Volume is preserved at edge values (0.0 and 1.0)
#[test]
fn test_volume_preserved_at_edge_values() {
    let tracks = vec![TestTrack::new("track1.flac"), TestTrack::new("track2.flac")];

    // Test volume = 0.0
    {
        let album = TestAlbum::new("Album", "Artist").with_tracks(tracks.clone());
        let mut state = TestPlaybackState::default()
            .with_volume(0.0)
            .with_queue(vec![TestQueueItem::from_album(album)]);
        state.is_playing = true;

        state.next_track();
        assert_eq!(state.volume, 0.0, "Volume 0.0 not preserved");
    }

    // Test volume = 1.0
    {
        let album = TestAlbum::new("Album", "Artist").with_tracks(tracks);
        let mut state = TestPlaybackState::default()
            .with_volume(1.0)
            .with_queue(vec![TestQueueItem::from_album(album)]);
        state.is_playing = true;

        state.next_track();
        assert_eq!(state.volume, 1.0, "Volume 1.0 not preserved");
    }
}

/// Test: Position resets when changing tracks (correct behavior)
#[test]
fn test_position_resets_on_track_change() {
    let tracks = vec![TestTrack::new("track1.flac"), TestTrack::new("track2.flac")];
    let album = TestAlbum::new("Album", "Artist").with_tracks(tracks);

    let mut state = TestPlaybackState::default().with_queue(vec![TestQueueItem::from_album(album)]);

    state.is_playing = true;
    state.position_secs = 120.0; // Simulate playback at 2 minutes

    // Advance to next track
    state.next_track();

    // Position SHOULD reset (this is correct behavior)
    assert_eq!(
        state.position_secs, 0.0,
        "Position should reset on track change"
    );
}

/// Test: Muted state is preserved across track changes
#[test]
fn test_muted_state_preserved() {
    let tracks = vec![TestTrack::new("track1.flac"), TestTrack::new("track2.flac")];
    let album = TestAlbum::new("Album", "Artist").with_tracks(tracks);

    let mut state = TestPlaybackState::default().with_queue(vec![TestQueueItem::from_album(album)]);

    state.muted = true;
    state.is_playing = true;

    state.next_track();

    assert!(state.muted, "Muted state not preserved");
}

/// Test: Filter state preserved when clearing search
#[test]
fn test_filter_preserved_when_clearing_search() {
    let albums = vec![
        TestAlbum::new("Album 1", "Artist 1").with_channels(2),
        TestAlbum::new("Album 2", "Artist 2").with_channels(6),
    ];

    let mut state = TestLibraryState::default().with_albums(albums);

    // Apply channel filter
    state.set_channel_filter(TestChannelFilter::Stereo);

    // Verify filter works
    let filtered_before_search = state.filtered_albums().len();
    assert_eq!(filtered_before_search, 1); // Only stereo album

    // Apply search
    state.search_query = "Album".to_string();

    // Clear search
    state.clear_search();

    // Filter MUST still be active
    let filtered_after_clear = state.filtered_albums().len();
    assert_eq!(
        filtered_after_clear, filtered_before_search,
        "Channel filter was lost when clearing search"
    );
    assert_eq!(state.channel_filter, TestChannelFilter::Stereo);
}

/// Test: Genre selection preserved when clearing search
#[test]
fn test_genre_selection_preserved_when_clearing_search() {
    let albums = vec![
        TestAlbum::new("Rock Album", "Artist 1").with_genre("Rock"),
        TestAlbum::new("Jazz Album", "Artist 2").with_genre("Jazz"),
        TestAlbum::new("Pop Album", "Artist 3").with_genre("Pop"),
    ];

    let mut state = TestLibraryState::default().with_albums(albums);

    // Select genre
    state.selected_genre = Some("Jazz".to_string());

    // Apply search (bypasses genre filter)
    state.search_query = "Album".to_string();
    assert_eq!(state.filtered_albums().len(), 3); // Search returns all

    // Clear search
    state.clear_search();

    // Genre filter MUST still be active
    assert_eq!(state.filtered_albums().len(), 1);
    assert_eq!(state.selected_genre, Some("Jazz".to_string()));
}

/// Test: Seek fails gracefully with no track loaded
#[test]
fn test_seek_fails_gracefully_no_track() {
    let mut state = TestPlaybackState::default();
    assert!(state.current_queue_index.is_none());

    let result = state.seek_to(30.0);

    // Should fail, not crash
    assert!(result.is_err());
    assert_eq!(state.position_secs, 0.0);
}

/// Test: Volume clamping prevents invalid values
#[test]
fn test_volume_clamping() {
    let mut state = TestPlaybackState::default();

    state.set_volume(-0.5);
    assert_eq!(state.volume, 0.0, "Negative volume not clamped");

    state.set_volume(2.0);
    assert_eq!(state.volume, 1.0, "Volume > 1.0 not clamped");

    state.set_volume(0.5);
    assert_eq!(state.volume, 0.5, "Valid volume was modified");
}

/// Test: Queue end handling preserves state
#[test]
fn test_queue_end_preserves_state() {
    let album =
        TestAlbum::new("Album", "Artist").with_tracks(vec![TestTrack::new("only_track.flac")]);

    let mut state = TestPlaybackState::default()
        .with_volume(0.8)
        .with_queue(vec![TestQueueItem::from_album(album)]);

    state.is_playing = true;

    // Try to advance past end
    let next = state.next_track();
    assert!(next.is_none(), "Should return None at queue end");

    // Volume MUST still be preserved
    assert_eq!(state.volume, 0.8, "Volume changed at queue end");

    // Playing should stop
    assert!(!state.is_playing, "Should stop playing at queue end");
}
