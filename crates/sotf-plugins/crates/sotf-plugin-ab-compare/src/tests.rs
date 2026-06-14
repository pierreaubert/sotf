use super::*;
use sotf_host::host::DawHost;
use sotf_host::plugin::{PluginInfo, ProcessContext};

mod latency_passthrough;
mod misc;

#[test]
fn test_ab_compare_creation() {
    let plugin = ABComparePlugin::new(2).unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn test_bypass_mode() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(48000).unwrap();
    plugin
        .set_parameter(
            ParameterId("bypass".to_string()),
            ParameterValue::Bool(true),
        )
        .unwrap();

    let input = vec![1.0, 0.5, 0.8, 0.3]; // 2 frames, 2 channels
    let mut output = vec![0.0; 4];
    let context = ProcessContext::new(48000, 2);

    plugin.process(&input, &mut output, &context).unwrap();

    assert_eq!(input, output, "Bypass should pass through unchanged");
}

#[test]
fn test_path_config_serialization() {
    // Test None
    let none_config = PathConfig::None;
    let json = serde_json::to_string(&none_config).unwrap();
    assert!(json.contains("None"));

    // Test Plugin
    let plugin_config = PathConfig::Plugin {
        plugin_type: "EQ".to_string(),
        parameters: serde_json::json!({"filters": []}),
    };
    let json = serde_json::to_string(&plugin_config).unwrap();
    assert!(json.contains("Plugin"));
    assert!(json.contains("EQ"));

    // Test Rack
    let rack_config = PathConfig::Rack {
        plugins: vec![PluginInRack {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": -6.0}),
        }],
    };
    let json = serde_json::to_string(&rack_config).unwrap();
    assert!(json.contains("Rack"));

    // Test deserialization
    let deserialized: PathConfig = serde_json::from_str(&json).unwrap();
    match deserialized {
        PathConfig::Rack { plugins } => {
            assert_eq!(plugins.len(), 1);
            assert_eq!(plugins[0].plugin_type, "gain");
        }
        _ => panic!("Expected Rack"),
    }
}

#[test]
fn test_mix_pure_a() {
    let params = ABComparePluginParams {
        path_a: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": -6.0}),
        },
        path_b: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": 6.0}),
        },
        mix: -1.0, // Pure A
        auto_gain_enabled: false,
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    // Process multiple times to let smoothers settle
    let input = vec![1.0; 4800 * 2]; // 100ms at 48kHz
    let mut output = vec![0.0; 4800 * 2];
    let context = ProcessContext::new(48000, 4800);

    for _ in 0..5 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    // At mix=-1 (pure A) with -6dB gain, output should be ~0.5
    // Check the last samples (smoothers should have settled)
    let last_sample = output[output.len() - 1];
    assert!(
        (last_sample - 0.5).abs() < 0.1,
        "Expected ~0.5, got {}",
        last_sample
    );
}

#[test]
fn test_mix_pure_b() {
    let params = ABComparePluginParams {
        path_a: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": 6.0}),
        },
        path_b: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": -6.0}),
        },
        mix: 1.0, // Pure B
        auto_gain_enabled: false,
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    // Process multiple times to let smoothers settle
    let input = vec![1.0; 4800 * 2];
    let mut output = vec![0.0; 4800 * 2];
    let context = ProcessContext::new(48000, 4800);

    for _ in 0..5 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    // At mix=+1 (pure B) with -6dB gain, output should be ~0.5
    let last_sample = output[output.len() - 1];
    assert!(
        (last_sample - 0.5).abs() < 0.1,
        "Expected ~0.5, got {}",
        last_sample
    );
}

#[test]
fn test_binary_mode() {
    let params = ABComparePluginParams {
        mix_mode: MixMode::Binary,
        selected_path: 0, // A
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    // Switch to B
    plugin
        .set_parameter(
            ParameterId("selected_path".to_string()),
            ParameterValue::Int(1),
        )
        .unwrap();

    let value = plugin.get_parameter(&ParameterId("selected_path".to_string()));
    assert_eq!(value, Some(ParameterValue::Int(1)));
}

#[test]
fn test_multichannel_support() {
    // Test with 5 channels
    let mut plugin = ABComparePlugin::new(5).unwrap();
    plugin.initialize(48000).unwrap();

    let input = vec![0.5; 5 * 1024]; // 1024 frames, 5 channels
    let mut output = vec![0.0; 5 * 1024];
    let context = ProcessContext::new(48000, 1024);

    plugin.process(&input, &mut output, &context).unwrap();

    // Pass-through with no plugins should work
    // Note: smoothers may affect output slightly
}

#[test]
fn test_empty_path_fast_path_matches_equal_power_mix() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(48000).unwrap();

    let input = vec![0.25; 512 * 2];
    let mut output = vec![0.0; 512 * 2];
    let context = ProcessContext::new(48000, 512);

    plugin.process(&input, &mut output, &context).unwrap();

    let expected = 0.25 * std::f32::consts::SQRT_2;
    for &sample in &output {
        assert!(
            (sample - expected).abs() < 1e-6,
            "default empty A/B path should use equal-power 50/50 mix"
        );
    }
}

#[test]
fn test_empty_path_fast_gain_is_reused_after_mix_changes() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(48000).unwrap();

    let input = vec![0.5f32; 512 * 2];
    let mut output = vec![0.0f32; 512 * 2];
    let context = ProcessContext::new(48000, 512);

    plugin
        .set_parameter(
            ParameterId("auto_gain_enabled".to_string()),
            ParameterValue::Bool(false),
        )
        .unwrap();

    plugin.process(&input, &mut output, &context).unwrap();
    let expected =
        0.5f32 * std::f32::consts::FRAC_PI_4.cos() + 0.5f32 * std::f32::consts::FRAC_PI_4.sin();
    for &sample in &output {
        assert!((sample - expected).abs() < 1e-6);
    }

    plugin
        .set_parameter(ParameterId("mix".to_string()), ParameterValue::Float(1.0))
        .unwrap();
    plugin.mix_smoother.reset(1.0);
    plugin.process(&input, &mut output, &context).unwrap();
    for &sample in &output {
        assert!(
            (sample - 0.5).abs() < 1e-5,
            "pure B should retain unity gain"
        );
    }

    assert!(
        (plugin.empty_path_fast_gain - 1.0).abs() < 1e-6,
        "cached gain should switch to the pure B value"
    );

    plugin
        .set_parameter(ParameterId("mix".to_string()), ParameterValue::Float(-1.0))
        .unwrap();
    plugin.mix_smoother.reset(-1.0);
    plugin.process(&input, &mut output, &context).unwrap();
    for &sample in &output {
        assert!(
            (sample - 0.5).abs() < 1e-5,
            "pure A should retain unity gain"
        );
    }

    assert!(
        (plugin.empty_path_fast_gain - 1.0).abs() < 1e-6,
        "pure-path mix values should keep cached gain at unity"
    );
}

#[test]
fn test_reset() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(48000).unwrap();

    // Process some audio
    let input = vec![1.0; 1000 * 2];
    let mut output = vec![0.0; 1000 * 2];
    let context = ProcessContext::new(48000, 1000);
    plugin.process(&input, &mut output, &context).unwrap();

    // Reset should not panic
    plugin.reset();

    // Loudness should be reset
    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();
    assert!(
        ab_data.loudness_a_lufs.is_infinite() || ab_data.loudness_a_lufs < -60.0,
        "Loudness should be reset"
    );
}

#[test]
fn test_rack_configuration() {
    let params = ABComparePluginParams {
        path_a: PathConfig::Rack {
            plugins: vec![
                PluginInRack {
                    plugin_type: "gain".to_string(),
                    parameters: serde_json::json!({"gain_db": -3.0}),
                },
                PluginInRack {
                    plugin_type: "gain".to_string(),
                    parameters: serde_json::json!({"gain_db": -3.0}),
                },
            ],
        },
        mix: -1.0,
        auto_gain_enabled: false,
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    // Two -3dB gains = -6dB total
    let input = vec![1.0; 4800 * 2];
    let mut output = vec![0.0; 4800 * 2];
    let context = ProcessContext::new(48000, 4800);

    for _ in 0..5 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let last_sample = output[output.len() - 1];
    assert!(
        (last_sample - 0.5).abs() < 0.1,
        "Two -3dB gains should give ~0.5, got {}",
        last_sample
    );
}

#[test]
fn test_get_data() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(48000).unwrap();

    // Process some audio
    let input = vec![0.5; 4800 * 2];
    let mut output = vec![0.0; 4800 * 2];
    let context = ProcessContext::new(48000, 4800);
    plugin.process(&input, &mut output, &context).unwrap();

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

    // Verify data structure is populated
    assert!(!ab_data.bypass_active);
}

#[test]
fn test_runtime_path_change() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(48000).unwrap();

    // Change path A at runtime
    let new_config =
        r#"{"type": "Plugin", "plugin_type": "gain", "parameters": {"gain_db": -12.0}}"#;
    plugin
        .set_parameter(
            ParameterId("path_a_config".to_string()),
            ParameterValue::String(new_config.to_string()),
        )
        .unwrap();

    // Verify it works
    let input = vec![1.0; 1024 * 2];
    let mut output = vec![0.0; 1024 * 2];
    let context = ProcessContext::new(48000, 1024);

    plugin.process(&input, &mut output, &context).unwrap();
}

#[test]
fn test_auto_gain_enabled_by_default() {
    let params = ABComparePluginParams::default();
    assert!(
        params.auto_gain_enabled,
        "Auto-gain should be enabled by default"
    );
}

#[test]
fn test_auto_gain_from_params_enabled() {
    let params = ABComparePluginParams {
        auto_gain_enabled: true,
        loudness_type: LoudnessType::ShortTerm,
        max_auto_gain_db: 18.0,
        gain_smoothing_ms: 200.0,
        ..Default::default()
    };

    let plugin = ABComparePlugin::from_params(2, params).unwrap();

    // Verify auto-gain is enabled via parameter
    let value = plugin.get_parameter(&ParameterId("auto_gain_enabled".to_string()));
    assert_eq!(value, Some(ParameterValue::Bool(true)));
}

#[test]
fn test_auto_gain_from_params_disabled() {
    let params = ABComparePluginParams {
        auto_gain_enabled: false,
        ..Default::default()
    };

    let plugin = ABComparePlugin::from_params(2, params).unwrap();

    let value = plugin.get_parameter(&ParameterId("auto_gain_enabled".to_string()));
    assert_eq!(value, Some(ParameterValue::Bool(false)));
}

#[test]
fn test_auto_gain_parameter_set_get() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(48000).unwrap();

    // Test auto_gain_enabled
    plugin
        .set_parameter(
            ParameterId("auto_gain_enabled".to_string()),
            ParameterValue::Bool(false),
        )
        .unwrap();
    let value = plugin.get_parameter(&ParameterId("auto_gain_enabled".to_string()));
    assert_eq!(value, Some(ParameterValue::Bool(false)));

    plugin
        .set_parameter(
            ParameterId("auto_gain_enabled".to_string()),
            ParameterValue::Bool(true),
        )
        .unwrap();
    let value = plugin.get_parameter(&ParameterId("auto_gain_enabled".to_string()));
    assert_eq!(value, Some(ParameterValue::Bool(true)));
}

#[test]
fn test_auto_gain_parameter_loudness_type() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(48000).unwrap();

    // Set to ShortTerm (1)
    plugin
        .set_parameter(
            ParameterId("loudness_type".to_string()),
            ParameterValue::Int(1),
        )
        .unwrap();

    // Set back to Momentary (0)
    plugin
        .set_parameter(
            ParameterId("loudness_type".to_string()),
            ParameterValue::Int(0),
        )
        .unwrap();

    // Should not panic
}

#[test]
fn test_auto_gain_parameter_max_db() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(48000).unwrap();

    // Set max auto-gain
    plugin
        .set_parameter(
            ParameterId("max_auto_gain_db".to_string()),
            ParameterValue::Float(20.0),
        )
        .unwrap();

    // Value should be clamped to valid range
    // (max is 24.0 according to parameters())
}

#[test]
fn test_auto_gain_parameter_smoothing() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(48000).unwrap();

    // Set gain smoothing
    plugin
        .set_parameter(
            ParameterId("gain_smoothing_ms".to_string()),
            ParameterValue::Float(250.0),
        )
        .unwrap();

    // Should not panic
}

#[test]
fn test_auto_gain_params_serialization() {
    let params = ABComparePluginParams {
        auto_gain_enabled: true,
        loudness_type: LoudnessType::ShortTerm,
        max_auto_gain_db: 15.0,
        gain_smoothing_ms: 150.0,
        ..Default::default()
    };

    // Serialize
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("auto_gain_enabled"));
    assert!(json.contains("loudness_type"));
    assert!(json.contains("max_auto_gain_db"));
    assert!(json.contains("gain_smoothing_ms"));

    // Deserialize
    let deserialized: ABComparePluginParams = serde_json::from_str(&json).unwrap();
    assert!(deserialized.auto_gain_enabled);
    assert_eq!(deserialized.loudness_type, LoudnessType::ShortTerm);
    assert!((deserialized.max_auto_gain_db - 15.0).abs() < 0.01);
    assert!((deserialized.gain_smoothing_ms - 150.0).abs() < 0.01);
}

#[test]
fn test_latency_compensation_no_latency() {
    let channels = 2;

    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(48000).unwrap();

    // Default paths have 0 latency
    assert_eq!(plugin.delay_a.len, 0);
    assert_eq!(plugin.delay_b.len, 0);
    assert_eq!(plugin.latency_samples(), 0);
}

#[test]
fn test_band_mask_reduces_out_of_band_energy() {
    // Band mask 500-2000 Hz: output should have reduced energy below 500Hz
    // compared to the unmasked full-spectrum case.
    let channels = 1;

    // Unmasked reference
    let params_full = ABComparePluginParams {
        path_a: PathConfig::None,
        path_b: PathConfig::None,
        auto_gain_enabled: false,
        band_mask_low_hz: 20.0,
        band_mask_high_hz: 20000.0,
        mix_transition_ms: 5.0,
        ..Default::default()
    };
    let mut plugin_full = ABComparePlugin::from_params(channels, params_full).unwrap();
    plugin_full.initialize(48000).unwrap();

    // Masked version
    let params_masked = ABComparePluginParams {
        path_a: PathConfig::None,
        path_b: PathConfig::None,
        auto_gain_enabled: false,
        band_mask_low_hz: 500.0,
        band_mask_high_hz: 2000.0,
        mix_transition_ms: 5.0,
        ..Default::default()
    };
    let mut plugin_masked = ABComparePlugin::from_params(channels, params_masked).unwrap();
    plugin_masked.initialize(48000).unwrap();

    // Generate broadband signal (white-ish noise via simple LCG)
    let num_frames = 48000; // 1 second
    let mut input = vec![0.0f32; num_frames * channels];
    let mut seed: u32 = 12345;
    for s in input.iter_mut() {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        *s = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
    }
    // Scale down to avoid auto-gain artifacts
    for s in input.iter_mut() {
        *s *= 0.3;
    }

    let mut output_full = vec![0.0; num_frames * channels];
    let mut output_masked = vec![0.0; num_frames * channels];
    let context = ProcessContext::new(48000, num_frames);

    // Process several blocks
    for _ in 0..3 {
        plugin_full
            .process(&input, &mut output_full, &context)
            .unwrap();
        plugin_masked
            .process(&input, &mut output_masked, &context)
            .unwrap();
    }

    // The masked output should have less total energy than the full output
    // because the band mask removes frequencies outside 500-2000 Hz
    let energy_full: f32 = output_full.iter().map(|s| s * s).sum();
    let energy_masked: f32 = output_masked.iter().map(|s| s * s).sum();

    assert!(
        energy_masked < energy_full * 0.8,
        "Band mask should reduce energy: full={}, masked={}",
        energy_full,
        energy_masked
    );
}

/// Regression test for the hard 4096-frame buffer cap.
///
/// Before the fix, `process()` returned an error for blocks > 4096 frames.
/// After the fix, the buffers grow dynamically to accommodate the request.
#[test]
fn test_large_block_beyond_4096_succeeds() {
    let channels = 2;
    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(48000).unwrap();

    // Use a block size of 8192 frames (2× the old hard cap)
    let num_frames = 8192;
    let input = vec![0.3f32; num_frames * channels];
    let mut output = vec![0.0f32; num_frames * channels];
    let context = ProcessContext::new(48000, num_frames);

    // Before the fix this would return Err("Internal buffers too small…").
    // After the fix it must succeed.
    plugin
        .process(&input, &mut output, &context)
        .expect("process should succeed for blocks larger than 4096 frames");
}

/// Verifies that band_mask_active() returns false at the full-spectrum edges.
/// Values exactly at the parameter min/max must NOT trigger the filter.
#[test]
fn test_band_mask_active_at_full_spectrum_edges() {
    let params = ABComparePluginParams {
        band_mask_low_hz: 20.0,     // parameter minimum
        band_mask_high_hz: 20000.0, // parameter maximum
        ..Default::default()
    };
    let plugin = ABComparePlugin::from_params(2, params).unwrap();
    assert!(
        !plugin.band_mask_active(),
        "band_mask_active should be false when both edges are at full-spectrum limits"
    );
}

/// Verifies that a very small deviation inside the parameter range activates the mask.
#[test]
fn test_band_mask_active_triggers_at_narrowed_range() {
    // High-pass raised to 100 Hz — mask should activate
    let params_hp = ABComparePluginParams {
        band_mask_low_hz: 100.0,
        band_mask_high_hz: 20000.0,
        ..Default::default()
    };
    let plugin_hp = ABComparePlugin::from_params(2, params_hp).unwrap();
    assert!(
        plugin_hp.band_mask_active(),
        "band_mask_active should be true when low cutoff is above minimum"
    );

    // Low-pass lowered to 10000 Hz — mask should activate
    let params_lp = ABComparePluginParams {
        band_mask_low_hz: 20.0,
        band_mask_high_hz: 10000.0,
        ..Default::default()
    };
    let plugin_lp = ABComparePlugin::from_params(2, params_lp).unwrap();
    assert!(
        plugin_lp.band_mask_active(),
        "band_mask_active should be true when high cutoff is below maximum"
    );
}

