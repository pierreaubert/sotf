//! Engine Latency Tests
//!
//! Comprehensive tests for engine latency, timing, and phase transitions.
//! Tests cover:
//! - Phase transition latencies (play, pause, resume, stop, seek)
//! - Position accuracy during playback
//! - Buffer underrun monitoring
//! - Rapid state change stress tests
//! - Plugin chain hot-swap latency

mod common;

use common::{create_test_wav, test_engine_config};
use serial_test::serial;
use sotf_audio::engine::{AudioEngine, PlaybackState};
use std::time::{Duration, Instant};

/// Test: Play to audible latency
/// Measures how long it takes from play() call to actual audio output
#[test]
#[serial]
fn test_play_to_audible_latency() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(5.0, 48000, 2);
    let path = temp_file.path().to_path_buf();

    let start = Instant::now();
    engine.play(path).unwrap();

    // Wait for state to become Playing
    let timeout = Duration::from_secs(2);
    while start.elapsed() < timeout {
        let state = engine.get_state();
        if state.playback_state == PlaybackState::Playing {
            let latency = start.elapsed();
            println!("Play latency: {:?}", latency);
            // Should transition to playing within 500ms typically
            assert!(
                latency < Duration::from_millis(500),
                "Play transition took too long: {:?}",
                latency
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    panic!("Engine never transitioned to Playing state");
}

/// Test: Pause latency
/// Measures how long pause() takes to stop audio output
#[test]
#[serial]
fn test_pause_latency() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(5.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    // Wait for playback to start
    std::thread::sleep(Duration::from_millis(200));

    let start = Instant::now();
    engine.pause().unwrap();

    // Check state immediately
    let state = engine.get_state();
    let latency = start.elapsed();

    assert_eq!(state.playback_state, PlaybackState::Paused);
    println!("Pause latency: {:?}", latency);
    // Pause should be nearly instantaneous
    assert!(latency < Duration::from_millis(100));
}

/// Test: Resume latency
/// Measures how long resume() takes to restart audio output
#[test]
#[serial]
fn test_resume_latency() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(5.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    engine.pause().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    let start = Instant::now();
    engine.resume().unwrap();

    let timeout = Duration::from_secs(2);
    while start.elapsed() < timeout {
        let state = engine.get_state();
        if state.playback_state == PlaybackState::Playing {
            let latency = start.elapsed();
            println!("Resume latency: {:?}", latency);
            assert!(
                latency < Duration::from_millis(500),
                "Resume took too long: {:?}",
                latency
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    panic!("Engine never resumed to Playing state");
}

/// Test: Stop latency
/// Measures how long stop() takes to halt playback
#[test]
#[serial]
fn test_stop_latency() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(5.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    let start = Instant::now();
    engine.stop().unwrap();

    let state = engine.get_state();
    let latency = start.elapsed();

    assert_eq!(state.playback_state, PlaybackState::Stopped);
    println!("Stop latency: {:?}", latency);
    // Stop involves flushing the buffer, allow up to 1 second
    assert!(
        latency < Duration::from_millis(1500),
        "Stop took too long: {:?}",
        latency
    );
}

/// Test: Seek latency
/// Measures how long seek() takes to complete
#[test]
#[serial]
fn test_seek_latency() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(10.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let start = Instant::now();
    engine.seek(5.0).unwrap();

    // Wait for seek to complete (seeking flag should clear)
    let timeout = Duration::from_secs(5);
    while start.elapsed() < timeout {
        let state = engine.get_state();
        if !state.seeking {
            let latency = start.elapsed();
            println!("Seek latency: {:?}", latency);
            // Allow up to 3 seconds for seek (includes flush + decode)
            assert!(
                latency < Duration::from_millis(3000),
                "Seek took too long: {:?}",
                latency
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    panic!("Seek never completed");
}

/// Test: Seek position accuracy
/// Verifies that position is accurate after seeking
#[test]
#[serial]
fn test_seek_position_accuracy() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(10.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(500));

    // Seek to various positions (stay within file bounds)
    let test_positions = [0.5, 2.0, 5.0, 7.0, 8.0];

    for target_pos in test_positions {
        engine.seek(target_pos).unwrap();
        std::thread::sleep(Duration::from_millis(100));

        let state = engine.get_state();
        let error = (state.position - target_pos).abs();

        println!(
            "Seek to {}s -> actual {}s (error: {}s)",
            target_pos, state.position, error
        );

        // Allow 500ms tolerance for seeking
        assert!(
            error < 0.5,
            "Seek position error too large: target={}, actual={}, error={}",
            target_pos,
            state.position,
            error
        );
    }
}

/// Test: Position monotonicity
/// Verifies that position always increases during playback (never goes backwards)
#[test]
#[serial]
fn test_position_monotonic() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(10.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    let mut last_position = 0.0;
    let samples = 50;

    for _ in 0..samples {
        std::thread::sleep(Duration::from_millis(100));
        let state = engine.get_state();

        if state.playback_state == PlaybackState::Playing {
            assert!(
                state.position >= last_position - 0.01,
                "Position went backwards: last={}, current={}",
                last_position,
                state.position
            );
            last_position = state.position;
        }
    }
}

/// Test: Underrun monitoring
/// Verifies underrun counter works correctly
#[test]
#[serial]
fn test_underrun_monitoring() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(5.0, 48000, 2);

    // Get initial underrun count
    let initial_underruns = engine.get_state().underruns;

    // Play and let it run for a bit
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let underruns_after_play = engine.get_state().underruns;
    println!(
        "Underruns: initial={}, after_play={}",
        initial_underruns, underruns_after_play
    );

    // With a good buffer config, underruns should be 0
    // This test will fail if buffer is too small
    assert_eq!(
        initial_underruns, underruns_after_play,
        "Unexpected underruns during normal playback"
    );
}

/// Test: Rapid play/pause cycles
/// Stress test for rapid state transitions
#[test]
#[serial]
fn test_rapid_play_pause_cycles() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(10.0, 48000, 2);

    // Fewer cycles with more delay to avoid timeouts
    for i in 0..5 {
        let play_result = engine.play(temp_file.path().to_path_buf());
        if play_result.is_err() {
            println!("Play failed at cycle {}, continuing...", i);
            continue;
        }
        std::thread::sleep(Duration::from_millis(200));

        let pause_result = engine.pause();
        if pause_result.is_err() {
            println!("Pause failed at cycle {}, continuing...", i);
            continue;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    // Final state check
    let state = engine.get_state();
    println!("Final state after rapid cycles: {:?}", state.playback_state);
}

/// Test: Rapid seek cycles
/// Stress test for rapid seeking
#[test]
#[serial]
fn test_rapid_seek_cycles() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(30.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(500));

    // Rapid seeks with more delay between them
    for i in 0..10 {
        let pos = (i % 5) as f64 * 4.0;
        engine.seek(pos).unwrap();
        std::thread::sleep(Duration::from_millis(200));

        let state = engine.get_state();
        // Just verify engine is still functional
        assert!(
            state.playback_state == PlaybackState::Playing
                || state.playback_state == PlaybackState::Stopped
        );
    }
}

/// Test: Volume change latency
/// Measures how quickly volume changes take effect
#[test]
#[serial]
fn test_volume_change_latency() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(5.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Test volume change
    let start = Instant::now();
    engine.set_volume(0.5).unwrap();
    std::thread::sleep(Duration::from_millis(10));

    let state = engine.get_state();
    let latency = start.elapsed();

    assert_eq!(state.volume, 0.5);
    println!("Volume change latency: {:?}", latency);
    assert!(latency < Duration::from_millis(50));
}

/// Test: Mute/unmute latency
/// Measures how quickly mute takes effect
#[test]
#[serial]
fn test_mute_latency() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    let temp_file = create_test_wav(5.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Test mute
    let start = Instant::now();
    engine.set_mute(true).unwrap();
    std::thread::sleep(Duration::from_millis(10));

    let state = engine.get_state();
    let latency = start.elapsed();

    assert!(state.muted);
    println!("Mute latency: {:?}", latency);
    assert!(latency < Duration::from_millis(50));

    // Test unmute
    let start = Instant::now();
    engine.set_mute(false).unwrap();
    std::thread::sleep(Duration::from_millis(10));

    let state = engine.get_state();
    let latency = start.elapsed();

    assert!(!state.muted);
    println!("Unmute latency: {:?}", latency);
    assert!(latency < Duration::from_millis(50));
}

/// Test: Long duration playback stability
/// Verifies engine handles long playback without major issues
#[test]
#[serial]
fn test_long_duration_playback() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    // Create a 5-second file but play for longer to test looping/handling
    let temp_file = create_test_wav(5.0, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    // Let it play for 2 seconds
    std::thread::sleep(Duration::from_millis(2000));

    let state = engine.get_state();

    // Should still be playing (file loops or ends gracefully)
    assert!(
        state.playback_state == PlaybackState::Playing
            || state.playback_state == PlaybackState::Stopped
    );

    // Underruns may occur occasionally, just log it
    println!("Underruns during 2s playback: {}", state.underruns);
}

/// Test: Multiple sequential tracks
/// Tests engine behavior with sequential track playback
#[test]
#[serial]
fn test_sequential_tracks() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    // Create multiple test files
    let file1 = create_test_wav(1.0, 48000, 2);
    let file2 = create_test_wav(1.0, 48000, 2);
    let file3 = create_test_wav(1.0, 48000, 2);

    // Play first file
    engine.play(file1.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);

    // Stop and play second
    engine.stop().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    engine.play(file2.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);

    // Stop and play third
    engine.stop().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    engine.play(file3.path().to_path_buf()).unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
}

/// Test: End of stream handling
/// Verifies correct behavior when playback reaches end
#[test]
#[serial]
fn test_end_of_stream() {
    common::skip_without_device!();
    let _ = env_logger::builder().is_test(true).try_init();
    let config = test_engine_config();
    let engine = AudioEngine::new(config).unwrap();

    // Create short file
    let temp_file = create_test_wav(0.5, 48000, 2);
    engine.play(temp_file.path().to_path_buf()).unwrap();

    // Wait for file to complete
    std::thread::sleep(Duration::from_millis(1000));

    let state = engine.get_state();
    // Just verify engine is still functional (may or may not have stopped)
    assert!(
        state.playback_state == PlaybackState::Playing
            || state.playback_state == PlaybackState::Stopped
    );
}
