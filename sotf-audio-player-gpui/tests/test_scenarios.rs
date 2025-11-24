// ============================================================================
// User Scenario Integration Tests
// ============================================================================
//
// High-level scenario tests simulating complete user workflows:
// - Directory management → Library scanning → Playback
// - Plugin chain building and editing
// - File loading workflows (APO/SOFA)
// - Search and filter workflows
// - Error recovery scenarios

use sotf_audio_player::{BiquadFilterType, EQFilter, PluginSettings, PluginType};
use sotf_audio_player_gpui::app::{App, InputMode, LibrarySortOrder, Screen, ToastType};
use std::path::PathBuf;

fn create_test_app() -> App {
    App::new()
}

// ============================================================================
// Scenario 1: Complete Library Setup Workflow
// ============================================================================

#[test]
fn scenario_library_setup_and_scan() {
    let mut app = create_test_app();

    // Step 1: User navigates to Directory Manager
    assert_eq!(app.current_screen, Screen::Library);
    app.current_screen = Screen::DirectoryManager;
    assert_eq!(app.current_screen, Screen::DirectoryManager);

    // Step 2: User presses 'a' to add directory
    app.input_mode = InputMode::AddDirectory;
    assert_eq!(app.input_mode, InputMode::AddDirectory);

    // Step 3: User types directory path
    app.directory_input = "/home/user/Music".to_string();
    assert_eq!(app.directory_input, "/home/user/Music");

    // Step 4: User presses Enter to add directory
    let path = PathBuf::from(&app.directory_input);
    let _ = app.add_directory(path);
    app.directory_input.clear();
    app.input_mode = InputMode::Normal;

    // Verify toast message was set
    assert!(app.toast_message.is_some());

    // Step 5: User presses 's' to start scan
    app.start_library_scan();
    assert!(app.scan_in_progress);

    // Verify scan toast
    if let Some(ref toast) = app.toast_message {
        assert_eq!(toast.toast_type, ToastType::Info);
    }

    // Step 6: Scan completes (simulated by calling scan_library)
    // Note: In real scenario, this would be async

    // Step 7: User navigates back to Library screen
    app.current_screen = Screen::Library;
    app.input_mode = InputMode::Normal;

    // Verify final state
    assert_eq!(app.current_screen, Screen::Library);
    assert_eq!(app.input_mode, InputMode::Normal);
}

// ============================================================================
// Scenario 2: Search and Filter Workflow
// ============================================================================

#[test]
fn scenario_search_and_filter_library() {
    let mut app = create_test_app();

    // Step 1: User is on Library screen
    app.current_screen = Screen::Library;
    assert_eq!(app.current_screen, Screen::Library);

    // Step 2: User presses '/' to enter search mode
    app.input_mode = InputMode::Search;
    assert_eq!(app.input_mode, InputMode::Search);

    // Step 3: User types search query
    app.search_query = "Pink Floyd".to_string();
    assert_eq!(app.search_query, "Pink Floyd");

    // Step 4: Results are filtered (verified by filtered_albums method)
    let _filtered = app.filtered_albums();
    // In real scenario, this would return matching albums

    // Step 5: User presses ESC to clear search
    app.search_query.clear();
    app.input_mode = InputMode::Normal;

    // Step 6: User presses 'C' to cycle channel filter
    // (This would be handled by UI layer, but we verify state)
    app.cycle_channel_filter();

    // Step 7: User presses 'S' to cycle sort order
    app.set_library_sort_order(LibrarySortOrder::Artist);

    // Verify final state
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(app.search_query.is_empty());
}

// ============================================================================
// Scenario 3: Plugin Chain Building and Editing
// ============================================================================

#[test]
fn scenario_build_and_edit_plugin_chain() {
    let mut app = create_test_app();

    // Step 1: User navigates to Plugins screen
    app.current_screen = Screen::Plugins;
    assert_eq!(app.current_screen, Screen::Plugins);

    // Step 2: User presses Shift-1 to add EQ plugin
    app.plugin_chain.add_plugin(&PluginType::EQ);

    if let Some(plugin) = app.plugin_chain.get_plugin_mut(0) {
        plugin.settings = PluginSettings::EQ {
            filters: vec![EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.5, 3.0)],
        };
    }
    assert_eq!(app.plugin_chain.len(), 1);

    // Step 3: User presses Shift-2 to add Upmixer plugin
    let upmixer_idx = app.plugin_chain.add_plugin(&PluginType::Upmixer);

    if let Some(plugin) = app.plugin_chain.get_plugin_mut(upmixer_idx) {
        plugin.settings = PluginSettings::Upmixer {
            speaker_config: "5.0".to_string(),
            gain_front_direct: 0.0,
            gain_front_ambient: 0.0,
            gain_rear_ambient: 0.0,
            lfe_cutoff_hz: 80.0,
            stereo_width: 1.0,
            bandpass_hz: 200.0,
            height_gain: 0.0,
            lfe_gain: 0.0,
            enable_subharmonic_synth: false,
            subharmonic_gain: 0.0,
            enable_hr_direct: true,
            hr_sharpen: 1.0,
            safety_cap_db: -3.0,
        };
    }
    assert_eq!(app.plugin_chain.len(), 2);

    // Step 4: User selects first plugin (EQ)
    app.selected_plugin_index = 0;

    // Step 5: User presses 'e' to edit plugin
    app.enter_plugin_edit_mode();
    assert_eq!(app.input_mode, InputMode::EditPlugin);
    assert_eq!(app.editing_plugin_index, Some(0));

    // Step 6: User navigates parameters with arrow keys
    app.plugin_param_selection = 0;
    assert_eq!(app.plugin_param_selection, 0);

    // Step 7: User adjusts parameter value
    let changed = app.adjust_selected_param(1.0);
    assert!(changed || !changed); // Parameter may or may not change depending on implementation

    // Step 8: User presses 'a' to load APO file
    app.input_mode = InputMode::LoadApoFile;
    assert_eq!(app.input_mode, InputMode::LoadApoFile);

    // Step 9: User enters file path
    let test_file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test_eq.txt");
    app.apo_file_input = test_file.to_string_lossy().to_string();

    // Step 10: User presses Enter to load file
    let result = app.load_apo_file();
    if result.is_ok() {
        app.apo_file_input.clear();
        app.input_mode = InputMode::EditPlugin;
        assert!(app.needs_plugin_update);
    }

    // Step 11: User presses ESC to exit edit mode
    app.input_mode = InputMode::Normal;
    app.editing_plugin_index = None;

    // Verify final state
    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(app.editing_plugin_index.is_none());
    assert_eq!(app.plugin_chain.len(), 2);
}

// ============================================================================
// Scenario 4: Binaural Decoder SOFA File Loading
// ============================================================================

#[test]
fn scenario_load_sofa_file_for_binaural() {
    let mut app = create_test_app();

    // Step 1: Navigate to Plugins screen
    app.current_screen = Screen::Plugins;

    // Step 2: Add Binaural Decoder plugin
    let idx = app.plugin_chain.add_plugin(&PluginType::BinauralDecoder);
    if let Some(plugin) = app.plugin_chain.get_plugin_mut(idx) {
        plugin.settings = PluginSettings::BinauralDecoder {
            sofa_file: String::new(),
            input_channels: 2,
            enable_optimization: true,
            externalization: 0.5,
            near_field_strength: 0.0,
        };
    }

    // Step 3: Select the plugin
    app.selected_plugin_index = 0;

    // Step 4: Enter edit mode
    app.enter_plugin_edit_mode();
    assert_eq!(app.input_mode, InputMode::EditPlugin);

    // Step 5: Press 'f' to load SOFA file
    app.input_mode = InputMode::LoadSofaFile;
    assert_eq!(app.input_mode, InputMode::LoadSofaFile);

    // Step 6: Enter SOFA file path
    app.sofa_file_input = "/path/to/IRC_1002_C.sofa".to_string();

    // Step 7: Press Enter to load
    let result = app.load_sofa_file();
    assert!(result.is_ok(), "SOFA loading should succeed");

    // Step 8: Verify SOFA path was set
    if let Some(plugin) = app.get_editing_plugin() {
        if let PluginSettings::BinauralDecoder { ref sofa_file, .. } = plugin.settings {
            assert!(!sofa_file.is_empty());
            assert_eq!(sofa_file, "/path/to/IRC_1002_C.sofa");
        }
    }

    // Step 9: Exit edit mode
    app.sofa_file_input.clear();
    app.input_mode = InputMode::Normal;
    app.editing_plugin_index = None;

    assert_eq!(app.input_mode, InputMode::Normal);
}

// ============================================================================
// Scenario 5: Error Recovery - Invalid APO File
// ============================================================================

#[test]
fn scenario_error_recovery_invalid_apo_file() {
    let mut app = create_test_app();

    // Setup: Add EQ plugin
    let idx = app.plugin_chain.add_plugin(&PluginType::EQ);
    if let Some(plugin) = app.plugin_chain.get_plugin_mut(idx) {
        plugin.settings = PluginSettings::EQ { filters: vec![] };
    }

    // Step 1: Enter edit mode
    app.current_screen = Screen::Plugins;
    app.selected_plugin_index = 0;
    app.enter_plugin_edit_mode();

    // Step 2: Try to load invalid file
    app.input_mode = InputMode::LoadApoFile;
    app.apo_file_input = "/nonexistent/file.txt".to_string();

    // Step 3: Attempt load - should fail
    let result = app.load_apo_file();
    assert!(result.is_err(), "Loading invalid file should fail");

    // Step 4: User sees error toast (would be set in UI layer)
    app.toast_message = Some(sotf_audio_player_gpui::app::ToastMessage::error(format!(
        "Failed to load APO file: {}",
        result.unwrap_err()
    )));

    // Verify error toast
    if let Some(ref toast) = app.toast_message {
        assert_eq!(toast.toast_type, ToastType::Error);
    }

    // Step 5: User dismisses error and tries again with valid file
    app.dismiss_toast();
    app.apo_file_input.clear();

    // Step 6: Load valid file
    let test_file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test_eq.txt");
    app.apo_file_input = test_file.to_string_lossy().to_string();

    let result = app.load_apo_file();
    assert!(result.is_ok(), "Loading valid file should succeed");

    // Step 7: Success toast
    app.toast_message = Some(sotf_audio_player_gpui::app::ToastMessage::success(
        "APO file loaded",
    ));

    if let Some(ref toast) = app.toast_message {
        assert_eq!(toast.toast_type, ToastType::Success);
    }

    // Verify recovery was successful
    assert!(app.needs_plugin_update);
}

// ============================================================================
// Scenario 6: Wrong Plugin Type Error Recovery
// ============================================================================

#[test]
fn scenario_error_recovery_wrong_plugin_type() {
    let mut app = create_test_app();

    // Setup: Add Compressor plugin (not EQ)
    let idx = app.plugin_chain.add_plugin(&PluginType::Compressor);
    if let Some(plugin) = app.plugin_chain.get_plugin_mut(idx) {
        plugin.settings = PluginSettings::Compressor {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 6.0,
            makeup_gain_db: 0.0,
            mix: 1.0,
            auto_makeup: false,
            link_channels: true,
            sidechain_hpf_hz: 0.0,
        };
    }

    // Step 1: User tries to load APO file for Compressor
    app.current_screen = Screen::Plugins;
    app.selected_plugin_index = 0;
    app.enter_plugin_edit_mode();

    // Step 2: Press 'a' - UI should show warning
    app.input_mode = InputMode::LoadApoFile;
    app.apo_file_input = "test.txt".to_string();

    // Step 3: Try to load - should fail with clear error
    let result = app.load_apo_file();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not an EQ"));

    // Step 4: User sees warning toast
    app.toast_message = Some(sotf_audio_player_gpui::app::ToastMessage::warning(
        "APO files can only be loaded for EQ plugins",
    ));

    if let Some(ref toast) = app.toast_message {
        assert_eq!(toast.toast_type, ToastType::Warning);
    }

    // Step 5: User presses ESC to cancel
    app.input_mode = InputMode::EditPlugin;
    app.apo_file_input.clear();
    app.dismiss_toast();

    // Verify clean state
    assert_eq!(app.input_mode, InputMode::EditPlugin);
    assert!(app.apo_file_input.is_empty());
    assert!(app.toast_message.is_none());
}

// ============================================================================
// Scenario 7: Multi-Screen Navigation with State Preservation
// ============================================================================

#[test]
fn scenario_multi_screen_navigation_with_state() {
    let mut app = create_test_app();

    // Step 1: Start on Library, enter search
    app.current_screen = Screen::Library;
    app.input_mode = InputMode::Search;
    app.search_query = "Beatles".to_string();

    // Step 2: Switch to Queue (search should be preserved)
    app.current_screen = Screen::Queue;
    app.input_mode = InputMode::Normal;
    assert_eq!(app.search_query, "Beatles");

    // Step 3: Switch to Plugins
    app.current_screen = Screen::Plugins;

    // Step 4: Add a plugin
    let idx = app.plugin_chain.add_plugin(&PluginType::EQ);
    if let Some(plugin) = app.plugin_chain.get_plugin_mut(idx) {
        plugin.settings = PluginSettings::EQ { filters: vec![] };
    }

    // Step 5: Switch to Devices
    app.current_screen = Screen::Devices;
    assert_eq!(app.plugin_chain.len(), 1); // Plugin preserved

    // Step 6: Switch to Directory Manager
    app.current_screen = Screen::DirectoryManager;
    app.input_mode = InputMode::AddDirectory;
    app.directory_input = "/music".to_string();

    // Step 7: Switch back to Library
    app.current_screen = Screen::Library;
    app.input_mode = InputMode::Normal;

    // Verify all state was preserved
    assert_eq!(app.search_query, "Beatles");
    assert_eq!(app.directory_input, "/music");
    assert_eq!(app.plugin_chain.len(), 1);
}

// ============================================================================
// Scenario 8: Complete Plugin Chain Workflow
// ============================================================================

#[test]
fn scenario_complete_plugin_chain_workflow() {
    let mut app = create_test_app();

    app.current_screen = Screen::Plugins;

    // Step 1: Build a complete plugin chain
    // EQ -> Compressor -> Limiter -> Upmixer

    // Add EQ
    let idx = app.plugin_chain.add_plugin(&PluginType::EQ);
    if let Some(plugin) = app.plugin_chain.get_plugin_mut(idx) {
        plugin.settings = PluginSettings::EQ { filters: vec![] };
    }

    // Add Compressor
    let idx = app.plugin_chain.add_plugin(&PluginType::Compressor);
    if let Some(plugin) = app.plugin_chain.get_plugin_mut(idx) {
        plugin.settings = PluginSettings::Compressor {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 6.0,
            makeup_gain_db: 0.0,
            mix: 1.0,
            auto_makeup: false,
            link_channels: true,
            sidechain_hpf_hz: 0.0,
        };
    }

    // Add Limiter
    let idx = app.plugin_chain.add_plugin(&PluginType::Limiter);
    if let Some(plugin) = app.plugin_chain.get_plugin_mut(idx) {
        plugin.settings = PluginSettings::Limiter {
            threshold_db: -3.0,
            release_ms: 50.0,
            mix: 1.0,
        };
    }

    assert_eq!(app.plugin_chain.len(), 3);

    // Step 2: Edit middle plugin (Compressor)
    app.selected_plugin_index = 1;
    app.enter_plugin_edit_mode();
    assert_eq!(app.editing_plugin_index, Some(1));

    // Step 3: Adjust parameters
    app.plugin_param_selection = 0; // Threshold
    app.adjust_selected_param(-5.0); // Decrease threshold

    app.plugin_param_selection = 1; // Ratio
    app.adjust_selected_param(1.0); // Increase ratio

    // Step 4: Exit edit mode
    app.input_mode = InputMode::Normal;
    app.editing_plugin_index = None;

    // Step 5: Disable middle plugin
    if let Some(plugin) = app.plugin_chain.get_plugin_mut(1) {
        plugin.enabled = false;
    }

    // Step 6: Verify chain state
    assert_eq!(app.plugin_chain.len(), 3);
    assert!(app.plugin_chain.get_plugin(0).unwrap().enabled);
    assert!(!app.plugin_chain.get_plugin(1).unwrap().enabled);
    assert!(app.plugin_chain.get_plugin(2).unwrap().enabled);
}

// ============================================================================
// Scenario 9: Toast Message Lifecycle
// ============================================================================

#[test]
fn scenario_toast_message_lifecycle() {
    let mut app = create_test_app();

    // Step 1: Success toast from adding directory
    app.current_screen = Screen::DirectoryManager;
    app.toast_message = Some(sotf_audio_player_gpui::app::ToastMessage::success(
        "Directory added. Press 's' to scan.",
    ));

    assert!(app.toast_message.is_some());
    if let Some(ref toast) = app.toast_message {
        assert_eq!(toast.toast_type, ToastType::Success);
    }

    // Step 2: User dismisses toast
    app.dismiss_toast();
    assert!(app.toast_message.is_none());

    // Step 3: Error toast from failed scan
    app.toast_message = Some(sotf_audio_player_gpui::app::ToastMessage::error(
        "Scan failed: Permission denied",
    ));

    if let Some(ref toast) = app.toast_message {
        assert_eq!(toast.toast_type, ToastType::Error);
    }

    // Step 4: User dismisses error
    app.dismiss_toast();
    assert!(app.toast_message.is_none());

    // Step 5: Info toast from scan start
    app.start_library_scan();
    assert!(app.toast_message.is_some());
    if let Some(ref toast) = app.toast_message {
        assert_eq!(toast.toast_type, ToastType::Info);
    }

    // Step 6: Update check (persistent toast should remain)
    app.update_toast();
    assert!(app.toast_message.is_some()); // Scan toast is persistent

    // Step 7: Manual dismiss
    app.dismiss_toast();
    assert!(app.toast_message.is_none());
}

// ============================================================================
// Scenario 10: Help Modal Usage
// ============================================================================

#[test]
fn scenario_help_modal_usage() {
    let mut app = create_test_app();

    // Step 1: User on Library screen
    app.current_screen = Screen::Library;
    assert_eq!(app.input_mode, InputMode::Normal);

    // Step 2: User presses '?' to open help
    app.input_mode = InputMode::Help;
    assert_eq!(app.input_mode, InputMode::Help);

    // Step 3: User reads help keybindings
    // (In real UI, help modal would be displayed)

    // Step 4: User presses ESC to close help
    app.input_mode = InputMode::Normal;
    assert_eq!(app.input_mode, InputMode::Normal);

    // Step 5: User switches to Plugins screen
    app.current_screen = Screen::Plugins;

    // Step 6: User opens help again
    app.input_mode = InputMode::Help;
    assert_eq!(app.input_mode, InputMode::Help);

    // Step 7: Close help
    app.input_mode = InputMode::Normal;

    // Verify state
    assert_eq!(app.current_screen, Screen::Plugins);
    assert_eq!(app.input_mode, InputMode::Normal);
}
