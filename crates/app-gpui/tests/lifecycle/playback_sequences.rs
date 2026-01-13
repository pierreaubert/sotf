//! Playback sequence tests - multi-song playback workflows.
//!
//! Tests realistic playback scenarios that span multiple tracks and albums,
//! verifying state preservation throughout the playback lifecycle.

use crate::common::state_builder::{
    TestAlbum, TestPlaybackState, TestQueueItem, TestTrack,
};

// =============================================================================
// Helper: Create multi-track albums for testing
// =============================================================================

fn create_album_with_tracks(name: &str, track_count: usize) -> TestAlbum {
    let tracks: Vec<TestTrack> = (1..=track_count)
        .map(|i| TestTrack::new(&format!("{}_track_{}.flac", name, i)))
        .collect();
    TestAlbum::new(name, &format!("{} Artist", name)).with_tracks(tracks)
}

fn create_playback_state_with_albums(albums: Vec<TestAlbum>, volume: f32) -> TestPlaybackState {
    let queue: Vec<TestQueueItem> = albums.into_iter().map(TestQueueItem::from_album).collect();
    TestPlaybackState::default()
        .with_volume(volume)
        .with_queue(queue)
}

// =============================================================================
// Sequence: Full Album Playback
// =============================================================================

/// Simulate playing through an entire album track by track
#[test]
fn test_full_album_playback_preserves_volume() {
    // Setup: Album with 5 tracks, custom volume
    let album = create_album_with_tracks("TestAlbum", 5);
    let mut state = create_playback_state_with_albums(vec![album], 0.42);
    state.is_playing = true;

    // Play through all tracks
    for track_num in 1..5 {
        // Verify we're on the expected track
        assert_eq!(
            state.current_queue_index,
            Some(0),
            "Should stay on first album"
        );
        let current_item = &state.queue[0];
        assert_eq!(
            current_item.current_track_index,
            track_num - 1,
            "Track index mismatch"
        );

        // Advance to next track
        let next = state.next_track();
        assert!(next.is_some(), "Should have next track");

        // CRITICAL: Volume must be preserved
        assert_eq!(state.volume, 0.42, "Volume changed on track {}", track_num);

        // Position should reset
        assert_eq!(state.position_secs, 0.0, "Position should reset");
    }

    // After last track in album, next_track should move to next album (or end)
    let next = state.next_track();
    assert!(next.is_none(), "Should be at end of queue");
    assert!(!state.is_playing, "Should stop playing at end");

    // Volume still preserved even after playback ends
    assert_eq!(state.volume, 0.42, "Volume changed after playback ended");
}

/// Simulate playing through multiple albums
#[test]
fn test_multi_album_playback_preserves_state() {
    let albums = vec![
        create_album_with_tracks("Album1", 3),
        create_album_with_tracks("Album2", 2),
        create_album_with_tracks("Album3", 4),
    ];
    let mut state = create_playback_state_with_albums(albums, 0.75);
    state.is_playing = true;

    let mut tracks_played = 0;
    let expected_total_tracks = 3 + 2 + 4;

    // Play through all albums
    while state.next_track().is_some() {
        tracks_played += 1;

        // Volume must always be preserved
        assert_eq!(
            state.volume, 0.75,
            "Volume changed after {} tracks",
            tracks_played
        );

        // Safety check to prevent infinite loop
        if tracks_played > expected_total_tracks + 1 {
            panic!("Too many tracks played - possible infinite loop");
        }
    }

    // Should have played through all tracks (minus first which we started on)
    assert_eq!(
        tracks_played,
        expected_total_tracks - 1,
        "Should play all tracks"
    );
    assert_eq!(state.volume, 0.75, "Final volume mismatch");
}

// =============================================================================
// Sequence: Interrupted Playback
// =============================================================================

/// Test pause/resume doesn't affect volume
#[test]
fn test_pause_resume_preserves_volume() {
    let album = create_album_with_tracks("Album", 3);
    let mut state = create_playback_state_with_albums(vec![album], 0.33);
    state.is_playing = true;
    state.position_secs = 45.0;

    // Pause
    state.is_playing = false;

    assert_eq!(state.volume, 0.33, "Volume changed on pause");
    assert_eq!(state.position_secs, 45.0, "Position changed on pause");

    // Resume
    state.is_playing = true;

    assert_eq!(state.volume, 0.33, "Volume changed on resume");
    assert_eq!(state.position_secs, 45.0, "Position changed on resume");

    // Next track
    state.next_track();

    assert_eq!(state.volume, 0.33, "Volume changed on next track");
}

/// Test seek operations during playback
#[test]
fn test_seek_during_playback_preserves_volume() {
    let album = create_album_with_tracks("Album", 3);
    let mut state = create_playback_state_with_albums(vec![album], 0.88);
    state.is_playing = true;
    state.duration_secs = 180.0;

    // Multiple seek operations
    let seek_positions = [0.0, 30.0, 90.0, 150.0, 180.0, 200.0]; // Last one exceeds duration

    for &pos in &seek_positions {
        let _ = state.seek_to(pos);
        assert_eq!(state.volume, 0.88, "Volume changed after seek to {}", pos);
    }

    // Verify position is clamped
    assert!(
        state.position_secs <= state.duration_secs,
        "Position exceeds duration"
    );
}

// =============================================================================
// Sequence: Volume Adjustments During Playback
// =============================================================================

/// Test volume adjustments during multi-track playback
#[test]
fn test_volume_adjustments_persist_across_tracks() {
    let albums = vec![
        create_album_with_tracks("Album1", 2),
        create_album_with_tracks("Album2", 2),
    ];
    let mut state = create_playback_state_with_albums(albums, 0.5);
    state.is_playing = true;

    // Change volume
    state.volume = 0.25;

    // Play next track
    state.next_track();
    assert_eq!(state.volume, 0.25, "Volume not preserved after track change");

    // Change volume again
    state.volume = 0.80;

    // Skip to next album
    state.next_track(); // End of album 1
    state.next_track(); // First track of album 2
    assert_eq!(
        state.volume, 0.80,
        "Volume not preserved across album change"
    );
}

/// Test edge volume values persist
#[test]
fn test_edge_volume_values_persist() {
    let album = create_album_with_tracks("Album", 5);
    let mut state = create_playback_state_with_albums(vec![album], 0.0);
    state.is_playing = true;

    // Test with volume = 0 (muted)
    state.next_track();
    assert_eq!(state.volume, 0.0, "Zero volume not preserved");

    // Test with volume = 1.0 (max)
    state.volume = 1.0;
    state.next_track();
    assert_eq!(state.volume, 1.0, "Max volume not preserved");

    // Test with very small volume
    state.volume = 0.001;
    state.next_track();
    assert!((state.volume - 0.001).abs() < f32::EPSILON, "Small volume not preserved");
}

// =============================================================================
// Sequence: Mute State
// =============================================================================

/// Test mute state persists through playback
#[test]
fn test_mute_persists_through_playback() {
    let album = create_album_with_tracks("Album", 4);
    let mut state = create_playback_state_with_albums(vec![album], 0.6);
    state.is_playing = true;
    state.muted = true;

    // Play through several tracks
    for _ in 0..3 {
        state.next_track();
        assert!(state.muted, "Mute state changed on track change");
        assert_eq!(state.volume, 0.6, "Volume changed while muted");
    }

    // Unmute
    state.muted = false;
    state.next_track();
    assert!(!state.muted, "Unmute didn't persist");
    assert_eq!(state.volume, 0.6, "Volume changed after unmute");
}

// =============================================================================
// Sequence: Rapid Operations
// =============================================================================

/// Test rapid next track operations
#[test]
fn test_rapid_track_skipping() {
    let albums = vec![
        create_album_with_tracks("Album1", 10),
        create_album_with_tracks("Album2", 10),
    ];
    let mut state = create_playback_state_with_albums(albums, 0.55);
    state.is_playing = true;

    // Rapidly skip through many tracks
    for i in 0..15 {
        let result = state.next_track();
        assert_eq!(
            state.volume, 0.55,
            "Volume changed after rapid skip {}",
            i + 1
        );
        if result.is_none() {
            break;
        }
    }
}

/// Test alternating operations
#[test]
fn test_alternating_play_pause_next() {
    let album = create_album_with_tracks("Album", 5);
    let mut state = create_playback_state_with_albums(vec![album], 0.7);

    // Alternating pattern
    for _ in 0..3 {
        state.is_playing = true;
        assert_eq!(state.volume, 0.7);

        state.is_playing = false;
        assert_eq!(state.volume, 0.7);

        state.next_track();
        assert_eq!(state.volume, 0.7);
    }
}

// =============================================================================
// Sequence: Empty/Edge Cases
// =============================================================================

/// Test behavior with single-track album
#[test]
fn test_single_track_album() {
    let album = TestAlbum::new("SingleTrack", "Artist")
        .with_tracks(vec![TestTrack::new("only_track.flac")]);
    let mut state = create_playback_state_with_albums(vec![album], 0.5);
    state.is_playing = true;

    // Next should end playback
    let result = state.next_track();
    assert!(result.is_none(), "Should have no next track");
    assert!(!state.is_playing, "Should stop playing");
    assert_eq!(state.volume, 0.5, "Volume changed");
}

/// Test behavior with empty queue
#[test]
fn test_empty_queue_operations() {
    let mut state = TestPlaybackState::default().with_volume(0.5);

    // Operations on empty queue should not panic
    let result = state.next_track();
    assert!(result.is_none());
    assert_eq!(state.volume, 0.5, "Volume changed on empty queue operation");

    let seek_result = state.seek_to(30.0);
    assert!(seek_result.is_err(), "Seek should fail with no track");
    assert_eq!(state.volume, 0.5, "Volume changed on failed seek");
}

// =============================================================================
// Sequence: Complete Workflow Simulation
// =============================================================================

/// Simulate a realistic listening session
#[test]
fn test_realistic_listening_session() {
    let albums = vec![
        create_album_with_tracks("Morning Jazz", 6),
        create_album_with_tracks("Work Focus", 8),
        create_album_with_tracks("Evening Chill", 5),
    ];
    let mut state = create_playback_state_with_albums(albums, 0.4);

    // Morning: Start playing at moderate volume
    state.is_playing = true;
    state.duration_secs = 240.0;

    // Listen to a few tracks
    state.next_track();
    state.next_track();

    // Seek within track
    let _ = state.seek_to(60.0);

    // Lower volume for a call
    state.volume = 0.1;
    state.muted = true;

    // Pause for the call
    state.is_playing = false;

    // Resume after call
    state.muted = false;
    state.is_playing = true;

    // Skip rest of album
    while state.queue[state.current_queue_index.unwrap()].current_track_index < 5 {
        state.next_track();
    }

    // Move to next album, increase volume for focus
    state.next_track();
    state.volume = 0.6;

    // Skip through work album
    for _ in 0..5 {
        state.next_track();
    }

    // Final state verification
    assert_eq!(state.volume, 0.6, "Volume not at expected level");
    assert!(state.is_playing, "Should still be playing");
    assert!(!state.muted, "Should not be muted");
}
