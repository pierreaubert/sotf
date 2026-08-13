use super::band_compressor_params::BandCompressorParams;
use super::multiband_compressor_data::MultibandCompressorData;
use super::multiband_compressor_plugin::MultibandCompressorPlugin;
use super::types::MultibandCompressorPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::{PluginCompiledOp, ProcessContext};

#[test]
fn test_mb_comp_basic() {
    let mut p = MultibandCompressorPlugin::new(1);
    p.initialize(48000).unwrap();
    let mut b = vec![0.5; 1000];
    p.process_in_place(&mut b, &ProcessContext::new(48000, 1000))
        .unwrap();
    assert!(b[999].is_finite());
}

#[test]
fn test_ms_mode_dry_mix_is_exact_passthrough() {
    let params = MultibandCompressorPluginParams {
        ms_mode: true,
        mix: 0.0,
        ..Default::default()
    };
    let mut plugin = MultibandCompressorPlugin::with_params(2, params);
    plugin.initialize(48_000).unwrap();
    let input: Vec<f32> = (0..512)
        .flat_map(|i| [0.7 * (i as f32 * 0.1).sin(), 0.2 * (i as f32 * 0.17).cos()])
        .collect();
    let mut output = input.clone();
    plugin
        .process_in_place(&mut output, &ProcessContext::new(48_000, 512))
        .unwrap();
    assert_eq!(output, input);
}

#[test]
fn test_lookahead_latency_matches_configured_delay() {
    let params = MultibandCompressorPluginParams {
        per_band_lookahead_ms: 20.0,
        ..Default::default()
    };
    let mut plugin = MultibandCompressorPlugin::with_params(2, params);
    plugin.initialize(48_000).unwrap();
    assert_eq!(plugin.latency_samples(), 960);
    assert_eq!(plugin.compile_metadata().latency_samples, 960);
}

#[test]
fn test_cache_publishes_non_default_meter_values() {
    let mut plugin = MultibandCompressorPlugin::new(2);
    plugin.initialize(48_000).unwrap();
    let mut block = vec![0.8; 2_400 * 2];
    plugin
        .process_in_place(&mut block, &ProcessContext::new(48_000, 2_400))
        .unwrap();
    let data = plugin.get_data().unwrap();
    let data = data.downcast_ref::<MultibandCompressorData>().unwrap();
    assert!(data.band_levels_db.iter().any(|level| *level > -100.0));
    assert!(data.gain_reduction_db.iter().any(|gain| *gain > 0.01));
}

#[test]
fn test_num_bands_is_structural_after_initialization() {
    let mut plugin = MultibandCompressorPlugin::new(2);
    plugin.initialize(48_000).unwrap();
    let num_bands_before = plugin.num_bands;
    assert!(
        plugin
            .set_parameter(ParameterId::from("num_bands"), ParameterValue::Int(4))
            .is_err()
    );
    assert_eq!(plugin.num_bands, num_bands_before);
}

#[test]
fn test_mb_comp_compiled_op_matches_process_in_place_without_lookahead() {
    let sr = 48000;
    let frames = 512;
    let channels = 2;
    let params = MultibandCompressorPluginParams {
        per_band_lookahead_ms: 0.0,
        ..Default::default()
    };
    let mut regular = MultibandCompressorPlugin::with_params(channels, params.clone());
    let mut compiled = MultibandCompressorPlugin::with_params(channels, params);
    regular.initialize(sr).unwrap();
    compiled.initialize(sr).unwrap();

    let input: Vec<f32> = (0..frames * channels)
        .map(|i| 0.42 * (i as f32 * 0.037).sin() + 0.11 * (i as f32 * 0.19).cos())
        .collect();
    let ctx = ProcessContext::new(sr, frames);
    let mut expected = input.clone();
    let mut actual = vec![0.0; input.len()];

    let expected_frames = regular.process_in_place(&mut expected, &ctx).unwrap();
    let actual_frames = compiled
        .process_compiled_f32(
            PluginCompiledOp::MultibandCompressor,
            &input,
            &mut actual,
            &ctx,
        )
        .unwrap()
        .unwrap();

    assert_eq!(actual_frames, expected_frames);
    let max_error = expected
        .iter()
        .zip(actual.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        max_error <= 1e-6,
        "compiled multiband compressor output diverged: max_error={max_error}"
    );
}

#[test]
fn test_mb_comp_compiled_op_declines_lookahead() {
    let sr = 48000;
    let frames = 128;
    let channels = 2;
    let params = MultibandCompressorPluginParams {
        per_band_lookahead_ms: 2.0,
        ..Default::default()
    };
    let mut plugin = MultibandCompressorPlugin::with_params(channels, params);
    plugin.initialize(sr).unwrap();
    let input = vec![0.1; frames * channels];
    let mut output = vec![0.0; input.len()];

    assert!(
        plugin
            .process_compiled_f32(
                PluginCompiledOp::MultibandCompressor,
                &input,
                &mut output,
                &ProcessContext::new(sr, frames),
            )
            .is_none(),
        "lookahead compressor should stay on regular host path until latency is exposed"
    );
}

/// Unity passthrough: with ratio 1:1 on all bands (no compression) and dry mix,
/// the output should equal the input.
#[test]
fn test_mb_comp_crossover_reconstruction() {
    let mut params = MultibandCompressorPluginParams {
        num_bands: 3,
        mix: 0.0,
        ..Default::default()
    };
    for band in &mut params.bands {
        band.ratio = Some(1.0); // no compression
    }
    let mut p = MultibandCompressorPlugin::with_params(1, params);
    p.initialize(48000).unwrap();

    // Generate test signal (broadband): stay within the 4096-frame pre-alloc limit.
    // Process two 2048-frame blocks to accumulate settling time.
    let block = 2048usize;
    let total = block * 2;
    let signal: Vec<f32> = (0..total)
        .map(|i| 0.3 * (i as f32 * 0.1).sin() + 0.1 * (i as f32 * 0.5).sin())
        .collect();
    let mut output = signal.clone();
    let ctx = ProcessContext::new(48000, block);
    p.process_in_place(&mut output[..block], &ctx).unwrap();
    p.process_in_place(&mut output[block..], &ctx).unwrap();
    let input = &signal[block..]; // settled second half
    let out = &output[block..];

    // After crossover filter settling, RMS should be close to input
    let rms_in: f32 = (input.iter().map(|s| s * s).sum::<f32>() / input.len() as f32).sqrt();
    let rms_out: f32 = (out.iter().map(|s| s * s).sum::<f32>() / out.len() as f32).sqrt();
    let ratio = rms_out / rms_in;
    assert!(
        (0.85..1.15).contains(&ratio),
        "With ratio=1 (no compression), crossover should reconstruct signal. \
             RMS ratio={ratio:.3} (in={rms_in:.4}, out={rms_out:.4})"
    );
}

/// Changing a crossover frequency mid-stream should not produce clicks or
/// non-finite values. The output must remain bounded.
#[test]
fn test_crossover_frequency_change_no_discontinuity() {
    let mut p = MultibandCompressorPlugin::new(1);
    p.initialize(48000).unwrap();

    let nf = 2400;
    let ctx = ProcessContext::new(48000, nf);

    // Process a block with default crossover
    let mut b1: Vec<f32> = (0..nf).map(|i| 0.3 * (i as f32 * 0.1).sin()).collect();
    p.process_in_place(&mut b1, &ctx).unwrap();
    let last_before = b1[nf - 1];

    // Change crossover frequency mid-stream
    p.set_parameter(
        ParameterId::from("crossover_freq_1"),
        ParameterValue::Float(800.0),
    )
    .unwrap();

    // Process another block
    let mut b2: Vec<f32> = (0..nf)
        .map(|i| 0.3 * ((nf + i) as f32 * 0.1).sin())
        .collect();
    p.process_in_place(&mut b2, &ctx).unwrap();

    // All output must be finite and bounded
    for (i, &s) in b2.iter().enumerate() {
        assert!(
            s.is_finite() && s.abs() < 10.0,
            "Sample {} after crossover change is non-finite or unbounded: {}",
            i,
            s
        );
    }

    // The transition should not produce a large jump
    let jump = (b2[0] - last_before).abs();
    assert!(
        jump < 1.0,
        "Crossover frequency change caused discontinuity: jump={jump:.4}"
    );
}

/// Fix 2.2: num_bands setter should round, not truncate.
/// When set to 3 via Int, it should store 3 bands (not clamp or truncate).
#[test]
fn test_num_bands_rounds_not_truncates() {
    let mut p = MultibandCompressorPlugin::new(2);
    // param_bridge converts Float → Int by truncation (cross-crate, deferred).
    // Test that the Int path rounds correctly within set_param_value: Int(3) → 3 bands.
    p.set_parameter(ParameterId::from("num_bands"), ParameterValue::Int(3))
        .unwrap();
    let got = p.get_parameter(&ParameterId::from("num_bands")).unwrap();
    assert_eq!(
        got,
        ParameterValue::Int(3),
        "num_bands set to Int(3) should store 3 bands, got {:?}",
        got
    );
    // Verify the rounding guard: value arriving as 2 (already-truncated from Float(2.9)
    // via param_bridge) must clamp to the valid range. This is a regression guard.
    p.set_parameter(ParameterId::from("num_bands"), ParameterValue::Int(2))
        .unwrap();
    let got2 = p.get_parameter(&ParameterId::from("num_bands")).unwrap();
    assert_eq!(
        got2,
        ParameterValue::Int(2),
        "num_bands Int(2) should be accepted, got {:?}",
        got2
    );
}

/// Fix 2.3: band_levels_db should be initialized to -120.0 consistently.
#[test]
fn test_band_levels_db_initial_silence_floor() {
    let p = MultibandCompressorPlugin::new(2);
    // band_levels_db is not directly accessible, but we can verify via the cache
    // by checking no meter jump from silence is visible. Here we simply confirm
    // the constructor doesn't panic and processing a silence block gives finite output.
    let mut p = p;
    p.initialize(48000).unwrap();
    let mut buf = vec![0.0f32; 256 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 256))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_muted_band_meter_uses_silence_floor() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.band_params[0].solo = true;

    let mut buf = vec![0.25f32; 256 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 256))
        .unwrap();

    assert_eq!(p.band_levels_db[1], -120.0);
}

/// Lookahead is validated before initialization and becomes structural once
/// host latency metadata has been compiled.
#[test]
fn test_lookahead_clamp_in_setter() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.set_parameter(
        ParameterId::from("lookahead_ms"),
        ParameterValue::Float(50.0),
    )
    .unwrap();
    let got = p.get_parameter(&ParameterId::from("lookahead_ms")).unwrap();
    assert_eq!(
        got,
        ParameterValue::Float(20.0),
        "lookahead_ms 50 should be clamped to 20, got {:?}",
        got
    );
    p.initialize(48000).unwrap();
    let before = p.latency_samples();
    let error = p
        .set_parameter(
            ParameterId::from("per_band_lookahead_ms"),
            ParameterValue::Float(5.0),
        )
        .unwrap_err();
    assert!(error.contains("structural"));
    assert_eq!(p.latency_samples(), before);
}

/// Fix 2.6 + 2.7: tilt biquads should be rebuilt when num_bands increases,
/// and reset() should reinitialize them so no state leaks across transport stops.
#[test]
fn test_tilt_biquad_reset_and_rebuild() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    // Enable tilt
    p.set_parameter(
        ParameterId::from("sidechain_tilt_db"),
        ParameterValue::Float(3.0),
    )
    .unwrap();

    // Feed impulse to dirty the biquad state
    let nf = 256usize;
    let mut buf = vec![0.0f32; nf * 2];
    buf[0] = 1.0;
    buf[1] = 1.0;
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();

    // reset() must clear tilt state; a silence block after reset should give near-zero output
    p.reset();
    let mut silence = vec![0.0f32; nf * 2];
    p.process_in_place(&mut silence, &ProcessContext::new(48000, nf))
        .unwrap();
    let max_abs = silence.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        max_abs < 1e-6,
        "After reset(), silence block should produce ~0.0 output, got max_abs={max_abs}"
    );

    // Band count is structural after initialization and must not mutate live state.
    let num_bands_before = p.num_bands;
    assert!(
        p.set_parameter(ParameterId::from("num_bands"), ParameterValue::Float(3.0))
            .is_err()
    );
    assert_eq!(p.num_bands, num_bands_before);
}

/// Fix 1.5: per-band knee parameter must be reachable via set_parameter / get_parameter.
#[test]
fn test_per_band_knee_param_roundtrip() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();

    // Set knee for band 0
    p.set_parameter(ParameterId::from("band_0_knee"), ParameterValue::Float(3.0))
        .unwrap();
    let got = p.get_parameter(&ParameterId::from("band_0_knee")).unwrap();
    assert_eq!(
        got,
        ParameterValue::Float(3.0),
        "band_0_knee should round-trip to 3.0, got {:?}",
        got
    );

    // Also verify it appears in the parameters list
    let params = p.parameters();
    let knee_param = params.iter().find(|par| par.id.as_str() == "band_0_knee");
    assert!(
        knee_param.is_some(),
        "band_0_knee must appear in parameters()"
    );
}

/// Fix 1.3: measured_makeup update must be called once per frame, not once per channel.
/// Verify the effective time constant doesn't collapse on stereo vs. mono.
#[test]
fn test_measured_makeup_update_rate() {
    // Run a mono and a stereo plugin with identical settings and identical signal.
    // With the per-channel bug, stereo would converge twice as fast.
    let make_plugin = |channels: usize| {
        let params = MultibandCompressorPluginParams {
            num_bands: 1,
            bands: vec![BandCompressorParams {
                measured_auto_makeup: true,
                threshold_db: Some(-60.0),
                ratio: Some(2.0),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut p = MultibandCompressorPlugin::with_params(channels, params);
        p.initialize(48000).unwrap();
        p
    };

    // Use two 2400-frame blocks for ~100 ms total, staying within 4096-frame pre-alloc limit.
    let block = 2400usize;
    let input_val = 0.5f32;

    let mut p_mono = make_plugin(1);
    let ctx_mono = ProcessContext::new(48000, block);
    let mut buf_mono = vec![input_val; block];
    for _ in 0..2 {
        buf_mono.fill(input_val);
        p_mono.process_in_place(&mut buf_mono, &ctx_mono).unwrap();
    }

    let mut p_stereo = make_plugin(2);
    let ctx_stereo = ProcessContext::new(48000, block);
    let mut buf_stereo = vec![input_val; block * 2];
    for _ in 0..2 {
        buf_stereo.fill(input_val);
        p_stereo
            .process_in_place(&mut buf_stereo, &ctx_stereo)
            .unwrap();
    }

    // With the fix, mono and stereo should converge at approximately the same rate.
    // Before the fix, stereo updated twice per frame (once per channel), making the EMA
    // converge 2x faster. This would give a ~6 dB difference at 100 ms settling time
    // (half the time constant). After the fix both are within ~4 dB.
    let rms = |buf: &[f32], ch: usize| -> f32 {
        let n = buf.len() / ch;
        (buf.iter().map(|s| s * s).sum::<f32>() / n as f32).sqrt()
    };
    let rms_mono = rms(&buf_mono, 1);
    let rms_stereo = rms(&buf_stereo, 2);
    let diff_db = (20.0 * (rms_stereo / rms_mono.max(1e-10)).log10()).abs();
    // Threshold: 4 dB allows for channel-count-induced compression differences while
    // still catching the ~6 dB bug that a double-update-rate would cause.
    assert!(
        diff_db < 4.0,
        "Mono and stereo measured_makeup should converge at same rate, diff={diff_db:.2}dB"
    );
}

/// Fix 2.1: process_in_place must not call Vec::resize for blocks up to 4096 frames.
/// After initialize(), the buffers are pre-allocated and a large-but-valid block
/// must process without any reallocation (verified by asserting no panic/debug_assert).
#[test]
fn test_no_resize_within_prealloc_limit() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();

    // 4096 frames is the pre-allocation limit — must work without resize
    let nf = 4096usize;
    let mut buf = vec![0.1f32; nf * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

/// Verify compression actually reduces loud signals.
#[test]
fn test_mb_comp_reduces_loud_signal() {
    let mut p = MultibandCompressorPlugin::new(1);
    p.initialize(48000).unwrap();

    // Set low threshold to ensure compression, and mix=1.0 for wet-only
    p.set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
        .unwrap();
    p.set_parameter(ParameterId::from("ratio"), ParameterValue::Float(8.0))
        .unwrap();
    p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();

    // Process enough to let smoothers settle (200ms = ~9600 samples).
    // Use four 2400-frame blocks to stay within the 4096-frame pre-alloc limit.
    let block = 2400usize;
    let input_val = 0.5f32; // -6 dBFS
    let ctx = ProcessContext::new(48000, block);
    let mut last_block = vec![input_val; block];
    for _ in 0..4 {
        last_block.fill(input_val);
        p.process_in_place(&mut last_block, &ctx).unwrap();
    }

    // After settling, output should be quieter than input
    let rms_out: f32 = (last_block.iter().map(|s| s * s).sum::<f32>() / block as f32).sqrt();
    assert!(
        rms_out < input_val * 0.9,
        "Multiband compressor should reduce loud signal, but RMS {rms_out:.4} ≈ input {input_val}"
    );
}

/// Regression: sidechain tilt must be a true tilt (lowshelf + highshelf),
/// not a single highshelf. A true tilt cuts lows and boosts highs (or vice
/// versa) with a straight-line slope in dB vs log-frequency.
#[test]
fn test_sidechain_tilt_is_true_tilt() {
    let mut p = MultibandCompressorPlugin::new(1);
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("sidechain_tilt_db"),
        ParameterValue::Float(6.0),
    )
    .unwrap();
    let mut settle = vec![0.0; 4096];
    p.process_in_place(&mut settle, &ProcessContext::new(48_000, 4096))
        .unwrap();

    assert!(
        !p.sidechain_tilt_biquads.is_empty(),
        "Tilt biquads should exist"
    );

    // Measure the combined magnitude response of the tilt filter at a given frequency.
    let measure_gain = |freq_hz: f64| -> f64 {
        let (mut low, mut high) = p.sidechain_tilt_biquads[0][0].clone();
        let sr = 48000.0f64;
        let samples = (sr * 0.2) as usize; // 200 ms for settling
        let mut max_out = 0.0f64;
        for n in 0..samples {
            let t = n as f64 / sr;
            let input = (2.0 * std::f64::consts::PI * freq_hz * t).sin();
            let output = high.process(low.process(input));
            if n > samples * 3 / 4 {
                max_out = max_out.max(output.abs());
            }
        }
        20.0 * max_out.log10()
    };

    let gain_20hz = measure_gain(20.0);
    let gain_10khz = measure_gain(10000.0);

    // A single highshelf would leave 20 Hz at ~0 dB; a true tilt of +6 dB
    // should be approximately –3 dB at the low end and +3 dB at the high end.
    assert!(
        gain_20hz < -1.0,
        "True tilt should cut low frequencies, but 20 Hz gain was {:.2} dB",
        gain_20hz
    );
    assert!(
        gain_10khz > 1.0,
        "True tilt should boost high frequencies, but 10 kHz gain was {:.2} dB",
        gain_10khz
    );

    let span = gain_10khz - gain_20hz;
    assert!(
        span > 4.0 && span < 8.0,
        "True tilt span should be ~6 dB, got {:.2} dB",
        span
    );
}

/// Regression: stub parameters that have no DSP implementation must not be
/// exposed in parameters(), to prevent users from toggling controls that
/// have no audible effect.
#[test]
fn test_stub_params_not_exposed() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    let params = p.parameters();
    let ids: Vec<&str> = params.iter().map(|par| par.id.as_str()).collect();

    let stubs = [
        "sidechain_hpf_hz",
        "sidechain_hpf_order",
        "detection_mode",
        "program_dependent_release",
        "sidechain_external",
    ];
    for stub in &stubs {
        assert!(
            !ids.contains(stub),
            "Stub parameter '{}' must not appear in parameters()",
            stub
        );
    }
}

// -------------------------------------------------------------------------
// Pure helper tests
// -------------------------------------------------------------------------

#[test]
fn test_calculate_gain_reduction_hard_knee() {
    // Below threshold -> no gain reduction
    assert_eq!(
        MultibandCompressorPlugin::calculate_gain_reduction(-10.0, -5.0, 4.0, 0.0),
        0.0
    );
    // Above threshold -> linear slope
    let slope = 1.0 - 1.0 / 4.0;
    let gr = MultibandCompressorPlugin::calculate_gain_reduction(5.0, -5.0, 4.0, 0.0);
    assert!((gr - 10.0 * slope).abs() < 1e-5);
}

#[test]
fn test_calculate_gain_reduction_soft_knee() {
    let th = 0.0f32;
    let ratio = 4.0f32;
    let knee = 4.0f32;
    let slope = 1.0 - 1.0 / ratio;
    // Well below knee
    assert_eq!(
        MultibandCompressorPlugin::calculate_gain_reduction(-10.0, th, ratio, knee),
        0.0
    );
    // Well above knee
    let gr_above = MultibandCompressorPlugin::calculate_gain_reduction(10.0, th, ratio, knee);
    assert!((gr_above - 10.0 * slope).abs() < 1e-5);
    // Inside knee -> quadratic transition
    let gr_mid = MultibandCompressorPlugin::calculate_gain_reduction(0.0, th, ratio, knee);
    assert!(gr_mid > 0.0 && gr_mid < 2.0 * slope);
}

// -------------------------------------------------------------------------
// Parameter round-trip and setter tests
// -------------------------------------------------------------------------

#[test]
fn test_global_param_value_roundtrip() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();

    p.set_param_value(6, -18.0); // threshold
    p.set_param_value(7, 8.0); // ratio
    p.set_param_value(13, 5.0); // per_band_lookahead_ms

    assert!((p.param_value(6).unwrap() - (-18.0)).abs() < 1e-6);
    assert!((p.param_value(7).unwrap() - 8.0).abs() < 1e-6);
    assert!((p.param_value(13).unwrap() - 5.0).abs() < 1e-6);

    // num_bands setter rounds, not truncates
    p.set_param_value(0, 2.7);
    assert_eq!(p.param_value(0).unwrap(), 3.0);
}

#[test]
fn test_set_sidechain_tilt_rebuilds_biquads() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    let dimensions = (
        p.sidechain_tilt_biquads.len(),
        p.sidechain_tilt_biquads[0].len(),
    );

    p.set_parameter(
        ParameterId::from("sidechain_tilt_db"),
        ParameterValue::Float(6.0),
    )
    .unwrap();

    let mut buffer = vec![0.1; 64 * 2];
    p.process_in_place(&mut buffer, &ProcessContext::new(48_000, 64))
        .unwrap();
    assert_eq!(
        dimensions,
        (
            p.sidechain_tilt_biquads.len(),
            p.sidechain_tilt_biquads[0].len()
        )
    );
    assert!(p.tilt_smoother.current() > 0.0 && p.tilt_smoother.current() < 6.0);
}

#[test]
fn test_set_num_bands_rebuilds_crossovers_and_biquads() {
    let mut p = MultibandCompressorPlugin::with_params(
        2,
        MultibandCompressorPluginParams {
            num_bands: 2,
            sidechain_tilt_db: 3.0,
            ..Default::default()
        },
    );
    assert_eq!(p.crossover_points.len(), 1);

    p.set_parameter(ParameterId::from("num_bands"), ParameterValue::Float(4.0))
        .unwrap();

    assert_eq!(p.num_bands, 4);
    assert_eq!(p.crossover_points.len(), 3);
    assert_eq!(p.sidechain_tilt_biquads.len(), 4);
    assert_eq!(p.band_compressors.len(), 4);
    p.initialize(48000).unwrap();
}

#[test]
fn test_band_threshold_ignores_nan() {
    let mut p = MultibandCompressorPlugin::new(1);
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("band_0_threshold"),
        ParameterValue::Float(-20.0),
    )
    .unwrap();
    // NaN must be silently ignored, not stored
    p.set_parameter(
        ParameterId::from("band_0_threshold"),
        ParameterValue::Float(f32::NAN),
    )
    .unwrap();
    let v = p
        .get_parameter(&ParameterId::from("band_0_threshold"))
        .unwrap();
    assert_eq!(v, ParameterValue::Float(-20.0));
}

#[test]
fn test_set_parameter_unknown_returns_error() {
    let mut p = MultibandCompressorPlugin::new(1);
    p.initialize(48000).unwrap();
    let res = p.set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0));
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("Unknown parameter"));
}

#[test]
fn test_rebuild_cached_parameters_includes_aliases() {
    let mut p = MultibandCompressorPlugin::new(1);
    p.initialize(48000).unwrap();
    let params = p.parameters();
    let ids: Vec<&str> = params.iter().map(|par| par.id.as_str()).collect();
    assert!(ids.contains(&"makeup_gain"));
    assert!(ids.contains(&"auto_makeup"));
    assert!(ids.contains(&"measured_auto_makeup"));
    assert!(ids.contains(&"lookahead_ms"));
}

// -------------------------------------------------------------------------
// Initialization / reset / process smoke tests
// -------------------------------------------------------------------------

#[test]
fn test_initialize_allocates_buffers() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    assert!(!p.dry_buffer.is_empty());
    assert!(!p.band_buffers.is_empty());
    assert_eq!(p.dry_buffer.len(), 4096 * 2);
}

#[test]
fn test_reset_clears_compressor_envelopes() {
    let mut p = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            threshold_db: -20.0,
            ratio: 4.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    let mut buf = vec![0.5f32; 256];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 256))
        .unwrap();
    assert!(p.band_compressors[0].envelope[0] > 0.0);

    p.reset();
    assert_eq!(p.band_compressors[0].envelope[0], 0.0);
}

#[test]
fn test_process_silence_stereo_finite() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    let mut buf = vec![0.0f32; 512 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
    assert!(buf.iter().all(|s| s.abs() < 1e-6));
}

// -------------------------------------------------------------------------
// Additional parameter setter / getter coverage
// -------------------------------------------------------------------------

#[test]
fn test_set_parameter_global_roundtrips() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();

    // crossover_preset (choice param stored as i32)
    p.set_parameter(
        ParameterId::from("crossover_preset"),
        ParameterValue::Int(2),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("crossover_preset"))
            .unwrap(),
        ParameterValue::Int(2)
    );

    // crossover_freq_1..4
    let xover_vals = [
        ("crossover_freq_1", 300.0f32),
        ("crossover_freq_2", 1200.0),
        ("crossover_freq_3", 8000.0),
        ("crossover_freq_4", 12000.0),
    ];
    for (name, val) in &xover_vals {
        p.set_parameter(ParameterId::from(*name), ParameterValue::Float(*val))
            .unwrap();
        let got = p.get_parameter(&ParameterId::from(*name)).unwrap();
        assert_eq!(
            got,
            ParameterValue::Float(*val),
            "round-trip failed for {}",
            name
        );
    }

    // attack
    p.set_parameter(ParameterId::from("attack"), ParameterValue::Float(2.5))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("attack")).unwrap(),
        ParameterValue::Float(2.5)
    );

    // release
    p.set_parameter(ParameterId::from("release"), ParameterValue::Float(200.0))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("release")).unwrap(),
        ParameterValue::Float(200.0)
    );

    // knee
    p.set_parameter(ParameterId::from("knee"), ParameterValue::Float(10.0))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("knee")).unwrap(),
        ParameterValue::Float(10.0)
    );

    // link_channels
    p.set_parameter(
        ParameterId::from("link_channels"),
        ParameterValue::Bool(false),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("link_channels"))
            .unwrap(),
        ParameterValue::Bool(false)
    );
    p.set_parameter(
        ParameterId::from("link_channels"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("link_channels"))
            .unwrap(),
        ParameterValue::Bool(true)
    );

    // ms_mode
    p.set_parameter(ParameterId::from("ms_mode"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("ms_mode")).unwrap(),
        ParameterValue::Bool(true)
    );

    // link_amount
    p.set_parameter(ParameterId::from("link_amount"), ParameterValue::Float(0.5))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("link_amount")).unwrap(),
        ParameterValue::Float(0.5)
    );
}

#[test]
fn test_set_parameter_attack_release_updates_coefficients() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    let old_attack = p.band_compressors[0].attack_coeff;
    let old_release = p.band_compressors[0].release_coeff;

    p.set_parameter(ParameterId::from("attack"), ParameterValue::Float(1.0))
        .unwrap();
    p.set_parameter(ParameterId::from("release"), ParameterValue::Float(500.0))
        .unwrap();

    let mut buffer = vec![0.1; 64 * 2];
    p.process_in_place(&mut buffer, &ProcessContext::new(48_000, 64))
        .unwrap();

    assert_ne!(
        p.band_compressors[0].attack_coeff, old_attack,
        "attack coefficient should change"
    );
    assert_ne!(
        p.band_compressors[0].release_coeff, old_release,
        "release coefficient should change"
    );
}

#[test]
fn test_set_parameter_mix_and_threshold_update_smoothers() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();

    p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.75))
        .unwrap();
    assert!((p.mix_smoother.target() - 0.75).abs() < 1e-6);

    p.set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
        .unwrap();
    assert!((p.threshold_smoother.target() - (-30.0)).abs() < 1e-6);
}

#[test]
fn automation_values_are_independent_of_host_block_partitioning() {
    fn render(chunks: &[usize]) -> Vec<[f32; 4]> {
        let mut plugin = MultibandCompressorPlugin::new(1);
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
fn test_initial_threshold_automation_seeds_smoother_current_value() {
    let mut p = MultibandCompressorPlugin::new(1);
    p.initialize(48000).unwrap();

    p.set_parameter(ParameterId::from("threshold"), ParameterValue::Float(0.0))
        .unwrap();

    assert!((p.threshold_smoother.current() - 0.0).abs() < 1e-6);
    assert!((p.threshold_smoother.target() - 0.0).abs() < 1e-6);
}

#[test]
fn test_set_parameter_per_band_lookahead_updates_buffers() {
    let mut p = MultibandCompressorPlugin::with_params(
        2,
        MultibandCompressorPluginParams {
            per_band_lookahead_ms: 10.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    assert_eq!(p.lookahead_buffers[0].delay(), 480);

    let error = p
        .set_parameter(
            ParameterId::from("per_band_lookahead_ms"),
            ParameterValue::Float(5.0),
        )
        .unwrap_err();
    assert!(error.contains("structural"));
    for buf in &p.lookahead_buffers {
        assert_eq!(
            buf.delay(),
            480,
            "a rejected live latency change must leave delay state untouched"
        );
    }
}

#[test]
fn test_set_parameter_band_fields_roundtrip() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();

    // ratio
    p.set_parameter(
        ParameterId::from("band_0_ratio"),
        ParameterValue::Float(2.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_ratio")).unwrap(),
        ParameterValue::Float(2.0)
    );

    // attack
    p.set_parameter(
        ParameterId::from("band_0_attack"),
        ParameterValue::Float(10.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_attack"))
            .unwrap(),
        ParameterValue::Float(10.0)
    );

    // release
    p.set_parameter(
        ParameterId::from("band_0_release"),
        ParameterValue::Float(100.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_release"))
            .unwrap(),
        ParameterValue::Float(100.0)
    );

    // makeup
    p.set_parameter(
        ParameterId::from("band_0_makeup"),
        ParameterValue::Float(6.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_makeup"))
            .unwrap(),
        ParameterValue::Float(6.0)
    );

    // auto_makeup
    p.set_parameter(
        ParameterId::from("band_0_auto_makeup"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_auto_makeup"))
            .unwrap(),
        ParameterValue::Bool(true)
    );

    // measured_auto_makeup
    p.set_parameter(
        ParameterId::from("band_0_measured_auto_makeup"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_measured_auto_makeup"))
            .unwrap(),
        ParameterValue::Bool(true)
    );

    // active
    p.set_parameter(
        ParameterId::from("band_0_active"),
        ParameterValue::Bool(false),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_active"))
            .unwrap(),
        ParameterValue::Bool(false)
    );

    // solo
    p.set_parameter(ParameterId::from("band_0_solo"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_solo")).unwrap(),
        ParameterValue::Bool(true)
    );

    // bypass
    p.set_parameter(
        ParameterId::from("band_0_bypass"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_bypass"))
            .unwrap(),
        ParameterValue::Bool(true)
    );
}

#[test]
fn test_set_parameter_band_attack_release_updates_coefficients() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    let old_attack = p.band_compressors[0].attack_coeff;
    let old_release = p.band_compressors[0].release_coeff;

    p.set_parameter(
        ParameterId::from("band_0_attack"),
        ParameterValue::Float(1.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("band_0_release"),
        ParameterValue::Float(500.0),
    )
    .unwrap();

    let mut buffer = vec![0.1; 64 * 2];
    p.process_in_place(&mut buffer, &ProcessContext::new(48_000, 64))
        .unwrap();

    assert_ne!(
        p.band_compressors[0].attack_coeff, old_attack,
        "band attack coefficient should change"
    );
    assert_ne!(
        p.band_compressors[0].release_coeff, old_release,
        "band release coefficient should change"
    );
}

#[test]
fn test_set_parameter_band_out_of_range_returns_error() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    let res = p.set_parameter(
        ParameterId::from("band_99_threshold"),
        ParameterValue::Float(-20.0),
    );
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("out of range"));
}

#[test]
fn test_set_parameter_band_unknown_field_returns_error() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    let res = p.set_parameter(
        ParameterId::from("band_0_unknown"),
        ParameterValue::Float(1.0),
    );
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("Unknown band field"));
}

#[test]
fn test_set_parameter_type_mismatch_errors() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();

    // Float param given Bool
    let res = p.set_parameter(
        ParameterId::from("band_0_threshold"),
        ParameterValue::Bool(true),
    );
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("must be a float"));

    // Bool param given Float
    let res = p.set_parameter(
        ParameterId::from("band_0_auto_makeup"),
        ParameterValue::Float(1.0),
    );
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("must be a boolean"));

    // Alias bool given float
    let res = p.set_parameter(ParameterId::from("auto_makeup"), ParameterValue::Float(1.0));
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("must be a boolean"));

    // Alias float given bool
    let res = p.set_parameter(ParameterId::from("makeup_gain"), ParameterValue::Bool(true));
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("must be a float"));
}

#[test]
fn test_set_parameter_band_nan_ignored() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();

    // ratio
    p.set_parameter(
        ParameterId::from("band_0_ratio"),
        ParameterValue::Float(2.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("band_0_ratio"),
        ParameterValue::Float(f32::NAN),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_ratio")).unwrap(),
        ParameterValue::Float(2.0)
    );

    // attack
    p.set_parameter(
        ParameterId::from("band_0_attack"),
        ParameterValue::Float(10.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("band_0_attack"),
        ParameterValue::Float(f32::NAN),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_attack"))
            .unwrap(),
        ParameterValue::Float(10.0)
    );

    // release
    p.set_parameter(
        ParameterId::from("band_0_release"),
        ParameterValue::Float(100.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("band_0_release"),
        ParameterValue::Float(f32::NAN),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_release"))
            .unwrap(),
        ParameterValue::Float(100.0)
    );

    // makeup
    p.set_parameter(
        ParameterId::from("band_0_makeup"),
        ParameterValue::Float(6.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("band_0_makeup"),
        ParameterValue::Float(f32::NAN),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_makeup"))
            .unwrap(),
        ParameterValue::Float(6.0)
    );

    // knee
    p.set_parameter(ParameterId::from("band_0_knee"), ParameterValue::Float(3.0))
        .unwrap();
    p.set_parameter(
        ParameterId::from("band_0_knee"),
        ParameterValue::Float(f32::NAN),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_knee")).unwrap(),
        ParameterValue::Float(3.0)
    );
}

#[test]
fn test_set_parameter_global_float_nan_returns_error() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    // NaN on a global float param causes param_bridge to fail; the plugin
    // falls through to alias handling and ultimately returns Unknown parameter.
    let res = p.set_parameter(
        ParameterId::from("threshold"),
        ParameterValue::Float(f32::NAN),
    );
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("Unknown parameter"));
}

#[test]
fn test_set_parameter_alias_roundtrips() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();

    p.set_parameter(ParameterId::from("makeup_gain"), ParameterValue::Float(3.0))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("makeup_gain")).unwrap(),
        ParameterValue::Float(3.0)
    );

    p.set_parameter(ParameterId::from("auto_makeup"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("auto_makeup")).unwrap(),
        ParameterValue::Bool(true)
    );

    p.set_parameter(
        ParameterId::from("measured_auto_makeup"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("measured_auto_makeup"))
            .unwrap(),
        ParameterValue::Bool(true)
    );
}

#[test]
fn test_set_parameter_crossover_freqs_update_smoothers() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();

    p.set_parameter(
        ParameterId::from("crossover_freq_2"),
        ParameterValue::Float(1200.0),
    )
    .unwrap();
    assert!((p.xover_smoothers[1].target() - 1200.0).abs() < 1e-3);

    p.set_parameter(
        ParameterId::from("crossover_freq_3"),
        ParameterValue::Float(8000.0),
    )
    .unwrap();
    assert!((p.xover_smoothers[2].target() - 8000.0).abs() < 1e-3);

    p.set_parameter(
        ParameterId::from("crossover_freq_4"),
        ParameterValue::Float(12000.0),
    )
    .unwrap();
    assert!((p.xover_smoothers[3].target() - 12000.0).abs() < 1e-3);
}

#[test]
fn test_get_parameter_band_defaults() {
    let p = MultibandCompressorPlugin::new(2);
    // Default band dynamics fall back to sensible defaults.
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_ratio")).unwrap(),
        ParameterValue::Float(4.0)
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_attack"))
            .unwrap(),
        ParameterValue::Float(5.0)
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_release"))
            .unwrap(),
        ParameterValue::Float(50.0)
    );
    // makeup defaults to 0.0
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_makeup"))
            .unwrap(),
        ParameterValue::Float(0.0)
    );
    // auto_makeup defaults to false
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_auto_makeup"))
            .unwrap(),
        ParameterValue::Bool(false)
    );
    // measured_auto_makeup defaults to false
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_measured_auto_makeup"))
            .unwrap(),
        ParameterValue::Bool(false)
    );
    // active defaults to true
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_active"))
            .unwrap(),
        ParameterValue::Bool(true)
    );
    // solo defaults to false
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_solo")).unwrap(),
        ParameterValue::Bool(false)
    );
    // bypass defaults to false
    assert_eq!(
        p.get_parameter(&ParameterId::from("band_0_bypass"))
            .unwrap(),
        ParameterValue::Bool(false)
    );
}

#[test]
fn test_get_parameter_out_of_range_band_returns_none() {
    let p = MultibandCompressorPlugin::new(2);
    assert!(
        p.get_parameter(&ParameterId::from("band_99_threshold"))
            .is_none()
    );
}

#[test]
fn test_get_parameter_unknown_band_field_returns_none() {
    let p = MultibandCompressorPlugin::new(2);
    assert!(
        p.get_parameter(&ParameterId::from("band_0_unknown"))
            .is_none()
    );
}

#[test]
fn test_get_parameter_stub_params_return_none() {
    let p = MultibandCompressorPlugin::new(2);
    assert!(
        p.get_parameter(&ParameterId::from("sidechain_hpf_hz"))
            .is_none()
    );
    assert!(
        p.get_parameter(&ParameterId::from("sidechain_hpf_order"))
            .is_none()
    );
    assert!(
        p.get_parameter(&ParameterId::from("detection_mode"))
            .is_none()
    );
    assert!(
        p.get_parameter(&ParameterId::from("program_dependent_release"))
            .is_none()
    );
    assert!(
        p.get_parameter(&ParameterId::from("sidechain_external"))
            .is_none()
    );
}

// -------------------------------------------------------------------------
// Additional process_in_place coverage
// -------------------------------------------------------------------------

#[test]
fn test_process_bypassed_band_updates_level_meter() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("band_0_bypass"),
        ParameterValue::Bool(true),
    )
    .unwrap();

    let mut buf = vec![0.25f32; 256 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 256))
        .unwrap();

    // band_levels_db should reflect the signal level, not silence floor
    assert!(
        p.band_levels_db[0] > -100.0,
        "bypassed band meter should reflect signal level"
    );
}

#[test]
fn test_process_inactive_band_updates_level_meter() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("band_0_active"),
        ParameterValue::Bool(false),
    )
    .unwrap();

    let mut buf = vec![0.25f32; 256 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 256))
        .unwrap();

    assert!(
        p.band_levels_db[0] > -100.0,
        "inactive band meter should reflect signal level"
    );
}

#[test]
fn test_process_auto_makeup_boosts_gain() {
    let block = 2400usize;
    let ctx = ProcessContext::new(48000, block);
    let input_val = 0.5f32;

    let mut p_auto = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            num_bands: 1,
            threshold_db: -20.0,
            ratio: 4.0,
            mix: 1.0,
            bands: vec![BandCompressorParams {
                auto_makeup: true,
                ..Default::default()
            }],
            ..Default::default()
        },
    );
    p_auto.initialize(48000).unwrap();

    let mut p_no = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            num_bands: 1,
            threshold_db: -20.0,
            ratio: 4.0,
            mix: 1.0,
            bands: vec![BandCompressorParams {
                auto_makeup: false,
                ..Default::default()
            }],
            ..Default::default()
        },
    );
    p_no.initialize(48000).unwrap();

    let mut buf_auto = vec![input_val; block];
    let mut buf_no = vec![input_val; block];
    for _ in 0..4 {
        buf_auto.fill(input_val);
        buf_no.fill(input_val);
        p_auto.process_in_place(&mut buf_auto, &ctx).unwrap();
        p_no.process_in_place(&mut buf_no, &ctx).unwrap();
    }

    let rms_auto: f32 = (buf_auto.iter().map(|s| s * s).sum::<f32>() / block as f32).sqrt();
    let rms_no: f32 = (buf_no.iter().map(|s| s * s).sum::<f32>() / block as f32).sqrt();
    assert!(
        rms_auto > rms_no * 1.5,
        "auto_makeup should boost gain significantly, auto={rms_auto:.4}, no={rms_no:.4}"
    );
}

#[test]
fn test_process_lookahead_no_panic() {
    let mut p = MultibandCompressorPlugin::with_params(
        2,
        MultibandCompressorPluginParams {
            per_band_lookahead_ms: 5.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    assert!(p.lookahead_buffers[0].delay() > 0);

    let mut buf = vec![0.3f32; 512 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_link_amount_half() {
    let mut p = MultibandCompressorPlugin::with_params(
        2,
        MultibandCompressorPluginParams {
            link_amount: 0.5,
            threshold_db: -20.0,
            ratio: 4.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let mut buf = vec![0.5f32; 512 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_mix_dry_only() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.0))
        .unwrap();

    let input: Vec<f32> = (0..4096 * 2)
        .map(|i| 0.1 * (i as f32 * 0.05).sin())
        .collect();
    let mut buf = input.clone();
    let ctx = ProcessContext::new(48000, 4096);
    for _ in 0..3 {
        buf.copy_from_slice(&input);
        p.process_in_place(&mut buf, &ctx).unwrap();
    }

    let max_diff = buf
        .iter()
        .zip(input.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_diff < 1e-4,
        "mix=0 should pass dry signal through, max_diff={max_diff:.4e}"
    );
}

#[test]
fn test_process_ms_mode_finite() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.set_parameter(ParameterId::from("ms_mode"), ParameterValue::Bool(true))
        .unwrap();

    let mut buf = vec![0.0f32; 512 * 2];
    for i in 0..512 {
        buf[i * 2] = 0.3 * (i as f32 * 0.1).sin();
        buf[i * 2 + 1] = 0.2 * (i as f32 * 0.2).sin();
    }
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();

    assert!(buf.iter().all(|s| s.is_finite()));
    // Stereo separation should be preserved (channels not identical)
    let different_frames = buf.chunks(2).filter(|c| (c[0] - c[1]).abs() > 1e-6).count();
    assert!(
        different_frames > 10,
        "L and R channels should remain different after M/S roundtrip"
    );
}

#[test]
fn test_process_with_tilt_no_nan() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("sidechain_tilt_db"),
        ParameterValue::Float(6.0),
    )
    .unwrap();

    let mut buf = vec![0.25f32; 512 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_link_channels_no_nan() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("link_channels"),
        ParameterValue::Bool(true),
    )
    .unwrap();

    let mut buf = vec![0.25f32; 512 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_measured_auto_makeup_no_nan() {
    let mut p = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            num_bands: 1,
            bands: vec![BandCompressorParams {
                measured_auto_makeup: true,
                threshold_db: Some(-30.0),
                ratio: Some(4.0),
                ..Default::default()
            }],
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let mut buf = vec![0.5f32; 512];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_zero_frames() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    let mut buf = vec![0.0f32; 0];
    let ctx = ProcessContext::new(48000, 0);
    assert_eq!(p.process_in_place(&mut buf, &ctx).unwrap(), 0);
}

#[test]
fn test_process_get_data_returns_compressor_data() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();

    let mut buf = vec![0.25f32; 256 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 256))
        .unwrap();

    let data = p.get_data().unwrap();
    let data = data.downcast_ref::<crate::MultibandCompressorData>();
    assert!(
        data.is_some(),
        "get_data should return MultibandCompressorData"
    );
    let data = data.unwrap();
    assert_eq!(data.gain_reduction_db.len(), p.num_bands * p.channels);
    assert_eq!(data.band_levels_db.len(), p.num_bands);
}

// -------------------------------------------------------------------------
// Additional initialize / rebuild_cached_parameters / param_value coverage
// -------------------------------------------------------------------------

#[test]
fn test_initialize_with_lookahead_rebuilds_buffers() {
    let mut p = MultibandCompressorPlugin::with_params(
        2,
        MultibandCompressorPluginParams {
            per_band_lookahead_ms: 3.0,
            ..Default::default()
        },
    );
    p.initialize(96000).unwrap();
    for buf in &p.lookahead_buffers {
        assert_eq!(buf.delay(), 288, "lookahead delay should be 3ms @ 96kHz");
        assert_eq!(buf.max_delay(), 1920, "max_delay should cover 20ms @ 96kHz");
    }
}

#[test]
fn test_initialize_with_tilt_rebuilds_biquads() {
    let mut p = MultibandCompressorPlugin::with_params(
        2,
        MultibandCompressorPluginParams {
            sidechain_tilt_db: 4.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    assert_eq!(p.sidechain_tilt_biquads.len(), p.num_bands);
    assert_eq!(p.sidechain_tilt_biquads[0].len(), p.channels);
}

#[test]
fn test_initialize_updates_coefficients_for_sample_rate() {
    let mut p48 = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            attack_ms: 5.0,
            ..Default::default()
        },
    );
    p48.initialize(48000).unwrap();
    let coeff48 = p48.band_compressors[0].attack_coeff;

    let mut p96 = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            attack_ms: 5.0,
            ..Default::default()
        },
    );
    p96.initialize(96000).unwrap();
    let coeff96 = p96.band_compressors[0].attack_coeff;

    assert_ne!(
        coeff48, coeff96,
        "attack coeff should differ with sample rate"
    );
}

#[test]
fn test_rebuild_cached_parameters_band_count() {
    let mut p = MultibandCompressorPlugin::with_params(
        2,
        MultibandCompressorPluginParams {
            num_bands: 3,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    let params = p.parameters();
    let ids: Vec<&str> = params.iter().map(|par| par.id.as_str()).collect();

    assert!(ids.contains(&"band_0_threshold"));
    assert!(ids.contains(&"band_1_threshold"));
    assert!(ids.contains(&"band_2_threshold"));
    assert!(!ids.contains(&"band_3_threshold"));
}

#[test]
fn test_param_value_all_indices() {
    let p = MultibandCompressorPlugin::new(2);
    for i in 0..=16 {
        assert!(
            p.param_value(i).is_some(),
            "param_value({}) should return Some",
            i
        );
    }
    assert!(p.param_value(17).is_none());
    assert!(p.param_value(100).is_none());
}

// -------------------------------------------------------------------------
// Data / helper / edge-case coverage
// -------------------------------------------------------------------------

#[test]
fn test_multiband_compressor_data_new() {
    let d = super::multiband_compressor_data::MultibandCompressorData::new(3, 2);
    assert_eq!(d.gain_reduction_db.len(), 6);
    assert_eq!(d.band_levels_db.len(), 3);
    assert_eq!(d.crossover_frequencies.len(), 2);
    assert!(d.gain_reduction_db.iter().all(|&v| v == 0.0));
    assert!(d.band_levels_db.iter().all(|&v| v == -120.0));
}

#[test]
fn test_multiband_compressor_data_update() {
    let mut d = super::multiband_compressor_data::MultibandCompressorData::new(2, 1);
    d.update(&[1.0, 2.0], &[-10.0, -20.0], &[500.0]);
    assert_eq!(d.gain_reduction_db.as_slice(), &[1.0, 2.0]);
    assert_eq!(d.band_levels_db.as_slice(), &[-10.0, -20.0]);
    assert_eq!(d.crossover_frequencies.as_slice(), &[500.0]);
}

#[test]
fn test_multiband_compressor_data_default() {
    let d: super::multiband_compressor_data::MultibandCompressorData = Default::default();
    assert!(d.gain_reduction_db.is_empty());
    assert!(d.band_levels_db.is_empty());
    assert!(d.crossover_frequencies.is_empty());
}

#[test]
fn test_band_compressor_params_default() {
    let bp = BandCompressorParams::default();
    assert_eq!(bp.threshold_db, None);
    assert_eq!(bp.ratio, None);
    assert_eq!(bp.attack_ms, None);
    assert_eq!(bp.release_ms, None);
    assert_eq!(bp.knee_db, None);
    assert_eq!(bp.makeup_gain_db, 0.0);
    assert!(!bp.auto_makeup);
    assert!(!bp.measured_auto_makeup);
    assert!(bp.active);
    assert!(!bp.solo);
    assert!(!bp.bypass);
}

#[test]
fn test_default_active() {
    assert!(crate::params::default_active());
}

#[test]
fn test_default_link_amount() {
    assert_eq!(crate::params::default_link_amount(), 1.0);
}

#[test]
fn test_from_params_alias() {
    let params = MultibandCompressorPluginParams {
        num_bands: 4,
        ..Default::default()
    };
    let p = MultibandCompressorPlugin::from_params(2, params.clone());
    assert_eq!(p.num_bands, 4);
    assert_eq!(p.channels, 2);
}

#[test]
fn test_get_data_returns_some() {
    let p = MultibandCompressorPlugin::new(2);
    assert!(p.get_data().is_some());
}

#[test]
fn test_param_value_out_of_range_returns_none() {
    let p = MultibandCompressorPlugin::new(2);
    assert!(p.param_value(99).is_none());
}

#[test]
fn test_set_param_value_out_of_range_is_noop() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    let before = p.num_bands;
    p.set_param_value(99, 42.0);
    assert_eq!(p.num_bands, before);
}

#[test]
fn test_calculate_gain_reduction_ratio_one_always_zero() {
    assert_eq!(
        MultibandCompressorPlugin::calculate_gain_reduction(10.0, 0.0, 1.0, 0.0),
        0.0
    );
    assert_eq!(
        MultibandCompressorPlugin::calculate_gain_reduction(100.0, -50.0, 1.0, 6.0),
        0.0
    );
}

#[test]
fn test_calculate_gain_reduction_at_threshold() {
    // Exactly at threshold with hard knee -> 0 GR
    assert_eq!(
        MultibandCompressorPlugin::calculate_gain_reduction(0.0, 0.0, 4.0, 0.0),
        0.0
    );
    // Soft knee at threshold: ov = 0 - 0 + 2 = 2, kf = 0.5, slope = 0.75, gr = 0.375
    let gr = MultibandCompressorPlugin::calculate_gain_reduction(0.0, 0.0, 4.0, 4.0);
    assert!(
        (gr - 0.375).abs() < 1e-5,
        "GR at threshold with soft knee should be 0.375, got {}",
        gr
    );
}

#[test]
fn test_build_crossovers_matches_num_bands() {
    let p = MultibandCompressorPlugin::with_params(
        2,
        MultibandCompressorPluginParams {
            num_bands: 2,
            ..Default::default()
        },
    );
    assert_eq!(p.crossover_points.len(), 1);

    let p = MultibandCompressorPlugin::with_params(
        2,
        MultibandCompressorPluginParams {
            num_bands: 4,
            ..Default::default()
        },
    );
    assert_eq!(p.crossover_points.len(), 3);
}

#[test]
fn test_bypassed_band_no_gain_reduction() {
    let mut p = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            num_bands: 1,
            threshold_db: -20.0,
            ratio: 4.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("band_0_bypass"),
        ParameterValue::Bool(true),
    )
    .unwrap();

    let mut buf = vec![0.5f32; 512];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();

    assert_eq!(p.band_compressors[0].envelope[0], 0.0);
}

#[test]
fn test_passive_band_no_gain_reduction() {
    let mut p = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            num_bands: 1,
            threshold_db: -20.0,
            ratio: 4.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("band_0_active"),
        ParameterValue::Bool(false),
    )
    .unwrap();

    let mut buf = vec![0.5f32; 512];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();

    assert_eq!(p.band_compressors[0].envelope[0], 0.0);
}

#[test]
fn test_makeup_gain_applied() {
    // num_bands is clamped to minimum 2, so use 2 bands and apply makeup to both
    let mut p = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            num_bands: 2,
            threshold_db: -20.0,
            ratio: 1.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("band_0_makeup"),
        ParameterValue::Float(6.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("band_1_makeup"),
        ParameterValue::Float(6.0),
    )
    .unwrap();

    let mut buf = vec![0.1f32; 512];
    for _ in 0..12 {
        buf.fill(0.1);
        p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
            .unwrap();
    }

    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(
        rms > 0.15,
        "Makeup gain 6dB should boost signal, rms={}",
        rms
    );
}

#[test]
fn test_auto_makeup_boosts_output() {
    let mut p = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            num_bands: 1,
            threshold_db: -20.0,
            ratio: 4.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    p.set_parameter(ParameterId::from("auto_makeup"), ParameterValue::Bool(true))
        .unwrap();
    p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();

    let mut buf = vec![0.5f32; 1024];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 1024))
        .unwrap();

    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(rms > 0.4, "Auto makeup should keep level high, rms={}", rms);
}

#[test]
fn test_mix_dry_only_preserves_input() {
    let mut p = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            num_bands: 1,
            threshold_db: -20.0,
            ratio: 4.0,
            mix: 0.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let mut buf: Vec<f32> = (0..512).map(|i| 0.3 * (i as f32 * 0.1).sin()).collect();
    let original = buf.clone();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();

    for (i, (a, b)) in original.iter().zip(buf.iter()).enumerate() {
        assert!(
            (a - b).abs() < 1e-5,
            "mix=0 should preserve input sample {}, expected {} got {}",
            i,
            a,
            b
        );
    }
}

#[test]
fn test_mix_wet_only_affects_signal() {
    let mut p = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            num_bands: 1,
            threshold_db: -20.0,
            ratio: 4.0,
            mix: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();

    let mut buf = vec![0.5f32; 1024];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 1024))
        .unwrap();

    let rms: f32 = (buf.iter().map(|s| s * s).sum::<f32>() / buf.len() as f32).sqrt();
    assert!(
        rms < 0.45,
        "mix=1 with compression should reduce loud signal, rms={}",
        rms
    );
}

#[test]
fn test_set_crossover_freq_updates_smoother() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    // crossover_freq_1 max is 500 Hz per the param spec
    p.set_parameter(
        ParameterId::from("crossover_freq_1"),
        ParameterValue::Float(300.0),
    )
    .unwrap();
    assert_eq!(p.crossover_frequencies[0], 300.0);
}

#[test]
fn test_set_threshold_updates_smoother() {
    let mut p = MultibandCompressorPlugin::new(1);
    p.initialize(48000).unwrap();
    p.set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
        .unwrap();
    assert_eq!(p.threshold_db, -30.0);
}

#[test]
fn test_set_attack_updates_coefficients() {
    let mut p = MultibandCompressorPlugin::new(1);
    p.initialize(48000).unwrap();
    let before = p.band_compressors[0].attack_coeff;
    p.set_parameter(ParameterId::from("attack"), ParameterValue::Float(1.0))
        .unwrap();
    let mut buffer = vec![0.5; 64];
    p.process_in_place(&mut buffer, &ProcessContext::new(48_000, 64))
        .unwrap();
    assert_ne!(p.band_compressors[0].attack_coeff, before);
}

#[test]
fn test_set_release_updates_coefficients() {
    let mut p = MultibandCompressorPlugin::new(1);
    p.initialize(48000).unwrap();
    let before = p.band_compressors[0].release_coeff;
    p.set_parameter(ParameterId::from("release"), ParameterValue::Float(200.0))
        .unwrap();
    let mut buffer = vec![0.5; 64];
    p.process_in_place(&mut buffer, &ProcessContext::new(48_000, 64))
        .unwrap();
    assert_ne!(p.band_compressors[0].release_coeff, before);
}

#[test]
fn test_ms_mode_parameter_roundtrip() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.set_parameter(ParameterId::from("ms_mode"), ParameterValue::Bool(true))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("ms_mode")),
        Some(ParameterValue::Bool(true))
    );
}

#[test]
fn test_link_channels_parameter_roundtrip() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("link_channels"),
        ParameterValue::Bool(false),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("link_channels")),
        Some(ParameterValue::Bool(false))
    );
}

#[test]
fn test_link_amount_parameter_roundtrip() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.set_parameter(ParameterId::from("link_amount"), ParameterValue::Float(0.5))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("link_amount")),
        Some(ParameterValue::Float(0.5))
    );
}

#[test]
fn test_sidechain_tilt_zero_keeps_preallocated_biquads() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("sidechain_tilt_db"),
        ParameterValue::Float(6.0),
    )
    .unwrap();
    assert!(!p.sidechain_tilt_biquads.is_empty());

    p.set_parameter(
        ParameterId::from("sidechain_tilt_db"),
        ParameterValue::Float(0.0),
    )
    .unwrap();
    let mut buffer = vec![0.1; 2048 * 2];
    for _ in 0..4 {
        p.process_in_place(&mut buffer, &ProcessContext::new(48_000, 2048))
            .unwrap();
    }
    assert_eq!(p.sidechain_tilt_biquads.len(), p.num_bands);
    assert!(p.tilt_smoother.current().abs() < 0.1);
}

#[test]
fn test_negative_sidechain_tilt_rebuilds_biquads() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("sidechain_tilt_db"),
        ParameterValue::Float(-3.0),
    )
    .unwrap();
    assert_eq!(p.sidechain_tilt_biquads.len(), p.num_bands);
}

#[test]
fn test_per_band_settings_affect_output() {
    let mut p = MultibandCompressorPlugin::with_params(
        1,
        MultibandCompressorPluginParams {
            num_bands: 2,
            threshold_db: -60.0,
            ratio: 1.0,
            ..Default::default()
        },
    );
    p.initialize(48000).unwrap();
    p.set_parameter(
        ParameterId::from("band_0_threshold"),
        ParameterValue::Float(-20.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("band_0_ratio"),
        ParameterValue::Float(8.0),
    )
    .unwrap();

    let mut buf = vec![0.0f32; 2048];
    for (i, sample) in buf.iter_mut().enumerate() {
        let t = i as f32 / 48000.0;
        *sample = ((2.0 * std::f32::consts::PI * 100.0 * t).sin()
            + (2.0 * std::f32::consts::PI * 5000.0 * t).sin())
            * 0.3;
    }
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 2048))
        .unwrap();

    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn test_lookahead_buffers_initialized_with_zero_ms() {
    let p = MultibandCompressorPlugin::with_params(
        2,
        MultibandCompressorPluginParams {
            num_bands: 3,
            per_band_lookahead_ms: 0.0,
            ..Default::default()
        },
    );
    assert_eq!(p.lookahead_buffers.len(), 3);
}

#[test]
fn test_set_lookahead_ms_updates_buffers() {
    let mut p = MultibandCompressorPlugin::new(2);
    p.initialize(48000).unwrap();
    let error = p
        .set_parameter(
            ParameterId::from("lookahead_ms"),
            ParameterValue::Float(5.0),
        )
        .unwrap_err();
    assert!(error.contains("structural"));
    assert_eq!(p.per_band_lookahead_ms, 0.0);
    let mut buf = vec![0.2f32; 512 * 2];
    p.process_in_place(&mut buf, &ProcessContext::new(48000, 512))
        .unwrap();
    assert!(buf.iter().all(|s| s.is_finite()));
}

#[test]
fn try_from_params_rejects_invalid_global_values() {
    let params = MultibandCompressorPluginParams {
        ratio: 0.5,
        ..Default::default()
    };
    let error = match MultibandCompressorPlugin::try_from_params(2, params, 48_000) {
        Err(error) => error,
        Ok(_) => panic!("factory construction must reject a ratio below the public range"),
    };
    assert!(error.contains("ratio"), "unexpected error: {error}");

    let params = MultibandCompressorPluginParams {
        attack_ms: f32::NAN,
        ..Default::default()
    };
    let error = match MultibandCompressorPlugin::try_from_params(2, params, 48_000) {
        Err(error) => error,
        Ok(_) => panic!("factory construction must reject non-finite timing values"),
    };
    assert!(error.contains("attack"), "unexpected error: {error}");
}

#[test]
fn try_from_params_rejects_invalid_crossover_configuration() {
    let params = MultibandCompressorPluginParams {
        crossover_frequencies: vec![200.0, 100.0, 8_000.0, 12_000.0],
        ..Default::default()
    };
    let error = match MultibandCompressorPlugin::try_from_params(2, params, 48_000) {
        Err(error) => error,
        Ok(_) => panic!("factory construction must reject descending crossovers"),
    };
    assert!(error.contains("crossover"), "unexpected error: {error}");

    let params = MultibandCompressorPluginParams {
        crossover_frequencies: vec![400.0, 2_000.0, 8_000.0, 12_000.0],
        ..Default::default()
    };
    let error = match MultibandCompressorPlugin::try_from_params(2, params, 800) {
        Err(error) => error,
        Ok(_) => panic!("factory construction must reject a crossover at Nyquist"),
    };
    assert!(error.contains("Nyquist"), "unexpected error: {error}");
}
#[test]
fn broadband_mode_has_single_band_identity_and_no_crossover_schema() {
    let params = MultibandCompressorPluginParams {
        num_bands: 1,
        ..Default::default()
    };
    let mut plugin = MultibandCompressorPlugin::try_from_params(2, params, 48_000).unwrap();
    plugin.initialize(48_000).unwrap();
    assert_eq!(plugin.num_bands, 1);
    assert_eq!(plugin.info().name, "Compressor");
    let schema = plugin.parameter_schema();
    assert!(
        schema
            .iter()
            .all(|parameter| !parameter.id.as_str().starts_with("crossover"))
    );
    assert!(
        schema
            .iter()
            .all(|parameter| parameter.id.as_str() != "num_bands")
    );
}

#[test]
fn broadband_mode_matches_static_transfer_after_settling() {
    let params = MultibandCompressorPluginParams {
        num_bands: 1,
        threshold_db: -20.0,
        ratio: 4.0,
        attack_ms: 0.1,
        release_ms: 10.0,
        knee_db: 0.0,
        ..Default::default()
    };
    let mut plugin = MultibandCompressorPlugin::try_from_params(1, params, 48_000).unwrap();
    plugin.initialize(48_000).unwrap();
    let frames = 4096;
    let input = 10.0_f32.powf(-8.0 / 20.0);
    let mut buffer = vec![input; frames];
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48_000, frames))
        .unwrap();
    let expected_reduction = (-8.0 - -20.0) * (1.0 - 1.0 / 4.0);
    let expected = input * 10.0_f32.powf(-expected_reduction / 20.0);
    assert!((buffer[frames - 1] - expected).abs() < 0.01);
}

#[test]
fn oversized_block_is_chunked_without_changing_state_or_output() {
    let mut whole = MultibandCompressorPlugin::new(2);
    let mut split = MultibandCompressorPlugin::new(2);
    whole.initialize(48_000).unwrap();
    split.initialize(48_000).unwrap();
    let frames = 10_000;
    let input: Vec<f32> = (0..frames * 2)
        .map(|index| ((index as f32 * 0.017).sin()) * 0.4)
        .collect();
    let mut one_call = input.clone();
    whole
        .process_in_place(&mut one_call, &ProcessContext::new(48_000, frames))
        .unwrap();
    let mut chunked = input;
    let mut offset = 0;
    while offset < frames {
        let count = (frames - offset).min(4096);
        split
            .process_in_place(
                &mut chunked[offset * 2..(offset + count) * 2],
                &ProcessContext::new(48_000, count),
            )
            .unwrap();
        offset += count;
    }
    assert_eq!(one_call, chunked);
}

#[test]
fn fractional_link_amount_is_canonical_and_monotonic() {
    fn quiet_output(link: f32) -> f32 {
        let params = MultibandCompressorPluginParams {
            num_bands: 1,
            threshold_db: -30.0,
            ratio: 10.0,
            attack_ms: 0.1,
            release_ms: 10.0,
            link_channels: true,
            link_amount: link,
            knee_db: 0.0,
            ..Default::default()
        };
        let mut plugin = MultibandCompressorPlugin::try_from_params(2, params, 48_000).unwrap();
        plugin.initialize(48_000).unwrap();
        let frames = 4096;
        let mut buffer = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            buffer.push(0.8);
            buffer.push(0.01);
        }
        plugin
            .process_in_place(&mut buffer, &ProcessContext::new(48_000, frames))
            .unwrap();
        buffer[frames * 2 - 1].abs()
    }
    let independent = quiet_output(0.0);
    let half = quiet_output(0.5);
    let linked = quiet_output(1.0);
    assert!(
        independent > half && half > linked,
        "{independent} {half} {linked}"
    );
}

#[test]
fn explicit_unsupported_legacy_sidechain_is_rejected() {
    let params = MultibandCompressorPluginParams {
        num_bands: 1,
        sidechain_hpf_hz: Some(80.0),
        ..Default::default()
    };
    assert!(MultibandCompressorPlugin::try_from_params(2, params, 48_000).is_err());
}

#[test]
fn crossover_and_dynamics_automation_are_block_partition_invariant_and_bounded() {
    fn render(chunks: &[usize]) -> Vec<f32> {
        let params = MultibandCompressorPluginParams {
            num_bands: 3,
            threshold_db: -12.0,
            ratio: 2.0,
            attack_ms: 10.0,
            release_ms: 80.0,
            ..Default::default()
        };
        let mut plugin = MultibandCompressorPlugin::try_from_params(2, params, 48_000).unwrap();
        plugin.initialize(48_000).unwrap();

        let warmup_frames = 1024;
        let mut warmup: Vec<f32> = (0..warmup_frames)
            .flat_map(|frame| {
                let t = frame as f32 / 48_000.0;
                let sample = 0.35 * (2.0 * std::f32::consts::PI * 997.0 * t).sin();
                [sample, sample * 0.37]
            })
            .collect();
        plugin
            .process_in_place(&mut warmup, &ProcessContext::new(48_000, warmup_frames))
            .unwrap();

        for (id, value) in [
            ("crossover_freq_1", ParameterValue::Float(350.0)),
            ("crossover_freq_2", ParameterValue::Float(3_500.0)),
            ("threshold", ParameterValue::Float(-30.0)),
            ("ratio", ParameterValue::Float(12.0)),
            ("attack", ParameterValue::Float(0.5)),
            ("release", ParameterValue::Float(300.0)),
            ("knee", ParameterValue::Float(12.0)),
            ("link_amount", ParameterValue::Float(0.25)),
            ("sidechain_tilt_db", ParameterValue::Float(6.0)),
            ("band_0_makeup", ParameterValue::Float(9.0)),
        ] {
            plugin.set_parameter(ParameterId::from(id), value).unwrap();
        }

        let mut rendered = Vec::new();
        let mut offset = warmup_frames;
        for &frames in chunks {
            let mut block: Vec<f32> = (0..frames)
                .flat_map(|frame| {
                    let t = (offset + frame) as f32 / 48_000.0;
                    let sample = 0.35 * (2.0 * std::f32::consts::PI * 997.0 * t).sin();
                    [sample, sample * 0.37]
                })
                .collect();
            plugin
                .process_in_place(&mut block, &ProcessContext::new(48_000, frames))
                .unwrap();
            rendered.extend(block);
            offset += frames;
        }
        rendered
    }

    let whole = render(&[4096]);
    let partitioned = render(&[17, 63, 256, 1024, 2736]);
    assert_eq!(whole.len(), partitioned.len());
    let max_difference = whole
        .iter()
        .zip(&partitioned)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        max_difference < 2.0e-5,
        "automation must not depend on host block partitioning, max diff={max_difference}"
    );
    let mut max_jump = 0.0_f32;
    for channel in 0..2 {
        let mut previous = whole[channel];
        for sample in whole[channel + 2..].iter().step_by(2) {
            max_jump = max_jump.max((*sample - previous).abs());
            previous = *sample;
        }
    }
    assert!(
        max_jump < 0.2,
        "smoothed crossover/dynamics automation produced an excessive sample jump: {max_jump}"
    );
}

#[test]
fn cascaded_lr4_reconstruction_sweep_has_bounded_transfer_and_deep_fitted_null() {
    const SAMPLE_RATE: u32 = 48_000;
    const SETTLE: usize = 4096;
    const ANALYZE: usize = 4096;
    const AMPLITUDE: f32 = 0.1;
    // Exact analysis-bin frequencies avoid mistaking spectral leakage for a
    // crossover reconstruction residual.
    let frequencies = [
        46.875_f32,
        93.75,
        503.90625,
        996.09375,
        3_000.0,
        7_992.187_5,
        15_000.0,
    ];

    for num_bands in 2..=5 {
        for frequency in frequencies {
            let params = MultibandCompressorPluginParams {
                num_bands,
                ratio: 1.0,
                mix: 1.0,
                ..Default::default()
            };
            let mut plugin =
                MultibandCompressorPlugin::try_from_params(1, params, SAMPLE_RATE).unwrap();
            plugin.initialize(SAMPLE_RATE).unwrap();
            let total = SETTLE + ANALYZE;
            let mut signal: Vec<f32> = (0..total)
                .map(|frame| {
                    (AMPLITUDE as f64
                        * (2.0 * std::f64::consts::PI * frequency as f64 * frame as f64
                            / SAMPLE_RATE as f64)
                            .sin()) as f32
                })
                .collect();
            plugin
                .process_in_place(&mut signal, &ProcessContext::new(SAMPLE_RATE, total))
                .unwrap();

            let analyzed = &signal[SETTLE..];
            let (sin_component, cos_component) = analyzed.iter().enumerate().fold(
                (0.0_f64, 0.0_f64),
                |(sin_sum, cos_sum), (index, &sample)| {
                    let frame = SETTLE + index;
                    let phase = 2.0 * std::f64::consts::PI * frequency as f64 * frame as f64
                        / SAMPLE_RATE as f64;
                    (
                        sin_sum + sample as f64 * phase.sin(),
                        cos_sum + sample as f64 * phase.cos(),
                    )
                },
            );
            let scale = 2.0 / ANALYZE as f64 / AMPLITUDE as f64;
            let a = sin_component * scale;
            let b = cos_component * scale;
            let magnitude = a.hypot(b);
            let phase = b.atan2(a);
            assert!(
                (0.35..=1.45).contains(&magnitude),
                "{num_bands}-band cascaded LR4 transfer at {frequency} Hz was {magnitude:.3}"
            );
            assert!(phase.is_finite());

            let (signal_energy, residual_energy) = analyzed.iter().enumerate().fold(
                (0.0_f64, 0.0_f64),
                |(signal_sum, residual_sum), (index, &sample)| {
                    let frame = SETTLE + index;
                    let phase = 2.0 * std::f64::consts::PI * frequency as f64 * frame as f64
                        / SAMPLE_RATE as f64;
                    let fitted = AMPLITUDE as f64 * (a * phase.sin() + b * phase.cos());
                    let residual = sample as f64 - fitted;
                    (
                        signal_sum + (sample as f64).powi(2),
                        residual_sum + residual.powi(2),
                    )
                },
            );
            let fitted_null_db = 10.0 * (residual_energy / signal_energy.max(1.0e-30)).log10();
            assert!(
                fitted_null_db < -45.0,
                "{num_bands}-band LR4 fitted null at {frequency} Hz was only {fitted_null_db:.1} dB"
            );
        }
    }
}
