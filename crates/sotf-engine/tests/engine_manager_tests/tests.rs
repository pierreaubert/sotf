use serial_test::serial;
use sotf_audio::engine::{AudioEngine, PlaybackState, PluginConfig};
use sotf_audio::manager::{AudioEngineManager, StreamingEvent, StreamingState};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const ASYNC_STATE_TIMEOUT: Duration = Duration::from_secs(3);

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    condition()
}

#[test]
#[serial]
fn test_engine_creation() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config);

    assert!(engine.is_ok(), "Failed to create audio engine");
}

#[test]
#[serial]
fn test_engine_initial_state() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let state = engine.get_state();

    assert_eq!(state.playback_state, PlaybackState::Stopped);
    assert_eq!(state.position, 0.0);
    assert_eq!(state.volume, 1.0);
    assert!(!state.muted);
    assert_eq!(state.current_file, None);
}

#[test]
#[serial]
fn test_engine_get_playback_state_returns_playback_state_without_cloning() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    assert_eq!(engine.get_playback_state(), PlaybackState::Stopped);
}

#[test]
#[serial]
fn test_engine_play_file() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();
    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(0.5, 48000, 2);
    let path = temp_file.path().to_path_buf();

    let result = engine.play(path.clone());
    assert!(result.is_ok(), "Failed to start playback");

    std::thread::sleep(Duration::from_millis(100));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
    assert_eq!(state.current_file, Some(path));
}

#[test]
#[serial]
fn test_engine_pause_resume() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(1.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    std::thread::sleep(Duration::from_millis(100));

    // Pause
    engine.pause().unwrap();
    std::thread::sleep(Duration::from_millis(50));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Paused);

    // Resume
    engine.resume().unwrap();
    std::thread::sleep(Duration::from_millis(50));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
}

#[test]
#[serial]
fn test_engine_stop() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(1.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    std::thread::sleep(Duration::from_millis(100));

    // Stop
    engine.stop().unwrap();
    std::thread::sleep(Duration::from_millis(50));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Stopped);
    assert_eq!(state.position, 0.0);
}

#[test]
#[serial]
fn test_engine_seek() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();
    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(2.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    std::thread::sleep(Duration::from_millis(200));

    // Seek to 1 second
    engine.seek(1.0).unwrap();
    assert!(
        wait_until(ASYNC_STATE_TIMEOUT, || !engine.get_state().seeking),
        "Seek did not complete within {ASYNC_STATE_TIMEOUT:?}"
    );

    let position = engine.get_state().position;

    // Position should be around 1 second (with some tolerance for timing)
    assert!(
        (position - 1.0).abs() < 0.2,
        "Position after seek should be ~1.0s, got {}s",
        position
    );
}

#[test]
#[serial]
fn test_engine_seek_without_file_returns_error() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let result = engine.seek(1.0);

    assert!(
        result.is_err(),
        "Seek without a loaded file should fail synchronously"
    );

    let state = engine.get_state();
    assert!(!state.seeking);
    assert_eq!(state.position, 0.0);
}

#[test]
#[serial]
fn test_engine_volume_control() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    // Test various volume levels
    let volumes = [0.0, 0.5, 1.0, 1.5, 2.0];

    for &vol in &volumes {
        engine.set_volume(vol).unwrap();
        std::thread::sleep(Duration::from_millis(20));

        let state = engine.get_state();
        assert_eq!(state.volume, vol, "Volume should be set to {}", vol);
    }
}

#[test]
#[serial]
fn test_engine_mute() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();
    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    // Mute
    engine.set_mute(true).unwrap();
    std::thread::sleep(Duration::from_millis(20));

    let state = engine.get_state();
    assert!(state.muted);

    // Unmute
    engine.set_mute(false).unwrap();
    std::thread::sleep(Duration::from_millis(20));

    let state = engine.get_state();
    assert!(!state.muted);
}

#[test]
#[serial]
fn test_engine_update_plugin_chain() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    // Create a simple plugin chain with a gain plugin
    let plugins = vec![PluginConfig::new(
        "gain",
        serde_json::json!({
            "gain_db": -6.0
        }),
    )];

    let result = engine.update_plugin_chain(&plugins);
    assert!(result.is_ok(), "Failed to update plugin chain");

    std::thread::sleep(Duration::from_millis(100));
}

#[test]
#[serial]
fn test_engine_update_plugin_chain_allows_upmixer_channel_increase() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();
    let config = super::common::test_engine_config_with(|c| {
        c.output_channels = 2;
    });
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(2.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    std::thread::sleep(Duration::from_millis(200));

    // Adding an upmixer at runtime should succeed — the playback thread
    // handles channel count changes via UpdateChannels and downmixes if needed.
    let plugins = vec![PluginConfig::new(
        "upmixer",
        serde_json::json!({
            "speaker_config": "5.0"
        }),
    )];

    let result = engine.update_plugin_chain(&plugins);
    assert!(
        result.is_ok(),
        "Upmixer update should succeed, got: {:?}",
        result.err()
    );

    std::thread::sleep(Duration::from_millis(200));

    let state = engine.get_state();
    assert_eq!(state.num_channels, 5);
    assert!(
        state.last_error.is_none(),
        "Expected no error, got: {:?}",
        state.last_error
    );
}

#[test]
#[serial]
fn test_engine_bypass_processing() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    // Enable bypass
    engine.set_bypass(true).unwrap();
    std::thread::sleep(Duration::from_millis(20));

    let state = engine.get_state();
    assert!(state.processing_bypassed);

    // Disable bypass
    engine.set_bypass(false).unwrap();
    std::thread::sleep(Duration::from_millis(20));

    let state = engine.get_state();
    assert!(!state.processing_bypassed);
}

#[test]
#[serial]
fn test_engine_set_plugin_parameter_propagates_processing_error() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let result = engine.set_plugin_parameter(999, "gain_db".to_string(), "-6.0".to_string());

    assert!(
        result.is_err(),
        "Invalid plugin index should return an error"
    );
}

#[test]
#[serial]
fn test_engine_invalid_file() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let result = engine.play(PathBuf::from("/nonexistent/file.wav"));

    assert!(result.is_err(), "Invalid file should fail synchronously");

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Stopped);
}

#[test]
#[serial]
fn test_engine_multiple_plays() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    // Play first file
    let temp_file1 = super::common::create_test_wav(0.3, 48000, 2);
    engine.play(temp_file1.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Play second file (should stop first)
    let temp_file2 = super::common::create_test_wav(0.3, 48000, 2);
    engine.play(temp_file2.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
    assert_eq!(state.current_file, Some(temp_file2.path().to_path_buf()));
}

#[test]
#[serial]
fn test_engine_rapid_commands() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(2.0, 48000, 2);
    let path = temp_file.path().to_path_buf();

    // Rapidly send various commands
    for i in 0..20 {
        match i % 4 {
            0 => engine.play(path.clone()).ok(),
            1 => engine.pause().ok(),
            2 => engine.resume().ok(),
            3 => engine.set_volume(i as f32 / 20.0).ok(),
            _ => None,
        };

        std::thread::sleep(Duration::from_millis(10));
    }

    // Should handle rapid commands without crashing
    std::thread::sleep(Duration::from_millis(100));
    let state = engine.get_state();
    assert!(
        state.last_error.is_none(),
        "Engine should remain responsive"
    );
}

#[test]
#[serial]
fn test_engine_shutdown() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(1.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    std::thread::sleep(Duration::from_millis(100));

    // Shutdown
    engine.shutdown().unwrap();

    std::thread::sleep(Duration::from_millis(100));

    // Commands should fail after shutdown
    let result = engine.play(temp_file.path().to_path_buf());
    assert!(result.is_err(), "Commands should fail after shutdown");
}

#[test]
#[serial]
fn test_engine_position_tracking() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(2.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    // Track distinct position reports until there is enough evidence of progress.
    let mut positions: Vec<f64> = Vec::new();
    let started = Instant::now();
    while started.elapsed() < ASYNC_STATE_TIMEOUT && positions.len() < 3 {
        if let Ok(pos) = engine.get_position()
            && positions
                .last()
                .is_none_or(|previous| (pos - *previous).abs() > f64::EPSILON)
        {
            positions.push(pos);
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(
        positions.len() >= 3,
        "Should get at least three distinct position updates within {ASYNC_STATE_TIMEOUT:?}; got {positions:?}"
    );
    assert!(
        positions.windows(2).all(|window| window[1] > window[0]),
        "Distinct positions should increase during playback; got {positions:?}"
    );
}

#[test]
#[serial]
fn test_engine_state_consistency() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(1.0, 48000, 2);
    let path = temp_file.path().to_path_buf();

    // Play
    engine.play(path.clone()).unwrap();
    std::thread::sleep(Duration::from_millis(50));

    let state1 = engine.get_state();
    assert_eq!(state1.playback_state, PlaybackState::Playing);
    assert_eq!(state1.current_file, Some(path));

    // Pause
    engine.pause().unwrap();
    std::thread::sleep(Duration::from_millis(50));

    let state2 = engine.get_state();
    assert_eq!(state2.playback_state, PlaybackState::Paused);
    assert_eq!(state2.current_file, state1.current_file);

    // Stop
    engine.stop().unwrap();
    std::thread::sleep(Duration::from_millis(50));

    let state3 = engine.get_state();
    assert_eq!(state3.playback_state, PlaybackState::Stopped);
}

#[test]
#[serial]
fn test_engine_config_with_plugins() {
    super::common::skip_without_device!();

    let plugins = vec![
        PluginConfig::new("gain", serde_json::json!({"gain_db": -3.0})),
        PluginConfig::new("gain", serde_json::json!({"gain_db": 6.0})),
    ];

    let config = super::common::test_engine_config_with(|c| {
        c.plugins = plugins;
    });

    let engine = AudioEngine::new(config);
    assert!(engine.is_ok(), "Should create engine with plugin config");
}

#[test]
#[serial]
fn test_engine_custom_sample_rate() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config_with(|c| {
        c.output_sample_rate = 96000;
    });

    let result = AudioEngine::new(config);

    match result {
        Ok(engine) => {
            let state = engine.get_state();
            // State might report different sample rate if no file is loaded
            assert!(state.sample_rate > 0, "Sample rate should be positive");
        }
        Err(e) => {
            // May fail if hardware doesn't support 96kHz
            assert!(
                e.contains("audio") || e.contains("device") || e.contains("rate"),
                "Error should be audio-related: {}",
                e
            );
        }
    }
}

#[test]
#[serial]
fn test_engine_drop_cleanup() {
    super::common::skip_without_device!();

    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(1.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    std::thread::sleep(Duration::from_millis(100));

    // Drop should clean up all threads
    drop(engine);

    std::thread::sleep(Duration::from_millis(100));
    // Should complete without panic
}

#[test]
#[serial]
fn test_engine_remove_plugin_during_playback() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();
    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(5.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    std::thread::sleep(Duration::from_millis(200));

    // Add a gain plugin
    let plugins = vec![PluginConfig::new(
        "gain",
        serde_json::json!({"gain_db": -3.0}),
    )];
    engine.update_plugin_chain(&plugins).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Remove it (empty chain)
    engine.update_plugin_chain(&[]).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    let state = engine.get_state();
    assert_eq!(
        state.playback_state,
        PlaybackState::Playing,
        "Playback should continue after removing plugin"
    );
    assert!(
        state.last_error.is_none(),
        "No error expected, got: {:?}",
        state.last_error
    );

    // Verify audio is still flowing (position should have advanced)
    let pos1 = engine.get_position().unwrap();
    let mut pos2 = pos1;
    assert!(
        wait_until(ASYNC_STATE_TIMEOUT, || {
            pos2 = engine.get_position().unwrap();
            pos2 > pos1 + 0.05
        }),
        "Position should advance after plugin removal within {ASYNC_STATE_TIMEOUT:?}: {pos1} -> {pos2}"
    );
}

#[test]
#[serial]
fn test_engine_remove_all_plugins_during_playback() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();
    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(2.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Start with two plugins
    let plugins = vec![
        PluginConfig::new("gain", serde_json::json!({"gain_db": -3.0})),
        PluginConfig::new("gain", serde_json::json!({"gain_db": -6.0})),
    ];
    engine.update_plugin_chain(&plugins).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Remove all plugins at once
    engine.update_plugin_chain(&[]).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
    assert!(state.last_error.is_none());
}

#[test]
#[serial]
fn test_engine_rapid_plugin_updates_during_playback() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();
    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(3.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Rapidly update plugin chain 10 times
    for i in 0..10 {
        let gain = -(i as f64) * 0.5;
        let plugins = vec![PluginConfig::new(
            "gain",
            serde_json::json!({"gain_db": gain}),
        )];
        let result = engine.update_plugin_chain(&plugins);
        assert!(
            result.is_ok(),
            "Update {} should succeed, got: {:?}",
            i,
            result.err()
        );
    }

    std::thread::sleep(Duration::from_millis(300));

    let state = engine.get_state();
    assert_eq!(
        state.playback_state,
        PlaybackState::Playing,
        "Playback should survive rapid plugin updates"
    );
    assert!(
        state.last_error.is_none(),
        "No error expected, got: {:?}",
        state.last_error
    );
}

#[test]
#[serial]
fn test_engine_update_preserves_playback_after_channel_change() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();
    let config = super::common::test_engine_config_with(|c| {
        c.output_channels = 2;
    });
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(10.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Add upmixer (2ch -> 5ch)
    let plugins = vec![PluginConfig::new(
        "upmixer",
        serde_json::json!({"speaker_config": "5.0"}),
    )];
    engine.update_plugin_chain(&plugins).unwrap();
    assert!(
        wait_until(ASYNC_STATE_TIMEOUT, || engine.get_state().num_channels == 5),
        "Channel count did not reach 5 within {ASYNC_STATE_TIMEOUT:?}; state: {:?}",
        engine.get_state()
    );

    let state = engine.get_state();
    assert_eq!(state.num_channels, 5, "Should be 5 channels after upmixer");
    assert_eq!(state.playback_state, PlaybackState::Playing);

    // Remove upmixer (back to 2ch)
    engine.update_plugin_chain(&[]).unwrap();
    assert!(
        wait_until(ASYNC_STATE_TIMEOUT, || engine.get_state().num_channels == 2),
        "Channel count did not return to 2 within {ASYNC_STATE_TIMEOUT:?}; state: {:?}",
        engine.get_state()
    );

    let state = engine.get_state();
    assert_eq!(
        state.num_channels, 2,
        "Should be 2 channels after removing upmixer"
    );
    assert_eq!(
        state.playback_state,
        PlaybackState::Playing,
        "Playback should continue after channel count change"
    );
    assert!(
        state.last_error.is_none(),
        "No error expected, got: {:?}",
        state.last_error
    );
}

#[test]
#[serial]
fn test_manager_idle_volume_and_mute_are_persisted() {
    let manager = AudioEngineManager::new();

    manager.set_volume(0.65).unwrap();
    manager.set_mute(true).unwrap();

    assert_eq!(manager.get_state(), StreamingState::Idle);
    assert_eq!(manager.get_volume(), 0.65);
    assert!(manager.is_muted());
}

#[test]
#[serial]
fn test_engine_auto_advance_after_end_of_stream() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();

    let device = super::common::require_virtual_device();
    let mut manager = AudioEngineManager::new();
    manager.set_allow_virtual_output(true);

    // Load and play a short file
    let temp_file1 = super::common::create_test_wav(0.3, 48000, 2);
    manager.load_file(temp_file1.path()).unwrap();
    manager
        .start_playback(Some(device.clone()), vec![], 2)
        .unwrap();

    // Wait for end-of-stream
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut got_eos = false;
    while Instant::now() < deadline {
        if let Some(StreamingEvent::EndOfStream) = manager.try_recv_event() {
            got_eos = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if !got_eos && manager.get_engine_state().playback_callback_count == 0 {
        eprintln!("Skipping EOF auto-advance test: virtual output device produced no callbacks");
        let _ = manager.stop();
        return;
    }
    assert!(got_eos, "Should receive EndOfStream within 5s");

    // Simulate the TUI auto-advance path: persist volume before the old engine
    // is stopped, then stop and start the next track. After EOS the manager is
    // Idle, but the old engine handle is still present until stop().
    let volume_result = manager.set_volume(0.65);
    assert!(
        volume_result.is_ok(),
        "set_volume() after end-of-stream should only persist the next volume, got: {:?}",
        volume_result.err()
    );

    let stop_result = manager.stop();
    assert!(
        stop_result.is_ok(),
        "stop() after end-of-stream should succeed, got: {:?}",
        stop_result.err()
    );

    // Load and play second file
    let temp_file2 = super::common::create_test_wav(0.5, 48000, 2);
    manager.load_file(temp_file2.path()).unwrap();
    manager.start_playback(Some(device), vec![], 2).unwrap();

    std::thread::sleep(Duration::from_millis(100));

    assert_eq!(manager.get_state(), StreamingState::Playing);
    assert_eq!(manager.get_volume(), 0.65);
}

#[test]
#[serial]
fn test_engine_eof_transition_to_stopped() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();
    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    // Short 0.2s file
    let temp_file = super::common::create_test_wav(0.2, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    // Wait for playback to reach the end and stop.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut stopped = false;
    while Instant::now() < deadline {
        if engine.get_playback_state() == PlaybackState::Stopped {
            stopped = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    if !stopped && engine.get_state().playback_callback_count == 0 {
        eprintln!("Skipping EOF transition test: virtual output device produced no callbacks");
        return;
    }
    assert!(
        stopped,
        "Engine should transition to Stopped after reaching EOF"
    );
    let position = engine.get_position().unwrap();
    assert!(
        (0.15..=0.25).contains(&position),
        "Position at EOF should be ~0.2s, got {position}s"
    );
    let state = engine.get_state();
    assert!(
        state.last_error.is_none(),
        "No error expected at EOF, got: {:?}",
        state.last_error
    );
}

#[test]
#[serial]
fn test_engine_44100_source_on_48000_device() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();
    let config = super::common::test_engine_config_with(|c| {
        c.output_sample_rate = 48000;
    });
    let engine = AudioEngine::new(config).unwrap();

    // Source file is 44.1 kHz while the engine/output device runs at 48 kHz.
    let temp_file = super::common::create_test_wav(1.0, 44100, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    std::thread::sleep(Duration::from_millis(300));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
    assert!(
        state.last_error.is_none(),
        "Sample-rate mismatch should not produce an error, got: {:?}",
        state.last_error
    );

    let pos1 = engine.get_position().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let pos2 = engine.get_position().unwrap();
    assert!(
        pos2 > pos1,
        "Position should advance despite sample-rate mismatch: {pos1} -> {pos2}"
    );
}

#[test]
#[serial]
fn test_engine_high_gain_plugin_does_not_crash_or_error() {
    super::common::skip_without_device!();

    let _ = env_logger::builder().is_test(true).try_init();
    let config = super::common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = super::common::create_test_wav(5.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Extreme gain will push samples outside [-1, 1]. The engine's output
    // path clamps before the device, so the engine must remain playable.
    let plugins = vec![PluginConfig::new(
        "gain",
        serde_json::json!({"gain_db": 30.0}),
    )];
    engine.update_plugin_chain(&plugins).unwrap();

    let pos1 = engine.get_position().unwrap();
    let mut pos2 = pos1;
    assert!(
        wait_until(ASYNC_STATE_TIMEOUT, || {
            pos2 = engine.get_position().unwrap();
            pos2 > pos1 + 0.05
        }),
        "Position should advance with high-gain plugin within {ASYNC_STATE_TIMEOUT:?}: {pos1} -> {pos2}"
    );

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
    assert!(
        state.last_error.is_none(),
        "High-gain plugin should not cause an engine error, got: {:?}",
        state.last_error
    );
}
