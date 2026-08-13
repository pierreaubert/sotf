use super::misc::adaptive_alpha;
use super::misc::compress_gr;
use super::misc::fft_size_from_index;
use super::misc::smooth_spectral_envelope;
use super::spectral_compressor_plugin::SpectralCompressorPlugin;
use super::spectral_compressor_plugin_params::SpectralCompressorPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;

mod misc;

#[test]
fn test_mix_zero_is_delayed_by_reported_latency_and_block_independent() {
    let params = SpectralCompressorPluginParams {
        mix: 0.0,
        ..Default::default()
    };
    let frames = 3072;
    let mut input = vec![0.0f32; frames * 2];
    input[0] = 1.0;
    input[1] = -1.0;

    let render = |chunk_frames: usize| {
        let mut plugin = SpectralCompressorPlugin::from_params(2, params.clone());
        plugin.initialize(48000).unwrap();
        let mut output = input.clone();
        for chunk in output.chunks_mut(chunk_frames * 2) {
            let nf = chunk.len() / 2;
            plugin
                .process_in_place(chunk, &ProcessContext::new(48000, nf))
                .unwrap();
        }
        (output, plugin.latency_samples())
    };

    let (single_block, latency) = render(frames);
    let (small_blocks, small_latency) = render(64);
    assert_eq!(latency, small_latency);
    assert_eq!(single_block, small_blocks);

    for frame in 0..latency {
        assert_eq!(single_block[frame * 2], 0.0);
        assert_eq!(single_block[frame * 2 + 1], 0.0);
    }
    assert_eq!(single_block[latency * 2], 1.0);
    assert_eq!(single_block[latency * 2 + 1], -1.0);
}

#[test]
fn partial_mix_has_fixed_latency_across_host_block_sizes() {
    let params = SpectralCompressorPluginParams {
        threshold_db: 0.0,
        ratio: 1.0,
        knee_db: 0.0,
        mix: 0.5,
        ..Default::default()
    };
    let frames = 8192;
    let input: Vec<f32> = (0..frames)
        .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48_000.0).sin())
        .collect();

    let render = |chunk_frames: usize| {
        let mut plugin = SpectralCompressorPlugin::from_params(1, params.clone());
        plugin.initialize(48_000).unwrap();
        let mut output = input.clone();
        for chunk in output.chunks_mut(chunk_frames) {
            let nf = chunk.len();
            plugin
                .process_in_place(chunk, &ProcessContext::new(48_000, nf))
                .unwrap();
        }
        output
    };

    let large = render(frames);
    let medium = render(1024);
    let small = render(64);
    let medium_diff = large
        .iter()
        .zip(&medium)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    let small_diff = large
        .iter()
        .zip(&small)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(medium_diff < 1.0e-5, "1024-frame max diff {medium_diff}");
    assert!(small_diff < 1.0e-5, "64-frame max diff {small_diff}");
}

/// Serialized construction obeys the public timing ranges.
#[test]
fn test_zero_attack_release_coefficients() {
    let params = SpectralCompressorPluginParams {
        attack_ms: 0.0,
        release_ms: 0.0,
        ..Default::default()
    };
    assert!(SpectralCompressorPlugin::try_from_params(2, params).is_err());
}

#[test]
fn test_spectral_smoothing_symmetric_bounds() {
    let original = vec![10.0, 2.0, 7.0, 0.0, 3.0, 9.0, 1.0];
    for amount in [0.0, 0.1, 0.5, 1.0] {
        let mut forward = original.clone();
        let mut reverse: Vec<_> = original.iter().copied().rev().collect();
        let mut forward_prefix = vec![0.0; forward.len() + 1];
        let mut reverse_prefix = vec![0.0; reverse.len() + 1];
        smooth_spectral_envelope(&mut forward, amount, &mut forward_prefix);
        smooth_spectral_envelope(&mut reverse, amount, &mut reverse_prefix);
        reverse.reverse();
        assert_eq!(
            forward, reverse,
            "smoothing changed under reversal at {amount}"
        );
    }

    let mut dc = vec![10.0, 0.0, 0.0, 0.0, 0.0];
    let mut nyquist = vec![0.0, 0.0, 0.0, 0.0, 10.0];
    let mut prefix = vec![0.0; 6];
    smooth_spectral_envelope(&mut dc, 1.0, &mut prefix);
    smooth_spectral_envelope(&mut nyquist, 1.0, &mut prefix);
    nyquist.reverse();
    assert_eq!(dc, nyquist);
    assert!(dc.iter().skip(1).any(|value| *value > 0.0));
    assert!(dc.iter().any(|value| *value < 10.0));
}

// -------------------------------------------------------------------------
// Pure helper tests
// -------------------------------------------------------------------------

#[test]
fn test_compress_gr_hard_knee() {
    assert_eq!(compress_gr(-10.0, -5.0, 4.0, 0.0), 0.0);
    let slope = 1.0 - 1.0 / 4.0;
    let gr = compress_gr(5.0, -5.0, 4.0, 0.0);
    assert!((gr - 10.0 * slope).abs() < 1e-5);
}

#[test]
fn test_compress_gr_soft_knee() {
    let slope = 1.0 - 1.0 / 4.0;
    assert_eq!(compress_gr(-10.0, 0.0, 4.0, 4.0), 0.0);
    let gr_above = compress_gr(10.0, 0.0, 4.0, 4.0);
    assert!((gr_above - 10.0 * slope).abs() < 1e-5);
    let gr_mid = compress_gr(0.0, 0.0, 4.0, 4.0);
    assert!(gr_mid > 0.0 && gr_mid < 2.0 * slope);
}

#[test]
fn test_smooth_spectral_envelope_edge_cases() {
    let mut empty: Vec<f32> = vec![];
    let mut empty_prefix = vec![0.0];
    smooth_spectral_envelope(&mut empty, 0.5, &mut empty_prefix); // must not panic
    let mut one = vec![7.0f32];
    let mut one_prefix = vec![0.0; 2];
    smooth_spectral_envelope(&mut one, 0.5, &mut one_prefix);
    assert_eq!(one[0], 7.0);
    let mut flat = vec![1.0f32; 4];
    let mut flat_prefix = vec![0.0; 5];
    smooth_spectral_envelope(&mut flat, 0.5, &mut flat_prefix);
    assert!(flat.iter().all(|&s| (s - 1.0).abs() < 1e-6));

    let original = vec![1.0, 2.0, 3.0, 4.0];
    let mut identity = original.clone();
    let mut identity_prefix = vec![0.0; 5];
    smooth_spectral_envelope(&mut identity, 0.0, &mut identity_prefix);
    assert_eq!(identity, original);
}

#[test]
fn adaptive_estimator_has_sample_rate_and_fft_invariant_time_constant() {
    const TAU_SECONDS: f32 = 0.5;
    for sample_rate in [44_100, 48_000, 96_000, 192_000] {
        for fft_size in [1024, 2048, 4096] {
            let hop = fft_size / 4;
            let alpha = adaptive_alpha(hop, sample_rate, TAU_SECONDS);
            let hops = (TAU_SECONDS * sample_rate as f32 / hop as f32).round() as usize;
            let remaining = alpha.powi(hops as i32);
            assert!(
                (remaining - (-1.0_f32).exp()).abs() < 0.025,
                "tau drifted at {sample_rate} Hz / N={fft_size}: {remaining}"
            );
        }
    }
}

#[test]
fn adaptive_processing_is_finite_across_supported_rates_and_fft_sizes() {
    for sample_rate in [44_100, 48_000, 96_000, 192_000] {
        for fft_size_index in 0..3 {
            let params = SpectralCompressorPluginParams {
                fft_size_index,
                adaptive_threshold: true,
                adaptive_offset_db: -6.0,
                ..Default::default()
            };
            let mut plugin = SpectralCompressorPlugin::from_params(2, params);
            plugin.initialize(sample_rate).unwrap();
            let frames = 8192;
            let mut audio = vec![0.0; frames * 2];
            for frame in 0..frames {
                let sample =
                    0.2 * (std::f32::consts::TAU * 997.0 * frame as f32 / sample_rate as f32).sin();
                audio[frame * 2] = sample;
                audio[frame * 2 + 1] = sample;
            }
            plugin
                .process_in_place(&mut audio, &ProcessContext::new(sample_rate, frames))
                .unwrap();
            assert!(audio.iter().all(|sample| sample.is_finite()));
        }
    }
}

#[test]
fn test_fft_size_from_index_out_of_range() {
    assert_eq!(fft_size_from_index(0), 1024);
    assert_eq!(fft_size_from_index(1), 2048);
    assert_eq!(fft_size_from_index(2), 4096);
    assert_eq!(fft_size_from_index(99), 2048);
}

// -------------------------------------------------------------------------
// Process tests for additional modes
// -------------------------------------------------------------------------

#[test]
fn test_delta_listen_outputs_difference_signal() {
    // Delta mode outputs (wet - dry). With active compression the delta
    // must be non-zero after the initial STFT latency.
    let params = SpectralCompressorPluginParams {
        threshold_db: -40.0,
        ratio: 8.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut plugin = SpectralCompressorPlugin::from_params(1, params);
    plugin.initialize(48000).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("delta_listen"),
            ParameterValue::Bool(true),
        )
        .unwrap();

    let nf = plugin.latency_samples() + 4096;
    let mut buf: Vec<f32> = (0..nf)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0).sin())
        .collect();
    let original = buf.clone();
    plugin
        .process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();

    let latency = plugin.latency_samples();
    let max_diff = buf[latency..]
        .iter()
        .zip(&original[latency..])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_diff > 0.001,
        "Delta listen should differ from dry when compression is active, got {max_diff}"
    );
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_adaptive_threshold_no_nan() {
    let params = SpectralCompressorPluginParams::default();
    let mut plugin = SpectralCompressorPlugin::from_params(2, params);
    plugin.initialize(48000).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("adaptive_threshold"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("adaptive_offset_db"),
            ParameterValue::Float(3.0),
        )
        .unwrap();

    let nf = 8192usize;
    let mut buf: Vec<f32> = (0..nf * 2)
        .map(|i| 0.1 * ((i / 2) as f32 * 0.05).sin())
        .collect();
    plugin
        .process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_target_mode_tonal_no_nan() {
    let params = SpectralCompressorPluginParams::default();
    let mut plugin = SpectralCompressorPlugin::from_params(2, params);
    plugin.initialize(48000).unwrap();
    plugin
        .set_parameter(ParameterId::from("target_mode"), ParameterValue::Int(1))
        .unwrap();

    let nf = 8192usize;
    let mut buf: Vec<f32> = (0..nf * 2)
        .map(|i| 0.2 * ((i / 2) as f32 * 0.03).sin())
        .collect();
    plugin
        .process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_reset_clears_stft_state() {
    let params = SpectralCompressorPluginParams::default();
    let mut plugin = SpectralCompressorPlugin::from_params(1, params);
    plugin.initialize(48000).unwrap();

    let nf = plugin.latency_samples() + 256;
    let mut buf = vec![0.0f32; nf];
    buf[plugin.latency_samples()] = 1.0;
    plugin
        .process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();

    plugin.reset();
    let mut silence = vec![0.0f32; nf];
    plugin
        .process_in_place(&mut silence, &ProcessContext::new(48000, nf))
        .unwrap();
    let max_abs = silence.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        max_abs < 1e-3,
        "After reset, silence should produce near-zero output, got {max_abs}"
    );
}

#[test]
fn test_recompute_coefficients_after_parameter_change() {
    let params = SpectralCompressorPluginParams {
        attack_ms: 5.0,
        release_ms: 50.0,
        ..Default::default()
    };
    let mut plugin = SpectralCompressorPlugin::from_params(1, params);
    plugin.initialize(48000).unwrap();
    let old_attack = plugin.attack_coeff;

    plugin
        .set_parameter(ParameterId::from("attack"), ParameterValue::Float(0.5))
        .unwrap();

    assert!(plugin.attack_coeff.is_finite());
    assert!(
        plugin.attack_coeff < old_attack,
        "Faster attack should produce a smaller EMA coefficient"
    );
}

#[test]
fn local_energy_detector_is_stable_across_fft_size_and_bin_alignment() {
    fn detector_gr(fft_index: usize, fractional_bin: f32) -> f32 {
        let params = SpectralCompressorPluginParams {
            fft_size_index: fft_index,
            threshold_db: -40.0,
            ratio: 8.0,
            attack_ms: 0.1,
            knee_db: 0.0,
            spectral_smoothing: 0.0,
            ..Default::default()
        };
        let mut plugin = SpectralCompressorPlugin::from_params(1, params);
        plugin.initialize(48_000).unwrap();
        let fft_size = plugin.fft_size;
        let bin = 43.0 + fractional_bin;
        for frame in 0..fft_size {
            plugin.stft.input_buffers[0][frame] =
                0.1 * (std::f32::consts::TAU * bin * frame as f32 / fft_size as f32).sin();
        }
        plugin.stft.input_fill = fft_size;
        plugin.process_spectral_hop();
        plugin.stft.detector_gr[0]
            .iter()
            .copied()
            .fold(0.0_f32, f32::max)
    }

    let reference = detector_gr(0, 0.0);
    for fft_index in 0..3 {
        for fractional_bin in [0.0, 0.25, 0.5] {
            let measured = detector_gr(fft_index, fractional_bin);
            assert!(
                (measured - reference).abs() < 0.75,
                "detector moved by {:.2} dB for FFT index {fft_index}, bin offset {fractional_bin}",
                measured - reference
            );
        }
    }
}

#[test]
fn channel_link_preserves_gain_for_correlated_layout_channels() {
    fn quiet_channel_envelope(link: f32) -> f32 {
        let params = SpectralCompressorPluginParams {
            fft_size_index: 0,
            threshold_db: -30.0,
            ratio: 10.0,
            attack_ms: 0.1,
            knee_db: 0.0,
            spectral_smoothing: 0.0,
            channel_link: link,
            ..Default::default()
        };
        let mut plugin = SpectralCompressorPlugin::from_params(2, params);
        plugin.initialize(48_000).unwrap();
        let fft_size = plugin.fft_size;
        for frame in 0..fft_size {
            let tone = (std::f32::consts::TAU * 32.0 * frame as f32 / fft_size as f32).sin();
            plugin.stft.input_buffers[0][frame] = 0.8 * tone;
            plugin.stft.input_buffers[1][frame] = 0.001 * tone;
        }
        plugin.stft.input_fill = fft_size;
        plugin.process_spectral_hop();
        plugin.stft.bin_envelopes[1][32]
    }

    let independent = quiet_channel_envelope(0.0);
    let linked = quiet_channel_envelope(1.0);
    assert!(
        independent < 0.1,
        "quiet independent channel compressed by {independent} dB"
    );
    assert!(
        linked > 10.0,
        "linked quiet channel did not follow the loud channel: {linked} dB"
    );
}

#[test]
fn adaptive_estimator_primes_from_first_valid_spectrum_and_reprime_on_enable() {
    let params = SpectralCompressorPluginParams {
        fft_size_index: 0,
        adaptive_threshold: true,
        ..Default::default()
    };
    let mut plugin = SpectralCompressorPlugin::from_params(1, params);
    plugin.initialize(48_000).unwrap();
    let fft_size = plugin.fft_size;
    for frame in 0..fft_size {
        plugin.stft.input_buffers[0][frame] =
            0.02 * (std::f32::consts::TAU * 24.0 * frame as f32 / fft_size as f32).sin();
    }
    plugin.stft.input_fill = fft_size;
    plugin.process_spectral_hop();
    assert!(plugin.stft.adaptive_initialized[0]);
    assert!(plugin.stft.adaptive_avg[0][24] < -25.0);
    assert!(plugin.stft.adaptive_avg[0][24] > -40.0);

    plugin
        .set_parameter(
            ParameterId::from("adaptive_threshold"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("adaptive_threshold"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    assert!(!plugin.stft.adaptive_initialized[0]);
}

#[test]
fn targeted_reset_matches_fresh_instance_for_initial_hops() {
    for target_mode in [1, 2] {
        let params = SpectralCompressorPluginParams {
            fft_size_index: 0,
            target_mode,
            threshold_db: -35.0,
            ratio: 6.0,
            ..Default::default()
        };
        let mut reset_plugin = SpectralCompressorPlugin::from_params(1, params.clone());
        let mut fresh_plugin = SpectralCompressorPlugin::from_params(1, params);
        reset_plugin.initialize(48_000).unwrap();
        fresh_plugin.initialize(48_000).unwrap();

        let mut precondition = vec![0.0; 8192];
        for (frame, sample) in precondition.iter_mut().enumerate() {
            *sample = if frame % 127 == 0 {
                0.8
            } else {
                0.2 * (std::f32::consts::TAU * 997.0 * frame as f32 / 48_000.0).sin()
            };
        }
        reset_plugin
            .process_in_place(&mut precondition, &ProcessContext::new(48_000, 8192))
            .unwrap();
        reset_plugin.reset();

        let mut reset_output = vec![0.0; 8192];
        for (frame, sample) in reset_output.iter_mut().enumerate() {
            *sample = 0.25 * (std::f32::consts::TAU * 733.0 * frame as f32 / 48_000.0).sin();
        }
        let mut fresh_output = reset_output.clone();
        reset_plugin
            .process_in_place(
                &mut reset_output,
                &ProcessContext::new(48_000, fresh_output.len()),
            )
            .unwrap();
        fresh_plugin
            .process_in_place(&mut fresh_output, &ProcessContext::new(48_000, 8192))
            .unwrap();
        let max_error = reset_output
            .iter()
            .zip(fresh_output)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_error < 1e-6,
            "target {target_mode} reset error: {max_error}"
        );
    }
}

#[test]
fn targeted_processing_has_no_cross_channel_control_leakage_at_twelve_channels() {
    fn render(channel_eleven_active: bool) -> Vec<f32> {
        let params = SpectralCompressorPluginParams {
            fft_size_index: 0,
            target_mode: 1,
            threshold_db: -35.0,
            ratio: 6.0,
            channel_link: 0.0,
            ..Default::default()
        };
        let mut plugin = SpectralCompressorPlugin::from_params(12, params);
        plugin.initialize(48_000).unwrap();
        let frames = 8192;
        let mut audio = vec![0.0; frames * 12];
        for frame in 0..frames {
            audio[frame * 12] =
                0.3 * (std::f32::consts::TAU * 997.0 * frame as f32 / 48_000.0).sin();
            if channel_eleven_active {
                audio[frame * 12 + 11] = if frame % 113 == 0 { 0.9 } else { 0.0 };
            }
        }
        plugin
            .process_in_place(&mut audio, &ProcessContext::new(48_000, frames))
            .unwrap();
        audio.chunks_exact(12).map(|frame| frame[0]).collect()
    }

    assert_eq!(render(false), render(true));
}
