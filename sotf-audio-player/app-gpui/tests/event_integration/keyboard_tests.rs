//! Keyboard event integration tests.
//!
//! These tests verify that keyboard events are routed correctly based on
//! input mode and that keybindings are properly isolated.

use super::test_view::*;
use gpui::*;

// =============================================================================
// Input Mode Isolation Tests
// =============================================================================

/// Test that keybindings fire in normal mode
#[gpui::test]
async fn test_keybindings_fire_in_normal_mode(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Verify we're in normal mode
    assert_eq!(sim.input_mode(), TestInputMode::Normal);

    // Press space - should trigger PlayPause
    sim.keystroke("space");
    assert!(sim.action_triggered("PlayPause"), "Space should trigger PlayPause in normal mode");

    // Press + - should trigger VolumeUp
    sim.keystroke("+");
    assert!(sim.action_triggered("VolumeUp"), "+ should trigger VolumeUp in normal mode");

    // Press number keys - should trigger filters
    sim.keystroke("1");
    assert!(sim.action_triggered("SetFilter1"), "1 should trigger SetFilter1 in normal mode");
}

/// Test that keybindings do NOT fire in search mode
#[gpui::test]
async fn test_keybindings_blocked_in_search_mode(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Enter search mode
    sim.set_input_mode(TestInputMode::Search);
    sim.clear_actions();

    // These keys have bindings in normal mode but should NOT trigger in search mode
    let conflicting_keys = ["space", "1", "2", "3", "4", "5", "0", "+", "-"];

    for key in conflicting_keys {
        sim.keystroke(key);
    }

    // No actions should have been triggered
    let actions = sim.actions();
    assert!(
        actions.is_empty(),
        "Keybindings should NOT fire in search mode, but got: {:?}",
        actions
    );
}

/// Test that keybindings do NOT fire in text entry mode
#[gpui::test]
async fn test_keybindings_blocked_in_text_entry_mode(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Enter text entry mode
    sim.set_input_mode(TestInputMode::TextEntry);
    sim.clear_actions();

    // These keys have bindings but should NOT trigger
    sim.keystroke("space");
    sim.keystroke("n");
    sim.keystroke("p");

    // No actions should have been triggered
    assert!(
        sim.actions().is_empty(),
        "Keybindings should NOT fire in text entry mode"
    );
}

// =============================================================================
// Search Mode Workflow Tests
// =============================================================================

/// Test search mode entry via '/' key
#[gpui::test]
async fn test_search_mode_entry(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Start in normal mode
    assert_eq!(sim.input_mode(), TestInputMode::Normal);

    // Press '/' to enter search mode
    sim.keystroke("/");

    // Should trigger ToggleSearch action
    assert!(sim.action_triggered("ToggleSearch"), "/ should trigger ToggleSearch");

    // Should now be in search mode
    assert_eq!(sim.input_mode(), TestInputMode::Search);
}

/// Test search mode exit via Escape key
#[gpui::test]
async fn test_search_mode_exit_via_escape(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Enter search mode and type something
    sim.set_input_mode(TestInputMode::Search);
    sim.state_mut().search_query = "test query".to_string();

    // Press Escape
    sim.keystroke("escape");

    // Should exit to normal mode
    assert_eq!(sim.input_mode(), TestInputMode::Normal);

    // Search query should be cleared
    assert!(sim.search_query().is_empty(), "Search query should be cleared on escape");
}

/// Test typing in search mode adds to query
#[gpui::test]
async fn test_typing_in_search_mode(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Enter search mode
    sim.set_input_mode(TestInputMode::Search);

    // Type some characters
    sim.type_text("jazz");

    // Search query should contain typed text
    let query = sim.search_query();
    assert!(
        query.contains("jazz") || query == "jazz",
        "Search query should contain 'jazz', got: {}",
        query
    );
}

/// Test backspace in search mode
#[gpui::test]
async fn test_backspace_in_search_mode(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Enter search mode with existing query
    sim.set_input_mode(TestInputMode::Search);
    sim.state_mut().search_query = "test".to_string();

    // Press backspace
    sim.keystroke("backspace");

    // Should remove last character
    assert_eq!(sim.search_query(), "tes");
}

// =============================================================================
// Volume Control Tests
// =============================================================================

/// Test volume up key
#[gpui::test]
async fn test_volume_up_key(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    let initial_volume = sim.volume();

    // Press + multiple times
    sim.keystroke("+");
    sim.keystroke("+");
    sim.keystroke("+");

    // Volume should have increased
    assert!(
        sim.volume() > initial_volume,
        "Volume should increase: {} -> {}",
        initial_volume,
        sim.volume()
    );

    // VolumeUp action should have been triggered 3 times
    assert_eq!(sim.action_count("VolumeUp"), 3);
}

/// Test volume down key
#[gpui::test]
async fn test_volume_down_key(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    let initial_volume = sim.volume();

    // Press - multiple times
    sim.keystroke("-");
    sim.keystroke("-");

    // Volume should have decreased
    assert!(
        sim.volume() < initial_volume,
        "Volume should decrease: {} -> {}",
        initial_volume,
        sim.volume()
    );
}

// =============================================================================
// Playback Control Tests
// =============================================================================

/// Test space toggles playback
#[gpui::test]
async fn test_space_toggles_playback(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Initially not playing
    assert!(!sim.is_playing());

    // Press space - should start playing
    sim.keystroke("space");
    assert!(sim.is_playing(), "Space should toggle playback ON");

    // Press space again - should stop playing
    sim.keystroke("space");
    assert!(!sim.is_playing(), "Space should toggle playback OFF");
}

// =============================================================================
// Filter Tests
// =============================================================================

/// Test number keys set filters
#[gpui::test]
async fn test_number_keys_set_filters(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Test each number key
    sim.keystroke("1");
    assert_eq!(sim.state().filter_rating, Some(1));

    sim.keystroke("3");
    assert_eq!(sim.state().filter_rating, Some(3));

    sim.keystroke("5");
    assert_eq!(sim.state().filter_rating, Some(5));

    // 0 should clear filter
    sim.keystroke("0");
    assert_eq!(sim.state().filter_rating, None);
}

// =============================================================================
// Context Switching Tests
// =============================================================================

/// Test that mode transitions preserve state correctly
#[gpui::test]
async fn test_mode_transitions_preserve_state(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Set some state
    sim.keystroke("space"); // Toggle playback
    sim.keystroke("3");     // Set filter to 3

    let was_playing = sim.is_playing();
    let filter = sim.state().filter_rating;

    // Enter and exit search mode
    sim.keystroke("/");
    assert_eq!(sim.input_mode(), TestInputMode::Search);

    // Exit via escape
    sim.keystroke("escape");
    assert_eq!(sim.input_mode(), TestInputMode::Normal);

    // State should be preserved
    assert_eq!(sim.is_playing(), was_playing, "Playback state should be preserved");
    assert_eq!(sim.state().filter_rating, filter, "Filter should be preserved");
}

/// Test rapid mode switching
#[gpui::test]
async fn test_rapid_mode_switching(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Rapidly switch modes
    for _ in 0..10 {
        sim.keystroke("/");     // Enter search
        sim.keystroke("escape"); // Exit search
    }

    // Should end in normal mode
    assert_eq!(sim.input_mode(), TestInputMode::Normal);

    // ToggleSearch should have been called 10 times
    assert_eq!(sim.action_count("ToggleSearch"), 10);
}

// =============================================================================
// Edge Case Tests
// =============================================================================

/// Test empty escape (escape in normal mode)
#[gpui::test]
async fn test_escape_in_normal_mode(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Press escape in normal mode - should do nothing harmful
    sim.keystroke("escape");
    sim.keystroke("escape");

    // Should still be in normal mode
    assert_eq!(sim.input_mode(), TestInputMode::Normal);
}

/// Test unknown keys don't crash
#[gpui::test]
async fn test_unknown_keys_handled(cx: &mut TestAppContext) {
    register_test_keybindings(cx);
    let (window, state) = create_test_window(cx);

    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();

    let mut sim = EventSimulator::new(&mut visual_cx, state);

    // Press various unbound keys - should not crash
    sim.keystroke("q");
    sim.keystroke("w");
    sim.keystroke("e");
    sim.keystroke("r");
    sim.keystroke("t");

    // Should still be in normal mode, no actions triggered
    assert_eq!(sim.input_mode(), TestInputMode::Normal);
}
