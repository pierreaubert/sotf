//! Integration tests for sotf-plugin-saturation.
//!
//! These tests exercise the public `InPlacePlugin` API as a black box:
//! construction, initialization, parameter get/set, audio processing, bypass,
//! reset, and error paths.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{AutoOversampledPlugin, ParametricInPlacePluginAdapter};
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
    assert_eq!(plugin.latency_samples(), 0);

    let params = plugin.parameters();
    let ids: Vec<_> = params.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&"mode"));
    assert!(ids.contains(&"drive"));
    assert!(ids.contains(&"mix"));
    assert!(ids.contains(&"dynamic_amount"));
}

#[test]
fn host_wraps_exactly_once_and_aligns_dry_wet_in_one_time_domain() {
    fn wrapped(mix: f32) -> AutoOversampledPlugin {
        let inner = SaturationPlugin::from_params(
            1,
            SaturationPluginParams {
                mode: "Soft Clip".to_string(),
                drive: 8.0,
                oversampling: "2x".to_string(),
                mix,
                dc_blocker_enabled: false,
                use_adaa: false,
                ..Default::default()
            },
        );
        let adapter = ParametricInPlacePluginAdapter::new(inner);
        let mut wrapped = AutoOversampledPlugin::new(Box::new(adapter), 2).unwrap();
        wrapped.initialize(SR).unwrap();
        wrapped
    }

    let mut dry = wrapped(0.0);
    let mut mixed = wrapped(0.5);
    assert_eq!(dry.preferred_oversampling(), None);
    assert!(dry.latency_samples() > 0);
    let frames = 4096;
    let mut input = vec![0.0; frames];
    input[0] = 0.5;
    let mut dry_output = vec![0.0; frames];
    let mut mixed_output = vec![0.0; frames];
    dry.process(&input, &mut dry_output, &ctx(frames)).unwrap();
    mixed
        .process(&input, &mut mixed_output, &ctx(frames))
        .unwrap();
    let peak = |samples: &[f32]| {
        samples
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap()
            .0
    };
    assert_eq!(peak(&dry_output), peak(&mixed_output));
}

#[test]
fn drive_automation_is_callback_partition_invariant_for_every_topology() {
    let frames = 2_048;
    let input: Vec<f32> = (0..frames)
        .map(|frame| {
            let low = (2.0 * std::f32::consts::PI * 317.0 * frame as f32 / SR as f32).sin();
            let high = (2.0 * std::f32::consts::PI * 7_913.0 * frame as f32 / SR as f32).sin();
            0.25 * low + 0.15 * high
        })
        .collect();
    let partitions = [1, 17, 3, 64, 5, 127, 2, 251, 31, 509, 7, 89];

    for mode in ["Soft Clip", "Tube", "Tape", "Exciter", "Asymmetric"] {
        for oversampling in ["Off", "2x", "4x"] {
            let make_plugin = || {
                let mut plugin = SaturationPlugin::from_params(
                    1,
                    SaturationPluginParams {
                        mode: mode.to_string(),
                        drive: 1.0,
                        oversampling: oversampling.to_string(),
                        exciter_freq: 3_000.0,
                        dynamic_amount: 0.4,
                        mix: 1.0,
                        dc_blocker_enabled: false,
                        use_adaa: false,
                        ..Default::default()
                    },
                );
                plugin.initialize(SR).unwrap();
                plugin
                    .set_parameter(ParameterId::from("drive"), ParameterValue::Float(15.0))
                    .unwrap();
                plugin
            };

            let mut whole_plugin = make_plugin();
            let mut whole = input.clone();
            whole_plugin
                .process_in_place(&mut whole, &ctx(frames))
                .unwrap();

            let mut partitioned_plugin = make_plugin();
            let mut partitioned = input.clone();
            let mut offset = 0;
            let mut partition_index = 0;
            while offset < frames {
                let count = partitions[partition_index % partitions.len()].min(frames - offset);
                partitioned_plugin
                    .process_in_place(&mut partitioned[offset..offset + count], &ctx(count))
                    .unwrap();
                offset += count;
                partition_index += 1;
            }

            let max_error = whole
                .iter()
                .zip(&partitioned)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f32, f32::max);
            assert!(
                max_error < 2e-5,
                "{mode}/{oversampling} drive automation depends on callback partitioning: {max_error}"
            );
        }
    }
}

#[test]
fn dynamic_control_survives_every_mode_and_actual_host_oversampling_factor() {
    fn render(mode: &str, factor: u32, dynamic_amount: f32) -> Vec<f32> {
        let inner = SaturationPlugin::from_params(
            1,
            SaturationPluginParams {
                mode: mode.to_string(),
                drive: 3.0,
                oversampling: match factor {
                    2 => "2x",
                    4 => "4x",
                    _ => "Off",
                }
                .to_string(),
                exciter_freq: 3_000.0,
                dynamic_amount,
                dynamic_attack_ms: 0.1,
                dynamic_release_ms: 1.0,
                mix: 1.0,
                dc_blocker_enabled: false,
                use_adaa: false,
                ..Default::default()
            },
        );
        let adapter = ParametricInPlacePluginAdapter::new(inner);
        let mut plugin: Box<dyn Plugin> = if factor == 1 {
            Box::new(adapter)
        } else {
            Box::new(AutoOversampledPlugin::new(Box::new(adapter), factor).unwrap())
        };
        plugin.initialize(SR).unwrap();
        let frames = 4_096;
        let input: Vec<f32> = (0..frames)
            .map(|frame| {
                let amplitude = if frame < frames / 2 { 0.08 } else { 0.7 };
                amplitude * (2.0 * std::f32::consts::PI * 7_500.0 * frame as f32 / SR as f32).sin()
            })
            .collect();
        let mut output = vec![0.0; frames];
        plugin.process(&input, &mut output, &ctx(frames)).unwrap();
        output
    }

    for mode in ["Soft Clip", "Tube", "Tape", "Exciter", "Asymmetric"] {
        for factor in [1, 2, 4] {
            let static_output = render(mode, factor, 0.0);
            let dynamic_output = render(mode, factor, 1.0);
            assert!(dynamic_output.iter().all(|sample| sample.is_finite()));
            let difference: f32 = static_output
                .iter()
                .zip(&dynamic_output)
                .skip(2_048)
                .map(|(a, b)| (a - b).abs())
                .sum();
            assert!(
                difference > 0.1,
                "{mode}/{factor}x discarded dynamic drive (difference={difference})"
            );
        }
    }
}

#[test]
fn four_x_host_oversampling_reduces_out_of_band_harmonic_aliases() {
    fn render(factor: u32) -> Vec<f32> {
        let inner = SaturationPlugin::from_params(
            1,
            SaturationPluginParams {
                mode: "Asymmetric".to_string(),
                drive: 12.0,
                oversampling: if factor == 4 { "4x" } else { "Off" }.to_string(),
                mix: 1.0,
                dc_blocker_enabled: false,
                use_adaa: false,
                ..Default::default()
            },
        );
        let adapter = ParametricInPlacePluginAdapter::new(inner);
        let mut plugin: Box<dyn Plugin> = if factor == 4 {
            Box::new(AutoOversampledPlugin::new(Box::new(adapter), 4).unwrap())
        } else {
            Box::new(adapter)
        };
        plugin.initialize(SR).unwrap();
        let frames = 8_192;
        let input = sine(9_000.0, frames, 0.8);
        let mut output = vec![0.0; frames];
        plugin.process(&input, &mut output, &ctx(frames)).unwrap();
        output
    }

    fn magnitude(samples: &[f32], frequency: f32) -> f32 {
        let (real, imag) =
            samples
                .iter()
                .enumerate()
                .fold((0.0_f32, 0.0_f32), |(real, imag), (index, sample)| {
                    let phase = 2.0 * std::f32::consts::PI * frequency * index as f32 / SR as f32;
                    (real + sample * phase.cos(), imag - sample * phase.sin())
                });
        2.0 * (real * real + imag * imag).sqrt() / samples.len() as f32
    }

    let direct = render(1);
    let oversampled = render(4);
    let direct_tail = &direct[4_096..];
    let oversampled_tail = &oversampled[4_096..];
    // A 9 kHz waveshaper's 3rd and 5th harmonics alias to 21 kHz and 3 kHz.
    let direct_alias = magnitude(direct_tail, 21_000.0).hypot(magnitude(direct_tail, 3_000.0));
    let oversampled_alias =
        magnitude(oversampled_tail, 21_000.0).hypot(magnitude(oversampled_tail, 3_000.0));
    assert!(
        oversampled_alias < direct_alias * 0.5,
        "4x host oversampling did not sufficiently reject aliases: direct={direct_alias}, 4x={oversampled_alias}"
    );
}

#[test]
fn asymmetric_mode_matches_independent_normalized_curve_oracle() {
    fn oracle(x: f32, drive: f32, tone: f32) -> f32 {
        let bias = 0.08 + 0.16 * (tone - 1.0).clamp(0.0, 2.0);
        let bias_tanh = bias.tanh();
        let centered = (x * drive + bias).tanh() - bias_tanh;
        if centered >= 0.0 {
            centered / (1.0 - bias_tanh)
        } else {
            centered / (1.0 + bias_tanh)
        }
    }

    let drive = 3.5;
    let tone = 2.25;
    let mut plugin = SaturationPlugin::from_params(
        1,
        SaturationPluginParams {
            mode: "Asymmetric".into(),
            drive,
            tone,
            oversampling: "Off".into(),
            mix: 1.0,
            dc_blocker_enabled: false,
            use_adaa: false,
            ..Default::default()
        },
    );
    plugin.initialize(SR).unwrap();
    let input = [-1.0, -0.5, -0.125, 0.0, 0.125, 0.5, 1.0];
    let mut output = input;
    let frames = output.len();
    plugin.process_in_place(&mut output, &ctx(frames)).unwrap();

    for (actual, input) in output.into_iter().zip(input) {
        let expected = oracle(input, drive, tone);
        assert!(
            (actual - expected).abs() < 2e-6,
            "asymmetric oracle mismatch for {input}: actual={actual}, expected={expected}"
        );
        assert!(actual.abs() <= 1.0);
    }
}

#[test]
fn asymmetric_even_harmonics_and_dc_blocker_have_measured_contracts() {
    fn render(dc_blocker_enabled: bool) -> Vec<f32> {
        let mut plugin = SaturationPlugin::from_params(
            1,
            SaturationPluginParams {
                mode: "Asymmetric".into(),
                drive: 5.0,
                tone: 2.5,
                oversampling: "Off".into(),
                mix: 1.0,
                dc_blocker_enabled,
                use_adaa: false,
                ..Default::default()
            },
        );
        plugin.initialize(SR).unwrap();
        let mut output = sine(1_000.0, SR as usize, 0.5);
        let frames = output.len();
        plugin.process_in_place(&mut output, &ctx(frames)).unwrap();
        output
    }

    fn mean(samples: &[f32]) -> f32 {
        samples.iter().sum::<f32>() / samples.len() as f32
    }

    fn magnitude(samples: &[f32], frequency: f32) -> f32 {
        let (real, imag) =
            samples
                .iter()
                .enumerate()
                .fold((0.0_f32, 0.0_f32), |(real, imag), (index, sample)| {
                    let phase = 2.0 * std::f32::consts::PI * frequency * index as f32 / SR as f32;
                    (real + sample * phase.cos(), imag - sample * phase.sin())
                });
        2.0 * real.hypot(imag) / samples.len() as f32
    }

    let raw = render(false);
    let blocked = render(true);
    let raw_tail = &raw[SR as usize / 2..];
    let blocked_tail = &blocked[SR as usize / 2..];
    let raw_dc = mean(raw_tail).abs();
    let blocked_dc = mean(blocked_tail).abs();
    assert!(
        raw_dc > 1e-3,
        "asymmetric curve should expose measurable programme DC"
    );
    assert!(
        blocked_dc < raw_dc * 0.1,
        "DC blocker insufficient: raw={raw_dc}, blocked={blocked_dc}"
    );

    let fundamental = magnitude(raw_tail, 1_000.0);
    let second = magnitude(raw_tail, 2_000.0);
    let third = magnitude(raw_tail, 3_000.0);
    let thd = second.hypot(third) / fundamental;
    assert!(
        second > 1e-3,
        "asymmetric curve must generate even harmonics"
    );
    assert!((0.01..1.0).contains(&thd), "unexpected THD ratio: {thd}");
}

#[test]
fn asymmetric_stereo_channels_are_independent() {
    let frames = 2_048;
    let mut plugin = SaturationPlugin::from_params(
        2,
        SaturationPluginParams {
            mode: "Asymmetric".into(),
            drive: 8.0,
            tone: 2.0,
            oversampling: "Off".into(),
            mix: 1.0,
            dc_blocker_enabled: false,
            use_adaa: false,
            ..Default::default()
        },
    );
    plugin.initialize(SR).unwrap();
    let mut buffer = vec![0.0; frames * 2];
    for frame in 0..frames {
        buffer[frame * 2] =
            0.5 * (2.0 * std::f32::consts::PI * 997.0 * frame as f32 / SR as f32).sin();
    }
    plugin.process_in_place(&mut buffer, &ctx(frames)).unwrap();
    assert!(buffer.iter().step_by(2).any(|sample| sample.abs() > 0.1));
    assert!(
        buffer
            .iter()
            .skip(1)
            .step_by(2)
            .all(|sample| *sample == 0.0),
        "silent right channel leaked from left"
    );
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
    plugin.initialize(SR).unwrap();
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
        Some(ParameterValue::Bool(false))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("use_adaa")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn legacy_float_booleans_are_accepted() {
    let mut plugin = SaturationPlugin::from_params(
        1,
        SaturationPluginParams {
            dc_blocker_enabled: true,
            use_adaa: false,
            ..Default::default()
        },
    );

    plugin
        .set_parameter(ParameterId::from("dc_blocker"), ParameterValue::Float(0.0))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("use_adaa"), ParameterValue::Float(1.0))
        .unwrap();

    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dc_blocker")),
        Some(ParameterValue::Bool(false))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("use_adaa")),
        Some(ParameterValue::Bool(true))
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

    assert!(
        plugin
            .set_parameter(
                ParameterId::from("mode"),
                ParameterValue::String("Exciter".to_string()),
            )
            .unwrap_err()
            .contains("structural")
    );
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("oversampling"),
                ParameterValue::String("4x".to_string()),
            )
            .unwrap_err()
            .contains("structural")
    );

    let mut plugin = SaturationPlugin::from_params(
        1,
        SaturationPluginParams {
            mode: "Exciter".to_string(),
            oversampling: "2x".to_string(),
            ..Default::default()
        },
    );
    plugin.initialize(SR).unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("oversampling")),
        Some(ParameterValue::String("2x".to_string()))
    );
    assert_eq!(plugin.preferred_oversampling(), Some(2));
    assert_eq!(plugin.latency_samples(), 0);

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
