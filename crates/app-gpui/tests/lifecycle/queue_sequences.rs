//! Queue sequence tests - queue management workflows.
//!
//! Tests realistic queue operations including adding albums, navigating
//! through the queue, and managing playback order.
//! Uses real `QueueController`, `Album`, and `Track` from `sotf_audio_player`.

use crate::common::factories::album_with_tracks;
use sotf_audio_player::controllers::queue::{QueueController, QueuePlaybackEffect};

// =============================================================================
// Sequence: Queue Building
// =============================================================================

/// Test building queue from empty
#[test]
fn test_build_queue_from_empty() {
    let mut queue = QueueController::new();
    assert!(queue.is_empty());
    assert!(queue.current_index().is_none());

    // Add first album
    let _ = queue.add_album(album_with_tracks("Album1", "Artist1", 5));
    assert_eq!(queue.len(), 1);
    // current_index is still None until start() is called
    assert!(queue.current_index().is_none());

    // Add more albums
    let _ = queue.add_album(album_with_tracks("Album2", "Artist2", 3));
    let _ = queue.add_album(album_with_tracks("Album3", "Artist3", 4));
    assert_eq!(queue.len(), 3);

    // Start playback
    let effect = queue.start();
    assert!(matches!(effect, QueuePlaybackEffect::Play(_)));
    assert_eq!(queue.current_index(), Some(0));
}

/// Test queue with diverse album sizes
#[test]
fn test_queue_diverse_albums() {
    let mut queue = QueueController::new();
    let _ = queue.add_album(album_with_tracks("Single", "Artist", 1));
    let _ = queue.add_album(album_with_tracks("EP", "Artist", 4));
    let _ = queue.add_album(album_with_tracks("Album", "Artist", 12));
    let _ = queue.add_album(album_with_tracks("Double", "Artist", 24));
    queue.start();

    let mut total_next_calls = 0;
    loop {
        let effect = queue.next_track();
        match effect {
            QueuePlaybackEffect::Play(_) => total_next_calls += 1,
            QueuePlaybackEffect::Stop => break,
            QueuePlaybackEffect::None => panic!("Unexpected None effect"),
            QueuePlaybackEffect::Reload(_) => panic!("Unexpected Reload effect"),
        }
        if total_next_calls > 50 {
            panic!("Too many tracks - possible infinite loop");
        }
    }

    // Total tracks = 1 + 4 + 12 + 24 = 41. We start on the first, so 40 next_track calls.
    assert_eq!(total_next_calls, 40);
}

// =============================================================================
// Sequence: Queue Navigation
// =============================================================================

/// Test navigating forward through queue, verifying album transitions
#[test]
fn test_navigate_forward_through_queue() {
    let mut queue = QueueController::new();
    let _ = queue.add_album(album_with_tracks("Album1", "Artist1", 3));
    let _ = queue.add_album(album_with_tracks("Album2", "Artist2", 2));
    let _ = queue.add_album(album_with_tracks("Album3", "Artist3", 4));
    queue.start();

    // Verify starting position
    assert_eq!(queue.current_index(), Some(0));
    assert!(queue.current_track().is_some());

    // Album1: 3 tracks. Start on track 1.
    // next -> track 2 (still Album1)
    assert!(matches!(queue.next_track(), QueuePlaybackEffect::Play(_)));
    assert_eq!(queue.current_index(), Some(0));

    // next -> track 3 (still Album1)
    assert!(matches!(queue.next_track(), QueuePlaybackEffect::Play(_)));
    assert_eq!(queue.current_index(), Some(0));

    // next -> crosses to Album2, track 1
    assert!(matches!(queue.next_track(), QueuePlaybackEffect::Play(_)));
    assert_eq!(queue.current_index(), Some(1));

    // Album2: 2 tracks. Currently on track 1.
    // next -> track 2 (still Album2)
    assert!(matches!(queue.next_track(), QueuePlaybackEffect::Play(_)));
    assert_eq!(queue.current_index(), Some(1));

    // next -> crosses to Album3, track 1
    assert!(matches!(queue.next_track(), QueuePlaybackEffect::Play(_)));
    assert_eq!(queue.current_index(), Some(2));

    // Album3: 4 tracks. Currently on track 1.
    // 3 more next_track calls for tracks 2, 3, 4
    for _ in 0..3 {
        assert!(matches!(queue.next_track(), QueuePlaybackEffect::Play(_)));
        assert_eq!(queue.current_index(), Some(2));
    }

    // End of queue
    assert_eq!(queue.next_track(), QueuePlaybackEffect::Stop);
}

/// Test that current_track returns the right track after navigation
#[test]
fn test_current_track_updates_on_navigation() {
    let mut queue = QueueController::new();
    let _ = queue.add_album(album_with_tracks("Album", "Artist", 3));
    queue.start();

    let first_track = queue.current_track().unwrap().title.clone();
    queue.next_track();
    let second_track = queue.current_track().unwrap().title.clone();

    assert_ne!(
        first_track, second_track,
        "Track should change after next_track"
    );
}

// =============================================================================
// Sequence: Queue Edge Cases
// =============================================================================

/// Test single-track queue
#[test]
fn test_single_track_queue() {
    let mut queue = QueueController::new();
    let _ = queue.add_album(album_with_tracks("Single", "Artist", 1));
    queue.start();

    // Next should return Stop immediately
    assert_eq!(queue.next_track(), QueuePlaybackEffect::Stop);
}

/// Test empty queue operations
#[test]
fn test_empty_queue_safety() {
    let mut queue = QueueController::new();

    // start on empty queue returns None
    let effect = queue.start();
    assert!(matches!(effect, QueuePlaybackEffect::None));

    // next_track on empty/unstarted queue returns Stop
    let effect = queue.next_track();
    assert_eq!(effect, QueuePlaybackEffect::Stop);

    assert!(queue.current_track().is_none());
    assert!(queue.current_index().is_none());
}

/// Test queue with single-track albums — each next_track crosses an album boundary
#[test]
fn test_single_track_albums_navigation() {
    let mut queue = QueueController::new();
    for i in 0..5 {
        let _ = queue.add_album(album_with_tracks(&format!("Album{}", i), "Artist", 1));
    }
    queue.start();
    assert_eq!(queue.current_index(), Some(0));

    for expected_idx in 1..5 {
        let effect = queue.next_track();
        assert!(matches!(effect, QueuePlaybackEffect::Play(_)));
        assert_eq!(queue.current_index(), Some(expected_idx));
    }

    assert_eq!(queue.next_track(), QueuePlaybackEffect::Stop);
}

// =============================================================================
// Sequence: Dynamic Queue Addition
// =============================================================================

/// Test adding albums to queue during playback
#[test]
fn test_dynamic_queue_addition() {
    let mut queue = QueueController::new();
    let _ = queue.add_album(album_with_tracks("Initial", "Artist", 3));
    queue.start();

    // Listen to first track, then add more
    queue.next_track();

    // Add another album
    let _ = queue.add_album(album_with_tracks("Added", "Artist", 2));
    assert_eq!(queue.len(), 2);

    // Continue through original album
    queue.next_track(); // track 3 of Initial

    // Cross to Added album
    let effect = queue.next_track();
    assert!(matches!(effect, QueuePlaybackEffect::Play(_)));
    assert_eq!(queue.current_index(), Some(1));
}

/// Test adding album while at end of queue
#[test]
fn test_add_album_after_exhaustion() {
    let mut queue = QueueController::new();
    let _ = queue.add_album(album_with_tracks("First", "Artist", 1));
    queue.start();

    // Exhaust the queue
    assert_eq!(queue.next_track(), QueuePlaybackEffect::Stop);

    // Add new album and jump to it
    let _ = queue.add_album(album_with_tracks("Second", "Artist", 2));
    let effect = queue.jump_to(1);
    assert!(matches!(effect, QueuePlaybackEffect::Play(_)));
    assert_eq!(queue.current_index(), Some(1));
}

// =============================================================================
// Sequence: Queue with Duplicates
// =============================================================================

/// Test queue with identical albums
#[test]
fn test_queue_with_duplicates() {
    let mut queue = QueueController::new();
    // Add same album structure 3 times
    for _ in 0..3 {
        let _ = queue.add_album(album_with_tracks("Repeat", "Artist", 3));
    }
    queue.start();

    let mut album_transitions = 0;
    let mut last_album_idx = 0;

    loop {
        let effect = queue.next_track();
        match effect {
            QueuePlaybackEffect::Play(_) => {
                let current_album = queue.current_index().unwrap();
                if current_album != last_album_idx {
                    album_transitions += 1;
                    last_album_idx = current_album;
                }
            }
            QueuePlaybackEffect::Stop => break,
            QueuePlaybackEffect::None => panic!("Unexpected None"),
            QueuePlaybackEffect::Reload(_) => panic!("Unexpected Reload"),
        }
    }

    // Should have transitioned through 2 album boundaries (0->1, 1->2)
    assert_eq!(album_transitions, 2);
}

// =============================================================================
// Sequence: Long Queue
// =============================================================================

/// Test very long queue navigation
#[test]
fn test_long_queue() {
    let mut queue = QueueController::new();
    for i in 0..100 {
        let _ = queue.add_album(album_with_tracks(&format!("Album{}", i), "Artist", 1));
    }
    queue.start();

    let mut count = 0;
    loop {
        let effect = queue.next_track();
        match effect {
            QueuePlaybackEffect::Play(_) => count += 1,
            QueuePlaybackEffect::Stop => break,
            QueuePlaybackEffect::None => panic!("Unexpected None"),
            QueuePlaybackEffect::Reload(_) => panic!("Unexpected Reload"),
        }
        if count > 150 {
            panic!("Infinite loop detected");
        }
    }

    // 100 single-track albums: start on first, so 99 next_track calls
    assert_eq!(count, 99);
}

// =============================================================================
// Sequence: Queue Operations (remove, clear, jump)
// =============================================================================

/// Test removing items from the queue
#[test]
fn test_queue_remove_during_playback() {
    let mut queue = QueueController::new();
    let _ = queue.add_album(album_with_tracks("A", "Artist", 2));
    let _ = queue.add_album(album_with_tracks("B", "Artist", 2));
    let _ = queue.add_album(album_with_tracks("C", "Artist", 2));
    queue.start();

    // Remove a non-current album (C)
    let (effect, was_current) = queue.remove(2);
    assert!(!was_current);
    assert!(matches!(effect, QueuePlaybackEffect::None));
    assert_eq!(queue.len(), 2);

    // Current index unchanged
    assert_eq!(queue.current_index(), Some(0));
}

/// Test clearing the queue
#[test]
fn test_queue_clear() {
    let mut queue = QueueController::new();
    let _ = queue.add_album(album_with_tracks("A", "Artist", 3));
    let _ = queue.add_album(album_with_tracks("B", "Artist", 3));
    queue.start();

    queue.clear();
    assert!(queue.is_empty());
    assert!(queue.current_index().is_none());
    assert!(queue.current_track().is_none());
}

/// Test jump_to specific album
#[test]
fn test_queue_jump_to() {
    let mut queue = QueueController::new();
    let _ = queue.add_album(album_with_tracks("A", "Artist", 3));
    let _ = queue.add_album(album_with_tracks("B", "Artist", 3));
    let _ = queue.add_album(album_with_tracks("C", "Artist", 3));
    queue.start();

    // Jump to album C (index 2)
    let effect = queue.jump_to(2);
    assert!(matches!(effect, QueuePlaybackEffect::Play(_)));
    assert_eq!(queue.current_index(), Some(2));

    // Should be on first track of C
    let track = queue.current_track().unwrap();
    assert!(track.title.as_ref().unwrap().contains("Track 1"));
}

/// Test jump_to invalid index
#[test]
fn test_queue_jump_to_invalid() {
    let mut queue = QueueController::new();
    let _ = queue.add_album(album_with_tracks("A", "Artist", 2));
    queue.start();

    let effect = queue.jump_to(99);
    assert!(matches!(effect, QueuePlaybackEffect::None));
    // Current position unchanged
    assert_eq!(queue.current_index(), Some(0));
}

// =============================================================================
// Sequence: Realistic Session
// =============================================================================

/// Simulate a realistic listening session with queue management
#[test]
fn test_realistic_queue_session() {
    let mut queue = QueueController::new();
    let _ = queue.add_album(album_with_tracks("Morning Jazz", "Various", 8));
    let _ = queue.add_album(album_with_tracks("Focus Music", "Study Beats", 12));
    let _ = queue.add_album(album_with_tracks("Evening Chill", "Lo-Fi", 6));
    queue.start();

    // Morning: listen to a few tracks
    queue.next_track();
    queue.next_track();
    queue.next_track();

    // Skip rest of first album to get to Focus Music
    while queue.current_index() == Some(0) {
        let effect = queue.next_track();
        if matches!(effect, QueuePlaybackEffect::Stop) {
            panic!("Should not hit Stop while skipping first album");
        }
    }

    assert_eq!(queue.current_index(), Some(1));

    // Listen to some focus tracks
    for _ in 0..5 {
        queue.next_track();
    }

    // Add a bonus album mid-session
    let _ = queue.add_album(album_with_tracks("Bonus", "Surprise", 3));
    assert_eq!(queue.len(), 4);

    // Continue through remaining tracks
    let mut remaining = 0;
    loop {
        let effect = queue.next_track();
        match effect {
            QueuePlaybackEffect::Play(_) => remaining += 1,
            QueuePlaybackEffect::Stop => break,
            QueuePlaybackEffect::None => panic!("Unexpected None"),
            QueuePlaybackEffect::Reload(_) => panic!("Unexpected Reload"),
        }
        if remaining > 50 {
            panic!("Infinite loop");
        }
    }

    // Verify we played through everything
    assert!(remaining > 0);
}
