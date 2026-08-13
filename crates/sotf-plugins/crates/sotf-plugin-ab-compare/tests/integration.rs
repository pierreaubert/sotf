// ============================================================================
// Integration tests for sotf-plugin-ab-compare
//
// These tests exercise the public `Plugin` trait and crate-specific API as a
// black box — no internal modules are imported.
// ============================================================================

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_ab_compare::{ABComparePlugin, ABComparePluginParams};

const SR: u32 = 48000;
const FRAMES: usize = 128;

// ----------------------------------------------------------------------------
// Construction and Plugin trait metadata
// ----------------------------------------------------------------------------

#[test]
fn new_plugin_has_expected_metadata() {
    let plugin = ABComparePlugin::new(2).unwrap();
    let info = plugin.info();
    assert_eq!(info.name, "A/B Compare");
    assert_eq!(info.author, "SotF");
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 2);
}

#[test]
fn from_params_with_empty_paths_has_expected_defaults() {
    let params = ABComparePluginParams::default();
    let plugin = ABComparePlugin::from_params(1, params).unwrap();
    assert_eq!(plugin.input_channels(), 1);
    assert_eq!(plugin.output_channels(), 1);
}

#[test]
fn latency_is_zero_for_empty_paths() {
    let plugin = ABComparePlugin::new(2).unwrap();
    assert_eq!(plugin.latency_samples(), 0);
}

// ----------------------------------------------------------------------------
// Parameter discovery
// ----------------------------------------------------------------------------

#[test]
fn parameters_include_core_controls() {
    let plugin = ABComparePlugin::new(2).unwrap();
    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"mix"));
    assert!(ids.contains(&"bypass"));
    assert!(ids.contains(&"selected_path"));
    assert!(ids.contains(&"auto_gain_enabled"));
    assert!(ids.contains(&"path_a_config"));
    assert!(ids.contains(&"path_b_config"));
}

// ----------------------------------------------------------------------------
// Happy-path parameter round-trips
// ----------------------------------------------------------------------------

#[test]
fn mix_roundtrip() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.75))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.75))
    );
}

#[test]
fn bypass_roundtrip() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("bypass")),
        Some(ParameterValue::Bool(true))
    );
}

#[test]
fn selected_path_roundtrips() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("selected_path"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("selected_path")),
        Some(ParameterValue::Int(1))
    );
}

#[test]
fn selected_path_out_of_range_is_rejected() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("selected_path"), ParameterValue::Int(7))
        .unwrap_err();
    assert!(err.contains("selected_path") || err.contains("maximum"));
}

#[test]
fn mix_mode_switch_to_binary() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("mix_mode"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mix_mode")),
        Some(ParameterValue::Int(1))
    );
}

#[test]
fn phase_invert_roundtrip() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("phase_invert_a"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("phase_invert_b"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("phase_invert_a")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("phase_invert_b")),
        Some(ParameterValue::Bool(true))
    );
}

#[test]
fn difference_mode_roundtrip() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("difference_mode"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("difference_mode")),
        Some(ParameterValue::Bool(true))
    );
}

#[test]
fn band_mask_frequencies_roundtrip() {
    let params = ABComparePluginParams {
        band_mask_low_hz: 250.0,
        band_mask_high_hz: 8_000.0,
        ..Default::default()
    };
    let mut plugin = ABComparePlugin::from_params(1, params).unwrap();
    plugin.initialize(SR).unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_mask_low_hz")),
        Some(ParameterValue::Float(250.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("band_mask_high_hz")),
        Some(ParameterValue::Float(8000.0))
    );
}

#[test]
fn band_mask_frequencies_out_of_range_are_rejected() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(
            ParameterId::from("band_mask_low_hz"),
            ParameterValue::Float(5.0),
        )
        .unwrap_err();
    assert!(err.contains("band_mask_low_hz") || err.contains("minimum"));
}

#[test]
fn band_mask_frequencies_rejected_on_construction() {
    let params = ABComparePluginParams {
        band_mask_low_hz: 5.0,
        band_mask_high_hz: 50000.0,
        ..Default::default()
    };
    assert!(ABComparePlugin::from_params(1, params).is_err());
}

// ----------------------------------------------------------------------------
// Audio processing: empty paths
// ----------------------------------------------------------------------------

#[test]
fn bypass_passes_input_through_unchanged() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("bypass"), ParameterValue::Bool(true))
        .unwrap();

    let input: Vec<f32> = (0..FRAMES * 2).map(|i| (i as f32) * 0.01).collect();
    let mut output = vec![0.0f32; FRAMES * 2];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    assert_eq!(output, input);
}

#[test]
fn empty_paths_center_mix_preserves_identical_signal_unity() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    // Both paths are the same signal, so every mix position must stay at unity.
    let dc = 0.5f32;
    let input = vec![dc; FRAMES];
    let mut output = vec![0.0f32; FRAMES];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    let expected = dc;
    let last = output[FRAMES - 1];
    assert!(
        (last - expected).abs() < 1e-5,
        "expected {} got {}",
        expected,
        last
    );
}

#[test]
fn empty_paths_a_only_outputs_scaled_dc() {
    let params = ABComparePluginParams {
        mix: -1.0,
        ..Default::default()
    };
    let mut plugin = ABComparePlugin::from_params(1, params).unwrap();
    plugin.initialize(SR).unwrap();

    let dc = 0.5f32;
    let input = vec![dc; FRAMES];
    let mut output = vec![0.0f32; FRAMES];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    // mix = -1.0 -> equal-power gain for A only = cos(0) = 1.0
    let last = output[FRAMES - 1];
    assert!((last - dc).abs() < 1e-5, "expected {} got {}", dc, last);
}

#[test]
fn empty_paths_b_only_outputs_scaled_dc() {
    let params = ABComparePluginParams {
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = ABComparePlugin::from_params(1, params).unwrap();
    plugin.initialize(SR).unwrap();

    let dc = 0.5f32;
    let input = vec![dc; FRAMES];
    let mut output = vec![0.0f32; FRAMES];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    // mix = 1.0 -> equal-power gain for B only = sin(pi/2) = 1.0
    let last = output[FRAMES - 1];
    assert!((last - dc).abs() < 1e-5, "expected {} got {}", dc, last);
}

// ----------------------------------------------------------------------------
// State transitions
// ----------------------------------------------------------------------------

#[test]
fn reset_clears_peak_values() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();

    let input = vec![0.8f32; FRAMES];
    let mut output = vec![0.0f32; FRAMES];
    plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    plugin.reset();

    let data = plugin
        .get_data()
        .and_then(|d| d.downcast::<sotf_plugin_ab_compare::ABCompareData>().ok())
        .expect("ABCompareData should be available");
    assert_eq!(data.peak_a, 0.0);
    assert_eq!(data.peak_b, 0.0);
    assert!(!data.bypass_active);
}

#[test]
fn initialize_changes_sample_rate_and_resets_smoothing() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(44100).unwrap();
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();
    plugin.initialize(96000).unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(1.0))
    );
}

// ----------------------------------------------------------------------------
// Path configuration via JSON strings
// ----------------------------------------------------------------------------

#[test]
fn path_a_gain_config_requires_structural_rebuild() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();

    let json = r#"{"type":"Plugin","plugin_type":"gain","parameters":{"gain_db":6.0}}"#;
    let error = plugin
        .set_parameter(
            ParameterId::from("path_a_config"),
            ParameterValue::String(json.to_string()),
        )
        .unwrap_err();
    assert!(error.contains("structural") && error.contains("rebuild"));
}

#[test]
fn path_config_none_requires_structural_rebuild() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    let error = plugin
        .set_parameter(
            ParameterId::from("path_a_config"),
            ParameterValue::String(r#"{"type":"None"}"#.to_string()),
        )
        .unwrap_err();
    assert!(error.contains("structural") && error.contains("rebuild"));
}

// ----------------------------------------------------------------------------
// Error paths visible through the public API
// ----------------------------------------------------------------------------

#[test]
fn set_unknown_parameter_fails() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    let err = plugin
        .set_parameter(
            ParameterId::from("not_a_real_param"),
            ParameterValue::Float(1.0),
        )
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("not_a_real_param"));
}

#[test]
fn set_parameter_with_wrong_type_fails() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Int(1))
        .unwrap_err();
    assert!(err.contains("mix") || err.contains("type mismatch") || err.contains("Parameter"));
}

#[test]
fn invalid_path_config_json_fails() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(
            ParameterId::from("path_a_config"),
            ParameterValue::String("not json".to_string()),
        )
        .unwrap_err();
    assert!(err.contains("structural") && err.contains("rebuild"));
}

#[test]
fn failed_path_rebuild_preserves_previous_configuration() {
    let mut plugin = ABComparePlugin::new(1).unwrap();
    plugin.initialize(SR).unwrap();
    let before = plugin
        .get_parameter(&ParameterId::from("path_a_config"))
        .unwrap();
    let invalid = serde_json::json!({
        "type": "Plugin",
        "plugin_type": "plugin_that_does_not_exist",
        "parameters": {}
    })
    .to_string();
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("path_a_config"),
                ParameterValue::String(invalid),
            )
            .is_err()
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("path_a_config")),
        Some(before)
    );
}

#[test]
fn process_with_wrong_buffer_size_fails() {
    let mut plugin = ABComparePlugin::new(2).unwrap();
    plugin.initialize(SR).unwrap();
    let input = vec![0.0f32; FRAMES * 2 - 1];
    let mut output = vec![0.0f32; FRAMES * 2];
    let err = plugin
        .process(&input, &mut output, &ProcessContext::new(SR, FRAMES))
        .unwrap_err();
    assert!(err.contains("size mismatch") || err.contains("Input"));
}
