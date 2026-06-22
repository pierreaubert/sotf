//! Integration tests for the SOTF Spectral Compressor plugin.
//!
//! These tests exercise the public `InPlacePlugin` API as a black box.

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_spectral_compressor::{SpectralCompressorPlugin, SpectralCompressorPluginParams};

fn ctx(sample_rate: u32, num_frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(sample_rate, num_frames)
}

fn make_sine(sample_rate: u32, freq: f32, frames: usize, channels: usize) -> Vec<f32> {
    let mut buf = Vec::with_capacity(frames * channels);
    for i in 0..frames {
        let t = i as f32 / sample_rate as f32;
        let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.95;
        for _ in 0..channels {
            buf.push(sample);
        }
    }
    buf
}

#[test]
fn info_and_channels() {
    let params = SpectralCompressorPluginParams::default();
    let plugin = SpectralCompressorPlugin::from_params(2, params);

    let info = plugin.info();
    assert_eq!(info.name, "Spectral Compressor");
    assert_eq!(info.version, "1.0.0");
    assert_eq!(info.author, "Sotf");

    assert_eq!(plugin.channels(), 2);
    assert_eq!(plugin.input_channels(), 2);

    let params = plugin.parameters();
    assert!(!params.is_empty());
    assert!(
        params
            .iter()
            .any(|p| p.id == ParameterId::from("threshold"))
    );
    assert!(params.iter().any(|p| p.id == ParameterId::from("mix")));
}

#[test]
fn parameter_roundtrip() {
    let mut plugin =
        SpectralCompressorPlugin::from_params(2, SpectralCompressorPluginParams::default());

    plugin
        .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
        .expect("valid threshold");
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("threshold")),
        Some(ParameterValue::Float(-30.0))
    );

    plugin
        .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(4.0))
        .expect("valid ratio");
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("ratio")),
        Some(ParameterValue::Float(4.0))
    );

    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
        .expect("valid mix");
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.5))
    );

    plugin
        .set_parameter(
            ParameterId::from("target_mode"),
            ParameterValue::String("Tonal".to_string()),
        )
        .expect("valid target mode");
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("target_mode")),
        Some(ParameterValue::String("Tonal".to_string()))
    );

    // Unknown parameter returns an error through the public API
    assert!(
        plugin
            .set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
            .is_err(),
        "unknown parameter should fail"
    );
}

#[test]
fn initialize_changes_sample_rate() {
    let mut plugin =
        SpectralCompressorPlugin::from_params(2, SpectralCompressorPluginParams::default());
    plugin.initialize(44_100).expect("initialize succeeds");

    let num_frames = 4_096;
    let mut buffer = vec![0.0f32; num_frames * plugin.channels()];
    plugin
        .process_in_place(&mut buffer, &ctx(44_100, num_frames))
        .unwrap();
}

#[test]
fn process_silence() {
    let mut plugin =
        SpectralCompressorPlugin::from_params(2, SpectralCompressorPluginParams::default());
    let num_frames = 4_096;
    let mut buffer = vec![0.0f32; num_frames * plugin.channels()];

    let frames = plugin
        .process_in_place(&mut buffer, &ctx(48_000, num_frames))
        .expect("process succeeds");
    assert_eq!(frames, num_frames);

    assert!(
        buffer.iter().all(|s| s.is_finite()),
        "silent input must produce finite output"
    );
    assert!(
        buffer.iter().all(|s| s.abs() < 1e-6),
        "silent input should remain silent"
    );
}

#[test]
fn mix_zero_passthrough() {
    let mut params = SpectralCompressorPluginParams::default();
    params.mix = 0.0;
    let mut plugin = SpectralCompressorPlugin::from_params(2, params);
    plugin
        .set_parameter(
            ParameterId::from("delta_listen"),
            ParameterValue::Bool(false),
        )
        .unwrap();

    let num_frames = 4_096;
    let mut buffer = make_sine(48_000, 1_000.0, num_frames, plugin.channels());
    let expected = buffer.clone();

    plugin
        .process_in_place(&mut buffer, &ctx(48_000, num_frames))
        .unwrap();

    assert!(buffer.iter().all(|s| s.is_finite()));
    for (got, want) in buffer.iter().zip(expected.iter()) {
        assert!(
            (got - want).abs() < 1e-5,
            "mix=0 should pass the dry signal through unchanged"
        );
    }
}

#[test]
fn compression_reduces_loud_signal() {
    let mut plugin =
        SpectralCompressorPlugin::from_params(2, SpectralCompressorPluginParams::default());
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-40.0))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(10.0))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("attack"), ParameterValue::Float(0.1))
        .unwrap();
    plugin
        .set_parameter(ParameterId::from("release"), ParameterValue::Float(10.0))
        .unwrap();

    let num_frames = 8_192;
    let mut buffer = make_sine(48_000, 1_000.0, num_frames, plugin.channels());

    plugin
        .process_in_place(&mut buffer, &ctx(48_000, num_frames))
        .unwrap();

    assert!(buffer.iter().all(|s| s.is_finite()));

    let skip = plugin.latency_samples().max(512);
    let tail_peak = buffer[skip * plugin.channels()..]
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    assert!(
        tail_peak < 0.2,
        "a loud steady sine should be heavily reduced; got peak {tail_peak}"
    );
}

#[test]
fn delta_listen() {
    let mut plugin =
        SpectralCompressorPlugin::from_params(2, SpectralCompressorPluginParams::default());
    plugin
        .set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("delta_listen"),
            ParameterValue::Bool(true),
        )
        .unwrap();

    let num_frames = 4_096;
    let input = make_sine(48_000, 1_000.0, num_frames, plugin.channels());
    let mut buffer = input.clone();

    plugin
        .process_in_place(&mut buffer, &ctx(48_000, num_frames))
        .unwrap();

    assert!(buffer.iter().all(|s| s.is_finite()));
    // The delta output should not be identical to the original input.
    assert_ne!(
        buffer, input,
        "delta listen should return a processed difference signal"
    );
}

#[test]
fn fft_size_change_updates_latency() {
    let mut plugin =
        SpectralCompressorPlugin::from_params(2, SpectralCompressorPluginParams::default());
    let default_latency = plugin.latency_samples();
    assert_eq!(default_latency, 2048 - 512);

    plugin
        .set_parameter(ParameterId::from("fft_size"), ParameterValue::Int(0))
        .unwrap();
    assert_eq!(plugin.latency_samples(), 1024 - 256);

    plugin
        .set_parameter(ParameterId::from("fft_size"), ParameterValue::Int(2))
        .unwrap();
    assert_eq!(plugin.latency_samples(), 4096 - 1024);
}

#[test]
fn reset_clears_state() {
    let mut plugin =
        SpectralCompressorPlugin::from_params(2, SpectralCompressorPluginParams::default());
    let num_frames = 4_096;
    let mut buffer = make_sine(48_000, 1_000.0, num_frames, plugin.channels());

    plugin
        .process_in_place(&mut buffer, &ctx(48_000, num_frames))
        .unwrap();
    let max_before = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(max_before > 1e-6);

    plugin.reset();

    let mut silent = vec![0.0f32; num_frames * plugin.channels()];
    plugin
        .process_in_place(&mut silent, &ctx(48_000, num_frames))
        .unwrap();
    assert!(
        silent.iter().all(|s| s.abs() < 1e-6),
        "after reset, silent input should produce silent output"
    );
}

#[test]
fn wrong_buffer_size_returns_error() {
    let mut plugin =
        SpectralCompressorPlugin::from_params(2, SpectralCompressorPluginParams::default());
    let num_frames = 64;
    let mut good_buffer = vec![0.0f32; num_frames * plugin.channels()];
    let mut bad_buffer = vec![0.0f32; num_frames * plugin.channels() - 1];

    assert!(
        plugin
            .process_in_place(&mut bad_buffer, &ctx(48_000, num_frames))
            .is_err(),
        "buffer size mismatch should fail"
    );
    assert!(
        plugin
            .process_in_place(&mut good_buffer, &ctx(48_000, num_frames))
            .is_ok(),
        "correct buffer size should succeed"
    );
}
