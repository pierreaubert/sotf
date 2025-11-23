// ============================================================================
// File Loading Integration Tests
// ============================================================================
//
// Tests for APO and SOFA file loading functionality:
// - APO file loading for EQ plugins
// - SOFA file loading for Binaural Decoder plugins
// - Error handling for invalid files
// - Input mode transitions during file loading

use sotf_audio_player::{Plugin, PluginSettings, PluginType};
use sotf_audio_player_gpui::app::{App, InputMode};
use std::path::PathBuf;

fn create_test_app() -> App {
    App::new()
}

#[test]
fn test_apo_file_loading_for_eq_plugin() {
    let mut app = create_test_app();

    // Add an EQ plugin to the chain
    let eq_plugin = Plugin {
        plugin_type: PluginType::EQ,
        enabled: true,
        settings: PluginSettings::EQ { filters: vec![] },
    };
    app.plugin_chain.plugins.push(eq_plugin);

    // Enter edit mode for the EQ plugin
    app.editing_plugin_index = Some(0);
    app.input_mode = InputMode::LoadApoFile;

    // Set path to test APO file
    let test_file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test_eq.txt");
    app.apo_file_input = test_file.to_string_lossy().to_string();

    // Load the APO file
    let result = app.load_apo_file();

    // Should succeed
    assert!(
        result.is_ok(),
        "APO file loading should succeed: {:?}",
        result
    );

    // Verify filters were loaded
    if let Some(plugin) = app.get_editing_plugin() {
        if let PluginSettings::EQ { ref filters } = plugin.settings {
            assert!(
                !filters.is_empty(),
                "Filters should be loaded from APO file"
            );
            assert_eq!(filters.len(), 4, "Should have 4 filters from test file");
        } else {
            panic!("Plugin should be an EQ");
        }
    } else {
        panic!("Should have an editing plugin");
    }

    // Verify needs_plugin_update flag is set
    assert!(app.needs_plugin_update, "Should flag plugin update needed");
}

#[test]
fn test_apo_file_loading_invalid_file() {
    let mut app = create_test_app();

    // Add an EQ plugin
    let eq_plugin = Plugin {
        plugin_type: PluginType::EQ,
        enabled: true,
        settings: PluginSettings::EQ { filters: vec![] },
    };
    app.plugin_chain.plugins.push(eq_plugin);

    app.editing_plugin_index = Some(0);
    app.input_mode = InputMode::LoadApoFile;

    // Set path to non-existent file
    app.apo_file_input = "/nonexistent/path/to/file.txt".to_string();

    // Load should fail
    let result = app.load_apo_file();
    assert!(result.is_err(), "Loading non-existent file should fail");
}

#[test]
fn test_apo_file_loading_wrong_plugin_type() {
    let mut app = create_test_app();

    // Add a non-EQ plugin (e.g., Compressor)
    let compressor_plugin = Plugin {
        plugin_type: PluginType::Compressor,
        enabled: true,
        settings: PluginSettings::Compressor {
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
        },
    };
    app.plugin_chain.plugins.push(compressor_plugin);

    app.editing_plugin_index = Some(0);
    app.input_mode = InputMode::LoadApoFile;
    app.apo_file_input = "test.txt".to_string();

    // Should fail because plugin is not an EQ
    let result = app.load_apo_file();
    assert!(result.is_err(), "Loading APO for non-EQ plugin should fail");
    assert!(result.unwrap_err().contains("not an EQ"));
}

#[test]
fn test_apo_file_loading_no_plugin_editing() {
    let mut app = create_test_app();

    // No plugin being edited
    app.editing_plugin_index = None;
    app.input_mode = InputMode::LoadApoFile;
    app.apo_file_input = "test.txt".to_string();

    // Should fail
    let result = app.load_apo_file();
    assert!(
        result.is_err(),
        "Loading APO with no editing plugin should fail"
    );
    assert!(result.unwrap_err().contains("No plugin"));
}

#[test]
fn test_sofa_file_loading_for_binaural_decoder() {
    let mut app = create_test_app();

    // Add a Binaural Decoder plugin
    let binaural_plugin = Plugin {
        plugin_type: PluginType::BinauralDecoder,
        enabled: true,
        settings: PluginSettings::BinauralDecoder {
            sofa_file: String::new(),
            input_channels: 2,
            enable_optimization: true,
            externalization: 0.5,
            near_field_strength: 0.0,
        },
    };
    app.plugin_chain.plugins.push(binaural_plugin);

    app.editing_plugin_index = Some(0);
    app.input_mode = InputMode::LoadSofaFile;
    app.sofa_file_input = "/path/to/hrtf.sofa".to_string();

    // Load the SOFA file
    let result = app.load_sofa_file();

    // Should succeed
    assert!(
        result.is_ok(),
        "SOFA file loading should succeed: {:?}",
        result
    );

    // Verify path was set
    if let Some(plugin) = app.get_editing_plugin() {
        if let PluginSettings::BinauralDecoder { ref sofa_file, .. } = plugin.settings {
            assert!(!sofa_file.is_empty(), "SOFA file path should be set");
            assert_eq!(sofa_file, "/path/to/hrtf.sofa");
        } else {
            panic!("Plugin should be a Binaural Decoder");
        }
    } else {
        panic!("Should have an editing plugin");
    }

    // Verify needs_plugin_update flag is set
    assert!(app.needs_plugin_update, "Should flag plugin update needed");
}

#[test]
fn test_sofa_file_loading_wrong_plugin_type() {
    let mut app = create_test_app();

    // Add an EQ plugin (not Binaural Decoder)
    let eq_plugin = Plugin {
        plugin_type: PluginType::EQ,
        enabled: true,
        settings: PluginSettings::EQ { filters: vec![] },
    };
    app.plugin_chain.plugins.push(eq_plugin);

    app.editing_plugin_index = Some(0);
    app.input_mode = InputMode::LoadSofaFile;
    app.sofa_file_input = "/path/to/hrtf.sofa".to_string();

    // Should fail because plugin is not a Binaural Decoder
    let result = app.load_sofa_file();
    assert!(
        result.is_err(),
        "Loading SOFA for non-Binaural plugin should fail"
    );
    assert!(result.unwrap_err().contains("not a Binaural Decoder"));
}

#[test]
fn test_sofa_file_loading_no_plugin_editing() {
    let mut app = create_test_app();

    // No plugin being edited
    app.editing_plugin_index = None;
    app.input_mode = InputMode::LoadSofaFile;
    app.sofa_file_input = "/path/to/hrtf.sofa".to_string();

    // Should fail
    let result = app.load_sofa_file();
    assert!(
        result.is_err(),
        "Loading SOFA with no editing plugin should fail"
    );
    assert!(result.unwrap_err().contains("No plugin"));
}

#[test]
fn test_file_input_clearing() {
    let mut app = create_test_app();

    // Set file inputs
    app.apo_file_input = "/path/to/eq.txt".to_string();
    app.sofa_file_input = "/path/to/hrtf.sofa".to_string();

    assert!(!app.apo_file_input.is_empty());
    assert!(!app.sofa_file_input.is_empty());

    // Clear inputs
    app.apo_file_input.clear();
    app.sofa_file_input.clear();

    assert!(app.apo_file_input.is_empty());
    assert!(app.sofa_file_input.is_empty());
}

#[test]
fn test_input_mode_transitions_file_loading() {
    let mut app = create_test_app();

    // Normal -> LoadApoFile
    app.input_mode = InputMode::LoadApoFile;
    assert_eq!(app.input_mode, InputMode::LoadApoFile);

    // LoadApoFile -> EditPlugin (after successful load)
    app.input_mode = InputMode::EditPlugin;
    assert_eq!(app.input_mode, InputMode::EditPlugin);

    // EditPlugin -> LoadSofaFile
    app.input_mode = InputMode::LoadSofaFile;
    assert_eq!(app.input_mode, InputMode::LoadSofaFile);

    // LoadSofaFile -> Normal (on cancel)
    app.input_mode = InputMode::Normal;
    assert_eq!(app.input_mode, InputMode::Normal);
}
