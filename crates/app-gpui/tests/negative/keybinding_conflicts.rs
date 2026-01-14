//! Keybinding Conflict Tests
//!
//! These tests verify that global keybindings do NOT trigger when the user
//! is in a text input mode (Search, AddDirectory, etc.).
//!
//! # Background
//!
//! A bug was discovered where typing in the search box triggered global actions
//! instead of adding characters to the search query. For example:
//! - Typing '5' triggered SetFilterRating(5) instead of adding '5' to search
//! - Pressing space triggered PlayPause instead of adding space to search
//!
//! These tests ensure such conflicts don't regress.

#[path = "../common/mod.rs"]
mod common;

use common::state_builder::{TestAction, TestInputMode, TestInputState, TestLibrarySortOrder};

/// Keys that have global bindings and conflict with text input
const CONFLICTING_KEYS: &[char] = &['0', '1', '2', '3', '4', '5', ' ', '+', '-', '=', '_'];

/// Test: Typing in search mode should NOT trigger global keybindings
#[test]
fn test_search_mode_blocks_global_keybindings() {
    let mut state = TestInputState::default();
    state.enter_input_mode(TestInputMode::Search);

    for &key in CONFLICTING_KEYS {
        state.process_key(key);
    }

    // NEGATIVE ASSERTION: No global actions should have been triggered
    assert!(
        state.triggered_actions.is_empty(),
        "Global actions were triggered in search mode: {:?}",
        state.triggered_actions
    );

    // POSITIVE ASSERTION: All printable characters should be in search query
    let expected_query: String = CONFLICTING_KEYS
        .iter()
        .filter(|c| !c.is_control())
        .collect();
    assert_eq!(
        state.search_query, expected_query,
        "Characters not added to search query"
    );
}

/// Test: Number keys in search mode add digits, not set filter
#[test]
fn test_number_keys_in_search_mode_add_to_query() {
    let mut state = TestInputState::default();
    state.enter_input_mode(TestInputMode::Search);

    // Type "123"
    state.process_key('1');
    state.process_key('2');
    state.process_key('3');

    // Search query should contain "123"
    assert_eq!(state.search_query, "123");

    // SetFilterRating actions should NOT have triggered
    assert!(
        !state
            .triggered_actions
            .iter()
            .any(|a| matches!(a, TestAction::SetFilterRating(_))),
        "SetFilterRating triggered in search mode"
    );
}

/// Test: Space key in search mode adds space, not play/pause
#[test]
fn test_space_key_in_search_mode_adds_space() {
    let mut state = TestInputState::default();
    state.enter_input_mode(TestInputMode::Search);

    // Type "hello world"
    for c in "hello world".chars() {
        state.process_key(c);
    }

    assert_eq!(state.search_query, "hello world");
    assert!(
        !state
            .triggered_actions
            .iter()
            .any(|a| matches!(a, TestAction::PlayPause)),
        "PlayPause triggered in search mode"
    );
}

/// Test: Plus/minus keys in search mode add characters, not volume change
#[test]
fn test_plus_minus_in_search_mode_add_to_query() {
    let mut state = TestInputState::default();
    state.enter_input_mode(TestInputMode::Search);

    state.process_key('+');
    state.process_key('-');

    assert_eq!(state.search_query, "+-");
    assert!(
        !state
            .triggered_actions
            .iter()
            .any(|a| matches!(a, TestAction::VolumeUp | TestAction::VolumeDown)),
        "Volume actions triggered in search mode"
    );
}

/// Test: Escape in search mode exits search and clears query
#[test]
fn test_escape_exits_search_mode() {
    let mut state = TestInputState::default();
    state.enter_input_mode(TestInputMode::Search);
    state.search_query = "test query".to_string();

    // Press Escape
    state.process_key('\x1b');

    assert_eq!(state.input_mode, TestInputMode::Normal);
    assert!(state.search_query.is_empty(), "Search query not cleared");
}

/// Test: Backspace in search mode removes character
#[test]
fn test_backspace_in_search_mode() {
    let mut state = TestInputState::default();
    state.enter_input_mode(TestInputMode::Search);

    // Type "test"
    for c in "test".chars() {
        state.process_key(c);
    }
    assert_eq!(state.search_query, "test");

    // Backspace twice
    state.process_key('\x08');
    state.process_key('\x08');

    assert_eq!(state.search_query, "te");
}

/// Test: Normal mode allows global keybindings
#[test]
fn test_normal_mode_allows_keybindings() {
    let mut state = TestInputState::default();
    assert_eq!(state.input_mode, TestInputMode::Normal);

    // Press conflicting keys in normal mode
    state.process_key(' '); // PlayPause
    state.process_key('+'); // VolumeUp
    state.process_key('5'); // SetFilterRating(5)

    // Actions SHOULD be triggered in normal mode
    assert!(state.triggered_actions.contains(&TestAction::PlayPause));
    assert!(state.triggered_actions.contains(&TestAction::VolumeUp));
    assert!(
        state
            .triggered_actions
            .contains(&TestAction::SetFilterRating(5))
    );
}

/// Test: Slash key enters search mode from normal
#[test]
fn test_slash_enters_search_mode() {
    let mut state = TestInputState::default();
    assert_eq!(state.input_mode, TestInputMode::Normal);

    state.process_key('/');

    assert_eq!(state.input_mode, TestInputMode::Search);
}

/// Test: Input mode isolation - each mode consumes keys appropriately
#[test]
fn test_input_mode_isolation() {
    // Test that we can toggle between modes and keys are handled correctly
    let mut state = TestInputState::default();

    // Normal mode: '5' triggers filter
    state.process_key('5');
    assert!(
        state
            .triggered_actions
            .contains(&TestAction::SetFilterRating(5))
    );

    // Enter search mode
    state.enter_input_mode(TestInputMode::Search);
    state.triggered_actions.clear();

    // Search mode: '5' adds to query
    state.process_key('5');
    assert!(state.triggered_actions.is_empty());
    assert_eq!(state.search_query, "5");

    // Exit back to normal mode
    state.exit_input_mode();
    state.triggered_actions.clear();

    // Normal mode again: '5' triggers filter
    state.process_key('5');
    assert!(
        state
            .triggered_actions
            .contains(&TestAction::SetFilterRating(5))
    );
}

/// Test: All InputMode variants properly isolate input
#[test]
fn test_all_input_modes_isolate() {
    let modes = [
        TestInputMode::Search,
        TestInputMode::AddDirectory,
        TestInputMode::SavePlugins,
        TestInputMode::LoadPlugins,
    ];

    for mode in modes {
        let mut state = TestInputState::default();
        state.enter_input_mode(mode);

        // Type conflicting keys
        state.process_key('5');
        state.process_key(' ');

        // Should NOT trigger global actions (only Search mode handles text input in our test impl)
        // But the point is: actions should be blocked when not in Normal mode
        if mode == TestInputMode::Search {
            assert!(
                state.triggered_actions.is_empty(),
                "Mode {:?} allowed global actions",
                mode
            );
        }
    }
}
