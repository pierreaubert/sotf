// ============================================================================
// Integration tests for sotf-plugin-eq
//
// These tests exercise the crate's public API as a black box through the
// InPlacePlugin trait (and the Plugin adapter) with realistic end-to-end
// workflows.
// ============================================================================

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use sotf_host::AutoGainParams;
use sotf_host::InPlacePluginAdapter;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, Plugin, ProcessContext};
use sotf_plugin_eq::{BiquadFilterConfig, EqFilterTopology, EqPlugin, EqPluginParams};

const SAMPLE_RATE: u32 = 48_000;
const FRAMES: usize = 64;

// ----------------------------------------------------------------------------
// Instantiation and metadata
// ----------------------------------------------------------------------------

#[test]
fn info_returns_expected_metadata() {
    let plugin = EqPlugin::new(2, vec![]);
    let info = plugin.info();
    assert_eq!(info.name, "Parametric EQ");
    assert_eq!(info.version, "2.0.0");
    assert_eq!(info.author, "SotF");
}

#[test]
fn channels_matches_constructor() {
    let plugin = EqPlugin::new(2, vec![]);
    assert_eq!(plugin.channels(), 2);

    let plugin = EqPlugin::new(1, vec![]);
    assert_eq!(plugin.channels(), 1);
}

#[test]
fn parameters_include_global_and_band_params() {
    let f = Biquad::new(BiquadFilterType::Peak, 1000.0, SAMPLE_RATE as f64, 1.0, 0.0);
    let plugin = EqPlugin::new(1, vec![f]);

    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();

    assert!(ids.contains(&"auto_gain_enabled"));
    assert!(ids.contains(&"oversampling"));
    assert!(ids.contains(&"tdf2"));
    assert!(ids.contains(&"topology"));
    assert!(ids.contains(&"band_0_freq"));
    assert!(ids.contains(&"band_0_q"));
    assert!(ids.contains(&"band_0_gain"));
    assert!(ids.contains(&"band_0_order"));
}

// ----------------------------------------------------------------------------
// Happy-path processing
// ----------------------------------------------------------------------------

#[test]
fn empty_chain_is_exact_passthrough() {
    let mut plugin = EqPlugin::new(2, vec![]);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input: Vec<f32> = (0..FRAMES * 2)
        .map(|i| ((i % 13) as f32 - 6.0) / 7.0)
        .collect();
    let mut output = input.clone();

    let processed = plugin
        .process_in_place(&mut output, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();

    assert_eq!(processed, FRAMES);
    let max_error: f32 = input
        .iter()
        .zip(output.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_error < 1e-5,
        "empty EQ chain should pass through unchanged: max_error={}",
        max_error
    );
}

#[test]
fn from_params_builds_processable_plugin() {
    let params = EqPluginParams {
        filters: vec![BiquadFilterConfig {
            filter_type: "peak".to_string(),
            freq: 1000.0,
            q: 1.0,
            db_gain: 6.0,
            order: 2,
            topology: EqFilterTopology::Biquad,
            lambda: None,
            kautz_sections: vec![],
        }],
        channel_filters: None,
        auto_gain: AutoGainParams::default(),
    };

    let mut plugin = EqPlugin::from_params(2, SAMPLE_RATE, params).unwrap();
    let mut buffer = vec![0.5f32; FRAMES * 2];

    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();

    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn plugin_adapter_exposes_plugin_trait() {
    let mut plugin = InPlacePluginAdapter::new(EqPlugin::new(1, vec![]));
    plugin.initialize(SAMPLE_RATE).unwrap();

    assert_eq!(plugin.input_channels(), 1);
    assert_eq!(plugin.output_channels(), 1);

    let input = vec![0.3f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES];
    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(SAMPLE_RATE, FRAMES),
        )
        .unwrap();

    let max_error: f32 = input
        .iter()
        .zip(output.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_error < 1e-5,
        "adapter passthrough failed: {}",
        max_error
    );
}

// ----------------------------------------------------------------------------
// Parameter roundtrips and state transitions
// ----------------------------------------------------------------------------

#[test]
fn parameter_roundtrip_global_params() {
    let mut plugin = EqPlugin::new(2, vec![]);
    plugin.initialize(SAMPLE_RATE).unwrap();

    plugin
        .set_parameter(ParameterId::from("oversampling"), ParameterValue::Int(2))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("oversampling")),
        Some(ParameterValue::Int(2))
    );

    plugin
        .set_parameter(ParameterId::from("tdf2"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("tdf2")),
        Some(ParameterValue::Bool(true))
    );

    plugin
        .set_parameter(
            ParameterId::from("topology"),
            ParameterValue::String("SVF".to_string()),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("topology")),
        Some(ParameterValue::String("SVF".to_string()))
    );

    plugin
        .set_parameter(
            ParameterId::from("auto_gain_enabled"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("auto_gain_enabled")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn parameter_roundtrip_band_params() {
    let f = Biquad::new(BiquadFilterType::Peak, 1000.0, SAMPLE_RATE as f64, 1.0, 0.0);
    let mut plugin = EqPlugin::new(1, vec![f]);
    plugin.initialize(SAMPLE_RATE).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("band_0_freq"),
            ParameterValue::Float(2500.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_0_freq")),
        Some(ParameterValue::Float(2500.0))
    );

    plugin
        .set_parameter(ParameterId::from("band_0_q"), ParameterValue::Float(2.5))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_0_q")),
        Some(ParameterValue::Float(2.5))
    );

    plugin
        .set_parameter(
            ParameterId::from("band_0_gain"),
            ParameterValue::Float(-6.0),
        )
        .unwrap();
    let got = plugin.get_parameter(&ParameterId::from("band_0_gain"));
    assert!(
        matches!(got, Some(ParameterValue::Float(v)) if (v - (-6.0)).abs() < 0.01),
        "band gain round-trip drift: {:?}",
        got
    );

    plugin
        .set_parameter(ParameterId::from("band_0_order"), ParameterValue::Int(4))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_0_order")),
        Some(ParameterValue::Int(4))
    );
}

#[test]
fn topology_switch_to_svf_produces_finite_output() {
    let f = Biquad::new(BiquadFilterType::Peak, 1000.0, SAMPLE_RATE as f64, 1.0, 6.0);
    let mut plugin = EqPlugin::new(2, vec![f]);
    plugin.initialize(SAMPLE_RATE).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("topology"),
            ParameterValue::String("SVF".to_string()),
        )
        .unwrap();

    let mut buffer = vec![0.25f32; FRAMES * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();

    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn oversampling_switch_updates_internal_state() {
    let mut plugin = EqPlugin::new(1, vec![]);
    plugin.initialize(SAMPLE_RATE).unwrap();

    // Enable 2x oversampling
    plugin
        .set_parameter(ParameterId::from("oversampling"), ParameterValue::Int(2))
        .unwrap();
    let mut buffer = vec![0.1f32; FRAMES];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();
    assert!(buffer.iter().all(|s| s.is_finite()));

    // Disable oversampling
    plugin
        .set_parameter(ParameterId::from("oversampling"), ParameterValue::Int(1))
        .unwrap();
    let mut buffer = vec![0.1f32; FRAMES];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn reset_returns_detinistic_state() {
    let f = Biquad::new(
        BiquadFilterType::Highpass,
        200.0,
        SAMPLE_RATE as f64,
        0.707,
        0.0,
    );
    let mut plugin = EqPlugin::new(1, vec![f]);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let input: Vec<f32> = (0..FRAMES).map(|i| ((i % 5) as f32) / 5.0).collect();

    let mut run1 = input.clone();
    plugin
        .process_in_place(&mut run1, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();

    plugin.reset();

    let mut run2 = input.clone();
    plugin
        .process_in_place(&mut run2, &ProcessContext::new(SAMPLE_RATE, FRAMES))
        .unwrap();

    let max_error: f32 = run1
        .iter()
        .zip(run2.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_error < 1e-5,
        "reset should restore deterministic state: max_error={}",
        max_error
    );
}

// ----------------------------------------------------------------------------
// Error paths
// ----------------------------------------------------------------------------

#[test]
fn invalid_oversampling_factor_errors() {
    let mut plugin = EqPlugin::new(1, vec![]);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let result = plugin.set_parameter(ParameterId::from("oversampling"), ParameterValue::Int(3));
    assert!(result.is_err(), "oversampling factor 3 should be rejected");
    assert!(result.unwrap_err().contains("Invalid oversampling"));
}

#[test]
fn odd_band_order_errors() {
    let f = Biquad::new(BiquadFilterType::Peak, 1000.0, SAMPLE_RATE as f64, 1.0, 0.0);
    let mut plugin = EqPlugin::new(1, vec![f]);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let result = plugin.set_parameter(ParameterId::from("band_0_order"), ParameterValue::Int(3));
    assert!(result.is_err(), "odd band order should be rejected");
    assert!(result.unwrap_err().contains("even"));
}

#[test]
fn unknown_parameter_errors() {
    let mut plugin = EqPlugin::new(1, vec![]);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let result = plugin.set_parameter(
        ParameterId::from("not_a_real_param"),
        ParameterValue::Float(1.0),
    );
    assert!(result.is_err(), "unknown parameter should be rejected");
}

#[test]
fn new_per_channel_rejects_mismatched_channel_count() {
    let f = Biquad::new(BiquadFilterType::Peak, 1000.0, SAMPLE_RATE as f64, 1.0, 0.0);
    let result = EqPlugin::new_per_channel(2, vec![vec![f]]);
    assert!(result.is_err(), "channel count mismatch should be rejected");
}
