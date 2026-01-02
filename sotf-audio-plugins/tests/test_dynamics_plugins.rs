// Integration tests for dynamics processing plugins

use sotf_plugins::{
    CompressorPlugin, GatePlugin, InPlacePlugin, InPlacePluginAdapter, LimiterPlugin, PluginHost,
};

#[test]
fn test_compressor_basic() {
    let mut host = PluginHost::new(2, 48000);

    // Add compressor: -20dB threshold, 4:1 ratio
    let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(compressor)))
        .unwrap();

    // Test with loud signal (should be compressed)
    let input = vec![0.8; 2048 * 2]; // Stereo, loud signal
    let mut output = vec![0.0; 2048 * 2];

    host.process(&input, &mut output).unwrap();

    // Output should be attenuated
    let input_rms: f32 = input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32;
    let output_rms: f32 = output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32;

    assert!(output_rms < input_rms, "Compressor should reduce RMS level");
    println!(
        "Compressor: Input RMS = {:.4}, Output RMS = {:.4}",
        input_rms.sqrt(),
        output_rms.sqrt()
    );
}

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
fn test_gate_silences_quiet_signals() {
    let mut host = PluginHost::new(2, 48000);

    // Add gate at -40dB
    let gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(gate)))
        .unwrap();

    // Test with quiet signal (should be gated)
    let quiet_level = 0.001; // About -60dB
    let input = vec![quiet_level; 2048 * 2];
    let mut output = vec![0.0; 2048 * 2];

    host.process(&input, &mut output).unwrap();

    // Output should be more attenuated than input
    let input_rms: f32 = input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32;
    let output_rms: f32 = output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32;

    assert!(
        output_rms < input_rms,
        "Gate should attenuate quiet signals"
    );
    println!(
        "Gate: Input RMS = {:.6}, Output RMS = {:.6}",
        input_rms.sqrt(),
        output_rms.sqrt()
    );
}

#[test]
fn test_gate_passes_loud_signals() {
    let mut host = PluginHost::new(2, 48000);

    // Add gate at -40dB
    let gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(gate)))
        .unwrap();

    // Test with loud signal (should pass through)
    let loud_level = 0.5; // About -6dB, well above threshold
    let input = vec![loud_level; 2048 * 2];
    let mut output = vec![0.0; 2048 * 2];

    host.process(&input, &mut output).unwrap();

    // Output should be similar to input (gate is open)
    let input_rms: f32 = input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32;
    let output_rms: f32 = output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32;

    // Allow some difference due to attack time
    let rms_ratio = output_rms / input_rms;
    assert!(
        rms_ratio > 0.8,
        "Gate should pass loud signals: ratio = {}",
        rms_ratio
    );
    println!(
        "Gate (loud): Input RMS = {:.4}, Output RMS = {:.4}, Ratio = {:.2}",
        input_rms.sqrt(),
        output_rms.sqrt(),
        rms_ratio
    );
}

#[test]
fn test_dynamics_chain() {
    // Test a full dynamics processing chain: Gate -> Compressor -> Limiter
    let mut host = PluginHost::new(2, 48000);

    // Add gate to remove noise
    let gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(gate)))
        .unwrap();

    // Add compressor for dynamic range control
    let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 6.0); // +6dB makeup gain
    host.add_plugin(Box::new(InPlacePluginAdapter::new(compressor)))
        .unwrap();

    // Add limiter for peak control (hard limiting)
    let limiter = LimiterPlugin::new(2, -0.1, 50.0, 5.0, false);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(limiter)))
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

#[test]
fn test_compressor_parameters() {
    use sotf_plugins::{ParameterId, ParameterValue};

    let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
    compressor.initialize(48000).unwrap();

    // Test parameter queries
    let params = compressor.parameters();
    assert_eq!(params.len(), 10);

    // Modify threshold
    compressor
        .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
        .unwrap();

    let threshold = compressor.get_parameter(&ParameterId::from("threshold"));
    assert_eq!(threshold, Some(ParameterValue::Float(-30.0)));

    // Modify ratio
    compressor
        .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(8.0))
        .unwrap();

    let ratio = compressor.get_parameter(&ParameterId::from("ratio"));
    assert_eq!(ratio, Some(ParameterValue::Float(8.0)));
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
