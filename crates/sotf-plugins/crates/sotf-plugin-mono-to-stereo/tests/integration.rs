// ============================================================================
// Integration tests for sotf-plugin-mono-to-stereo
//
// These tests exercise the public `Plugin` trait and crate-specific API as a
// black box — no internal modules are imported.
// ============================================================================

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_mono_to_stereo::{MonoToStereoPlugin, MonoToStereoPluginParams};

const SR: u32 = 48000;
const FRAMES: usize = 4096;

// ----------------------------------------------------------------------------
// Construction and Plugin trait metadata
// ----------------------------------------------------------------------------

#[test]
fn new_plugin_has_expected_metadata() {
    let plugin = MonoToStereoPlugin::new();
    let info = plugin.info();
    assert_eq!(info.name, "MonoToStereo");
    assert_eq!(info.author, "Sotf");
    assert_eq!(plugin.input_channels(), 1);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn from_params_uses_provided_values() {
    let params = MonoToStereoPluginParams {
        stereo_width: 0.25,
        freq_dependent: false,
        haas_delay_ms: 2.5,
    };
    let plugin = MonoToStereoPlugin::from_params(1, params);
    assert_eq!(plugin.input_channels(), 1);
    assert_eq!(plugin.output_channels(), 2);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("stereo_width")),
        Some(ParameterValue::Float(0.25))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("haas_delay_ms")),
        Some(ParameterValue::Float(2.5))
    );
}

// ----------------------------------------------------------------------------
// Parameter discovery and round-trips
// ----------------------------------------------------------------------------

#[test]
fn parameters_include_expected_controls() {
    let plugin = MonoToStereoPlugin::new();
    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"stereo_width"));
    assert!(ids.contains(&"haas_delay_ms"));
    assert!(ids.contains(&"decor_low_hz"));
    assert!(ids.contains(&"decor_high_hz"));
    assert!(ids.contains(&"freq_dependent"));
}

#[test]
fn stereo_width_roundtrip() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("stereo_width"),
            ParameterValue::Float(0.75),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("stereo_width")),
        Some(ParameterValue::Float(0.75))
    );
}

#[test]
fn haas_delay_roundtrip() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("haas_delay_ms"),
            ParameterValue::Float(3.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("haas_delay_ms")),
        Some(ParameterValue::Float(3.0))
    );
}

#[test]
fn decor_frequencies_roundtrip() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("decor_low_hz"),
            ParameterValue::Float(200.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("decor_high_hz"),
            ParameterValue::Float(3000.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("decor_low_hz")),
        Some(ParameterValue::Float(200.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("decor_high_hz")),
        Some(ParameterValue::Float(3000.0))
    );
}

#[test]
fn freq_dependent_roundtrip() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("freq_dependent"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("freq_dependent")),
        Some(ParameterValue::Bool(false))
    );
}

// ----------------------------------------------------------------------------
// Audio processing
// ----------------------------------------------------------------------------

#[test]
fn process_zero_input_produces_finite_output() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(SR).unwrap();

    let input = vec![0.0f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    let processed = plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert_eq!(processed, FRAMES);
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn process_dc_input_produces_stereo_output() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(SR).unwrap();

    let dc = 0.5f32;
    let input = vec![dc; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    // After the STFT latency, some sample should carry the DC contribution.
    let max_left = output
        .iter()
        .step_by(2)
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    let max_right = output
        .iter()
        .skip(1)
        .step_by(2)
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_left > 0.01 || max_right > 0.01,
        "stereo output should carry non-zero energy for DC input (max L={}, R={})",
        max_left,
        max_right
    );
}

#[test]
fn zero_width_outputs_nearly_identical_channels() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("stereo_width"),
            ParameterValue::Float(0.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("haas_delay_ms"),
            ParameterValue::Float(0.0),
        )
        .unwrap();

    let dc = 0.5f32;
    let input = vec![dc; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    let last_left = output[(FRAMES - 1) * 2];
    let last_right = output[(FRAMES - 1) * 2 + 1];
    assert!(
        (last_left - last_right).abs() < 0.05,
        "with width=0 and no haas delay left and right should be nearly identical, got L={} R={}",
        last_left,
        last_right
    );
}

// ----------------------------------------------------------------------------
// State transitions
// ----------------------------------------------------------------------------

#[test]
fn reset_then_process_still_works() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(SR).unwrap();

    let input = vec![0.5f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    plugin.reset();

    let mut output2 = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output2, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert!(output2.iter().all(|s| s.is_finite()));
}

#[test]
fn initialize_changes_sample_rate() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(44100).unwrap();
    plugin.initialize(96000).unwrap();
    // No public accessor for sample rate; success means state updated.
    let input = vec![0.5f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(96000, FRAMES))
        .unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
}

// ----------------------------------------------------------------------------
// Error paths visible through the public API
// ----------------------------------------------------------------------------

#[test]
fn set_unknown_parameter_fails() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("nonexistent"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("nonexistent"));
}

#[test]
fn process_with_correct_output_size_succeeds() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(SR).unwrap();
    let input = vec![0.5f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    let frames = plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
}

#[test]
fn latency_reported_as_fft_size() {
    let plugin = MonoToStereoPlugin::new();
    // The plugin reports STFT pipeline latency equal to its FFT size.
    assert_eq!(plugin.latency_samples(), 2048);
}
