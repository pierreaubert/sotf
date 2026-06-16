// Integration tests for sotf-plugin-de-esser — exercises the public API only.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_de_esser::{DeEsserPlugin, DeEsserPluginParams};

fn make_sine(freq_hz: f32, sample_rate: u32, num_frames: usize, amplitude: f32) -> Vec<f32> {
    (0..num_frames)
        .map(|i| {
            amplitude * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sample_rate as f32).sin()
        })
        .collect()
}

fn rms(buf: &[f32]) -> f32 {
    let sum: f32 = buf.iter().map(|x| x * x).sum();
    (sum / buf.len().max(1) as f32).sqrt()
}

#[test]
fn info_and_channels_match_construction() {
    let plugin = DeEsserPlugin::new(2);
    assert_eq!(plugin.channels(), 2);
    let info = plugin.info();
    assert_eq!(info.name, "DeEsser");
}

#[test]
fn initialize_changes_sample_rate() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(44100).unwrap();
    plugin.initialize(96000).unwrap();
}

#[test]
fn parameter_roundtrip() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    let cases: &[(&str, ParameterValue)] = &[
        ("frequency", ParameterValue::Float(10000.0)),
        ("q", ParameterValue::Float(2.5)),
        ("threshold", ParameterValue::Float(-30.0)),
        ("ratio", ParameterValue::Float(8.0)),
        ("attack", ParameterValue::Float(2.0)),
        ("release", ParameterValue::Float(50.0)),
        ("mode", ParameterValue::String("Wideband".to_string())),
        ("mix", ParameterValue::Float(0.25)),
    ];

    for &(id, ref value) in cases {
        plugin
            .parametric_set_parameter(ParameterId::from(id), value.clone())
            .unwrap();
        let got = plugin.parametric_get_parameter(&ParameterId::from(id));
        assert_eq!(
            got,
            Some(value.clone()),
            "roundtrip failed for parameter {}",
            id
        );
    }
}

#[test]
fn mode_variants_roundtrip() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    plugin
        .parametric_set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("Wideband".to_string()),
        )
        .unwrap();
    assert_eq!(
        plugin.parametric_get_parameter(&ParameterId::from("mode")),
        Some(ParameterValue::String("Wideband".to_string()))
    );

    plugin
        .parametric_set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("Split-Band".to_string()),
        )
        .unwrap();
    assert_eq!(
        plugin.parametric_get_parameter(&ParameterId::from("mode")),
        Some(ParameterValue::String("Split-Band".to_string()))
    );
}

#[test]
fn invalid_parameter_rejected() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();

    // Out of range.
    assert!(
        plugin
            .parametric_set_parameter(ParameterId::from("frequency"), ParameterValue::Float(100.0))
            .is_err()
    );
    assert!(
        plugin
            .parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(-0.1))
            .is_err()
    );
    // NaN.
    assert!(
        plugin
            .parametric_set_parameter(
                ParameterId::from("threshold"),
                ParameterValue::Float(f32::NAN)
            )
            .is_err()
    );
    // Unknown parameter.
    assert!(
        plugin
            .parametric_set_parameter(ParameterId::from("unknown"), ParameterValue::Float(1.0))
            .is_err()
    );

    assert!(
        plugin
            .parametric_get_parameter(&ParameterId::from("unknown"))
            .is_none()
    );
}

#[test]
fn process_zero_frames_returns_zero() {
    let mut plugin = DeEsserPlugin::new(1);
    plugin.initialize(48000).unwrap();
    let mut buffer = [0.0f32; 0];
    let ctx = ProcessContext::new(48000, 0);
    assert_eq!(plugin.process_in_place(&mut buffer, &ctx).unwrap(), 0);
}

#[test]
fn process_zero_channels_returns_num_frames() {
    let mut plugin = DeEsserPlugin::new(0);
    plugin.initialize(48000).unwrap();
    let mut buffer = [0.0f32; 0];
    let ctx = ProcessContext::new(48000, 64);
    assert_eq!(plugin.process_in_place(&mut buffer, &ctx).unwrap(), 64);
}

#[test]
fn reset_clears_state() {
    let sr = 48000u32;
    let mut plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 7000.0,
            q: 1.5,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Wideband".to_string(),
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = make_sine(8000.0, sr, 4800, 0.5);
    let ctx = ProcessContext::new(sr, 4800);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    plugin.reset();

    let mut low = make_sine(200.0, sr, 4800, 0.5);
    let input_rms = rms(&low);
    plugin.process_in_place(&mut low, &ctx).unwrap();
    let output_rms = rms(&low);
    assert!(
        output_rms > input_rms * 0.9,
        "reset should restore LF pass-through: input={:.4}, output={:.4}",
        input_rms,
        output_rms
    );
}

#[test]
fn wideband_reduces_sibilance() {
    let sr = 48000u32;
    let num_frames = 48000;
    let amplitude = 0.5;

    let mut plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 8000.0,
            q: 1.5,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Wideband".to_string(),
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = make_sine(8000.0, sr, num_frames, amplitude);
    let input_rms = rms(&buf);

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let output_rms = rms(&buf[num_frames / 2..]);
    assert!(
        output_rms < input_rms * 0.5,
        "8kHz signal should be reduced: input={:.4}, output={:.4}",
        input_rms,
        output_rms
    );
}

#[test]
fn low_frequency_passthrough() {
    let sr = 48000u32;
    let num_frames = 48000;
    let amplitude = 0.5;

    let mut plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 7000.0,
            q: 1.5,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Wideband".to_string(),
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = make_sine(200.0, sr, num_frames, amplitude);
    let input_rms = rms(&buf);

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let output_rms = rms(&buf[num_frames / 2..]);
    assert!(
        output_rms > input_rms * 0.9,
        "200Hz signal should pass through: input={:.4}, output={:.4}",
        input_rms,
        output_rms
    );
}

#[test]
fn split_band_attenuates_hf_passthrough_lf() {
    let sr = 48000u32;
    let num_frames = 48000;
    let amplitude = 0.5;

    let mut plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 7000.0,
            q: 1.5,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Split-Band".to_string(),
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    let mut hf = make_sine(8000.0, sr, num_frames, amplitude);
    let input_hf_rms = rms(&hf);
    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut hf, &ctx).unwrap();
    let output_hf_rms = rms(&hf[num_frames / 2..]);
    assert!(
        output_hf_rms < input_hf_rms * 0.7,
        "split-band: 8kHz should be reduced: input={:.4}, output={:.4}",
        input_hf_rms,
        output_hf_rms
    );

    plugin.reset();
    let mut lf = make_sine(200.0, sr, num_frames, amplitude);
    let input_lf_rms = rms(&lf);
    plugin.process_in_place(&mut lf, &ctx).unwrap();
    let output_lf_rms = rms(&lf[num_frames / 2..]);
    assert!(
        output_lf_rms > input_lf_rms * 0.85,
        "split-band: 200Hz should pass through: input={:.4}, output={:.4}",
        input_lf_rms,
        output_lf_rms
    );
}

#[test]
fn mix_zero_is_dry() {
    let sr = 48000u32;
    let num_frames = 4800;

    let mut plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 7000.0,
            q: 1.5,
            threshold: -20.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Wideband".to_string(),
            mix: 0.0,
        },
    );
    plugin.initialize(sr).unwrap();

    // Warm up the 5 ms mix smoother so it converges to dry.
    let mut warmup = vec![0.0f32; 4800];
    let warmup_ctx = ProcessContext::new(sr, warmup.len());
    plugin.process_in_place(&mut warmup, &warmup_ctx).unwrap();

    let input = make_sine(8000.0, sr, num_frames, 0.5);
    let mut buf = input.clone();
    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let max_diff = buf
        .iter()
        .zip(input.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_diff < 1.0e-4,
        "mix=0 should pass dry signal through, max diff {max_diff}"
    );
}

#[test]
fn stereo_channels_processed_independently() {
    let sr = 48000u32;
    let num_frames = 48000;
    let amplitude = 0.5;

    let mut plugin = DeEsserPlugin::from_params(
        2,
        DeEsserPluginParams {
            frequency: 7000.0,
            q: 1.5,
            threshold: -35.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            mode: "Wideband".to_string(),
            mix: 1.0,
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = Vec::with_capacity(num_frames * 2);
    let mut low_input = Vec::with_capacity(num_frames);
    let mut high_input = Vec::with_capacity(num_frames);
    for i in 0..num_frames {
        let low = amplitude * (2.0 * std::f32::consts::PI * 200.0 * i as f32 / sr as f32).sin();
        let high = amplitude * (2.0 * std::f32::consts::PI * 8000.0 * i as f32 / sr as f32).sin();
        buf.push(low);
        buf.push(high);
        low_input.push(low);
        high_input.push(high);
    }

    let input_low_rms = rms(&low_input);
    let input_high_rms = rms(&high_input);

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    let mut low_output = Vec::with_capacity(num_frames);
    let mut high_output = Vec::with_capacity(num_frames);
    for frame in 0..num_frames {
        low_output.push(buf[frame * 2]);
        high_output.push(buf[frame * 2 + 1]);
    }

    let output_low_rms = rms(&low_output);
    let output_high_rms = rms(&high_output);

    assert!(
        output_low_rms > input_low_rms * 0.9,
        "Low band should remain mostly untouched: input={:.4}, output={:.4}",
        input_low_rms,
        output_low_rms
    );
    assert!(
        output_high_rms < input_high_rms * 0.7,
        "High band should be reduced: input={:.4}, output={:.4}",
        input_high_rms,
        output_high_rms
    );
}

#[test]
fn from_params_clamps_out_of_bounds() {
    let plugin = DeEsserPlugin::from_params(
        1,
        DeEsserPluginParams {
            frequency: 100.0,
            q: 10.0,
            threshold: 10.0,
            ratio: 0.5,
            attack_ms: 0.01,
            release_ms: 1.0,
            mode: "Wideband".to_string(),
            mix: -1.0,
        },
    );
    assert_eq!(
        plugin.parametric_get_parameter(&ParameterId::from("frequency")),
        Some(ParameterValue::Float(2000.0))
    );
    assert_eq!(
        plugin.parametric_get_parameter(&ParameterId::from("q")),
        Some(ParameterValue::Float(5.0))
    );
    assert_eq!(
        plugin.parametric_get_parameter(&ParameterId::from("threshold")),
        Some(ParameterValue::Float(0.0))
    );
    assert_eq!(
        plugin.parametric_get_parameter(&ParameterId::from("ratio")),
        Some(ParameterValue::Float(1.0))
    );
    assert_eq!(
        plugin.parametric_get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.0))
    );
}

#[test]
fn parameters_list_contains_expected_ids() {
    let plugin = DeEsserPlugin::new(1);
    let params = plugin.parametric_parameters();
    let ids: Vec<_> = params.iter().map(|p| p.id.clone()).collect();
    assert!(ids.contains(&ParameterId::from("frequency")));
    assert!(ids.contains(&ParameterId::from("q")));
    assert!(ids.contains(&ParameterId::from("threshold")));
    assert!(ids.contains(&ParameterId::from("ratio")));
    assert!(ids.contains(&ParameterId::from("attack")));
    assert!(ids.contains(&ParameterId::from("release")));
    assert!(ids.contains(&ParameterId::from("mode")));
    assert!(ids.contains(&ParameterId::from("mix")));
}
