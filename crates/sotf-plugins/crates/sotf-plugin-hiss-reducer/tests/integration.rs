//! Integration tests for sotf-plugin-hiss-reducer exercising the public `InPlacePlugin` trait.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
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
    assert_eq!(info.version, "1.0.0");
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

    // High-frequency noise above the default 4 kHz cutoff.
    let mut buffer: Vec<f32> = (0..256).map(|i| (i as f32 * 0.5).sin() * 0.5).collect();
    let input = buffer.clone();
    plugin.process_in_place(&mut buffer, &ctx(256)).unwrap();
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
    uninit.process_in_place(&mut buf_uninit, &ctx(8)).unwrap();

    let mut init = HissReducerPlugin::new(1);
    init.initialize(SR).unwrap();
    let mut buf_init = vec![0.5f32; 8];
    init.process_in_place(&mut buf_init, &ctx(8)).unwrap();

    assert_eq!(buf_uninit, buf_init);
}
