use super::crossover_mode::CrossoverMode;
use super::crossover_plugin::CrossoverPlugin;
use super::misc::is_linear_phase_type;
use super::parse::{parse_channel_freq_id, parse_channel_mode_id};
use super::per_channel_op_mode::PerChannelOpMode;
use super::types::CrossoverPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};

#[test]
fn test_crossover_basic() {
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    p.initialize(48000).unwrap();
    let input = vec![1.0; 1000];
    let mut output = vec![0.0; 1000];
    p.process(&input, &mut output, &ProcessContext::new(48000, 1000))
        .unwrap();
    assert!(output[999].is_finite());
}

#[test]
fn test_crossover_highpass() {
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "high").unwrap();
    p.initialize(48000).unwrap();
    let input = vec![1.0; 1000];
    let mut output = vec![0.0; 1000];
    p.process(&input, &mut output, &ProcessContext::new(48000, 1000))
        .unwrap();
    assert!(output[999].is_finite());
}

#[test]
fn test_linear_phase_crossover_reconstructs_delayed_input() {
    let mut p = CrossoverPlugin::new(1, "LinearPhase", 1000.0, "both").unwrap();
    p.set_parameter(ParameterId::from("fir_taps"), ParameterValue::Int(127))
        .unwrap();
    p.initialize(48000).unwrap();
    let latency = p.latency_samples();
    let frames = 512;
    let input: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.1).sin()).collect();
    let mut output = vec![0.0; frames * 2];
    p.process(&input, &mut output, &ProcessContext::new(48000, frames))
        .unwrap();

    let mut max_error = 0.0f32;
    for i in (latency + 16)..frames {
        let reconstructed = output[i * 2] + output[i * 2 + 1];
        max_error = max_error.max((reconstructed - input[i - latency]).abs());
    }
    assert!(
        max_error < 0.02,
        "linear-phase bands should reconstruct delayed input, max_error={max_error}"
    );
}

#[test]
fn test_multiband_linear_phase_crossover_reconstructs_delayed_input() {
    let mut p = CrossoverPlugin::new_multiway(1, "LinearPhase", 500.0, "both", &[5_000.0]).unwrap();
    p.set_parameter(ParameterId::from("fir_taps"), ParameterValue::Int(127))
        .unwrap();
    p.initialize(48_000).unwrap();

    let latency = p.latency_samples();
    let frames = 768;
    let input: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.1).sin()).collect();
    let mut output = vec![0.0; frames * 3];
    p.process(&input, &mut output, &ProcessContext::new(48_000, frames))
        .unwrap();

    let mut max_error = 0.0f32;
    for i in (latency + 16)..frames {
        let reconstructed = output[i * 3] + output[i * 3 + 1] + output[i * 3 + 2];
        max_error = max_error.max((reconstructed - input[i - latency]).abs());
    }
    assert!(
        max_error < 0.02,
        "multiway linear-phase bands should reconstruct delayed input, max_error={max_error}"
    );
}

#[test]
fn test_crossover_stereo() {
    let mut p = CrossoverPlugin::new(2, "LR24", 500.0, "low").unwrap();
    p.initialize(48000).unwrap();
    let input = vec![0.5; 200];
    let mut output = vec![0.0; 200];
    p.process(&input, &mut output, &ProcessContext::new(48000, 100))
        .unwrap();
    assert!(output[0].is_finite());
    assert!(output[199].is_finite());
}

#[test]
fn test_crossover_dc_passes_lowpass() {
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    p.initialize(48000).unwrap();
    let input = vec![1.0; 10000];
    let mut output = vec![0.0; 10000];
    p.process(&input, &mut output, &ProcessContext::new(48000, 10000))
        .unwrap();
    assert!(
        output[9999] > 0.9,
        "DC through lowpass should be near 1.0, got {}",
        output[9999]
    );
}

#[test]
fn test_crossover_dc_rejected_highpass() {
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "high").unwrap();
    p.initialize(48000).unwrap();
    let input = vec![1.0; 10000];
    let mut output = vec![0.0; 10000];
    p.process(&input, &mut output, &ProcessContext::new(48000, 10000))
        .unwrap();
    assert!(
        output[9999].abs() < 0.1,
        "DC through highpass should be near 0.0, got {}",
        output[9999]
    );
}

#[test]
fn test_crossover_invalid_output() {
    let result = CrossoverPlugin::new(1, "LR24", 1000.0, "invalid");
    assert!(result.is_err());
}

#[test]
fn test_crossover_both_mode_doubles_channels() {
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "both").unwrap();
    p.initialize(48000).unwrap();
    assert_eq!(p.input_channels(), 1);
    assert_eq!(p.output_channels(), 2); // 1 channel * 2 bands

    // Process DC: low band should have the signal, high should be ~0
    let num_frames = 10000;
    let input = vec![1.0f32; num_frames];
    let mut output = vec![0.0f32; num_frames * 2]; // 2 output channels
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    // Last frame: output[idx*2] = low, output[idx*2+1] = high
    let last = (num_frames - 1) * 2;
    assert!(
        output[last] > 0.9,
        "DC low band should be near 1.0, got {}",
        output[last]
    );
    assert!(
        output[last + 1].abs() < 0.1,
        "DC high band should be near 0.0, got {}",
        output[last + 1]
    );
}

#[test]
fn test_crossover_both_bands_sum_preserves_energy() {
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "both").unwrap();
    p.initialize(48000).unwrap();

    // Feed a signal and verify low + high sum has comparable energy to input.
    // LR4 crossovers sum to flat magnitude but introduce group delay,
    // so per-sample comparison with undelayed input is not valid.
    // Instead, verify RMS energy is preserved.
    let num_frames = 10000;
    let input: Vec<f32> = (0..num_frames).map(|i| (i as f32 * 0.05).sin()).collect();
    let mut output = vec![0.0f32; num_frames * 2];
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    // Compare RMS of input vs RMS of (low+high) over the settled region.
    // Use at least 5000 samples for settle to ensure the filter has fully settled.
    let settle = 5000;
    let input_rms: f32 =
        (input[settle..].iter().map(|s| s * s).sum::<f32>() / (num_frames - settle) as f32).sqrt();

    let sum_rms: f32 = ((settle..num_frames)
        .map(|f| {
            let s = output[f * 2] + output[f * 2 + 1];
            s * s
        })
        .sum::<f32>()
        / (num_frames - settle) as f32)
        .sqrt();

    let ratio = sum_rms / input_rms;
    assert!(
        (ratio - 1.0).abs() < 0.01,
        "RMS ratio should be near 1.0 (flat sum), got {}",
        ratio
    );
}

#[test]
fn test_crossover_stereo_both_mode() {
    let mut p = CrossoverPlugin::new(2, "LR24", 1000.0, "both").unwrap();
    p.initialize(48000).unwrap();
    assert_eq!(p.input_channels(), 2);
    assert_eq!(p.output_channels(), 4); // 2 channels * 2 bands

    let num_frames = 100;
    let input = vec![0.5f32; num_frames * 2];
    let mut output = vec![0.0f32; num_frames * 4];
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();
    // All outputs should be finite
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn test_crossover_3way() {
    let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
    p.initialize(48000).unwrap();
    assert_eq!(p.input_channels(), 1);
    assert_eq!(p.output_channels(), 3); // 1 channel * 3 bands

    let num_frames = 10000;
    let input = vec![1.0f32; num_frames]; // DC
    let mut output = vec![0.0f32; num_frames * 3];
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    // DC should pass through lowest band only
    let last = (num_frames - 1) * 3;
    assert!(
        output[last] > 0.9,
        "3-way DC band 0 (low) should be near 1.0, got {}",
        output[last]
    );
    assert!(
        output[last + 1].abs() < 0.1,
        "3-way DC band 1 (mid) should be near 0.0, got {}",
        output[last + 1]
    );
    assert!(
        output[last + 2].abs() < 0.1,
        "3-way DC band 2 (high) should be near 0.0, got {}",
        output[last + 2]
    );
}

#[test]
fn test_crossover_4way() {
    let mut p = CrossoverPlugin::new_multiway(1, "LR24", 200.0, "both", &[1000.0, 5000.0]).unwrap();
    p.initialize(48000).unwrap();
    assert_eq!(p.input_channels(), 1);
    assert_eq!(p.output_channels(), 4); // 1 channel * 4 bands

    let num_frames = 1000;
    let input: Vec<f32> = (0..num_frames).map(|i| (i as f32 * 0.1).sin()).collect();
    let mut output = vec![0.0f32; num_frames * 4];
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    // All outputs should be finite
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn test_crossover_3way_lowpass_mode() {
    // In lowpass mode, 3-way should output only the lowest band
    let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "low", &[5000.0]).unwrap();
    p.initialize(48000).unwrap();
    assert_eq!(p.output_channels(), 1); // Only lowest band

    let num_frames = 10000;
    let input = vec![1.0f32; num_frames];
    let mut output = vec![0.0f32; num_frames];
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    // DC passes through lowpass
    assert!(
        output[9999] > 0.9,
        "3-way lowpass DC should be near 1.0, got {}",
        output[9999]
    );
}

#[test]
fn test_crossover_output_selection_highpass_rejects_dc() {
    // Highpass mode should reject DC (output near zero)
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "high").unwrap();
    p.initialize(48000).unwrap();
    let num_frames = 10000;
    let input = vec![1.0f32; num_frames]; // DC
    let mut output = vec![0.0; num_frames];
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();
    assert!(
        output[num_frames - 1].abs() < 0.05,
        "Highpass should reject DC, got {}",
        output[num_frames - 1]
    );
}

#[test]
fn test_crossover_output_selection_lowpass_passes_dc() {
    // Lowpass mode should pass DC (output near 1.0)
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    p.initialize(48000).unwrap();
    let num_frames = 10000;
    let input = vec![1.0f32; num_frames]; // DC
    let mut output = vec![0.0; num_frames];
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();
    assert!(
        output[num_frames - 1] > 0.95,
        "Lowpass should pass DC, got {}",
        output[num_frames - 1]
    );
}

#[test]
fn test_crossover_mode_parameter() {
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    p.initialize(48000).unwrap();
    assert_eq!(p.output_channels(), 1);

    assert!(
        p.set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("both".to_string()),
        )
        .is_err()
    );
    assert_eq!(p.output_channels(), 1);

    let val = p.get_parameter(&ParameterId::from("mode"));
    assert_eq!(val, Some(ParameterValue::String("lowpass".to_string())));
}

#[test]
fn rejects_more_than_four_bands_and_invalid_frequencies() {
    assert!(
        CrossoverPlugin::new_multiway(1, "LR24", 100.0, "both", &[500.0, 2_000.0, 8_000.0])
            .is_err()
    );
    for frequency in [0.0, -1.0, f64::NAN, f64::INFINITY, 24_000.0] {
        assert!(CrossoverPlugin::new(1, "LR24", frequency, "both").is_err());
    }
    assert!(CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[500.0]).is_err());
    assert!(CrossoverPlugin::new(0, "LR24", 500.0, "both").is_err());
}

#[test]
fn process_rejects_wrong_buffer_lengths() {
    let mut plugin = CrossoverPlugin::new(2, "LR24", 1_000.0, "both").unwrap();
    plugin.initialize(48_000).unwrap();
    let context = ProcessContext::new(48_000, 4);
    for input_len in [7, 9] {
        let input = vec![0.0; input_len];
        let mut output = vec![0.0; 16];
        assert!(plugin.process(&input, &mut output, &context).is_err());
    }
    let input = vec![0.0; 8];
    for output_len in [15, 17] {
        let mut output = vec![0.0; output_len];
        assert!(plugin.process(&input, &mut output, &context).is_err());
    }
}

/// Changing frequency_2 on a 3-way crossover should not panic and should
/// continue producing finite output.
#[test]
fn test_3way_frequency_update_no_panic() {
    let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
    p.initialize(48000).unwrap();
    assert_eq!(p.output_channels(), 3); // 3 bands

    let num_frames = 2000;
    let ctx = ProcessContext::new(48000, num_frames);

    // Process a block before parameter change
    let input: Vec<f32> = (0..num_frames)
        .map(|i| 0.3 * (i as f32 * 0.1).sin())
        .collect();
    let mut output = vec![0.0f32; num_frames * 3];
    p.process(&input, &mut output, &ctx).unwrap();

    // Change frequency_2 (the second crossover point)
    p.set_parameter(
        ParameterId::from("frequency_2"),
        ParameterValue::Float(8000.0),
    )
    .unwrap();

    // Verify the parameter was accepted
    let val = p.get_parameter(&ParameterId::from("frequency_2"));
    assert_eq!(val, Some(ParameterValue::Float(8000.0)));

    // Process another block after the change -- must not panic
    let input2: Vec<f32> = (0..num_frames)
        .map(|i| 0.3 * ((num_frames + i) as f32 * 0.1).sin())
        .collect();
    let mut output2 = vec![0.0f32; num_frames * 3];
    p.process(&input2, &mut output2, &ctx).unwrap();

    // All output must be finite
    assert!(
        output2.iter().all(|s| s.is_finite()),
        "All output samples must be finite after frequency_2 change"
    );

    // At least some output should be non-zero
    let has_signal = output2.iter().any(|s| s.abs() > 1e-6);
    assert!(
        has_signal,
        "Output should contain non-zero samples after frequency change"
    );
}

/// Stable parameter IDs must never be silently rebound by sorting.
#[test]
fn test_all_frequencies_remain_sorted_after_primary_update() {
    // 3-way: [500, 5000]
    let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
    p.initialize(48000).unwrap();

    // Move primary frequency above the second point — without the fix this
    // would leave all_frequencies = [10000, 5000] (unsorted).
    assert!(
        p.set_parameter(
            ParameterId::from("frequency"),
            ParameterValue::Float(10000.0),
        )
        .is_err()
    );

    // Verify the vector is still in ascending order.
    let freqs = p.all_frequencies.clone();
    let mut sorted = freqs.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(
        freqs, sorted,
        "all_frequencies must remain sorted after primary frequency change; got {:?}",
        freqs
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("frequency")),
        Some(ParameterValue::Float(500.0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("frequency_2")),
        Some(ParameterValue::Float(5_000.0))
    );

    // Plugin must still produce finite output.
    let num_frames = 1000;
    let input: Vec<f32> = (0..num_frames).map(|i| (i as f32 * 0.1).sin()).collect();
    let num_bands = p.num_bands();
    let mut output = vec![0.0f32; num_frames * num_bands];
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();
    assert!(output.iter().all(|s| s.is_finite()));
}

/// §2.1: Setting 'frequency_2' to a value smaller than 'frequency' must
/// also maintain sorted order.
#[test]
fn test_all_frequencies_remain_sorted_after_extra_freq_update() {
    // 3-way: [500, 5000]
    let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
    p.initialize(48000).unwrap();

    // Move frequency_2 below the primary — without the fix this would leave
    // all_frequencies = [500, 200] (unsorted).
    assert!(
        p.set_parameter(
            ParameterId::from("frequency_2"),
            ParameterValue::Float(200.0),
        )
        .is_err()
    );

    let freqs = p.all_frequencies.clone();
    let mut sorted = freqs.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(
        freqs, sorted,
        "all_frequencies must remain sorted after frequency_2 change; got {:?}",
        freqs
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("frequency")),
        Some(ParameterValue::Float(500.0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("frequency_2")),
        Some(ParameterValue::Float(5_000.0))
    );
}

#[test]
fn ordinary_frequency_update_preserves_smoothing() {
    let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5_000.0]).unwrap();
    p.initialize(48_000).unwrap();
    let before = p.freq_smoother.current();

    p.set_parameter(ParameterId::from("frequency"), ParameterValue::Float(750.0))
        .unwrap();

    assert_eq!(p.freq_smoother.current(), before);
    assert_eq!(p.freq_smoother.target(), 750.0);
}

/// §2.2: "frequency_1" must NOT be parsed as a valid extra-freq parameter.
#[test]
fn test_parse_extra_freq_index_rejects_idx_less_than_2() {
    // "frequency_1" should return None — it is not a valid parameter.
    assert_eq!(CrossoverPlugin::parse_extra_freq_index("frequency_1"), None);
    assert_eq!(CrossoverPlugin::parse_extra_freq_index("frequency_0"), None);
    // "frequency_2" must still map to smoother index 0.
    assert_eq!(
        CrossoverPlugin::parse_extra_freq_index("frequency_2"),
        Some(0)
    );
    // "frequency_3" must map to smoother index 1.
    assert_eq!(
        CrossoverPlugin::parse_extra_freq_index("frequency_3"),
        Some(1)
    );
}

/// §4.1: Unsupported crossover type strings must return an error.
#[test]
fn test_unsupported_crossover_type_returns_error() {
    let result = CrossoverPlugin::new(1, "LR12", 1000.0, "low");
    assert!(
        result.is_err(),
        "LR12 crossover type must be rejected with an error"
    );
    let result2 = CrossoverPlugin::new(1, "BW18", 1000.0, "low");
    assert!(
        result2.is_err(),
        "BW18 crossover type must be rejected with an error"
    );
    // Case-insensitive acceptance of the supported types.
    assert!(CrossoverPlugin::new(1, "lr24", 1000.0, "low").is_ok());
    assert!(CrossoverPlugin::new(1, "LR4", 1000.0, "low").is_ok());
    assert!(CrossoverPlugin::new(1, "LR24", 1000.0, "low").is_ok());
}

/// §4.2: CrossoverMode::from_str must be case-insensitive (no allocation path).
#[test]
fn test_crossover_mode_from_str_is_case_insensitive() {
    assert_eq!(CrossoverMode::from_str("LOW"), Ok(CrossoverMode::Lowpass));
    assert_eq!(
        CrossoverMode::from_str("Lowpass"),
        Ok(CrossoverMode::Lowpass)
    );
    assert_eq!(CrossoverMode::from_str("HP"), Ok(CrossoverMode::Highpass));
    assert_eq!(CrossoverMode::from_str("BOTH"), Ok(CrossoverMode::Both));
}

/// §4.3: reset() must snap smoothers to their targets to avoid a
/// click on the next block when a parameter was mid-transition.
#[test]
fn test_reset_snaps_smoothers_to_target() {
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    p.initialize(48000).unwrap();

    // Start a slow parameter transition (20 ms @ 48 kHz = ~960 samples to converge).
    p.set_parameter(
        ParameterId::from("frequency"),
        ParameterValue::Float(5000.0),
    )
    .unwrap();

    // Process only a few samples so the smoother is mid-transition.
    let input = vec![0.0f32; 16];
    let mut output = vec![0.0f32; 16];
    p.process(&input, &mut output, &ProcessContext::new(48000, 16))
        .unwrap();

    // Reset must snap the smoother current to target.
    p.reset();
    let current = p.freq_smoother.current();
    let target = p.freq_smoother.target();
    assert_eq!(
        current, target,
        "After reset(), smoother current ({}) must equal target ({})",
        current, target
    );
}

/// A graph must be rebuilt with valid cutoffs when its sample rate changes.
#[test]
fn test_initialize_rejects_frequency_above_nyquist() {
    let mut p = CrossoverPlugin::new(1, "LR24", 20000.0, "low").unwrap();
    assert!(p.initialize(32000).is_err());
}

#[test]
fn test_per_channel_construction_and_output_shape() {
    let p = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![80.0, 100.0, 120.0],
        vec![
            PerChannelOpMode::Highpass,
            PerChannelOpMode::Lowpass,
            PerChannelOpMode::Mute,
        ],
    )
    .unwrap();
    assert!(p.is_per_channel());
    assert_eq!(p.input_channels(), 3);
    assert_eq!(p.output_channels(), 3);
}

#[test]
fn test_per_channel_mute_outputs_silence() {
    let mut p = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![1000.0, 1000.0],
        vec![PerChannelOpMode::Highpass, PerChannelOpMode::Mute],
    )
    .unwrap();
    p.initialize(48000).unwrap();
    let num_frames = 512;
    let mut input = vec![0.0f32; num_frames * 2];
    for f in 0..num_frames {
        input[f * 2] = 1.0; // ch0: DC
        input[f * 2 + 1] = 1.0; // ch1: DC (muted)
    }
    let mut output = vec![0.0; num_frames * 2];
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();
    // Muted channel must be exactly silence.
    for f in 0..num_frames {
        assert_eq!(output[f * 2 + 1], 0.0, "muted channel must be zero");
    }
}

#[test]
fn test_per_channel_independent_cutoffs() {
    // Two channels with different cutoffs and different modes: ch0 LP@200,
    // ch1 HP@5000. Drive both with white noise; ch0 should preserve LF
    // energy, ch1 should preserve HF energy.
    let mut p = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![200.0, 5000.0],
        vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
    )
    .unwrap();
    let sr = 48000u32;
    p.initialize(sr).unwrap();

    let num_frames = 8192;
    let mut input = vec![0.0f32; num_frames * 2];
    // 100 Hz tone on ch0 (below LP cutoff, should pass)
    // 100 Hz tone on ch1 (below HP cutoff, should be attenuated)
    for f in 0..num_frames {
        let t = f as f32 / sr as f32;
        let lf = (2.0 * std::f32::consts::PI * 100.0 * t).sin();
        input[f * 2] = lf;
        input[f * 2 + 1] = lf;
    }
    let mut output = vec![0.0f32; num_frames * 2];
    p.process(&input, &mut output, &ProcessContext::new(sr, num_frames))
        .unwrap();

    // Skip transient: measure RMS on the tail half.
    let tail = num_frames / 2;
    let mut rms_ch0 = 0.0;
    let mut rms_ch1 = 0.0;
    for f in tail..num_frames {
        rms_ch0 += output[f * 2] * output[f * 2];
        rms_ch1 += output[f * 2 + 1] * output[f * 2 + 1];
    }
    rms_ch0 = (rms_ch0 / (num_frames - tail) as f32).sqrt();
    rms_ch1 = (rms_ch1 / (num_frames - tail) as f32).sqrt();
    // ch0 LP@200 sees a 100 Hz tone in passband → ~0.707
    assert!(
        rms_ch0 > 0.5,
        "ch0 LP@200 should pass 100Hz tone (rms={rms_ch0})"
    );
    // ch1 HP@5000 sees a 100 Hz tone deep in stopband → ~0
    assert!(
        rms_ch1 < 0.05,
        "ch1 HP@5000 should reject 100Hz tone (rms={rms_ch1})"
    );
}

#[test]
fn test_per_channel_passthrough_preserves_input() {
    let mut p = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![1000.0, 1000.0],
        vec![PerChannelOpMode::Highpass, PerChannelOpMode::Passthrough],
    )
    .unwrap();
    p.initialize(48000).unwrap();
    let num_frames = 256;
    let mut input = vec![0.0f32; num_frames * 2];
    for f in 0..num_frames {
        input[f * 2] = 0.5;
        input[f * 2 + 1] = 0.5;
    }
    let mut output = vec![0.0; num_frames * 2];
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();
    for f in 0..num_frames {
        // ch1 (Passthrough) must be exactly the input — bit-for-bit.
        assert_eq!(
            output[f * 2 + 1],
            input[f * 2 + 1],
            "passthrough channel must be bitwise identical to input at frame {f}"
        );
    }
}

#[test]
fn test_per_channel_set_get_frequency_and_mode() {
    let mut p = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![100.0, 200.0],
        vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
    )
    .unwrap();
    // Update channel 0 frequency.
    p.set_parameter(
        ParameterId::from("channel_frequency_0"),
        ParameterValue::Float(250.0),
    )
    .unwrap();
    let got = p
        .get_parameter(&ParameterId::from("channel_frequency_0"))
        .unwrap();
    assert_eq!(got, ParameterValue::Float(250.0));
    // Update channel 1 mode to passthrough.
    p.set_parameter(
        ParameterId::from("channel_mode_1"),
        ParameterValue::String("passthrough".to_string()),
    )
    .unwrap();
    let got = p
        .get_parameter(&ParameterId::from("channel_mode_1"))
        .unwrap();
    assert_eq!(got, ParameterValue::String("passthrough".to_string()));
    p.initialize(48000).unwrap();
    assert!(
        p.set_parameter(
            ParameterId::from("channel_frequency_0"),
            ParameterValue::Float(300.0),
        )
        .is_err()
    );
}

#[test]
fn test_per_channel_initialize_rejects_above_nyquist() {
    let mut p = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![10_000.0, 20_000.0],
        vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Lowpass],
    )
    .unwrap();
    assert!(p.initialize(32000).is_err());
}

#[test]
fn test_per_channel_from_params_rejects_mismatched_channels() {
    let params = CrossoverPluginParams {
        crossover_type: "LR24".to_string(),
        frequency: 0.0,
        output: "lowpass".to_string(),
        extra_frequencies: vec![],
        fir_taps: None,
        channel_frequencies_hz: vec![80.0, 100.0],
        channel_modes: vec!["highpass".to_string(), "mute".to_string()],
    };
    // 2 frequencies but channels=3: must error, not silently use 2.
    assert!(CrossoverPlugin::from_params(3, &params).is_err());
}

#[test]
fn test_per_channel_rejects_global_frequency_and_mode_writes() {
    let mut p = CrossoverPlugin::new_per_channel(
        "LR24",
        vec![100.0, 200.0],
        vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
    )
    .unwrap();
    p.initialize(48000).unwrap();
    // Writing the global `frequency` / `mode` must error in per-channel
    // mode — silently updating unused global state would mask routing bugs.
    assert!(
        p.set_parameter(ParameterId::from("frequency"), ParameterValue::Float(500.0))
            .is_err(),
        "global frequency write must be rejected in per-channel mode"
    );
    assert!(
        p.set_parameter(
            ParameterId::from("mode"),
            ParameterValue::String("highpass".to_string())
        )
        .is_err(),
        "global mode write must be rejected in per-channel mode"
    );
    // Per-channel writes are structural once initialized.
    assert!(
        p.set_parameter(
            ParameterId::from("channel_frequency_0"),
            ParameterValue::Float(150.0)
        )
        .is_err()
    );
}

#[test]
fn test_per_channel_from_params() {
    let params = CrossoverPluginParams {
        crossover_type: "LR24".to_string(),
        frequency: 0.0,
        output: "lowpass".to_string(),
        extra_frequencies: vec![],
        fir_taps: None,
        channel_frequencies_hz: vec![80.0, 100.0],
        channel_modes: vec!["highpass".to_string(), "mute".to_string()],
    };
    let p = CrossoverPlugin::from_params(2, &params).unwrap();
    assert!(p.is_per_channel());
    assert_eq!(
        p.op_modes,
        vec![PerChannelOpMode::Highpass, PerChannelOpMode::Mute]
    );
}

#[test]
fn test_parse_channel_freq_id_edge_cases() {
    assert_eq!(parse_channel_freq_id("channel_frequency_0"), Some(0));
    assert_eq!(parse_channel_freq_id("channel_frequency_12"), Some(12));
    assert_eq!(parse_channel_freq_id("channel_frequency_"), None);
    assert_eq!(parse_channel_freq_id("channel_frequency_x"), None);
    assert_eq!(parse_channel_freq_id("frequency_0"), None);
    assert_eq!(parse_channel_freq_id("channel_mode_0"), None);
}

#[test]
fn test_parse_channel_mode_id_edge_cases() {
    assert_eq!(parse_channel_mode_id("channel_mode_0"), Some(0));
    assert_eq!(parse_channel_mode_id("channel_mode_3"), Some(3));
    assert_eq!(parse_channel_mode_id("channel_mode_"), None);
    assert_eq!(parse_channel_mode_id("channel_mode_x"), None);
    assert_eq!(parse_channel_mode_id("channel_frequency_0"), None);
}

#[test]
fn test_is_linear_phase_type_variations() {
    assert!(is_linear_phase_type("LinearPhase"));
    assert!(is_linear_phase_type("linear_phase"));
    assert!(is_linear_phase_type("linear-phase"));
    assert!(is_linear_phase_type("FIR"));
    assert!(is_linear_phase_type("lpfir"));
    assert!(!is_linear_phase_type("LR24"));
    assert!(!is_linear_phase_type("unknown"));
}

#[test]
fn test_nan_frequency_is_rejected() {
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    p.initialize(48000).unwrap();
    let before = p.freq_smoother.target();
    assert!(
        p.set_parameter(
            ParameterId::from("frequency"),
            ParameterValue::Float(f32::NAN)
        )
        .is_err()
    );
    assert_eq!(p.freq_smoother.target(), before);
}

#[test]
fn test_nan_extra_frequency_is_rejected() {
    let mut p = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
    p.initialize(48000).unwrap();
    let before = p.extra_freq_smoothers[0].target();
    assert!(
        p.set_parameter(
            ParameterId::from("frequency_2"),
            ParameterValue::Float(f32::NAN),
        )
        .is_err()
    );
    assert_eq!(p.extra_freq_smoothers[0].target(), before);
}

#[test]
fn test_process_nan_input_does_not_panic() {
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    p.initialize(48000).unwrap();
    let input = vec![f32::NAN; 64];
    let mut output = vec![0.0; 64];
    p.process(&input, &mut output, &ProcessContext::new(48000, 64))
        .unwrap();
    // NaN propagates rather than being silently replaced; the important
    // property is that the filter does not panic or produce Inf.
    assert!(output.iter().any(|s| s.is_nan()));
    assert!(!output.iter().any(|s| s.is_infinite()));
}

#[test]
fn test_unknown_parameter_returns_error() {
    let mut p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    p.initialize(48000).unwrap();
    assert!(
        p.set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0))
            .is_err()
    );
}

#[test]
fn test_get_parameter_unknown_returns_none() {
    let p = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    assert_eq!(p.get_parameter(&ParameterId::from("not_a_param")), None);
}

#[test]
fn test_num_bands_calc_output_channels_and_is_multiway() {
    let p2 = CrossoverPlugin::new(2, "LR24", 1000.0, "both").unwrap();
    assert_eq!(p2.num_bands(), 2);
    assert_eq!(p2.calc_output_channels(), 4);
    assert!(!p2.is_multiway());

    let p3 = CrossoverPlugin::new_multiway(1, "LR24", 500.0, "both", &[5000.0]).unwrap();
    assert_eq!(p3.num_bands(), 3);
    assert_eq!(p3.calc_output_channels(), 3);
    assert!(p3.is_multiway());

    let p_low = CrossoverPlugin::new(1, "LR24", 1000.0, "low").unwrap();
    assert_eq!(p_low.calc_output_channels(), 1);
}

#[test]
fn test_from_params_invalid_output_mode_errors() {
    let params = CrossoverPluginParams {
        crossover_type: "LR24".to_string(),
        frequency: 1000.0,
        output: "invalid".to_string(),
        extra_frequencies: vec![],
        fir_taps: None,
        channel_frequencies_hz: vec![],
        channel_modes: vec![],
    };
    assert!(CrossoverPlugin::from_params(1, &params).is_err());
}

#[test]
fn test_per_channel_from_params_fills_missing_modes_with_default() {
    let params = CrossoverPluginParams {
        crossover_type: "LR24".to_string(),
        frequency: 0.0,
        output: "lowpass".to_string(),
        extra_frequencies: vec![],
        fir_taps: None,
        channel_frequencies_hz: vec![100.0, 200.0],
        channel_modes: vec!["highpass".to_string()],
    };
    let p = CrossoverPlugin::from_params(2, &params).unwrap();
    assert_eq!(
        p.op_modes,
        vec![PerChannelOpMode::Highpass, PerChannelOpMode::Lowpass]
    );
}

#[test]
fn test_per_channel_mode_from_str_invalid_errors() {
    assert!(PerChannelOpMode::from_str("notamode").is_err());
    assert!(PerChannelOpMode::from_str("").is_err());
}
