use super::band_expander_params::BandExpanderParams;
use super::misc::MAX_BLOCK_FRAMES;
use super::misc::parse_detection_mode;
use super::multiband_expander_data::MultibandExpanderData;
use super::multiband_expander_plugin::MultibandExpanderPlugin;
use super::spectral_state::SpectralState;
use super::types::GateState;
use super::types::MultibandExpanderPluginParams;
use sotf_host::detector::DetectionMode;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;

#[test]
fn test_mb_exp_basic() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    let mut b = vec![0.1; 1000];
    p.process_in_place(&mut b, &ProcessContext::new(48000, 1000))
        .unwrap();
    assert!(b[999].is_finite());
}

/// Verify that low-frequency content triggers expansion in the lowest band
/// even with default detection settings (no sidechain HPF blocking bass).
#[test]
fn test_low_frequency_triggers_expansion() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 3,
        threshold_db: -20.0,
        ratio: 4.0,
        attack_ms: 1.0,
        release_ms: 50.0,
        range_db: 40.0,
        mix: 1.0,
        ..Default::default()
    };
    params.bands = vec![
        BandExpanderParams {
            threshold_db: Some(-20.0),
            ratio: Some(4.0),
            hold_ms: Some(0.0),
            hysteresis_db: Some(0.0),
            range_db: Some(40.0),
            ..Default::default()
        },
        BandExpanderParams::default(),
        BandExpanderParams::default(),
    ];
    let mut p = MultibandExpanderPlugin::with_params(1, params);
    p.initialize(48000).unwrap();

    // Feed a loud 50 Hz signal (above threshold) to open the gate
    let nf = 9600;
    let mut loud: Vec<f32> = (0..nf)
        .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / 48000.0).sin())
        .collect();
    let ctx = ProcessContext::new(48000, nf);
    p.process_in_place(&mut loud, &ctx).unwrap();

    // Verify the loud signal passed through with reasonable level
    let rms_loud: f32 =
        (loud[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
    assert!(
        rms_loud > 0.05,
        "Loud 50 Hz signal should pass through expander (gate open), RMS={rms_loud:.6}"
    );

    // Now feed a very quiet 50 Hz signal (below threshold)
    let quiet_amp = 0.001;
    let mut quiet: Vec<f32> = (0..nf)
        .map(|i| quiet_amp * (2.0 * std::f32::consts::PI * 50.0 * (nf + i) as f32 / 48000.0).sin())
        .collect();
    p.process_in_place(&mut quiet, &ctx).unwrap();

    // The quiet signal should be attenuated (gate closing)
    let rms_quiet: f32 =
        (quiet[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
    let input_rms = quiet_amp / std::f32::consts::SQRT_2;
    assert!(
        rms_quiet < input_rms,
        "Quiet 50 Hz signal should be attenuated by expander, \
             but rms_out={rms_quiet:.8} >= input_rms={input_rms:.8}"
    );
}

/// Regression: attack/release coefficients were swapped in per-band processing.
/// With fast attack and slow release, quiet signals below threshold should be
/// attenuated quickly (gate closes fast).
#[test]
fn test_mb_expander_attack_release_not_swapped() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 2,
        mix: 1.0,       // wet-only to observe expansion effect
        range_db: 60.0, // allow up to 60 dB of expansion attenuation
        ..Default::default()
    };
    params.bands = vec![
        BandExpanderParams {
            threshold_db: Some(-20.0),
            ratio: Some(10.0),
            attack_ms: Some(1.0),
            release_ms: Some(200.0),
            hold_ms: Some(0.0),
            hysteresis_db: Some(0.0),
            range_db: Some(60.0),
            ..Default::default()
        },
        BandExpanderParams {
            threshold_db: Some(-20.0),
            ratio: Some(10.0),
            attack_ms: Some(1.0),
            release_ms: Some(200.0),
            hold_ms: Some(0.0),
            hysteresis_db: Some(0.0),
            range_db: Some(60.0),
            ..Default::default()
        },
    ];
    let mut p = MultibandExpanderPlugin::with_params(1, params);
    p.initialize(48000).unwrap();

    // Feed loud broadband signal to open gates
    let mut loud = Vec::with_capacity(9600);
    for i in 0..9600 {
        loud.push(0.5 * (i as f32 * 0.3).sin());
    }
    p.process_in_place(&mut loud, &ProcessContext::new(48000, 9600))
        .unwrap();

    // Feed quiet broadband signal — gates should close fast with 1ms attack
    let quiet_peak = 0.001f32;
    let mut quiet = Vec::with_capacity(2400);
    for i in 0..2400 {
        quiet.push(quiet_peak * (i as f32 * 0.3).sin());
    }
    let quiet_rms_in: f32 = (quiet.iter().map(|s| s * s).sum::<f32>() / quiet.len() as f32).sqrt();
    p.process_in_place(&mut quiet, &ProcessContext::new(48000, 2400))
        .unwrap();

    // After 50ms with 1ms attack (and 0ms hold), the signal should be attenuated.
    let quiet_rms_out: f32 =
        (quiet[1200..].iter().map(|s| s * s).sum::<f32>() / (quiet.len() - 1200) as f32).sqrt();
    assert!(
        quiet_rms_out < quiet_rms_in * 0.8,
        "Multiband expander gate should close fast with 1ms attack, \
             but RMS out {quiet_rms_out:.6} is too close to RMS in {quiet_rms_in:.6}. \
             Attack/release coefficients may be swapped."
    );
}

/// Unity passthrough: with threshold at minimum and ratio 1:1,
/// the expander should not alter the signal significantly.
#[test]
fn test_mb_expander_unity_passthrough() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 3,
        ..Default::default()
    };
    for band in &mut params.bands {
        band.ratio = Some(1.0); // no expansion
    }
    let mut p = MultibandExpanderPlugin::with_params(2, params);
    p.initialize(48000).unwrap();

    // Generate test signal
    let mut input = vec![0.0f32; 4800 * 2];
    for i in 0..4800 {
        let val = 0.3 * (i as f32 * 0.05).sin();
        input[i * 2] = val;
        input[i * 2 + 1] = val;
    }
    let mut output = input.clone();
    p.process_in_place(&mut output, &ProcessContext::new(48000, 4800))
        .unwrap();

    // After settling (crossover filter delay), output should be close to input.
    // Allow for crossover phase shift but RMS should be similar.
    let rms_in: f32 =
        (input[2400..].iter().map(|s| s * s).sum::<f32>() / (input.len() - 2400) as f32).sqrt();
    let rms_out: f32 =
        (output[2400..].iter().map(|s| s * s).sum::<f32>() / (output.len() - 2400) as f32).sqrt();
    let ratio = rms_out / rms_in;
    assert!(
        (0.7..1.3).contains(&ratio),
        "Unity ratio (1:1) should pass through, but RMS ratio is {ratio:.3}"
    );
}

/// Spectral mode: basic smoke test — output must be finite and non-silent for a
/// loud input signal (threshold set very low so gate is open).
#[test]
fn test_spectral_mode_basic() {
    let params = MultibandExpanderPluginParams {
        num_bands: 3,
        threshold_db: -80.0, // very low threshold: gate always open
        ratio: 2.0,
        attack_ms: 5.0,
        release_ms: 50.0,
        range_db: 40.0,
        mix: 1.0,
        processing_mode: "spectral".to_string(),
        ..Default::default()
    };
    let mut p = MultibandExpanderPlugin::with_params(2, params);
    p.initialize(48000).unwrap();

    // The streaming scheduler emits a fixed one-window causal latency.
    assert_eq!(
        p.latency_samples(),
        1024,
        "Spectral mode latency should match its streamed one-window delay"
    );

    // Generate pink-noise-like signal using sum of sines
    let nf = 8192usize;
    let mut signal = vec![0.0f32; nf * 2];
    for i in 0..nf {
        let t = i as f32 / 48000.0;
        let s = 0.3 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
            + 0.15 * (2.0 * std::f32::consts::PI * 880.0 * t).sin()
            + 0.08 * (2.0 * std::f32::consts::PI * 3520.0 * t).sin();
        signal[i * 2] = s;
        signal[i * 2 + 1] = s;
    }

    let mut buf = signal.clone();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();

    // All output samples must be finite
    for (i, &s) in buf.iter().enumerate() {
        assert!(s.is_finite(), "Sample {i} is not finite: {s}");
    }

    // After the latency fill (~fft_size - hop_size = 512 frames), output must not be all-zeros
    let rms_out: f32 =
        (buf[512 * 2..].iter().map(|s| s * s).sum::<f32>() / ((nf - 512) * 2) as f32).sqrt();
    assert!(
        rms_out > 1e-5,
        "Spectral mode output should not be silent for loud input, RMS={rms_out:.8}"
    );
}

#[test]
fn spectral_streamed_impulse_delay_is_block_size_independent() {
    for block_size in [64usize, 128, 256, 512, 1024] {
        let mut params = MultibandExpanderPluginParams {
            num_bands: 3,
            threshold_db: -100.0,
            ratio: 1.0,
            mix: 1.0,
            processing_mode: "spectral".to_string(),
            ..Default::default()
        };
        for band in &mut params.bands {
            band.ratio = Some(1.0);
        }
        let mut plugin = MultibandExpanderPlugin::with_params(1, params);
        plugin.initialize(48_000).unwrap();

        let fft_size = plugin.spectral.as_ref().unwrap().fft_size;
        let impulse_index = fft_size / 2;
        let total_frames = fft_size * 5;
        let mut input = vec![0.0f32; total_frames];
        input[impulse_index] = 1.0;
        let mut output = Vec::with_capacity(total_frames);
        for chunk in input.chunks(block_size) {
            let mut block = chunk.to_vec();
            plugin
                .process_in_place(&mut block, &ProcessContext::new(48_000, chunk.len()))
                .unwrap();
            output.extend_from_slice(&block);
        }

        let peak_index = output
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .map(|(index, _)| index)
            .unwrap();
        assert_eq!(
            peak_index.saturating_sub(impulse_index),
            plugin.latency_samples(),
            "block size {block_size} changed spectral latency"
        );
    }
}

#[test]
fn spectral_hops_reuse_preallocated_band_metadata() {
    let params = MultibandExpanderPluginParams {
        num_bands: 3,
        processing_mode: "spectral".to_string(),
        ..Default::default()
    };
    let mut plugin = MultibandExpanderPlugin::with_params(1, params);
    plugin.initialize(48_000).unwrap();

    let band_info_ptr = plugin.spectral.as_ref().unwrap().band_info.as_ptr();
    let band_info_capacity = plugin.spectral.as_ref().unwrap().band_info.capacity();
    let mut signal = vec![0.1f32; 4096];
    plugin
        .process_in_place(&mut signal, &ProcessContext::new(48_000, 4096))
        .unwrap();

    let band_info = &plugin.spectral.as_ref().unwrap().band_info;
    assert_eq!(band_info.as_ptr(), band_info_ptr);
    assert_eq!(band_info.capacity(), band_info_capacity);
    let source = include_str!("multiband_expander_plugin.rs");
    assert!(
        !source.contains("let band_info: Vec"),
        "spectral hop must not allocate a band-info Vec"
    );
}

/// Integration test: verify that the multiband expander actually attenuates audio.
///
/// A quiet DC-offset signal at -40 dBFS is fed to a 2-band expander whose
/// threshold is set at -20 dB and ratio at 4:1.  After processing, the
/// output RMS must be lower than the input RMS — confirming that expansion
/// is being applied and not just passing audio through unchanged.
#[test]
fn test_multiband_expander_processes_audio() {
    let params = MultibandExpanderPluginParams {
        num_bands: 2,
        threshold_db: -20.0,
        ratio: 4.0,
        attack_ms: 1.0,
        release_ms: 50.0,
        range_db: 60.0,
        hold_ms: 0.0,
        hysteresis_db: 0.0,
        mix: 1.0,
        ..Default::default()
    };
    let mut p = MultibandExpanderPlugin::with_params(1, params);
    p.initialize(48000).unwrap();

    // Quiet DC-offset signal at -40 dBFS (well below -20 dB threshold)
    let amp = 10.0_f32.powf(-40.0 / 20.0);
    let num_frames = 48000usize; // 1 second
    let mut buffer = vec![amp; num_frames];

    let input_rms = amp; // DC: RMS == amplitude

    let ctx = ProcessContext::new(48000, num_frames);
    p.process_in_place(&mut buffer, &ctx).unwrap();

    // Measure RMS of the second half to let the expander settle
    let half = num_frames / 2;
    let output_rms: f32 =
        (buffer[half..].iter().map(|s| s * s).sum::<f32>() / (num_frames - half) as f32).sqrt();

    assert!(
        output_rms < input_rms * 0.9,
        "Multiband expander should attenuate a -40 dBFS signal below the -20 dB threshold, \
             but output_rms={output_rms:.8} is not significantly less than input_rms={input_rms:.8}"
    );
}

/// Spectral mode: below-threshold signal should be attenuated compared to time-domain mode.
///
/// Both modes are configured identically. A quiet signal (below threshold) is fed to each.
/// The spectral mode attenuation is compared against the time-domain mode attenuation.
/// We do not require them to be identical (STFT resolution differs from sample-accurate
/// tracking), but both should attenuate significantly relative to the unprocessed signal.
#[test]
fn test_spectral_vs_time_domain_attenuation() {
    let sr = 48000u32;
    let nf = 16384usize; // enough for multiple STFT hops

    // Quiet broadband signal (below a -20 dB threshold)
    let quiet_amp = 0.005f32; // ~ -46 dBFS
    let signal: Vec<f32> = (0..nf)
        .map(|i| {
            quiet_amp
                * ((2.0 * std::f32::consts::PI * 200.0 * i as f32 / sr as f32).sin()
                    + (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin()
                    + (2.0 * std::f32::consts::PI * 4000.0 * i as f32 / sr as f32).sin())
                / 3.0
        })
        .collect();
    let input_rms: f32 = (signal.iter().map(|s| s * s).sum::<f32>() / nf as f32).sqrt();

    let make_params = |mode: &str| MultibandExpanderPluginParams {
        num_bands: 3,
        threshold_db: -20.0,
        ratio: 8.0,
        attack_ms: 10.0,
        release_ms: 100.0,
        knee_db: 0.0,
        range_db: 60.0,
        hysteresis_db: 0.0,
        hold_ms: 0.0,
        mix: 1.0,
        processing_mode: mode.to_string(),
        crossover_frequencies: vec![300.0, 3000.0, 8000.0, 12000.0],
        ..Default::default()
    };

    let mut td_plugin = MultibandExpanderPlugin::with_params(1, make_params("time_domain"));
    td_plugin.initialize(sr).unwrap();

    let mut sp_plugin = MultibandExpanderPlugin::with_params(1, make_params("spectral"));
    sp_plugin.initialize(sr).unwrap();

    let ctx = ProcessContext::new(sr, nf);

    let mut td_buf = signal.clone();
    td_plugin.process_in_place(&mut td_buf, &ctx).unwrap();

    let mut sp_buf = signal.clone();
    sp_plugin.process_in_place(&mut sp_buf, &ctx).unwrap();

    // Use the second half of the buffer to avoid transient settling
    let half = nf / 2;
    let td_rms: f32 =
        (td_buf[half..].iter().map(|s| s * s).sum::<f32>() / (nf - half) as f32).sqrt();
    let sp_rms: f32 =
        (sp_buf[half..].iter().map(|s| s * s).sum::<f32>() / (nf - half) as f32).sqrt();

    // Both modes should attenuate: output RMS must be < 80% of input RMS
    assert!(
        td_rms < input_rms * 0.8,
        "Time-domain mode should attenuate below-threshold signal, \
             input_rms={input_rms:.6}, td_rms={td_rms:.6}"
    );
    // Spectral mode has STFT latency and OLA settling; use a looser threshold
    assert!(
        sp_rms < input_rms * 0.98,
        "Spectral mode should attenuate below-threshold signal, \
             input_rms={input_rms:.6}, sp_rms={sp_rms:.6}"
    );
}

/// Regression: spectral mode with more than 5 bands must not panic.
///
/// Before the fix the hop function used a fixed [BandInfo; 5] array; any
/// bin assigned to band index >= 5 would be an out-of-bounds access.
#[test]
fn test_spectral_mode_more_than_5_bands_no_panic() {
    // Max allowed num_bands from params spec is 5 for the current clamp, but the
    // underlying code should not panic even if constructed directly.
    // We test num_bands = 5 (the max) to exercise the Vec path.
    let params = MultibandExpanderPluginParams {
        num_bands: 5,
        threshold_db: -80.0,
        ratio: 2.0,
        mix: 1.0,
        processing_mode: "spectral".to_string(),
        crossover_frequencies: vec![200.0, 800.0, 3200.0, 10000.0],
        ..Default::default()
    };
    let mut p = MultibandExpanderPlugin::with_params(2, params);
    p.initialize(48000).unwrap();
    let mut buf = vec![0.1f32; 4096 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 4096))
        .unwrap();
    assert!(
        buf.iter().all(|s| s.is_finite()),
        "All samples must be finite"
    );
}

/// The causal streaming schedule reports the one-window delay measured by the
/// chunked impulse regression above.
#[test]
fn test_spectral_mode_latency_correct() {
    let params = MultibandExpanderPluginParams {
        num_bands: 2,
        processing_mode: "spectral".to_string(),
        ..Default::default()
    };
    let mut p = MultibandExpanderPlugin::with_params(2, params);
    p.initialize(48000).unwrap();
    assert_eq!(p.latency_samples(), 1024);
}

#[test]
fn test_time_domain_chunks_oversized_blocks_without_resizing() {
    let mut p = MultibandExpanderPlugin::new(2);
    p.initialize(48000).unwrap();
    let dry_len = p.dry_buffer.len();
    let band_len = p.band_buffers.len();

    let frames = MAX_BLOCK_FRAMES + 1;
    let mut buf = vec![0.0f32; frames * 2];
    let processed = p
        .process_in_place(&mut buf, &ProcessContext::new(48000, frames))
        .unwrap();

    assert_eq!(processed, frames);
    assert_eq!(p.dry_buffer.len(), dry_len);
    assert_eq!(p.band_buffers.len(), band_len);
    assert!(buf.iter().all(|s| s.is_finite()));
}

/// Regression: measured auto-makeup tracker must not jitter on stereo material.
///
/// With the bug, per-channel loop updates interleaved L/R envelopes into a
/// single tracker, causing make-up gain to oscillate. The fix updates once per
/// frame using max(L, R) envelope. We verify no NaNs and that the plugin
/// processes without panicking.
#[test]
fn test_measured_auto_makeup_stereo_no_jitter() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 2,
        threshold_db: -30.0,
        ratio: 4.0,
        mix: 1.0,
        ..Default::default()
    };
    params.bands = vec![
        BandExpanderParams {
            measured_auto_makeup: true,
            threshold_db: Some(-30.0),
            ratio: Some(4.0),
            ..Default::default()
        },
        BandExpanderParams {
            measured_auto_makeup: true,
            threshold_db: Some(-30.0),
            ratio: Some(4.0),
            ..Default::default()
        },
    ];
    let mut p = MultibandExpanderPlugin::with_params(2, params);
    p.initialize(48000).unwrap();

    // Feed stereo signal with different L/R amplitudes to stress test makeup tracker
    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf)
        .flat_map(|i| {
            let t = i as f32 / 48000.0;
            let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.05;
            [s, s * 0.7]
        })
        .collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(
        buf.iter().all(|s| s.is_finite()),
        "Stereo measured auto-makeup must not produce NaN/inf"
    );
}

/// Regression: initialize() must call reset() so old envelope state doesn't
/// leak across sample-rate changes or re-initialization.
#[test]
fn test_initialize_clears_state() {
    let mut p = MultibandExpanderPlugin::new(2);
    p.initialize(48000).unwrap();

    // Run a loud signal to drive up envelope state
    let nf = 4800usize;
    let mut loud: Vec<f32> = (0..nf * 2).map(|_| 0.8).collect();
    p.process_in_place(&mut loud, &ProcessContext::new(48000, nf))
        .unwrap();

    // Re-initialize (simulates host sample-rate change)
    p.initialize(48000).unwrap();

    // Feed silence — if state was not cleared, residual envelope might still be active.
    // The output must be finite (not NaN/inf from a contaminated envelope).
    let mut silent = vec![0.0f32; nf * 2];
    p.process_in_place(&mut silent, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(
        silent.iter().all(|s| s.is_finite()),
        "Output after re-initialize must be finite"
    );
}

/// Regression: dry/wet mix must not produce comb-filter artifacts when
/// lookahead is active. We verify that with mix=0 (dry only) the output
/// equals the input delayed by lookahead_samples, not the undelayed input.
#[test]
fn test_lookahead_dry_path_is_latency_compensated() {
    let la_ms = 5.0f32;
    let sr = 48000u32;
    let params = MultibandExpanderPluginParams {
        num_bands: 2,
        lookahead_ms: la_ms,
        mix: 0.0,            // pure dry — output should be a delayed copy of input
        threshold_db: -80.0, // gate always open
        ratio: 1.0,
        ..Default::default()
    };
    let mut p = MultibandExpanderPlugin::with_params(1, params);
    p.initialize(sr).unwrap();

    let la_samples = (la_ms * 0.001 * sr as f32).round() as usize;
    // Build a 1-second buffer with an impulse at position `la_samples`
    let nf = sr as usize;
    let mut buf = vec![0.0f32; nf];
    buf[la_samples] = 1.0;

    p.process_in_place(&mut buf, &ProcessContext::new(sr, nf))
        .unwrap();

    // With latency-compensated dry path and mix=0:
    // The impulse at input[la_samples] should appear at output[la_samples + la_samples],
    // but since the LookaheadBuffer pre-fills with zeros, the output should have a peak
    // somewhere near la_samples * 2 and be zeroed at position 0.
    // More practically: output[0..la_samples] must be ~zero (no premature signal).
    let early_energy: f32 = buf[..la_samples].iter().map(|s| s * s).sum();
    assert!(
        early_energy < 1e-10,
        "With compensated dry path, no energy before the lookahead window. \
             Got early_energy={early_energy}"
    );
}

// -------------------------------------------------------------------------
// Pure helper tests
// -------------------------------------------------------------------------

#[test]
fn test_parse_detection_mode() {
    assert_eq!(
        parse_detection_mode("rms"),
        Ok(DetectionMode::Rms { window_ms: 10.0 })
    );
    assert_eq!(parse_detection_mode("peak"), Ok(DetectionMode::Peak));
    assert_eq!(
        parse_detection_mode("RMS"),
        Ok(DetectionMode::Rms { window_ms: 10.0 })
    );
    assert!(parse_detection_mode("unknown").is_err());
}

#[test]
fn test_calculate_expansion_attenuation() {
    let th = -10.0f32;
    let ratio = 4.0f32;
    let knee = 0.0f32;
    let range = 20.0f32;
    // Above threshold -> no attenuation
    assert_eq!(
        MultibandExpanderPlugin::calculate_expansion_attenuation(-5.0, th, ratio, knee, range),
        0.0
    );
    // Well below -> capped at range
    let att =
        MultibandExpanderPlugin::calculate_expansion_attenuation(-40.0, th, ratio, knee, range);
    assert_eq!(att, range);
    let slope = 1.0 - 1.0 / ratio.max(1.0);
    let att2 =
        MultibandExpanderPlugin::calculate_expansion_attenuation(-15.0, th, ratio, knee, range);
    assert!((att2 - 5.0 * slope).abs() < 1e-5);
}

// -------------------------------------------------------------------------
// Parameter round-trip and setter tests
// -------------------------------------------------------------------------

#[test]
fn test_global_param_value_roundtrip() {
    let mut p = MultibandExpanderPlugin::new(2);
    p.initialize(48000).unwrap();

    p.set_param_value(6, -25.0); // threshold
    p.set_param_value(17, 5.0); // lookahead_ms
    assert!((p.param_value(6).unwrap() - (-25.0)).abs() < 1e-6);
    assert!((p.param_value(17).unwrap() - 5.0).abs() < 1e-6);

    // detection_mode maps to index 1 for RMS
    p.set_param_value(16, 1.0);
    assert_eq!(p.param_value(16).unwrap(), 1.0);
    assert_eq!(p.detection_mode, "rms");
}

#[test]
fn test_processing_mode_is_structural() {
    let mut p = MultibandExpanderPlugin::new(2);
    p.initialize(48000).unwrap();
    assert!(p.spectral.is_none());

    assert!(
        p.set_parameter(ParameterId::from("processing_mode"), ParameterValue::Int(1))
            .is_err()
    );
    assert!(p.spectral.is_none());
}

#[test]
fn test_num_bands_is_structural_in_spectral_mode() {
    let mut p = MultibandExpanderPlugin::with_params(
        2,
        MultibandExpanderPluginParams {
            num_bands: 2,
            processing_mode: "spectral".to_string(),
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    let max_band_before = p
        .spectral
        .as_ref()
        .unwrap()
        .bin_to_band
        .iter()
        .max()
        .copied()
        .unwrap();
    assert_eq!(max_band_before, 1);

    assert!(
        p.set_parameter(ParameterId::from("num_bands"), ParameterValue::Int(4))
            .is_err()
    );
    assert_eq!(p.num_bands, 2);
}

#[test]
fn test_set_parameter_invalid_band_index_error() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    let res = p.set_parameter(
        ParameterId::from("band_99_threshold"),
        ParameterValue::Float(-10.0),
    );
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("out of range"));
}

#[test]
fn test_set_parameter_unknown_returns_error() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    let res = p.set_parameter(
        ParameterId::from("not_a_real_param"),
        ParameterValue::Float(0.0),
    );
    assert!(res.is_err());
}

#[test]
fn test_lookahead_ms_roundtrip_and_clamp() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("lookahead_ms"),
        ParameterValue::Float(25.0),
    )
    .unwrap();
    let v = p.get_parameter(&ParameterId::from("lookahead_ms")).unwrap();
    assert_eq!(v, ParameterValue::Float(20.0));
    assert_eq!(
        p.latency_samples(),
        (20.0_f32 * 0.001 * 48000.0).round() as usize
    );
}

// -------------------------------------------------------------------------
// Initialization / reset / process smoke tests
// -------------------------------------------------------------------------

#[test]
fn test_reset_clears_expander_gate_state() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 1,
            threshold_db: -20.0,
            ratio: 10.0,
            range_db: 60.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    let mut buf = vec![0.001f32; 4800];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 4800))
        .unwrap();

    // Gate should have reacted (envelope > 0 or state moved)
    assert!(p.band_expanders[0].envelope[0] > 0.0);

    p.reset();
    assert_eq!(p.band_expanders[0].gate_state[0], GateState::Open);
    assert_eq!(p.band_expanders[0].envelope[0], 0.0);
}

#[test]
fn test_process_silence_finite() {
    let mut p = MultibandExpanderPlugin::new(2);
    p.initialize(48000).unwrap();
    let mut buf = vec![0.0f32; 512 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_time_domain_unity_ratio_passthrough() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 2,
            ratio: 1.0,
            threshold_db: -20.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    let nf = 2048usize;
    let mut buf: Vec<f32> = (0..nf).map(|i| 0.2 * (i as f32 * 0.1).sin()).collect();
    let original = buf.clone();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();

    let rms_in = (original.iter().map(|s| s * s).sum::<f32>() / nf as f32).sqrt();
    let rms_out = (buf.iter().map(|s| s * s).sum::<f32>() / nf as f32).sqrt();
    let ratio = rms_out / rms_in;
    assert!(
        (0.7..1.3).contains(&ratio),
        "Unity ratio should pass through, ratio={ratio:.3}"
    );
}

// -------------------------------------------------------------------------
// Plugin interface tests
// -------------------------------------------------------------------------

#[test]
fn test_plugin_interface() {
    let p = MultibandExpanderPlugin::new(2);
    assert_eq!(p.channels(), 2);
    assert_eq!(p.info().name, "Multiband Expander");
    assert!(!p.parameters().is_empty());
}

#[test]
fn test_get_data() {
    let p = MultibandExpanderPlugin::new(2);
    let data = p.get_data();
    assert!(data.is_some());
}

#[test]
fn test_from_params() {
    let params = MultibandExpanderPluginParams::default();
    let p = MultibandExpanderPlugin::from_params(2, params);
    assert_eq!(p.channels(), 2);
}

// -------------------------------------------------------------------------
// param_value / set_param_value edge cases
// -------------------------------------------------------------------------

#[test]
fn test_param_value_out_of_range() {
    let p = MultibandExpanderPlugin::new(1);
    assert!(p.param_value(99).is_none());
}

#[test]
fn test_set_param_value_out_of_range() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.set_param_value(99, 1.0); // should not panic
}

// -------------------------------------------------------------------------
// get_parameter comprehensive tests
// -------------------------------------------------------------------------

#[test]
fn test_get_parameter_global() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    let v = p.get_parameter(&ParameterId::from("threshold")).unwrap();
    assert!(matches!(v, ParameterValue::Float(_)));
}

#[test]
fn test_get_parameter_band_fields() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 2,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_threshold"))
            .unwrap(),
        ParameterValue::Float(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_ratio")).unwrap(),
        ParameterValue::Float(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_attack"))
            .unwrap(),
        ParameterValue::Float(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_release"))
            .unwrap(),
        ParameterValue::Float(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_knee")).unwrap(),
        ParameterValue::Float(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_range")).unwrap(),
        ParameterValue::Float(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_hysteresis"))
            .unwrap(),
        ParameterValue::Float(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_hold")).unwrap(),
        ParameterValue::Float(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_auto")).unwrap(),
        ParameterValue::Bool(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_measured"))
            .unwrap(),
        ParameterValue::Bool(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_active"))
            .unwrap(),
        ParameterValue::Bool(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_solo")).unwrap(),
        ParameterValue::Bool(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("band_0_bypass"))
            .unwrap(),
        ParameterValue::Bool(_)
    ));
}

#[test]
fn test_get_parameter_invalid_band() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    assert!(
        p.get_parameter(&ParameterId::from("band_99_threshold"))
            .is_none()
    );
}

#[test]
fn test_get_parameter_unknown() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    assert!(
        p.get_parameter(&ParameterId::from("unknown_param"))
            .is_none()
    );
}

#[test]
fn test_get_parameter_single_band_aliases() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();

    assert!(matches!(
        p.get_parameter(&ParameterId::from("auto_makeup")).unwrap(),
        ParameterValue::Bool(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("measured_auto_makeup"))
            .unwrap(),
        ParameterValue::Bool(_)
    ));
    assert!(matches!(
        p.get_parameter(&ParameterId::from("sidechain_hpf_hz"))
            .unwrap(),
        ParameterValue::Float(_)
    ));
}

// -------------------------------------------------------------------------
// set_parameter comprehensive tests with side effects
// -------------------------------------------------------------------------

#[test]
fn test_set_parameter_crossover_freq() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 3,
            processing_mode: "spectral".to_string(),
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    p.set_parameter(
        ParameterId::from("crossover_freq_1"),
        ParameterValue::Float(500.0),
    )
    .unwrap();
    assert!((p.crossover_frequencies[0] - 500.0).abs() < 1e-3);
}

#[test]
fn test_set_parameter_all_crossover_freqs() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();

    p.set_parameter(
        ParameterId::from("crossover_freq_2"),
        ParameterValue::Float(800.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("crossover_freq_3"),
        ParameterValue::Float(6000.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("crossover_freq_4"),
        ParameterValue::Float(11000.0),
    )
    .unwrap();

    assert!((p.crossover_frequencies[1] - 800.0).abs() < 1e-3);
    assert!((p.crossover_frequencies[2] - 6000.0).abs() < 1e-3);
    assert!((p.crossover_frequencies[3] - 11000.0).abs() < 1e-3);
}

#[test]
fn test_set_parameter_threshold() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    p.set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
        .unwrap();
    assert!((p.threshold_db - (-30.0)).abs() < 1e-6);
}

#[test]
fn test_set_parameter_mix() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
        .unwrap();
    assert!((p.mix - 0.5).abs() < 1e-6);
}

#[test]
fn automation_values_are_independent_of_host_block_partitioning() {
    fn render(chunks: &[usize]) -> Vec<[f32; 2]> {
        let mut plugin = MultibandExpanderPlugin::new(1);
        plugin.initialize(48_000).unwrap();
        plugin
            .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
            .unwrap();
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.25))
            .unwrap();

        let mut values = Vec::new();
        for &nf in chunks {
            let mut buffer = vec![0.0; nf];
            plugin
                .process_in_place(&mut buffer, &ProcessContext::new(48_000, nf))
                .unwrap();
            values.extend_from_slice(&plugin.automation_values[..nf]);
        }
        values
    }

    let one_block = render(&[1024]);
    let small_blocks = render(&[64; 16]);
    assert_eq!(one_block.len(), small_blocks.len());
    for (a, b) in one_block.iter().zip(&small_blocks) {
        assert!((a[0] - b[0]).abs() < 1.0e-6);
        assert!((a[1] - b[1]).abs() < 1.0e-6);
    }
}

#[test]
fn test_set_parameter_attack_release_updates_coefficients() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    let old_attack = p.band_expanders[0].attack_coeff;
    let old_release = p.band_expanders[0].release_coeff;

    p.set_parameter(ParameterId::from("attack"), ParameterValue::Float(10.0))
        .unwrap();
    p.set_parameter(ParameterId::from("release"), ParameterValue::Float(500.0))
        .unwrap();

    assert_ne!(p.band_expanders[0].attack_coeff, old_attack);
    assert_ne!(p.band_expanders[0].release_coeff, old_release);
}

#[test]
fn test_set_parameter_detection_mode() {
    let mut p = MultibandExpanderPlugin::new(2);
    p.initialize(48000).unwrap();

    p.set_parameter(ParameterId::from("detection_mode"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(p.detection_mode, "rms");

    p.set_parameter(ParameterId::from("detection_mode"), ParameterValue::Int(0))
        .unwrap();
    assert_eq!(p.detection_mode, "peak");
}

#[test]
fn test_set_parameter_band_threshold() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 2,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    p.set_parameter(
        ParameterId::from("band_0_threshold"),
        ParameterValue::Float(-25.0),
    )
    .unwrap();
    assert!((p.band_params[0].threshold_db.unwrap() - (-25.0)).abs() < 1e-6);
}

#[test]
fn test_set_parameter_band_bools() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 2,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    p.set_parameter(ParameterId::from("band_0_auto"), ParameterValue::Bool(true))
        .unwrap();
    assert!(p.band_params[0].auto_makeup);

    p.set_parameter(
        ParameterId::from("band_0_measured"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert!(p.band_params[0].measured_auto_makeup);

    p.set_parameter(
        ParameterId::from("band_0_active"),
        ParameterValue::Bool(false),
    )
    .unwrap();
    assert!(!p.band_params[0].active);

    p.set_parameter(ParameterId::from("band_0_solo"), ParameterValue::Bool(true))
        .unwrap();
    assert!(p.band_params[0].solo);

    p.set_parameter(
        ParameterId::from("band_0_bypass"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert!(p.band_params[0].bypass);
}

#[test]
fn test_set_parameter_auto_makeup_alias() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();

    p.set_parameter(ParameterId::from("auto_makeup"), ParameterValue::Bool(true))
        .unwrap();
    assert!(p.band_params[0].auto_makeup);
}

#[test]
fn test_set_parameter_measured_auto_makeup_alias() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();

    p.set_parameter(
        ParameterId::from("measured_auto_makeup"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert!(p.band_params[0].measured_auto_makeup);
}

#[test]
fn test_set_parameter_sidechain_hpf_alias() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();

    p.set_parameter(
        ParameterId::from("sidechain_hpf_hz"),
        ParameterValue::Float(120.0),
    )
    .unwrap();
    assert!((p.sidechain_hpf_hz - 120.0).abs() < 1e-6);
}

#[test]
fn test_set_parameter_processing_mode_time_domain_is_structural() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            processing_mode: "spectral".to_string(),
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    assert!(p.spectral.is_some());

    assert!(
        p.set_parameter(ParameterId::from("processing_mode"), ParameterValue::Int(0))
            .is_err()
    );
    assert!(p.spectral.is_some());
}

#[test]
fn test_set_parameter_num_bands_rejected_as_structural() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 2,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    assert!(
        p.set_parameter(ParameterId::from("num_bands"), ParameterValue::Int(4))
            .is_err()
    );
    assert_eq!(p.num_bands, 2);
}

#[test]
fn test_set_parameter_global_misc() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();

    p.set_parameter(ParameterId::from("range"), ParameterValue::Float(60.0))
        .unwrap();
    assert!((p.range_db - 60.0).abs() < 1e-6);

    p.set_parameter(ParameterId::from("knee"), ParameterValue::Float(12.0))
        .unwrap();
    assert!((p.knee_db - 12.0).abs() < 1e-6);

    p.set_parameter(ParameterId::from("hysteresis"), ParameterValue::Float(8.0))
        .unwrap();
    assert!((p.hysteresis_db - 8.0).abs() < 1e-6);

    p.set_parameter(ParameterId::from("hold"), ParameterValue::Float(20.0))
        .unwrap();
    assert!((p.hold_ms - 20.0).abs() < 1e-6);
}

#[test]
fn test_set_parameter_link_channels() {
    let mut p = MultibandExpanderPlugin::new(2);
    p.initialize(48000).unwrap();

    p.set_parameter(
        ParameterId::from("link_channels"),
        ParameterValue::Bool(false),
    )
    .unwrap();
    assert!(!p.link_channels);

    p.set_parameter(
        ParameterId::from("link_channels"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert!(p.link_channels);
}

#[test]
fn test_set_parameter_invalid_type() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();

    // processing_mode expects int
    let res = p.set_parameter(
        ParameterId::from("processing_mode"),
        ParameterValue::Float(1.0),
    );
    assert!(res.is_err());

    // auto_makeup expects bool
    let res = p.set_parameter(ParameterId::from("auto_makeup"), ParameterValue::Float(1.0));
    assert!(res.is_err());
}

#[test]
fn test_set_parameter_band_invalid_type() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();

    let res = p.set_parameter(
        ParameterId::from("band_0_threshold"),
        ParameterValue::Bool(true),
    );
    assert!(res.is_err());

    let res = p.set_parameter(ParameterId::from("band_0_auto"), ParameterValue::Float(1.0));
    assert!(res.is_err());
}

#[test]
fn test_set_parameter_processing_mode_boundary() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();

    assert!(
        p.set_parameter(ParameterId::from("processing_mode"), ParameterValue::Int(0))
            .is_err()
    );
    assert_eq!(p.processing_mode, "time_domain");

    assert!(
        p.set_parameter(ParameterId::from("processing_mode"), ParameterValue::Int(1))
            .is_err()
    );
    assert_eq!(p.processing_mode, "time_domain");
}

#[test]
fn test_rebuild_cached_parameters() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 3,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let params_before = p.parameters().len();
    p.rebuild_cached_parameters();
    let params_after = p.parameters().len();
    assert_eq!(params_before, params_after);
    assert!(params_after > 0);
}

// -------------------------------------------------------------------------
// Initialization tests
// -------------------------------------------------------------------------

#[test]
fn test_initialize_different_sample_rate() {
    let mut p = MultibandExpanderPlugin::new(2);
    p.initialize(96000).unwrap();
    assert_eq!(p.sample_rate, 96000);
}

#[test]
fn test_initialize_with_lookahead() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            lookahead_ms: 5.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    let expected = (5.0_f32 * 0.001 * 48000.0).round() as usize;
    assert_eq!(p.latency_samples(), expected);
}

#[test]
fn test_initialize_spectral_mode() {
    let mut p = MultibandExpanderPlugin::with_params(
        2,
        MultibandExpanderPluginParams {
            processing_mode: "spectral".to_string(),
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    assert!(p.spectral.is_some());
}

// -------------------------------------------------------------------------
// process_in_place time-domain tests (various branch coverage)
// -------------------------------------------------------------------------

#[test]
fn test_process_empty_buffer() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    let mut buf: Vec<f32> = vec![];
    let res = p.process_in_place(&mut buf, &ProcessContext::new(48000, 0));
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), 0);
}

#[test]
fn test_process_rejects_wrong_buffer_lengths_without_advancing_cache_counter() {
    let mut p = MultibandExpanderPlugin::new(2);
    p.initialize(48_000).unwrap();
    for len in [7, 9] {
        let mut buffer = vec![0.0; len];
        assert!(
            p.process_in_place(&mut buffer, &ProcessContext::new(48_000, 4))
                .is_err()
        );
        assert_eq!(p.cache_update_counter, 0);
    }
}

#[test]
fn test_cache_snapshot_updates_after_processing() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48_000).unwrap();
    let mut phase = 0.0_f32;
    let mut buffer: Vec<f32> = (0..4096)
        .map(|_| {
            let sample = phase.sin() * 0.5;
            phase += std::f32::consts::TAU * 1_000.0 / 48_000.0;
            sample
        })
        .collect();
    for _ in 0..10 {
        p.process_in_place(&mut buffer, &ProcessContext::new(48_000, 4096))
            .unwrap();
    }
    let data = p.get_data().unwrap();
    let data = data.downcast::<MultibandExpanderData>().unwrap();
    assert!(data.band_levels_db.iter().any(|level| *level > -100.0));
}

#[test]
fn single_band_identity_schema_and_broadband_path_are_real() {
    let params = MultibandExpanderPluginParams {
        num_bands: 1,
        sidechain_hpf_hz: Some(0.0),
        ratio: 1.0,
        ..Default::default()
    };
    let mut plugin = MultibandExpanderPlugin::try_from_params(2, params, 48_000).unwrap();
    assert_eq!(plugin.info().name, "Expander");
    assert_eq!(plugin.num_bands, 1);
    assert!(plugin.crossover_points.is_empty());
    let ids: Vec<_> = plugin.parameters().into_iter().map(|p| p.id).collect();
    assert!(!ids.iter().any(|id| id.as_str() == "num_bands"));
    assert!(!ids.iter().any(|id| id.as_str().starts_with("crossover_")));

    let frames = 4096;
    let mut signal = vec![0.0; frames * 2];
    for frame in 0..frames {
        let t = frame as f32 / 48_000.0;
        let sample = 0.2 * (std::f32::consts::TAU * 100.0 * t).sin()
            + 0.2 * (std::f32::consts::TAU * 6_000.0 * t).sin();
        signal[frame * 2] = sample;
        signal[frame * 2 + 1] = sample;
    }
    let expected = signal.clone();
    plugin
        .process_in_place(&mut signal, &ProcessContext::new(48_000, frames))
        .unwrap();
    let error = signal
        .iter()
        .zip(expected)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(error < 1e-5, "single-band unity transfer error: {error}");
}

#[test]
fn sidechain_hpf_rejects_low_frequency_trigger() {
    fn render(hpf: f32) -> f32 {
        let params = MultibandExpanderPluginParams {
            num_bands: 1,
            threshold_db: -30.0,
            ratio: 8.0,
            attack_ms: 0.1,
            release_ms: 10.0,
            knee_db: 0.0,
            range_db: 60.0,
            hysteresis_db: 0.0,
            hold_ms: 0.0,
            sidechain_hpf_hz: Some(hpf),
            ..Default::default()
        };
        let mut plugin = MultibandExpanderPlugin::try_from_params(1, params, 48_000).unwrap();
        let mut signal: Vec<f32> = (0..48_000)
            .map(|i| 0.1 * (std::f32::consts::TAU * 50.0 * i as f32 / 48_000.0).sin())
            .collect();
        plugin
            .process_in_place(&mut signal, &ProcessContext::new(48_000, 48_000))
            .unwrap();
        signal[24_000..].iter().map(|x| x * x).sum::<f32>()
    }
    let no_hpf = render(0.0);
    let strong_hpf = render(500.0);
    assert!(
        strong_hpf < no_hpf * 0.2,
        "HPF did not reject LF detector energy"
    );
}

#[test]
fn bypassed_band_obeys_common_lookahead_latency() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 1,
        lookahead_ms: 5.0,
        sidechain_hpf_hz: Some(0.0),
        ..Default::default()
    };
    params.bands = vec![BandExpanderParams {
        bypass: true,
        ..Default::default()
    }];
    let mut plugin = MultibandExpanderPlugin::try_from_params(1, params, 48_000).unwrap();
    let mut impulse = vec![0.0; 512];
    impulse[0] = 1.0;
    plugin
        .process_in_place(&mut impulse, &ProcessContext::new(48_000, 512))
        .unwrap();
    assert_eq!(impulse.iter().position(|x| x.abs() > 0.5), Some(240));
}

#[test]
fn crossover_automation_is_callback_partition_invariant() {
    let params = MultibandExpanderPluginParams {
        ratio: 1.0,
        sidechain_hpf_hz: Some(0.0),
        ..Default::default()
    };
    let mut whole = MultibandExpanderPlugin::try_from_params(2, params.clone(), 48_000).unwrap();
    let mut split = MultibandExpanderPlugin::try_from_params(2, params, 48_000).unwrap();
    for plugin in [&mut whole, &mut split] {
        plugin
            .set_parameter(
                ParameterId::from("crossover_freq_1"),
                ParameterValue::Float(500.0),
            )
            .unwrap();
    }
    let mut source = vec![0.0; 2048];
    for frame in 0..1024 {
        let sample = (std::f32::consts::TAU * 997.0 * frame as f32 / 48_000.0).sin();
        source[frame * 2] = sample;
        source[frame * 2 + 1] = -sample;
    }
    let mut a = source.clone();
    let mut b = source;
    whole
        .process_in_place(&mut a, &ProcessContext::new(48_000, 1024))
        .unwrap();
    for chunk in b.chunks_exact_mut(64 * 2) {
        split
            .process_in_place(chunk, &ProcessContext::new(48_000, 64))
            .unwrap();
    }
    let max_error = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0_f32, f32::max);
    assert!(max_error < 1e-5, "partition error: {max_error}");
}

#[test]
fn malformed_presets_are_rejected_fallibly() {
    let params = MultibandExpanderPluginParams {
        detection_mode: "average".into(),
        ..Default::default()
    };
    assert!(MultibandExpanderPlugin::try_from_params(2, params, 48_000).is_err());
    let params = MultibandExpanderPluginParams {
        processing_mode: "magic".into(),
        ..Default::default()
    };
    assert!(MultibandExpanderPlugin::try_from_params(2, params, 48_000).is_err());
    let params = MultibandExpanderPluginParams {
        crossover_frequencies: vec![2_000.0, 1_000.0, 8_000.0, 12_000.0],
        ..Default::default()
    };
    assert!(MultibandExpanderPlugin::try_from_params(2, params, 48_000).is_err());
    let params = MultibandExpanderPluginParams {
        threshold_db: f32::NAN,
        ..Default::default()
    };
    assert!(MultibandExpanderPlugin::try_from_params(2, params, 48_000).is_err());
}

#[test]
fn malformed_band_overrides_and_band_counts_are_rejected_fallibly() {
    for num_bands in [0, 6] {
        let params = MultibandExpanderPluginParams {
            num_bands,
            ..Default::default()
        };
        assert!(
            MultibandExpanderPlugin::try_from_params(2, params, 48_000).is_err(),
            "accepted invalid band count {num_bands}"
        );
    }

    let invalid_bands = [
        (
            "threshold",
            BandExpanderParams {
                threshold_db: Some(f32::NAN),
                ..Default::default()
            },
        ),
        (
            "ratio",
            BandExpanderParams {
                ratio: Some(0.5),
                ..Default::default()
            },
        ),
        (
            "attack",
            BandExpanderParams {
                attack_ms: Some(f32::INFINITY),
                ..Default::default()
            },
        ),
        (
            "release",
            BandExpanderParams {
                release_ms: Some(9.0),
                ..Default::default()
            },
        ),
        (
            "knee",
            BandExpanderParams {
                knee_db: Some(21.0),
                ..Default::default()
            },
        ),
        (
            "range",
            BandExpanderParams {
                range_db: Some(-1.0),
                ..Default::default()
            },
        ),
        (
            "hysteresis",
            BandExpanderParams {
                hysteresis_db: Some(13.0),
                ..Default::default()
            },
        ),
        (
            "hold",
            BandExpanderParams {
                hold_ms: Some(501.0),
                ..Default::default()
            },
        ),
    ];
    for (name, band) in invalid_bands {
        let params = MultibandExpanderPluginParams {
            num_bands: 1,
            bands: vec![band],
            ..Default::default()
        };
        assert!(
            MultibandExpanderPlugin::try_from_params(2, params, 48_000).is_err(),
            "accepted invalid per-band {name} override"
        );
    }
}

#[test]
fn spectral_mode_publishes_live_analyzer_snapshots() {
    let params = MultibandExpanderPluginParams {
        num_bands: 1,
        processing_mode: "spectral".into(),
        threshold_db: -10.0,
        ratio: 8.0,
        attack_ms: 0.1,
        release_ms: 10.0,
        knee_db: 0.0,
        range_db: 60.0,
        hysteresis_db: 0.0,
        hold_ms: 0.0,
        detection_mode: "Peak".into(),
        link_channels: true,
        lookahead_ms: 0.0,
        sidechain_hpf_hz: Some(0.0),
        ..Default::default()
    };
    let mut plugin = MultibandExpanderPlugin::try_from_params(1, params, 48_000).unwrap();
    let mut phase = 0.0_f32;
    for _ in 0..4 {
        let mut audio: Vec<f32> = (0..4096)
            .map(|_| {
                let sample = 0.001 * phase.sin();
                phase += std::f32::consts::TAU * 1_000.0 / 48_000.0;
                sample
            })
            .collect();
        plugin
            .process_in_place(&mut audio, &ProcessContext::new(48_000, 4096))
            .unwrap();
    }
    let data = plugin
        .get_data()
        .unwrap()
        .downcast::<MultibandExpanderData>()
        .unwrap();
    assert!(
        data.attenuation_db[0] > 1.0,
        "spectral attenuation snapshot remained frozen: {:?}",
        data.attenuation_db
    );
    assert!(data.band_levels_db[0] < -10.0);
}

#[test]
fn cache_cadence_depends_on_samples_not_callback_count() {
    fn counter_after(block: usize, total: usize) -> usize {
        let mut plugin = MultibandExpanderPlugin::new(1);
        plugin.initialize(48_000).unwrap();
        let mut remaining = total;
        while remaining > 0 {
            let frames = remaining.min(block);
            let mut audio = vec![0.0; frames];
            plugin
                .process_in_place(&mut audio, &ProcessContext::new(48_000, frames))
                .unwrap();
            remaining -= frames;
        }
        plugin.cache_update_counter
    }
    assert_eq!(counter_after(32, 1_280), counter_after(1_280, 1_280));
    assert_eq!(counter_after(64, 2_000), counter_after(1_000, 2_000));
}

#[test]
fn non_finite_input_is_sanitized_without_poisoning_state() {
    let mut plugin = MultibandExpanderPlugin::new(2);
    plugin.initialize(48_000).unwrap();
    let mut audio = vec![f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.25];
    plugin
        .process_in_place(&mut audio, &ProcessContext::new(48_000, 2))
        .unwrap();
    assert!(audio.iter().all(|sample| sample.is_finite()));
    let mut followup = vec![0.25; 256];
    plugin
        .process_in_place(&mut followup, &ProcessContext::new(48_000, 128))
        .unwrap();
    assert!(followup.iter().all(|sample| sample.is_finite()));
}

#[test]
fn spectral_mode_rejects_controls_without_feature_parity() {
    for mutate in [0, 1, 2, 3] {
        let mut params = MultibandExpanderPluginParams {
            processing_mode: "spectral".into(),
            detection_mode: "Peak".into(),
            link_channels: true,
            lookahead_ms: 0.0,
            sidechain_hpf_hz: Some(0.0),
            ..Default::default()
        };
        match mutate {
            0 => params.detection_mode = "RMS".into(),
            1 => params.link_channels = false,
            2 => params.lookahead_ms = 1.0,
            _ => params.sidechain_hpf_hz = Some(80.0),
        }
        assert!(MultibandExpanderPlugin::try_from_params(2, params, 48_000).is_err());
    }
}

#[test]
fn test_reset_clears_complete_observable_state() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48_000).unwrap();
    let mut buffer = vec![0.5; 4096];
    p.process_in_place(&mut buffer, &ProcessContext::new(48_000, 4096))
        .unwrap();
    p.reset();
    assert_eq!(p.cache_update_counter, 0);
    assert!(p.band_levels_db.iter().all(|level| *level == -120.0));
    assert!(p.attenuation_flattened.iter().all(|value| *value == 0.0));
}

#[test]
fn test_process_link_channels_rms() {
    let mut p = MultibandExpanderPlugin::with_params(
        2,
        MultibandExpanderPluginParams {
            num_bands: 2,
            link_channels: true,
            detection_mode: "rms".to_string(),
            threshold_db: -20.0,
            ratio: 4.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf * 2).map(|i| 0.1 * (i as f32 * 0.05).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_unlink_channels_peak() {
    let mut p = MultibandExpanderPlugin::with_params(
        2,
        MultibandExpanderPluginParams {
            num_bands: 2,
            link_channels: false,
            detection_mode: "peak".to_string(),
            threshold_db: -20.0,
            ratio: 4.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf * 2).map(|i| 0.1 * (i as f32 * 0.05).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_unlink_channels_rms() {
    let mut p = MultibandExpanderPlugin::with_params(
        2,
        MultibandExpanderPluginParams {
            num_bands: 2,
            link_channels: false,
            detection_mode: "rms".to_string(),
            threshold_db: -20.0,
            ratio: 4.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf * 2).map(|i| 0.1 * (i as f32 * 0.05).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_bypassed_band() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 3,
        threshold_db: -20.0,
        ratio: 4.0,
        mix: 1.0,
        ..Default::default()
    };
    params.bands = vec![
        BandExpanderParams {
            bypass: true,
            ..Default::default()
        },
        BandExpanderParams::default(),
        BandExpanderParams::default(),
    ];
    let mut p = MultibandExpanderPlugin::with_params(1, params);
    p.initialize(48000).unwrap();

    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf).map(|i| 0.2 * (i as f32 * 0.1).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_inactive_band() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 3,
        threshold_db: -20.0,
        ratio: 4.0,
        mix: 1.0,
        ..Default::default()
    };
    params.bands = vec![
        BandExpanderParams {
            active: false,
            ..Default::default()
        },
        BandExpanderParams::default(),
        BandExpanderParams::default(),
    ];
    let mut p = MultibandExpanderPlugin::with_params(1, params);
    p.initialize(48000).unwrap();

    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf).map(|i| 0.2 * (i as f32 * 0.1).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_solo_band() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 3,
        threshold_db: -20.0,
        ratio: 4.0,
        mix: 1.0,
        ..Default::default()
    };
    params.bands = vec![
        BandExpanderParams {
            solo: true,
            ..Default::default()
        },
        BandExpanderParams::default(),
        BandExpanderParams::default(),
    ];
    let mut p = MultibandExpanderPlugin::with_params(1, params);
    p.initialize(48000).unwrap();

    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf).map(|i| 0.2 * (i as f32 * 0.1).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_auto_makeup() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 1,
        threshold_db: -20.0,
        ratio: 4.0,
        range_db: 40.0,
        mix: 1.0,
        ..Default::default()
    };
    params.bands = vec![BandExpanderParams {
        auto_makeup: true,
        ..Default::default()
    }];
    let mut p = MultibandExpanderPlugin::with_params(1, params);
    p.initialize(48000).unwrap();

    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf).map(|i| 0.05 * (i as f32 * 0.1).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_time_domain_lookahead() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 2,
            lookahead_ms: 3.0,
            threshold_db: -20.0,
            ratio: 4.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf).map(|i| 0.1 * (i as f32 * 0.1).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_knee_expansion() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 1,
            threshold_db: -20.0,
            ratio: 4.0,
            knee_db: 6.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf).map(|i| 0.05 * (i as f32 * 0.1).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_hysteresis_and_hold() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 1,
            threshold_db: -20.0,
            ratio: 4.0,
            hysteresis_db: 4.0,
            hold_ms: 10.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf).map(|i| 0.05 * (i as f32 * 0.1).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_mix_dry_only() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 2,
            mix: 0.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let nf = 2048usize;
    let mut buf: Vec<f32> = (0..nf).map(|i| 0.2 * (i as f32 * 0.1).sin()).collect();
    let original = buf.clone();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();

    let rms_in = (original.iter().map(|s| s * s).sum::<f32>() / nf as f32).sqrt();
    let rms_out = (buf.iter().map(|s| s * s).sum::<f32>() / nf as f32).sqrt();
    let ratio = rms_out / rms_in;
    assert!(
        (0.5..1.5).contains(&ratio),
        "Dry only should pass through, ratio={ratio:.3}"
    );
}

#[test]
fn test_process_mix_wet_only() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 2,
            mix: 1.0,
            threshold_db: -20.0,
            ratio: 4.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let nf = 2048usize;
    let mut buf: Vec<f32> = (0..nf).map(|i| 0.05 * (i as f32 * 0.1).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_stereo_separate_channels() {
    let mut p = MultibandExpanderPlugin::with_params(
        2,
        MultibandExpanderPluginParams {
            num_bands: 2,
            link_channels: false,
            threshold_db: -20.0,
            ratio: 4.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let nf = 4800usize;
    let mut buf: Vec<f32> = (0..nf)
        .flat_map(|i| {
            let t = i as f32 * 0.05;
            [0.1 * t.sin(), 0.05 * t.cos()]
        })
        .collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

// -------------------------------------------------------------------------
// process_in_place spectral mode branch coverage
// -------------------------------------------------------------------------

#[test]
fn test_process_spectral_solo_band() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 3,
        processing_mode: "spectral".to_string(),
        threshold_db: -20.0,
        ratio: 4.0,
        mix: 1.0,
        ..Default::default()
    };
    params.bands = vec![
        BandExpanderParams {
            solo: true,
            ..Default::default()
        },
        BandExpanderParams::default(),
        BandExpanderParams::default(),
    ];
    let mut p = MultibandExpanderPlugin::with_params(2, params);
    p.initialize(48000).unwrap();

    let nf = 8192usize;
    let mut buf: Vec<f32> = (0..nf)
        .flat_map(|i| {
            let t = i as f32 * 0.05;
            [0.1 * t.sin(), 0.1 * t.cos()]
        })
        .collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_spectral_bypassed_band() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 3,
        processing_mode: "spectral".to_string(),
        threshold_db: -20.0,
        ratio: 4.0,
        mix: 1.0,
        ..Default::default()
    };
    params.bands = vec![
        BandExpanderParams {
            bypass: true,
            ..Default::default()
        },
        BandExpanderParams::default(),
        BandExpanderParams::default(),
    ];
    let mut p = MultibandExpanderPlugin::with_params(2, params);
    p.initialize(48000).unwrap();

    let nf = 8192usize;
    let mut buf: Vec<f32> = (0..nf)
        .flat_map(|i| {
            let t = i as f32 * 0.05;
            [0.1 * t.sin(), 0.1 * t.cos()]
        })
        .collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_spectral_inactive_band() {
    let mut params = MultibandExpanderPluginParams {
        num_bands: 3,
        processing_mode: "spectral".to_string(),
        threshold_db: -20.0,
        ratio: 4.0,
        mix: 1.0,
        ..Default::default()
    };
    params.bands = vec![
        BandExpanderParams {
            active: false,
            ..Default::default()
        },
        BandExpanderParams::default(),
        BandExpanderParams::default(),
    ];
    let mut p = MultibandExpanderPlugin::with_params(2, params);
    p.initialize(48000).unwrap();

    let nf = 8192usize;
    let mut buf: Vec<f32> = (0..nf)
        .flat_map(|i| {
            let t = i as f32 * 0.05;
            [0.1 * t.sin(), 0.1 * t.cos()]
        })
        .collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

// -------------------------------------------------------------------------
// Helper / latency tests
// -------------------------------------------------------------------------

#[test]
fn test_latency_samples_lookahead() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            lookahead_ms: 5.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    let expected = (5.0_f32 * 0.001 * 48000.0).round() as usize;
    assert_eq!(p.latency_samples(), expected);
}

#[test]
fn test_latency_samples_zero() {
    let mut p = MultibandExpanderPlugin::new(1);
    p.initialize(48000).unwrap();
    assert_eq!(p.latency_samples(), 0);
}

#[test]
fn test_build_crossovers() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 3,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    assert_eq!(p.crossover_points.len(), 2);

    p.set_param_value(0, 5.0); // num_bands = 5
    p.build_crossovers();
    assert_eq!(p.crossover_points.len(), 4);
}

#[test]
fn test_update_coefficients() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 2,
            attack_ms: 1.0,
            release_ms: 50.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    let old_attack = p.band_expanders[0].attack_coeff;

    p.attack_ms = 10.0;
    p.update_coefficients();

    assert_ne!(p.band_expanders[0].attack_coeff, old_attack);
    assert!(p.band_expanders[0].attack_coeff > 0.0);
    assert!(p.band_expanders[0].attack_coeff < 1.0);
}

#[test]
fn test_update_lookahead_delay() {
    let mut p = MultibandExpanderPlugin::with_params(
        1,
        MultibandExpanderPluginParams {
            num_bands: 2,
            lookahead_ms: 5.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    p.lookahead_ms = 10.0;
    p.update_lookahead_delay();
    // Just verify it doesn't panic
}

// -------------------------------------------------------------------------
// SpectralState helper tests
// -------------------------------------------------------------------------

#[test]
fn test_compute_bin_to_band() {
    use super::spectral_state::SpectralState;

    let bin_to_band =
        SpectralState::compute_bin_to_band(1024, 513, 48000, &[300.0, 3000.0, 8000.0, 12000.0], 5);

    assert_eq!(bin_to_band[0], 0); // DC bin -> lowest band
    assert_eq!(bin_to_band[10], 1); // ~469 Hz -> above 300 Hz crossover
}

#[test]
fn spectral_window_product_is_constant_overlap_add() {
    let state = SpectralState::new(256, 1, 48_000, &[1_000.0], 2);
    let overlap_count = state.fft_size / state.hop_size;
    let mut min_sum = f32::INFINITY;
    let mut max_sum = f32::NEG_INFINITY;

    for sample in 0..state.fft_size {
        let sum = (0..overlap_count)
            .map(|shift| {
                let idx = (sample + shift * state.hop_size) % state.fft_size;
                state.analysis_window[idx] * state.analysis_window[idx]
            })
            .sum::<f32>();
        min_sum = min_sum.min(sum);
        max_sum = max_sum.max(sum);
    }

    assert!(
        max_sum - min_sum < 1.0e-5,
        "analysis*synthesis window sum must be constant, range={min_sum}..{max_sum}"
    );
}

#[test]
fn test_spectral_state_reset() {
    use super::spectral_state::SpectralState;

    let mut ss = SpectralState::new(1024, 2, 48000, &[300.0, 3000.0], 3);

    ss.input_fill = 500;
    ss.bin_states[0][0].envelope_db = 10.0;
    ss.bin_states[0][0].gate_state = GateState::Closing;
    ss.output_accumulator_fill = 100;

    ss.reset();

    assert_eq!(ss.input_fill, 0);
    assert_eq!(ss.bin_states[0][0].envelope_db, 0.0);
    assert_eq!(ss.bin_states[0][0].gate_state, GateState::Open);
    assert_eq!(ss.output_accumulator_fill, 0);
}

// -------------------------------------------------------------------------
// MultibandExpanderData tests
// -------------------------------------------------------------------------

#[test]
fn test_multiband_expander_data() {
    use super::multiband_expander_data::MultibandExpanderData;

    let data = MultibandExpanderData::new(3, 2);
    assert_eq!(data.attenuation_db.len(), 6);
    assert_eq!(data.is_open.len(), 3);
    assert_eq!(data.band_levels_db.len(), 3);
    assert_eq!(data.crossover_frequencies.len(), 2);

    let default = MultibandExpanderData::default();
    assert!(default.attenuation_db.is_empty());

    let mut data = MultibandExpanderData::new(2, 1);
    data.update(&[1.0, 2.0], &[true, false], &[-10.0, -20.0], &[500.0]);
    assert_eq!(data.attenuation_db[0], 1.0);
    assert!(data.is_open[0]);
    assert_eq!(data.band_levels_db[1], -20.0);
    assert_eq!(data.crossover_frequencies[0], 500.0);
}

#[test]
fn test_calculate_expansion_attenuation_knee() {
    let th = -10.0f32;
    let ratio = 4.0f32;
    let knee = 6.0f32;
    let range = 20.0f32;

    // Above threshold + knee/2 -> no attenuation
    let att =
        MultibandExpanderPlugin::calculate_expansion_attenuation(-5.0, th, ratio, knee, range);
    assert_eq!(att, 0.0);

    // Well below threshold - knee/2 -> full attenuation (capped at range)
    let att =
        MultibandExpanderPlugin::calculate_expansion_attenuation(-40.0, th, ratio, knee, range);
    assert_eq!(att, range);

    // Inside knee -> soft attenuation
    let att =
        MultibandExpanderPlugin::calculate_expansion_attenuation(-10.0, th, ratio, knee, range);
    assert!(att >= 0.0);
    assert!(att < range);
}
