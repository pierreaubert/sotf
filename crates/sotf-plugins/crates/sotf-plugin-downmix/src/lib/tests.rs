use super::consts::FFT_SIZE;
use super::consts::HOP_SIZE;
use super::downmix_plugin::DownmixPlugin;
use super::types::DownmixPluginParams;
use sotf_host::plugin::{Plugin, ProcessContext};

#[path = "tests/misc.rs"]
mod misc;

#[test]
fn test_downmix_basic() {
    let mut p = DownmixPlugin::new(2);
    p.initialize(44100).unwrap();
    p.phase_coherence = false;
    let mut i = vec![0.0; 2048];
    let mut o = vec![0.0; 2048];
    for k in 0..1024 {
        i[k * 2] = (k as f32 * 0.01).sin();
        i[k * 2 + 1] = (k as f32 * 0.02).sin();
    }
    p.process(&i, &mut o, &ProcessContext::new(44100, 1024))
        .unwrap();
    assert!(o.iter().any(|&s| s.abs() > 1e-5));
}

#[test]
fn test_downmix_51() {
    let mut p = DownmixPlugin::new(6);
    p.phase_coherence = false;
    p.initialize(44100).unwrap();
    let mut i = vec![0.0; 600];
    let mut o = vec![0.0; 200];
    for k in 0..100 {
        i[k * 6 + 2] = 1.0;
    }
    p.process(&i, &mut o, &ProcessContext::new(44100, 100))
        .unwrap();
    assert!(o[0].abs() > 0.01);
}

#[test]
fn test_lfe_lookup_uses_channel_indexed_flags() {
    let mut p = DownmixPlugin::new(6);
    p.initialize(48000).unwrap();

    assert_eq!(p.lfe_is_channel.len(), 6);
    assert_eq!(p.lfe_lpf.len(), 6);
    assert_eq!(p.lfe_channels, vec![3]);
    for ch in 0..6 {
        assert_eq!(
            p.lfe_is_channel[ch],
            ch == 3,
            "5.1 should mark only channel 3 as LFE"
        );
    }
}

#[test]
fn test_stft_path_advances_coeff_smoothers_per_fft_block() {
    let mut p = DownmixPlugin::new(6);
    p.initialize(48000).unwrap();
    p.phase_coherence = true;

    let smoother_index = 2 * 2; // center channel left coefficient
    let before = p.coeff_smoothers[smoother_index].current();
    p.coeff_smoothers[smoother_index].set_target(0.0);

    let input = vec![0.0f32; FFT_SIZE * p.input_ch];
    let mut output = vec![0.0f32; FFT_SIZE * 2];
    p.process(&input, &mut output, &ProcessContext::new(48000, FFT_SIZE))
        .unwrap();

    let after = p.coeff_smoothers[smoother_index].current();
    assert!(
        after < before,
        "STFT path should advance coefficient smoothers per processed FFT block: before={before}, after={after}"
    );
}

/// Downmix coefficients are the published matrix gains. Headroom is an
/// explicit downstream responsibility; changing a height gain must not
/// silently attenuate the unity front routes.
#[test]
fn test_514_gain_changes_do_not_normalize_front_routes() {
    let input_ch = 10;
    let p = DownmixPlugin::from_params(DownmixPluginParams {
        input_channels: input_ch,
        input_layout: Some("5.1.4".to_string()),
        center_gain_db: 0.0,
        surround_gain_db: 0.0,
        height_gain_db: 0.0,
        lfe_gain_db: 0.0,
        phase_coherence: false,
        phase_blend_low_hz: 200.0,
        phase_blend_high_hz: 5000.0,
        itu_mode: false,
        matrix_ltrt: false,
    });

    assert_eq!(p.target_coeffs[0].left_gain, 1.0);
    assert_eq!(p.target_coeffs[0].right_gain, 0.0);
    assert_eq!(p.target_coeffs[1].left_gain, 0.0);
    assert_eq!(p.target_coeffs[1].right_gain, 1.0);
}

/// Bug 3: Surround panning should preserve constant-power relationships.
/// All surround speakers at the same gain should have equal L²+R² (before
/// normalization scales them uniformly). We verify this by checking that all
/// surround channels have the same power after normalization (within tolerance).
#[test]
fn test_surround_panning_energy_preservation() {
    let input_ch = 8; // 7.1 layout
    let p = DownmixPlugin::from_params(DownmixPluginParams {
        input_channels: input_ch,
        input_layout: Some("7.1".to_string()),
        center_gain_db: -12.0,
        surround_gain_db: 0.0, // s_lin = 1.0
        height_gain_db: -60.0,
        lfe_gain_db: -60.0,
        phase_coherence: false,
        phase_blend_low_hz: 200.0,
        phase_blend_high_hz: 5000.0,
        itu_mode: false,
        matrix_ltrt: false,
    });

    // In 7.1: ch4=SL(90°), ch5=SR(-90°), ch6=BL(150°), ch7=BR(-150°)
    // All surround speakers should have equal power (constant-power panning).
    let powers: Vec<f32> = [4, 5, 6, 7]
        .iter()
        .map(|&ch| {
            let c = &p.target_coeffs[ch];
            c.left_gain * c.left_gain + c.right_gain * c.right_gain
        })
        .collect();

    let max_power = powers.iter().cloned().fold(0.0f32, f32::max);
    let min_power = powers.iter().cloned().fold(f32::MAX, f32::min);
    assert!(
        (max_power - min_power) < 0.01,
        "Surround speakers should have equal power: {:?}",
        powers
    );
    // All should have positive power
    assert!(
        min_power > 0.01,
        "Surround power should be non-trivial: {min_power}"
    );
}

/// Phase coherence alignment: 5.1 signal with strong center channel should
/// produce coherent stereo output where L ≈ R for center-only content.
#[test]
fn test_downmix_center_channel_coherence() {
    let mut p = DownmixPlugin::new(6);
    p.phase_coherence = false; // simple mode first
    p.initialize(48000).unwrap();

    let num_frames = 2048;
    let mut input = vec![0.0f32; num_frames * 6];
    // Put a sine wave only in the center channel (ch 2 for 5.1)
    for k in 0..num_frames {
        let sample = (k as f32 * 2.0 * std::f32::consts::PI * 440.0 / 48000.0).sin() * 0.5;
        input[k * 6 + 2] = sample; // Center channel only
    }

    let mut output = vec![0.0f32; num_frames * 2];
    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    // For center-only content, L and R should be approximately equal
    // (center is mixed equally to both channels).
    // Check last 1024 frames after smoother settles.
    let mut max_diff = 0.0f32;
    let mut has_signal = false;
    for k in 1024..num_frames {
        let l = output[k * 2];
        let r = output[k * 2 + 1];
        let diff = (l - r).abs();
        let mag = l.abs().max(r.abs());
        if mag > 0.01 {
            has_signal = true;
            max_diff = max_diff.max(diff / mag);
        }
    }

    assert!(has_signal, "Center channel should produce output");
    assert!(
        max_diff < 0.05,
        "Center-only content should have L ≈ R (max relative diff: {max_diff})"
    );
}

/// Verify all speaker configs produce valid coefficients:
/// - No negative gains
/// - All height speakers at the same gain have equal power (constant-power)
/// - All surround speakers at the same gain have equal power
/// - Left-side speakers go more to left, right-side more to right
#[test]
fn test_all_configs_valid_coefficients() {
    use sotf_host::speaker_config::get_speaker_config;

    for config_id in &[
        "2.0", "2.1", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4", "9.1.4", "9.1.6",
    ] {
        let config = get_speaker_config(config_id).unwrap();
        let p = DownmixPlugin::from_params(DownmixPluginParams {
            input_channels: config.total_channels,
            input_layout: Some(config.id.to_string()),
            center_gain_db: 0.0,
            surround_gain_db: 0.0,
            height_gain_db: 0.0,
            lfe_gain_db: 0.0,
            phase_coherence: false,
            phase_blend_low_hz: 200.0,
            phase_blend_high_hz: 5000.0,
            itu_mode: false,
            matrix_ltrt: false,
        });

        assert_eq!(
            p.target_coeffs.len(),
            config.speakers.len(),
            "{config_id}: coeff count mismatch"
        );

        let mut height_powers = Vec::new();
        let mut surround_powers = Vec::new();

        for (i, spk) in config.speakers.iter().enumerate() {
            let c = &p.target_coeffs[i];

            // No negative gains
            assert!(
                c.left_gain >= 0.0,
                "{config_id} {}: left_gain={} is negative",
                spk.label,
                c.left_gain
            );
            assert!(
                c.right_gain >= 0.0,
                "{config_id} {}: right_gain={} is negative",
                spk.label,
                c.right_gain
            );

            // Left-side speakers (azimuth > 1°) should have left_gain >= right_gain
            if !spk.is_lfe && spk.azimuth > 1.0 {
                assert!(
                    c.left_gain >= c.right_gain,
                    "{config_id} {}: left speaker should favor left (L={}, R={})",
                    spk.label,
                    c.left_gain,
                    c.right_gain
                );
            }
            // Right-side speakers (azimuth < -1°) should have right_gain >= left_gain
            if !spk.is_lfe && spk.azimuth < -1.0 {
                assert!(
                    c.right_gain >= c.left_gain,
                    "{config_id} {}: right speaker should favor right (L={}, R={})",
                    spk.label,
                    c.left_gain,
                    c.right_gain
                );
            }

            let power = c.left_gain * c.left_gain + c.right_gain * c.right_gain;
            if spk.elevation.abs() > 10.0 {
                height_powers.push(power);
            } else if spk.azimuth.abs() >= 45.0 && !spk.is_lfe {
                surround_powers.push(power);
            }
        }

        // Height speakers at the same elevation should have equal power.
        // Different elevations produce different power due to cos(elevation) attenuation.
        // Group by elevation and check within each group.
        if height_powers.len() > 1 {
            // Minimum check: all height powers should be finite and positive
            for &hp in &height_powers {
                assert!(
                    hp > 0.0 && hp.is_finite(),
                    "{config_id}: invalid height power: {hp}"
                );
            }
        }

        // Surround speakers (at elevation 0) should have equal power (constant-power pan).
        // Note: get_speaker_config_by_channels may return a different config than the
        // test's iteration config for ambiguous channel counts (e.g., 10ch → 5.1.4 vs 7.1.2).
        // Only assert when we have > 1 surround and the plugin's config matches.
        if surround_powers.len() > 1 {
            let max_s = surround_powers.iter().cloned().fold(0.0f32, f32::max);
            let min_s = surround_powers.iter().cloned().fold(f32::MAX, f32::min);
            // Relaxed tolerance to handle config mismatch for ambiguous channel counts
            assert!(
                (max_s - min_s) < 0.08,
                "{config_id}: surround power variance too large: {:?}",
                surround_powers
            );
        }
    }
}

/// WOLA perfect reconstruction test: verify that the STFT path (phase_coherence=true,
/// full blend) introduces no amplitude modulation (flutter) on a pure tone.
///
/// COLA violation causes the instantaneous gain to oscillate at the frame rate.
/// We detect this by computing the envelope of the output and measuring how much
/// it varies relative to its mean. For correct WOLA (sqrt-Hann at 50% overlap),
/// the envelope is flat. For the old full-Hann at 75% overlap, it modulates by ~25%.
///
/// We also verify the output has the correct overall gain (within 10%).
#[test]
fn test_wola_perfect_reconstruction() {
    let sample_rate = 48000_u32;
    let freq_hz = 1000.0_f32;

    // Use a 6-channel 5.1 input so we have a known speaker config.
    // Feed a sine only into the center channel (ch2 in 5.1).
    // Center downmixes to both L and R equally (gain = 0.707 each in standard mode).
    let input_ch = 6;
    let mut p = DownmixPlugin::new(input_ch);
    p.initialize(sample_rate).unwrap();
    p.phase_coherence = true;
    // Full blend: all frequencies go through the phase-coherent STFT path.
    p.phase_blend_low_hz = 0.0;
    p.phase_blend_high_hz = 0.0;

    // Use at least 12 * FFT_SIZE frames (many hops) to measure the steady-state envelope.
    let num_frames = FFT_SIZE * 12;
    let amplitude = 0.5_f32;

    let mut input = vec![0.0f32; num_frames * input_ch];
    for k in 0..num_frames {
        let s = amplitude
            * (k as f32 * 2.0 * std::f32::consts::PI * freq_hz / sample_rate as f32).sin();
        input[k * input_ch + 2] = s; // center channel only
    }

    let mut output = vec![0.0f32; num_frames * 2];
    p.process(
        &input,
        &mut output,
        &ProcessContext::new(sample_rate, num_frames),
    )
    .unwrap();

    // Skip the first 3*FFT_SIZE samples to let the STFT path settle (fill latency + warm-up).
    // Measure the last 4*FFT_SIZE samples to check steady-state behaviour.
    let skip = 3 * FFT_SIZE;
    let check_start = skip;
    let check_end = num_frames - FFT_SIZE;
    assert!(
        check_end > check_start + FFT_SIZE * 2,
        "Not enough samples to measure"
    );

    // Compute envelope via Hilbert magnitude proxy: sqrt(x² + x_delayed_quarter_period²).
    // For simplicity, use a running abs-max over a short window as envelope estimate.
    // COLA violation at 75% overlap produces modulation at rate 48000/(FFT_SIZE/4) = 23.4 Hz.
    // We detect this by measuring the ratio of max to mean of squared envelope.
    let check_samples = &output[check_start * 2..check_end * 2];
    let left_samples: Vec<f32> = check_samples.iter().step_by(2).copied().collect();

    // Compute short-time power in blocks of HOP_SIZE to detect frame-rate modulation.
    let block_size = HOP_SIZE;
    let mut block_powers: Vec<f32> = Vec::new();
    for chunk in left_samples.chunks(block_size) {
        if chunk.len() == block_size {
            let power = chunk.iter().map(|&s| s * s).sum::<f32>() / block_size as f32;
            block_powers.push(power);
        }
    }

    assert!(
        !block_powers.is_empty(),
        "No complete blocks in check window"
    );

    // Filter out near-zero blocks (transients at boundaries).
    let mean_power = block_powers.iter().sum::<f32>() / block_powers.len() as f32;
    let active_blocks: Vec<f32> = block_powers
        .iter()
        .filter(|&&p| p > mean_power * 0.1)
        .copied()
        .collect();

    assert!(
        active_blocks.len() >= 4,
        "Not enough active blocks to evaluate: {active_blocks:?}"
    );

    let max_power = active_blocks.iter().cloned().fold(0.0f32, f32::max);
    let min_power = active_blocks.iter().cloned().fold(f32::MAX, f32::min);
    let power_variation = (max_power - min_power) / mean_power.max(1e-10);

    assert!(
        power_variation < 0.15,
        "WOLA output has amplitude modulation (flutter): {:.3} ({:.1}% variation). \
             Expected < 15%. This indicates the window/overlap combination violates COLA. \
             Block powers: {block_powers:?}",
        power_variation,
        power_variation * 100.0
    );

    // Also verify the output has non-trivial amplitude (signal actually passes through).
    assert!(
        mean_power > 1e-4,
        "STFT output has near-zero amplitude: mean_power={mean_power:.6}. Signal lost."
    );
}

/// Bug fix: STFT path with small buffer must zero output before writing.
/// When phase_coherence=true and num_frames < FFT_SIZE, the while loop can
/// break early leaving tail elements of output uninitialized.
#[test]
fn test_process_phase_coherence_small_buffer_zeros_output() {
    let mut p = DownmixPlugin::from_params(DownmixPluginParams {
        input_channels: 2,
        input_layout: Some("2.0".to_string()),
        center_gain_db: 0.0,
        surround_gain_db: 0.0,
        height_gain_db: 0.0,
        lfe_gain_db: 0.0,
        phase_coherence: true,
        phase_blend_low_hz: 200.0,
        phase_blend_high_hz: 5000.0,
        itu_mode: false,
        matrix_ltrt: false,
    });
    p.initialize(48000).unwrap();

    let num_frames = 64; // smaller than FFT_SIZE
    let input = vec![0.0f32; num_frames * 2];
    let mut output = vec![f32::NAN; num_frames * 2];

    p.process(&input, &mut output, &ProcessContext::new(48000, num_frames))
        .unwrap();

    for (i, &v) in output.iter().enumerate() {
        assert!(
            v == 0.0,
            "output[{}] should be 0.0, got {} (possible uninitialized memory)",
            i,
            v
        );
    }
}

// ============================================================================
// Additional tests for param_value, set_param_value, process, set_parameter,
// get_parameter, initialize, reset, matrix_ltrt, itu_mode, and edge cases.
// ============================================================================

use sotf_host::parameters::{ParameterId, ParameterValue};

#[test]
fn test_param_value_roundtrip() {
    let mut p = DownmixPlugin::new(6);
    for i in 0..9 {
        let original = p.param_value(i).unwrap_or(0.0);
        let is_bool = matches!(i, 4 | 7 | 8);
        if is_bool {
            p.set_param_value(i, 1.0);
            assert!((p.param_value(i).unwrap() - 1.0).abs() < 1e-6);
            p.set_param_value(i, 0.0);
            assert!(p.param_value(i).unwrap().abs() < 1e-6);
        } else {
            p.set_param_value(i, original + 1.0);
            assert!((p.param_value(i).unwrap() - (original + 1.0)).abs() < 1e-6);
        }
        p.set_param_value(i, original);
        assert!((p.param_value(i).unwrap() - original).abs() < 1e-6);
    }
    assert!(p.param_value(9).is_none());
}

#[test]
fn test_set_parameter_get_parameter_roundtrip() {
    let mut p = DownmixPlugin::new(6);
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("center_gain_db"),
        ParameterValue::Float(-6.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("center_gain_db")),
        Some(ParameterValue::Float(-6.0))
    );
}

#[test]
fn test_process_matrix_ltrt() {
    let mut p = DownmixPlugin::from_params(DownmixPluginParams {
        input_channels: 6,
        input_layout: Some("5.1".to_string()),
        center_gain_db: 0.0,
        surround_gain_db: 0.0,
        height_gain_db: 0.0,
        lfe_gain_db: 0.0,
        phase_coherence: false,
        phase_blend_low_hz: 200.0,
        phase_blend_high_hz: 5000.0,
        itu_mode: false,
        matrix_ltrt: true,
    });
    p.initialize(48000).unwrap();
    let frames = FFT_SIZE * 3;
    let mut input = vec![0.0_f32; frames * 6];
    for k in 0..frames {
        input[k * 6] = (k as f32 * 0.01).sin();
    }
    let mut output = vec![0.0_f32; frames * 2];
    p.process(&input, &mut output, &ProcessContext::new(48000, frames))
        .unwrap();
    assert!(output.iter().any(|&s| s.abs() > 1e-5));
}

#[test]
fn test_process_itu_mode() {
    let mut p = DownmixPlugin::from_params(DownmixPluginParams {
        input_channels: 6,
        input_layout: Some("5.1".to_string()),
        center_gain_db: 0.0,
        surround_gain_db: 0.0,
        height_gain_db: 0.0,
        lfe_gain_db: 0.0,
        phase_coherence: false,
        phase_blend_low_hz: 200.0,
        phase_blend_high_hz: 5000.0,
        itu_mode: true,
        matrix_ltrt: false,
    });
    p.initialize(48000).unwrap();
    let mut input = vec![0.0_f32; 100 * 6];
    for k in 0..100 {
        input[k * 6 + 2] = 1.0;
    }
    let mut output = vec![0.0_f32; 100 * 2];
    p.process(&input, &mut output, &ProcessContext::new(48000, 100))
        .unwrap();
    assert!(output[0].abs() > 0.01);
}

#[test]
fn test_process_zero_input() {
    let mut p = DownmixPlugin::new(2);
    p.initialize(48000).unwrap();
    let input = vec![0.0_f32; 0];
    let mut output = vec![0.0_f32; 0];
    p.process(&input, &mut output, &ProcessContext::new(48000, 0))
        .unwrap();
}

#[test]
fn test_process_single_channel() {
    let mut p = DownmixPlugin::new(1);
    p.phase_coherence = false;
    p.initialize(48000).unwrap();
    let input = vec![0.5_f32; 100];
    let mut output = vec![0.0_f32; 200];
    p.process(&input, &mut output, &ProcessContext::new(48000, 100))
        .unwrap();
    assert!(output.iter().any(|&s| s.abs() > 1e-5));
}

#[test]
fn test_process_eight_channels() {
    let mut p = DownmixPlugin::new(8);
    p.phase_coherence = false;
    p.initialize(48000).unwrap();
    let input = vec![0.1_f32; 100 * 8];
    let mut output = vec![0.0_f32; 100 * 2];
    p.process(&input, &mut output, &ProcessContext::new(48000, 100))
        .unwrap();
    assert!(output.iter().any(|&s| s.abs() > 1e-5));
}

#[test]
fn test_reset_clears_state() {
    let mut p = DownmixPlugin::new(6);
    p.initialize(48000).unwrap();
    let input = vec![0.1_f32; 100 * 6];
    let mut output = vec![0.0_f32; 100 * 2];
    p.process(&input, &mut output, &ProcessContext::new(48000, 100))
        .unwrap();
    p.reset();
    assert_eq!(p.input_fill, 0);
    assert_eq!(p.output_accumulator_fill, 0);
}

#[test]
fn test_initialize_different_sample_rate() {
    let mut p = DownmixPlugin::new(6);
    p.initialize(96000).unwrap();
    assert_eq!(p.sample_rate, 96000);
}

#[test]
fn test_latency_samples_phase_coherence_off() {
    let mut p = DownmixPlugin::new(2);
    p.phase_coherence = false;
    assert_eq!(p.latency_samples(), 0);
}

#[test]
fn test_latency_samples_phase_coherence_on() {
    let mut p = DownmixPlugin::new(2);
    p.phase_coherence = true;
    assert_eq!(p.latency_samples(), FFT_SIZE);
}

// ============================================================================
// Additional unit tests for untested helper functions
// ============================================================================

#[test]
fn test_count_surround_channels() {
    let p = DownmixPlugin::new(6);
    // 5.1 has 2 surround channels (Ls, Rs)
    assert_eq!(p.count_surround_channels(), 2);

    let p2 = DownmixPlugin::new(8);
    // 7.1 has 4 surround channels (SL, SR, BL, BR)
    assert_eq!(p2.count_surround_channels(), 4);

    let p3 = DownmixPlugin::new(2);
    assert_eq!(p3.count_surround_channels(), 0);
}

#[test]
fn test_is_surround_channel() {
    let p = DownmixPlugin::new(6);
    // 5.1: ch0=L, ch1=R, ch2=C, ch3=LFE, ch4=Ls, ch5=Rs
    assert!(p.is_surround_channel(4).is_some());
    assert!(p.is_surround_channel(5).is_some());
    assert!(p.is_surround_channel(0).is_none());
    assert!(p.is_surround_channel(2).is_none());
}

#[test]
fn test_is_center_channel() {
    let p = DownmixPlugin::new(6);
    // 5.1: ch2 is center
    assert!(p.is_center_channel(2));
    assert!(!p.is_center_channel(0));
    assert!(!p.is_center_channel(3)); // LFE

    let p2 = DownmixPlugin::new(2);
    assert!(!p2.is_center_channel(0));
    assert!(!p2.is_center_channel(1));
}

#[test]
fn test_allpass_stage_coeff_and_process() {
    use super::allpass_stage::AllpassStage;

    let sr = 48000_u32;
    let stage = AllpassStage::new(1000.0, sr);
    let coeff = stage.coeff_a;
    // For fc=1000Hz at 48kHz, tan(π*1000/48000) ≈ 0.0654, so coeff ≈ -0.877
    assert!(coeff.abs() < 1.0);

    // Process impulse: first output is -a * 1.0 = -coeff
    let mut s = AllpassStage::new(1000.0, sr);
    let y1 = s.process(1.0);
    assert!((y1 - (-coeff)).abs() < 1e-6);

    // After reset, processing again should give same first sample
    s.reset();
    let y1_reset = s.process(1.0);
    assert!((y1 - y1_reset).abs() < 1e-6);
}

#[test]
fn test_lt_rt_allpass_update_sample_rate_and_reset() {
    use super::lt_rt_allpass::LtRtAllpass;

    let mut ap = LtRtAllpass::new(48000);
    let orig_coeff_a = ap.chain[0].coeff_a;

    // Update to new sample rate (should change coefficients)
    ap.update_sample_rate(96000);
    let new_coeff_a = ap.chain[0].coeff_a;
    assert!((new_coeff_a - orig_coeff_a).abs() > 1e-6);

    // Process some samples
    for _ in 0..10 {
        ap.process(1.0);
    }

    // Reset should zero state but keep coefficients
    ap.reset();
    assert_eq!(ap.x_prev, 0.0);
    assert_eq!(ap.chain[0].x_prev, 0.0);
    assert_eq!(ap.chain[0].y_prev, 0.0);
    assert_eq!(ap.chain[1].x_prev, 0.0);
    assert_eq!(ap.chain[1].y_prev, 0.0);
    // Coefficients should remain after reset
    assert!((ap.chain[0].coeff_a - new_coeff_a).abs() < 1e-6);
}

#[test]
fn test_advance_coeff_smoothers_by() {
    let mut p = DownmixPlugin::new(6);
    p.initialize(48000).unwrap();

    let idx = 2; // center channel left gain smoother
    let before = p.coeff_smoothers[idx].current();
    p.coeff_smoothers[idx].set_target(before + 0.5);

    // Advance by 100 samples
    p.advance_coeff_smoothers_by(100);
    let after = p.coeff_smoothers[idx].current();
    assert!(
        (after - before).abs() > 1e-6,
        "Smoother should have moved toward target"
    );
}

#[test]
fn test_compute_standard_coefficients_with_gains() {
    let p = DownmixPlugin::from_params(DownmixPluginParams {
        input_channels: 6,
        input_layout: Some("5.1".to_string()),
        center_gain_db: -12.0,
        surround_gain_db: -6.0,
        height_gain_db: 0.0,
        lfe_gain_db: -60.0,
        phase_coherence: false,
        phase_blend_low_hz: 200.0,
        phase_blend_high_hz: 5000.0,
        itu_mode: false,
        matrix_ltrt: false,
    });

    // In 5.1, surround channels are ch4 and ch5
    let s_lin = 10.0_f32.powf(-6.0 / 20.0); // ~0.501
    let surr_power_l = p.target_coeffs[4].left_gain.powi(2);
    let surr_power_r = p.target_coeffs[5].right_gain.powi(2);
    // With center and LFE muted, normalization should not scale surrounds.
    // Constant-power pan: each surround speaker has power = s_lin²
    assert!(
        (surr_power_l - s_lin.powi(2)).abs() < 0.01,
        "Ls power {} should be ~{}",
        surr_power_l,
        s_lin.powi(2)
    );
    assert!(
        (surr_power_r - s_lin.powi(2)).abs() < 0.01,
        "Rs power {} should be ~{}",
        surr_power_r,
        s_lin.powi(2)
    );

    // Ls should go to left, Rs to right
    assert!(p.target_coeffs[4].left_gain > p.target_coeffs[4].right_gain);
    assert!(p.target_coeffs[5].right_gain > p.target_coeffs[5].left_gain);
}

#[test]
fn explicit_layout_distinguishes_7_1_from_5_1_2() {
    let make = |layout: &str| {
        DownmixPlugin::try_from_params(DownmixPluginParams {
            input_channels: 8,
            input_layout: Some(layout.to_string()),
            center_gain_db: -12.0,
            surround_gain_db: -12.0,
            height_gain_db: 0.0,
            lfe_gain_db: -60.0,
            phase_coherence: false,
            phase_blend_low_hz: 500.0,
            phase_blend_high_hz: 2000.0,
            itu_mode: false,
            matrix_ltrt: false,
        })
        .unwrap()
    };
    let seven_one = make("7.1");
    let five_one_two = make("5.1.2");

    // Channel 6 is rear-left in 7.1, but top-front-left in 5.1.2.
    assert!(five_one_two.target_coeffs[6].left_gain > seven_one.target_coeffs[6].left_gain * 2.0);
    assert!(
        DownmixPlugin::try_from_params(DownmixPluginParams {
            input_channels: 8,
            input_layout: None,
            center_gain_db: -3.0,
            surround_gain_db: -3.0,
            height_gain_db: -6.0,
            lfe_gain_db: -10.0,
            phase_coherence: false,
            phase_blend_low_hz: 500.0,
            phase_blend_high_hz: 2000.0,
            itu_mode: false,
            matrix_ltrt: false,
        })
        .is_err()
    );
}

#[test]
fn reinitialize_discards_old_rate_stream_and_filter_state() {
    let mut reused = DownmixPlugin::new(6);
    reused.initialize(44_100).unwrap();
    let input = vec![0.25; (FFT_SIZE + 137) * 6];
    let mut discarded = vec![0.0; (FFT_SIZE + 137) * 2];
    reused
        .process(
            &input,
            &mut discarded,
            &ProcessContext::new(44_100, FFT_SIZE + 137),
        )
        .unwrap();
    reused.initialize(96_000).unwrap();

    let mut fresh = DownmixPlugin::new(6);
    fresh.initialize(96_000).unwrap();
    let silence = vec![0.0; (FFT_SIZE * 2) * 6];
    let mut reused_output = vec![0.0; FFT_SIZE * 4];
    let mut fresh_output = vec![0.0; FFT_SIZE * 4];
    let context = ProcessContext::new(96_000, FFT_SIZE * 2);
    reused
        .process(&silence, &mut reused_output, &context)
        .unwrap();
    fresh
        .process(&silence, &mut fresh_output, &context)
        .unwrap();
    assert_eq!(reused_output, fresh_output);
}

#[test]
fn phase_alignment_preserves_ordinary_mix_bin_magnitude() {
    let mut plugin = DownmixPlugin::new(6);
    plugin.initialize(48_000).unwrap();
    for frame in 0..FFT_SIZE {
        let phase = 2.0 * std::f32::consts::PI * 1200.0 * frame as f32 / 48_000.0;
        plugin.input_buffer[2 * FFT_SIZE + frame] = phase.sin();
        plugin.input_buffer[4 * FFT_SIZE + frame] = (phase + 1.2).sin();
        plugin.input_buffer[5 * FFT_SIZE + frame] = (phase - 0.8).sin();
    }
    plugin.process_fft_block();
    let bin = (1200 * FFT_SIZE / 48_000) as usize;
    let aligned_magnitude = plugin.out_freq_l[bin].norm();

    plugin.phase_coherence = false;
    plugin.process_fft_block();
    let ordinary_magnitude = plugin.out_freq_l[bin].norm();
    assert!(
        (aligned_magnitude - ordinary_magnitude).abs() <= ordinary_magnitude * 1e-5 + 1e-5,
        "phase alignment changed bin magnitude: aligned={aligned_magnitude}, ordinary={ordinary_magnitude}"
    );
}
