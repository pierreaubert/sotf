//! Input sequence tests - input mode and keyboard workflows.
//!
//! Tests realistic input sequences including mode transitions, search typing,
//! and keybinding interactions.

use crate::common::state_builder::{TestAction, TestInputMode, TestInputState};

// =============================================================================
// Sequence: Search Input Workflows
// =============================================================================

/// Test complete search workflow: enter → type → execute → exit
#[test]
fn test_complete_search_workflow() {
    let mut state = TestInputState::default();
    assert_eq!(state.input_mode, TestInputMode::Normal);

    // Enter search mode with '/'
    state.process_key('/');
    assert_eq!(state.input_mode, TestInputMode::Search);
    assert!(state.search_query.is_empty());

    // Type search query
    for c in "Miles Davis".chars() {
        state.process_key(c);
    }
    assert_eq!(state.search_query, "Miles Davis");

    // Exit with escape
    state.process_key('\x1b');
    assert_eq!(state.input_mode, TestInputMode::Normal);
    assert!(state.search_query.is_empty(), "Escape should clear query");
}

/// Test search with backspace corrections
#[test]
fn test_search_with_corrections() {
    let mut state = TestInputState::default();
    state.enter_input_mode(TestInputMode::Search);

    // Type with mistakes
    for c in "Milse".chars() {
        state.process_key(c);
    }
    assert_eq!(state.search_query, "Milse");

    // Backspace to correct
    state.process_key('\x08');
    state.process_key('\x08');
    assert_eq!(state.search_query, "Mil");

    // Continue typing correctly
    for c in "es".chars() {
        state.process_key(c);
    }
    assert_eq!(state.search_query, "Miles");
}

/// Test search mode isolation throughout typing
#[test]
fn test_search_isolation_during_typing() {
    let mut state = TestInputState::default();
    state.enter_input_mode(TestInputMode::Search);

    // Type characters that are also keybindings in normal mode
    let dangerous_keys = ['0', '1', '2', '3', '4', '5', ' ', '+', '-'];

    for &key in &dangerous_keys {
        state.process_key(key);
    }

    // All should be in search query, none should trigger actions
    assert!(
        state.triggered_actions.is_empty(),
        "No actions should trigger"
    );
    assert_eq!(
        state.search_query, "012345 +-",
        "All keys should be in query"
    );
}

/// Test rapid search mode enter/exit
#[test]
fn test_rapid_search_mode_toggle() {
    let mut state = TestInputState::default();

    for i in 0..10 {
        // Enter search
        state.process_key('/');
        assert_eq!(state.input_mode, TestInputMode::Search, "Iteration {}", i);

        // Type something
        state.process_key('a');

        // Exit
        state.process_key('\x1b');
        assert_eq!(state.input_mode, TestInputMode::Normal, "Iteration {}", i);
        assert!(state.search_query.is_empty(), "Query should clear");
    }

    // No actions should have been triggered
    assert!(state.triggered_actions.is_empty());
}

// =============================================================================
// Sequence: Normal Mode Keybinding Workflows
// =============================================================================

/// Test rating filter workflow
#[test]
fn test_rating_filter_sequence() {
    let mut state = TestInputState::default();

    // Set various ratings
    for key in ['1', '2', '3', '4', '5', '0'] {
        state.process_key(key);
    }

    // Verify all actions were triggered
    assert_eq!(state.triggered_actions.len(), 6);
    assert_eq!(state.triggered_actions[0], TestAction::SetFilterRating(1));
    assert_eq!(state.triggered_actions[5], TestAction::SetFilterAll);
}

/// Test volume control workflow
#[test]
fn test_volume_control_sequence() {
    let mut state = TestInputState::default();

    // Increase volume several times
    for _ in 0..5 {
        state.process_key('+');
    }

    // Decrease volume
    for _ in 0..3 {
        state.process_key('-');
    }

    // Verify actions
    assert_eq!(state.triggered_actions.len(), 8);
    assert_eq!(
        state
            .triggered_actions
            .iter()
            .filter(|a| **a == TestAction::VolumeUp)
            .count(),
        5
    );
    assert_eq!(
        state
            .triggered_actions
            .iter()
            .filter(|a| **a == TestAction::VolumeDown)
            .count(),
        3
    );
}

/// Test play/pause workflow
#[test]
fn test_play_pause_sequence() {
    let mut state = TestInputState::default();

    // Toggle play/pause multiple times
    for _ in 0..5 {
        state.process_key(' ');
    }

    assert_eq!(state.triggered_actions.len(), 5);
    assert!(
        state
            .triggered_actions
            .iter()
            .all(|a| *a == TestAction::PlayPause)
    );
}

// =============================================================================
// Sequence: Mixed Mode Workflows
// =============================================================================

/// Test search interruption workflow
#[test]
fn test_search_interrupted_by_escape() {
    let mut state = TestInputState::default();

    // Start search
    state.process_key('/');
    for c in "Pink".chars() {
        state.process_key(c);
    }

    // User decides to cancel
    state.process_key('\x1b');

    // Now in normal mode, can use keybindings
    state.process_key(' ');
    assert_eq!(state.triggered_actions.len(), 1);
    assert_eq!(state.triggered_actions[0], TestAction::PlayPause);
}

/// Test workflow: search → normal → search
#[test]
fn test_alternating_search_normal() {
    let mut state = TestInputState::default();

    // First search
    state.process_key('/');
    for c in "Jazz".chars() {
        state.process_key(c);
    }
    state.process_key('\x1b');

    // Normal mode action
    state.process_key('3');
    assert_eq!(state.triggered_actions.len(), 1);

    // Second search
    state.process_key('/');
    for c in "Rock".chars() {
        state.process_key(c);
    }
    assert_eq!(state.search_query, "Rock");

    // Exit and do more normal actions
    state.process_key('\x1b');
    state.process_key('+');
    state.process_key('-');

    assert_eq!(state.triggered_actions.len(), 3);
}

/// Test unrecognized keys in normal mode
#[test]
fn test_unrecognized_keys_normal_mode() {
    let mut state = TestInputState::default();

    // Keys that aren't bound
    let unbound = ['a', 'b', 'c', 'x', 'y', 'z', '!', '@', '#'];

    for &key in &unbound {
        let consumed = state.process_key(key);
        assert!(!consumed, "Key '{}' should not be consumed", key);
    }

    // No actions triggered
    assert!(state.triggered_actions.is_empty());
}

// =============================================================================
// Sequence: Edge Cases
// =============================================================================

/// Test empty search (just enter and exit)
#[test]
fn test_empty_search_workflow() {
    let mut state = TestInputState::default();

    // Enter search, immediately exit
    state.process_key('/');
    state.process_key('\x1b');

    assert_eq!(state.input_mode, TestInputMode::Normal);
    assert!(state.search_query.is_empty());
    assert!(state.triggered_actions.is_empty());
}

/// Test backspace on empty search
#[test]
fn test_backspace_empty_search() {
    let mut state = TestInputState::default();
    state.enter_input_mode(TestInputMode::Search);

    // Backspace on empty query
    state.process_key('\x08');
    state.process_key('\x08');
    state.process_key('\x08');

    // Should not crash, query still empty
    assert!(state.search_query.is_empty());
    assert_eq!(state.input_mode, TestInputMode::Search);
}

/// Test special characters in search
#[test]
fn test_special_characters_search() {
    let mut state = TestInputState::default();
    state.enter_input_mode(TestInputMode::Search);

    // Various special characters
    let special = "Tool (10,000 Days) [2006] - #1 Album!";
    for c in special.chars() {
        if !c.is_control() {
            state.process_key(c);
        }
    }

    assert_eq!(state.search_query, special);
}

/// Test control characters are rejected in search
#[test]
fn test_control_chars_rejected() {
    let mut state = TestInputState::default();
    state.enter_input_mode(TestInputMode::Search);

    // Various control characters (except backspace and escape which are handled)
    let controls = ['\x00', '\x01', '\x02', '\x03', '\x04', '\x05'];

    for &c in &controls {
        state.process_key(c);
    }

    // Query should be empty (controls rejected)
    assert!(state.search_query.is_empty());
}

// =============================================================================
// Sequence: All Input Modes
// =============================================================================

/// Test entering and exiting each input mode
#[test]
fn test_all_input_mode_lifecycle() {
    let mut state = TestInputState::default();

    // Test each mode that can be entered
    let modes = [
        TestInputMode::Search,
        TestInputMode::AddDirectory,
        TestInputMode::SavePlugins,
        TestInputMode::LoadPlugins,
    ];

    for mode in modes {
        // Enter mode
        state.enter_input_mode(mode);
        assert_eq!(state.input_mode, mode);

        // Exit mode
        state.exit_input_mode();
        assert_eq!(state.input_mode, TestInputMode::Normal);
    }
}

/// Test mode state isolation
#[test]
fn test_mode_state_isolation() {
    let mut state = TestInputState::default();

    // Add some actions in normal mode
    state.process_key(' ');
    state.process_key('+');
    assert_eq!(state.triggered_actions.len(), 2);

    // Enter search mode
    state.process_key('/');

    // Actions should still be there (not cleared)
    assert_eq!(state.triggered_actions.len(), 2);

    // Type in search
    state.process_key('t');
    state.process_key('e');
    state.process_key('s');
    state.process_key('t');

    // No new actions
    assert_eq!(state.triggered_actions.len(), 2);
    assert_eq!(state.search_query, "test");
}

// =============================================================================
// Sequence: Complex Realistic Workflows
// =============================================================================

/// Simulate a realistic user session with mixed inputs
#[test]
fn test_realistic_input_session() {
    let mut state = TestInputState::default();

    // User starts playing music
    state.process_key(' ');

    // User adjusts volume
    state.process_key('+');
    state.process_key('+');

    // User searches for specific album
    state.process_key('/');
    for c in "Dark Side".chars() {
        state.process_key(c);
    }
    // User realizes typo, fixes it
    state.process_key('\x08');
    state.process_key('\x08');
    state.process_key('\x08');
    state.process_key('\x08');
    for c in "Side".chars() {
        state.process_key(c);
    }

    // Cancel search (will search differently)
    state.process_key('\x1b');

    // User filters by rating instead
    state.process_key('5');

    // User lowers volume for a call
    state.process_key('-');
    state.process_key('-');
    state.process_key('-');

    // Verify final state
    assert_eq!(state.input_mode, TestInputMode::Normal);
    assert!(state.search_query.is_empty());
    assert_eq!(state.triggered_actions.len(), 7); // play, +, +, rating, -, -, -
}

/// Test rapid key presses
#[test]
fn test_rapid_keypresses() {
    let mut state = TestInputState::default();

    // Rapid normal mode keys
    for _ in 0..100 {
        state.process_key('+');
    }

    assert_eq!(state.triggered_actions.len(), 100);

    // Rapid search typing
    state.triggered_actions.clear();
    state.enter_input_mode(TestInputMode::Search);

    let long_query = "a".repeat(1000);
    for c in long_query.chars() {
        state.process_key(c);
    }

    assert_eq!(state.search_query.len(), 1000);
    assert!(state.triggered_actions.is_empty());
}
