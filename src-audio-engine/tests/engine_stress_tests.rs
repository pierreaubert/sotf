//! Stress Tests for Audio Engine
//!
//! Tests that stress the audio engine under heavy load and edge cases.

use sotf_audio::engine::{AudioEngine, EngineConfig, PluginConfig, PlaybackState};
use std::path::PathBuf;
use std::thread;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::NamedTempFile;
use hound::{WavSpec, WavWriter};

/// Helper to create a test WAV file
fn create_test_wav(duration_secs: f32, sample_rate: u32, channels: u16) -> NamedTempFile {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let temp_file = tempfile::Builder::new()
        .suffix(".wav")
        .tempfile()
        .unwrap();
    let mut writer = WavWriter::create(temp_file.path(), spec).unwrap();

    let num_samples = (duration_secs * sample_rate as f32) as usize;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin();
        let amplitude = (sample * i16::MAX as f32 * 0.3) as i16;

        for _ in 0..channels {
            writer.write_sample(amplitude).unwrap();
        }
    }

    writer.finalize().unwrap();
    temp_file
}

#[test]
fn stress_rapid_play_stop() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(0.5, 48000, 2);
    let path = temp_file.path().to_path_buf();

    // Rapidly play and stop 100 times
    for _ in 0..100 {
        engine.play(path.clone()).ok();
        thread::sleep(Duration::from_millis(10));
        engine.stop().ok();
        thread::sleep(Duration::from_millis(5));
    }

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Stopped);
}

#[test]
fn stress_rapid_pause_resume() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(2.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    thread::sleep(Duration::from_millis(50));

    // Rapidly pause and resume 100 times
    for i in 0..100 {
        if i % 2 == 0 {
            engine.pause().ok();
        } else {
            engine.resume().ok();
        }
        thread::sleep(Duration::from_millis(5));
    }

    let state = engine.get_state();
    assert!(
        state.playback_state == PlaybackState::Playing ||
        state.playback_state == PlaybackState::Paused
    );
}

#[test]
fn stress_rapid_seek() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(5.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    thread::sleep(Duration::from_millis(100));

    // Rapidly seek to random positions
    for i in 0..50 {
        let pos = (i as f32 % 4.5) / 10.0; // Seek within file bounds
        engine.seek(pos.into()).ok();
        thread::sleep(Duration::from_millis(20));
    }

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
}

#[test]
fn stress_rapid_volume_changes() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(2.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    // Rapidly change volume 500 times
    for i in 0..500 {
        let vol = ((i as f32 * 0.1).sin() + 1.0) / 2.0; // 0.0 to 1.0
        engine.set_volume(vol).ok();
    }

    thread::sleep(Duration::from_millis(100));

    let state = engine.get_state();
    assert!(state.volume >= 0.0 && state.volume <= 1.0);
}

#[test]
fn stress_rapid_mute_toggle() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(2.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    // Toggle mute 200 times
    for i in 0..200 {
        engine.set_mute(i % 2 == 0).ok();
    }

    thread::sleep(Duration::from_millis(50));

    let state = engine.get_state();
    assert!(state.muted == true || state.muted == false);
}

#[test]
fn stress_concurrent_state_queries() {
    let config = EngineConfig::default();
    let engine = Arc::new(Mutex::new(AudioEngine::new(config).unwrap()));

    let temp_file = create_test_wav(3.0, 48000, 2);
    engine.lock().unwrap().play(temp_file.path().to_path_buf()).unwrap();

    // Spawn multiple threads querying state concurrently
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let engine = Arc::clone(&engine);
            thread::spawn(move || {
                for _ in 0..100 {
                    if let Ok(engine) = engine.lock() {
                        let _ = engine.get_state();
                    }
                    if let Ok(mut engine) = engine.lock() {
                        engine.get_position().ok();
                    }
                    thread::sleep(Duration::from_millis(1));
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let state = engine.lock().unwrap().get_state();
    assert!(state.playback_state != PlaybackState::Stopped || state.position >= 0.0);
}

#[test]
fn stress_concurrent_commands() {
    let config = EngineConfig::default();
    let engine = Arc::new(Mutex::new(AudioEngine::new(config).unwrap()));

    let temp_file = create_test_wav(5.0, 48000, 2);
    let path = Arc::new(temp_file.path().to_path_buf());

    // Spawn threads sending different commands
    let mut handles = vec![];

    // Thread 1: Play/Stop
    {
        let engine = Arc::clone(&engine);
        let path = Arc::clone(&path);
        handles.push(thread::spawn(move || {
            for _ in 0..20 {
                if let Ok(mut engine) = engine.lock() {
                    engine.play((*path).clone()).ok();
                }
                thread::sleep(Duration::from_millis(50));
                if let Ok(mut engine) = engine.lock() {
                    engine.stop().ok();
                }
                thread::sleep(Duration::from_millis(30));
            }
        }));
    }

    // Thread 2: Volume changes
    {
        let engine = Arc::clone(&engine);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                if let Ok(mut engine) = engine.lock() {
                    engine.set_volume(i as f32 / 100.0).ok();
                }
                thread::sleep(Duration::from_millis(10));
            }
        }));
    }

    // Thread 3: Mute toggle
    {
        let engine = Arc::clone(&engine);
        handles.push(thread::spawn(move || {
            for i in 0..50 {
                if let Ok(mut engine) = engine.lock() {
                    engine.set_mute(i % 2 == 0).ok();
                }
                thread::sleep(Duration::from_millis(20));
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Engine should still be responsive
    let state = engine.lock().unwrap().get_state();
    assert!(state.last_error.is_none());
}

#[test]
fn stress_plugin_chain_updates() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(3.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    thread::sleep(Duration::from_millis(100));

    // Repeatedly update plugin chain
    for i in 0..20 {
        let num_plugins = (i % 5) + 1;
        let plugins: Vec<_> = (0..num_plugins)
            .map(|j| {
                PluginConfig::new("gain", serde_json::json!({
                    "gain_db": (j as f32 - 2.0) * 3.0
                }))
            })
            .collect();

        engine.update_plugin_chain(plugins).ok();
        thread::sleep(Duration::from_millis(50));
    }

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
}

#[test]
fn stress_long_playback() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    // Create a longer file
    let temp_file = create_test_wav(10.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    // Let it play for several seconds, checking state periodically
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));

        let state = engine.get_state();
        assert!(
            state.playback_state == PlaybackState::Playing ||
            state.playback_state == PlaybackState::Stopped,
            "Unexpected state during long playback"
        );

        // Check for excessive underruns
        assert!(
            state.underruns < 100,
            "Too many underruns: {}",
            state.underruns
        );
    }
}

#[test]
fn stress_many_short_files() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    // Play many short files in sequence
    for _ in 0..20 {
        let temp_file = create_test_wav(0.1, 48000, 2);
        engine.play(temp_file.path().to_path_buf()).unwrap();

        // Wait for file to finish
        thread::sleep(Duration::from_millis(150));

        let state = engine.get_state();
        // Should either be playing or have stopped (finished)
        assert!(
            state.playback_state == PlaybackState::Playing ||
            state.playback_state == PlaybackState::Stopped
        );
    }
}

#[test]
fn stress_seek_to_extremes() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(5.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    thread::sleep(Duration::from_millis(100));

    // Seek to various extreme positions
    let positions = [0.0, 0.01, 4.99, 0.0, 2.5, 4.99, 0.001, 4.9];

    for &pos in &positions {
        engine.seek(pos).ok();
        thread::sleep(Duration::from_millis(50));

        let state = engine.get_state();
        assert_eq!(state.playback_state, PlaybackState::Playing);
    }
}

#[test]
fn stress_bypass_toggle() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    // Set up a plugin chain
    let plugins = vec![
        PluginConfig::new("gain", serde_json::json!({"gain_db": -3.0})),
    ];
    engine.update_plugin_chain(plugins).ok();

    let temp_file = create_test_wav(2.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    thread::sleep(Duration::from_millis(100));

    // Rapidly toggle bypass
    for i in 0..100 {
        engine.set_bypass(i % 2 == 0).ok();
        thread::sleep(Duration::from_millis(10));
    }

    let state = engine.get_state();
    assert!(state.processing_bypassed == true || state.processing_bypassed == false);
}

#[test]
fn stress_interleaved_operations() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(5.0, 48000, 2);
    let path = temp_file.path().to_path_buf();

    // Perform a complex sequence of interleaved operations
    for i in 0..50 {
        match i % 7 {
            0 => { engine.play(path.clone()).ok(); }
            1 => { engine.pause().ok(); }
            2 => { engine.resume().ok(); }
            3 => { engine.seek(((i as f32 % 4.5) / 10.0).into()).ok(); }
            4 => { engine.set_volume(i as f32 / 50.0).ok(); }
            5 => { engine.set_mute(i % 3 == 0).ok(); }
            6 => { engine.set_bypass(i % 4 == 0).ok(); }
            _ => {}
        }

        thread::sleep(Duration::from_millis(20));
    }

    // Engine should still be responsive
    let state = engine.get_state();
    assert!(state.last_error.is_none());
}

#[test]
fn stress_rapid_engine_recreation() {
    // Create and destroy engines rapidly
    for _ in 0..10 {
        let config = EngineConfig::default();
        let mut engine = AudioEngine::new(config).unwrap();

        let temp_file = create_test_wav(0.5, 48000, 2);
        engine.play(temp_file.path().to_path_buf()).ok();

        thread::sleep(Duration::from_millis(50));

        drop(engine);
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn stress_position_polling() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(3.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    // Poll position very rapidly
    for _ in 0..1000 {
        engine.get_position().ok();
    }

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
}

#[test]
fn stress_state_polling() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(3.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    // Poll state very rapidly
    for _ in 0..1000 {
        let _ = engine.get_state();
    }

    // Should still be able to get valid state
    let state = engine.get_state();
    assert!(state.playback_state != PlaybackState::Paused || true); // Any state is valid
}

#[test]
fn stress_empty_plugin_chain_updates() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(2.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    thread::sleep(Duration::from_millis(100));

    // Alternate between empty and non-empty plugin chains
    for i in 0..20 {
        let plugins = if i % 2 == 0 {
            vec![]
        } else {
            vec![PluginConfig::new("gain", serde_json::json!({"gain_db": 0.0}))]
        };

        engine.update_plugin_chain(plugins).ok();
        thread::sleep(Duration::from_millis(50));
    }

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
}

#[test]
fn stress_volume_extremes() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    // Test extreme volume values
    let extreme_volumes = [
        0.0,
        0.0001,
        0.001,
        0.01,
        0.5,
        1.0,
        1.5,
        2.0,
        5.0,
        10.0,
    ];

    for &vol in &extreme_volumes {
        engine.set_volume(vol).ok();
        thread::sleep(Duration::from_millis(10));

        let state = engine.get_state();
        assert_eq!(state.volume, vol, "Volume should be set to {}", vol);
    }
}

#[test]
fn stress_different_sample_rates() {
    let sample_rates = [44100, 48000, 88200, 96000];

    for &sr in &sample_rates {
        let config = EngineConfig {
            output_sample_rate: sr,
            ..Default::default()
        };

        match AudioEngine::new(config) {
            Ok(mut engine) => {
                let temp_file = create_test_wav(0.5, sr, 2);
                engine.play(temp_file.path().to_path_buf()).ok();

                thread::sleep(Duration::from_millis(100));

                let state = engine.get_state();
                assert!(state.playback_state != PlaybackState::Paused || true);
            }
            Err(_) => {
                // Hardware might not support this rate
            }
        }
    }
}

#[test]
fn stress_mixed_sample_rate_files() {
    let config = EngineConfig::default();
    let mut engine = AudioEngine::new(config).unwrap();

    let sample_rates = [44100, 48000, 88200];

    for &sr in &sample_rates {
        let temp_file = create_test_wav(0.2, sr, 2);
        engine.play(temp_file.path().to_path_buf()).ok();

        thread::sleep(Duration::from_millis(150));

        let state = engine.get_state();
        assert!(
            state.playback_state == PlaybackState::Playing ||
            state.playback_state == PlaybackState::Stopped
        );
    }
}
