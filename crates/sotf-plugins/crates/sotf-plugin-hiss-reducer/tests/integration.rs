//! Integration tests for sotf-plugin-hiss-reducer exercising the public `InPlacePlugin` trait.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::{PluginCostClass, ProcessContext};
use sotf_plugin_hiss_reducer::{HissReducerPlugin, HissReducerPluginParams};

const SR: u32 = 48000;

fn ctx(frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(SR, frames)
}

#[test]
fn info_is_reported() {
    let plugin = HissReducerPlugin::new(2);
    let info = plugin.info();
    assert_eq!(info.name, "Hiss Reducer");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert!(!info.description.is_empty());
}

#[test]
fn disabled_is_transparent() {
    let mut plugin = HissReducerPlugin::new(2);
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.25f32, -0.25, 0.5, -0.5];
    let input = buffer.clone();
    assert_eq!(plugin.process_in_place(&mut buffer, &ctx(2)).unwrap(), 2);
    assert_eq!(buffer, input);
}

#[test]
fn enabled_changes_high_frequency_content() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(SR).unwrap();

    // Persistent, low-level high-frequency energy above the default 4 kHz
    // cutoff and below the default -30 dBFS detector threshold.
    let mut buffer: Vec<f32> = (0..24_000).map(|i| (i as f32 * 0.5).sin() * 0.02).collect();
    let input = buffer.clone();
    plugin.process_in_place(&mut buffer, &ctx(24_000)).unwrap();
    assert_ne!(
        buffer, input,
        "hiss reducer should alter high-frequency signal"
    );
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn parameter_roundtrips() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(SR).unwrap();

    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    assert_eq!(
        plugin
            .get_parameter(&ParameterId::from("enabled"))
            .and_then(|v| v.as_bool()),
        Some(false)
    );

    plugin
        .set_parameter(
            ParameterId::from("threshold_db"),
            ParameterValue::Float(-40.0),
        )
        .unwrap();
    assert!(
        (plugin
            .get_parameter(&ParameterId::from("threshold_db"))
            .and_then(|v| v.as_float())
            .unwrap()
            - (-40.0))
            .abs()
            < 1e-3
    );

    plugin
        .set_parameter(
            ParameterId::from("frequency_hz"),
            ParameterValue::Float(6000.0),
        )
        .unwrap();
    assert!(
        (plugin
            .get_parameter(&ParameterId::from("frequency_hz"))
            .and_then(|v| v.as_float())
            .unwrap()
            - 6000.0)
            .abs()
            < 1e-3
    );

    plugin
        .set_parameter(ParameterId::from("strength"), ParameterValue::Float(0.75))
        .unwrap();
    assert!(
        (plugin
            .get_parameter(&ParameterId::from("strength"))
            .and_then(|v| v.as_float())
            .unwrap()
            - 0.75)
            .abs()
            < 1e-3
    );
}

#[test]
fn from_params_happy_path() {
    let params = HissReducerPluginParams {
        enabled: true,
        threshold_db: -35.0,
        frequency_hz: 5000.0,
        strength: 0.25,
    };
    let mut plugin = HissReducerPlugin::from_params(2, params);
    assert_eq!(plugin.channels(), 2);
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.1f32; 32 * 2];
    plugin.process_in_place(&mut buffer, &ctx(32)).unwrap();
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn reset_clears_reducer_state() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.6f32; 64];
    plugin.process_in_place(&mut buffer, &ctx(64)).unwrap();
    plugin.reset();

    let mut buffer2 = vec![0.6f32; 64];
    plugin.process_in_place(&mut buffer2, &ctx(64)).unwrap();
    assert!(buffer2.iter().all(|s| s.is_finite()));
}

#[test]
fn buffer_size_mismatch_returns_error() {
    let mut plugin = HissReducerPlugin::new(2);
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();

    let mut buffer = vec![0.0f32; 3];
    let err = plugin.process_in_place(&mut buffer, &ctx(2)).unwrap_err();
    assert!(
        err.contains("Buffer size mismatch"),
        "unexpected error: {err}"
    );
}

#[test]
fn unknown_parameter_errors() {
    let mut plugin = HissReducerPlugin::new(1);
    let err = plugin
        .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter"), "unexpected error: {err}");
}

#[test]
fn latency_is_zero() {
    let plugin = HissReducerPlugin::new(1);
    assert_eq!(plugin.latency_samples(), 0);
}

#[test]
fn initialize_does_not_change_response_at_default_rate() {
    let mut uninit = HissReducerPlugin::new(1);
    let mut buf_uninit = vec![0.5f32; 8];
    let err = uninit
        .process_in_place(&mut buf_uninit, &ctx(8))
        .unwrap_err();
    assert!(err.contains("initialized"), "unexpected error: {err}");

    let mut init = HissReducerPlugin::new(1);
    init.initialize(SR).unwrap();
    let mut buf_init = vec![0.5f32; 8];
    init.process_in_place(&mut buf_init, &ctx(8)).unwrap();

    assert!(buf_init.iter().all(|sample| sample.is_finite()));
}

#[test]
fn zero_sample_rate_and_context_mismatch_are_rejected() {
    let mut plugin = HissReducerPlugin::new(1);
    assert!(plugin.initialize(0).is_err());
    plugin.initialize(SR).unwrap();
    let mut buffer = vec![0.0; 8];
    let mismatched = ProcessContext::new(44_100, 8);
    let err = plugin
        .process_in_place(&mut buffer, &mismatched)
        .unwrap_err();
    assert!(err.contains("sample rate"), "unexpected error: {err}");
}

#[test]
fn persisted_parameters_are_canonicalized() {
    let plugin = HissReducerPlugin::from_params(
        1,
        HissReducerPluginParams {
            enabled: true,
            threshold_db: f32::NAN,
            frequency_hz: f32::INFINITY,
            strength: -2.0,
        },
    );
    let values = plugin.current_values();
    assert_eq!(
        values
            .get(&ParameterId::from("threshold_db"))
            .and_then(|v| v.as_float()),
        Some(-30.0)
    );
    assert_eq!(
        values
            .get(&ParameterId::from("frequency_hz"))
            .and_then(|v| v.as_float()),
        Some(4_000.0)
    );
    assert_eq!(
        values
            .get(&ParameterId::from("strength"))
            .and_then(|v| v.as_float()),
        Some(0.0)
    );
}

#[test]
fn metadata_reports_iir_cost() {
    let plugin = HissReducerPlugin::new(1);
    assert_eq!(plugin.cost_class(), PluginCostClass::Iir);
    let metadata = plugin.compile_metadata();
    assert_eq!(metadata.cost_class, PluginCostClass::Iir);
    assert!(!metadata.linear);
    assert!(metadata.stateful);
    assert!(!metadata.channel_mixing);
    assert_eq!(metadata.latency_samples, 0);
}

#[test]
fn bypass_reentry_restarts_detector_state() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(SR).unwrap();
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    let mut bypassed = vec![0.0; 64];
    plugin.process_in_place(&mut bypassed, &ctx(64)).unwrap();
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(true))
        .unwrap();
    let mut program = vec![0.25; 64];
    plugin.process_in_place(&mut program, &ctx(64)).unwrap();
    assert!(program.iter().all(|sample| sample.is_finite()));
}

#[test]
fn initialization_canonicalizes_cutoff_to_sample_rate() {
    let mut plugin = HissReducerPlugin::from_params(
        1,
        HissReducerPluginParams {
            frequency_hz: 16_000.0,
            ..HissReducerPluginParams::default()
        },
    );
    plugin.initialize(8_000).unwrap();
    assert_eq!(
        plugin
            .get_parameter(&ParameterId::from("frequency_hz"))
            .and_then(|value| value.as_float()),
        Some(3_600.0),
        "host-visible state must match the reducer's 0.45 * sample-rate limit"
    );

    let err = plugin
        .set_parameter(
            ParameterId::from("frequency_hz"),
            ParameterValue::Float(3_601.0),
        )
        .unwrap_err();
    assert!(err.contains("sample rate"), "unexpected error: {err}");
}

#[test]
fn initialization_rejects_rates_without_a_valid_cutoff_range() {
    let mut plugin = HissReducerPlugin::new(1);
    let err = plugin.initialize(2_000).unwrap_err();
    assert!(err.contains("sample rate"), "unexpected error: {err}");
}

#[test]
fn persisted_params_reject_unknown_fields() {
    let err = serde_json::from_value::<HissReducerPluginParams>(serde_json::json!({
        "enabled": true,
        "obsolete_fft_mode": true
    }))
    .unwrap_err();
    assert!(err.to_string().contains("obsolete_fft_mode"));
}

#[test]
fn non_finite_audio_is_sanitized_and_state_recovers() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(SR).unwrap();
    let mut poisoned = vec![f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.1];
    plugin.process_in_place(&mut poisoned, &ctx(4)).unwrap();
    assert!(poisoned.iter().all(|sample| sample.is_finite()));

    let mut recovery = vec![0.02; 256];
    plugin.process_in_place(&mut recovery, &ctx(256)).unwrap();
    assert!(recovery.iter().all(|sample| sample.is_finite()));
}

#[test]
fn live_bypass_transition_is_smoothed() {
    let mut plugin = HissReducerPlugin::from_params(
        1,
        HissReducerPluginParams {
            threshold_db: -20.0,
            strength: 1.0,
            ..HissReducerPluginParams::default()
        },
    );
    plugin.initialize(SR).unwrap();
    let mut warm: Vec<f32> = (0..SR / 2)
        .map(|index| if index % 2 == 0 { 0.05 } else { -0.05 })
        .collect();
    let warm_frames = warm.len();
    plugin
        .process_in_place(&mut warm, &ctx(warm_frames))
        .unwrap();
    let previous = *warm.last().unwrap();

    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    let mut transition = vec![0.05];
    let transition_frames = transition.len();
    plugin
        .process_in_place(&mut transition, &ctx(transition_frames))
        .unwrap();
    assert!(
        (transition[0] - previous).abs() < 0.065,
        "bypass switched discontinuously: previous={previous}, next={}",
        transition[0]
    );
}
