#![allow(clippy::field_reassign_with_default)]
use sotf_plugins::{
    Plugin, ProcessContext, UpmixerPlugin, UpmixerPluginParams, XtcPlugin, XtcPluginParams,
};
use std::f32::consts::PI;

#[test]
fn test_xtc_stft_roundtrip_gain() {
    let fft_size = 1024;
    let sample_rate = 48000;
    let mut params = XtcPluginParams::default();
    params.fft_size = fft_size;
    params.bypass_xtc_filters = true; // Test OLA framework only
    params.auto_gain_enabled = false; // Disable auto-gain for pure OLA test

    let mut plugin = XtcPlugin::new(params, sample_rate).unwrap();
    plugin.initialize(sample_rate).unwrap();

    let num_frames = 8192;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let phase = 2.0 * PI * 1000.0 * i as f32 / sample_rate as f32;
        input[i * 2] = phase.sin() * 0.5;
        input[i * 2 + 1] = phase.cos() * 0.5;
    }
    let mut output = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Skip latency (fft_size - hop_size)
    let skip = fft_size;
    let mut max_diff = 0.0_f32;
    for i in skip..num_frames - skip {
        let diff_l = (output[i * 2] - input[i * 2]).abs();
        let diff_r = (output[i * 2 + 1] - input[i * 2 + 1]).abs();
        max_diff = max_diff.max(diff_l).max(diff_r);
    }

    assert!(
        max_diff < 1e-3,
        "XTC STFT roundtrip gain mismatch: max_diff = {}",
        max_diff
    );
}

#[test]
fn test_upmixer_stft_roundtrip_gain() {
    let fft_size = 2048;
    let sample_rate = 48000;
    let mut params = UpmixerPluginParams::default();
    params.fft_size = fft_size;
    params.bypass_all_processing = true; // Test OLA framework only

    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(sample_rate).unwrap();

    let num_frames = 8192;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let phase = 2.0 * PI * 1000.0 * i as f32 / sample_rate as f32;
        input[i * 2] = phase.sin() * 0.5;
        input[i * 2 + 1] = phase.cos() * 0.5;
    }

    // Upmixer output can have many channels, but in bypass we check if original L/R are preserved
    let mut output = vec![0.0_f32; num_frames * plugin.output_channels()];

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Skip latency
    let skip = fft_size;
    let mut max_diff = 0.0_f32;
    let out_ch = plugin.output_channels();
    for i in skip..num_frames - skip {
        let diff_l = (output[i * out_ch] - input[i * 2]).abs();
        let diff_r = (output[i * out_ch + 1] - input[i * 2 + 1]).abs();
        max_diff = max_diff.max(diff_l).max(diff_r);
    }

    assert!(
        max_diff < 1e-3,
        "Upmixer STFT roundtrip gain mismatch: max_diff = {}",
        max_diff
    );
}
