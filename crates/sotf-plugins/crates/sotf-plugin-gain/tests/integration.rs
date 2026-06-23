// ============================================================================
// Integration tests for sotf-plugin-gain
//
// These tests exercise the crate's public API through the `ParametricPlugin`
// trait as a black box. No internal modules are imported.
// ============================================================================

use sotf_host::db_to_linear;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_plugin::ParametricPlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_gain::{default_smoothing_ms, GainPlugin, GainPluginParams};

const SR: u32 = 48000;
const FRAMES: usize = 512;

// ----------------------------------------------------------------------------
// Construction and Plugin trait metadata
// ----------------------------------------------------------------------------

#[test]
fn new_plugin_has_expected_metadata() {
    let plugin = GainPlugin::new(2, 0.0);
    let info = plugin.plugin_info();
    assert_eq!(info.name, "Gain");
    assert_eq!(info.version, "1.2.0");
    assert_eq!(info.author, "Sotf");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.latency_samples(), 0);
}

#[test]
fn from_params_uses_global_gain_when_channel_gains_empty() {
    let params = GainPluginParams {
        gain_db: -12.0,
        smoothing_ms: default_smoothing_ms(),
        channel_gains: vec![],
    };
    let plugin = GainPlugin::from_params(2, params).unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert!(!plugin.is_per_channel());
    assert_eq!(plugin.gain_db(), -12.0);
}

#[test]
fn from_params_uses_per_channel_gains() {
    let params = GainPluginParams {
        gain_db: 0.0,
        smoothing_ms: default_smoothing_ms(),
        channel_gains: vec![-6.0, 3.0],
    };
    let plugin = GainPlugin::from_params(2, params).unwrap();
    assert!(plugin.is_per_channel());
    assert_eq!(plugin.channel_gain_db(0), Some(-6.0));
    assert_eq!(plugin.channel_gain_db(1), Some(3.0));
}

#[test]
fn from_params_rejects_channel_count_mismatch() {
    let params = GainPluginParams {
        gain_db: 0.0,
        smoothing_ms: default_smoothing_ms(),
        channel_gains: vec![0.0, 0.0, 0.0],
    };
    match GainPlugin::from_params(2, params) {
        Ok(_) => panic!("expected channel count mismatch error"),
        Err(err) => {
            assert!(err.contains("2") || err.contains("expected"));
            assert!(err.contains("3") || err.contains("got") || err.contains("actual"));
        }
    }
}

#[test]
fn new_per_channel_rejects_empty_vec() {
    let result = GainPlugin::new_per_channel(vec![]);
    assert!(result.is_err());
}

// ----------------------------------------------------------------------------
// Parameter discovery and round-trips
// ----------------------------------------------------------------------------

#[test]
fn parameters_include_expected_controls() {
    let plugin = GainPlugin::new(2, 0.0);
    let params = plugin.parametric_parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"gain_db"));
    assert!(ids.contains(&"smoothing_ms"));
}

#[test]
fn parameters_include_per_channel_gains() {
    let plugin = GainPlugin::new_per_channel(vec![0.0, -6.0, 6.0]).unwrap();
    let params = plugin.parametric_parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"gain_db_0"));
    assert!(ids.contains(&"gain_db_1"));
    assert!(ids.contains(&"gain_db_2"));
}

#[test]
fn gain_db_roundtrip() {
    let mut plugin = GainPlugin::new(2, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    plugin
        .parametric_set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(-6.0))
        .unwrap();
    assert_eq!(
        plugin.parametric_get_parameter(&ParameterId::from("gain_db")),
        Some(ParameterValue::Float(-6.0))
    );
}

#[test]
fn smoothing_ms_roundtrip() {
    let mut plugin = GainPlugin::new(2, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    plugin
        .parametric_set_parameter(
            ParameterId::from("smoothing_ms"),
            ParameterValue::Float(50.0),
        )
        .unwrap();
    assert_eq!(
        plugin.parametric_get_parameter(&ParameterId::from("smoothing_ms")),
        Some(ParameterValue::Float(50.0))
    );
}

#[test]
fn per_channel_gain_roundtrip() {
    let mut plugin = GainPlugin::new_per_channel(vec![0.0, 0.0]).unwrap();
    plugin.plugin_initialize(SR).unwrap();
    plugin
        .parametric_set_parameter(ParameterId::from("gain_db_0"), ParameterValue::Float(3.0))
        .unwrap();
    plugin
        .parametric_set_parameter(ParameterId::from("gain_db_1"), ParameterValue::Float(-3.0))
        .unwrap();
    assert_eq!(
        plugin.parametric_get_parameter(&ParameterId::from("gain_db_0")),
        Some(ParameterValue::Float(3.0))
    );
    assert_eq!(
        plugin.parametric_get_parameter(&ParameterId::from("gain_db_1")),
        Some(ParameterValue::Float(-3.0))
    );
}

// ----------------------------------------------------------------------------
// State transitions
// ----------------------------------------------------------------------------

#[test]
fn initialize_then_process_works() {
    let mut plugin = GainPlugin::new(2, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    let input = vec![0.5f32; FRAMES * 2];
    let mut buffer = vec![0.0f32; input.len()];
    let frames = plugin
        .process(&input, &mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
}

#[test]
fn reset_does_not_break_processing() {
    let mut plugin = GainPlugin::new(2, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    plugin.plugin_reset();
    let input = vec![0.5f32; FRAMES * 2];
    let mut buffer = vec![0.0f32; input.len()];
    let frames = plugin
        .process(&input, &mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
}

#[test]
fn initialize_changes_sample_rate() {
    let mut plugin = GainPlugin::with_smoothing(1, 0.0, default_smoothing_ms());
    plugin.plugin_initialize(44100).unwrap();
    plugin.set_gain_db(-6.0);

    // Process enough samples at 96k to let the smoother settle.
    plugin.plugin_initialize(96000).unwrap();
    let num_frames = 19200;
    let input = vec![1.0f32; num_frames];
    let mut buf = vec![0.0f32; input.len()];
    plugin
        .process(&input, &mut buf, &ProcessContext::new(96000, num_frames))
        .unwrap();
    let target = db_to_linear(-6.0);
    assert!((buf[num_frames - 1] - target).abs() < 0.01);
}

// ----------------------------------------------------------------------------
// Audio processing
// ----------------------------------------------------------------------------

#[test]
fn unity_gain_passthrough() {
    let mut plugin = GainPlugin::with_smoothing(2, 0.0, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    let input = vec![0.1f32, 0.2, 0.3, 0.4];
    let mut buffer = vec![0.0f32; input.len()];
    plugin
        .process(&input, &mut buffer, &ProcessContext::new(SR, 2))
        .unwrap();
    for (i, (&out, &inp)) in buffer.iter().zip(input.iter()).enumerate() {
        assert!(
            (out - inp).abs() < 1e-5,
            "sample {i}: expected {}, got {}",
            inp,
            out
        );
    }
}

#[test]
fn positive_gain_scales_signal() {
    let mut plugin = GainPlugin::with_smoothing(2, 6.0, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    let input = vec![0.1f32, 0.2, 0.3, 0.4];
    let mut buffer = vec![0.0f32; input.len()];
    plugin
        .process(&input, &mut buffer, &ProcessContext::new(SR, 2))
        .unwrap();
    let expected_linear = db_to_linear(6.0);
    for (i, (&out, &inp)) in buffer.iter().zip(input.iter()).enumerate() {
        assert!(
            (out - inp * expected_linear).abs() < 1e-4,
            "sample {i}: expected {}, got {}",
            inp * expected_linear,
            out
        );
    }
}

#[test]
fn negative_gain_attenuates_signal() {
    let mut plugin = GainPlugin::with_smoothing(2, -6.0, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    let input = vec![1.0f32, 1.0, 1.0, 1.0];
    let mut buffer = vec![0.0f32; input.len()];
    plugin
        .process(&input, &mut buffer, &ProcessContext::new(SR, 2))
        .unwrap();
    let expected = db_to_linear(-6.0);
    for (i, &out) in buffer.iter().enumerate() {
        assert!(
            (out - expected).abs() < 1e-4,
            "sample {i}: expected {}, got {}",
            expected,
            out
        );
    }
}

#[test]
fn per_channel_gains_apply_correctly() {
    let mut plugin = GainPlugin::new_per_channel(vec![0.0f32, -6.0]).unwrap();
    plugin.plugin_initialize(SR).unwrap();
    let input = vec![1.0f32, 1.0, 1.0, 1.0];
    let mut buffer = vec![0.0f32; input.len()];
    plugin
        .process(&input, &mut buffer, &ProcessContext::new(SR, 2))
        .unwrap();
    let ch0_gain = db_to_linear(0.0);
    let ch1_gain = db_to_linear(-6.0);
    assert!((buffer[0] - ch0_gain).abs() < 1e-4);
    assert!((buffer[1] - ch1_gain).abs() < 1e-4);
    assert!((buffer[2] - ch0_gain).abs() < 1e-4);
    assert!((buffer[3] - ch1_gain).abs() < 1e-4);
}

#[test]
fn zero_frames_returns_zero_and_leaves_buffer() {
    let mut plugin = GainPlugin::new(2, 6.0);
    plugin.plugin_initialize(SR).unwrap();
    let input = vec![0.5f32, 0.6, 0.7, 0.8];
    let original = input.clone();
    let mut buffer = vec![0.0f32; input.len()];
    let processed = plugin
        .process(&input, &mut buffer, &ProcessContext::new(SR, 0))
        .unwrap();
    assert_eq!(processed, 0);
    assert_eq!(buffer, original);
}

// ----------------------------------------------------------------------------
// Error paths visible through the public API
// ----------------------------------------------------------------------------

#[test]
fn set_unknown_parameter_fails() {
    let mut plugin = GainPlugin::new(2, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    let result = plugin
        .parametric_set_parameter(ParameterId::from("nonexistent"), ParameterValue::Float(1.0));
    match result {
        Ok(_) => panic!("expected unknown parameter error"),
        Err(err) => {
            assert!(
                err.contains("nonexistent") || err.contains("Invalid") || err.contains("unknown")
            );
        }
    }
}

#[test]
fn set_gain_out_of_range_fails() {
    let mut plugin = GainPlugin::new(2, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    assert!(plugin
        .parametric_set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(21.0))
        .is_err());
    assert!(plugin
        .parametric_set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(-61.0))
        .is_err());
}

#[test]
fn set_smoothing_out_of_range_fails() {
    let mut plugin = GainPlugin::new(2, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    assert!(plugin
        .parametric_set_parameter(
            ParameterId::from("smoothing_ms"),
            ParameterValue::Float(201.0)
        )
        .is_err());
    assert!(plugin
        .parametric_set_parameter(
            ParameterId::from("smoothing_ms"),
            ParameterValue::Float(-1.0)
        )
        .is_err());
}

#[test]
fn set_non_finite_gain_fails() {
    let mut plugin = GainPlugin::new(2, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    assert!(plugin
        .parametric_set_parameter(
            ParameterId::from("gain_db"),
            ParameterValue::Float(f32::NAN)
        )
        .is_err());
    assert!(plugin
        .parametric_set_parameter(
            ParameterId::from("gain_db"),
            ParameterValue::Float(f32::INFINITY)
        )
        .is_err());
}

#[test]
fn set_channel_gain_out_of_bounds_fails() {
    let mut plugin = GainPlugin::new(2, 0.0);
    plugin.plugin_initialize(SR).unwrap();
    let result =
        plugin.parametric_set_parameter(ParameterId::from("gain_db_5"), ParameterValue::Float(0.0));
    match result {
        Ok(_) => panic!("expected out-of-bounds channel gain error"),
        Err(err) => {
            assert!(err.contains("OOB") || err.contains("out of bounds") || err.contains("bounds"));
        }
    }
}

#[test]
fn output_is_finite_for_finite_input() {
    let mut plugin = GainPlugin::new(2, 12.0);
    plugin.plugin_initialize(SR).unwrap();
    let input: Vec<f32> = (0..FRAMES * 2)
        .map(|i| (i as f32 * 0.1).sin() * 0.5)
        .collect();
    let mut buffer = vec![0.0f32; input.len()];
    plugin
        .process(&input, &mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert!(buffer.iter().all(|s| s.is_finite()));
}
