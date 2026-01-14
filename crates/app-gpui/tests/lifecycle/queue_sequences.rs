//! Queue sequence tests - queue management workflows.
//!
//! Tests realistic queue operations including adding albums, navigating
//! through the queue, and managing playback order.

use crate::common::state_builder::{TestAlbum, TestPlaybackState, TestQueueItem, TestTrack};

// =============================================================================
// Helper: Create test data
// =============================================================================

fn create_album(name: &str, artist: &str, track_count: usize) -> TestAlbum {
    let tracks: Vec<TestTrack> = (1..=track_count)
        .map(|i| TestTrack::new(&format!("{}_{}.flac", name, i)))
        .collect();
    TestAlbum::new(name, artist).with_tracks(tracks)
}

fn create_queue(albums: Vec<TestAlbum>) -> Vec<TestQueueItem> {
    albums.into_iter().map(TestQueueItem::from_album).collect()
}

// =============================================================================
// Sequence: Queue Building
// =============================================================================

/// Test building queue from empty
#[test]
fn test_build_queue_from_empty() {
    let mut state = TestPlaybackState::default();
    assert!(state.queue.is_empty());
    assert!(state.current_queue_index.is_none());

    // Add first album
    let album1 = create_album("Album1", "Artist1", 5);
    state.queue.push(TestQueueItem::from_album(album1));
    state.current_queue_index = Some(0);

    assert_eq!(state.queue.len(), 1);
    assert_eq!(state.current_queue_index, Some(0));

    // Add more albums
    state.queue.push(TestQueueItem::from_album(create_album(
        "Album2", "Artist2", 3,
    )));
    state.queue.push(TestQueueItem::from_album(create_album(
        "Album3", "Artist3", 4,
    )));

    assert_eq!(state.queue.len(), 3);
    // Current index unchanged
    assert_eq!(state.current_queue_index, Some(0));
}

/// Test queue with diverse album sizes
#[test]
fn test_queue_diverse_albums() {
    let albums = vec![
        create_album("Single", "Artist", 1),
        create_album("EP", "Artist", 4),
        create_album("Album", "Artist", 12),
        create_album("Double", "Artist", 24),
    ];
    let mut state = TestPlaybackState::default()
        .with_volume(0.5)
        .with_queue(create_queue(albums));

    // Navigate through all tracks
    let mut total_tracks = 0;
    while state.next_track().is_some() {
        total_tracks += 1;
        assert_eq!(
            state.volume, 0.5,
            "Volume changed after {} tracks",
            total_tracks
        );
    }

    // Should have played all tracks minus the first (1-1 + 4-1 + 12-1 + 24-1 = 40)
    // Actually: we start at track 0, and count next_track calls
    // Single: 0 calls (1 track, next returns None)
    // EP: 3 calls, Album: 11 calls, Double: 23 calls
    // Plus transitions: 3 transitions
    assert_eq!(total_tracks, (1 - 1) + (4 - 1) + (12 - 1) + (24 - 1) + 3);
}

// =============================================================================
// Sequence: Queue Navigation
// =============================================================================

/// Test navigating forward through queue
#[test]
fn test_navigate_forward_through_queue() {
    let albums = vec![
        create_album("Album1", "Artist1", 3),
        create_album("Album2", "Artist2", 2),
        create_album("Album3", "Artist3", 4),
    ];
    let mut state = TestPlaybackState::default()
        .with_volume(0.65)
        .with_queue(create_queue(albums));
    state.is_playing = true;

    // Track positions through navigation
    let expected_positions = [
        (0, 0), // Start: Album1, Track0
        (0, 1), // Album1, Track1
        (0, 2), // Album1, Track2
        (1, 0), // Album2, Track0
        (1, 1), // Album2, Track1
        (2, 0), // Album3, Track0
        (2, 1), // Album3, Track1
        (2, 2), // Album3, Track2
        (2, 3), // Album3, Track3
    ];

    // Verify starting position
    assert_eq!(state.current_queue_index, Some(0));
    assert_eq!(state.queue[0].current_track_index, 0);

    // Navigate and verify each position
    for (i, &(album_idx, track_idx)) in expected_positions.iter().skip(1).enumerate() {
        let result = state.next_track();

        if album_idx < 2 || (album_idx == 2 && track_idx < 3) {
            assert!(result.is_some(), "Should have track at step {}", i);
        }

        assert_eq!(
            state.current_queue_index,
            Some(album_idx),
            "Wrong album at step {}",
            i
        );
        assert_eq!(
            state.queue[album_idx].current_track_index, track_idx,
            "Wrong track at step {}",
            i
        );
        assert_eq!(state.volume, 0.65, "Volume changed at step {}", i);
    }

    // Final next should end playback
    let result = state.next_track();
    assert!(result.is_none());
    assert!(!state.is_playing);
}

/// Test that position resets on track changes
#[test]
fn test_position_resets_on_navigation() {
    let albums = vec![create_album("Album", "Artist", 5)];
    let mut state = TestPlaybackState::default().with_queue(create_queue(albums));
    state.duration_secs = 180.0;
    state.position_secs = 120.0; // 2 minutes into track

    // Move to next track
    state.next_track();

    // Position should reset
    assert_eq!(state.position_secs, 0.0);
}

// =============================================================================
// Sequence: Queue State Preservation
// =============================================================================

/// Test all state preserved through queue navigation
#[test]
fn test_state_preserved_through_navigation() {
    let albums = vec![
        create_album("Album1", "Artist1", 3),
        create_album("Album2", "Artist2", 3),
    ];
    let mut state = TestPlaybackState::default()
        .with_volume(0.42)
        .with_queue(create_queue(albums));
    state.muted = true;
    state.is_playing = true;

    // Navigate through several tracks
    for _ in 0..5 {
        state.next_track();

        // All state preserved
        assert_eq!(state.volume, 0.42);
        assert!(state.muted);
        // is_playing can change at end of queue
    }
}

/// Test volume adjustments persist through queue
#[test]
fn test_volume_adjustments_in_queue() {
    let albums = vec![
        create_album("Album1", "Artist1", 2),
        create_album("Album2", "Artist2", 2),
    ];
    let mut state = TestPlaybackState::default()
        .with_volume(0.5)
        .with_queue(create_queue(albums));

    // Play first track, adjust volume
    state.volume = 0.3;

    // Next track
    state.next_track();
    assert_eq!(state.volume, 0.3);

    // Adjust again
    state.volume = 0.8;

    // Next album
    state.next_track();
    state.next_track();
    assert_eq!(state.volume, 0.8);
}

// =============================================================================
// Sequence: Queue Edge Cases
// =============================================================================

/// Test single-track queue
#[test]
fn test_single_track_queue() {
    let albums = vec![create_album("Single", "Artist", 1)];
    let mut state = TestPlaybackState::default()
        .with_volume(0.5)
        .with_queue(create_queue(albums));
    state.is_playing = true;

    // Next should end immediately
    let result = state.next_track();
    assert!(result.is_none());
    assert!(!state.is_playing);
    assert_eq!(state.volume, 0.5);
}

/// Test empty queue operations
#[test]
fn test_empty_queue_safety() {
    let mut state = TestPlaybackState::default().with_volume(0.5);

    // These should not panic
    let result = state.next_track();
    assert!(result.is_none());
    assert_eq!(state.volume, 0.5);

    // Seek should fail gracefully
    let seek_result = state.seek_to(30.0);
    assert!(seek_result.is_err());
    assert_eq!(state.volume, 0.5);
}

/// Test queue index bounds
#[test]
fn test_queue_index_bounds() {
    let albums = vec![create_album("Album", "Artist", 3)];
    let mut state = TestPlaybackState::default().with_queue(create_queue(albums));

    // Manually set invalid index
    state.current_queue_index = Some(100);

    // Operations should handle gracefully
    let result = state.next_track();
    assert!(result.is_none());
}

// =============================================================================
// Sequence: Complex Queue Workflows
// =============================================================================

/// Simulate realistic listening session with queue
#[test]
fn test_realistic_queue_session() {
    // Build a listening queue
    let albums = vec![
        create_album("Morning_Jazz", "Various", 8),
        create_album("Focus_Music", "Study_Beats", 12),
        create_album("Evening_Chill", "Lo_Fi", 6),
    ];
    let mut state = TestPlaybackState::default()
        .with_volume(0.4)
        .with_queue(create_queue(albums));
    state.is_playing = true;
    state.duration_secs = 240.0;

    // Morning: Listen to a few tracks
    state.next_track();
    state.next_track();
    state.next_track();

    // Adjust volume
    state.volume = 0.5;

    // Skip to next album
    while state.current_queue_index == Some(0) {
        if state.next_track().is_none() {
            break;
        }
    }

    // Should be on Focus_Music now
    assert_eq!(state.current_queue_index, Some(1));
    assert_eq!(state.volume, 0.5);

    // Listen and adjust volume for focus
    state.volume = 0.3;
    for _ in 0..5 {
        state.next_track();
    }
    assert_eq!(state.volume, 0.3);

    // Lower volume for a meeting
    state.volume = 0.1;
    state.muted = true;

    // Meeting over, unmute and raise volume
    state.muted = false;
    state.volume = 0.6;

    // Continue listening
    while state.next_track().is_some() {
        assert_eq!(state.volume, 0.6);
        assert!(!state.muted);
    }

    // End of queue
    assert!(!state.is_playing);
    assert_eq!(state.volume, 0.6);
}

/// Test adding to queue during playback
#[test]
fn test_dynamic_queue_addition() {
    let initial_album = create_album("Initial", "Artist", 3);
    let mut state = TestPlaybackState::default()
        .with_volume(0.5)
        .with_queue(vec![TestQueueItem::from_album(initial_album)]);
    state.is_playing = true;

    // Listen to first track, then add more to queue
    state.next_track();

    // Add another album to queue
    state.queue.push(TestQueueItem::from_album(create_album(
        "Added", "Artist", 2,
    )));

    // Continue listening through original album
    state.next_track();

    // Should now move to added album
    state.next_track();
    assert_eq!(state.current_queue_index, Some(1));

    // Volume preserved throughout
    assert_eq!(state.volume, 0.5);
}

/// Test queue with identical albums
#[test]
fn test_queue_with_duplicates() {
    let album = create_album("Repeat", "Artist", 3);
    let albums = vec![album.clone(), album.clone(), album];
    let mut state = TestPlaybackState::default()
        .with_volume(0.5)
        .with_queue(create_queue(albums));

    // Should be able to play through all copies
    let mut album_transitions = 0;
    let mut last_album_idx = 0;

    while let Some(_) = state.next_track() {
        let current_album = state.current_queue_index.unwrap();
        if current_album != last_album_idx {
            album_transitions += 1;
            last_album_idx = current_album;
        }
        assert_eq!(state.volume, 0.5);
    }

    // Should have transitioned through 2 albums (0→1, 1→2)
    assert_eq!(album_transitions, 2);
}

/// Test very long queue
#[test]
fn test_long_queue() {
    // Create 100 single-track albums
    let albums: Vec<TestAlbum> = (0..100)
        .map(|i| create_album(&format!("Album{}", i), "Artist", 1))
        .collect();

    let mut state = TestPlaybackState::default()
        .with_volume(0.5)
        .with_queue(create_queue(albums));
    state.is_playing = true;

    // Should be able to navigate through all
    let mut count = 0;
    while state.next_track().is_some() {
        count += 1;
        assert_eq!(state.volume, 0.5);

        // Safety limit
        if count > 150 {
            panic!("Infinite loop detected");
        }
    }

    // 100 albums, 1 track each = 99 next_track calls (start on first)
    assert_eq!(count, 99);
}
