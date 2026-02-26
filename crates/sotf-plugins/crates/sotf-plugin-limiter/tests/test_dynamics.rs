// Integration tests for Limiter plugin

use sotf_host::{InPlacePluginAdapter, PluginHost};
use sotf_plugin_limiter::LimiterPlugin;

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
