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

#[test]
fn test_gate_hysteresis_prevents_chatter() {
    // With hysteresis=4dB and threshold=-20dB:
    // - Open threshold = -20dB
    // - Close threshold = -24dB
    // A signal oscillating between -22dB and -18dB should not cause rapid
    // open/close transitions. With hysteresis, once open (at -18dB > -20dB),
    // the gate stays open until signal drops below -24dB (which -22dB does not).
    use sotf_host::{InPlacePlugin, ParameterId, ParameterValue, ProcessContext};
    use sotf_plugin_gate::GatePlugin;

    let sr = 48000u32;
    let mut gate = GatePlugin::new(1, -20.0, 1.0, 0.1, 0.0, 10.0);
    gate.initialize(sr).unwrap();

    // Set hysteresis to 4dB
    gate.set_parameter(
        ParameterId::from("hysteresis_db"),
        ParameterValue::Float(4.0),
    )
    .unwrap();

    // Generate 1s of signal that alternates between -18dB and -22dB every 100ms
    let num_frames = sr as usize;
    let amp_high = 10.0f32.powf(-18.0 / 20.0); // -18dB
    let amp_low = 10.0f32.powf(-22.0 / 20.0);  // -22dB
    let switch_period = sr as usize / 10; // 100ms

    let mut buffer = vec![0.0f32; num_frames];
    for (i, sample) in buffer.iter_mut().enumerate() {
        let cycle = i / switch_period;
        let amp = if cycle.is_multiple_of(2) { amp_high } else { amp_low };
        let t = i as f32 / sr as f32;
        *sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * amp;
    }

    // Process in blocks
    let block_size = 1024;
    let _ctx = ProcessContext {
        sample_rate: sr,
        num_frames: block_size,
    };
    for pos in (0..num_frames).step_by(block_size) {
        let end = (pos + block_size).min(num_frames);
        let nf = end - pos;
        let c = ProcessContext {
            sample_rate: sr,
            num_frames: nf,
        };
        gate.process_in_place(&mut buffer[pos..end], &c).unwrap();
    }

    // Count near-zero crossings (rapid gate transitions)
    // With hysteresis working, the output should be smooth — not rapidly alternating
    // between gated (near-zero) and open (signal). Count frames that are near-zero
    // in the second half (after gate has settled).
    let second_half = &buffer[num_frames / 2..];
    let near_zero_frames = second_half
        .iter()
        .filter(|&&s| s.abs() < 0.001)
        .count();

    // With proper hysteresis, the gate should stay mostly open (since -22dB > -24dB close threshold).
    // Near-zero frames should be a small fraction of the total.
    let zero_fraction = near_zero_frames as f32 / second_half.len() as f32;
    assert!(
        zero_fraction < 0.3,
        "With hysteresis, gate should stay mostly open. Near-zero fraction: {zero_fraction:.2}"
    );
}
