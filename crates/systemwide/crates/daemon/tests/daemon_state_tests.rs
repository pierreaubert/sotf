use sotf_audio::manager::AudioEngineManager;

// We need to simulate the structure of AudioDaemon in sotf_daemon.rs
// but since it's in a binary, we can't easily import it.
// We'll test the underlying AudioEngineManager which is what handles the state.

#[test]
fn test_audio_engine_manager_state_reporting() {
    let manager = AudioEngineManager::new();

    // Initial state
    assert_eq!(
        manager.get_state(),
        sotf_audio::manager::StreamingState::Idle
    );
    assert_eq!(manager.get_volume(), 1.0);
    assert!(!manager.is_muted());

    // Test volume update
    manager.set_volume(0.5).expect("Failed to set volume");
    assert_eq!(manager.get_volume(), 0.5);

    // Test mute update
    manager.set_mute(true).expect("Failed to set mute");
    assert!(manager.is_muted());

    // Verify volume is preserved even if engine is not running
    assert_eq!(manager.get_volume(), 0.5);
}

#[test]
fn test_device_matching_priority_logic() {
    // This tests the logic we refactored the daemon to use

    let host = cpal::default_host();

    // We can't easily mock cpal::Host, so we'll test the logic in sotf_audio::devices
    // that the daemon now calls.

    // If there are no devices, we can't test much, but we can verify it returns an Err
    // instead of panicking.
    let result = sotf_audio::devices::find_device(&host, "NonExistentDevice12345", false);
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(err.contains("not found"));
}

#[test]
fn daemon_plugin_reload_uses_hot_update_path() {
    let source = include_str!("../bin/sotf_daemon.rs");
    let reload_start = source
        .find("async fn reload_plugins")
        .expect("reload_plugins should exist");
    let reload_body = &source[reload_start..];

    assert!(
        source.contains("fn build_driver_plugin_chain"),
        "daemon should build the injected metering chain in one shared helper"
    );
    assert!(
        reload_body.contains("manager.update_plugin_chain(final_plugins)"),
        "plugin add/remove/update/reorder should hot-update the running engine"
    );
    assert!(
        reload_body.contains("No engine running")
            && reload_body.contains("handle_load_plugins_with_channels"),
        "full driver playback restart should be reserved for the no-engine fallback"
    );
}
