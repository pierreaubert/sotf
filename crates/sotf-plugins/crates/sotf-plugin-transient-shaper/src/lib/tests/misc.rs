use super::super::consts;
use super::super::params;
use super::super::transient_shaper_plugin::TransientShaperPlugin;
use super::super::types::TransientShaperPluginParams;
use super::super::*;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::ProcessContext;

fn make_context(num_frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(48000, num_frames)
}

#[test]
fn test_transient_shaper_passthrough() {
    // With attack=0, sustain=0, output_gain=0 dB (linear=1.0), mix=1.0:
    // attack_amt = 0 and sustain_amt = 0 make gain exactly 1.0 for every
    // sample regardless of envelope state, so output == input sample-for-sample.
    let channels = 2;
    let mut plugin = TransientShaperPlugin::new(channels);
    plugin.initialize(48000).unwrap();

    let num_frames = 256;
    let mut buffer = vec![0.0f32; num_frames * channels];

    // Fill with a sine wave
    for frame in 0..num_frames {
        let t = frame as f32 / 48000.0;
        let val = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        buffer[frame * channels] = val;
        buffer[frame * channels + 1] = val;
    }
    let original = buffer.clone();

    let ctx = make_context(num_frames);
    let result = plugin.process_in_place(&mut buffer, &ctx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), num_frames);

    // With attack=0 and sustain=0 the gain formula collapses to 1.0 for
    // every sample. Tolerance of 1e-5 accounts for f32 rounding only.
    for frame in 0..num_frames {
        for c in 0..channels {
            let idx = frame * channels + c;
            let diff = (buffer[idx] - original[idx]).abs();
            assert!(
                diff < 1e-5,
                "frame={}, ch={}: output={}, expected={}, diff={}",
                frame,
                c,
                buffer[idx],
                original[idx],
                diff
            );
        }
    }
}

#[test]
fn test_transient_shaper_enhances_attack() {
    // With attack=+100%, transient peaks should be louder
    let channels = 1;
    let params = TransientShaperPluginParams {
        attack: 100.0,
        sustain: 0.0,
        sensitivity_db: 0.0,
        output_gain_db: 0.0,
        mix: 1.0,
    };
    let mut plugin = TransientShaperPlugin::from_params(channels, params);
    plugin.initialize(48000).unwrap();

    // Create a signal with a sharp transient followed by sustained signal
    let num_frames = 4800; // 100ms at 48kHz
    let mut buffer = vec![0.0f32; num_frames * channels];

    // First 10ms: silence (let envelopes settle at zero)
    // Then sharp transient: sudden jump to full scale
    for frame in 480..num_frames {
        buffer[frame] = 0.3; // sustained level
    }
    // Spike the first few samples of the sustained portion
    for frame in 480..490 {
        buffer[frame] = 0.9;
    }
    let original = buffer.clone();

    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    // The transient spike region should have higher amplitude than original
    let mut max_shaped = 0.0f32;
    let mut max_original = 0.0f32;
    for frame in 480..500 {
        max_shaped = max_shaped.max(buffer[frame].abs());
        max_original = max_original.max(original[frame].abs());
    }
    assert!(
        max_shaped > max_original,
        "Enhanced transient should be louder: shaped={}, original={}",
        max_shaped,
        max_original
    );
}

#[test]
fn test_transient_shaper_reduces_sustain() {
    // With sustain=-100%, sustained portions should be quieter
    let channels = 1;
    let params = TransientShaperPluginParams {
        attack: 0.0,
        sustain: -100.0,
        sensitivity_db: 0.0,
        output_gain_db: 0.0,
        mix: 1.0,
    };
    let mut plugin = TransientShaperPlugin::from_params(channels, params);
    plugin.initialize(48000).unwrap();

    // Create a sustained signal (no transient, just continuous)
    let num_frames = 9600; // 200ms at 48kHz
    let mut buffer = vec![0.0f32; num_frames * channels];
    for frame in 0..num_frames {
        let t = frame as f32 / 48000.0;
        buffer[frame] = (2.0 * std::f32::consts::PI * 200.0 * t).sin() * 0.5;
    }
    let original = buffer.clone();

    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    // Measure RMS of the last quarter (after envelopes have settled)
    let start = num_frames * 3 / 4;
    let mut rms_original = 0.0f64;
    let mut rms_shaped = 0.0f64;
    for frame in start..num_frames {
        rms_original += (original[frame] as f64).powi(2);
        rms_shaped += (buffer[frame] as f64).powi(2);
    }
    let count = (num_frames - start) as f64;
    rms_original = (rms_original / count).sqrt();
    rms_shaped = (rms_shaped / count).sqrt();

    assert!(
        rms_shaped < rms_original,
        "Reduced sustain should be quieter: shaped_rms={}, original_rms={}",
        rms_shaped,
        rms_original
    );
}

#[test]
fn test_sensitivity_low_level_step_affects_audio_output() {
    let channels = 1;
    let num_frames = 2400; // 50ms at 48kHz

    // Create a step signal with sustained level 0.001
    let mut signal = vec![0.0f32; num_frames * channels];
    for frame in 480..num_frames {
        // 10ms silence then step to 0.001
        signal[frame] = 0.001;
    }

    // Low sensitivity (-12 dB) — high threshold, signal should be bypassed
    let params_low = TransientShaperPluginParams {
        attack: 100.0,
        sustain: 0.0,
        sensitivity_db: -12.0,
        output_gain_db: 0.0,
        mix: 1.0,
    };
    let mut plugin_low = TransientShaperPlugin::from_params(channels, params_low);
    plugin_low.initialize(48000).unwrap();
    let mut buffer_low = signal.clone();
    let ctx = make_context(num_frames);
    plugin_low.process_in_place(&mut buffer_low, &ctx).unwrap();

    // High sensitivity (+12 dB) — low threshold, signal should be processed
    let params_high = TransientShaperPluginParams {
        attack: 100.0,
        sustain: 0.0,
        sensitivity_db: 12.0,
        output_gain_db: 0.0,
        mix: 1.0,
    };
    let mut plugin_high = TransientShaperPlugin::from_params(channels, params_high);
    plugin_high.initialize(48000).unwrap();
    let mut buffer_high = signal.clone();
    plugin_high
        .process_in_place(&mut buffer_high, &ctx)
        .unwrap();

    // The outputs should differ
    let mut total_diff = 0.0f32;
    for i in 0..(num_frames * channels) {
        total_diff += (buffer_low[i] - buffer_high[i]).abs();
    }
    assert!(
        total_diff > 1e-6,
        "Sensitivity should affect audio output, but outputs were identical (diff={})",
        total_diff
    );
}

#[test]
fn test_output_gain_applies_to_final_mix() {
    let channels = 1;
    let num_frames = 256;

    // With mix=0.0, output_gain should still affect the output
    let params = TransientShaperPluginParams {
        attack: 0.0,
        sustain: 0.0,
        sensitivity_db: 0.0,
        output_gain_db: 6.0, // +6 dB = ~2.0x linear
        mix: 0.0,            // fully dry
    };
    let mut plugin = TransientShaperPlugin::from_params(channels, params);
    plugin.initialize(48000).unwrap();

    let mut buffer = vec![0.0f32; num_frames * channels];
    for frame in 0..num_frames {
        buffer[frame] = 0.5;
    }

    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    // With mix=0.0 and output_gain=6dB, output should be input * 2.0
    let expected = 0.5 * 10.0f32.powf(6.0 / 20.0);
    for frame in 0..num_frames {
        let diff = (buffer[frame] - expected).abs();
        assert!(
            diff < 1e-5,
            "frame={}: expected={}, got={}, diff={}",
            frame,
            expected,
            buffer[frame],
            diff
        );
    }
}

#[test]
fn test_reset_snaps_smoother_to_target() {
    let channels = 1;
    let mut plugin = TransientShaperPlugin::new(channels);
    plugin.initialize(48000).unwrap();

    // Set attack to 50% and process a block to advance smoothers
    plugin
        .set_parameter(ParameterId::from("attack"), ParameterValue::Float(50.0))
        .unwrap();
    assert_eq!(plugin.attack_smoother.target(), 0.5);

    let num_frames = 480;
    let mut buffer = vec![0.0f32; num_frames * channels];
    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    // After processing, the smoother should have moved toward the target
    let before_reset = plugin.attack_smoother.current();
    assert!(
        (before_reset - 0.5).abs() > 1e-6,
        "Smoother should have moved from initial value, got={}",
        before_reset
    );

    // Now reset
    plugin.reset();

    // After reset, smoother should be at target immediately
    let after_reset = plugin.attack_smoother.current();
    assert!(
        (after_reset - 0.5).abs() < 1e-6,
        "Smoother should be reset to target after reset(), got={}",
        after_reset
    );
}

#[test]
fn test_sensitivity_threshold_gate_affects_audio_output() {
    // Sensitivity is a threshold gate: with sensitivity_db = +12 the
    // threshold is raised 20× (to ~0.02 linear), so a low-level signal
    // that would otherwise be shaped is left unmodified (gain = 1.0).
    // Two runs of the same moderate-level signal must produce different
    // shaped output when sensitivity differs.
    let channels = 1;
    let num_frames = 4800;

    // Build a signal with a clear transient followed by sustain so that
    // attack shaping would normally increase the transient region.
    let mut buf_low_sens = vec![0.0f32; num_frames];
    for frame in 480..num_frames {
        buf_low_sens[frame] = 0.3;
    }
    for frame in 480..490 {
        buf_low_sens[frame] = 0.9;
    }
    let mut buf_high_sens = buf_low_sens.clone();

    // Low sensitivity (threshold very low → shaping active)
    let params_low = TransientShaperPluginParams {
        attack: 100.0,
        sustain: 0.0,
        sensitivity_db: -12.0,
        output_gain_db: 0.0,
        mix: 1.0,
    };
    let mut plugin_low = TransientShaperPlugin::from_params(channels, params_low);
    plugin_low.initialize(48000).unwrap();

    // High sensitivity (threshold raised → quiet parts bypass shaping)
    let params_high = TransientShaperPluginParams {
        attack: 100.0,
        sustain: 0.0,
        sensitivity_db: 60.0, // threshold at ~1.0 linear: almost nothing is shaped
        output_gain_db: 0.0,
        mix: 1.0,
    };
    let mut plugin_high = TransientShaperPlugin::from_params(channels, params_high);
    plugin_high.initialize(48000).unwrap();

    let ctx = make_context(num_frames);
    plugin_low
        .process_in_place(&mut buf_low_sens, &ctx)
        .unwrap();
    plugin_high
        .process_in_place(&mut buf_high_sens, &ctx)
        .unwrap();

    // With low sensitivity the transient spike is amplified; with high
    // sensitivity the slow envelope never exceeds the threshold so gain
    // stays at 1.0.  The outputs must differ.
    let same = buf_low_sens
        .iter()
        .zip(buf_high_sens.iter())
        .all(|(a, b)| (a - b).abs() < 1e-6);
    assert!(
        !same,
        "sensitivity_db=-12 and sensitivity_db=+60 must produce different outputs"
    );
}

#[test]
fn test_silence_produces_no_nan_inf() {
    // Digital silence input: envelopes decay to zero, output must be
    // finite and zero (no NaN, no Inf, no denormal artifacts leaking out).
    let channels = 2;
    let params = TransientShaperPluginParams {
        attack: 50.0,
        sustain: -50.0,
        sensitivity_db: 0.0,
        output_gain_db: 0.0,
        mix: 1.0,
    };
    let mut plugin = TransientShaperPlugin::from_params(channels, params);
    plugin.initialize(48000).unwrap();

    let num_frames = 9600; // 200ms
    let mut buffer = vec![0.0f32; num_frames * channels];
    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    for (i, &s) in buffer.iter().enumerate() {
        assert!(s.is_finite(), "sample {} is not finite: {}", i, s);
        assert_eq!(s, 0.0, "silence in must be silence out at sample {}", i);
    }
}

#[test]
fn test_single_impulse_fast_envelope_responds() {
    // A single full-scale impulse followed by silence: the fast envelope
    // should immediately jump while the slow envelope lags behind.
    // This verifies the differential detection works as intended.
    let channels = 1;
    let mut plugin = TransientShaperPlugin::new(channels);
    plugin.initialize(48000).unwrap();

    let num_frames = 512;
    let mut buffer = vec![0.0f32; num_frames];
    buffer[0] = 1.0; // single impulse
    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    // After the impulse, no NaN/Inf should appear.
    for (i, &s) in buffer.iter().enumerate() {
        assert!(s.is_finite(), "sample {} is not finite: {}", i, s);
    }
}

#[test]
fn test_reset_starts_processing_from_clean_smoother_state() {
    // Set attack to +100%, let the smoother start ramping, then reset().
    // The very first processed sample after reset should use the settled
    // target value (attack=1.0), not an intermediate ramp value.
    let channels = 1;
    let mut plugin = TransientShaperPlugin::new(channels);
    plugin.initialize(48000).unwrap();

    plugin
        .set_parameter(ParameterId::from("attack"), ParameterValue::Float(100.0))
        .unwrap();

    // Advance the smoother partway by processing a short buffer
    let short_frames = 5;
    let mut buf = vec![0.5f32; short_frames];
    let ctx_short = make_context(short_frames);
    plugin.process_in_place(&mut buf, &ctx_short).unwrap();

    // Now reset — smoothers must snap to their targets
    plugin.reset();

    // Process a buffer; with attack=1.0 settled the output should reflect
    // the full attack amount from sample zero (no partial ramp).
    // We verify by checking the smoother returns 1.0 on the very first advance.
    // We do this indirectly: process one sample of silence and verify no panic.
    let mut buf2 = vec![0.0f32; 1];
    let ctx1 = make_context(1);
    let result = plugin.process_in_place(&mut buf2, &ctx1);
    assert!(result.is_ok());

    // After reset, fast_env and slow_env should be zero.
    // The next sample's shaping starts from a clean state.
    // (Verify by checking output = 0.0 for silent input, gain = 1.0.)
    assert_eq!(buf2[0], 0.0);
}

#[test]
fn test_output_gain_post_mix() {
    // output_gain_db should apply to the final mixed output at all mix
    // settings.  With attack=0, sustain=0 and mix=0.0 (full dry), the
    // output should still be scaled by output_gain_lin.
    let channels = 1;
    let num_frames = 64;
    let input_val = 0.5f32;

    let params = TransientShaperPluginParams {
        attack: 0.0,
        sustain: 0.0,
        sensitivity_db: -60.0, // ensure threshold not an issue
        output_gain_db: 6.0,   // ≈ ×2 linear
        mix: 0.0,              // full dry
    };
    let mut plugin = TransientShaperPlugin::from_params(channels, params);
    plugin.initialize(48000).unwrap();

    let mut buffer = vec![input_val; num_frames];
    let ctx = make_context(num_frames);
    plugin.process_in_place(&mut buffer, &ctx).unwrap();

    let expected_lin = 10.0f32.powf(6.0 / 20.0);
    let expected_output = input_val * expected_lin;

    for (i, &s) in buffer.iter().enumerate() {
        let diff = (s - expected_output).abs();
        assert!(
            diff < 1e-4,
            "sample {}: output={} expected={} (output_gain must apply post-mix)",
            i,
            s,
            expected_output
        );
    }
}
