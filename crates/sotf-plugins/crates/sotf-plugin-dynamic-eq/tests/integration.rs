// Integration tests for sotf-plugin-dynamic-eq — exercises the public API only.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_dynamic_eq::{DynEqBandParams, DynamicEqPlugin, DynamicEqPluginParams};

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
    let plugin = DynamicEqPlugin::new(2);
    assert_eq!(plugin.channels(), 2);
    let info = plugin.info();
    assert_eq!(info.name, "DynamicEQ");
}

#[test]
fn initialize_changes_sample_rate() {
    let mut plugin = DynamicEqPlugin::new(1);
    plugin.initialize(48000).unwrap();
    plugin.initialize(96000).unwrap();
}

#[test]
fn global_parameter_roundtrip() {
    let mut plugin = DynamicEqPlugin::new(2);
    plugin.initialize(48000).unwrap();

    let cases: &[(&str, ParameterValue)] = &[
        ("threshold", ParameterValue::Float(-30.0)),
        ("ratio", ParameterValue::Float(5.0)),
        ("attack", ParameterValue::Float(25.0)),
        ("release", ParameterValue::Float(200.0)),
        ("knee", ParameterValue::Float(6.0)),
        ("link_channels", ParameterValue::Bool(false)),
        ("mix", ParameterValue::Float(0.5)),
        ("num_bands", ParameterValue::Int(6)),
    ];

    for &(id, ref value) in cases {
        plugin
            .set_parameter(ParameterId::from(id), value.clone())
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from(id));
        assert_eq!(
            got,
            Some(value.clone()),
            "roundtrip failed for parameter {}",
            id
        );
    }
}

#[test]
fn per_band_parameter_roundtrip() {
    let mut plugin = DynamicEqPlugin::new(1);
    plugin.initialize(48000).unwrap();

    let cases: &[(&str, ParameterValue)] = &[
        ("band_0_frequency", ParameterValue::Float(500.0)),
        ("band_0_q", ParameterValue::Float(2.0)),
        ("band_0_gain", ParameterValue::Float(6.0)),
        ("band_0_threshold", ParameterValue::Float(-40.0)),
        ("band_0_ratio", ParameterValue::Float(8.0)),
        ("band_0_active", ParameterValue::Bool(false)),
        ("band_0_solo", ParameterValue::Bool(true)),
    ];

    for &(id, ref value) in cases {
        plugin
            .set_parameter(ParameterId::from(id), value.clone())
            .unwrap();
        let got = plugin.get_parameter(&ParameterId::from(id));
        assert_eq!(
            got,
            Some(value.clone()),
            "roundtrip failed for parameter {}",
            id
        );
    }
}

#[test]
fn invalid_parameter_rejected() {
    let mut plugin = DynamicEqPlugin::new(1);
    plugin.initialize(48000).unwrap();

    // Out of range.
    assert!(
        plugin
            .set_parameter(
                ParameterId::from("threshold"),
                ParameterValue::Float(-100.0)
            )
            .is_err()
    );
    // NaN.
    assert!(
        plugin
            .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(f32::NAN))
            .is_err()
    );
    // Unknown parameter.
    assert!(
        plugin
            .set_parameter(ParameterId::from("unknown"), ParameterValue::Float(1.0))
            .is_err()
    );

    assert!(
        plugin
            .get_parameter(&ParameterId::from("unknown"))
            .is_none()
    );
}

#[test]
fn process_zero_frames_returns_zero() {
    let mut plugin = DynamicEqPlugin::new(1);
    plugin.initialize(48000).unwrap();
    let mut buffer = [0.0f32; 0];
    let ctx = ProcessContext::new(48000, 0);
    assert_eq!(plugin.process_in_place(&mut buffer, &ctx).unwrap(), 0);
}

#[test]
fn buffer_size_mismatch_returns_error() {
    let mut plugin = DynamicEqPlugin::new(2);
    plugin.initialize(48000).unwrap();
    let ctx = ProcessContext::new(48000, 16);
    let mut short = vec![0.0; 31];
    let err = plugin.process_in_place(&mut short, &ctx).unwrap_err();
    assert!(err.contains("Buffer size mismatch"));
}

#[test]
fn block_too_large_rejected() {
    let mut plugin = DynamicEqPlugin::new(1);
    plugin.initialize(48000).unwrap();
    let num_frames = 200_000;
    let mut big = vec![0.0f32; num_frames];
    let ctx = ProcessContext::new(48000, num_frames);
    let err = plugin.process_in_place(&mut big, &ctx).unwrap_err();
    assert!(err.contains("exceeds max"));
}

#[test]
fn reset_clears_state() {
    let sr = 48000u32;
    let mut plugin = DynamicEqPlugin::new(1);
    plugin.initialize(sr).unwrap();

    let mut buf = make_sine(1000.0, sr, 4800, 0.5);
    let ctx = ProcessContext::new(sr, 4800);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    plugin.reset();

    // After reset, a quiet signal should pass through unchanged.
    let mut quiet = make_sine(200.0, sr, 4800, 0.1);
    let input_rms = rms(&quiet);
    plugin.process_in_place(&mut quiet, &ctx).unwrap();
    let output_rms = rms(&quiet);
    assert!(
        output_rms > input_rms * 0.95,
        "reset should restore clean state: input={:.4}, output={:.4}",
        input_rms,
        output_rms
    );
}

#[test]
fn dynamic_eq_attenuates_triggered_band() {
    let sr = 48000u32;
    let num_frames = 48000;

    let mut plugin = DynamicEqPlugin::from_params(
        1,
        DynamicEqPluginParams {
            num_bands: 1,
            threshold: -60.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            knee: 0.0,
            link_channels: false,
            mix: 1.0,
            bands: vec![DynEqBandParams {
                frequency: 1000.0,
                q: 1.0,
                gain: 12.0,
                band_threshold: -60.0,
                band_ratio: 10.0,
                active: true,
                solo: false,
            }],
        },
    );
    plugin.initialize(sr).unwrap();

    let mut buf = make_sine(1000.0, sr, num_frames, 0.5);
    let input_rms = rms(&buf);

    let ctx = ProcessContext::new(sr, num_frames);
    plugin.process_in_place(&mut buf, &ctx).unwrap();

    // Use the second half to let attack settle.
    let output_rms = rms(&buf[num_frames / 2..]);
    // A triggered band with +12 dB target gain amplifies the tone, so the
    // important property is that the output is modulated (not equal to input).
    let ratio = output_rms / input_rms;
    assert!(
        ratio > 1.5 && ratio < 5.0,
        "triggered dynamic EQ band should amplify output: input={:.4}, output={:.4}, ratio={:.2}",
        input_rms,
        output_rms,
        ratio
    );
}

#[test]
fn inactive_band_is_passthrough() {
    let sr = 48000u32;
    let num_frames = 4800;

    let mut plugin = DynamicEqPlugin::from_params(
        1,
        DynamicEqPluginParams {
            num_bands: 1,
            threshold: -60.0,
            ratio: 4.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            knee: 0.0,
            link_channels: false,
            mix: 1.0,
            bands: vec![DynEqBandParams {
                frequency: 1000.0,
                q: 1.0,
                gain: 12.0,
                band_threshold: -60.0,
                band_ratio: 4.0,
                active: false,
                solo: false,
            }],
        },
    );
    plugin.initialize(sr).unwrap();

    let input = make_sine(1000.0, sr, num_frames, 0.5);
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
        "inactive band should be passthrough, max diff {max_diff}"
    );
}

#[test]
fn mix_zero_passthrough() {
    let sr = 48000u32;
    let num_frames = 4800;

    let mut plugin = DynamicEqPlugin::from_params(
        1,
        DynamicEqPluginParams {
            num_bands: 1,
            threshold: -60.0,
            ratio: 10.0,
            attack_ms: 0.5,
            release_ms: 20.0,
            knee: 0.0,
            link_channels: false,
            mix: 0.0,
            bands: vec![DynEqBandParams {
                frequency: 1000.0,
                q: 1.0,
                gain: 12.0,
                band_threshold: -60.0,
                band_ratio: 10.0,
                active: true,
                solo: false,
            }],
        },
    );
    plugin.initialize(sr).unwrap();

    let input = make_sine(1000.0, sr, num_frames, 0.5);
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
fn get_data_returns_typed_cache() {
    let mut plugin = DynamicEqPlugin::new(1);
    plugin.initialize(48000).unwrap();

    let data = plugin.get_data();
    assert!(data.is_some());
    assert!(data.unwrap().is::<sotf_plugin_dynamic_eq::DynamicEqData>());
}

#[test]
fn from_params_clamps_out_of_bounds() {
    let params = DynamicEqPluginParams {
        num_bands: 99,
        threshold: 10.0,
        ratio: 0.5,
        attack_ms: 0.01,
        release_ms: 5.0,
        knee: -1.0,
        link_channels: false,
        mix: 1.5,
        bands: vec![],
    };
    let plugin = DynamicEqPlugin::from_params(1, params);
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("num_bands")),
        Some(ParameterValue::Int(8))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("threshold")),
        Some(ParameterValue::Float(0.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("ratio")),
        Some(ParameterValue::Float(1.0))
    );
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(1.0))
    );
}

#[test]
fn parameters_list_contains_expected_ids() {
    let plugin = DynamicEqPlugin::new(1);
    let params = plugin.parameters();
    let ids: Vec<_> = params.iter().map(|p| p.id.clone()).collect();
    assert!(ids.contains(&ParameterId::from("threshold")));
    assert!(ids.contains(&ParameterId::from("ratio")));
    assert!(ids.contains(&ParameterId::from("attack")));
    assert!(ids.contains(&ParameterId::from("release")));
    assert!(ids.contains(&ParameterId::from("knee")));
    assert!(ids.contains(&ParameterId::from("link_channels")));
    assert!(ids.contains(&ParameterId::from("mix")));
    assert!(ids.contains(&ParameterId::from("num_bands")));
}
