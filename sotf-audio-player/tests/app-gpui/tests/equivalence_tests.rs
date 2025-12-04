//! State-behavior equivalence tests between TUI and GPUI implementations.
//!
//! These tests verify that both apps produce equivalent state changes
//! when the same operations are applied.

mod common;

use common::{
    AppAdapter, GpuiAdapter, Operation, OperationSequence, ScreenId, TestHarness, TuiAdapter,
    assert_equivalent, create_test_library, operations::sequences, run_equivalence_test,
};

// ============================================================================
// Test Setup Helpers
// ============================================================================

fn setup_test_harnesses() -> (TestHarness<TuiAdapter>, TestHarness<GpuiAdapter>) {
    let mut tui = TestHarness::new(TuiAdapter::new());
    let mut gpui = TestHarness::new(GpuiAdapter::new());

    // Load the same test library into both
    let albums = create_test_library();
    tui.adapter.load_test_library(&albums);
    gpui.adapter.load_test_library(&albums);

    // Verify initial states match
    assert_equivalent(&tui, &gpui, "initial state after loading test library");

    (tui, gpui)
}

// ============================================================================
// Navigation Tests
// ============================================================================

#[test]
fn test_screen_navigation_equivalence() {
    let (mut tui, mut gpui) = setup_test_harnesses();
    run_equivalence_test(&mut tui, &mut gpui, &sequences::screen_navigation());
}

#[test]
fn test_library_browsing_equivalence() {
    let (mut tui, mut gpui) = setup_test_harnesses();
    run_equivalence_test(&mut tui, &mut gpui, &sequences::library_browsing());
}

#[test]
fn test_album_selection_bounds() {
    let (mut tui, mut gpui) = setup_test_harnesses();

    // Try to select beyond bounds (should clamp)
    let sequence = OperationSequence::new("album_selection_bounds")
        .then(Operation::SelectAlbumAtIndex(100)) // Beyond max
        .then(Operation::SelectPreviousAlbum)
        .then(Operation::SelectPreviousAlbum)
        .then(Operation::SelectPreviousAlbum)
        .then(Operation::SelectPreviousAlbum)
        .then(Operation::SelectPreviousAlbum) // Should clamp at 0
        .then(Operation::SelectNextAlbum);

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}

// ============================================================================
// Search Tests
// ============================================================================

#[test]
fn test_search_workflow_equivalence() {
    let (mut tui, mut gpui) = setup_test_harnesses();
    run_equivalence_test(&mut tui, &mut gpui, &sequences::search_workflow());
}

#[test]
fn test_search_query_manipulation() {
    let (mut tui, mut gpui) = setup_test_harnesses();

    let sequence = OperationSequence::new("search_query_manipulation")
        .then(Operation::SetSearchQuery("Artist".into()))
        .then(Operation::SetSearchQuery("Artist A".into()))
        .then(Operation::ClearSearch)
        .then(Operation::SetSearchQuery("Beatles".into()));

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}

// ============================================================================
// Sort Order Tests
// ============================================================================

#[test]
fn test_sort_order_cycling_equivalence() {
    let (mut tui, mut gpui) = setup_test_harnesses();
    run_equivalence_test(&mut tui, &mut gpui, &sequences::sort_order_cycling());
}

// ============================================================================
// Channel Filter Tests
// ============================================================================

#[test]
fn test_channel_filter_cycling_equivalence() {
    let (mut tui, mut gpui) = setup_test_harnesses();
    run_equivalence_test(&mut tui, &mut gpui, &sequences::channel_filter_cycling());
}

// ============================================================================
// Queue Management Tests
// ============================================================================

#[test]
fn test_queue_management_equivalence() {
    let (mut tui, mut gpui) = setup_test_harnesses();
    run_equivalence_test(&mut tui, &mut gpui, &sequences::queue_management());
}

#[test]
fn test_queue_operations() {
    let (mut tui, mut gpui) = setup_test_harnesses();

    let sequence = OperationSequence::new("queue_operations")
        // Add multiple albums to queue
        .then(Operation::AddSelectedAlbumToQueue)
        .then(Operation::SelectNextAlbum)
        .then(Operation::AddSelectedAlbumToQueue)
        .then(Operation::SelectNextAlbum)
        .then(Operation::AddSelectedAlbumToQueue)
        // Navigate in queue
        .then(Operation::SwitchScreen(ScreenId::Queue))
        .then(Operation::SelectNextQueueItem)
        .then(Operation::SelectNextQueueItem)
        // Move items
        .then(Operation::MoveQueueItemUp)
        .then(Operation::MoveQueueItemDown)
        // Remove item
        .then(Operation::RemoveFromQueue(1))
        // Clear queue
        .then(Operation::ClearQueue);

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}

// ============================================================================
// Plugin Chain Tests
// ============================================================================

#[test]
fn test_plugin_chain_management_equivalence() {
    let (mut tui, mut gpui) = setup_test_harnesses();
    run_equivalence_test(&mut tui, &mut gpui, &sequences::plugin_chain_management());
}

#[test]
fn test_plugin_operations() {
    let (mut tui, mut gpui) = setup_test_harnesses();

    use common::PluginTypeId;

    let sequence = OperationSequence::new("plugin_operations")
        .then(Operation::SwitchScreen(ScreenId::Plugins))
        // Add multiple plugins
        .then(Operation::AddPlugin(PluginTypeId::Gain))
        .then(Operation::AddPlugin(PluginTypeId::EQ))
        .then(Operation::AddPlugin(PluginTypeId::Compressor))
        // Navigate
        .then(Operation::SelectNextPlugin)
        .then(Operation::SelectNextPlugin)
        // Toggle
        .then(Operation::TogglePlugin(1))
        // Reorder
        .then(Operation::MovePluginUp)
        .then(Operation::MovePluginDown)
        // Enter/exit edit mode
        .then(Operation::EnterPluginEdit)
        .then(Operation::ExitPluginEdit)
        // Remove plugin
        .then(Operation::RemovePlugin(0));

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}

// ============================================================================
// Volume Control Tests
// ============================================================================

#[test]
fn test_volume_control_equivalence() {
    let (mut tui, mut gpui) = setup_test_harnesses();
    run_equivalence_test(&mut tui, &mut gpui, &sequences::volume_control());
}

#[test]
fn test_volume_bounds() {
    let (mut tui, mut gpui) = setup_test_harnesses();

    let sequence = OperationSequence::new("volume_bounds")
        .then(Operation::SetVolume(0.0))
        .then(Operation::VolumeDown) // Should stay at 0
        .then(Operation::SetVolume(1.0))
        .then(Operation::VolumeUp) // Should stay at 1
        .then(Operation::SetVolume(0.5));

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}

// ============================================================================
// Playback State Tests
// ============================================================================

#[test]
fn test_playback_state_transitions() {
    let (mut tui, mut gpui) = setup_test_harnesses();

    let sequence = OperationSequence::new("playback_state_transitions")
        .then(Operation::Play)
        .then(Operation::Pause)
        .then(Operation::TogglePlayback) // Should play
        .then(Operation::TogglePlayback) // Should pause
        .then(Operation::Stop);

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}

// ============================================================================
// View Mode Tests
// ============================================================================

#[test]
fn test_view_mode_toggle() {
    let (mut tui, mut gpui) = setup_test_harnesses();

    let sequence = OperationSequence::new("view_mode_toggle")
        .then(Operation::SwitchScreen(ScreenId::Library))
        .then(Operation::ToggleViewMode)
        .then(Operation::ToggleViewMode)
        .then(Operation::ToggleViewMode);

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}

// ============================================================================
// Complex Workflow Tests
// ============================================================================

#[test]
fn test_full_user_workflow() {
    let (mut tui, mut gpui) = setup_test_harnesses();

    use common::PluginTypeId;

    // Simulate a realistic user workflow
    let sequence = OperationSequence::new("full_user_workflow")
        // Start in library, browse albums
        .then(Operation::SwitchScreen(ScreenId::Library))
        .then(Operation::SetSearchQuery("Artist".into()))
        .then(Operation::SelectNextAlbum)
        .then(Operation::SelectNextAlbum)
        .then(Operation::ClearSearch)
        // Add to queue
        .then(Operation::AddSelectedAlbumToQueue)
        .then(Operation::SelectNextAlbum)
        .then(Operation::AddSelectedAlbumToQueue)
        // Configure plugins
        .then(Operation::SwitchScreen(ScreenId::Plugins))
        .then(Operation::AddPlugin(PluginTypeId::EQ))
        .then(Operation::AddPlugin(PluginTypeId::Gain))
        // Set volume and start playback
        .then(Operation::SetVolume(0.3))
        .then(Operation::Play)
        // Check queue
        .then(Operation::SwitchScreen(ScreenId::Queue))
        .then(Operation::SelectNextQueueItem)
        // Adjust volume during playback
        .then(Operation::VolumeUp)
        .then(Operation::VolumeUp)
        // Pause and resume
        .then(Operation::Pause)
        .then(Operation::Play)
        // Return to library
        .then(Operation::SwitchScreen(ScreenId::Library));

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}

#[test]
fn test_rapid_state_changes() {
    let (mut tui, mut gpui) = setup_test_harnesses();

    // Rapid state changes to test edge cases
    let sequence = OperationSequence::new("rapid_state_changes")
        .then(Operation::Play)
        .then(Operation::Pause)
        .then(Operation::Play)
        .then(Operation::Pause)
        .then(Operation::TogglePlayback)
        .then(Operation::TogglePlayback)
        .then(Operation::VolumeUp)
        .then(Operation::VolumeDown)
        .then(Operation::VolumeUp)
        .then(Operation::VolumeDown)
        .then(Operation::SwitchScreen(ScreenId::Queue))
        .then(Operation::SwitchScreen(ScreenId::Library))
        .then(Operation::SwitchScreen(ScreenId::Plugins))
        .then(Operation::SwitchScreen(ScreenId::Library));

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_library_operations() {
    // Test with empty library
    let mut tui = TestHarness::new(TuiAdapter::new());
    let mut gpui = TestHarness::new(GpuiAdapter::new());

    // Don't load test library - keep it empty

    let sequence = OperationSequence::new("empty_library_operations")
        .then(Operation::SelectNextAlbum) // Should be safe with empty library
        .then(Operation::SelectPreviousAlbum)
        .then(Operation::AddSelectedAlbumToQueue) // Should not crash
        .then(Operation::SwitchScreen(ScreenId::Queue))
        .then(Operation::SelectNextQueueItem);

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}

#[test]
fn test_empty_queue_operations() {
    let (mut tui, mut gpui) = setup_test_harnesses();

    let sequence = OperationSequence::new("empty_queue_operations")
        .then(Operation::SwitchScreen(ScreenId::Queue))
        .then(Operation::SelectNextQueueItem)
        .then(Operation::SelectPreviousQueueItem)
        .then(Operation::MoveQueueItemUp)
        .then(Operation::MoveQueueItemDown)
        .then(Operation::ClearQueue); // Already empty

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}

#[test]
fn test_empty_plugin_chain_operations() {
    let mut tui = TestHarness::new(TuiAdapter::new());
    let mut gpui = TestHarness::new(GpuiAdapter::new());

    // Clear any default plugins first
    tui.adapter.app.plugin_chain = sotf_audio_player::PluginChain::new();
    gpui.adapter.app.plugin_chain = sotf_audio_player::PluginChain::new();

    let sequence = OperationSequence::new("empty_plugin_chain_operations")
        .then(Operation::SwitchScreen(ScreenId::Plugins))
        .then(Operation::SelectNextPlugin)
        .then(Operation::SelectPreviousPlugin)
        .then(Operation::MovePluginUp)
        .then(Operation::MovePluginDown)
        .then(Operation::EnterPluginEdit) // Should not crash with no plugins
        .then(Operation::ExitPluginEdit);

    run_equivalence_test(&mut tui, &mut gpui, &sequence);
}
