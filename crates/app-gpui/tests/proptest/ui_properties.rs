//! Property-based tests for Queue, QueueController, and PlaybackController.
//!
//! Tests the real types from `sotf-player` with generated data.

use proptest::prelude::*;
use sotf_audio_player::{
    Album, PlaybackController, Queue, QueueController, QueuePlaybackEffect, Track,
};
use std::path::PathBuf;

// =============================================================================
// Strategies
// =============================================================================

fn make_album(title: &str, track_count: usize) -> Album {
    Album {
        title: title.to_string(),
        tracks: (0..track_count)
            .map(|i| Track {
                path: PathBuf::from(format!("/music/{}/track_{}.flac", title, i + 1)),
                title: Some(format!("Track {}", i + 1)),
                duration_secs: Some(180),
                channels: Some(2),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

fn album_strategy() -> impl Strategy<Value = Album> {
    (1usize..10, 0u32..1000)
        .prop_map(|(track_count, id)| make_album(&format!("Album_{}", id), track_count))
}

fn queue_albums_strategy() -> impl Strategy<Value = Vec<Album>> {
    prop::collection::vec(album_strategy(), 1..8)
}

fn volume_strategy() -> impl Strategy<Value = f32> {
    -1.0f32..2.0
}

// =============================================================================
// Queue Property Tests
// =============================================================================

proptest! {
    /// INVARIANT: Queue current_index is always valid or None.
    #[test]
    fn queue_index_always_valid(
        albums in queue_albums_strategy(),
        ops in prop::collection::vec(0u8..3, 1..30)
    ) {
        let mut queue = Queue::new();
        for album in &albums {
            queue.add(album.clone());
        }
        queue.start();

        for op in ops {
            match op {
                0 => { queue.next_track(); }
                1 => { queue.previous_track(); }
                _ => {
                    if !queue.is_empty() {
                        queue.remove(0);
                    }
                }
            }

            if let Some(idx) = queue.current_index {
                prop_assert!(
                    idx < queue.len(),
                    "current_index {} >= queue.len() {}",
                    idx,
                    queue.len()
                );
            }
        }
    }

    /// INVARIANT: Exhausting queue via next_track returns None at the end.
    #[test]
    fn queue_exhaustion(albums in queue_albums_strategy()) {
        let total_tracks: usize = albums.iter().map(|a| a.tracks.len()).sum();

        let mut queue = Queue::new();
        for album in &albums {
            queue.add(album.clone());
        }
        queue.start();

        let mut steps = 0;
        while queue.next_track().is_some() {
            steps += 1;
            prop_assert!(
                steps < total_tracks + 1,
                "Queue didn't terminate after {} steps (expected max {})",
                steps,
                total_tracks
            );
        }

        // We started at track 1, so advancing (total-1) times reaches the end
        prop_assert_eq!(steps, total_tracks - 1);
    }

    /// INVARIANT: Removing an item before current_index adjusts the index correctly.
    #[test]
    fn remove_before_current_adjusts_index(album_count in 3usize..10) {
        let mut queue = Queue::new();
        for i in 0..album_count {
            queue.add(make_album(&format!("Album_{}", i), 1));
        }
        queue.start();

        // Advance to last album
        for _ in 0..album_count - 1 {
            queue.next_track();
        }
        prop_assert_eq!(queue.current_index, Some(album_count - 1));

        // Remove first album -- current should shift down
        let was_current = queue.remove(0);
        prop_assert!(!was_current);
        prop_assert_eq!(queue.current_index, Some(album_count - 2));
        prop_assert_eq!(queue.len(), album_count - 1);
    }

    /// INVARIANT: jump_to always lands on the first track of the target album.
    #[test]
    fn jump_to_resets_track_index(
        albums in queue_albums_strategy(),
        target in 0usize..8
    ) {
        let mut queue = Queue::new();
        for album in &albums {
            queue.add(album.clone());
        }
        queue.start();

        // Advance some tracks in the first album
        queue.next_track();

        let target = target % queue.len();
        let path = queue.jump_to(target);

        prop_assert!(path.is_some());
        prop_assert_eq!(queue.current_index, Some(target));

        // Verify it's the first track
        let item = &queue.items[target];
        prop_assert_eq!(item.current_track_index, 0);
    }

    /// INVARIANT: clear() leaves queue empty with no current index.
    #[test]
    fn clear_empties_queue(albums in queue_albums_strategy()) {
        let mut queue = Queue::new();
        for album in &albums {
            queue.add(album.clone());
        }
        queue.start();

        queue.clear();
        prop_assert!(queue.is_empty());
        prop_assert_eq!(queue.current_index, None);
    }
}

// =============================================================================
// QueueController Property Tests
// =============================================================================

proptest! {
    /// INVARIANT: QueueController.next_track returns Stop when queue is exhausted.
    #[test]
    fn controller_stops_at_end(albums in queue_albums_strategy()) {
        let mut ctrl = QueueController::new();
        for album in &albums {
            ctrl.add_album(album.clone());
        }
        ctrl.start();

        let total_tracks: usize = albums.iter().map(|a| a.tracks.len()).sum();
        for _ in 0..total_tracks {
            let effect = ctrl.next_track();
            if matches!(effect, QueuePlaybackEffect::Stop) {
                break;
            }
        }

        // One more should definitely be Stop
        let effect = ctrl.next_track();
        prop_assert_eq!(effect, QueuePlaybackEffect::Stop);
    }

    /// INVARIANT: play_album_now always returns Play effect and jumps to the new album.
    #[test]
    fn play_now_jumps_to_new_album(
        initial_albums in prop::collection::vec(album_strategy(), 1..4),
        new_album in album_strategy()
    ) {
        let mut ctrl = QueueController::new();
        for album in &initial_albums {
            ctrl.add_album(album.clone());
        }
        ctrl.start();

        let expected_index = ctrl.len();
        let effect = ctrl.play_album_now(new_album);

        prop_assert!(matches!(effect, QueuePlaybackEffect::Play(_)));
        prop_assert_eq!(ctrl.current_index(), Some(expected_index));
    }

    /// INVARIANT: Removing all items produces Stop effect.
    #[test]
    fn removing_all_stops(albums in queue_albums_strategy()) {
        let mut ctrl = QueueController::new();
        for album in &albums {
            ctrl.add_album(album.clone());
        }
        ctrl.start();

        let mut got_stop = false;
        while !ctrl.is_empty() {
            let (effect, _) = ctrl.remove(0);
            if effect == QueuePlaybackEffect::Stop {
                got_stop = true;
            }
        }

        prop_assert!(got_stop, "Never got Stop effect after removing all items");
        prop_assert!(ctrl.is_empty());
    }
}

// =============================================================================
// PlaybackController Property Tests
// =============================================================================

proptest! {
    /// INVARIANT: Volume is always clamped to [0.0, 1.0].
    #[test]
    fn volume_always_clamped(volume in volume_strategy()) {
        let mut ctrl = PlaybackController::new();
        ctrl.set_volume(volume);

        prop_assert!(
            ctrl.volume >= 0.0 && ctrl.volume <= 1.0,
            "Volume {} not clamped: got {}",
            volume,
            ctrl.volume
        );
    }

    /// INVARIANT: Effective volume is 0 when muted, regardless of volume setting.
    #[test]
    fn muted_effective_volume_zero(volume in 0.0f32..1.0) {
        let mut ctrl = PlaybackController::new();
        ctrl.set_volume(volume);
        ctrl.toggle_mute();

        prop_assert_eq!(ctrl.effective_volume(), 0.0);
    }

    /// INVARIANT: increase_volume + decrease_volume is approximately identity.
    #[test]
    fn volume_increase_decrease_roundtrip(volume in 0.1f32..0.9) {
        let mut ctrl = PlaybackController::new();
        ctrl.set_volume(volume);
        let initial = ctrl.volume;

        ctrl.increase_volume();
        ctrl.decrease_volume();

        let diff = (ctrl.volume - initial).abs();
        prop_assert!(
            diff < 0.001,
            "Volume roundtrip drift: {} -> {}",
            initial,
            ctrl.volume
        );
    }

    /// INVARIANT: toggle_mute is its own inverse.
    #[test]
    fn toggle_mute_inverse(_dummy in 0u8..1) {
        let mut ctrl = PlaybackController::new();
        prop_assert!(!ctrl.muted);

        ctrl.toggle_mute();
        prop_assert!(ctrl.muted);

        ctrl.toggle_mute();
        prop_assert!(!ctrl.muted);
    }

    /// INVARIANT: Repeated increase_volume never exceeds 1.0.
    #[test]
    fn volume_increase_bounded(steps in 1usize..100) {
        let mut ctrl = PlaybackController::new();
        ctrl.set_volume(0.5);

        for _ in 0..steps {
            ctrl.increase_volume();
            prop_assert!(ctrl.volume <= 1.0, "Volume exceeded 1.0: {}", ctrl.volume);
        }
    }

    /// INVARIANT: Repeated decrease_volume never goes below 0.0.
    #[test]
    fn volume_decrease_bounded(steps in 1usize..100) {
        let mut ctrl = PlaybackController::new();
        ctrl.set_volume(0.5);

        for _ in 0..steps {
            ctrl.decrease_volume();
            prop_assert!(ctrl.volume >= 0.0, "Volume below 0.0: {}", ctrl.volume);
        }
    }
}
