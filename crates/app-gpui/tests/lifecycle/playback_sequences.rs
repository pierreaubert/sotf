//! Playback sequence tests - multi-song playback workflows.
//!
//! Tests realistic playback scenarios that span multiple tracks and albums,
//! verifying state preservation throughout the playback lifecycle.
//! Uses real `PlaybackController` and `QueueController` from `sotf_audio_player`.

use crate::common::factories::album_with_tracks;
use sotf_audio_player::controllers::playback::PlaybackController;
use sotf_audio_player::controllers::queue::{QueueController, QueuePlaybackEffect};

// =============================================================================
// Sequence: Full Album Playback
// =============================================================================

/// Simulate playing through an entire album track by track.
/// Volume on PlaybackController must remain untouched by queue navigation.
#[test]
fn test_full_album_playback_preserves_volume() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.42);

    let mut queue = QueueController::new();
    queue.add_album(album_with_tracks("TestAlbum", "Artist", 5));
    queue.start();

    for i in 0..4 {
        let effect = queue.next_track();
        assert!(
            matches!(effect, QueuePlaybackEffect::Play(_)),
            "Expected Play effect at track {}",
            i + 2
        );
        assert_eq!(playback.volume, 0.42, "Volume changed on track {}", i + 2);
    }

    // After last track, queue returns Stop
    let effect = queue.next_track();
    assert_eq!(effect, QueuePlaybackEffect::Stop);

    // Volume preserved even after queue ends
    assert_eq!(playback.volume, 0.42);
}

/// Simulate playing through multiple albums
#[test]
fn test_multi_album_playback_preserves_state() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.75);

    let mut queue = QueueController::new();
    queue.add_album(album_with_tracks("Album1", "Artist", 3));
    queue.add_album(album_with_tracks("Album2", "Artist", 2));
    queue.add_album(album_with_tracks("Album3", "Artist", 4));
    queue.start();

    let expected_total_tracks = 3 + 2 + 4;
    let mut tracks_played = 0;

    loop {
        let effect = queue.next_track();
        match effect {
            QueuePlaybackEffect::Play(_) => {
                tracks_played += 1;
                assert_eq!(
                    playback.volume, 0.75,
                    "Volume changed after {} tracks",
                    tracks_played
                );
            }
            QueuePlaybackEffect::Stop => break,
            QueuePlaybackEffect::None => panic!("Unexpected None effect"),
        }

        if tracks_played > expected_total_tracks + 1 {
            panic!("Too many tracks played - possible infinite loop");
        }
    }

    // Started on track 1, so next_track calls = total - 1
    assert_eq!(tracks_played, expected_total_tracks - 1);
    assert_eq!(playback.volume, 0.75);
}

// =============================================================================
// Sequence: Interrupted Playback
// =============================================================================

/// Pause/resume on PlaybackController doesn't affect volume
#[test]
fn test_pause_resume_preserves_volume() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.33);
    playback.is_playing = true;

    let mut queue = QueueController::new();
    queue.add_album(album_with_tracks("Album", "Artist", 3));
    queue.start();

    // Pause
    playback.is_playing = false;
    assert_eq!(playback.volume, 0.33);

    // Resume
    playback.is_playing = true;
    assert_eq!(playback.volume, 0.33);

    // Next track
    queue.next_track();
    assert_eq!(playback.volume, 0.33);
}

// =============================================================================
// Sequence: Volume Adjustments During Playback
// =============================================================================

/// Volume adjustments persist across track changes
#[test]
fn test_volume_adjustments_persist_across_tracks() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.5);

    let mut queue = QueueController::new();
    queue.add_album(album_with_tracks("Album1", "Artist", 2));
    queue.add_album(album_with_tracks("Album2", "Artist", 2));
    queue.start();

    // Change volume
    playback.set_volume(0.25);

    // Play next track
    queue.next_track();
    assert_eq!(playback.volume, 0.25);

    // Change volume again
    playback.set_volume(0.80);

    // Cross album boundary
    queue.next_track(); // Album2, track 1
    assert_eq!(playback.volume, 0.80);

    queue.next_track(); // Album2, track 2
    assert_eq!(playback.volume, 0.80);
}

/// Edge volume values persist across track changes
#[test]
fn test_edge_volume_values_persist() {
    let mut playback = PlaybackController::new();
    let mut queue = QueueController::new();
    queue.add_album(album_with_tracks("Album", "Artist", 5));
    queue.start();

    // Volume = 0.0
    playback.set_volume(0.0);
    queue.next_track();
    assert_eq!(playback.volume, 0.0, "Zero volume not preserved");

    // Volume = 1.0
    playback.set_volume(1.0);
    queue.next_track();
    assert_eq!(playback.volume, 1.0, "Max volume not preserved");

    // Very small volume
    playback.set_volume(0.001);
    queue.next_track();
    assert!(
        (playback.volume - 0.001).abs() < f32::EPSILON,
        "Small volume not preserved"
    );
}

// =============================================================================
// Sequence: Mute State
// =============================================================================

/// Mute state persists through track changes
#[test]
fn test_mute_persists_through_playback() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.6);
    playback.muted = true;

    let mut queue = QueueController::new();
    queue.add_album(album_with_tracks("Album", "Artist", 4));
    queue.start();

    for _ in 0..3 {
        queue.next_track();
        assert!(playback.muted, "Mute state changed on track change");
        assert_eq!(playback.volume, 0.6, "Volume changed while muted");
    }

    // Unmute
    playback.muted = false;
    queue.next_track(); // This is Stop, but mute is on PlaybackController
    assert!(!playback.muted, "Unmute didn't persist");
    assert_eq!(playback.volume, 0.6, "Volume changed after unmute");
}

/// Mute does not affect stored volume
#[test]
fn test_mute_independence_from_volume() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.7);
    playback.muted = true;

    assert_eq!(playback.effective_volume(), 0.0, "Effective should be 0 when muted");
    assert_eq!(playback.volume, 0.7, "Stored volume should be unchanged");

    playback.muted = false;
    assert_eq!(playback.effective_volume(), 0.7, "Effective should restore after unmute");
}

// =============================================================================
// Sequence: Rapid Operations
// =============================================================================

/// Rapid next track operations preserve volume
#[test]
fn test_rapid_track_skipping() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.55);

    let mut queue = QueueController::new();
    queue.add_album(album_with_tracks("Album1", "Artist", 10));
    queue.add_album(album_with_tracks("Album2", "Artist", 10));
    queue.start();

    for i in 0..15 {
        let effect = queue.next_track();
        assert_eq!(playback.volume, 0.55, "Volume changed after rapid skip {}", i + 1);
        if matches!(effect, QueuePlaybackEffect::Stop) {
            break;
        }
    }
}

/// Alternating play/pause/next preserves volume
#[test]
fn test_alternating_play_pause_next() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.7);

    let mut queue = QueueController::new();
    queue.add_album(album_with_tracks("Album", "Artist", 5));
    queue.start();

    for _ in 0..3 {
        playback.is_playing = true;
        assert_eq!(playback.volume, 0.7);

        playback.is_playing = false;
        assert_eq!(playback.volume, 0.7);

        queue.next_track();
        assert_eq!(playback.volume, 0.7);
    }
}

// =============================================================================
// Sequence: Volume Control Methods
// =============================================================================

/// increase_volume and decrease_volume work correctly
#[test]
fn test_volume_step_operations() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.5);

    playback.increase_volume();
    assert!(playback.volume > 0.5, "Volume should increase");
    let after_increase = playback.volume;

    playback.decrease_volume();
    assert!(
        (playback.volume - 0.5).abs() < 0.01,
        "Volume should return near 0.5 after increase+decrease"
    );

    // Edge: increasing at max
    playback.set_volume(1.0);
    playback.increase_volume();
    assert_eq!(playback.volume, 1.0, "Volume should clamp at 1.0");

    // Edge: decreasing at min
    playback.set_volume(0.0);
    playback.decrease_volume();
    assert_eq!(playback.volume, 0.0, "Volume should clamp at 0.0");

    let _ = after_increase; // suppress unused warning
}

// =============================================================================
// Sequence: Complete Workflow Simulation
// =============================================================================

/// Simulate a realistic listening session using both controllers
#[test]
fn test_realistic_listening_session() {
    let mut playback = PlaybackController::new();
    playback.set_volume(0.4);

    let mut queue = QueueController::new();
    queue.add_album(album_with_tracks("Morning Jazz", "Various", 6));
    queue.add_album(album_with_tracks("Work Focus", "Study Beats", 8));
    queue.add_album(album_with_tracks("Evening Chill", "Lo-Fi", 5));
    queue.start();
    playback.is_playing = true;

    // Listen to a few tracks
    queue.next_track();
    queue.next_track();

    // Lower volume for a call
    playback.set_volume(0.1);
    playback.muted = true;
    playback.is_playing = false;

    // Resume after call
    playback.muted = false;
    playback.is_playing = true;

    // Skip rest of first album to get to second album
    while queue.current_index() == Some(0) {
        let effect = queue.next_track();
        if matches!(effect, QueuePlaybackEffect::Stop) {
            panic!("Should not hit Stop while skipping first album");
        }
    }

    // Should now be on second album
    assert_eq!(queue.current_index(), Some(1));

    // Increase volume for focus
    playback.set_volume(0.6);

    // Skip through some work album tracks
    for _ in 0..5 {
        queue.next_track();
    }

    // Final state verification
    assert_eq!(playback.volume, 0.6);
    assert!(playback.is_playing);
    assert!(!playback.muted);
}
