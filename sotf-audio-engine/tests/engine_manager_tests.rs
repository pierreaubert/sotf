//! Manager Thread Tests
//!
//! Unit tests for the manager thread that coordinates all worker threads.

mod common;

use sotf_audio::engine::{AudioEngine, PlaybackState, PluginConfig};
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_engine_creation() {
    let config = common::test_engine_config();
    let engine = AudioEngine::new(config);

    assert!(engine.is_ok(), "Failed to create audio engine");
}

#[test]
fn test_engine_initial_state() {
    let config = common::test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let state = engine.get_state();

    assert_eq!(state.playback_state, PlaybackState::Stopped);
    assert_eq!(state.position, 0.0);
    assert_eq!(state.volume, 1.0);
    assert!(!state.muted);
    assert_eq!(state.current_file, None);
}

#[test]
fn test_engine_play_file() {
    let _ = env_logger::builder().is_test(true).try_init();
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = common::create_test_wav(0.5, 48000, 2);
    let path = temp_file.path().to_path_buf();

    let result = engine.play(path.clone());
    assert!(result.is_ok(), "Failed to start playback");

    std::thread::sleep(Duration::from_millis(100));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
    assert_eq!(state.current_file, Some(path));
}

#[test]
fn test_engine_pause_resume() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = common::create_test_wav(1.0, 48000, 2);
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
fn test_engine_stop() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = common::create_test_wav(1.0, 48000, 2);
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
fn test_engine_seek() {
    let _ = env_logger::builder().is_test(true).try_init();
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = common::create_test_wav(2.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    std::thread::sleep(Duration::from_millis(200));

    // Seek to 1 second
    engine.seek(1.0).unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let position = engine.get_position().unwrap();

    // Position should be around 1 second (with some tolerance for timing)
    assert!(
        (position - 1.0).abs() < 0.2,
        "Position after seek should be ~1.0s, got {}s",
        position
    );
}

#[test]
fn test_engine_volume_control() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

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
fn test_engine_mute() {
    let _ = env_logger::builder().is_test(true).try_init();
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

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
fn test_engine_update_plugin_chain() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    // Create a simple plugin chain with a gain plugin
    let plugins = vec![PluginConfig::new(
        "gain",
        serde_json::json!({
            "gain_db": -6.0
        }),
    )];

    let result = engine.update_plugin_chain(plugins);
    assert!(result.is_ok(), "Failed to update plugin chain");

    std::thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_engine_bypass_processing() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

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
fn test_engine_invalid_file() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    let _result = engine.play(PathBuf::from("/nonexistent/file.wav"));

    // Should return an error or handle gracefully
    // The error might be immediate or async, so we check state
    std::thread::sleep(Duration::from_millis(200));

    let state = engine.get_state();
    // Should either fail to start or report an error
    assert!(
        state.playback_state == PlaybackState::Stopped || state.last_error.is_some(),
        "Should handle invalid file gracefully"
    );
}

#[test]
fn test_engine_multiple_plays() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    // Play first file
    let temp_file1 = common::create_test_wav(0.3, 48000, 2);
    engine.play(temp_file1.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Play second file (should stop first)
    let temp_file2 = common::create_test_wav(0.3, 48000, 2);
    engine.play(temp_file2.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
    assert_eq!(state.current_file, Some(temp_file2.path().to_path_buf()));
}

#[test]
fn test_engine_rapid_commands() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = common::create_test_wav(2.0, 48000, 2);
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
fn test_engine_shutdown() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = common::create_test_wav(1.0, 48000, 2);
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
fn test_engine_position_tracking() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = common::create_test_wav(2.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    // Track position over time
    let mut positions = Vec::new();

    for _ in 0..5 {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(pos) = engine.get_position() {
            positions.push(pos);
        }
    }

    // Positions should generally increase
    assert!(positions.len() >= 3, "Should get position updates");

    let increasing = positions.windows(2).filter(|w| w[1] > w[0]).count();
    let total_windows = positions.len() - 1;

    // Most position updates should show forward progress
    assert!(
        increasing as f32 / total_windows as f32 > 0.5,
        "Position should generally increase during playback"
    );
}

#[test]
fn test_engine_state_consistency() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = common::create_test_wav(1.0, 48000, 2);
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
fn test_engine_config_with_plugins() {
    let plugins = vec![
        PluginConfig::new("gain", serde_json::json!({"gain_db": -3.0})),
        PluginConfig::new("gain", serde_json::json!({"gain_db": 6.0})),
    ];

    let config = common::test_engine_config_with(|c| {
        c.plugins = plugins;
    });

    let engine = AudioEngine::new(config);
    assert!(engine.is_ok(), "Should create engine with plugin config");
}

#[test]
fn test_engine_custom_sample_rate() {
    let config = common::test_engine_config_with(|c| {
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
fn test_engine_drop_cleanup() {
    let config = common::test_engine_config();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = common::create_test_wav(1.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    std::thread::sleep(Duration::from_millis(100));

    // Drop should clean up all threads
    drop(engine);

    std::thread::sleep(Duration::from_millis(100));
    // Should complete without panic
}
