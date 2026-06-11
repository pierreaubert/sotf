//! Integration tests for sotf-plugin-declick exercising the public `InPlacePlugin` trait.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_declick::{DeclickPlugin, DeclickPluginParams};

const SR: u32 = 48000;

fn ctx(frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(SR, frames)
}

#[test]
fn info_is_reported() {
    let plugin = DeclickPlugin::new(2);
    let info = plugin.info();
    assert_eq!(info.name, "Declick");
    assert_eq!(info.version, "1.0.0");
    assert!(!info.description.is_empty());
}

#[test]
fn disabled_is_transparent() {
    let mut plugin = DeclickPlugin::new(1);
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();

    let mut buffer = vec![0.0f32, 0.25, 4.0, 0.25];
    let input = buffer.clone();
    assert_eq!(plugin.process_in_place(&mut buffer, &ctx(4)).unwrap(), 4);
    assert_eq!(buffer, input);
}

#[test]
fn enabled_processes_click() {
    let mut plugin = DeclickPlugin::new(1);
    plugin
        .set_parameter(ParameterId::from("sensitivity"), ParameterValue::Float(5.0))
        .unwrap();

    let mut buffer = vec![0.0f32; 10];
    for i in 0..100 {
        buffer.push((i as f32 * 0.1).sin() * 0.5);
    }
    let click_idx = buffer.len();
    buffer.push(2.0);
    buffer.extend([0.0f32; 10]);

    let frames = buffer.len();
    plugin.process_in_place(&mut buffer, &ctx(frames)).unwrap();
    assert!(buffer[click_idx] < 2.0, "click should be reduced");
}

#[test]
fn stereo_buffer_is_processed() {
    let mut plugin = DeclickPlugin::new(2);
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.0f32; 64 * 2];
    buffer[4] = 2.0;
    buffer[5] = -2.0;
    plugin.process_in_place(&mut buffer, &ctx(64)).unwrap();
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn enabled_roundtrip() {
    let mut plugin = DeclickPlugin::new(1);
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();
    let got = plugin
        .get_parameter(&ParameterId::from("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap();
    assert!(!got);
}

#[test]
fn sensitivity_roundtrip() {
    let mut plugin = DeclickPlugin::new(1);
    plugin
        .set_parameter(
            ParameterId::from("sensitivity"),
            ParameterValue::Float(42.0),
        )
        .unwrap();
    let got = plugin
        .get_parameter(&ParameterId::from("sensitivity"))
        .and_then(|v| v.as_float())
        .unwrap();
    assert!((got - 42.0).abs() < 1e-3);
}

#[test]
fn from_params_happy_path() {
    let params = DeclickPluginParams {
        enabled: true,
        sensitivity: 25.0,
    };
    let mut plugin = DeclickPlugin::from_params(2, params);
    assert_eq!(plugin.channels(), 2);
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.25f32; 32 * 2];
    plugin.process_in_place(&mut buffer, &ctx(32)).unwrap();
    assert!(buffer.iter().all(|s| s.is_finite()));
}

#[test]
fn reset_clears_suppressor_state() {
    let mut plugin = DeclickPlugin::new(1);
    plugin.initialize(SR).unwrap();

    let mut buffer = vec![0.8f32; 64];
    plugin.process_in_place(&mut buffer, &ctx(64)).unwrap();
    plugin.reset();

    let mut buffer2 = vec![0.8f32; 64];
    plugin.process_in_place(&mut buffer2, &ctx(64)).unwrap();
    assert!(buffer2.iter().all(|s| s.is_finite()));
}

#[test]
fn buffer_size_mismatch_returns_error() {
    let mut plugin = DeclickPlugin::new(1);
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .unwrap();

    let mut buffer = vec![0.0f32; 3];
    let err = plugin.process_in_place(&mut buffer, &ctx(4)).unwrap_err();
    assert!(
        err.contains("Buffer size mismatch"),
        "unexpected error: {err}"
    );
}

#[test]
fn unknown_parameter_errors() {
    let mut plugin = DeclickPlugin::new(1);
    let err = plugin
        .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter"), "unexpected error: {err}");
}

#[test]
fn latency_is_zero() {
    let plugin = DeclickPlugin::new(1);
    assert_eq!(plugin.latency_samples(), 0);
}
