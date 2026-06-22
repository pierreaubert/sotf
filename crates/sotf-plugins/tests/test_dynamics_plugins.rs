// Multi-plugin dynamics chain test (uses Gate + Compressor + Limiter)

use sotf_plugins::{
    CompressorPlugin, GatePlugin, LimiterPlugin, ParametricInPlacePluginAdapter, PluginHost,
};

#[test]
fn test_dynamics_chain() {
    // Test a full dynamics processing chain: Gate -> Compressor -> Limiter
    let mut host = PluginHost::new(2, 48000);

    // Add gate to remove noise
    let gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
    host.add_plugin(Box::new(ParametricInPlacePluginAdapter::new(gate)))
        .unwrap();

    // Add compressor for dynamic range control
    let compressor = CompressorPlugin::new(2); // +6dB makeup gain
    host.add_plugin(Box::new(ParametricInPlacePluginAdapter::new(compressor)))
        .unwrap();

    // Add limiter for peak control (hard limiting)
    let limiter = LimiterPlugin::new(2, -0.1, 50.0, 5.0, false);
    host.add_plugin(Box::new(ParametricInPlacePluginAdapter::new(limiter)))
        .unwrap();

    // Create a signal with varying dynamics
    let mut input = vec![0.0; 2048 * 2];
    for i in 0..2048 {
        let t = i as f32 / 48000.0;
        let envelope = if i < 512 {
            0.001 // Quiet start (should be gated)
        } else if i < 1024 {
            0.5 // Medium level
        } else {
            0.9 // Loud (should be compressed and limited)
        };
        input[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * envelope;
        input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * envelope;
    }
    let mut output = vec![0.0; 2048 * 2];

    host.process(&input, &mut output).unwrap();

    // Check that output is controlled
    let max_output = output.iter().map(|&x| x.abs()).fold(0.0_f32, f32::max);
    assert!(
        max_output <= 1.0,
        "Full chain should prevent clipping: max = {}",
        max_output
    );

    // Check that quiet part is attenuated
    let quiet_rms: f32 = (0..512)
        .map(|i| {
            let s0 = output[i * 2];
            let s1 = output[i * 2 + 1];
            s0 * s0 + s1 * s1
        })
        .sum::<f32>()
        / (512 * 2) as f32;

    // Check that loud part is compressed
    let loud_rms: f32 = (1024..2048)
        .map(|i| {
            let s0 = output[i * 2];
            let s1 = output[i * 2 + 1];
            s0 * s0 + s1 * s1
        })
        .sum::<f32>()
        / (1024 * 2) as f32;

    println!(
        "Full chain: Max = {:.4}, Quiet RMS = {:.6}, Loud RMS = {:.4}",
        max_output,
        quiet_rms.sqrt(),
        loud_rms.sqrt()
    );

    assert!(
        quiet_rms < loud_rms,
        "Dynamics chain should preserve some dynamic range"
    );
}
