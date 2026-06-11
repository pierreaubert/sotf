// ============================================================================
// Integration tests for sotf-plugin-band-split
//
// These tests exercise the public `Plugin` trait and crate-specific API as a
// black box — no internal modules are imported.
// ============================================================================

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_band_split::BandSplitPlugin;

const SR: u32 = 48000;
const FRAMES: usize = 256;

// ----------------------------------------------------------------------------
// Construction and Plugin trait metadata
// ----------------------------------------------------------------------------

#[test]
fn new_two_band_plugin_has_expected_metadata() {
    let plugin = BandSplitPlugin::new(2, 1000.0, "LR24").unwrap();
    let info = plugin.info();
    assert_eq!(info.name, "BandSplit");
    assert_eq!(info.author, "Sotf");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 4); // 2 in * 2 bands
}

#[test]
fn new_multiband_plugin_has_expected_channel_counts() {
    let plugin = BandSplitPlugin::new_multiband(1, &[250.0, 2000.0], "LR48").unwrap();
    assert_eq!(plugin.input_channels(), 1);
    assert_eq!(plugin.output_channels(), 3); // 1 in * 3 bands
}

#[test]
fn new_with_no_frequencies_fails() {
    let err = match BandSplitPlugin::new_multiband(1, &[], "LR24") {
        Err(e) => e,
        Ok(_) => panic!("expected an error"),
    };
    assert!(err.contains("At least one crossover frequency"));
}

#[test]
fn new_with_too_many_frequencies_fails() {
    let err = match BandSplitPlugin::new_multiband(1, &[100.0, 500.0, 1000.0, 4000.0], "LR24") {
        Err(e) => e,
        Ok(_) => panic!("expected an error"),
    };
    assert!(err.contains("Too many bands") || err.contains("max"));
}

// ----------------------------------------------------------------------------
// Parameter discovery and round-trips
// ----------------------------------------------------------------------------

#[test]
fn parameters_include_frequency_and_gains() {
    let plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"frequency"));
    assert!(ids.contains(&"crossover_type"));
    assert!(ids.contains(&"band_0_gain_db"));
    assert!(ids.contains(&"band_1_gain_db"));
}

#[test]
fn multiband_parameters_include_additional_frequencies() {
    let plugin = BandSplitPlugin::new_multiband(1, &[250.0, 2000.0], "LR24").unwrap();
    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"frequency"));
    assert!(ids.contains(&"frequency_2"));
    assert!(ids.contains(&"band_2_gain_db"));
}

#[test]
fn frequency_roundtrip() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("frequency"), ParameterValue::Float(500.0))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("frequency")),
        Some(ParameterValue::Float(500.0))
    );
}

#[test]
fn crossover_type_roundtrip() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("crossover_type"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("crossover_type")),
        Some(ParameterValue::Int(1))
    );
}

#[test]
fn band_gain_roundtrip() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("band_1_gain_db"),
            ParameterValue::Float(-6.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_1_gain_db")),
        Some(ParameterValue::Float(-6.0))
    );
}

#[test]
fn multiband_frequency_2_roundtrip() {
    let mut plugin = BandSplitPlugin::new_multiband(1, &[250.0, 2000.0], "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("frequency_2"),
            ParameterValue::Float(1500.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("frequency_2")),
        Some(ParameterValue::Float(1500.0))
    );
}

// ----------------------------------------------------------------------------
// Audio processing
// ----------------------------------------------------------------------------

#[test]
fn process_zero_input_produces_finite_output() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();

    let input = vec![0.0f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn dc_reconstructs_approximately() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();

    let dc = 0.5f32;
    let input = vec![dc; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    let low = output[(FRAMES - 1) * 2];
    let high = output[(FRAMES - 1) * 2 + 1];
    let sum = low + high;
    assert!(
        (sum - dc).abs() < 0.05,
        "split bands should reconstruct DC: got {} (low={}, high={})",
        sum,
        low,
        high
    );
}

#[test]
fn multiband_dc_reconstructs_approximately() {
    let mut plugin = BandSplitPlugin::new_multiband(1, &[250.0, 2000.0], "LR24").unwrap();
    plugin.initialize(SR).unwrap();

    let dc = 0.5f32;
    let input = vec![dc; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 3];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    let mut sum = 0.0f32;
    for band in 0..3 {
        sum += output[(FRAMES - 1) * 3 + band];
    }
    assert!(
        (sum - dc).abs() < 0.05,
        "3-band split should reconstruct DC: got {} expected {}",
        sum,
        dc
    );
}

#[test]
fn band_gain_attenuates_band() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();

    let frames = 4096;
    let dc = 0.5f32;
    let input = vec![dc; frames];
    let mut output_ref = vec![0.0f32; frames * 2];
    plugin
        .process(&input, &mut output_ref, &ProcessContext::new(SR, frames))
        .unwrap();

    plugin
        .set_parameter(
            ParameterId::from("band_0_gain_db"),
            ParameterValue::Float(-60.0),
        )
        .unwrap();

    let mut output_gain = vec![0.0f32; frames * 2];
    plugin
        .process(&input, &mut output_gain, &ProcessContext::new(SR, frames))
        .unwrap();

    let low_ref = output_ref[(frames - 1) * 2].abs();
    let low_gain = output_gain[(frames - 1) * 2].abs();
    assert!(
        low_gain < low_ref * 0.1,
        "band 0 should be strongly attenuated: ref={} gained={}",
        low_ref,
        low_gain
    );
}

// ----------------------------------------------------------------------------
// State transitions
// ----------------------------------------------------------------------------

#[test]
fn reset_then_process_continues() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
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
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(44100).unwrap();
    plugin.initialize(96000).unwrap();

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
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("not_a_param"));
}

#[test]
fn set_band_gain_for_out_of_range_band_fails() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
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
fn set_frequency_with_non_numeric_type_fails() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(
            ParameterId::from("frequency"),
            ParameterValue::String("five hundred".to_string()),
        )
        .unwrap_err();
    assert!(err.contains("frequency") || err.contains("type mismatch"));
}

#[test]
fn process_with_correct_output_size_succeeds() {
    let mut plugin = BandSplitPlugin::new(1, 1000.0, "LR24").unwrap();
    plugin.initialize(SR).unwrap();
    let input = vec![0.5f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES * 2];
    let frames = plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert_eq!(frames, FRAMES);
}
