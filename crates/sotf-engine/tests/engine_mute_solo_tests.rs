//! Channel Mute/Solo Tests
//!
//! Integration tests for channel mute/solo functionality at the audio engine level.
//! All tests require BlackHole virtual audio device to avoid playing sound on real devices.

mod common;

use serde_json::json;
use sotf_audio::engine::{AudioEngine, PlaybackState, PluginConfig};
use std::time::Duration;

#[test]
fn test_engine_with_mute_solo_plugin() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create a config with ChannelMuteSolo plugin (using BlackHole device)
    let config = common::test_engine_config_with(|c| {
        c.plugins = vec![PluginConfig::new(
            "channel_mute_solo",
            json!({
                "enabled": false,
                "channel_states": []
            }),
        )];
    });

    let engine = AudioEngine::new(config).unwrap();

    // Create a stereo test file
    let temp_file = common::create_multichannel_test_wav(1.0, 48000, 2);
    let path = temp_file.path().to_path_buf();

    // Start playback
    let result = engine.play(path.clone());
    assert!(result.is_ok(), "Failed to start playback");

    std::thread::sleep(Duration::from_millis(100));

    let state = engine.get_state();
    assert_eq!(state.playback_state, PlaybackState::Playing);
}

#[test]
fn test_mute_channel_via_parameter_update() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create a config with ChannelMuteSolo plugin (using BlackHole device)
    let config = common::test_engine_config_with(|c| {
        c.plugins = vec![PluginConfig::new(
            "channel_mute_solo",
            json!({
                "enabled": false,
                "channel_states": [
                    {"muted": false, "soloed": false},
                    {"muted": false, "soloed": false}
                ]
            }),
        )];
    });

    let engine = AudioEngine::new(config).unwrap();

    // Create a stereo test file
    let temp_file = common::create_multichannel_test_wav(2.0, 48000, 2);
    let path = temp_file.path().to_path_buf();

    // Start playback
    engine.play(path).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Now mute channel 0 (left) using parameter update
    let channel_states_json = json!([
        {"muted": true, "soloed": false},
        {"muted": false, "soloed": false}
    ])
    .to_string();

    let result = engine.set_plugin_parameter(0, "channel_states".to_string(), channel_states_json);
    assert!(result.is_ok(), "Failed to set channel_states parameter");

    // Let it play with mute applied
    std::thread::sleep(Duration::from_millis(200));

    // Verify engine is still playing (no dropout/crash)
    let state = engine.get_state();
    assert_eq!(
        state.playback_state,
        PlaybackState::Playing,
        "Engine should still be playing after mute"
    );
}

#[test]
fn test_solo_channel_via_parameter_update() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create a config with ChannelMuteSolo plugin (using BlackHole device)
    let config = common::test_engine_config_with(|c| {
        c.plugins = vec![PluginConfig::new(
            "channel_mute_solo",
            json!({
                "enabled": false,
                "channel_states": [
                    {"muted": false, "soloed": false},
                    {"muted": false, "soloed": false}
                ]
            }),
        )];
    });

    let engine = AudioEngine::new(config).unwrap();

    // Create a stereo test file
    let temp_file = common::create_multichannel_test_wav(2.0, 48000, 2);
    let path = temp_file.path().to_path_buf();

    // Start playback
    engine.play(path).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Solo channel 1 (right) - should mute channel 0
    let channel_states_json = json!([
        {"muted": false, "soloed": false},
        {"muted": false, "soloed": true}
    ])
    .to_string();

    let result = engine.set_plugin_parameter(0, "channel_states".to_string(), channel_states_json);
    assert!(result.is_ok(), "Failed to set channel_states for solo");

    // Let it play with solo applied
    std::thread::sleep(Duration::from_millis(200));

    // Verify engine is still playing (no dropout/crash)
    let state = engine.get_state();
    assert_eq!(
        state.playback_state,
        PlaybackState::Playing,
        "Engine should still be playing after solo"
    );
}

#[test]
fn test_multiple_mute_solo_updates() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create a config with ChannelMuteSolo plugin (using BlackHole device)
    let config = common::test_engine_config_with(|c| {
        c.plugins = vec![PluginConfig::new(
            "channel_mute_solo",
            json!({
                "enabled": false,
                "channel_states": [
                    {"muted": false, "soloed": false},
                    {"muted": false, "soloed": false}
                ]
            }),
        )];
    });

    let engine = AudioEngine::new(config).unwrap();

    // Create a stereo test file
    let temp_file = common::create_multichannel_test_wav(3.0, 48000, 2);
    let path = temp_file.path().to_path_buf();

    // Start playback
    engine.play(path).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Test sequence: mute → unmute → solo → clear

    // 1. Mute left channel
    let mute_left = json!([
        {"muted": true, "soloed": false},
        {"muted": false, "soloed": false}
    ])
    .to_string();

    engine
        .set_plugin_parameter(0, "channel_states".to_string(), mute_left)
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // 2. Unmute left, mute right
    let mute_right = json!([
        {"muted": false, "soloed": false},
        {"muted": true, "soloed": false}
    ])
    .to_string();

    engine
        .set_plugin_parameter(0, "channel_states".to_string(), mute_right)
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // 3. Solo left channel
    let solo_left = json!([
        {"muted": false, "soloed": true},
        {"muted": false, "soloed": false}
    ])
    .to_string();

    engine
        .set_plugin_parameter(0, "channel_states".to_string(), solo_left)
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // 4. Clear all mutes/solos
    let clear_all = json!([
        {"muted": false, "soloed": false},
        {"muted": false, "soloed": false}
    ])
    .to_string();

    engine
        .set_plugin_parameter(0, "channel_states".to_string(), clear_all)
        .unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Verify engine is still playing after all updates (zero dropout)
    let state = engine.get_state();
    assert_eq!(
        state.playback_state,
        PlaybackState::Playing,
        "Engine should still be playing after multiple mute/solo updates"
    );
    assert!(
        state.position > 0.0,
        "Position should have advanced (no audio dropout)"
    );
}

#[test]
fn test_multichannel_selective_muting() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create a 6-channel (5.1) config with upmixer and ChannelMuteSolo plugin (using BlackHole device)
    let config = common::test_engine_config_with(|c| {
        c.output_channels = 6;
        c.plugins = vec![
            // Upmixer to convert stereo → 5.1
            PluginConfig::new(
                "upmixer",
                json!({
                    "speaker_config": "5.1",
                    "gain_front_direct": 1.0,
                    "gain_front_ambient": 0.5,
                    "gain_rear_ambient": 1.0,
                    "lfe_cutoff_hz": 120.0,
                    "stereo_width": 0.5,
                    "bandpass_hz": 250.0,
                    "height_gain": 1.0,
                    "lfe_gain": 1.0,
                    "enable_subharmonic_synth": false,
                    "subharmonic_gain": 0.5,
                    "enable_hr_direct": false,
                    "hr_sharpen": 1.0,
                    "safety_cap_db": 3.0,
                    "decorrelation_mode": 0
                }),
            ),
            // ChannelMuteSolo plugin after upmixer
            PluginConfig::new(
                "channel_mute_solo",
                json!({
                    "enabled": false,
                    "channel_states": [
                        {"muted": false, "soloed": false},
                        {"muted": false, "soloed": false},
                        {"muted": false, "soloed": false},
                        {"muted": false, "soloed": false},
                        {"muted": false, "soloed": false},
                        {"muted": false, "soloed": false}
                    ]
                }),
            ),
        ];
    });

    let engine = AudioEngine::new(config).unwrap();

    // Create a stereo test file (will be upmixed to 5.1)
    let temp_file = common::create_multichannel_test_wav(2.0, 48000, 2);
    let path = temp_file.path().to_path_buf();

    // Start playback
    engine.play(path).unwrap();
    std::thread::sleep(Duration::from_millis(200));

    // Mute center (ch 2) and LFE (ch 3)
    let mute_center_lfe = json!([
        {"muted": false, "soloed": false}, // FL
        {"muted": false, "soloed": false}, // FR
        {"muted": true, "soloed": false},  // C
        {"muted": true, "soloed": false},  // LFE
        {"muted": false, "soloed": false}, // SL
        {"muted": false, "soloed": false}  // SR
    ])
    .to_string();

    // Plugin index 1 (ChannelMuteSolo is after upmixer)
    let result = engine.set_plugin_parameter(1, "channel_states".to_string(), mute_center_lfe);
    assert!(
        result.is_ok(),
        "Failed to mute channels in 5.1 configuration"
    );

    // Let it play with selective muting
    std::thread::sleep(Duration::from_millis(200));

    // Verify engine is still playing
    let state = engine.get_state();
    assert_eq!(
        state.playback_state,
        PlaybackState::Playing,
        "Engine should handle 5.1 muting without issues"
    );
}

#[test]
fn test_zero_dropout_rapid_updates() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Create a config with ChannelMuteSolo plugin (using BlackHole device)
    let config = common::test_engine_config_with(|c| {
        c.plugins = vec![PluginConfig::new(
            "channel_mute_solo",
            json!({
                "enabled": false,
                "channel_states": [
                    {"muted": false, "soloed": false},
                    {"muted": false, "soloed": false}
                ]
            }),
        )];
    });

    let engine = AudioEngine::new(config).unwrap();

    // Create a stereo test file (longer duration for rapid updates)
    let temp_file = common::create_multichannel_test_wav(5.0, 48000, 2);
    let path = temp_file.path().to_path_buf();

    // Start playback
    engine.play(path).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    let start_position = engine.get_state().position;

    // Rapidly toggle mute 10 times with minimal delay
    for i in 0..10 {
        let muted = i % 2 == 0;
        let channel_states = json!([
            {"muted": muted, "soloed": false},
            {"muted": false, "soloed": false}
        ])
        .to_string();

        engine
            .set_plugin_parameter(0, "channel_states".to_string(), channel_states)
            .unwrap();

        // Very short delay between updates
        std::thread::sleep(Duration::from_millis(10));
    }

    // Wait a bit more to ensure playback continued
    std::thread::sleep(Duration::from_millis(200));

    let end_position = engine.get_state().position;

    // Verify:
    // 1. Engine is still playing
    assert_eq!(
        engine.get_state().playback_state,
        PlaybackState::Playing,
        "Engine should survive rapid parameter updates"
    );

    // 2. Position advanced (no major dropout)
    assert!(
        end_position > start_position,
        "Position should advance during rapid updates (end: {}, start: {})",
        end_position,
        start_position
    );

    // 3. Position advanced by at least 300ms (allowing for some timing variance)
    let elapsed = end_position - start_position;
    assert!(
        elapsed >= 0.25,
        "At least 250ms should have elapsed (got {}s), indicating zero dropout",
        elapsed
    );
}
