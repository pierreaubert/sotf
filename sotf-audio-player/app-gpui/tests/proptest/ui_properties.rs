//! Property-based tests for UI state and playback.

#[path = "../common/mod.rs"]
mod common;

use common::state_builder::{
    TestAction, TestAlbum, TestInputMode, TestInputState, TestPlaybackState, TestQueueItem,
    TestTrack,
};
use proptest::prelude::*;

// =============================================================================
// Strategies
// =============================================================================

/// Generate a valid volume value (may be out of range to test clamping)
fn volume_strategy() -> impl Strategy<Value = f32> {
    -1.0f32..2.0
}

/// Generate a seek position
fn seek_position_strategy() -> impl Strategy<Value = f64> {
    -10.0f64..500.0
}

/// Generate an input mode
fn input_mode_strategy() -> impl Strategy<Value = TestInputMode> {
    prop_oneof![
        Just(TestInputMode::Normal),
        Just(TestInputMode::Search),
        Just(TestInputMode::AddDirectory),
        Just(TestInputMode::SavePlugins),
        Just(TestInputMode::LoadPlugins),
    ]
}

/// Generate a key that might conflict with keybindings
fn key_strategy() -> impl Strategy<Value = char> {
    prop_oneof![
        Just('0'),
        Just('1'),
        Just('2'),
        Just('3'),
        Just('4'),
        Just('5'),
        Just(' '),
        Just('+'),
        Just('-'),
        Just('/'),
        Just('\x1b'), // Escape
        Just('\x08'), // Backspace
        "[a-zA-Z]".prop_map(|s| s.chars().next().unwrap()),
    ]
}

/// Generate a test action
fn action_strategy() -> impl Strategy<Value = TestAction> {
    prop_oneof![
        Just(TestAction::PlayPause),
        Just(TestAction::NextTrack),
        Just(TestAction::PrevTrack),
        volume_strategy().prop_map(TestAction::SetVolume),
        seek_position_strategy().prop_map(TestAction::SeekTo),
        Just(TestAction::ToggleSearch),
        Just(TestAction::ClearSearch),
        "[a-z]{1,5}".prop_map(TestAction::TypeInSearch),
    ]
}

/// Generate a track list
fn track_list_strategy() -> impl Strategy<Value = Vec<TestTrack>> {
    prop::collection::vec(
        (1u32..1000).prop_map(|i| TestTrack::new(&format!("track{}.flac", i))),
        1..10,
    )
}

/// Generate a queue
fn queue_strategy() -> impl Strategy<Value = Vec<TestQueueItem>> {
    prop::collection::vec(
        track_list_strategy().prop_map(|tracks| {
            let album = TestAlbum::new("Album", "Artist").with_tracks(tracks);
            TestQueueItem::from_album(album)
        }),
        1..5,
    )
}

// =============================================================================
// Property Tests
// =============================================================================

proptest! {
    /// INVARIANT: Volume is always clamped to [0.0, 1.0]
    #[test]
    fn volume_always_clamped(volume in volume_strategy()) {
        let mut state = TestPlaybackState::default();
        state.set_volume(volume);

        prop_assert!(
            state.volume >= 0.0 && state.volume <= 1.0,
            "Volume {} not clamped to [0, 1]: got {}",
            volume, state.volume
        );
    }

    /// INVARIANT: Volume is preserved after next_track()
    #[test]
    fn volume_preserved_after_next_track(
        volume in 0.0f32..1.0,
        queue in queue_strategy()
    ) {
        let mut state = TestPlaybackState::default()
            .with_volume(volume)
            .with_queue(queue);
        state.is_playing = true;

        // Try multiple next_track calls
        for _ in 0..5 {
            state.next_track();

            prop_assert_eq!(
                state.volume, volume,
                "Volume changed from {} to {} after next_track",
                volume, state.volume
            );
        }
    }

    /// INVARIANT: Seek position is clamped to [0, duration]
    #[test]
    fn seek_position_clamped(
        position in seek_position_strategy(),
        duration in 60.0f64..600.0
    ) {
        let album = TestAlbum::new("Album", "Artist")
            .with_tracks(vec![TestTrack::new("track.flac").with_duration(duration)]);
        let mut state = TestPlaybackState::default()
            .with_queue(vec![TestQueueItem::from_album(album)]);
        state.duration_secs = duration;

        let _ = state.seek_to(position);

        prop_assert!(
            state.position_secs >= 0.0 && state.position_secs <= duration,
            "Position {} not clamped to [0, {}]: got {}",
            position, duration, state.position_secs
        );
    }

    /// INVARIANT: InputMode transitions are reversible
    #[test]
    fn input_mode_reversible(mode in input_mode_strategy()) {
        let mut state = TestInputState::default();

        // Enter mode
        state.enter_input_mode(mode);
        prop_assert_eq!(state.input_mode, mode);

        // Exit mode
        state.exit_input_mode();
        prop_assert_eq!(
            state.input_mode,
            TestInputMode::Normal,
            "Exiting {:?} didn't return to Normal",
            mode
        );
    }

    /// INVARIANT: Search mode consumes all non-control keys
    #[test]
    fn search_mode_consumes_keys(key in key_strategy()) {
        let mut state = TestInputState::default();
        state.enter_input_mode(TestInputMode::Search);

        let consumed = state.process_key(key);

        prop_assert!(
            consumed,
            "Key {:?} was not consumed in search mode",
            key
        );
    }

    /// INVARIANT: Normal mode triggers global actions for bound keys
    #[test]
    fn normal_mode_triggers_actions(key in key_strategy()) {
        let mut state = TestInputState::default();

        let consumed = state.process_key(key);

        // Certain keys should trigger actions in normal mode
        let should_trigger = matches!(key, '0'..='5' | ' ' | '+' | '-' | '=' | '_' | '/');

        if should_trigger {
            prop_assert!(
                !state.triggered_actions.is_empty() || state.input_mode == TestInputMode::Search,
                "Key {:?} should trigger action or mode change in normal mode",
                key
            );
        }
    }

    /// INVARIANT: Search mode does NOT trigger global actions
    #[test]
    fn search_mode_blocks_actions(key in key_strategy()) {
        let mut state = TestInputState::default();
        state.enter_input_mode(TestInputMode::Search);

        state.process_key(key);

        // Check that no global actions were triggered
        let action_triggered = !state.triggered_actions.is_empty();

        prop_assert!(
            !action_triggered,
            "Key {:?} triggered action {:?} in search mode",
            key, state.triggered_actions
        );
    }

    /// INVARIANT: Escape in search mode clears query and exits
    #[test]
    fn escape_clears_and_exits_search(query in "[a-zA-Z0-9]{0,20}") {
        let mut state = TestInputState::default();
        state.enter_input_mode(TestInputMode::Search);
        state.search_query = query;

        state.process_key('\x1b'); // Escape

        prop_assert_eq!(state.input_mode, TestInputMode::Normal);
        prop_assert!(state.search_query.is_empty(), "Search query not cleared");
    }

    /// INVARIANT: Backspace removes exactly one character
    #[test]
    fn backspace_removes_one_char(query in "[a-zA-Z]{1,20}") {
        let mut state = TestInputState::default();
        state.enter_input_mode(TestInputMode::Search);
        state.search_query = query.clone();

        let len_before = state.search_query.len();
        state.process_key('\x08'); // Backspace
        let len_after = state.search_query.len();

        prop_assert_eq!(
            len_after, len_before - 1,
            "Backspace should remove exactly one character"
        );
    }

    /// INVARIANT: Queue position is always valid or None
    #[test]
    fn queue_index_always_valid(queue in queue_strategy()) {
        let mut state = TestPlaybackState::default().with_queue(queue);
        state.is_playing = true;

        // Advance through queue
        for _ in 0..20 {
            state.next_track();

            if let Some(idx) = state.current_queue_index {
                prop_assert!(
                    idx < state.queue.len(),
                    "Queue index {} exceeds queue length {}",
                    idx, state.queue.len()
                );
            }
        }
    }

    /// INVARIANT: Playing state becomes false when queue exhausted
    #[test]
    fn playing_stops_at_queue_end(queue in queue_strategy()) {
        let mut state = TestPlaybackState::default().with_queue(queue);
        state.is_playing = true;

        // Exhaust queue
        while state.next_track().is_some() {}

        // At queue end, should stop playing
        prop_assert!(
            !state.is_playing,
            "Should stop playing at queue end"
        );
    }
}
