// Integration tests for Gate plugin

use sotf_host::{ParametricInPlacePlugin, ParametricInPlacePluginAdapter, PluginHost};
use sotf_plugin_gate::{GateData, GatePlugin};

// ---------------------------------------------------------------------------
// Bug regression tests (added in 0.5.5)
// ---------------------------------------------------------------------------

/// Attack time should control how fast the gate OPENS (not closes).
///
/// This test verifies the semantics by comparing the opening time measured
/// with a slow attack vs a fast attack:
///
///   Experiment A: slow attack = 100 ms, fast release = 1 ms.
///   Experiment B: fast attack =   1 ms, fast release = 1 ms.
///
/// Both gates start closed (300 ms silence before the loud tone).
/// We then feed a loud tone (-10 dBFS, above threshold) and measure the
/// output gain at +5 ms into the loud section.
///
/// With correct semantics (attack = open speed):
///   - Experiment A: slow open → gain very low at 5ms (barely opened).
///   - Experiment B: fast open → gain much higher at 5ms (mostly opened).
///     Condition: gain_B >> gain_A.
///
/// With reversed semantics (attack = close speed):
///   - release_coeff is used for opening, and release=1ms is fast in both.
///   - Both gates open equally fast regardless of attack_ms.
///   - gain_B ≈ gain_A.
#[test]
fn test_attack_controls_gate_open_speed() {
    use sotf_host::plugin::ProcessContext;
    let sr = 48000u32;

    let silence_frames = (0.3 * sr as f32) as usize; // 300 ms settle
    let loud_frames = (0.2 * sr as f32) as usize; // 200 ms loud

    let amp_loud = 10.0f32.powf(-10.0 / 20.0); // -10 dBFS, above -30 dB threshold

    let make_gate = |attack_ms: f32, release_ms: f32| {
        let mut g = GatePlugin::from_params(
            1,
            sotf_plugin_gate::GatePluginParams {
                threshold_db: -30.0,
                ratio: 100.0,
                attack_ms,
                hold_ms: 0.0,
                release_ms,
                mix: 1.0,
                link_channels: false,
                sidechain_hpf_hz: 0.0,
                sidechain_hpf_order: "2nd".to_string(),
                detection_mode: "peak".to_string(),
                sidechain_external: false,
                range_db: 80.0,
                hysteresis_db: 0.0,
                knee_db: 0.0,
                lookahead_ms: 0.0,
            },
        );
        g.initialize(sr).unwrap();
        g
    };

    // Build silence+loud buffer
    let total = silence_frames + loud_frames;
    let mut buf_a = vec![0.0f32; total];
    let mut buf_b = vec![0.0f32; total];
    for i in silence_frames..total {
        buf_a[i] = amp_loud;
        buf_b[i] = amp_loud;
    }
    let ctx = ProcessContext::new(sr, total);

    // Experiment A: slow attack = 100 ms
    let mut gate_a = make_gate(100.0, 1.0);
    gate_a.process_in_place(&mut buf_a, &ctx).unwrap();

    // Experiment B: fast attack = 1 ms
    let mut gate_b = make_gate(1.0, 1.0);
    gate_b.process_in_place(&mut buf_b, &ctx).unwrap();

    // Check gain at 5ms into the loud section (240 samples after transition).
    let check_offset = (0.005 * sr as f32) as usize; // 5 ms
    let check_idx = silence_frames + check_offset;
    let gain_slow_attack = buf_a[check_idx] / amp_loud;
    let gain_fast_attack = buf_b[check_idx] / amp_loud;

    // With correct semantics: fast attack opens faster → gain_fast >> gain_slow.
    // With reversed semantics: both use the same coeff (release=1ms) → similar gains.
    assert!(
        gain_fast_attack > gain_slow_attack * 5.0,
        "Fast attack (1ms) should open the gate ~5x faster than slow attack (100ms) at 5ms. \
         gain_fast={gain_fast_attack:.4} gain_slow={gain_slow_attack:.4}. \
         If gains are similar, attack_coeff and release_coeff are swapped."
    );
}

/// In linked-channel mode the monitoring `is_open` flag must reflect the
/// actual gate state (closed when signal is below threshold).
///
/// Bug: envelope[1..] stay at 0.0 (init value), so `any(a < 0.1)` is always
/// true even when channel 0 has full attenuation.
#[test]
fn test_linked_mode_is_open_false_when_gated() {
    use sotf_host::plugin::ProcessContext;

    let sr = 48000u32;
    // Linked stereo gate, threshold -30 dB.  No hold so gate closes cleanly.
    let mut gate = GatePlugin::from_params(
        2,
        sotf_plugin_gate::GatePluginParams {
            threshold_db: -30.0,
            ratio: 100.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 20.0,
            mix: 1.0,
            link_channels: true,
            sidechain_hpf_hz: 0.0,
            sidechain_hpf_order: "2nd".to_string(),
            detection_mode: "peak".to_string(),
            sidechain_external: false,
            range_db: 80.0,
            hysteresis_db: 0.0,
            knee_db: 0.0,
            lookahead_ms: 0.0,
        },
    );
    gate.initialize(sr).unwrap();

    // Feed 500 ms of silence: signal is -inf dB, well below -30 dB threshold.
    // After 500 ms the gate must be fully closed.
    let num_frames = (0.5 * sr as f32) as usize;
    let stride = 2; // 2 channels
    let mut buf = vec![0.0f32; num_frames * stride];

    // Process in 10-block chunks so the cache updater fires (updates every 10 blocks).
    let block_size = 512;
    for pos in (0..num_frames).step_by(block_size) {
        let end = (pos + block_size).min(num_frames);
        let nf = end - pos;
        let ctx = ProcessContext::new(sr, nf);
        gate.process_in_place(&mut buf[pos * stride..end * stride], &ctx)
            .unwrap();
    }

    // Read the gate's diagnostic data.
    let data_arc = gate.get_data().expect("GatePlugin must expose GateData");
    let gate_data = data_arc.downcast::<GateData>().expect("GateData downcast");

    assert!(
        !gate_data.is_open,
        "Gate (linked, 2-ch) should report is_open=false after 500 ms of silence below \
         threshold. Bug: envelope[1..] stay at 0.0 making any(a<0.1) always true."
    );
}

#[test]
fn test_gate_silences_quiet_signals() {
    let mut host = PluginHost::new(2, 48000);

    // Add gate at -40dB
    let gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
    host.add_plugin(Box::new(ParametricInPlacePluginAdapter::new(gate)))
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
    host.add_plugin(Box::new(ParametricInPlacePluginAdapter::new(gate)))
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
    use sotf_host::parameters::{ParameterId, ParameterValue};
    use sotf_host::plugin::ProcessContext;
    use sotf_plugin_gate::GatePlugin;

    let sr = 48000u32;
    let mut gate = GatePlugin::new(1, -20.0, 1.0, 0.1, 0.0, 10.0);
    gate.initialize(sr).unwrap();

    // Set hysteresis to 4dB
    gate.parametric_set_parameter(
        ParameterId::from("hysteresis_db"),
        ParameterValue::Float(4.0),
    )
    .unwrap();

    // Generate 1s of signal that alternates between -18dB and -22dB every 100ms
    let num_frames = sr as usize;
    let amp_high = 10.0f32.powf(-18.0 / 20.0); // -18dB
    let amp_low = 10.0f32.powf(-22.0 / 20.0); // -22dB
    let switch_period = sr as usize / 10; // 100ms

    let mut buffer = vec![0.0f32; num_frames];
    for (i, sample) in buffer.iter_mut().enumerate() {
        let cycle = i / switch_period;
        let amp = if cycle.is_multiple_of(2) {
            amp_high
        } else {
            amp_low
        };
        let t = i as f32 / sr as f32;
        *sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * amp;
    }

    // Process in blocks
    let block_size = 1024;
    let _ctx = ProcessContext::new(sr, block_size);
    for pos in (0..num_frames).step_by(block_size) {
        let end = (pos + block_size).min(num_frames);
        let nf = end - pos;
        let c = ProcessContext::new(sr, nf);
        gate.process_in_place(&mut buffer[pos..end], &c).unwrap();
    }

    // Count near-zero crossings (rapid gate transitions)
    // With hysteresis working, the output should be smooth — not rapidly alternating
    // between gated (near-zero) and open (signal). Count frames that are near-zero
    // in the second half (after gate has settled).
    let second_half = &buffer[num_frames / 2..];
    let near_zero_frames = second_half.iter().filter(|&&s| s.abs() < 0.001).count();

    // With proper hysteresis, the gate should stay mostly open (since -22dB > -24dB close threshold).
    // Near-zero frames should be a small fraction of the total.
    let zero_fraction = near_zero_frames as f32 / second_half.len() as f32;
    assert!(
        zero_fraction < 0.3,
        "With hysteresis, gate should stay mostly open. Near-zero fraction: {zero_fraction:.2}"
    );
}
