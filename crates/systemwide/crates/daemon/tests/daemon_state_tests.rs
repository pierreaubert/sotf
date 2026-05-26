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

#[test]
fn daemon_load_plugins_carries_hal_input_channels() {
    let source = include_str!("../bin/sotf_daemon.rs");

    assert!(
        source.contains("const MAX_HAL_CHANNELS: usize = 32"),
        "daemon should validate HAL channel counts up to 32"
    );
    assert!(
        source.contains("input_channels: usize")
            && source.contains("current_input_channels")
            && source.contains("channel_count: driver_input_channels as u32"),
        "load_plugins should carry requested HAL input channels into driver config"
    );
    assert!(
        source.contains("start_hal_playback_with_driver_config(")
            && source.contains("driver_input_channels"),
        "driver playback should be restarted with the resolved HAL input channel count"
    );
}

#[test]
fn daemon_metering_returns_channel_sized_fallbacks() {
    let source = include_str!("../bin/sotf_daemon.rs");

    assert!(
        source.contains("fn empty_loudness_json(channels: usize)")
            && source.contains("\"channel_peaks\": vec![0.0; channels]")
            && source.contains("empty_loudness_json(fallback_input_channels)")
            && source.contains("empty_loudness_json(fallback_output_channels)"),
        "get_metering should return zeroed N-channel meter payloads until analyzer data is available"
    );
}

#[test]
fn daemon_status_exposes_toolbar_device_and_playback_diagnostics() {
    let source = include_str!("../bin/sotf_daemon.rs");

    assert!(
        source.contains("\"selected_device\": selected_device")
            && source.contains("\"channels\": engine_state.num_channels")
            && source.contains("\"playback_callback_count\": engine_state.playback_callback_count")
            && source.contains(
                "\"playback_buffer_fill_percent\": engine_state.playback_buffer_fill_percent"
            )
            && source.contains("\"playback_frames_written\": engine_state.playback_frames_written"),
        "status should expose daemon-owned device/channel state and playback hardware diagnostics"
    );
}

#[test]
fn configbar_reconciles_device_picker_from_daemon_status() {
    let source = include_str!("../configbar/src/ConfigBar.swift");

    assert!(
        source.contains("let selectedDevice: String?")
            && source.contains("data[\"selected_device\"]?.value as? String")
            && source.contains("applyLoadedDevices(daemonSelectedDevice: status.selectedDevice)")
            && source.contains("programmaticDeviceSelection = daemonDevice"),
        "toolbar should parse selected_device from status and update its picker without re-owning daemon state"
    );
}

#[test]
fn daemon_pkg_preinstall_quiesces_running_daemon_before_upgrade() {
    let source = include_str!("../../../../../scripts/build-systemwide.sh");
    let preinstall_start = source
        .find("cat > \"$pkg_scripts/preinstall\"")
        .expect("app package preinstall should exist");
    let hal_preinstall_start = source
        .find("cat > \"$hal_pkg_scripts/preinstall\"")
        .expect("HAL package preinstall should exist");
    let preinstall_source = &source[preinstall_start..hal_preinstall_start];

    assert!(
        preinstall_source.contains("{\"command\":\"shutdown\"}"),
        "preinstall should request a graceful daemon shutdown over the control socket"
    );
    assert!(
        preinstall_source.contains("getconf DARWIN_USER_TEMP_DIR")
            && preinstall_source.contains("/tmp/sotf-${console_uid}/daemon.sock")
            && preinstall_source.contains("/tmp/autoeq_audio.sock"),
        "preinstall should try the daemon's current secure socket paths plus the legacy socket"
    );
    assert!(
        preinstall_source.contains("/usr/bin/pgrep -x \"sotf-daemon\""),
        "preinstall should wait for sotf-daemon to exit"
    );
    assert!(
        preinstall_source.matches("wait_for_daemon_exit 2").count() >= 2,
        "preinstall should wait 2 seconds after shutdown and after TERM"
    );
    assert!(
        preinstall_source.contains("/usr/bin/pkill -TERM -x \"sotf-daemon\"")
            && preinstall_source.contains("/usr/bin/pkill -KILL -x \"sotf-daemon\""),
        "preinstall should escalate from TERM to KILL as a last resort"
    );
}

#[test]
fn systemwide_app_icon_uses_configbar_svg_source() {
    let source = include_str!("../../../../../scripts/build-systemwide.sh");
    let create_icon_start = source
        .find("create_app_icon()")
        .expect("Systemwide package script should create an app icon");
    let create_icon_source = &source[create_icon_start..];

    assert!(
        create_icon_source.contains("CONFIGBAR_DIR/assets/icon.svg"),
        "Systemwide app icon should come from configbar/assets/icon.svg"
    );
    assert!(
        create_icon_source.contains("rsvg-convert") && create_icon_source.contains("magick"),
        "Systemwide app icon should rasterize the SVG without depending on the app-gpui artwork"
    );
    assert!(
        !create_icon_source.contains("crates/app-gpui/assets/sotf.jpg"),
        "Systemwide app icon should not reuse the GPUI app artwork"
    );
}

#[test]
fn configbar_output_device_refresh_tracks_channel_limits() {
    let configbar = include_str!("../configbar/src/ConfigBar.swift");
    let rack = include_str!("../configbar/src/PluginRackView.swift");

    assert!(
        configbar.contains("Button(action: {\n                            loadDevices()\n                        })"),
        "output device selector should expose a refresh button next to the picker"
    );
    assert!(
        configbar.contains("ForEach(outputChannelOptions"),
        "output channel menu should be constrained by the selected interface"
    );
    assert!(
        configbar.contains("let channelOptions = Array(1...32)")
            && configbar.contains("\"input_channels\": halInputChannels"),
        "toolbar should expose and send HAL input channel counts up to 32"
    );
    assert!(
        configbar.contains("syncOutputChannelsToSelectedDevice(applyChange: true)"),
        "device refresh/selection should clamp the channel selection when metadata changes"
    );
    let default_selection = configbar
        .find("if let physicalDefault = physicalDevices.first(where: { $0.is_default })")
        .expect("device discovery should prefer the system default output");
    let previous_selection = configbar
        .find("physicalDevices.contains(where: { $0.name == previousDevice })")
        .expect("device discovery should still fall back to the previous selection");
    assert!(
        default_selection < previous_selection,
        "system default output should take priority over a stale previous toolbar selection"
    );
    assert!(
        configbar.contains("availableOutputChannels: selectedOutputDeviceChannelLimit"),
        "plugin rack should receive the selected output device channel limit"
    );
    assert!(
        rack.contains("channelCompatibilityWarning")
            && rack.contains("exclamationmark.triangle.fill")
            && rack.contains("speakerConfigChannels"),
        "plugin rack should warn when plugin layouts exceed the interface channels"
    );
    assert!(
        configbar.contains("syncMeterArrays(inputChannels: newValue)")
            && configbar.contains("private func resizedPeaks")
            && configbar.contains("peaks.prefix(32)"),
        "toolbar meters should resize and clamp to the current N-channel layout"
    );
}

#[test]
fn configbar_plugin_edit_sheet_batches_parameter_edits_until_apply_or_close() {
    let source = include_str!("../configbar/src/PluginRackView.swift");
    let sheet_start = source
        .find("struct PluginEditSheet")
        .expect("PluginEditSheet should exist");
    let add_sheet_start = source
        .find("struct AddPluginSheet")
        .expect("AddPluginSheet should exist");
    let sheet_source = &source[sheet_start..add_sheet_start];

    assert!(
        sheet_source.contains("@State private var draftParameters"),
        "plugin editor should keep a local draft"
    );
    assert!(
        sheet_source.contains("onApply: @escaping ([String: Any]) -> Bool"),
        "apply should report whether daemon update succeeded"
    );
    for label in ["Load", "Save", "Apply", "Cancel", "Close"] {
        assert!(
            sheet_source.contains(&format!("Button(\"{label}\")")),
            "plugin editor should include a {label} button"
        );
    }
    assert!(
        !sheet_source.contains("Button(\"Done\")"),
        "top Done button should be removed"
    );
    assert!(
        sheet_source.contains("draftParameters = newParameters"),
        "per-control edits should update only the draft"
    );
    assert!(
        sheet_source.contains("PluginParameterFileFormat.allCases")
            && source.contains("case parameterJSON")
            && source.contains("case enginePluginJSON")
            && source.contains("case appGpuiPresetJSON")
            && source.contains("case rawParametersJSON"),
        "single-plugin load/save should cover every supported JSON parameter shape"
    );
    assert!(
        sheet_source.contains(
            "formatPicker.selectItem(at: PluginParameterFileFormat.parameterJSON.rawValue)"
        ) && sheet_source.contains("panel.allowedContentTypes = [.json]"),
        "JSON should remain the default on-disk parameter format"
    );
    assert!(
        sheet_source.contains("parametersFromSupportedJSON")
            && sheet_source.contains("pluginTypeAndParameters(fromAppGpuiSettings:")
            && sheet_source.contains("pluginEntriesFromChannels"),
        "load should accept raw parameters, engine plugin JSON, app-GPUI plugin records, and full plugin configs"
    );
    assert!(
        !source.contains("updateDebounceTask"),
        "per-control edits should no longer debounce daemon update_plugin calls"
    );
    assert!(
        source.contains("applyPluginUpdate(at: index, parameters: newParams)")
            && source.contains("client.updatePlugin(at: index, parameters: parameters)"),
        "only Apply/Close should send parameters to the daemon"
    );
}
