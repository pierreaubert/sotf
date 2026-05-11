// Integration tests for Limiter plugin

use sotf_host::{InPlacePlugin, InPlacePluginAdapter, PluginHost};
use sotf_plugin_limiter::LimiterPlugin;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::ProcessContext;

#[test]
fn test_limiter_prevents_clipping() {
    let mut host = PluginHost::new(2, 48000);

    // Add limiter at -0.1dB (hard limiting)
    let limiter = LimiterPlugin::new(2, -0.1, 50.0, 5.0, false);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(limiter)))
        .unwrap();

    // Test with signal that would clip
    let mut input = vec![0.0; 2048 * 2];
    for i in 0..2048 {
        input[i * 2] = (i as f32 * 0.01).sin() * 1.5; // Would exceed 1.0
        input[i * 2 + 1] = (i as f32 * 0.015).cos() * 1.5;
    }
    let mut output = vec![0.0; 2048 * 2];

    host.process(&input, &mut output).unwrap();

    // All output samples should be <= 1.0
    let max_output = output.iter().map(|&x| x.abs()).fold(0.0_f32, f32::max);
    let threshold_linear = 10.0_f32.powf(-0.1 / 20.0); // -0.1dB in linear

    assert!(
        max_output <= 1.0,
        "Limiter should prevent clipping: max = {}",
        max_output
    );
    println!(
        "Limiter: Max output = {:.4} (threshold = {:.4})",
        max_output, threshold_linear
    );
}

#[test]
fn test_limiter_soft_mode() {
    let mut host = PluginHost::new(2, 48000);

    // Add soft limiter at -0.1dB
    let limiter = LimiterPlugin::new(2, -0.1, 50.0, 5.0, true);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(limiter)))
        .unwrap();

    // Test with signal that would clip
    let mut input = vec![0.0; 2048 * 2];
    for i in 0..2048 {
        input[i * 2] = (i as f32 * 0.01).sin() * 1.5; // Would exceed 1.0
        input[i * 2 + 1] = (i as f32 * 0.015).cos() * 1.5;
    }
    let mut output = vec![0.0; 2048 * 2];

    host.process(&input, &mut output).unwrap();

    // All output samples should be <= 1.0 (soft limiter still respects threshold)
    let max_output = output.iter().map(|&x| x.abs()).fold(0.0_f32, f32::max);

    assert!(
        max_output <= 1.0,
        "Soft limiter should still prevent clipping: max = {}",
        max_output
    );

    // Soft limiter should produce smoother output (less harsh than hard limiter)
    // We can verify by checking that output is more continuous
    println!("Soft limiter: Max output = {:.4}", max_output);
}

#[test]
fn test_feed_forward_lookahead_tracks_peak() {
    let mut plugin = LimiterPlugin::new(
        2,   // stereo
        -1.0, // threshold
        50.0, // release
        5.0,  // 5 ms lookahead (~240 samples @ 48k)
        false,
    );
    plugin.initialize(48000).unwrap();
    plugin.set_parameter(
        ParameterId::from("feed_forward"),
        ParameterValue::Bool(true),
    ).unwrap();

    // Build a buffer with a loud transient in the middle.
    let mut buffer = vec![0.0f32; 512 * 2];
    for i in 0..512 {
        let amp = if i == 200 { 2.0 } else { 0.1 };
        buffer[i * 2] = amp;
        buffer[i * 2 + 1] = amp;
    }

    let context = ProcessContext {
        num_frames: 512,
        sample_rate: 48000,
    };
    plugin.process_in_place(&mut buffer, &context).unwrap();

    // With feed-forward, the limiter should have started reducing gain
    // BEFORE the transient arrives, so the peak should be clamped.
    let max_out = buffer.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
    // Threshold is -1 dB ≈ 0.891. Allow some overshoot for ISP/attack.
    assert!(
        max_out < 1.0,
        "Feed-forward should pre-emptively limit the transient, max_out={}",
        max_out
    );
}

#[test]
fn test_more_than_32_channels() {
    let mut plugin = LimiterPlugin::new(
        48,   // 48 channels
        -1.0,
        50.0,
        1.0,
        false,
    );
    plugin.initialize(48000).unwrap();

    // All 48 channels should be analyzed, not just the first 32.
    let mut buffer = vec![0.0f32; 64 * 48];
    // Put a peak on channel 40 (beyond the old 32-channel cap)
    for frame in 0..64 {
        buffer[frame * 48 + 40] = 2.0;
    }

    let context = ProcessContext {
        num_frames: 64,
        sample_rate: 48000,
    };
    plugin.process_in_place(&mut buffer, &context).unwrap();

    // Channel 40 should have been limited.
    let ch40_max = (0..64)
        .map(|f| buffer[f * 48 + 40].abs())
        .fold(0.0f32, f32::max);
    assert!(
        ch40_max < 1.0,
        "Channel 40 (beyond old 32-channel cap) should be limited, got {}",
        ch40_max
    );
}
