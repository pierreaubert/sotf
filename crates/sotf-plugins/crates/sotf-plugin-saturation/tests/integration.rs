//! Integration tests for sotf-plugin-saturation.
//!
//! These tests exercise the public `InPlacePlugin` API as a black box:
//! construction, initialization, parameter get/set, audio processing, bypass,
//! reset, and error paths.

use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::ProcessContext;
use sotf_plugin_saturation::{SaturationPlugin, SaturationPluginParams};

const SR: u32 = 48000;

fn ctx(frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(SR, frames)
}

fn sine(freq_hz: f32, frames: usize, amp: f32) -> Vec<f32> {
    (0..frames)
        .map(|i| amp * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / SR as f32).sin())
        .collect()
}

fn rms(buf: &[f32]) -> f32 {
    let sum: f32 = buf.iter().map(|x| x * x).sum();
    (sum / buf.len().max(1) as f32).sqrt()
}

#[test]
fn instantiate_and_declare_metadata() {
    let plugin = SaturationPlugin::from_params(
        2,
        SaturationPluginParams {
            mode: "Tube".to_string(),
            oversampling: "4x".to_string(),
            ..Default::default()
        },
    );
    assert_eq!(plugin.info().name, "Saturation");
    assert_eq!(plugin.channels(), 2);
    assert!(!plugin.supports_f64());
    assert_eq!(plugin.preferred_oversampling(), Some(4));

    let params = plugin.parameters();
    let ids: Vec<_> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"mode"));
    assert!(ids.contains(&"drive"));
    assert!(ids.contains(&"mix"));
    assert!(ids.contains(&"dynamic_amount"));
}

#[test]
fn initialize_changes_sample_rate() {
    let mut plugin = SaturationPlugin::new(1);
    plugin.initialize(SR).unwrap();
    // Initialization is expected to succeed and leave the plugin ready to process.
    let mut buf = sine(440.0, 64, 0.5);
    plugin.process_in_place(&mut buf, &ctx(64)).unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn parameter_roundtrip() {
    let mut plugin = SaturationPlugin::new(2);
    plugin.initialize(SR).unwrap();

    let cases: Vec<(ParameterId, ParameterValue)> = vec![
        (
            ParameterId::from("mode"),
            ParameterValue::String("Tape".to_string()),
        ),
        (ParameterId::from("drive"), ParameterValue::Float(8.0)),
        (ParameterId::from("tone"), ParameterValue::Float(2.5)),
        (
            ParameterId::from("exciter_freq"),
            ParameterValue::Float(5000.0),
        ),
        (
            ParameterId::from("oversampling"),
            ParameterValue::String("Off".to_string()),
        ),
        (
            ParameterId::from("output_gain"),
            ParameterValue::Float(-3.0),
        ),
        (ParameterId::from("mix"), ParameterValue::Float(0.75)),
        (
            ParameterId::from("dynamic_amount"),
            ParameterValue::Float(0.5),
        ),
        (
            ParameterId::from("dynamic_attack_ms"),
            ParameterValue::Float(10.0),
        ),
        (
            ParameterId::from("dynamic_release_ms"),
            ParameterValue::Float(100.0),
        ),
    ];

    for (id, value) in cases {
        plugin.set_parameter(id.clone(), value.clone()).unwrap();
        let read = plugin.get_parameter(&id).expect("parameter should exist");
        assert_eq!(read, value, "round-trip failed for {}", id);
    }
}

#[test]
fn boolean_state_from_params() {
    let plugin = SaturationPlugin::from_params(
        1,
        SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            dc_blocker_enabled: false,
            use_adaa: false,
            ..Default::default()
        },
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dc_blocker")),
        Some(ParameterValue::Float(0.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("use_adaa")),
        Some(ParameterValue::Float(0.0))
    );
}

#[test]
fn set_parameter_unknown_rejected() {
    let mut plugin = SaturationPlugin::new(1);
    let err = plugin
        .set_parameter(ParameterId::from("nope"), ParameterValue::Float(1.0))
        .unwrap_err();
    assert!(err.contains("Unknown parameter"), "unexpected error: {err}");
}

#[test]
fn set_parameter_type_mismatch_rejected() {
    let mut plugin = SaturationPlugin::new(1);
    let err = plugin
        .set_parameter(
            ParameterId::from("drive"),
            ParameterValue::String("high".to_string()),
        )
        .unwrap_err();
    assert!(
        err.contains("type mismatch") || err.contains("Parameter type mismatch"),
        "unexpected error: {err}"
    );
}

#[test]
fn process_soft_clip_bounds_output() {
    let mut plugin = SaturationPlugin::from_params(
        1,
        SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            drive: 10.0,
            oversampling: "Off".to_string(),
            output_gain_db: 0.0,
            mix: 1.0,
            use_adaa: false,
            dc_blocker_enabled: false,
            ..Default::default()
        },
    );
    plugin.initialize(SR).unwrap();

    let mut buf = sine(440.0, 2048, 1.0);
    plugin.process_in_place(&mut buf, &ctx(2048)).unwrap();

    let peak = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(peak.is_finite());
    assert!(
        peak <= 1.05,
        "soft-clip output should be bounded, got peak={peak}"
    );
    assert!(peak > 0.1, "output should not be silent");
}

#[test]
fn bypass_mix_zero_passthrough() {
    let mut plugin = SaturationPlugin::from_params(
        2,
        SaturationPluginParams {
            mode: "Soft Clip".to_string(),
            drive: 10.0,
            mix: 0.0,
            use_adaa: false,
            dc_blocker_enabled: false,
            ..Default::default()
        },
    );
    plugin.initialize(SR).unwrap();

    let frames = 256;
    let mut buf = vec![0.0f32; frames * 2];
    for frame in 0..frames {
        let v = (2.0 * std::f32::consts::PI * 220.0 * frame as f32 / SR as f32).sin() * 0.3;
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
fn reset_leaves_plugin_ready() {
    let mut plugin = SaturationPlugin::from_params(
        1,
        SaturationPluginParams {
            mode: "Exciter".to_string(),
            oversampling: "2x".to_string(),
            mix: 1.0,
            ..Default::default()
        },
    );
    plugin.initialize(SR).unwrap();

    // Warm up state.
    let mut buf = sine(1000.0, 512, 0.5);
    plugin.process_in_place(&mut buf, &ctx(512)).unwrap();

    // Reset and process again.
    plugin.reset();
    let mut buf2 = sine(1000.0, 512, 0.5);
    plugin.process_in_place(&mut buf2, &ctx(512)).unwrap();
    assert!(buf2.iter().all(|s| s.is_finite()));
}

#[test]
fn process_error_when_buffer_too_short() {
    let mut plugin = SaturationPlugin::new(2);
    plugin.initialize(SR).unwrap();
    let mut buf = vec![0.0f32; 31]; // 2 channels * 16 frames requires 32
    let err = plugin.process_in_place(&mut buf, &ctx(16)).unwrap_err();
    assert!(err.contains("buffer too short"), "unexpected error: {err}");
}

#[test]
fn mode_switch_and_oversampling_state() {
    let mut plugin = SaturationPlugin::new(1);
    plugin.initialize(SR).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("Exciter".to_string()),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("oversampling"),
            ParameterValue::String("2x".to_string()),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("oversampling")),
        Some(ParameterValue::String("2x".to_string()))
    );
    assert_eq!(plugin.preferred_oversampling(), Some(2));

    let mut buf = sine(8000.0, 512, 0.5);
    plugin.process_in_place(&mut buf, &ctx(512)).unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn output_gain_affects_level() {
    let make_plugin = |gain_db: f32| {
        let mut p = SaturationPlugin::from_params(
            1,
            SaturationPluginParams {
                mode: "Soft Clip".to_string(),
                drive: 2.0,
                mix: 1.0,
                output_gain_db: gain_db,
                use_adaa: false,
                dc_blocker_enabled: false,
                ..Default::default()
            },
        );
        p.initialize(SR).unwrap();
        p
    };

    let mut plugin_0db = make_plugin(0.0);
    // Let the output-gain smoother settle, then measure the steady-state level.
    plugin_0db
        .process_in_place(&mut sine(440.0, 4096, 0.5), &ctx(4096))
        .unwrap();
    let mut buf_0db = sine(440.0, 4096, 0.5);
    plugin_0db
        .process_in_place(&mut buf_0db, &ctx(4096))
        .unwrap();
    let rms_0db = rms(&buf_0db[2048..]);

    let mut plugin_quiet = make_plugin(-12.0);
    plugin_quiet
        .process_in_place(&mut sine(440.0, 4096, 0.5), &ctx(4096))
        .unwrap();
    let mut buf_quiet = sine(440.0, 4096, 0.5);
    plugin_quiet
        .process_in_place(&mut buf_quiet, &ctx(4096))
        .unwrap();
    let rms_quiet = rms(&buf_quiet[2048..]);

    assert!(
        rms_quiet < rms_0db * 0.5,
        "-12 dB gain should reduce level: {rms_quiet} vs {rms_0db}"
    );
}
