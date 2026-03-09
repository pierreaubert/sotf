use super::*;
use sotf_host::host::DawHost;
use sotf_host::plugin::{PluginInfo, ProcessContext};

/// Pass-through plugin that reports a fixed latency for testing.
struct LatencyPassthrough {
    channels: usize,
    latency: usize,
}

impl Plugin for LatencyPassthrough {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("LatencyPassthrough", "0.1", "test")
    }
    fn input_channels(&self) -> usize {
        self.channels
    }
    fn output_channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<sotf_host::parameters::Parameter> {
        vec![]
    }
    fn set_parameter(
        &mut self,
        _: sotf_host::parameters::ParameterId,
        _: sotf_host::parameters::ParameterValue,
    ) -> Result<(), String> {
        Err("none".into())
    }
    fn get_parameter(
        &self,
        _: &sotf_host::parameters::ParameterId,
    ) -> Option<sotf_host::parameters::ParameterValue> {
        None
    }
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        output[..input.len()].copy_from_slice(input);
        Ok(context.num_frames)
    }
    fn latency_samples(&self) -> usize {
        self.latency
    }
}

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 2,
    };

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 4800,
    };

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 4800,
    };

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 1024,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Pass-through with no plugins should work
    // Note: smoothers may affect output slightly
}

#[test]
fn test_reset() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(48000).unwrap();

    // Process some audio
    let input = vec![1.0; 1000 * 2];
    let mut output = vec![0.0; 1000 * 2];
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 1000,
    };
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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 4800,
    };

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 4800,
    };
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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 1024,
    };

    plugin.process(&input, &mut output, &context).unwrap();
}

// ========================================================================
// Auto-Gain Tests
// ========================================================================

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    // Process while disabled
    for _ in 0..5 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();
    assert!(
        ab_data.auto_gain_db.abs() < 0.01,
        "Auto-gain should be ~0 when disabled"
    );

    // Enable auto-gain at runtime
    plugin
        .set_parameter(
            ParameterId("auto_gain_enabled".to_string()),
            ParameterValue::Bool(true),
        )
        .unwrap();

    // Process more to build up auto-gain
    for _ in 0..10 {
        plugin.process(&input, &mut output, &context).unwrap();
    }

    let data = plugin.get_data().unwrap();
    let ab_data = data.downcast_ref::<ABCompareData>().unwrap();

    // Now auto-gain should be active (negative since B is louder)
    assert!(
        ab_data.auto_gain_db < -1.0,
        "Auto-gain should be negative after enabling, got {} dB",
        ab_data.auto_gain_db
    );

    // Disable again
    plugin
        .set_parameter(
            ParameterId("auto_gain_enabled".to_string()),
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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

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
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

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

// ========================================================================
// Latency Compensation Tests
// ========================================================================

#[test]
fn test_latency_compensation() {
    // Path A: passthrough (0 latency)
    // Path B: passthrough reporting 64 samples latency
    // The plugin should delay path A by 64 frames to align them.
    let channels = 2;
    let latency_frames = 64;

    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(48000).unwrap();

    // Replace host_b with one containing a latency-reporting plugin
    let mut host_b = DawHost::new(channels, 48000);
    host_b
        .add_plugin(Box::new(LatencyPassthrough {
            channels,
            latency: latency_frames,
        }))
        .unwrap();
    host_b.build().unwrap();
    plugin.host_b = host_b;
    plugin.update_latency_compensation();

    // Verify reported latency = max of both paths
    assert_eq!(plugin.latency_samples(), latency_frames);

    // Verify delay_a compensates the shorter path A
    assert_eq!(plugin.delay_a.len, latency_frames * channels);
    assert_eq!(plugin.delay_b.len, 0);

    // Send an impulse and verify alignment:
    // Path A output should be delayed by latency_frames relative to input.
    let num_frames = 256;
    let mut input = vec![0.0f32; num_frames * channels];
    // Impulse at frame 0
    for sample in input.iter_mut().take(channels) {
        *sample = 1.0;
    }

    let mut output = vec![0.0f32; num_frames * channels];
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };

    // Use pure-A mode (mix = -1) with auto-gain disabled
    plugin
        .set_parameter(
            ParameterId("mix".to_string()),
            ParameterValue::Float(-1.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId("auto_gain_enabled".to_string()),
            ParameterValue::Bool(false),
        )
        .unwrap();
    // Set very fast transition so smoother doesn't interfere
    plugin
        .set_parameter(
            ParameterId("mix_transition_ms".to_string()),
            ParameterValue::Float(5.0),
        )
        .unwrap();

    // Process a few silent blocks first to settle the smoother at -1.0
    let silent = vec![0.0f32; num_frames * channels];
    let mut discard = vec![0.0f32; num_frames * channels];
    for _ in 0..10 {
        plugin.process(&silent, &mut discard, &context).unwrap();
    }

    // Now send the impulse
    plugin.process(&input, &mut output, &context).unwrap();

    // Path A's impulse (frame 0) should appear at frame latency_frames in output
    // because delay_a delays it by latency_frames.
    let impulse_idx = latency_frames * channels;
    for ch in 0..channels {
        // Before the delay point: should be ~0
        // (tiny leakage from crossfade smoother is acceptable)
        if latency_frames > 0 {
            assert!(
                output[ch].abs() < 0.001,
                "Frame 0 ch {} should be silent (delayed), got {}",
                ch,
                output[ch]
            );
        }
        // At the delay point: should be ~1.0
        assert!(
            (output[impulse_idx + ch] - 1.0).abs() < 0.01,
            "Frame {} ch {} should be ~1.0 (impulse), got {}",
            latency_frames,
            ch,
            output[impulse_idx + ch]
        );
    }
}

#[test]
fn test_latency_compensation_reset() {
    let channels = 2;

    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(48000).unwrap();

    // Add latency to path B
    let mut host_b = DawHost::new(channels, 48000);
    host_b
        .add_plugin(Box::new(LatencyPassthrough {
            channels,
            latency: 32,
        }))
        .unwrap();
    host_b.build().unwrap();
    plugin.host_b = host_b;
    plugin.update_latency_compensation();

    // Fill delay with non-zero data
    let num_frames = 64;
    let input = vec![1.0f32; num_frames * channels];
    let mut output = vec![0.0f32; num_frames * channels];
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames,
    };
    plugin.process(&input, &mut output, &context).unwrap();

    // Reset should clear delay line contents
    plugin.reset();

    // Delay buffer should be all zeros after reset
    assert!(plugin.delay_a.buffer.iter().all(|&s| s == 0.0));
    assert!(plugin.delay_b.buffer.iter().all(|&s| s == 0.0));
}

#[test]
fn test_latency_compensation_equal_latency() {
    let channels = 2;

    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(48000).unwrap();

    // Both paths: 32 samples latency
    let mut host_a = DawHost::new(channels, 48000);
    host_a
        .add_plugin(Box::new(LatencyPassthrough {
            channels,
            latency: 32,
        }))
        .unwrap();
    host_a.build().unwrap();
    let mut host_b = DawHost::new(channels, 48000);
    host_b
        .add_plugin(Box::new(LatencyPassthrough {
            channels,
            latency: 32,
        }))
        .unwrap();
    host_b.build().unwrap();
    plugin.host_a = host_a;
    plugin.host_b = host_b;
    plugin.update_latency_compensation();

    // No compensation needed — both delays should be 0
    assert_eq!(plugin.delay_a.len, 0);
    assert_eq!(plugin.delay_b.len, 0);
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
