use super::super::*;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};

fn generate_sine_input(num_frames: usize, num_channels: usize) -> Vec<f32> {
    let mut input = vec![0.0_f32; num_frames * num_channels];
    for i in 0..num_frames {
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
        let sample = phase.sin() * 0.5;
        for ch in 0..num_channels {
            input[i * num_channels + ch] = sample;
        }
    }
    input
}

#[test]
fn construction_rejects_invalid_numeric_state() {
    let invalid = [f32::NAN, f32::INFINITY];
    for value in invalid {
        assert!(
            ABComparePlugin::from_params(
                2,
                ABComparePluginParams {
                    mix: value,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
    assert!(ABComparePlugin::from_params(0, ABComparePluginParams::default()).is_err());
    assert!(
        ABComparePlugin::from_params(
            2,
            ABComparePluginParams {
                selected_path: 2,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        ABComparePlugin::from_params(
            2,
            ABComparePluginParams {
                max_auto_gain_db: -1.0,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        ABComparePlugin::from_params(
            2,
            ABComparePluginParams {
                band_mask_low_hz: 10_000.0,
                band_mask_high_hz: 1_000.0,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn realtime_process_rejects_blocks_larger_than_prepared_capacity() {
    let params = ABComparePluginParams {
        path_b: PathConfig::Plugin {
            plugin_type: "gain".into(),
            parameters: serde_json::json!({"gain_db": 0.0}),
        },
        ..Default::default()
    };
    let mut plugin = ABComparePlugin::from_params(1, params).unwrap();
    plugin.initialize(48_000).unwrap();
    let input = vec![0.0; 48_001];
    let mut output = vec![0.0; 48_001];
    assert!(
        plugin
            .process(&input, &mut output, &ProcessContext::new(48_000, 48_001))
            .is_err()
    );
}

#[test]
fn unity_nested_path_uses_same_active_loudness_timeline_as_empty_paths() {
    let mut fast = ABComparePlugin::new(2).unwrap();
    fast.initialize(48_000).unwrap();
    let mut nested = ABComparePlugin::from_params(
        2,
        ABComparePluginParams {
            path_b: PathConfig::Plugin {
                plugin_type: "gain".into(),
                parameters: serde_json::json!({"gain_db": 0.0}),
            },
            ..Default::default()
        },
    )
    .unwrap();
    nested.initialize(48_000).unwrap();

    let input = generate_sine_input(128, 2);
    let mut fast_output = vec![0.0; input.len()];
    let mut nested_output = vec![0.0; input.len()];
    let context = ProcessContext::new(48_000, 128);
    for _ in 0..40 {
        fast.process(&input, &mut fast_output, &context).unwrap();
        nested
            .process(&input, &mut nested_output, &context)
            .unwrap();
    }
    let fast_data = fast.get_data().unwrap();
    let nested_data = nested.get_data().unwrap();
    let fast_data = fast_data.downcast_ref::<ABCompareData>().unwrap();
    let nested_data = nested_data.downcast_ref::<ABCompareData>().unwrap();
    assert!(
        (fast_data.loudness_a_lufs - nested_data.loudness_a_lufs).abs() < 0.01,
        "unity nested path changed active-slice loudness: {} vs {}",
        fast_data.loudness_a_lufs,
        nested_data.loudness_a_lufs
    );
}

#[test]
fn test_auto_gain_attenuates_louder_b() {
    // Path A: no processing (unity gain)
    // Path B: +6dB boost
    // Auto-gain should attenuate B to match A's loudness
    let params = ABComparePluginParams {
        path_a: PathConfig::None, // Unity gain
        path_b: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": 6.0}),
        },
        mix: 1.0, // Pure B
        auto_gain_enabled: true,
        gain_smoothing_ms: 10.0, // Fast smoothing for test
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 4800; // 100ms at 48kHz
    let input = generate_sine_input(num_frames, 2);
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    // Process multiple times to let loudness monitors and smoothers settle
    for _ in 0..10 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

    // Auto-gain should be negative (attenuating B which is louder)
    assert!(
        ab_data.auto_gain_db < 0.0,
        "Auto-gain should be negative to attenuate louder B, got {} dB",
        ab_data.auto_gain_db
    );
}

#[test]
fn test_auto_gain_boosts_quieter_b() {
    // Path A: no processing (unity gain)
    // Path B: -6dB cut
    // Auto-gain should boost B to match A's loudness
    let params = ABComparePluginParams {
        path_a: PathConfig::None, // Unity gain
        path_b: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": -6.0}),
        },
        mix: 1.0, // Pure B
        auto_gain_enabled: true,
        gain_smoothing_ms: 10.0, // Fast smoothing for test
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let input = generate_sine_input(num_frames, 2);
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    // Process multiple times
    for _ in 0..10 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

    // Auto-gain should be positive (boosting B which is quieter)
    assert!(
        ab_data.auto_gain_db > 0.0,
        "Auto-gain should be positive to boost quieter B, got {} dB",
        ab_data.auto_gain_db
    );
}

#[test]
fn test_auto_gain_disabled_no_compensation() {
    // Path A: no processing
    // Path B: +6dB boost
    // With auto-gain disabled, B should be louder
    let params = ABComparePluginParams {
        path_a: PathConfig::None,
        path_b: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": 6.0}),
        },
        mix: 1.0,
        auto_gain_enabled: false, // Disabled
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let input = generate_sine_input(num_frames, 2);
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    // Process multiple times
    for _ in 0..5 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

    // Auto-gain should be 0 when disabled
    assert!(
        ab_data.auto_gain_db.abs() < 0.01,
        "Auto-gain should be ~0 when disabled, got {} dB",
        ab_data.auto_gain_db
    );
}

#[test]
fn test_auto_gain_max_clamp() {
    // Path A: very quiet
    // Path B: very loud
    // Auto-gain should be clamped to max_auto_gain_db
    let params = ABComparePluginParams {
        path_a: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": -40.0}), // Very quiet A
        },
        path_b: PathConfig::None, // Unity gain B (much louder than A)
        mix: 1.0,
        auto_gain_enabled: true,
        max_auto_gain_db: 6.0, // Clamp to 6dB
        gain_smoothing_ms: 1.0,
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 9600; // 200ms
    let input = generate_sine_input(num_frames, 2);
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    // Process multiple times
    for _ in 0..10 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

    // Auto-gain should be clamped
    assert!(
        ab_data.auto_gain_db >= -6.5 && ab_data.auto_gain_db <= 6.5,
        "Auto-gain should be clamped to +/-6dB, got {} dB",
        ab_data.auto_gain_db
    );
}

#[test]
fn test_auto_gain_reset_clears_gain() {
    let params = ABComparePluginParams {
        path_a: PathConfig::None,
        path_b: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": 6.0}),
        },
        mix: 1.0,
        auto_gain_enabled: true,
        gain_smoothing_ms: 10.0,
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let input = generate_sine_input(num_frames, 2);
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    // Process to build up auto-gain
    for _ in 0..10 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    // Reset
    plugin.reset();

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

    // After reset, auto-gain should be 0
    assert!(
        ab_data.auto_gain_db.abs() < 0.01,
        "Auto-gain should be ~0 after reset, got {} dB",
        ab_data.auto_gain_db
    );

    // Loudness should be reset (infinite or very negative)
    assert!(
        ab_data.loudness_a_lufs.is_infinite() || ab_data.loudness_a_lufs < -60.0,
        "Loudness A should be reset"
    );
    assert!(
        ab_data.loudness_b_lufs.is_infinite() || ab_data.loudness_b_lufs < -60.0,
        "Loudness B should be reset"
    );
}

#[test]
fn test_auto_gain_get_data_includes_loudness() {
    let params = ABComparePluginParams {
        auto_gain_enabled: true,
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let input = generate_sine_input(num_frames, 2);
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    // Process to get loudness measurements
    for _ in 0..5 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

    // Loudness values should be finite (not -inf)
    assert!(
        ab_data.loudness_a_lufs.is_finite(),
        "Loudness A should be finite after processing"
    );
    assert!(
        ab_data.loudness_b_lufs.is_finite(),
        "Loudness B should be finite after processing"
    );

    // Peak values should be positive
    assert!(ab_data.peak_a > 0.0, "Peak A should be positive");
    assert!(ab_data.peak_b > 0.0, "Peak B should be positive");
}

#[test]
fn test_auto_gain_runtime_enable_disable() {
    let params = ABComparePluginParams {
        path_a: PathConfig::None,
        path_b: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": 6.0}),
        },
        mix: 1.0,
        auto_gain_enabled: false, // Start disabled
        gain_smoothing_ms: 10.0,
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let input = generate_sine_input(num_frames, 2);
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    // Process while disabled
    for _ in 0..5 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    // Scope each cache read: RealTimeCache uses a 2-slot double-buffer and skips
    // updates when the spare Arc is still held by a reader. `let` shadowing does
    // not drop the old binding at the shadow point — it lives until the end of the
    // function — so we must explicitly scope reads to release the Arc before
    // subsequent audio-thread writes. Real UI code drops per poll; this test must
    // match that pattern or it pins the cache on stale data.
    {
        let data = plugin.get_data().unwrap();
        let ab_data = data.downcast_ref::<ABCompareData>().unwrap();
        assert!(
            ab_data.auto_gain_db.abs() < 0.01,
            "Auto-gain should be ~0 when disabled"
        );
    }

    // Enable auto-gain at runtime
    plugin
        .set_parameter(
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(true),
        )
        .unwrap();

    // Process more to build up auto-gain
    for _ in 0..10 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    {
        let data = plugin.get_data().unwrap();
        let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

        // Now auto-gain should be active (negative since B is louder)
        assert!(
            ab_data.auto_gain_db < -1.0,
            "Auto-gain should be negative after enabling, got {} dB",
            ab_data.auto_gain_db
        );
    }

    // Disable again
    plugin
        .set_parameter(
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();

    // Process to let gain fade back to 0
    for _ in 0..20 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();
    assert!(
        ab_data.auto_gain_db.abs() < 0.5,
        "Auto-gain should return to ~0 when disabled, got {} dB",
        ab_data.auto_gain_db
    );
}

#[test]
fn test_auto_gain_equal_paths_no_compensation() {
    // Both paths have the same gain - auto-gain should be ~0
    let params = ABComparePluginParams {
        path_a: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": -3.0}),
        },
        path_b: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": -3.0}),
        },
        mix: 1.0,
        auto_gain_enabled: true,
        gain_smoothing_ms: 10.0,
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let input = generate_sine_input(num_frames, 2);
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    // Process multiple times
    for _ in 0..10 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

    // Auto-gain should be ~0 when paths are equal
    assert!(
        ab_data.auto_gain_db.abs() < 1.0,
        "Auto-gain should be ~0 when paths are equal, got {} dB",
        ab_data.auto_gain_db
    );
}

#[test]
fn test_auto_gain_multichannel() {
    // Test with 5 channels
    let params = ABComparePluginParams {
        path_a: PathConfig::None,
        path_b: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": 3.0}),
        },
        mix: 1.0,
        auto_gain_enabled: true,
        gain_smoothing_ms: 10.0,
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(5, params).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let input = generate_sine_input(num_frames, 5);
    let mut output = vec![0.0; num_frames * 5];
    let context = ProcessContext::new(48000, num_frames);

    // Should not panic with multichannel
    for _ in 0..10 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

    // Auto-gain should be working (negative since B is louder)
    assert!(
        ab_data.auto_gain_db < 0.0,
        "Auto-gain should work with multichannel, got {} dB",
        ab_data.auto_gain_db
    );
}

#[test]
fn test_difference_mode_identical_paths_silence() {
    // Both paths are identical (no processing) -> A - B should be ~silence
    let params = ABComparePluginParams {
        path_a: PathConfig::None,
        path_b: PathConfig::None,
        difference_mode: true,
        auto_gain_enabled: false,
        mix_transition_ms: 5.0,
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let input = generate_sine_input(num_frames, 2);
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    // Process several blocks for smoothers to settle
    for _ in 0..5 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    // A - B with identical paths should produce near-silence
    let rms: f32 = output.iter().map(|s| s * s).sum::<f32>() / output.len() as f32;
    assert!(
        rms < 1e-6,
        "Difference of identical paths should be ~silence, RMS={}",
        rms
    );
}

#[test]
fn test_difference_mode_a_sine_b_silence() {
    // Path A = pass-through (sine), Path B = muted (gain -100dB)
    // Difference mode: output = A - B ≈ A
    let params = ABComparePluginParams {
        path_a: PathConfig::None, // pass-through
        path_b: PathConfig::Plugin {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({"gain_db": -60.0}),
        },
        difference_mode: true,
        auto_gain_enabled: false,
        mix_transition_ms: 5.0,
        ..Default::default()
    };

    let mut plugin = ABComparePlugin::from_params(2, params).unwrap();
    plugin.initialize(48000).unwrap();

    let num_frames = 4800;
    let input = generate_sine_input(num_frames, 2);
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext::new(48000, num_frames);

    // Process several blocks for smoothers to settle
    for _ in 0..5 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    // Output ≈ A (since B is ~0). Compare last block's RMS to input RMS.
    let input_rms: f32 = input.iter().map(|s| s * s).sum::<f32>() / input.len() as f32;
    let output_rms: f32 = output.iter().map(|s| s * s).sum::<f32>() / output.len() as f32;
    let ratio = output_rms / input_rms;
    assert!(
        (ratio - 1.0).abs() < 0.1,
        "Difference output should match A when B≈0, RMS ratio={}",
        ratio
    );
}
