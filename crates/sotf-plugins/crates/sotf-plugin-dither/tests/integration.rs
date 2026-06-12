// ============================================================================
// Integration tests for sotf-plugin-dither
//
// Exercises the public InPlacePlugin API and crate-specific constructors as a
// black box.
// ============================================================================

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_dither::{DitherPlugin, DitherPluginParams};

const SR: u32 = 48000;
const FRAMES: usize = 256;

// ----------------------------------------------------------------------------
// Construction and metadata
// ----------------------------------------------------------------------------

#[test]
fn new_plugin_has_expected_metadata() {
    let plugin = DitherPlugin::new(2);
    let info = plugin.info();
    assert_eq!(info.name, "Dither");
    assert_eq!(info.author, "Sotf");
    assert_eq!(plugin.channels(), 2);
}

#[test]
fn from_params_honours_supplied_values() {
    let params = DitherPluginParams {
        bit_depth: 1,
        noise_shaping: false,
        dither_type: 2,
    };
    let plugin = DitherPlugin::from_params(2, params);
    assert_eq!(plugin.channels(), 2);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("bit_depth")),
        Some(ParameterValue::Int(1))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("noise_shaping")),
        Some(ParameterValue::Bool(false))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dither_type")),
        Some(ParameterValue::Int(2))
    );
}

// ----------------------------------------------------------------------------
// Parameter discovery and round-trips
// ----------------------------------------------------------------------------

#[test]
fn parameters_include_all_public_params() {
    let plugin = DitherPlugin::new(2);
    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"bit_depth"));
    assert!(ids.contains(&"noise_shaping"));
    assert!(ids.contains(&"dither_type"));
}

#[test]
fn bit_depth_roundtrip() {
    let mut plugin = DitherPlugin::new(2);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("bit_depth"), ParameterValue::Int(2))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("bit_depth")),
        Some(ParameterValue::Int(2))
    );
}

#[test]
fn noise_shaping_roundtrip() {
    let mut plugin = DitherPlugin::new(2);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("noise_shaping"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("noise_shaping")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn dither_type_roundtrip() {
    let mut plugin = DitherPlugin::new(2);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("dither_type"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dither_type")),
        Some(ParameterValue::Int(1))
    );
}

#[test]
fn out_of_range_ints_are_clamped_not_rejected() {
    let mut plugin = DitherPlugin::new(2);
    plugin.initialize(SR).unwrap();
    // bit_depth max is 2
    plugin
        .set_parameter(ParameterId::from("bit_depth"), ParameterValue::Int(100))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("bit_depth")),
        Some(ParameterValue::Int(2))
    );
}

// ----------------------------------------------------------------------------
// Audio processing
// ----------------------------------------------------------------------------

#[test]
fn process_zero_input_stays_zero() {
    let mut plugin = DitherPlugin::new(2);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("noise_shaping"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("dither_type"), ParameterValue::Int(1))
        .unwrap();

    let mut buffer = vec![0.0f32; FRAMES * 2];
    let frames = plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
    assert!(buffer.iter().all(|&s| s == 0.0));
}

#[test]
fn process_round_only_is_deterministic() {
    let mut plugin = DitherPlugin::new(2);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("dither_type"), ParameterValue::Int(1))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("noise_shaping"),
            ParameterValue::Bool(false),
        )
        .unwrap();

    let dc = 0.12345f32;
    let mut buffer = vec![dc; FRAMES * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    // Rounded 16-bit quantization of a small DC value should be identical on every sample.
    let first = buffer[0];
    assert!(buffer.iter().all(|&s| (s - first).abs() < 1e-7));
    assert!(first.abs() <= dc.abs() + 1e-4);
}

#[test]
fn changing_bit_depth_changes_output_scale() {
    let mut plugin_16 = DitherPlugin::new(2);
    plugin_16.initialize(SR).unwrap();
    plugin_16
        .set_parameter(ParameterId::from("bit_depth"), ParameterValue::Int(0))
        .unwrap();
    plugin_16
        .set_parameter(ParameterId::from("dither_type"), ParameterValue::Int(1))
        .unwrap();
    plugin_16
        .set_parameter(
            ParameterId::from("noise_shaping"),
            ParameterValue::Bool(false),
        )
        .unwrap();

    let mut plugin_24 = DitherPlugin::new(2);
    plugin_24.initialize(SR).unwrap();
    plugin_24
        .set_parameter(ParameterId::from("bit_depth"), ParameterValue::Int(2))
        .unwrap();
    plugin_24
        .set_parameter(ParameterId::from("dither_type"), ParameterValue::Int(1))
        .unwrap();
    plugin_24
        .set_parameter(
            ParameterId::from("noise_shaping"),
            ParameterValue::Bool(false),
        )
        .unwrap();

    // Use a value that is not exactly representable at either bit depth.
    let dc = 0.12345f32;
    let mut buf_16 = vec![dc; FRAMES * 2];
    let mut buf_24 = vec![dc; FRAMES * 2];
    plugin_16
        .process_in_place(&mut buf_16, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    plugin_24
        .process_in_place(&mut buf_24, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    // 24-bit quantization should be much closer to the original DC than 16-bit.
    let err_16 = (buf_16[0] - dc).abs();
    let err_24 = (buf_24[0] - dc).abs();
    assert!(
        err_24 < err_16,
        "24-bit error {} should be smaller than 16-bit error {}",
        err_24,
        err_16
    );
}

// ----------------------------------------------------------------------------
// State transitions
// ----------------------------------------------------------------------------

#[test]
fn reset_clears_error_history() {
    let mut plugin = DitherPlugin::new(2);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("noise_shaping"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("dither_type"), ParameterValue::Int(1))
        .unwrap();

    let mut buffer = vec![0.5f32; FRAMES * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    plugin.reset();

    // After reset, processing zero should again produce exactly zero.
    let mut silent = vec![0.0f32; FRAMES * 2];
    plugin
        .process_in_place(&mut silent, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert!(silent.iter().all(|&s| s == 0.0));
}

#[test]
fn initialize_changes_sample_rate_without_error() {
    let mut plugin = DitherPlugin::new(2);
    plugin.initialize(44100).unwrap();
    plugin.initialize(96000).unwrap();

    let mut buffer = vec![0.1f32; FRAMES * 2];
    let frames = plugin
        .process_in_place(&mut buffer, &ProcessContext::new(96000, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
    assert!(buffer.iter().all(|s| s.is_finite()));
}

// ----------------------------------------------------------------------------
// Error paths visible through the public API
// ----------------------------------------------------------------------------

#[test]
fn set_unknown_parameter_fails() {
    let mut plugin = DitherPlugin::new(2);
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Int(1))
        .unwrap_err();
    assert!(err.contains("Invalid or unknown parameter") || err.contains("not_a_param"));
}

#[test]
fn set_parameter_with_wrong_type_is_rejected() {
    let mut plugin = DitherPlugin::new(2);
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(
            ParameterId::from("bit_depth"),
            ParameterValue::String("16".to_string()),
        )
        .unwrap_err();
    assert!(err.contains("Invalid or unknown parameter") || err.contains("bit_depth"));
}

#[test]
fn get_unknown_parameter_returns_none() {
    let plugin = DitherPlugin::new(2);
    assert!(
        plugin
            .get_parameter(&ParameterId::from("does_not_exist"))
            .is_none()
    );
}
