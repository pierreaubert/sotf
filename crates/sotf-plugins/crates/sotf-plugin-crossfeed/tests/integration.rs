// ============================================================================
// Integration tests for sotf-plugin-crossfeed
//
// Exercises the public InPlacePlugin API and crate-specific constructors as a
// black box.
// ============================================================================

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_crossfeed::{
    CrossfeedMode, CrossfeedPlugin, CrossfeedPluginParams, CrossfeedPreset,
};

const SR: u32 = 48000;
const FRAMES: usize = 256;

// ----------------------------------------------------------------------------
// Construction and metadata
// ----------------------------------------------------------------------------

#[test]
fn default_plugin_has_expected_metadata() {
    let plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    let info = plugin.info();
    assert_eq!(info.name, "Crossfeed");
    assert_eq!(info.author, "SotF");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(plugin.channels(), 2);
}

#[test]
fn serialized_state_rejects_unknown_fields() {
    assert!(
        serde_json::from_value::<CrossfeedPluginParams>(serde_json::json!({
            "unexpected": true
        }))
        .is_err()
    );
}

#[test]
fn from_preset_off_sets_mode_off() {
    let params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Off);
    assert_eq!(params.mode, CrossfeedMode::Off);
}

#[test]
fn from_preset_bauer_sets_bauer_mode() {
    let params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Default);
    assert_eq!(params.mode, CrossfeedMode::Bauer);
}

#[test]
fn from_preset_meier_sets_meier_mode() {
    let params = CrossfeedPluginParams::from_preset(CrossfeedPreset::Meier);
    assert_eq!(params.mode, CrossfeedMode::Meier);
}

// ----------------------------------------------------------------------------
// Parameter discovery and round-trips
// ----------------------------------------------------------------------------

#[test]
fn parameters_include_all_public_params() {
    let plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    let params = plugin.parameters();
    let ids: Vec<&str> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"crossfeed_mode"));
    assert!(ids.contains(&"crossfeed_preset"));
    assert!(ids.contains(&"enabled"));
    assert!(ids.contains(&"mix"));
    assert!(ids.contains(&"bauer_fcut_hz"));
    assert!(ids.contains(&"bauer_feed_db"));
    assert!(ids.contains(&"meier_level"));
    assert!(ids.contains(&"mb_low_freq_hz"));
    assert!(ids.contains(&"mb_mid_high_freq_hz"));
    assert!(ids.contains(&"mb_low_feed_db"));
    assert!(ids.contains(&"mb_mid_feed_db"));
    assert!(ids.contains(&"mb_high_feed_db"));
    assert!(ids.contains(&"itd_delay_ms"));
    assert!(ids.contains(&"autogain_enabled"));
    assert!(ids.contains(&"autogain_target_lufs"));
    assert!(ids.contains(&"autogain_max_gain_db"));
    assert!(ids.contains(&"autogain_smoothing_ms"));
    assert!(ids.contains(&"head_yaw_deg"));
}

#[test]
fn enabled_roundtrip() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("enabled")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn mix_roundtrip() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.5))
    );
}

#[test]
fn bauer_params_roundtrip() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("bauer_fcut_hz"),
            ParameterValue::Float(500.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("bauer_feed_db"),
            ParameterValue::Float(2.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("bauer_fcut_hz")),
        Some(ParameterValue::Float(500.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("bauer_feed_db")),
        Some(ParameterValue::Float(2.0))
    );
}

#[test]
fn meier_level_roundtrip() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("meier_level"),
            ParameterValue::Float(42.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("meier_level")),
        Some(ParameterValue::Float(42.0))
    );
}

#[test]
fn multiband_feed_roundtrip() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("mb_low_feed_db"),
            ParameterValue::Float(-3.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("mb_mid_feed_db"),
            ParameterValue::Float(4.0),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("mb_high_feed_db"),
            ParameterValue::Float(5.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mb_low_feed_db")),
        Some(ParameterValue::Float(-3.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mb_mid_feed_db")),
        Some(ParameterValue::Float(4.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mb_high_feed_db")),
        Some(ParameterValue::Float(5.0))
    );
}

#[test]
fn itd_delay_roundtrip() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("itd_delay_ms"),
            ParameterValue::Float(0.3),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("itd_delay_ms")),
        Some(ParameterValue::Float(0.3))
    );
}

#[test]
fn head_yaw_roundtrip() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("head_yaw_deg"),
            ParameterValue::Float(30.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("head_yaw_deg")),
        Some(ParameterValue::Float(30.0))
    );
}

#[test]
fn autogain_enable_roundtrip() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("autogain_enabled"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("autogain_enabled")),
        Some(ParameterValue::Bool(true))
    );
}

#[test]
fn yaw_out_of_range_is_clamped() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("head_yaw_deg"),
            ParameterValue::Float(120.0),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("head_yaw_deg")),
        Some(ParameterValue::Float(90.0))
    );
}

// ----------------------------------------------------------------------------
// Audio processing
// ----------------------------------------------------------------------------

#[test]
fn process_zero_input_produces_finite_output() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.0f32; FRAMES * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn disabled_plugin_passthrough() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();

    let dc_l = 0.3f32;
    let dc_r = 0.7f32;
    let mut buffer: Vec<f32> = (0..FRAMES * 2)
        .map(|i| if i % 2 == 0 { dc_l } else { dc_r })
        .collect();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    assert!((buffer[(FRAMES - 1) * 2] - dc_l).abs() < 1e-5);
    assert!((buffer[(FRAMES - 1) * 2 + 1] - dc_r).abs() < 1e-5);
}

#[test]
fn mode_off_passthrough() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("crossfeed_mode"), ParameterValue::Int(0))
        .unwrap();

    let dc_l = 0.3f32;
    let dc_r = 0.7f32;
    let mut buffer: Vec<f32> = (0..FRAMES * 2)
        .map(|i| if i % 2 == 0 { dc_l } else { dc_r })
        .collect();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    assert!((buffer[(FRAMES - 1) * 2] - dc_l).abs() < 1e-5);
    assert!((buffer[(FRAMES - 1) * 2 + 1] - dc_r).abs() < 1e-5);
}

#[test]
fn bauer_mode_changes_stereo_signal() {
    let mut plugin =
        CrossfeedPlugin::new(CrossfeedPluginParams::from_preset(CrossfeedPreset::Default)).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();

    // Hard-panned left signal.
    let mut buffer: Vec<f32> = (0..FRAMES * 2)
        .map(|i| if i % 2 == 0 { 0.5 } else { 0.0 })
        .collect();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    // The right channel should now contain some crossfed signal from the left.
    assert!(
        buffer[(FRAMES - 1) * 2 + 1].abs() > 1e-4,
        "Bauer crossfeed should leak into the opposite channel"
    );
}

#[test]
fn meier_mode_changes_stereo_signal() {
    let mut plugin =
        CrossfeedPlugin::new(CrossfeedPluginParams::from_preset(CrossfeedPreset::Meier)).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();

    let mut buffer: Vec<f32> = (0..FRAMES * 2)
        .map(|i| if i % 2 == 0 { 0.5 } else { 0.0 })
        .collect();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    assert!(
        buffer[(FRAMES - 1) * 2 + 1].abs() > 1e-4,
        "Meier crossfeed should leak into the opposite channel"
    );
}

#[test]
fn multiband_mode_changes_stereo_signal() {
    let mut plugin =
        CrossfeedPlugin::new(CrossfeedPluginParams::from_preset(CrossfeedPreset::Mb)).unwrap();
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();

    let mut buffer: Vec<f32> = (0..FRAMES * 2)
        .map(|i| if i % 2 == 0 { 0.5 } else { 0.0 })
        .collect();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    assert!(
        buffer[(FRAMES - 1) * 2 + 1].abs() > 1e-4,
        "Multiband crossfeed should leak into the opposite channel"
    );
}

#[test]
fn mode_transition_resets_and_continues() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.3f32; FRAMES * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    plugin
        .set_parameter(ParameterId::from("crossfeed_mode"), ParameterValue::Int(2))
        .unwrap();
    plugin.reset();

    let mut buffer2 = vec![0.3f32; FRAMES * 2];
    plugin
        .process_in_place(&mut buffer2, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert!(buffer2.iter().all(|s| s.is_finite()));
}

// ----------------------------------------------------------------------------
// State transitions
// ----------------------------------------------------------------------------

#[test]
fn reset_then_process_continues() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.3f32; FRAMES * 2];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(SR, FRAMES))
        .unwrap();

    plugin.reset();

    let mut buffer2 = vec![0.3f32; FRAMES * 2];
    plugin
        .process_in_place(&mut buffer2, &ProcessContext::new(SR, FRAMES))
        .unwrap();
    assert!(buffer2.iter().all(|s| s.is_finite()));
}

#[test]
fn initialize_changes_sample_rate() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(44100).unwrap();
    plugin.initialize(96000).unwrap();

    let mut buffer = vec![0.3f32; FRAMES * 2];
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
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter") || err.contains("not_a_param"));
}

#[test]
fn set_head_yaw_with_non_float_fails() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(SR).unwrap();
    let err = plugin
        .set_parameter(ParameterId::from("head_yaw_deg"), ParameterValue::Int(45))
        .unwrap_err();
    assert!(err.contains("head_yaw_deg") || err.contains("float"));
}

#[test]
fn get_unknown_parameter_returns_none() {
    let plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    assert!(
        plugin
            .get_parameter(&ParameterId::from("does_not_exist"))
            .is_none()
    );
}
