//! Integration tests for sotf-plugin-delay.
//!
//! These tests exercise the public `InPlacePlugin` API as a black box:
//! construction, initialization, parameter get/set, audio processing, bypass,
//! reset, and error paths.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_delay::{DelayPlugin, DelayPluginParams};

const SR: u32 = 48000;

fn ctx(frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(SR, frames)
}

fn impulse(channels: usize, frame: usize) -> Vec<f32> {
    let mut buf = vec![0.0f32; channels * 1024];
    for ch in 0..channels {
        buf[frame * channels + ch] = 1.0;
    }
    buf
}

fn rms(buf: &[f32]) -> f32 {
    let sum: f32 = buf.iter().map(|x| x * x).sum();
    (sum / buf.len().max(1) as f32).sqrt()
}

#[test]
fn instantiate_and_declare_metadata() {
    let plugin = DelayPlugin::new(2, 100.0, 0.3, 0.5);
    assert_eq!(plugin.info().name, "Delay");
    assert_eq!(plugin.channels(), 2);
    assert!(!plugin.is_per_channel());

    let params = plugin.parameters();
    let ids: Vec<_> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"delay_ms"));
    assert!(ids.contains(&"feedback"));
    assert!(ids.contains(&"mix"));
}

#[test]
fn instantiate_per_channel() {
    let plugin = DelayPlugin::new_per_channel(vec![10.0, 20.0, 30.0]).unwrap();
    assert_eq!(plugin.channels(), 3);
    assert!(plugin.is_per_channel());
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("delay_ms_1")),
        Some(ParameterValue::Float(20.0))
    );
}

#[test]
fn instantiate_per_channel_empty_rejected() {
    let err = match DelayPlugin::new_per_channel(vec![]) {
        Err(e) => e,
        Ok(_) => panic!("expected empty per-channel delay to fail"),
    };
    assert!(err.contains("empty"), "unexpected error: {err}");
}

#[test]
fn from_params_channels_mismatch_rejected() {
    let result = DelayPlugin::from_params(
        2,
        DelayPluginParams {
            delay_ms: 0.0,
            feedback: 0.0,
            mix: 1.0,
            lfo_rate_hz: 0.0,
            lfo_depth_ms: 0.0,
            allpass_feedback: false,
            allpass_coeff: 0.5,
            channel_delays_ms: vec![10.0, 20.0, 30.0],
        },
    );
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("expected channel mismatch to fail"),
    };
    assert!(err.contains("does not match"), "unexpected error: {err}");
}

#[test]
fn initialize_and_reset() {
    let mut plugin = DelayPlugin::new(2, 50.0, 0.0, 1.0);
    plugin.initialize(SR).unwrap();
    plugin.reset();
    let mut buf = vec![0.0f32; 128];
    plugin.process_in_place(&mut buf, &ctx(64)).unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn parameter_roundtrip_scalar() {
    let mut plugin = DelayPlugin::new(1, 100.0, 0.3, 0.5);
    plugin.initialize(SR).unwrap();

    let cases: Vec<(ParameterId, ParameterValue)> = vec![
        (ParameterId::from("delay_ms"), ParameterValue::Float(250.0)),
        (ParameterId::from("feedback"), ParameterValue::Float(0.5)),
        (ParameterId::from("mix"), ParameterValue::Float(0.75)),
        (ParameterId::from("lfo_rate_hz"), ParameterValue::Float(2.0)),
        (
            ParameterId::from("lfo_depth_ms"),
            ParameterValue::Float(1.5),
        ),
        (
            ParameterId::from("allpass_feedback"),
            ParameterValue::Bool(true),
        ),
        (
            ParameterId::from("allpass_coeff"),
            ParameterValue::Float(0.7),
        ),
    ];

    for (id, value) in cases {
        plugin.set_parameter(id.clone(), value.clone()).unwrap();
        let read = plugin.get_parameter(&id).expect("parameter should exist");
        assert_eq!(read, value, "round-trip failed for {}", id);
    }
}

#[test]
fn parameter_roundtrip_per_channel() {
    let mut plugin = DelayPlugin::new_per_channel(vec![10.0, 20.0]).unwrap();
    plugin.initialize(SR).unwrap();

    plugin
        .set_parameter(ParameterId::from("delay_ms_0"), ParameterValue::Float(15.0))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("delay_ms_1"), ParameterValue::Float(25.0))
        .unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("delay_ms_0")),
        Some(ParameterValue::Float(15.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("delay_ms_1")),
        Some(ParameterValue::Float(25.0))
    );
}

#[test]
fn set_parameter_unknown_rejected() {
    let mut plugin = DelayPlugin::new(1, 100.0, 0.0, 0.5);
    let err = plugin
        .set_parameter(ParameterId::from("nope"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter"), "unexpected error: {err}");
}

#[test]
fn set_parameter_type_mismatch_rejected() {
    let mut plugin = DelayPlugin::new(1, 100.0, 0.0, 0.5);
    let err = plugin
        .set_parameter(
            ParameterId::from("delay_ms"),
            ParameterValue::String("fast".to_string()),
        )
        .unwrap_err();
    assert!(
        err.contains("type mismatch") || err.contains("must be a float"),
        "unexpected error: {err}"
    );
}

#[test]
fn delay_impulse_roundtrip() {
    let delay_ms = 10.0;
    let delay_frames = (delay_ms * SR as f32 / 1000.0) as usize;
    let mut plugin = DelayPlugin::new(2, delay_ms, 0.0, 1.0);
    plugin.initialize(SR).unwrap();

    let mut buf = impulse(2, 0);
    plugin.process_in_place(&mut buf, &ctx(1024)).unwrap();

    // With feedback=0 and mix=1, the impulse should appear at the delayed frame.
    let idx = delay_frames * 2;
    assert!(
        (buf[idx] - 1.0).abs() < 0.1,
        "expected delayed impulse around sample {delay_frames}, got {}",
        buf[idx]
    );
    assert_eq!(
        buf[0], 0.0,
        "initial output should be silent until delay line fills"
    );
}

#[test]
fn bypass_mix_zero_passthrough() {
    let mut plugin = DelayPlugin::new(2, 100.0, 0.0, 0.0);
    plugin.initialize(SR).unwrap();

    let frames = 128;
    let mut buf = vec![0.0f32; frames * 2];
    for frame in 0..frames {
        let v = (2.0 * std::f32::consts::PI * 220.0 * frame as f32 / SR as f32).sin() * 0.4;
        buf[frame * 2] = v;
        buf[frame * 2 + 1] = v;
    }
    let expected = buf.clone();

    plugin.process_in_place(&mut buf, &ctx(frames)).unwrap();
    for (i, (out, exp)) in buf.iter().zip(expected.iter()).enumerate() {
        assert!(
            (out - exp).abs() < 1e-5,
            "bypass sample {i} differs: {out} vs {exp}"
        );
    }
}

#[test]
fn reset_clears_delay_line() {
    let mut plugin = DelayPlugin::new(1, 50.0, 0.0, 1.0);
    plugin.initialize(SR).unwrap();

    // Feed an impulse.
    let mut buf = vec![0.0f32; 1024];
    buf[0] = 1.0;
    plugin.process_in_place(&mut buf, &ctx(1024)).unwrap();

    // Reset should empty the delay memory.
    plugin.reset();
    let mut silence = vec![0.0f32; 1024];
    plugin.process_in_place(&mut silence, &ctx(1024)).unwrap();
    assert!(
        silence.iter().all(|s| s.abs() < 1e-6),
        "delay line should be silent after reset"
    );
}

#[test]
fn feedback_decays_impulse() {
    let mut plugin = DelayPlugin::new(1, 10.0, 0.5, 1.0);
    plugin.initialize(SR).unwrap();

    let mut buf = vec![0.0f32; 4096];
    buf[0] = 1.0;
    plugin.process_in_place(&mut buf, &ctx(4096)).unwrap();

    let delay_frames = (10.0 * SR as f32 / 1000.0) as usize;
    let first = buf[delay_frames].abs();
    let second = buf[delay_frames * 2].abs();
    assert!(
        second < first * 0.6,
        "feedback should decay repeats: first={first} second={second}"
    );
}

#[test]
fn delay_with_lfo_produces_finite_output() {
    let mut plugin = DelayPlugin::from_params(
        1,
        DelayPluginParams {
            delay_ms: 20.0,
            feedback: 0.0,
            mix: 1.0,
            lfo_rate_hz: 2.0,
            lfo_depth_ms: 2.0,
            allpass_feedback: false,
            allpass_coeff: 0.5,
            channel_delays_ms: Vec::new(),
        },
    )
    .unwrap();
    plugin.initialize(SR).unwrap();

    let mut buf: Vec<f32> = (0..2048)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / SR as f32).sin() * 0.3)
        .collect();
    plugin.process_in_place(&mut buf, &ctx(2048)).unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
    assert!(rms(&buf) > 0.01, "LFO delay output should not be silent");
}
