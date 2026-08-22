use sotf_audio::manager::AudioEngineManager;

// We need to simulate the structure of AudioDaemon in sotf_daemon.rs
// but since it's in a binary, we can't easily import it.
// We'll test the underlying AudioEngineManager which is what handles the state.

fn daemon_source() -> String {
    [
        include_str!("../bin/sotf_daemon.rs"),
        include_str!("../bin/sotf_daemon/audio_daemon.rs"),
        include_str!("../bin/sotf_daemon/consts.rs"),
        include_str!("../bin/sotf_daemon/misc.rs"),
        include_str!("../bin/sotf_daemon/pipeline_reconfigure_outcome.rs"),
        include_str!("../bin/sotf_daemon/pipeline_supervisor.rs"),
    ]
    .join("\n")
}

fn configbar_source() -> &'static str {
    include_str!("../configbar/src/ConfigBar.swift")
}

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
fn configbar_daemon_cleanup_is_not_forceful_or_fuzzy() {
    let source = configbar_source();
    assert!(
        !source.contains("terminateExistingDaemons")
            && !source.contains("killExistingDaemons")
            && !source.contains("removeStaleSockets"),
        "ConfigBar must not kill unrelated daemons or unlink unverified sockets"
    );
    assert!(
        source.contains("isDaemonReachable()") && source.contains("Adopting existing live daemon"),
        "ConfigBar should adopt a live daemon instead of restarting it"
    );
    assert!(
        source.contains("process.terminate()"),
        "ConfigBar should use graceful termination for daemons it owns"
    );
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
    let source = daemon_source();
    let reload_start = source
        .find("fn reload_plugins_with_user_plugins")
        .expect("reload_plugins_with_user_plugins should exist");
    let reload_body = &source[reload_start..];

    assert!(
        source.contains("fn build_driver_plugin_chain"),
        "daemon should build the injected metering chain in one shared helper"
    );
    assert!(
        reload_body.contains("manager.update_plugin_chain(&plan.runtime_plugins)"),
        "plugin add/remove/update/reorder should hot-update the running engine"
    );
    assert!(
        reload_body.contains("StreamingState::Idle")
            && reload_body.contains("handle_load_plugins_with_channels"),
        "full driver playback restart should be reserved for the typed idle-engine fallback"
    );
}

#[test]
fn daemon_serializes_startup_before_rotating_the_audio_key() {
    let source = daemon_source();
    let lock = source
        .find("acquire_daemon_instance_lock(&ownership_resources)")
        .expect("daemon startup should acquire the complete runtime ownership lock set");
    let construct = source
        .find("let daemon = AudioDaemon::new()")
        .expect("daemon should be constructed after locking");
    let rotate = source
        .find("force_rotate()")
        .expect("daemon startup should rotate the AEAD session key");

    assert!(
        lock < construct && construct < rotate,
        "runtime ownership locks must precede KeyManager construction and key rotation"
    );
}

#[test]
fn daemon_load_plugins_carries_hal_input_channels() {
    let source = daemon_source();

    assert!(
        source.contains("const MAX_HAL_CHANNELS: usize = 32"),
        "daemon should validate HAL channel counts up to 32"
    );
    assert!(
        source.contains("input_channels: usize")
            && source.contains("PipelineSupervisor")
            && source.contains("DriverConfig::new(")
            && source.contains("plan.spec.input_channels as u32"),
        "load_plugins should carry requested HAL input channels into driver config"
    );
    assert!(
        source.contains("start_hal_playback_with_driver_config(")
            && source.contains("plan.spec.input_channels"),
        "driver playback should be restarted with the resolved HAL input channel count"
    );
}

#[test]
fn daemon_metering_returns_channel_sized_fallbacks() {
    let source = daemon_source();

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
    let source = daemon_source();

    assert!(
        source.contains("\"selected_device\": selected_device")
            && source.contains("\"input_channels\": input_channels")
            && source.contains("\"output_channels\": output_channels")
            && source.contains("\"channels\": engine_state.num_channels")
            && source.contains("\"playback_output_device\": engine_state.playback_output_device")
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
            && source.contains("let inputChannels: Int?")
            && source.contains("data[\"input_channels\"]?.value as? Int")
            && source.contains("let outputChannels: Int?")
            && source.contains("data[\"output_channels\"]?.value as? Int")
            && source.contains("applyLoadedDevices(daemonSelectedDevice: status.selectedDevice)")
            && source.contains("programmaticDeviceSelection = daemonDevice"),
        "toolbar should parse selected_device from status and update its picker without re-owning daemon state"
    );
}

#[test]
fn configbar_reconciles_daemon_owned_channel_counts_from_status() {
    let source = include_str!("../configbar/src/ConfigBar.swift");

    assert!(
        source.contains("if let inputChannels = status.inputChannels")
            && source.contains("let confirmed = min(max(inputChannels, 1), 32)")
            && source.contains("halInputChannels = confirmed")
            && source.contains("syncMeterArrays(inputChannels: confirmed)"),
        "toolbar status sync should adopt daemon-owned HAL input channels"
    );
    assert!(
        source.contains("let daemonOutputChannels = status.outputChannels ?? status.channels")
            && source.contains("halOutputChannels = confirmed")
            && source.contains("syncMeterArrays(outputChannels: confirmed)"),
        "toolbar status sync should prefer daemon-owned output channels while keeping legacy status.channels fallback"
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
        preinstall_source.contains("quit_systemwide_app")
            && preinstall_source
                .contains("tell application id \"org.spinorama.sotf-systemwide\" to quit")
            && preinstall_source.contains("/usr/bin/pkill -TERM -x \"sotf-systemwide\""),
        "preinstall should quit the running menu bar app before replacing the app bundle"
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
    assert!(
        preinstall_source.contains("cleanup_sotf_runtime_files")
            && preinstall_source.contains("${runtime_dir}/daemon.sock")
            && preinstall_source.contains("${runtime_dir}/audio.shm")
            && preinstall_source.contains("${runtime_dir}/session.key"),
        "preinstall should remove stale daemon socket/shared-memory runtime files after shutdown"
    );
}

#[test]
fn standalone_hal_installer_quiesces_running_system_before_replacing_driver() {
    let source = include_str!("../../../../../scripts/build-systemwide.sh");
    let install_start = source
        .find("cat > \"$DMG_DIR/install-hal.sh\"")
        .expect("standalone HAL install script should exist");
    let uninstall_start = source
        .find("cat > \"$DMG_DIR/uninstall-hal.sh\"")
        .expect("standalone HAL uninstall script should exist");
    let install_source = &source[install_start..uninstall_start];

    assert!(
        install_source.contains("quit_systemwide_app")
            && install_source.contains("quiesce_sotf_daemon")
            && install_source.find("quiesce_sotf_daemon").unwrap()
                < install_source
                    .find("for bundle in \"${LEGACY_BUNDLES[@]}\"")
                    .unwrap(),
        "standalone HAL installer should stop app/daemon before removing existing HAL bundles"
    );
    assert!(
        install_source.contains("{\"command\":\"shutdown\"}")
            && install_source.contains("sudo /usr/bin/pkill -TERM -x \"sotf-daemon\"")
            && install_source.contains("sudo /usr/bin/pkill -KILL -x \"sotf-daemon\""),
        "standalone HAL installer should use graceful daemon shutdown and sudo kill fallback"
    );
    assert!(
        install_source.contains("cleanup_sotf_runtime_files")
            && install_source.contains("${runtime_dir}/audio.shm"),
        "standalone HAL installer should clean stale runtime files before replacing the driver"
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
    let configbar_pure = include_str!("../configbar/src/ConfigBarPure.swift");
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
            && configbar.contains("\"input_channels\": requestedInputChannels"),
        "toolbar should expose and send HAL input channel counts up to 32"
    );
    assert!(
        configbar.contains("syncOutputChannelsToSelectedDevice(applyChange: true)"),
        "device refresh/selection should clamp the channel selection when metadata changes"
    );
    assert!(
        configbar.contains("@State private var deviceRecoveryTimer")
            && configbar.contains("Waiting for CoreAudio hardware devices...")
            && configbar.contains("startDeviceRecoveryPolling()")
            && configbar.contains("stopDeviceRecoveryPolling()"),
        "toolbar should keep polling while CoreAudio temporarily reports no hardware output devices"
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
            && configbar_pure.contains("sanitizeConfigBarPeaks"),
        "toolbar meters should resize and clamp to the current N-channel layout"
    );
}

#[test]
fn configbar_hal_stream_status_wording_matches_signal_scope() {
    let configbar = include_str!("../configbar/src/ConfigBar.swift");
    let shared_memory = include_str!("../../driver-hal/swift/Sources/SharedMemory.swift");

    assert!(
        configbar.contains("HAL Stream Active")
            && configbar.contains("HAL Stream Idle")
            && !configbar.contains("\"No Audio\""),
        "HAL status should describe the virtual HAL stream, not imply the whole system has no audio"
    );
    assert!(
        shared_memory.contains("if atomicLoad(&header.pointee.active) == 0")
            && shared_memory.contains("atomicStore(&header.pointee.active, 1)")
            && shared_memory.contains(
                "atomicStore(&header.pointee.writePosition, writePos + UInt64(samplesToWrite))"
            )
            && shared_memory.contains(
                "atomicStore(&header.pointee.writePosition, writePos + UInt64(floatCount))"
            ),
        "successful HAL shared-memory writes should mark the HAL stream active even if StartIO state was stale"
    );
}

#[test]
fn configbar_menu_bar_icon_uses_health_tint_streaming_background_and_recording_dot() {
    let configbar = include_str!("../configbar/src/ConfigBar.swift");

    assert!(
        configbar.contains("let issue = !daemonRunning || currentState == .error")
            && configbar.contains("image.isTemplate = true")
            && !configbar.contains("button.contentTintColor"),
        "menu bar should let AppKit tint the template icon for the active appearance"
    );
    assert!(
        configbar.contains(
            "layer.backgroundColor = streaming ? NSColor.systemGreen.cgColor : NSColor.clear.cgColor"
        ),
        "streaming audio should use a green menu bar icon background"
    );
    assert!(
        configbar.contains("setRecordingDotVisible(currentState == .recording && !issue")
            && configbar.contains("dot.backgroundColor = NSColor.systemOrange.cgColor"),
        "recording should add an orange dot on top of the streaming status icon"
    );
    assert!(
        !configbar.contains("button.contentTintColor = .systemRed")
            && !configbar.contains("button.contentTintColor = .systemGreen"),
        "status should not encode health by tinting the icon red or green"
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
        source.contains("applyPluginUpdate(for: pluginID, parameters: newParams)")
            && source.contains("\"command\": \"update_plugin\"")
            && source.contains("client.sendCommandAsync("),
        "only Apply/Close should send parameters asynchronously to the daemon"
    );
}

#[test]
fn configbar_and_daemon_support_isolated_lab_runtime_paths() {
    let configbar = include_str!("../configbar/src/ConfigBar.swift");
    let daemon = daemon_source();
    let security = [
        include_str!("../bin/security.rs"),
        include_str!("../bin/security/get.rs"),
        include_str!("../bin/security/misc.rs"),
    ]
    .join("\n");

    assert!(
        configbar.contains("SOTF_DAEMON_SOCKET_PATH")
            && configbar.contains("SOTF_SYSTEMWIDE_RUNTIME_DIR")
            && configbar.contains("daemon.sock"),
        "toolbar should be able to connect to an isolated lab daemon socket"
    );
    assert!(
        daemon.contains("SOTF_DAEMON_SOCKET_PATH")
            && daemon.contains("SOTF_SYSTEMWIDE_RUNTIME_DIR")
            && security.contains("secure_socket_path_from_env"),
        "daemon should bind to explicit lab socket/runtime-dir overrides"
    );
}

#[test]
fn configbar_plugin_chain_loader_delegates_artifact_planning_to_daemon() {
    let source = include_str!("../configbar/src/ConfigBar.swift");

    assert!(
        source.contains("\"command\": \"load_plugin_artifact\"")
            && source.contains("\"artifact\": json")
            && !source.contains("private func normalizedPluginConfigs"),
        "toolbar should send whole plugin artifacts to the daemon instead of normalizing them locally"
    );
    assert!(
        !source.contains("allPlugins.append(contentsOf: channelPlugins)"),
        "toolbar must not flatten per-channel or graph-style plugin artifacts into a linear rack"
    );
}

#[test]
fn configbar_channel_apply_uses_patch_intent_instead_of_replaying_plugins() {
    let source = include_str!("../configbar/src/ConfigBar.swift");
    let apply_start = source
        .find("private func applyHALConfiguration()")
        .expect("applyHALConfiguration should exist");
    let load_start = source
        .find("private func loadPluginConfig()")
        .expect("loadPluginConfig should exist");
    let body = &source[apply_start..load_start];

    assert!(
        body.contains("\"command\": \"set_pipeline_channels\"")
            && body.contains("\"input_channels\": requestedInputChannels")
            && body.contains("\"output_channels\": requestedOutputChannels"),
        "HAL channel apply should use the daemon's typed channel patch command"
    );
    assert!(
        !body.contains("client.getPlugins()") && !body.contains("\"command\": \"load_plugins\""),
        "HAL channel apply must not replay a stale plugin list"
    );
}
