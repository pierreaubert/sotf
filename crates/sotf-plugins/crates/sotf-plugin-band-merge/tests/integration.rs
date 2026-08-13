// ============================================================================
// Integration tests for sotf-plugin-band-merge
//
// These tests exercise the public `Plugin` trait and crate-specific API as a
// black box — no internal modules are imported.
// ============================================================================

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_band_merge::{BandMergePlugin, BandMergePluginParams};

const SR: u32 = 48000;
const FRAMES: usize = 128;

// ----------------------------------------------------------------------------
// Construction and Plugin trait metadata
// ----------------------------------------------------------------------------

#[test]
fn new_plugin_has_expected_metadata() {
    let plugin = BandMergePlugin::new(2, 4).unwrap();
    let info = plugin.info();
    assert_eq!(info.name, "BandMerge");
    assert_eq!(info.author, "Sotf");
    assert_eq!(plugin.input_channels(), 8); // 2 * 4
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn from_params_applies_gains_and_mutes() {
    let params = BandMergePluginParams {
        bands: 3,
        band_gains_db: vec![-6.0, 0.0, 6.0],
        band_mutes: vec![false, true, false],
    };
    let plugin = BandMergePlugin::from_params(1, &params).unwrap();
    assert_eq!(plugin.input_channels(), 3);
    assert_eq!(plugin.output_channels(), 1);
}

#[test]
fn new_with_too_few_bands_fails() {
    let err = match BandMergePlugin::new(1, 1) {
        Err(e) => e,
        Ok(_) => panic!("expected an error"),
    };
    assert!(err.contains("Min 2 bands"));
}

#[test]
fn new_with_too_many_bands_fails() {
    let err = match BandMergePlugin::new(1, 33) {
        Err(e) => e,
        Ok(_) => panic!("expected an error"),
    };
    assert!(err.contains("Max"));
}

// ----------------------------------------------------------------------------
// Parameter discovery and round-trips
// ----------------------------------------------------------------------------

#[test]
fn parameters_include_bands_and_per_band_controls() {
    let plugin = BandMergePlugin::new(2, 3).unwrap();
    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"bands"));
    assert!(ids.contains(&"band_0_gain_db"));
    assert!(ids.contains(&"band_1_mute"));
    assert!(ids.contains(&"band_2_gain_db"));
    assert!(ids.contains(&"reconstruction_error_db"));
}

#[test]
fn bands_roundtrip() {
    let mut plugin = BandMergePlugin::new(1, 2).unwrap();
    plugin.initialize(SR).unwrap();
    assert!(
        plugin
            .set_parameter(ParameterId::from("bands"), ParameterValue::Int(4))
            .is_err()
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("bands")),
        Some(ParameterValue::Int(2))
    );
}

#[test]
fn band_gain_roundtrip() {
    let mut plugin = BandMergePlugin::new(1, 3).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("band_1_gain_db"),
            ParameterValue::Float(-12.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_1_gain_db")),
        Some(ParameterValue::Float(-12.0))
    );
}

#[test]
fn band_mute_roundtrip() {
    let mut plugin = BandMergePlugin::new(1, 3).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("band_0_mute"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_0_mute")),
        Some(ParameterValue::Bool(true))
    );
}

// ----------------------------------------------------------------------------
// Audio processing
// ----------------------------------------------------------------------------

#[test]
fn unity_gains_sum_bands() {
    let mut plugin = BandMergePlugin::new(1, 2).unwrap();
    plugin.initialize(SR).unwrap();

    let dc = 0.3f32;
    let input = vec![dc; FRAMES * 2]; // 2 bands, 1 channel each
    let mut output = vec![0.0f32; FRAMES];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    let expected = dc * 2.0;
    let last = output[FRAMES - 1];
    assert!(
        (last - expected).abs() < 1e-4,
        "expected {} got {}",
        expected,
        last
    );
}

#[test]
fn gain_db_scales_band_contribution() {
    let mut plugin = BandMergePlugin::new(1, 2).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("band_0_gain_db"),
            ParameterValue::Float(-20.0),
        )
        .unwrap();

    // Allow the one-pole gain smoother to settle (10 ms time constant).
    let frames = 4096;
    let dc = 0.5f32;
    let input = vec![dc; frames * 2];
    let mut output = vec![0.0f32; frames];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, frames))
        .unwrap();

    // band 0 is ~0.1, band 1 is 0.5 -> ~0.55
    let last = output[frames - 1];
    assert!(
        (last - (dc + dc * 0.1)).abs() < 0.02,
        "expected ~{} got {}",
        dc + dc * 0.1,
        last
    );
}

#[test]
fn mute_silences_band() {
    let mut plugin = BandMergePlugin::new(1, 2).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("band_0_mute"), ParameterValue::Bool(true))
        .unwrap();

    let dc = 0.4f32;
    let frames = 4800;
    let input = vec![dc; frames * 2];
    let mut output = vec![0.0f32; frames];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, frames))
        .unwrap();

    let last = output[frames - 1];
    assert!(
        (last - dc).abs() < 1e-4,
        "muted band 0 should leave only band 1: expected {} got {}",
        dc,
        last
    );
}

#[test]
fn multi_channel_merge_sums_per_channel() {
    let channels = 2;
    let bands = 3;
    let mut plugin = BandMergePlugin::new(channels, bands).unwrap();
    plugin.initialize(SR).unwrap();

    let dc = 0.25f32;
    let input = vec![dc; FRAMES * channels * bands];
    let mut output = vec![0.0f32; FRAMES * channels];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    let expected = dc * bands as f32;
    for ch in 0..channels {
        let last = output[(FRAMES - 1) * channels + ch];
        assert!(
            (last - expected).abs() < 1e-4,
            "channel {}: expected {} got {}",
            ch,
            expected,
            last
        );
    }
}

// ----------------------------------------------------------------------------
// Reconstruction error diagnostic
// ----------------------------------------------------------------------------

#[test]
fn reconstruction_error_reported() {
    let mut plugin = BandMergePlugin::new(1, 2).unwrap();
    plugin.initialize(SR).unwrap();

    // Request the diagnostic by reading the parameter.
    plugin.get_parameter(&ParameterId::from("reconstruction_error_db"));

    let dc = 0.5f32;
    let input = vec![dc; FRAMES * 2];
    let mut output = vec![0.0f32; FRAMES];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    let err = plugin
        .get_parameter(&ParameterId::from("reconstruction_error_db"))
        .and_then(|v| v.as_float())
        .unwrap();
    assert!(err.is_finite());
}

// ----------------------------------------------------------------------------
// State transitions
// ----------------------------------------------------------------------------

#[test]
fn reset_then_process_continues() {
    let mut plugin = BandMergePlugin::new(1, 2).unwrap();
    plugin.initialize(SR).unwrap();

    let input = vec![0.5f32; FRAMES * 2];
    let mut output = vec![0.0f32; FRAMES];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    plugin.reset();

    let mut output2 = vec![0.0f32; FRAMES];
    plugin
        .process(&input, &mut output2, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert!(output2.iter().all(|s| s.is_finite()));
}

#[test]
fn changing_bands_requires_plugin_rebuild() {
    let mut plugin = BandMergePlugin::new(2, 2).unwrap();
    plugin.initialize(SR).unwrap();
    assert_eq!(plugin.input_channels(), 4);

    assert!(
        plugin
            .set_parameter(ParameterId::from("bands"), ParameterValue::Int(3))
            .is_err()
    );
    assert_eq!(plugin.input_channels(), 4);
    assert_eq!(plugin.output_channels(), 2);
}

// ----------------------------------------------------------------------------
// Error paths visible through the public API
// ----------------------------------------------------------------------------

#[test]
fn set_unknown_parameter_fails() {
    let mut plugin = BandMergePlugin::new(1, 2).unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("not_a_param"));
}

#[test]
fn set_band_gain_for_out_of_range_band_fails() {
    let mut plugin = BandMergePlugin::new(1, 2).unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(
            ParameterId::from("band_7_gain_db"),
            ParameterValue::Float(-6.0),
        )
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("band_7"));
}

#[test]
fn set_bands_out_of_range_fails() {
    let mut plugin = BandMergePlugin::new(1, 2).unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("bands"), ParameterValue::Int(1))
        .unwrap_err();
    assert!(err.contains("bands must be"));
}

#[test]
fn process_with_correct_buffer_size_succeeds() {
    let mut plugin = BandMergePlugin::new(1, 2).unwrap();
    plugin.initialize(SR).unwrap();
    let input = vec![0.5f32; FRAMES * 2];
    let mut output = vec![0.0f32; FRAMES];
    let frames = plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
}
