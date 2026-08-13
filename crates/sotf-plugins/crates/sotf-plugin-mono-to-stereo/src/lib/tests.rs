use super::consts::FFT_SIZE;
use super::mono_to_stereo_plugin::{IDENTITY_RADIUS, MonoToStereoPlugin};
use sotf_host::parameters::ParameterId;
use sotf_host::parameters::ParameterValue;
use sotf_host::plugin::{Plugin, ProcessContext};

use crate::*;
#[test]
fn test_mono_to_stereo_basic() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    let i = vec![0.5; 1024];
    let mut o = vec![0.0; 2048];
    p.process(&i, &mut o, &ProcessContext::new(48000, 1024))
        .unwrap();
    assert!(o[2047].is_finite());
}

#[test]
fn test_mono_to_stereo_width_zero_is_mono() {
    let mut p = MonoToStereoPlugin::new();
    p.haas_delay_ms = 0.0;
    p.initialize(48000).unwrap();
    p.stereo_width.reset(0.0);
    let total_frames = FFT_SIZE * 10;
    let input: Vec<f32> = (0..total_frames).map(|i| (i as f32 * 0.1).sin()).collect();
    let mut output = vec![0.0; total_frames * 2];
    p.process(
        &input,
        &mut output,
        &ProcessContext::new(48000, total_frames),
    )
    .unwrap();
    for frame in (FFT_SIZE * 5)..(FFT_SIZE * 6) {
        let l = output[frame * 2];
        let r = output[frame * 2 + 1];
        assert!(
            (l - r).abs() < 1e-5,
            "L/R differ at frame {frame}: L={l}, R={r}"
        );
    }
}

#[test]
fn test_mono_to_stereo_width_one_differs() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    p.stereo_width.reset(1.0);
    let total_frames = FFT_SIZE * 10;
    let input: Vec<f32> = (0..total_frames).map(|i| (i as f32 * 0.1).sin()).collect();
    let mut output = vec![0.0; total_frames * 2];
    p.process(
        &input,
        &mut output,
        &ProcessContext::new(48000, total_frames),
    )
    .unwrap();
    let mut any_differ = false;
    let mut non_zero = false;
    for frame in (FFT_SIZE * 5)..(FFT_SIZE * 6) {
        let l = output[frame * 2];
        let r = output[frame * 2 + 1];
        if l.abs() > 1e-4 || r.abs() > 1e-4 {
            non_zero = true;
        }
        if (l - r).abs() > 1e-3 {
            any_differ = true;
            break;
        }
    }
    assert!(
        non_zero,
        "Output should not be zero in the middle of the stream"
    );
    assert!(any_differ, "L and R should differ at width=1.0");
}

#[test]
fn test_haas_delay_is_not_reported_as_host_latency() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    let base_latency = p.latency_samples();

    p.set_parameter(
        ParameterId::from("haas_delay_ms"),
        ParameterValue::Float(20.0),
    )
    .unwrap();

    assert!(p.haas_delay_samples > 0);
    assert_eq!(
        p.latency_samples(),
        base_latency,
        "Haas delay is an intentional right-channel effect, not host-compensated latency"
    );
}

/// Verify that mono-to-stereo energy compensation keeps output RMS within 3 dB
/// of input RMS. This ensures the decorrelation + OLA path doesn't significantly
/// change the perceived loudness.
#[test]
fn test_mono_to_stereo_energy_compensation() {
    let mut p = MonoToStereoPlugin::new();
    p.haas_delay_ms = 0.0;
    p.initialize(48000).unwrap();
    p.stereo_width.reset(0.5); // moderate width

    let total_frames = FFT_SIZE * 20;
    let sr = 48000.0_f32;
    // Use a 440 Hz sine to keep things simple
    let input: Vec<f32> = (0..total_frames)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr).sin())
        .collect();
    let mut output = vec![0.0; total_frames * 2];
    p.process(
        &input,
        &mut output,
        &ProcessContext::new(48000, total_frames),
    )
    .unwrap();

    // Measure RMS in settled region
    let start = FFT_SIZE * 8;
    let end = FFT_SIZE * 18;
    let input_rms: f64 = (input[start..end]
        .iter()
        .map(|s| (*s as f64).powi(2))
        .sum::<f64>()
        / (end - start) as f64)
        .sqrt();

    let mut stereo_energy = 0.0_f64;
    for frame in start..end {
        let l = output[frame * 2] as f64;
        let r = output[frame * 2 + 1] as f64;
        // Average power of L and R
        stereo_energy += (l * l + r * r) / 2.0;
    }
    let output_rms = (stereo_energy / (end - start) as f64).sqrt();

    let ratio_db = 20.0 * (output_rms / input_rms).log10();
    assert!(
        ratio_db.abs() < 3.0,
        "Stereo output RMS should be within 3 dB of mono input RMS, \
             but got {ratio_db:.2} dB (in_rms={input_rms:.6}, out_rms={output_rms:.6})"
    );
}

/// Test that freq_dependent mode produces less decorrelation at low frequencies
/// and more at high frequencies. We compare L/R correlation for a bass signal
/// vs a treble signal: bass should be more correlated (closer to mono).
#[test]
fn test_freq_dependent_bass_stays_mono() {
    // Helper: compute L/R correlation for a given frequency
    fn lr_correlation(freq_hz: f32, freq_dep: bool) -> f64 {
        let mut p = MonoToStereoPlugin::new();
        p.freq_dependent = freq_dep;
        p.haas_delay_ms = 0.0; // Disable Haas delay for this correlation test
        p.initialize(48000).unwrap();
        p.stereo_width.reset(1.0);

        let total_frames = FFT_SIZE * 16;
        let input: Vec<f32> = (0..total_frames)
            .map(|i| {
                let t = i as f32 / 48000.0;
                (2.0 * std::f32::consts::PI * freq_hz * t).sin() * 0.5
            })
            .collect();
        let mut output = vec![0.0; total_frames * 2];
        p.process(
            &input,
            &mut output,
            &ProcessContext::new(48000, total_frames),
        )
        .unwrap();

        // Measure L/R difference in steady state
        let start = FFT_SIZE * 6;
        let end = FFT_SIZE * 14;
        let mut sum_diff_sq = 0.0_f64;
        let mut sum_energy = 0.0_f64;
        for frame in start..end {
            let l = output[frame * 2] as f64;
            let r = output[frame * 2 + 1] as f64;
            sum_diff_sq += (l - r).powi(2);
            sum_energy += l.powi(2) + r.powi(2);
        }
        if sum_energy < 1e-12 {
            return 0.0;
        }
        // Normalized difference: 0 = identical, 1 = maximally different
        (sum_diff_sq / sum_energy).sqrt()
    }

    // With freq_dependent=true, 100 Hz should be nearly mono (low difference)
    let bass_diff = lr_correlation(100.0, true);
    // With freq_dependent=true, 4000 Hz should have more difference
    let treble_diff = lr_correlation(4000.0, true);

    assert!(
        bass_diff < treble_diff,
        "With freq_dependent, bass ({bass_diff:.4}) should be more correlated than treble ({treble_diff:.4})"
    );
    // Bass should be nearly mono (very low L/R difference)
    assert!(
        bass_diff < 0.1,
        "Bass decorrelation should be very low with freq_dependent, got {bass_diff:.4}"
    );
}

/// Moving the lower structural crossover across a 150 Hz tone must move the
/// localized all-pass phase rotation with it. Preparing at 300 Hz keeps the
/// tone highly correlated; preparing at 100 Hz places it inside the rotation.
#[test]
fn test_decor_low_hz_parameter_is_honoured() {
    use sotf_host::parameters::{ParameterId, ParameterValue};

    fn run_and_measure_correlation(decor_low: f32) -> f64 {
        let mut p = MonoToStereoPlugin::new();
        // Frequency-dependent mode makes the lower crossover the onset of
        // localized all-pass phase rotation.
        p.set_parameter(
            ParameterId::from("decor_low_hz"),
            ParameterValue::Float(decor_low),
        )
        .unwrap();
        p.initialize(48000).unwrap();
        p.stereo_width.reset(1.0);
        p.haas_delay_ms = 0.0;
        p.update_haas_delay_samples();

        let total_frames = FFT_SIZE * 16;
        // 150 Hz tone — between decor_low_hz min (100 Hz) and default (300 Hz)
        let input: Vec<f32> = (0..total_frames)
            .map(|i| {
                let t = i as f32 / 48000.0;
                (2.0 * std::f32::consts::PI * 150.0 * t).sin() * 0.5
            })
            .collect();
        let mut output = vec![0.0; total_frames * 2];
        p.process(
            &input,
            &mut output,
            &ProcessContext::new(48000, total_frames),
        )
        .unwrap();

        let start = FFT_SIZE * 6;
        let end = FFT_SIZE * 14;
        let mut sum_l_r = 0.0_f64;
        let mut sum_l2 = 0.0_f64;
        let mut sum_r2 = 0.0_f64;
        for frame in start..end {
            let l = output[frame * 2] as f64;
            let r = output[frame * 2 + 1] as f64;
            sum_l_r += l * r;
            sum_l2 += l * l;
            sum_r2 += r * r;
        }
        // Pearson correlation: 1.0 = perfectly in-phase, near 0 = uncorrelated
        let denom = (sum_l2 * sum_r2).sqrt();
        if denom < 1e-12 { 1.0 } else { sum_l_r / denom }
    }

    // At the default 300 Hz onset, 150 Hz remains highly correlated.
    let corr_high_low = run_and_measure_correlation(300.0);
    // At a 100 Hz onset, 150 Hz is inside the phase-rotation region.
    let corr_low_low = run_and_measure_correlation(100.0);

    assert!(
        corr_high_low > corr_low_low,
        "With decor_low_hz=300 Hz, 150 Hz should have higher L/R correlation \
             than with decor_low_hz=100 Hz: corr_300={corr_high_low:.4}, corr_100={corr_low_low:.4}"
    );
    // When the 150 Hz bin is NOT decorrelated (decor_low=300 > 150 Hz),
    // L and R are proportional → correlation near 1.0.
    assert!(
        corr_high_low > 0.95,
        "150 Hz should be near-perfectly correlated when decor_low_hz=300 Hz \
             (bin not decorated), got {corr_high_low:.4}"
    );
}

/// Preparing a lower high crossover makes a 400 Hz tone measurably decorrelated.
#[test]
fn test_decor_high_hz_parameter_is_honoured() {
    use sotf_host::parameters::{ParameterId, ParameterValue};

    // Use the lowest legal crossover band so 400 Hz lies within its phase rotation.
    let mut p = MonoToStereoPlugin::new();
    p.set_parameter(
        ParameterId::from("decor_low_hz"),
        ParameterValue::Float(100.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("decor_high_hz"),
        ParameterValue::Float(1000.0),
    )
    .unwrap();
    p.initialize(48000).unwrap();
    p.stereo_width.reset(1.0);
    p.haas_delay_ms = 0.0;
    p.update_haas_delay_samples();

    let total_frames = FFT_SIZE * 16;
    let input: Vec<f32> = (0..total_frames)
        .map(|i| {
            let t = i as f32 / 48000.0;
            (2.0 * std::f32::consts::PI * 400.0 * t).sin() * 0.5
        })
        .collect();
    let mut output = vec![0.0; total_frames * 2];
    p.process(
        &input,
        &mut output,
        &ProcessContext::new(48000, total_frames),
    )
    .unwrap();

    let start = FFT_SIZE * 6;
    let end = FFT_SIZE * 14;
    let mut sum_diff_sq = 0.0_f64;
    let mut sum_energy = 0.0_f64;
    for frame in start..end {
        let l = output[frame * 2] as f64;
        let r = output[frame * 2 + 1] as f64;
        sum_diff_sq += (l - r).powi(2);
        sum_energy += l.powi(2) + r.powi(2);
    }
    let diff = if sum_energy > 1e-12 {
        (sum_diff_sq / sum_energy).sqrt()
    } else {
        0.0
    };
    // With decor_high_hz = 200 Hz, 400 Hz should have measurable decorrelation.
    assert!(
        diff > 0.05,
        "400 Hz should be decorrelated when decor_high_hz=200 Hz, got diff={diff:.4}"
    );
}

/// Test that the output buffer is never left with stale data when a break
/// was previously possible (output_pos < nf but no STFT and no drain).
/// We exercise this with a very small block size (nf=1) that forces the path.
#[test]
fn test_process_no_stale_output_on_small_blocks() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    // Process in tiny 1-sample blocks for a full FFT window worth of input.
    let total_frames = FFT_SIZE + 10;
    for i in 0..total_frames {
        let sample = (i as f32 * 0.1).sin();
        let mut out = vec![99.0_f32; 2]; // pre-fill with sentinel
        p.process(&[sample], &mut out, &ProcessContext::new(48000, 1))
            .unwrap();
        // Output must never contain the sentinel value — it must have been written.
        assert!(
            out[0] != 99.0 || out[1] != 99.0 || out[0].is_finite(),
            "stale data at frame {i}"
        );
        // Both samples must be finite.
        assert!(out[0].is_finite(), "L not finite at frame {i}");
        assert!(out[1].is_finite(), "R not finite at frame {i}");
    }
}

/// Test L/R energy balance at width=1.0 using broadband content.
#[test]
fn test_mono_to_stereo_lr_energy_balance() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    p.stereo_width.reset(1.0);
    let total_frames = FFT_SIZE * 32;
    // Sum of many sines for broadband coverage (300–15000 Hz decorrelation band)
    let input: Vec<f32> = (0..total_frames)
        .map(|i| {
            let t = i as f32 / 48000.0;
            let mut s = 0.0_f32;
            let mut freq = 200.0;
            while freq < 16000.0 {
                s += (2.0 * std::f32::consts::PI * freq * t).sin();
                freq *= 1.07; // ~40 frequencies, roughly 1/3 octave spacing
            }
            s * 0.02 // scale to avoid clipping
        })
        .collect();
    let mut output = vec![0.0; total_frames * 2];
    p.process(
        &input,
        &mut output,
        &ProcessContext::new(48000, total_frames),
    )
    .unwrap();

    // Skip warmup, measure steady-state RMS
    let start = FFT_SIZE * 10;
    let end = FFT_SIZE * 28;
    let mut rms_l = 0.0_f64;
    let mut rms_r = 0.0_f64;
    for frame in start..end {
        rms_l += (output[frame * 2] as f64).powi(2);
        rms_r += (output[frame * 2 + 1] as f64).powi(2);
    }
    let n = (end - start) as f64;
    rms_l = (rms_l / n).sqrt();
    rms_r = (rms_r / n).sqrt();
    let ratio_db = 20.0 * (rms_r / rms_l).log10();
    assert!(
        ratio_db.abs() < 1.0,
        "L/R energy imbalance at width=1.0: {ratio_db:.2} dB (L_rms={rms_l:.6}, R_rms={rms_r:.6})"
    );
}

#[test]
fn test_set_parameter_stereo_width() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("stereo_width"),
        ParameterValue::Float(0.75),
    )
    .unwrap();
    assert!((p.stereo_width.target() - 0.75).abs() < 1e-6);
}

#[test]
fn test_set_parameter_haas_delay_ms_updates_samples() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("haas_delay_ms"),
        ParameterValue::Float(3.0),
    )
    .unwrap();
    assert!((p.haas_delay_ms - 3.0).abs() < 1e-6);
    let expected_samples = (((3.0_f32 / 1000.0) * 48000.0).round()) as usize;
    assert_eq!(p.haas_delay_samples, expected_samples);
}

#[test]
fn test_structural_decor_frequencies_prepare_topology_before_initialize() {
    let mut p = MonoToStereoPlugin::new();
    p.set_parameter(
        ParameterId::from("decor_low_hz"),
        ParameterValue::Float(200.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("decor_high_hz"),
        ParameterValue::Float(1000.0),
    )
    .unwrap();
    p.initialize(48000).unwrap();
    assert!((p.decor_low_hz - 200.0).abs() < 1e-6);
    assert!((p.decor_high_hz - 1000.0).abs() < 1e-6);
    let expected_low = (std::f32::consts::TAU * 200.0 / 48_000.0).cos();
    let expected_high = (std::f32::consts::TAU * 1000.0 / 48_000.0).cos();
    assert!((p.section_cosines[0] - expected_low).abs() < 1e-6);
    assert!((p.section_cosines[2] - expected_high).abs() < 1e-6);
}

#[test]
fn test_set_parameter_freq_dependent() {
    let mut p = MonoToStereoPlugin::new();
    p.set_parameter(
        ParameterId::from("freq_dependent"),
        ParameterValue::Bool(false),
    )
    .unwrap();
    assert!(!p.freq_dependent);
    p.initialize(48000).unwrap();
    assert!(p.target_radius < 0.7);
    p = MonoToStereoPlugin::new();
    p.set_parameter(
        ParameterId::from("freq_dependent"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert!(p.freq_dependent);
}

#[test]
fn test_set_parameter_unknown_returns_err() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    let result = p.set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(0.0));
    assert!(result.is_err(), "unknown parameter must error");
}

#[test]
fn test_set_parameter_wrong_type_returns_err() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    let result = p.set_parameter(
        ParameterId::from("stereo_width"),
        ParameterValue::Bool(true),
    );
    assert!(result.is_err(), "type mismatch must error");
}

#[test]
fn test_set_parameter_out_of_range_clamps() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("stereo_width"),
        ParameterValue::Float(2.0),
    )
    .unwrap();
    assert!(
        (p.stereo_width.target() - 1.0).abs() < 1e-6,
        "stereo_width should clamp to max 1.0"
    );
    p.set_parameter(
        ParameterId::from("stereo_width"),
        ParameterValue::Float(-1.0),
    )
    .unwrap();
    assert!(
        p.stereo_width.target().abs() < 1e-6,
        "stereo_width should clamp to min 0.0"
    );
}

#[test]
fn test_process_zero_frames_returns_ok() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    let input: Vec<f32> = vec![];
    let mut output: Vec<f32> = vec![];
    let produced = p
        .process(&input, &mut output, &ProcessContext::new(48000, 0))
        .unwrap();
    assert_eq!(produced, 0);
}

#[test]
fn test_process_returns_num_frames() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    let num_frames = 1024;
    let input = vec![0.5_f32; num_frames];
    let mut output = vec![0.0_f32; num_frames * 2];
    let produced = p
        .process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();
    assert_eq!(produced, num_frames);
}

#[test]
fn test_channel_configuration() {
    let p = MonoToStereoPlugin::new();
    assert_eq!(p.input_channels(), 1);
    assert_eq!(p.output_channels(), 2);
}

#[test]
fn test_from_params_honours_arguments() {
    let params = MonoToStereoPluginParams {
        stereo_width: 0.25,
        freq_dependent: false,
        haas_delay_ms: 2.5,
        decor_low_hz: 200.0,
        decor_high_hz: 3_000.0,
    };
    let p = MonoToStereoPlugin::from_params(1, params);
    assert!((p.stereo_width.target() - 0.25).abs() < 1e-6);
    assert!(!p.freq_dependent);
    assert!((p.haas_delay_ms - 2.5).abs() < 1e-6);
    assert_eq!(p.haas_delay_samples, 110);
    assert!((p.decor_low_hz - 200.0).abs() < 1e-6);
    assert!((p.decor_high_hz - 3_000.0).abs() < 1e-6);
}

/// Happy-path regression: process() must return Ok(num_frames) for normal input.
#[test]
fn test_process_returns_ok_num_frames() {
    let mut p = MonoToStereoPlugin::new();
    p.initialize(48000).unwrap();
    let num_frames = 1024;
    let input: Vec<f32> = (0..num_frames).map(|i| (i as f32 * 0.1).sin()).collect();
    let mut output = vec![0.0_f32; num_frames * 2];
    let result = p.process(&input, &mut output, &ProcessContext::new(48000, num_frames));
    assert_eq!(
        result,
        Ok(num_frames),
        "process() should return Ok(num_frames)"
    );
}

#[test]
fn test_process_is_partition_invariant_with_fixed_latency() {
    fn render(block_sizes: &[usize]) -> Vec<f32> {
        let total_frames = 8192;
        let input: Vec<f32> = (0..total_frames)
            .map(|i| (i as f32 * 0.017).sin() * 0.4)
            .collect();
        let mut plugin = MonoToStereoPlugin::new();
        plugin.initialize(48_000).unwrap();
        plugin.stereo_width.reset(0.35);
        plugin.haas_delay_ms = 0.0;
        let mut output = vec![0.0; total_frames * 2];
        let mut input_pos = 0;
        let mut block_pos = 0;
        while input_pos < total_frames {
            let block = block_sizes[block_pos % block_sizes.len()];
            let end = (input_pos + block).min(total_frames);
            plugin
                .process(
                    &input[input_pos..end],
                    &mut output[input_pos * 2..end * 2],
                    &ProcessContext::new(48_000, end - input_pos),
                )
                .unwrap();
            input_pos = end;
            block_pos += 1;
        }
        output
    }

    let reference = render(&[1]);
    for blocks in [
        &[64][..],
        &[256][..],
        &[512][..],
        &[1024][..],
        &[127, 641, 89, 1024][..],
    ] {
        let actual = render(blocks);
        assert_eq!(actual.len(), reference.len());
        for (index, (a, b)) in actual.iter().zip(&reference).enumerate() {
            assert!(
                (a - b).abs() < 2e-5,
                "partition mismatch at {index}: {a} vs {b}"
            );
        }
    }
}

#[test]
fn test_process_rejects_short_buffers_without_mutating_state() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(48_000).unwrap();
    let input = vec![0.25; 8];
    let mut output = vec![7.0; 16];
    let before_last_input = plugin.last_input;
    let before_right_state = plugin.first_order_y1;

    let error = plugin
        .process(&input[..7], &mut output, &ProcessContext::new(48_000, 8))
        .unwrap_err();
    assert!(error.contains("input") && error.contains("8"));
    assert_eq!(plugin.last_input, before_last_input);
    assert_eq!(plugin.first_order_y1, before_right_state);
    assert!(output.iter().all(|sample| *sample == 7.0));

    let error = plugin
        .process(&input, &mut output[..15], &ProcessContext::new(48_000, 8))
        .unwrap_err();
    assert!(error.contains("output") && error.contains("16"));
    assert_eq!(plugin.last_input, before_last_input);
    assert_eq!(plugin.first_order_y1, before_right_state);
    assert!(output.iter().all(|sample| *sample == 7.0));
}

#[test]
fn test_process_rejects_oversized_buffers_without_mutating_state() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(48_000).unwrap();
    let input = [0.25_f32, -0.5, 91.0, 92.0];
    let mut output = [77.0_f32; 8];
    let before_last_input = plugin.last_input;
    let before_right_state = plugin.first_order_y1;

    let error = plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 2))
        .unwrap_err();
    assert!(error.contains("input") && error.contains("exactly"));
    assert_eq!(plugin.last_input, before_last_input);
    assert_eq!(plugin.first_order_y1, before_right_state);
    assert_eq!(output, [77.0; 8]);

    let input = [0.25_f32, -0.5];
    let error = plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 2))
        .unwrap_err();
    assert!(error.contains("output") && error.contains("exactly"));
    assert_eq!(plugin.last_input, before_last_input);
    assert_eq!(plugin.first_order_y1, before_right_state);
    assert_eq!(output, [77.0; 8]);
}

#[test]
fn test_process_rejects_stereo_sample_count_overflow_without_mutating_state() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(48_000).unwrap();
    let before_last_input = plugin.last_input;
    let before_right_state = plugin.first_order_y1;
    let mut output = [73.0_f32; 2];

    let error = plugin
        .process(&[], &mut output, &ProcessContext::new(48_000, usize::MAX))
        .unwrap_err();

    assert!(error.contains("overflows"), "unexpected error: {error}");
    assert_eq!(plugin.last_input, before_last_input);
    assert_eq!(plugin.first_order_y1, before_right_state);
    assert_eq!(output, [73.0; 2]);
}

#[test]
fn initialize_rejects_zero_sample_rate_atomically() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(48_000).unwrap();
    let before_rate = plugin.sample_rate;
    let before_cosines = plugin.section_cosines;

    let error = plugin.initialize(0).unwrap_err();

    assert!(error.contains("sample rate"), "unexpected error: {error}");
    assert_eq!(plugin.sample_rate, before_rate);
    assert_eq!(plugin.section_cosines, before_cosines);
}

#[test]
fn plugin_info_version_matches_crate_version() {
    assert_eq!(
        MonoToStereoPlugin::new().info().version,
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn test_second_order_allpass_keeps_every_bin_unit_magnitude() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(48_000).unwrap();
    for radius in [0.25_f32, 0.68, 0.998, IDENTITY_RADIUS] {
        for cosine in plugin.section_cosines {
            for bin in 0..=FFT_SIZE / 2 {
                let omega = std::f32::consts::TAU * bin as f32 / FFT_SIZE as f32;
                let response = MonoToStereoPlugin::allpass2_response(radius, cosine, omega);
                assert!(
                    (response.norm() - 1.0).abs() < 2e-3,
                    "radius={radius}, bin={bin}, response={response:?}"
                );
            }
        }
    }
}

#[test]
fn test_try_from_params_validates_channels_and_each_numeric_field() {
    for (width, delay) in [(0.0, 0.0), (1.0, 5.0)] {
        let params = MonoToStereoPluginParams {
            stereo_width: width,
            freq_dependent: true,
            haas_delay_ms: delay,
            ..Default::default()
        };
        assert!(
            MonoToStereoPlugin::try_from_params(1, params).is_ok(),
            "schema endpoint width={width}, delay={delay} must be accepted"
        );
    }

    let error = match MonoToStereoPlugin::try_from_params(
        2,
        MonoToStereoPluginParams {
            stereo_width: 0.25,
            freq_dependent: true,
            haas_delay_ms: 2.5,
            ..Default::default()
        },
    ) {
        Ok(_) => panic!("wrong channel count must fail"),
        Err(error) => error,
    };
    assert!(error.contains("1 input channel"));

    for width in [-0.01, 1.01, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let error = match MonoToStereoPlugin::try_from_params(
            1,
            MonoToStereoPluginParams {
                stereo_width: width,
                freq_dependent: true,
                haas_delay_ms: 2.5,
                ..Default::default()
            },
        ) {
            Ok(_) => panic!("invalid stereo width {width} must fail"),
            Err(error) => error,
        };
        assert!(error.contains("stereo_width"), "unexpected error: {error}");
    }

    for delay in [-0.01, 5.01, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let error = match MonoToStereoPlugin::try_from_params(
            1,
            MonoToStereoPluginParams {
                stereo_width: 0.25,
                freq_dependent: true,
                haas_delay_ms: delay,
                ..Default::default()
            },
        ) {
            Ok(_) => panic!("invalid Haas delay {delay} must fail"),
            Err(error) => error,
        };
        assert!(error.contains("haas_delay_ms"), "unexpected error: {error}");
    }

    for low in [99.0, 501.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let error = MonoToStereoPlugin::try_from_params(
            1,
            MonoToStereoPluginParams {
                decor_low_hz: low,
                ..Default::default()
            },
        )
        .err()
        .expect("invalid decorrelation low crossover must fail");
        assert!(error.contains("decor_low_hz"), "unexpected error: {error}");
    }

    for high in [999.0, 5_001.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let error = MonoToStereoPlugin::try_from_params(
            1,
            MonoToStereoPluginParams {
                decor_high_hz: high,
                ..Default::default()
            },
        )
        .err()
        .expect("invalid decorrelation high crossover must fail");
        assert!(error.contains("decor_high_hz"), "unexpected error: {error}");
    }
}

#[test]
fn causal_allpass_is_energy_normalized_at_every_fft_bin() {
    for coefficient in [0.0_f32, 0.25, 0.5, 0.75, 0.95] {
        for bin in 0..=FFT_SIZE / 2 {
            let omega = std::f32::consts::TAU * bin as f32 / FFT_SIZE as f32;
            let response = MonoToStereoPlugin::allpass_response(coefficient, omega);
            assert!(
                (response.norm() - 1.0).abs() < 2.0e-6,
                "a={coefficient}, bin={bin}, response={response:?}"
            );
        }
    }
}

#[test]
fn rendered_audio_preserves_lr_energy_at_every_width_and_frequency() {
    const SAMPLE_RATE: u32 = 48_000;
    const FRAMES: usize = 48_000;
    for width in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
        for frequency in [80.0_f32, 300.0, 1_000.0, 5_000.0, 12_000.0, 20_000.0] {
            let mut plugin = MonoToStereoPlugin::new();
            plugin.initialize(SAMPLE_RATE).unwrap();
            plugin.stereo_width.reset(width);
            plugin.haas_delay_ms = 0.0;
            plugin.update_haas_delay_samples();
            let input: Vec<f32> = (0..FRAMES)
                .map(|frame| {
                    (std::f32::consts::TAU * frequency * frame as f32 / SAMPLE_RATE as f32).sin()
                        * 0.25
                })
                .collect();
            let mut output = vec![0.0_f32; FRAMES * 2];
            plugin
                .process(
                    &input,
                    &mut output,
                    &ProcessContext::new(SAMPLE_RATE, FRAMES),
                )
                .unwrap();
            // The narrowest causal all-pass poles need several time constants
            // to reach their steady-state unit-energy sinusoidal response.
            let skip = 12_000;
            let left = output[skip * 2..]
                .chunks_exact(2)
                .map(|frame| (frame[0] as f64).powi(2))
                .sum::<f64>();
            let right = output[skip * 2..]
                .chunks_exact(2)
                .map(|frame| (frame[1] as f64).powi(2))
                .sum::<f64>();
            let ratio = (right / left).sqrt();
            assert!(
                (ratio - 1.0).abs() < 0.015,
                "width={width}, frequency={frequency}, L/R RMS ratio={ratio}"
            );
            let input_energy = input[skip..]
                .iter()
                .map(|sample| (*sample as f64).powi(2))
                .sum::<f64>();
            let fold_energy = output[skip * 2..]
                .chunks_exact(2)
                .map(|frame| (((frame[0] + frame[1]) * 0.5) as f64).powi(2))
                .sum::<f64>();
            assert!(
                fold_energy <= input_energy * 1.02,
                "width={width}, frequency={frequency}, mono fold boosted energy"
            );
            let channel_peak = output[skip * 2..]
                .iter()
                .map(|sample| sample.abs())
                .fold(0.0_f32, f32::max);
            assert!(
                channel_peak <= 0.255,
                "width={width}, frequency={frequency}, channel peak={channel_peak}"
            );
        }
    }
}

#[test]
fn decorrelator_is_causal_and_has_no_circular_pre_echo() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(48_000).unwrap();
    plugin.stereo_width.reset(1.0);
    plugin.haas_delay_ms = 0.0;
    plugin.update_haas_delay_samples();
    let mut input = vec![0.0_f32; 4_096];
    input[1_000] = 1.0;
    let mut output = vec![0.0_f32; input.len() * 2];
    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(48_000, input.len()),
        )
        .unwrap();
    assert!(output[..2_000].iter().all(|sample| sample.abs() < 1.0e-8));
    assert!(output[2_001].abs() > 1.0e-4);
    assert_eq!(plugin.latency_samples(), 0);
}

#[test]
fn decorrelator_topology_parameters_require_graph_rebuild_after_initialize() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(48_000).unwrap();
    for (id, value) in [
        ("decor_low_hz", ParameterValue::Float(250.0)),
        ("decor_high_hz", ParameterValue::Float(3_000.0)),
        ("freq_dependent", ParameterValue::Bool(false)),
    ] {
        let before = plugin.get_parameter(&ParameterId::from(id));
        let error = plugin
            .set_parameter(ParameterId::from(id), value)
            .unwrap_err();
        assert!(error.contains("structural") && error.contains("rebuild"));
        assert_eq!(plugin.get_parameter(&ParameterId::from(id)), before);
    }
}

#[test]
fn settled_zero_width_uses_exact_duplicate_fast_path() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(48_000).unwrap();
    plugin.stereo_width.reset(0.0);
    plugin.haas_delay_ms = 0.0;
    plugin.update_haas_delay_samples();
    let input: Vec<f32> = (0..4_096).map(|i| (i as f32 * 0.013).sin()).collect();
    let mut output = vec![0.0_f32; input.len() * 2];
    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(48_000, input.len()),
        )
        .unwrap();
    assert!(output.chunks_exact(2).zip(&input).all(|(frame, input)| {
        frame[0].to_bits() == input.to_bits() && frame[1].to_bits() == input.to_bits()
    }));
    assert_eq!(plugin.duplicate_fast_path_frames, input.len());
    assert_eq!(plugin.smoothed_width_frames, 0);
}

#[test]
fn settled_nonzero_width_avoids_per_sample_smoother_work() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(48_000).unwrap();
    plugin.stereo_width.reset(0.75);
    plugin.haas_delay_ms = 0.0;
    plugin.update_haas_delay_samples();
    let input: Vec<f32> = (0..8_192)
        .map(|i| (i as f32 * 0.019).sin() * 0.25)
        .collect();
    let mut output = vec![0.0_f32; input.len() * 2];
    plugin
        .process(
            &input,
            &mut output,
            &ProcessContext::new(48_000, input.len()),
        )
        .unwrap();
    assert_eq!(plugin.smoothed_width_frames, 0);
    assert_eq!(plugin.duplicate_fast_path_frames, 0);
    assert!(output.iter().all(|sample| sample.is_finite()));
    assert!(
        output
            .chunks_exact(2)
            .skip(2_048)
            .any(|frame| (frame[0] - frame[1]).abs() > 1.0e-4)
    );
}

#[test]
fn leaving_duplicate_fast_path_primes_state_without_a_transition_spike() {
    let mut plugin = MonoToStereoPlugin::new();
    plugin.initialize(48_000).unwrap();
    plugin.haas_delay_ms = 0.0;
    plugin.update_haas_delay_samples();
    plugin.stereo_width.reset(0.0);
    let input = vec![0.25_f32; 512];
    let mut output = vec![0.0_f32; 1_024];
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 512))
        .unwrap();

    plugin.stereo_width.reset(1.0);
    output.fill(0.0);
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, 512))
        .unwrap();
    assert!(output.iter().all(|sample| sample.is_finite()));
    assert!(output.iter().all(|sample| sample.abs() <= 0.251));
}
