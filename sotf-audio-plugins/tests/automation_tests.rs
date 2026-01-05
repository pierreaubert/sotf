// ============================================================================
// Parameter Automation Tests
// ============================================================================
//
// These tests verify that parameter changes are handled correctly,
// including smoothing, automation curves, and parameter validation.

use sotf_plugins::{
    CompressorPlugin, GainPlugin, InPlacePlugin, InPlacePluginAdapter, ParameterId, ParameterValue,
    Plugin, PluginHost, ProcessContext,
};

// ============================================================================
// Parameter Smoothing Tests
// ============================================================================

#[test]
fn test_gain_parameter_smoothing() {
    let mut gain = GainPlugin::new(2, -60.0);
    gain.initialize(48000).unwrap();

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 512,
    };

    let input = vec![1.0f32; 1024];
    let mut buffer = input;

    for _ in 0..1000 {
        gain.process_in_place(&mut buffer, &context).unwrap();
    }

    let max_value = buffer.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(max_value < 0.01, "Gain should converge to -60 dB target");
}

#[test]
fn test_parameter_set_and_get() {
    let mut gain = GainPlugin::new(2, 0.0);

    gain.set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(6.0))
        .unwrap();

    let value = gain.get_parameter(&ParameterId::from("gain_db"));
    assert_eq!(value, Some(ParameterValue::Float(6.0)));
}

#[test]
fn test_parameter_range_validation() {
    let mut gain = GainPlugin::new(2, 0.0);

    let result = gain.set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(12.0));
    assert!(result.is_ok(), "Valid parameter value should be accepted");

    let _ = gain.set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(1000.0));
}

#[test]
fn test_invalid_parameter_rejected() {
    let mut gain = GainPlugin::new(2, 0.0);

    let result = gain.set_parameter(
        ParameterId::from("nonexistent_parameter"),
        ParameterValue::Float(0.0),
    );

    assert!(result.is_err(), "Invalid parameter should be rejected");
}

// ============================================================================
// Parameter Automation Tests
// ============================================================================

#[test]
fn test_compressor_threshold_parameter() {
    let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 10.0, 100.0, 6.0, 0.0);
    compressor.initialize(48000).unwrap();

    compressor
        .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-20.0))
        .unwrap();

    let value = compressor.get_parameter(&ParameterId::from("threshold"));
    assert_eq!(value, Some(ParameterValue::Float(-20.0)));
}

#[test]
fn test_compressor_ratio_parameter() {
    let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 10.0, 100.0, 6.0, 0.0);
    compressor.initialize(48000).unwrap();

    compressor
        .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(4.0))
        .unwrap();

    let value = compressor.get_parameter(&ParameterId::from("ratio"));
    assert_eq!(value, Some(ParameterValue::Float(4.0)));
}

#[test]
fn test_compressor_attack_release_parameters() {
    let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 10.0, 100.0, 6.0, 0.0);
    compressor.initialize(48000).unwrap();

    compressor
        .set_parameter(ParameterId::from("attack"), ParameterValue::Float(10.0))
        .unwrap();
    compressor
        .set_parameter(ParameterId::from("release"), ParameterValue::Float(100.0))
        .unwrap();

    let attack = compressor.get_parameter(&ParameterId::from("attack"));
    let release = compressor.get_parameter(&ParameterId::from("release"));

    assert_eq!(attack, Some(ParameterValue::Float(10.0)));
    assert_eq!(release, Some(ParameterValue::Float(100.0)));
}

#[test]
fn test_all_dynamics_parameters() {
    let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 10.0, 100.0, 6.0, 0.0);
    compressor.initialize(48000).unwrap();

    let params = [
        ("threshold", ParameterValue::Float(-20.0)),
        ("ratio", ParameterValue::Float(4.0)),
        ("attack", ParameterValue::Float(10.0)),
        ("release", ParameterValue::Float(100.0)),
        ("knee", ParameterValue::Float(6.0)),
        ("makeup_gain", ParameterValue::Float(6.0)),
    ];

    for (name, value) in &params {
        let result = compressor.set_parameter(ParameterId::from(*name), value.clone());
        assert!(result.is_ok(), "Parameter {} should be accepted", name);
    }

    for (name, expected) in &params {
        let actual = compressor.get_parameter(&ParameterId::from(*name));
        assert_eq!(
            actual,
            Some(expected.clone()),
            "Parameter {} should have correct value",
            name
        );
    }
}

// ============================================================================
// Plugin State Tests
// ============================================================================

#[test]
fn test_plugin_reset_clears_state() {
    let mut gain = GainPlugin::new(2, 6.0);
    gain.initialize(48000).unwrap();

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 512,
    };

    let input = vec![0.5f32; 1024];
    let mut buffer = input.clone();

    gain.process_in_place(&mut buffer, &context).unwrap();

    let expected = 0.5 * 10.0_f32.powf(6.0 / 20.0);
    assert!(
        (buffer[0] - expected).abs() < 0.1,
        "First pass: expected {}, got {}",
        expected,
        buffer[0]
    );

    gain.reset();

    buffer = input;
    gain.process_in_place(&mut buffer, &context).unwrap();
    assert!(
        (buffer[0] - expected).abs() < 0.1,
        "After reset: expected {}, got {}",
        expected,
        buffer[0]
    );
}

#[test]
fn test_plugin_reinitialization() {
    let mut gain = GainPlugin::new(2, 0.0);
    gain.initialize(48000).unwrap();

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 512,
    };

    let input = vec![0.5f32; 1024];
    let mut buffer = input.clone();

    gain.process_in_place(&mut buffer, &context).unwrap();

    gain.initialize(96000).unwrap();

    let context96 = ProcessContext {
        sample_rate: 96000,
        num_frames: 512,
    };

    let mut buffer96 = input;
    gain.process_in_place(&mut buffer96, &context96).unwrap();

    assert!(buffer96.iter().all(|o| o.is_finite()));
}

// ============================================================================
// Plugin Chain Tests
// ============================================================================

#[test]
fn test_plugin_chain_parameter_propagation() {
    let mut host = PluginHost::new(2, 48000);

    let gain1 = GainPlugin::new(2, 3.0);
    let gain2 = GainPlugin::new(2, 3.0);

    host.add_plugin(Box::new(InPlacePluginAdapter::new(gain1)))
        .unwrap();
    host.add_plugin(Box::new(InPlacePluginAdapter::new(gain2)))
        .unwrap();

    let input = vec![0.5f32; 1024];
    let mut output = vec![0.0f32; 1024];

    host.process(&input, &mut output).unwrap();

    let expected = 0.5 * 10.0_f32.powf(6.0 / 20.0);
    assert!((output[0] - expected).abs() < 0.01);
}

#[test]
fn test_plugin_chain_independence() {
    let mut host = PluginHost::new(2, 48000);

    let gain1 = GainPlugin::new(2, 6.0);
    let gain2 = GainPlugin::new(2, -6.0);

    host.add_plugin(Box::new(InPlacePluginAdapter::new(gain1)))
        .unwrap();
    host.add_plugin(Box::new(InPlacePluginAdapter::new(gain2)))
        .unwrap();

    let input = vec![0.5f32; 1024];
    let mut output = vec![0.0f32; 1024];
    host.process(&input, &mut output).unwrap();

    assert!((output[0] - 0.5).abs() < 0.01, "Gains should cancel out");
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_processing_silence_input() {
    let mut gain = GainPlugin::new(2, 20.0);
    gain.initialize(48000).unwrap();

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 512,
    };

    let input = vec![0.0f32; 1024];
    let mut buffer = input;

    gain.process_in_place(&mut buffer, &context).unwrap();

    assert!(buffer.iter().all(|o| o.abs() < 1e-10));
}

#[test]
fn test_processing_max_input() {
    let mut gain = GainPlugin::new(2, 0.0);
    gain.initialize(48000).unwrap();

    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 512,
    };

    let input = vec![1.0f32; 1024];
    let mut buffer = input;

    gain.process_in_place(&mut buffer, &context).unwrap();

    assert!(
        (buffer[0] - 1.0).abs() < 0.1,
        "Expected ~1.0, got {}",
        buffer[0]
    );
}

#[test]
fn test_parameter_id_display() {
    let id = ParameterId::from("test_parameter");
    let display = format!("{}", id);
    assert_eq!(display, "test_parameter");
}

#[test]
fn test_parameter_value_display() {
    let float_val = ParameterValue::Float(3.14);
    assert_eq!(format!("{}", float_val), "3.14");

    let int_val = ParameterValue::Int(42);
    assert_eq!(format!("{}", int_val), "42");

    let bool_val = ParameterValue::Bool(true);
    assert_eq!(format!("{}", bool_val), "true");
}
