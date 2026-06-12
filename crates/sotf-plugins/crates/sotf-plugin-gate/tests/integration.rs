//! Integration tests for sotf-plugin-gate.
//!
//! These tests exercise the public `InPlacePlugin` API as a black box:
//! construction, initialization, parameter get/set, audio processing, bypass,
//! reset, and error paths.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_gate::{GateData, GatePlugin, GatePluginParams};

const SR: u32 = 48000;

fn ctx(frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(SR, frames)
}

fn rms(buf: &[f32]) -> f32 {
    let sum: f32 = buf.iter().map(|x| x * x).sum();
    (sum / buf.len().max(1) as f32).sqrt()
}

fn dc(level: f32, frames: usize) -> Vec<f32> {
    vec![level; frames]
}

#[test]
fn instantiate_and_declare_metadata() {
    let plugin = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
    assert_eq!(plugin.info().name, "Gate");
    assert_eq!(plugin.channels(), 2);
    assert_eq!(plugin.input_channels(), 2);

    let params = plugin.parameters();
    let ids: Vec<_> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"threshold"));
    assert!(ids.contains(&"ratio"));
    assert!(ids.contains(&"mix"));
}

#[test]
fn from_params_sets_sidechain_state() {
    let mut plugin = GatePlugin::from_params(
        2,
        GatePluginParams {
            threshold_db: -30.0,
            ratio: 20.0,
            attack_ms: 1.0,
            hold_ms: 5.0,
            release_ms: 50.0,
            mix: 1.0,
            link_channels: true,
            sidechain_hpf_hz: 100.0,
            sidechain_hpf_order: "4th".to_string(),
            detection_mode: "rms".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 4.0,
            knee_db: 3.0,
            lookahead_ms: 5.0,
        },
    );
    plugin.initialize(SR).unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(
        plugin.latency_samples(),
        (5.0 * SR as f32 / 1000.0) as usize
    );
}

#[test]
fn initialize_and_reset() {
    let mut plugin = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
    plugin.initialize(SR).unwrap();
    plugin.reset();
    let mut buf = dc(0.1, 256);
    plugin.process_in_place(&mut buf, &ctx(128)).unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn parameter_roundtrip() {
    let mut plugin = GatePlugin::new(1, -40.0, 10.0, 1.0, 10.0, 100.0);
    plugin.initialize(SR).unwrap();

    let cases: Vec<(ParameterId, ParameterValue)> = vec![
        (ParameterId::from("threshold"), ParameterValue::Float(-30.0)),
        (ParameterId::from("ratio"), ParameterValue::Float(20.0)),
        (ParameterId::from("attack"), ParameterValue::Float(5.0)),
        (ParameterId::from("hold"), ParameterValue::Float(20.0)),
        (ParameterId::from("release"), ParameterValue::Float(200.0)),
        (ParameterId::from("mix"), ParameterValue::Float(0.75)),
        (
            ParameterId::from("link_channels"),
            ParameterValue::Bool(false),
        ),
        (
            ParameterId::from("sidechain_hpf_hz"),
            ParameterValue::Float(50.0),
        ),
        (
            ParameterId::from("sidechain_hpf_order"),
            ParameterValue::Int(1),
        ),
        (ParameterId::from("detection_mode"), ParameterValue::Int(1)),
        (
            ParameterId::from("sidechain_external"),
            ParameterValue::Bool(true),
        ),
        (ParameterId::from("range_db"), ParameterValue::Float(60.0)),
        (
            ParameterId::from("hysteresis_db"),
            ParameterValue::Float(4.0),
        ),
        (ParameterId::from("knee_db"), ParameterValue::Float(3.0)),
        (
            ParameterId::from("lookahead_ms"),
            ParameterValue::Float(10.0),
        ),
    ];

    for (id, value) in cases {
        plugin.set_parameter(id.clone(), value.clone()).unwrap();
        let read = plugin.get_parameter(&id).expect("parameter should exist");
        assert_eq!(read, value, "round-trip failed for {}", id);
    }

    // External sidechain changes input channel count.
    assert_eq!(plugin.input_channels(), 2);
}

#[test]
fn set_parameter_unknown_rejected() {
    let mut plugin = GatePlugin::new(1, -40.0, 10.0, 1.0, 10.0, 100.0);
    let err = plugin
        .set_parameter(ParameterId::from("nope"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter"), "unexpected error: {err}");
}

#[test]
fn set_parameter_type_mismatch_rejected() {
    let mut plugin = GatePlugin::new(1, -40.0, 10.0, 1.0, 10.0, 100.0);
    let err = plugin
        .set_parameter(
            ParameterId::from("threshold"),
            ParameterValue::String("low".to_string()),
        )
        .unwrap_err();
    assert!(
        err.contains("type mismatch") || err.contains("Parameter type mismatch"),
        "unexpected error: {err}"
    );
}

#[test]
fn loud_signal_passes() {
    let mut plugin = GatePlugin::from_params(
        2,
        GatePluginParams {
            threshold_db: -40.0,
            ratio: 10.0,
            attack_ms: 1.0,
            hold_ms: 10.0,
            release_ms: 100.0,
            mix: 1.0,
            link_channels: true,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    plugin.initialize(SR).unwrap();

    let frames = 4096;
    let input = dc(0.5, frames * 2);
    let mut buf = input.clone();
    plugin.process_in_place(&mut buf, &ctx(frames)).unwrap();

    let ratio = rms(&buf) / rms(&input);
    assert!(
        ratio > 0.8,
        "loud signal should pass through: ratio={ratio}"
    );
}

#[test]
fn quiet_signal_is_attenuated() {
    let mut plugin = GatePlugin::from_params(
        2,
        GatePluginParams {
            threshold_db: -40.0,
            ratio: 10.0,
            attack_ms: 1.0,
            hold_ms: 10.0,
            release_ms: 100.0,
            mix: 1.0,
            link_channels: true,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    plugin.initialize(SR).unwrap();

    let frames = SR as usize; // 1 second
    let input = dc(0.001, frames * 2);
    let mut buf = input.clone();
    plugin.process_in_place(&mut buf, &ctx(frames)).unwrap();

    let in_rms = rms(&input);
    // Measure the steady-state tail to avoid the initial attack/hold/release ramp.
    let out_rms = rms(&buf[frames..]);
    assert!(
        out_rms < in_rms * 0.2,
        "quiet signal should be heavily attenuated: in={in_rms} out={out_rms}"
    );
}

#[test]
fn bypass_mix_zero_passthrough() {
    let mut plugin = GatePlugin::from_params(
        2,
        GatePluginParams {
            threshold_db: -40.0,
            ratio: 10.0,
            attack_ms: 1.0,
            hold_ms: 10.0,
            release_ms: 100.0,
            mix: 0.0,
            link_channels: true,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    plugin.initialize(SR).unwrap();

    // Set mix to 0 explicitly and let its smoother settle.
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.0))
        .unwrap();

    let make_sine = |frames: usize| {
        let mut v = vec![0.0f32; frames * 2];
        for frame in 0..frames {
            let s = (2.0 * std::f32::consts::PI * 220.0 * frame as f32 / SR as f32).sin() * 0.3;
            v[frame * 2] = s;
            v[frame * 2 + 1] = s;
        }
        v
    };

    let mut warmup = make_sine(8192);
    plugin.process_in_place(&mut warmup, &ctx(8192)).unwrap();

    let frames = 256;
    let mut buf = make_sine(frames);
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
fn reset_returns_deterministic_state() {
    let mut plugin = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -30.0,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 50.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    plugin.initialize(SR).unwrap();

    // First run opens the gate on a loud signal.
    let mut loud = dc(0.5, SR as usize);
    plugin
        .process_in_place(&mut loud, &ctx(SR as usize))
        .unwrap();

    // Reset and repeat with the same loud signal.
    plugin.reset();
    let mut loud2 = dc(0.5, SR as usize);
    plugin
        .process_in_place(&mut loud2, &ctx(SR as usize))
        .unwrap();

    // Both should pass the signal similarly.
    assert!(
        (rms(&loud) - rms(&loud2)).abs() < 1e-4,
        "reset should yield deterministic loud-signal behavior"
    );
}

#[test]
fn diagnostic_data_exposed() {
    let mut plugin = GatePlugin::from_params(
        1,
        GatePluginParams {
            threshold_db: -30.0,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 20.0,
            mix: 1.0,
            link_channels: false,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    plugin.initialize(SR).unwrap();

    // Feed silence in small blocks so the cache updater fires.
    let block_size = 512;
    for _ in 0..20 {
        let mut silence = vec![0.0f32; block_size];
        plugin
            .process_in_place(&mut silence, &ctx(block_size))
            .unwrap();
    }

    let data = plugin
        .get_data()
        .expect("gate should expose diagnostic data");
    let gate_data = data.downcast::<GateData>().expect("must be GateData");
    assert!(
        !gate_data.is_open,
        "gate should report closed after silence"
    );
}
