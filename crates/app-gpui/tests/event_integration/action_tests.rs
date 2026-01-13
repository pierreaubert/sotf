//! Action dispatch integration tests.
//!
//! These tests verify that actions are properly dispatched through the
//! GPUI action system and that handlers receive the correct events.

use super::test_view::*;
use gpui::*;

// =============================================================================
// Action Sequence Tests
// =============================================================================

/// Test a realistic user workflow
#[gpui::test]
async fn test_realistic_workflow(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // User starts playing music
    sim.keystroke("space");
    assert!(sim.is_playing(), "Should be playing");

    // User adjusts volume
    sim.keystroke("+");
    sim.keystroke("+");
    let volume_after_up = sim.volume();

    // User wants to search - enters search mode
    sim.keystroke("/");
    assert_eq!(sim.input_mode(), TestInputMode::Search);

    // While in search mode, accidental space shouldn't toggle playback
    sim.clear_actions();
    sim.keystroke("space");
    assert!(
        !sim.action_triggered("PlayPause"),
        "Space in search mode should NOT trigger PlayPause"
    );

    // User searches for something
    sim.type_text("jazz");

    // User cancels search
    sim.keystroke("escape");
    assert_eq!(sim.input_mode(), TestInputMode::Normal);

    // Volume and playback should be preserved
    assert!(sim.is_playing(), "Should still be playing after search");
    assert_eq!(
        sim.volume(),
        volume_after_up,
        "Volume should be preserved after search"
    );

    // User filters by rating
    sim.keystroke("4");
    assert_eq!(sim.state().filter_rating, Some(4));
}

/// Test that multiple actions in sequence work correctly
#[gpui::test]
async fn test_action_sequence(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Perform a sequence of actions
    sim.keystroke("space"); // Play
    sim.keystroke("n");     // Next track
    sim.keystroke("n");     // Next track
    sim.keystroke("+");     // Volume up
    sim.keystroke("-");     // Volume down
    sim.keystroke("p");     // Previous track
    sim.keystroke("space"); // Pause

    // Verify all actions were recorded
    let actions = sim.actions();
    assert_eq!(actions.len(), 7, "Should have 7 actions");
    assert_eq!(actions[0], "PlayPause");
    assert_eq!(actions[1], "NextTrack");
    assert_eq!(actions[2], "NextTrack");
    assert_eq!(actions[3], "VolumeUp");
    assert_eq!(actions[4], "VolumeDown");
    assert_eq!(actions[5], "PrevTrack");
    assert_eq!(actions[6], "PlayPause");
}

/// Test all filter keys
#[gpui::test]
async fn test_all_filter_keys(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Test each filter rating
    for (key, expected_rating) in [("1", Some(1)), ("2", Some(2)), ("3", Some(3)), ("4", Some(4)), ("5", Some(5)), ("0", None)] {
        sim.keystroke(key);
        assert_eq!(
            sim.state().filter_rating,
            expected_rating,
            "Key '{}' should set filter to {:?}",
            key,
            expected_rating
        );
    }
}

// =============================================================================
// Action Count and Timing Tests
// =============================================================================

/// Test rapid action dispatch
#[gpui::test]
async fn test_rapid_action_dispatch(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Rapidly press volume up
    for _ in 0..20 {
        sim.keystroke("+");
    }

    // All actions should be recorded
    assert_eq!(sim.action_count("VolumeUp"), 20);

    // Volume should be at max (1.0)
    assert!((sim.volume() - 1.0).abs() < 0.01, "Volume should be at max");
}

/// Test volume bounds
#[gpui::test]
async fn test_volume_bounds(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Decrease volume a lot
    for _ in 0..30 {
        sim.keystroke("-");
    }

    // Volume should be at min (0.0)
    assert!(sim.volume() >= 0.0, "Volume should not go below 0");
    assert!(sim.volume() <= 0.01, "Volume should be near 0");

    // Increase volume a lot
    for _ in 0..30 {
        sim.keystroke("+");
    }

    // Volume should be at max (1.0)
    assert!(sim.volume() <= 1.0, "Volume should not exceed 1");
    assert!(sim.volume() >= 0.99, "Volume should be near 1");
}

// =============================================================================
// State Isolation Tests
// =============================================================================

/// Test that search mode properly isolates all conflicting keys
#[gpui::test]
async fn test_complete_search_isolation(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Set initial state
    sim.state_mut().volume = 0.5;
    sim.state_mut().is_playing = true;
    sim.state_mut().filter_rating = Some(3);

    let initial_volume = sim.volume();
    let initial_playing = sim.is_playing();
    let initial_filter = sim.state().filter_rating;

    // Enter search mode
    sim.set_input_mode(TestInputMode::Search);
    sim.clear_actions();

    // Press ALL conflicting keys
    sim.keystroke("space");
    sim.keystroke("+");
    sim.keystroke("-");
    sim.keystroke("0");
    sim.keystroke("1");
    sim.keystroke("2");
    sim.keystroke("3");
    sim.keystroke("4");
    sim.keystroke("5");
    sim.keystroke("n");
    sim.keystroke("p");

    // NO actions should have been triggered
    assert!(
        sim.actions().is_empty(),
        "No actions should trigger in search mode: {:?}",
        sim.actions()
    );

    // State should be completely unchanged
    assert_eq!(sim.volume(), initial_volume, "Volume should be unchanged");
    assert_eq!(sim.is_playing(), initial_playing, "Playback should be unchanged");
    assert_eq!(sim.state().filter_rating, initial_filter, "Filter should be unchanged");
}

/// Test that text input mode also isolates keys
#[gpui::test]
async fn test_complete_text_entry_isolation(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Enter text entry mode (simulating directory input, plugin save, etc.)
    sim.set_input_mode(TestInputMode::TextEntry);
    sim.clear_actions();

    // Press keys that would normally be bound
    sim.keystroke("space");
    sim.keystroke("+");
    sim.keystroke("1");

    // No actions should trigger
    assert!(
        sim.actions().is_empty(),
        "No actions should trigger in text entry mode: {:?}",
        sim.actions()
    );
}

// =============================================================================
// Complex Workflow Tests
// =============================================================================

/// Test a complex multi-step workflow
#[gpui::test]
async fn test_complex_workflow(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // 1. Start playing
    sim.keystroke("space");
    assert!(sim.is_playing());

    // 2. Set filter to 4
    sim.keystroke("4");
    assert_eq!(sim.state().filter_rating, Some(4));

    // 3. Search for something
    sim.keystroke("/");
    sim.type_text("Pink Floyd");

    // 4. Cancel search
    sim.keystroke("escape");

    // 5. Adjust volume
    sim.keystroke("+");
    sim.keystroke("+");

    // 6. Search again
    sim.keystroke("/");
    sim.type_text("Tool");
    sim.keystroke("escape");

    // 7. Change filter
    sim.keystroke("5");

    // 8. Pause
    sim.keystroke("space");

    // Final state checks
    assert!(!sim.is_playing(), "Should be paused");
    assert_eq!(sim.state().filter_rating, Some(5), "Filter should be 5");
    assert!(sim.volume() > 1.0 - 0.2 - 0.01, "Volume should have increased");
}

/// Test recovery from invalid states
#[gpui::test]
async fn test_state_recovery(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Manually put into search mode without going through toggle
    sim.set_input_mode(TestInputMode::Search);
    sim.state_mut().search_query = "stuck query".to_string();

    // User presses escape to recover
    sim.keystroke("escape");

    // Should be back to normal
    assert_eq!(sim.input_mode(), TestInputMode::Normal);
    assert!(sim.search_query().is_empty());

    // Actions should now work
    sim.keystroke("space");
    assert!(sim.action_triggered("PlayPause"));
}
