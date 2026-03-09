#![allow(clippy::field_reassign_with_default)]
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_xtc::{XtcPlugin, XtcPluginParams};
use std::f32::consts::PI;

#[test]
fn test_xtc_saturation_fix() {
    let fft_size = 1024;
    let sample_rate = 48000;
    let mut params = XtcPluginParams::default();
    params.fft_size = fft_size;
    params.auto_gain_enabled = true;
    params.max_gain_db = 12.0; // Allow significant filter gain
    params.auto_gain_max_db = 24.0; // Allow significant reduction

    let mut plugin = XtcPlugin::new(params, sample_rate).unwrap();
    plugin.initialize(sample_rate).unwrap();

    // High-amplitude mono signal (should trigger saturation if not compensated)
    let num_blocks = 100; // Give auto-gain time to settle
    let num_frames = num_blocks * fft_size;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let phase = 2.0 * PI * 1000.0 * i as f32 / sample_rate as f32;
        let sample = phase.sin() * 0.9; // Near full scale
        input[i * 2] = sample;
        input[i * 2 + 1] = sample;
    }

    let mut output = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Check the last few blocks for peaks
    let skip = (num_blocks - 10) * fft_size;
    let mut max_peak = 0.0_f32;
    for i in skip..num_frames {
        max_peak = max_peak
            .max(output[i * 2].abs())
            .max(output[i * 2 + 1].abs());
    }

    println!("XTC max peak after settle: {:.4}", max_peak);

    // Peak should be below 1.0 (actually below 0.95 due to limiter)
    assert!(
        max_peak <= 0.96,
        "XTC still saturating: max_peak = {}",
        max_peak
    );
}
