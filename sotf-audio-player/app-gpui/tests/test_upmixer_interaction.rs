use sotf_audio_player::{PluginSettings, PluginType};
use sotf_audio_player_gpui::app::App;
use sotf_audio_player_gpui::app::types::PluginUpdateType;

#[test]
fn test_upmixer_potentiometer_updates_engine() {
    // 1. Initialize App
    // App::new() creates a default app state with a plugin chain.
    let mut app = App::new();

    // 2. Add Upmixer plugin
    // App::new() adds LoudnessMonitor by default. We add Upmixer.
    let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Upmixer);

    // 3. Verify Upmixer is added
    let plugin = app.plugin_chain.get_plugin(plugin_idx).expect("Plugin should exist");
    assert!(matches!(plugin.settings, PluginSettings::Upmixer { .. }));

    // 4. Simulate user interaction: changing a potentiometer
    // We target "Center Gain" (gain_front_ambient) which corresponds to parameter index 2
    // as defined in `sotf-audio-player/app-gpui/plugins/editing.rs`.
    let param_idx = 2; // gain_front_ambient
    let new_value = 3.5; // dB

    // Clear any previous pending updates
    app.pending_plugin_update = None;

    // Call the action that the UI would trigger
    app.set_plugin_param(plugin_idx, param_idx, new_value);

    // 5. Verify App State Update
    let plugin = app.plugin_chain.get_plugin(plugin_idx).expect("Plugin should exist");
    if let PluginSettings::Upmixer { gain_front_ambient, .. } = plugin.settings {
        assert_eq!(gain_front_ambient, new_value, "App state gain_front_ambient should be updated to 3.5");
    } else {
        panic!("Plugin should be Upmixer");
    }

    // 6. Verify Engine Update Flag
    // The app should flag that a parameter update is pending
    assert!(app.pending_plugin_update.is_some(), "Should have pending update");

    if let Some(update_type) = &app.pending_plugin_update {
        match update_type {
            PluginUpdateType::Parameter { plugin_index, param_index } => {
                assert_eq!(*plugin_index, plugin_idx);
                assert_eq!(*param_index, param_idx);
            },
            PluginUpdateType::Structural => {
                // Parameter updates for Upmixer might sometimes trigger Structural updates
                // if param_index_to_engine_param returns None, but for gain_front_ambient it should return Some.
                // Let's verify expectations from editing.rs
                panic!("Expected Parameter update, got Structural");
            }
        }
    }
}
