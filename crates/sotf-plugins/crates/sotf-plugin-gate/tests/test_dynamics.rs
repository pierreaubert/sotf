// Integration tests for Gate plugin

use sotf_host::{InPlacePluginAdapter, PluginHost};
use sotf_plugin_gate::GatePlugin;

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
