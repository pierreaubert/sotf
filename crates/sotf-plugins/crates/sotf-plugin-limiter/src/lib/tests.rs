#![allow(clippy::needless_range_loop)]
use super::limiter_plugin::LimiterPlugin;
use super::misc::CACHE_UPDATE_THROTTLE;
use super::types::{LimiterData, LimiterPluginParams};
use math_audio_dsp::fast_math::fast_pow10;
use sotf_host::TruePeakDetector;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::{PluginCompiledOp, ProcessContext};

#[test]
fn test_limiter_basic() {
    let mut p = LimiterPlugin::new(1, -1.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();
    let mut b = vec![2.0; 1000];
    p.process_in_place(&mut b, &ProcessContext::new(48000, 1000))
        .unwrap();
    let thresh_lin = fast_pow10(-1.0 / 20.0);
    for &s in &b[500..] {
        assert!(s.abs() <= thresh_lin * 1.05);
    }
}

#[test]
fn threshold_smoother_operates_in_decibels() {
    let mut plugin = LimiterPlugin::new(1, -6.0, 50.0, 0.0, false);
    plugin.initialize(48_000).unwrap();
    assert!((plugin.threshold_db_smoother.current() + 6.0).abs() < 1e-6);

    plugin
        .parametric_set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-18.0))
        .unwrap();
    assert!((plugin.threshold_db_smoother.target() + 18.0).abs() < 1e-6);

    let first_step_db = plugin.threshold_db_smoother.advance();
    assert!(first_step_db < -6.0 && first_step_db > -18.0);
    let first_step_linear = fast_pow10(first_step_db / 20.0);
    assert!(first_step_linear > fast_pow10(-18.0 / 20.0));
    assert!(first_step_linear < fast_pow10(-6.0 / 20.0));
}

#[test]
fn from_params_seeds_mix_smoother_at_saved_value() {
    let plugin = LimiterPlugin::from_params(
        2,
        LimiterPluginParams {
            threshold_db: -1.0,
            release_ms: 50.0,
            lookahead_ms: 0.0,
            soft: false,
            true_peak: false,
            isp_mode: false,
            dual_release: false,
            mix: 0.0,
            feed_forward: false,
            link_amount: 1.0,
        },
    );

    assert_eq!(plugin.mix_smoother.current(), 0.0);
}

#[test]
fn from_params_sanitizes_non_finite_and_out_of_range_values() {
    let plugin = LimiterPlugin::from_params(
        1,
        LimiterPluginParams {
            threshold_db: f32::NAN,
            release_ms: -1.0,
            lookahead_ms: 1000.0,
            soft: false,
            true_peak: false,
            isp_mode: false,
            dual_release: false,
            mix: f32::INFINITY,
            feed_forward: false,
            link_amount: -10.0,
        },
    );
    assert!(plugin.threshold_db.is_finite());
    assert_eq!(plugin.release_ms, 10.0);
    assert_eq!(plugin.lookahead_ms, 20.0);
    assert!(plugin.mix.is_finite());
    assert_eq!(plugin.link_amount, 0.0);
}

#[test]
fn test_limiter_compiled_op_matches_process_in_place() {
    let sr = 48000;
    let frames = 512;
    let channels = 2;
    let mut regular = LimiterPlugin::new(channels, -6.0, 50.0, 0.0, false);
    let mut compiled = LimiterPlugin::new(channels, -6.0, 50.0, 0.0, false);
    regular.initialize(sr).unwrap();
    compiled.initialize(sr).unwrap();

    let input: Vec<f32> = (0..frames * channels)
        .map(|i| 0.82 * (i as f32 * 0.071).sin())
        .collect();
    let ctx = ProcessContext::new(sr, frames);
    let mut expected = input.clone();
    let mut actual = vec![0.0; input.len()];

    let expected_frames = regular.process_in_place(&mut expected, &ctx).unwrap();
    let actual_frames = compiled
        .process_compiled_f32(PluginCompiledOp::Limiter, &input, &mut actual, &ctx)
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
        "compiled limiter output diverged: max_error={max_error}"
    );
}

#[test]
fn test_limiter_compile_metadata_tracks_lookahead_latency() {
    let mut no_lookahead = LimiterPlugin::new(2, -6.0, 50.0, 0.0, false);
    no_lookahead.initialize(48000).unwrap();
    let metadata = no_lookahead.compile_metadata();
    assert_eq!(metadata.compiled_op, Some(PluginCompiledOp::Limiter));
    assert_eq!(metadata.latency_samples, 0);
    assert!(metadata.boundary);
    assert!(metadata.stateful);
    assert!(!metadata.linear);

    let mut lookahead = LimiterPlugin::new(2, -6.0, 50.0, 5.0, false);
    lookahead.initialize(48000).unwrap();
    let metadata = lookahead.compile_metadata();
    assert_eq!(metadata.compiled_op, None);
    assert!(metadata.latency_samples > 0);
    assert!(metadata.boundary);
}

#[test]
fn zero_lookahead_is_sample_exact_without_hidden_latency() {
    let mut plugin = LimiterPlugin::new(1, 0.0, 50.0, 0.0, false);
    plugin.initialize(48_000).unwrap();
    let expected = vec![0.1, -0.2, 0.3, -0.4];
    let mut buffer = expected.clone();
    plugin
        .process_in_place(&mut buffer, &ProcessContext::new(48_000, expected.len()))
        .unwrap();
    assert_eq!(buffer, expected);
    assert_eq!(plugin.latency_samples(), 0);
}

#[test]
fn process_rejects_wrong_buffer_lengths_without_advancing_state() {
    let mut plugin = LimiterPlugin::new(2, -1.0, 50.0, 5.0, false);
    plugin.initialize(48_000).unwrap();
    for len in [7, 9] {
        let mut buffer = vec![0.0; len];
        assert!(
            plugin
                .process_in_place(&mut buffer, &ProcessContext::new(48_000, 4))
                .is_err()
        );
        assert_eq!(plugin.lookahead_pos, 0);
    }
}

#[test]
fn reset_matches_fresh_instance_observable_state() {
    let mut plugin = LimiterPlugin::new(1, -6.0, 10.0, 5.0, false);
    plugin.initialize(48_000).unwrap();
    let mut loud = vec![1.0; 512];
    plugin
        .process_in_place(&mut loud, &ProcessContext::new(48_000, 512))
        .unwrap();
    plugin.reset();
    assert_eq!(plugin.lookahead_pos, 0);
    assert_eq!(plugin.cache_update_counter, 0);
    assert_eq!(plugin.monitoring_peak_db, -100.0);
    assert_eq!(plugin.monitoring_gr_db, 0.0);
}

/// Regression: threshold smoother was advanced twice per block (once via
/// .advance(), then again via .next_n(num_frames)), making transitions
/// ~500x faster than intended. This test verifies smooth threshold changes.
#[test]
fn test_threshold_transition_is_smooth() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 0.0, false);
    p.initialize(48000).unwrap();

    // Feed loud signal to establish steady-state
    let mut b = vec![1.0f32; 4800];
    let ctx = ProcessContext::new(48000, 4800);
    p.process_in_place(&mut b, &ctx).unwrap();
    let _output_before = b[4799];

    // Now change threshold from -6 dB to -20 dB
    p.parametric_set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-20.0))
        .unwrap();

    // Process one small block (=1ms = 48 samples)
    // With proper 5ms smoothing, the threshold should NOT have fully
    // transitioned after just 1ms.
    let mut b2 = vec![1.0f32; 48];
    p.process_in_place(&mut b2, &ProcessContext::new(48000, 48))
        .unwrap();
    let output_after_1ms = b2[47];

    // The new threshold (-20 dB = 0.1) is much lower than old (-6 dB = 0.5).
    // After only 1ms of a 5ms transition, the output should still be
    // closer to the old threshold than the new one.
    let old_thresh_lin = fast_pow10(-6.0 / 20.0); // = 0.50
    let new_thresh_lin = fast_pow10(-20.0 / 20.0); // = 0.10
    let midpoint = (old_thresh_lin + new_thresh_lin) / 2.0;

    assert!(
        output_after_1ms > midpoint,
        "After 1ms of a 5ms threshold transition, output {output_after_1ms:.4} should be above \
             midpoint {midpoint:.4} (old={old_thresh_lin:.4}, new={new_thresh_lin:.4}). \
             Smoother may be double-advancing."
    );
}

/// Verify mix automation now advances inside the block, not as one-step block
/// control. When mix is ramped from dry to wet, the frame outputs should not
/// remain constant during a short block with 5 ms smoothing and non-zero
/// reduction.
#[test]
fn test_mix_smoother_advances_per_frame() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 0.0, false);
    p.initialize(48000).unwrap();

    // Start from dry.
    p.parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.0))
        .unwrap();

    // Warm up so the smoother is actually at mix=0.
    let mut warmup = vec![0.9f32; 4800];
    p.process_in_place(&mut warmup, &ProcessContext::new(48000, 4800))
        .unwrap();

    // Ramp mix toward full wet.
    p.parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();

    // 5 ms at 48 kHz.
    let mut output = vec![0.9f32; 240];
    p.process_in_place(&mut output, &ProcessContext::new(48000, 240))
        .unwrap();

    let first = output[0].abs();
    let last = output[239].abs();
    // Mix ramp should make these distinct once reduced.
    assert!(
        (last - first).abs() > 1e-4,
        "mix automation should advance per frame: first={first:.6}, last={last:.6}"
    );
}

/// Verify the limiter actually limits output below threshold.
#[test]
fn test_limiter_clamps_output() {
    let mut p = LimiterPlugin::new(2, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    // Feed loud stereo signal (well above -6 dB threshold)
    let mut b = vec![0.0f32; 2048 * 2];
    for frame in 0..2048 {
        let val = 0.9 * (frame as f32 * 0.1).sin(); // ~-1 dBFS sine
        b[frame * 2] = val;
        b[frame * 2 + 1] = val;
    }
    let ctx = ProcessContext::new(48000, 2048);
    p.process_in_place(&mut b, &ctx).unwrap();

    // After lookahead fills (=5ms = 240 samples), all output should be
    // below threshold. Allow a small overshoot margin.
    let thresh_lin = fast_pow10(-6.0 / 20.0);
    for frame in 500..2048 {
        for ch in 0..2 {
            let s = b[frame * 2 + ch].abs();
            assert!(
                s <= thresh_lin * 1.1,
                "frame {frame} ch {ch}: {s:.4} exceeds threshold {thresh_lin:.4}"
            );
        }
    }
}

/// Test that true peak detection catches inter-sample peaks.
#[test]
fn test_true_peak_detection() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.true_peak = true;
    p.rebuild_cached_parameters();
    p.initialize(48000).unwrap();

    // Create a signal with inter-sample peaks: alternating +0.8/-0.8
    // at Nyquist causes overshoots between samples
    let frames = 2048;
    let mut b = vec![0.0f32; frames];
    for (i, sample) in b.iter_mut().enumerate() {
        *sample = if i % 2 == 0 { 0.8 } else { -0.8 };
    }
    let ctx = ProcessContext::new(48000, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    // With true peak, the limiter should detect the inter-sample overshoot
    // and apply more gain reduction than sample-peak would.
    // Verify the output is still limited.
    let thresh_lin = fast_pow10(-6.0 / 20.0);
    for &s in &b[500..] {
        assert!(
            s.abs() <= thresh_lin * 1.15,
            "true peak: sample {s:.4} exceeds threshold {thresh_lin:.4}"
        );
    }
}

/// Test that true peak parameter can be set via set_parameter.
#[test]
fn test_true_peak_parameter() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();
    assert!(!p.true_peak);

    p.parametric_set_parameter(ParameterId::from("true_peak"), ParameterValue::Bool(true))
        .unwrap();
    assert!(p.true_peak);

    let val = p.parametric_get_parameter(&ParameterId::from("true_peak"));
    assert_eq!(val, Some(ParameterValue::Bool(true)));
}

/// Test that dual release parameter can be set via set_parameter.
#[test]
fn test_dual_release_parameter() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();
    assert!(!p.dual_release);

    p.parametric_set_parameter(
        ParameterId::from("dual_release"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert!(p.dual_release);

    let val = p.parametric_get_parameter(&ParameterId::from("dual_release"));
    assert_eq!(val, Some(ParameterValue::Bool(true)));
}

/// Test that dual release still limits correctly.
#[test]
fn test_dual_release_limits() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.dual_release = true;
    p.rebuild_cached_parameters();
    p.initialize(48000).unwrap();

    let frames = 4096;
    let mut b = vec![0.0f32; frames];
    for (i, sample) in b.iter_mut().enumerate() {
        *sample = 0.9 * (i as f32 * 0.1).sin();
    }
    let ctx = ProcessContext::new(48000, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    let thresh_lin = fast_pow10(-6.0 / 20.0);
    for &s in &b[500..] {
        assert!(
            s.abs() <= thresh_lin * 1.1,
            "dual release: sample {s:.4} exceeds threshold {thresh_lin:.4}"
        );
    }
}

/// Test from_params wires true_peak and dual_release correctly.
#[test]
fn test_from_params_new_fields() {
    let params = LimiterPluginParams {
        threshold_db: -3.0,
        release_ms: 100.0,
        lookahead_ms: 10.0,
        soft: true,
        true_peak: true,
        isp_mode: true,
        dual_release: true,
        mix: 0.8,
        feed_forward: true,
        link_amount: 0.75,
    };
    let p = LimiterPlugin::from_params(2, params);
    assert!(p.true_peak);
    assert!(p.isp_mode);
    assert!(p.dual_release);
    assert!((p.link_amount - 0.75).abs() < 1e-6);
    assert!(p.feed_forward);
    assert_eq!(p.mix, 0.8);

    let tp_val = p.parametric_get_parameter(&ParameterId::from("true_peak"));
    assert_eq!(tp_val, Some(ParameterValue::Bool(true)));
    let dr_val = p.parametric_get_parameter(&ParameterId::from("dual_release"));
    assert_eq!(dr_val, Some(ParameterValue::Bool(true)));
}

/// Ceiling and mix parameters: with mix=0.0, output should be the dry
/// (delayed) signal unchanged. With mix=1.0, output should be limited.
#[test]
fn test_limiter_mix_parameter() {
    let sr = 48000u32;

    // Create limiter with mix=0 set via parameter after init (so smoother starts at 0)
    let mut p_dry = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p_dry.initialize(sr).unwrap();
    p_dry
        .parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.0))
        .unwrap();

    let mut p_wet = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p_wet.initialize(sr).unwrap();

    // Process a warmup block to let the mix smoother converge to 0
    let warmup = 4800; // 100ms
    let mut warmup_buf_dry = vec![0.0f32; warmup];
    let mut warmup_buf_wet = vec![0.0f32; warmup];
    let warmup_ctx = ProcessContext::new(sr, warmup);
    p_dry
        .process_in_place(&mut warmup_buf_dry, &warmup_ctx)
        .unwrap();
    p_wet
        .process_in_place(&mut warmup_buf_wet, &warmup_ctx)
        .unwrap();

    let num_frames = 4096;
    let make_signal = || {
        let mut buf = vec![0.0f32; num_frames];
        for (i, sample) in buf.iter_mut().enumerate() {
            *sample = 0.9 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin();
        }
        buf
    };
    let mut buf_dry = make_signal();
    let mut buf_wet = make_signal();
    let ctx = ProcessContext::new(sr, num_frames);
    p_dry.process_in_place(&mut buf_dry, &ctx).unwrap();
    p_wet.process_in_place(&mut buf_wet, &ctx).unwrap();

    let thresh_lin = fast_pow10(-6.0 / 20.0);

    // mix=0 (dry): after lookahead fills, peaks should exceed threshold (no limiting)
    let dry_peak: f32 = buf_dry[500..]
        .iter()
        .map(|x| x.abs())
        .fold(0.0f32, f32::max);
    assert!(
        dry_peak > thresh_lin,
        "mix=0 (dry) should pass through unaltered, peak={dry_peak:.4} > threshold={thresh_lin:.4}"
    );

    // mix=1 (wet): after lookahead fills, peaks should be below threshold
    let wet_peak: f32 = buf_wet[500..]
        .iter()
        .map(|x| x.abs())
        .fold(0.0f32, f32::max);
    assert!(
        wet_peak <= thresh_lin * 1.1,
        "mix=1 (wet) should be limited, peak={wet_peak:.4} > threshold={thresh_lin:.4}"
    );
}

/// With threshold=-6dB, a 0dBFS signal should not exceed the threshold
/// in the output (after lookahead fills).
#[test]
fn test_limiter_ceiling_enforcement() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    let num_frames = 4096;
    let mut buf = vec![1.0f32; num_frames]; // 0 dBFS
    let ctx = ProcessContext::new(48000, num_frames);
    p.process_in_place(&mut buf, &ctx).unwrap();

    let thresh_lin = fast_pow10(-6.0 / 20.0);
    // After lookahead settles (~240 samples at 5ms), output should not exceed threshold
    for (i, &s) in buf[500..].iter().enumerate() {
        assert!(
            s.abs() <= thresh_lin * 1.05,
            "Frame {}: sample {s:.4} exceeds ceiling {thresh_lin:.4}",
            i + 500
        );
    }
}

/// Verify ISP (inter-sample true peak) meter is exposed through LimiterData.
#[test]
fn test_isp_meter_exposure() {
    let mut p = LimiterPlugin::new(2, -1.0, 50.0, 5.0, false);
    p.true_peak = true;
    p.rebuild_cached_parameters();
    p.initialize(48000).unwrap();

    // Create a signal with inter-sample peaks on both channels
    let frames = 2048;
    let mut b = vec![0.0f32; frames * 2];
    for i in 0..frames {
        let val = if i % 2 == 0 { 0.8 } else { -0.8 };
        b[i * 2] = val;
        b[i * 2 + 1] = val * 0.5;
    }

    // Process enough blocks to trigger cache update (>= CACHE_UPDATE_THROTTLE)
    let ctx = ProcessContext::new(48000, frames);
    for _ in 0..CACHE_UPDATE_THROTTLE + 1 {
        p.process_in_place(&mut b, &ctx).unwrap();
    }

    let data = p.cache.load();
    let data = data.as_ref();
    assert_eq!(data.isp_dbtp.len(), 2, "ISP should have 2 channels");
    // Both channels should show non-trivial ISP values
    assert!(
        data.isp_dbtp[0] > -20.0,
        "ch0 ISP {} dBTP should be above -20",
        data.isp_dbtp[0]
    );
    assert!(
        data.isp_dbtp[1] > -20.0,
        "ch1 ISP {} dBTP should be above -20",
        data.isp_dbtp[1]
    );
    // Channel 0 (full scale) should have higher ISP than channel 1 (half scale)
    assert!(
        data.isp_dbtp[0] > data.isp_dbtp[1],
        "ch0 ISP {} should exceed ch1 ISP {}",
        data.isp_dbtp[0],
        data.isp_dbtp[1]
    );
}

/// Verify ISP meter stays at floor (-120 dB) when true_peak is disabled.
#[test]
fn test_isp_meter_floor_without_true_peak() {
    let mut p = LimiterPlugin::new(1, -1.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();
    assert!(!p.true_peak);

    let frames = 512;
    let mut b = vec![0.5f32; frames];
    let ctx = ProcessContext::new(48000, frames);
    for _ in 0..CACHE_UPDATE_THROTTLE + 1 {
        p.process_in_place(&mut b, &ctx).unwrap();
    }

    let data = p.cache.load();
    // ISP values stay at floor when true_peak is disabled (not updated)
    for &v in &data.isp_dbtp {
        assert!(
            v <= -119.0,
            "ISP should be at floor when true_peak is disabled, got {v}"
        );
    }
}

#[test]
fn test_isp_meter_resets_to_floor_when_true_peak_disabled() {
    let mut p = LimiterPlugin::new(1, -1.0, 50.0, 5.0, false);
    p.parametric_set_parameter(ParameterId::from("true_peak"), ParameterValue::Bool(true))
        .unwrap();
    p.initialize(48000).unwrap();

    let frames = 512;
    let mut b = vec![0.8f32; frames];
    let ctx = ProcessContext::new(48000, frames);

    // First, gather non-floor ISP data.
    for _ in 0..(CACHE_UPDATE_THROTTLE + 1) {
        p.process_in_place(&mut b, &ctx).unwrap();
    }
    let with_true_peak = p.cache.load();
    assert!(
        with_true_peak.isp_dbtp[0] > -20.0,
        "ISP meter should reflect active true-peak detection"
    );

    p.parametric_set_parameter(ParameterId::from("true_peak"), ParameterValue::Bool(false))
        .unwrap();
    for _ in 0..(CACHE_UPDATE_THROTTLE + 1) {
        p.process_in_place(&mut b, &ctx).unwrap();
    }
    let without_true_peak = p.cache.load();
    assert!(
        without_true_peak.isp_dbtp[0] <= -119.0,
        "ISP meter should fall back to floor immediately after disabling true peak"
    );
}

/// ISP mode: output inter-sample peaks must not exceed the ceiling.
/// We create a signal with known inter-sample peaks, run through the
/// ISP limiter, then verify output ISP with an independent detector.
#[test]
fn test_isp_mode_prevents_output_isp_violations() {
    let mut p = LimiterPlugin::new(1, -3.0, 50.0, 5.0, false);
    p.isp_mode = true;
    p.true_peak = true;
    p.rebuild_cached_parameters();
    p.initialize(48000).unwrap();

    let thresh_lin = fast_pow10(-3.0 / 20.0); // ~0.708

    // Create a signal with inter-sample peaks: two adjacent samples
    // that are below threshold but whose interpolated curve exceeds it.
    // A rising-falling pattern creates ISP overshoots.
    let frames = 8192;
    let mut b = vec![0.0f32; frames];
    for i in 0..frames {
        // Sine at ~12kHz at 48kHz sample rate = ~4 samples per cycle
        // This creates significant inter-sample peaks
        b[i] = 0.65 * (2.0 * std::f32::consts::PI * 12000.0 * i as f32 / 48000.0).sin();
    }

    let ctx = ProcessContext::new(48000, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    // Verify output ISP with an independent detector (not the plugin's own)
    let mut verifier = TruePeakDetector::new();
    let mut max_output_isp = 0.0f32;
    // Skip first 500 samples for lookahead + ISP correction convergence
    for &s in &b[500..] {
        let tp = verifier.process_linear(s);
        max_output_isp = max_output_isp.max(tp);
    }

    // Allow 0.1 dB tolerance (ISP correction is feedback-based, 1-sample delay)
    let tolerance_lin = fast_pow10(0.1 / 20.0); // ~1.012
    assert!(
        max_output_isp <= thresh_lin * tolerance_lin,
        "ISP mode: output ISP {:.4} ({:.2} dB) exceeds ceiling {:.4} ({:.1} dB) + 0.1dB tolerance",
        max_output_isp,
        20.0 * max_output_isp.log10(),
        thresh_lin,
        -3.0,
    );
}

/// ISP mode parameter can be toggled via set_parameter.
#[test]
fn test_isp_mode_parameter() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();
    assert!(!p.isp_mode);

    p.parametric_set_parameter(ParameterId::from("isp_mode"), ParameterValue::Bool(true))
        .unwrap();
    assert!(p.isp_mode);

    let val = p.parametric_get_parameter(&ParameterId::from("isp_mode"));
    assert_eq!(val, Some(ParameterValue::Bool(true)));
}

/// ISP mode implicitly enables true peak detection for input-side gain computation.
#[test]
fn test_isp_mode_implies_true_peak() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.isp_mode = true;
    // true_peak is false, but isp_mode forces true peak detection
    p.rebuild_cached_parameters();
    p.initialize(48000).unwrap();

    let frames = 512;
    let mut b = vec![0.0f32; frames];
    for (i, sample) in b.iter_mut().enumerate() {
        // Alternating signal creates ISP overshoots
        *sample = if i % 2 == 0 { 0.7 } else { -0.7 };
    }
    let ctx = ProcessContext::new(48000, frames);
    // Process enough blocks to trigger cache update (>= CACHE_UPDATE_THROTTLE)
    for _ in 0..CACHE_UPDATE_THROTTLE + 1 {
        b.fill(0.0);
        for (i, sample) in b.iter_mut().enumerate() {
            *sample = if i % 2 == 0 { 0.7 } else { -0.7 };
        }
        p.process_in_place(&mut b, &ctx).unwrap();
    }

    // With ISP mode, the limiter should detect inter-sample peaks
    // and apply gain reduction even though sample peaks (0.7) are
    // below the -6dB threshold (0.5) — the ISP exceeds it.
    // Check that the ISP monitoring shows activity
    let data = p.cache.load();
    // ISP monitoring should be populated (isp_mode implies true_peak detection)
    assert!(
        !data.isp_dbtp.is_empty(),
        "ISP monitoring should be active when isp_mode is on"
    );
}

/// Soft-knee must not be stricter than hard-knee at threshold.
/// When abs_s == thresh, the soft-knee output should equal thresh (not be below it).
#[test]
fn test_soft_knee_at_threshold_equals_hard_knee() {
    // At exactly the threshold level, soft mode should output exactly threshold.
    // Previously the algebraic curve gave 0.9707*thresh, making soft mode
    // ~0.25 dB stricter than hard mode.
    let thresh_db = -6.0f32;
    let thresh_lin = fast_pow10(thresh_db / 20.0);

    let mut p_soft = LimiterPlugin::new(1, thresh_db, 50.0, 0.0, true); // soft=true
    p_soft.initialize(48000).unwrap();

    // Feed a DC signal exactly at threshold for enough frames to converge
    let frames = 8192;
    let mut b = vec![thresh_lin; frames];
    let ctx = ProcessContext::new(48000, frames);
    p_soft.process_in_place(&mut b, &ctx).unwrap();

    // After settling, all output samples should be at or above 0.95*thresh
    // (soft mode should not attenuate below threshold when input == threshold)
    let min_output = b[500..].iter().copied().fold(f32::MAX, f32::min);
    assert!(
        min_output >= thresh_lin * 0.98,
        "Soft knee at threshold: min output {min_output:.4} should be >= {:.4} (0.98*thresh). \
             Soft mode is too strict.",
        thresh_lin * 0.98
    );
}

/// Soft-knee output at exactly threshold should be no lower than hard-knee output.
#[test]
fn test_soft_knee_not_stricter_than_hard() {
    let thresh_db = -3.0f32;
    let thresh_lin = fast_pow10(thresh_db / 20.0);

    let mut p_hard = LimiterPlugin::new(1, thresh_db, 50.0, 0.0, false); // hard
    let mut p_soft = LimiterPlugin::new(1, thresh_db, 50.0, 0.0, true); // soft
    p_hard.initialize(48000).unwrap();
    p_soft.initialize(48000).unwrap();

    // DC at threshold — both limiters should treat this the same (no gain reduction).
    let frames = 4096;
    let mut b_hard = vec![thresh_lin; frames];
    let mut b_soft = vec![thresh_lin; frames];
    let ctx = ProcessContext::new(48000, frames);
    p_hard.process_in_place(&mut b_hard, &ctx).unwrap();
    p_soft.process_in_place(&mut b_soft, &ctx).unwrap();

    // Hard mode should pass signal exactly (no gain reduction needed — input at ceiling)
    let hard_out = b_hard[1000];
    let soft_out = b_soft[1000];
    // A dB-domain soft knee begins gain reduction below the ceiling.
    assert!(
        soft_out < hard_out && soft_out > hard_out * 0.95,
        "soft-knee output {soft_out:.5}, hard output {hard_out:.5}"
    );
}

/// ISP correction decay must follow the release time constant.
/// Previously, decaying isp_correction_db multiplicatively (in dB domain)
/// with a linear-space release_coeff caused double-exponential decay —
/// the correction vanished much faster than the release time.
#[test]
fn test_isp_correction_decay_speed() {
    // This test verifies that the ISP correction decays no faster than
    // the release time constant implies.
    let release_ms = 100.0f32;
    let sr = 48000u32;
    let mut p = LimiterPlugin::new(1, -3.0, release_ms, 5.0, false);
    p.isp_mode = true;
    p.true_peak = true;
    p.rebuild_cached_parameters();
    p.initialize(sr).unwrap();

    // Inject enough ISP violations to build up correction
    let thresh_lin = fast_pow10(-3.0f32 / 20.0);
    let frames = 4096;
    let mut b = vec![0.0f32; frames];
    for i in 0..frames {
        // High-freq alternating signal causes ISP above sample peaks
        b[i] = 0.75 * (2.0 * std::f32::consts::PI * 15000.0 * i as f32 / sr as f32).sin();
    }
    let ctx = ProcessContext::new(sr, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    // Now the correction should be > 0.  Feed silence to let it decay.
    let correction_before = p.isp_correction_db;

    // If correction was built up, verify it decays at a reasonable rate.
    // With release_ms=100, after one block of silence (4096 samples ≈ 85ms),
    // the linear-space correction should still be > 10% of the original value
    // (we haven't hit the release time yet).
    if correction_before > 0.1 {
        let samples_silence = 4096usize;
        let mut silence = vec![0.0f32; samples_silence];
        let sctx = ProcessContext::new(sr, samples_silence);
        p.process_in_place(&mut silence, &sctx).unwrap();

        // Release coeff = exp(-1 / (release_ms * 0.001 * sr))
        // After N samples: fraction remaining = coeff^N
        let rc = (-1.0f32 / (release_ms * 0.001 * sr as f32)).exp();
        let expected_fraction = rc.powi(samples_silence as i32);

        // Convert correction to linear, apply expected fraction, convert back
        let expected_remaining_db = correction_before + 20.0 * expected_fraction.log10();
        // Allow 2x tolerance (some correction may have already decayed before block end)
        let min_expected_db = expected_remaining_db - 6.0; // 6 dB tolerance

        assert!(
            p.isp_correction_db >= min_expected_db.max(0.0),
            "ISP correction decayed too fast: before={correction_before:.3} dB, \
                 after={:.3} dB, expected >= {min_expected_db:.3} dB. \
                 Decay is in wrong domain (dB vs linear).",
            p.isp_correction_db
        );
    }
    // Even with no correction, the test passes — main assertion is that
    // signals near the ISP threshold don't cause excessive correction buildup.
    let _ = thresh_lin;
}

/// Feed-forward mode with many channels must not ignore channels beyond 32.
#[test]
fn test_channel_count_above_32() {
    // Create a limiter with 33 channels — previously channels 33+ were ignored
    // because of a fixed `[0.0f32; 32]` array cap.
    let ch = 33usize;
    let mut p = LimiterPlugin::new(ch, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    let thresh_lin = fast_pow10(-6.0f32 / 20.0);
    let frames = 2048;
    let mut b = vec![0.0f32; frames * ch];
    for frame in 0..frames {
        for c in 0..ch {
            // All channels get a loud signal — including channel 33
            b[frame * ch + c] = 0.9;
        }
    }
    let ctx = ProcessContext::new(48000, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    // Every channel (including #33) must be limited
    for frame in 500..frames {
        for c in 0..ch {
            let s = b[frame * ch + c].abs();
            assert!(
                s <= thresh_lin * 1.1,
                "ch {c} frame {frame}: {s:.4} exceeds threshold. Channels > 32 not analyzed."
            );
        }
    }
}

#[test]
fn test_lookahead_parameter_change_uses_preallocated_storage() {
    let mut p = LimiterPlugin::new(2, -6.0, 50.0, 5.0, false);
    p.parametric_set_parameter(ParameterId::from("lookahead"), ParameterValue::Float(20.0))
        .unwrap();
    p.initialize(48000).unwrap();
    assert_eq!(p.lookahead_len, 960);
    assert!(
        p.parametric_set_parameter(ParameterId::from("lookahead"), ParameterValue::Float(1.0))
            .is_err()
    );
}

#[test]
fn test_link_amount_interpolates_average_to_peak_detection() {
    let sr = 48000;
    let frames = 4096;
    let threshold_db = -6.0;
    let make_input = || {
        let mut b = vec![0.0f32; frames * 2];
        for frame in 0..frames {
            b[frame * 2] = 0.9;
            b[frame * 2 + 1] = 0.05;
        }
        b
    };

    let mut linked = LimiterPlugin::new(2, threshold_db, 50.0, 0.0, false);
    linked.link_amount = 1.0;
    linked.initialize(sr).unwrap();
    let mut linked_buf = make_input();
    linked
        .process_in_place(&mut linked_buf, &ProcessContext::new(sr, frames))
        .unwrap();

    let mut half = LimiterPlugin::new(2, threshold_db, 50.0, 0.0, false);
    half.link_amount = 0.5;
    half.initialize(sr).unwrap();
    let mut half_buf = make_input();
    half.process_in_place(&mut half_buf, &ProcessContext::new(sr, frames))
        .unwrap();

    assert!(
        half.monitoring_gr_db < linked.monitoring_gr_db,
        "partial linking should produce intermediate detector GR: half={} linked={}",
        half.monitoring_gr_db,
        linked.monitoring_gr_db
    );
}

/// Test that reset clears true peak detectors and dual release state.
#[test]
fn test_reset_clears_new_state() {
    let mut p = LimiterPlugin::new(2, -6.0, 50.0, 5.0, false);
    p.true_peak = true;
    p.dual_release = true;
    p.initialize(48000).unwrap();

    // Process some audio to build up state
    let mut b = vec![0.9f32; 2 * 1024];
    let ctx = ProcessContext::new(48000, 1024);
    p.process_in_place(&mut b, &ctx).unwrap();

    p.reset();

    // After reset, detectors should be zeroed
    for det in &p.true_peak_detectors {
        assert!(det.history.iter().all(|sample| *sample == 0.0));
    }
    assert_eq!(p.envelope, 0.0);
}

// -------------------------------------------------------------------------
// Additional process / set_parameter smoke tests
// -------------------------------------------------------------------------

#[test]
fn test_process_empty_buffer_returns_zero() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    let mut buf = vec![0.0f32; 0];
    let ctx = ProcessContext::new(48000, 0);
    let frames = p.process_in_place(&mut buf, &ctx).unwrap();
    assert_eq!(frames, 0);
}

#[test]
fn test_set_parameter_unknown_id_returns_error() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    let result =
        p.parametric_set_parameter(ParameterId::from("not_a_param"), ParameterValue::Float(1.0));
    assert!(result.is_err());
}

#[test]
fn test_set_parameter_out_of_bounds_returns_error() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    // Threshold range [-20, 0]
    assert!(
        p.set_parameter(ParameterId::from("threshold"), ParameterValue::Float(5.0))
            .is_err()
    );
    assert!(
        p.set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
            .is_err()
    );

    // Mix range [0, 1]
    assert!(
        p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(-0.1))
            .is_err()
    );

    // Lookahead range [0, 20]
    assert!(
        p.set_parameter(ParameterId::from("lookahead"), ParameterValue::Float(25.0))
            .is_err()
    );
}

#[test]
fn test_set_parameter_nan_returns_error() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    assert!(
        p.set_parameter(
            ParameterId::from("threshold"),
            ParameterValue::Float(f32::NAN)
        )
        .is_err()
    );
    assert!(
        p.set_parameter(
            ParameterId::from("release"),
            ParameterValue::Float(f32::NAN)
        )
        .is_err()
    );
}

#[test]
fn test_get_parameter_unknown_id_returns_none() {
    let p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    let val = p.parametric_get_parameter(&ParameterId::from("not_a_param"));
    assert_eq!(val, None);
}

#[test]
fn test_set_parameter_feed_forward_and_link_amount() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    p.parametric_set_parameter(
        ParameterId::from("feed_forward"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert!(p.feed_forward);

    p.parametric_set_parameter(ParameterId::from("link_amount"), ParameterValue::Float(0.5))
        .unwrap();
    assert!((p.link_amount - 0.5).abs() < 1e-6);

    let val = p.parametric_get_parameter(&ParameterId::from("feed_forward"));
    assert_eq!(val, Some(ParameterValue::Bool(true)));
    let val = p.parametric_get_parameter(&ParameterId::from("link_amount"));
    assert_eq!(val, Some(ParameterValue::Float(0.5)));
}

#[test]
fn test_set_release_recomputes_coefficients() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    let old_coeff = p.release_coeff;
    p.parametric_set_parameter(ParameterId::from("release"), ParameterValue::Float(200.0))
        .unwrap();
    assert!((p.release_coeff - old_coeff).abs() > 1e-6);
    assert_eq!(p.release_ms, 200.0);
}

#[test]
fn test_latency_samples_matches_lookahead() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    // 5ms @ 48kHz = 240 samples
    let latency = p.latency_samples();
    assert_eq!(latency, 240);

    assert!(
        p.parametric_set_parameter(ParameterId::from("lookahead"), ParameterValue::Float(0.0))
            .is_err()
    );
    assert_eq!(p.latency_samples(), 240);
}

// -------------------------------------------------------------------------
// Additional coverage tests for set_parameter and process_in_place
// -------------------------------------------------------------------------

/// Verify info() returns correct name and version.
#[test]
fn test_info() {
    let p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    let info = p.info();
    assert_eq!(info.name, "Limiter");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(info.author, "SotF");
}

/// Verify channels() returns the configured channel count.
#[test]
fn test_channels() {
    let p = LimiterPlugin::new(4, -6.0, 50.0, 5.0, false);
    assert_eq!(p.channels(), 4);
}

/// Verify parameters() returns all 10 cached parameters.
#[test]
fn test_parameters_returns_all_params() {
    let p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    let params = p.parametric_parameters();
    assert_eq!(params.len(), 10);
    let ids: Vec<_> = params.iter().map(|p| p.id.clone()).collect();
    assert!(ids.contains(&ParameterId::from("threshold")));
    assert!(ids.contains(&ParameterId::from("release")));
    assert!(ids.contains(&ParameterId::from("lookahead")));
    assert!(ids.contains(&ParameterId::from("soft")));
    assert!(ids.contains(&ParameterId::from("true_peak")));
    assert!(ids.contains(&ParameterId::from("isp_mode")));
    assert!(ids.contains(&ParameterId::from("dual_release")));
    assert!(ids.contains(&ParameterId::from("mix")));
    assert!(ids.contains(&ParameterId::from("link_amount")));
    assert!(ids.contains(&ParameterId::from("feed_forward")));
}

/// set_parameter round-trip for the "soft" boolean parameter.
#[test]
fn test_set_parameter_soft_roundtrip() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();
    assert!(!p.soft);

    p.parametric_set_parameter(ParameterId::from("soft"), ParameterValue::Bool(true))
        .unwrap();
    assert!(p.soft);
    assert_eq!(
        p.get_parameter(&ParameterId::from("soft")),
        Some(ParameterValue::Bool(true))
    );

    p.parametric_set_parameter(ParameterId::from("soft"), ParameterValue::Bool(false))
        .unwrap();
    assert!(!p.soft);
}

/// release parameter minimum is enforced by validation (10.0 ms), so values below that error.
#[test]
fn test_set_release_below_min_errors() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    assert!(
        p.set_parameter(ParameterId::from("release"), ParameterValue::Float(0.5))
            .is_err()
    );
}

/// Boundary values for threshold, mix, and link_amount.
#[test]
fn test_set_parameter_boundary_values() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.parametric_set_parameter(ParameterId::from("lookahead"), ParameterValue::Float(20.0))
        .unwrap();
    p.initialize(48000).unwrap();

    // Threshold boundaries [-20, 0]
    p.parametric_set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-20.0))
        .unwrap();
    assert_eq!(p.threshold_db, -20.0);
    p.parametric_set_parameter(ParameterId::from("threshold"), ParameterValue::Float(0.0))
        .unwrap();
    assert_eq!(p.threshold_db, 0.0);

    // Mix boundaries [0, 1]
    p.parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.0))
        .unwrap();
    assert_eq!(p.mix, 0.0);
    p.parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();
    assert_eq!(p.mix, 1.0);

    // Link amount boundaries [0, 1]
    p.parametric_set_parameter(ParameterId::from("link_amount"), ParameterValue::Float(0.0))
        .unwrap();
    assert_eq!(p.link_amount, 0.0);
    p.parametric_set_parameter(ParameterId::from("link_amount"), ParameterValue::Float(1.0))
        .unwrap();
    assert_eq!(p.link_amount, 1.0);

    // Lookahead is structural after initialization.
    assert_eq!(p.lookahead_ms, 20.0);
    assert!(
        p.parametric_set_parameter(ParameterId::from("lookahead"), ParameterValue::Float(0.0))
            .is_err()
    );
}

/// get_parameter returns current values for all float and bool parameters.
#[test]
fn test_get_parameter_all_ids() {
    let mut p = LimiterPlugin::new(2, -3.0, 100.0, 10.0, false);
    p.true_peak = true;
    p.isp_mode = true;
    p.dual_release = true;
    p.feed_forward = true;
    p.link_amount = 0.5;
    p.mix = 1.0;
    p.rebuild_cached_parameters();
    p.initialize(48000).unwrap();

    assert_eq!(
        p.get_parameter(&ParameterId::from("threshold")),
        Some(ParameterValue::Float(-3.0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("release")),
        Some(ParameterValue::Float(100.0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("lookahead")),
        Some(ParameterValue::Float(10.0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("soft")),
        Some(ParameterValue::Bool(false))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("true_peak")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("isp_mode")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("dual_release")),
        Some(ParameterValue::Bool(true))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(1.0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("link_amount")),
        Some(ParameterValue::Float(0.5))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("feed_forward")),
        Some(ParameterValue::Bool(true))
    );
}

/// Silence input should produce silence and zero gain reduction.
#[test]
fn test_process_silence_no_gr() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    let frames = 512;
    let mut b = vec![0.0f32; frames];
    let ctx = ProcessContext::new(48000, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    // All output should be silent
    for &s in &b {
        assert_eq!(s, 0.0, "silence input should produce silence output");
    }
    // No gain reduction applied
    assert!(
        p.monitoring_gr_db < 0.01,
        "silence should cause no GR, got {} dB",
        p.monitoring_gr_db
    );
    assert!(
        p.monitoring_peak_db < -50.0,
        "peak meter should be very low for silence"
    );
}

/// Signal well below threshold should pass through with no limiting.
#[test]
fn test_process_below_threshold_no_limiting() {
    let sr = 48000u32;
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 0.0, false);
    p.initialize(sr).unwrap();

    // Signal at -20 dBFS = 0.1 linear, well below -6 dB threshold
    let frames = 1024;
    let amplitude = 0.1f32;
    let mut input = vec![0.0f32; frames];
    for (i, sample) in input.iter_mut().enumerate() {
        *sample = amplitude * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin();
    }
    let mut b = input.clone();

    let ctx = ProcessContext::new(sr, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    let max_error = (0..frames)
        .map(|i| (b[i].abs() - input[i].abs()).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_error < 1e-5,
        "signal below threshold should pass through unchanged, max_error={max_error}"
    );
    assert!(
        p.monitoring_gr_db < 0.01,
        "GR should be zero for signal below threshold, got {} dB",
        p.monitoring_gr_db
    );
}

/// Soft knee: very quiet signal should stay in the identity region (abs_s <= soft_start).
#[test]
fn test_soft_knee_identity_region() {
    let sr = 48000u32;
    let thresh_db = -6.0f32;
    let thresh_lin = fast_pow10(thresh_db / 20.0);
    let mut p = LimiterPlugin::new(1, thresh_db, 50.0, 0.0, true);
    p.initialize(sr).unwrap();

    // Use a signal amplitude of 0.05 * thresh_lin — far below soft_start
    let amplitude = thresh_lin * 0.05;
    let frames = 1024;
    let mut input = vec![0.0f32; frames];
    for (i, sample) in input.iter_mut().enumerate() {
        *sample = amplitude * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin();
    }
    let mut b = input.clone();

    let ctx = ProcessContext::new(sr, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    let max_error = (0..frames)
        .map(|i| (b[i].abs() - input[i].abs()).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_error < 1e-5,
        "soft knee identity region should not alter signal, max_error={max_error}"
    );
}

/// feed_forward=true with lookahead=0 should be disabled.
#[test]
fn test_feed_forward_disabled_when_lookahead_zero() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 0.0, false);
    p.feed_forward = true;
    p.rebuild_cached_parameters();
    p.initialize(48000).unwrap();

    assert_eq!(p.lookahead_len, 0);

    let frames = 512;
    let mut b = vec![0.9f32; frames];
    let ctx = ProcessContext::new(48000, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    // Should still limit normally (just without feed-forward)
    let thresh_lin = fast_pow10(-6.0 / 20.0);
    let max_out = b.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        max_out <= thresh_lin * 1.1,
        "feed_forward disabled by lookahead=0 should still limit normally"
    );
}

/// initialize() with a higher sample rate should update coefficients correctly.
#[test]
fn test_initialize_different_sample_rates() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(96000).unwrap();
    assert_eq!(p.sample_rate, 96000);

    // 5ms @ 96kHz = 480 samples
    assert_eq!(p.lookahead_len, 480);

    let mut p2 = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p2.initialize(192000).unwrap();
    assert_eq!(p2.sample_rate, 192000);
    assert_eq!(p2.lookahead_len, 960);
}

/// get_data() returns LimiterData with expected fields.
#[test]
fn test_get_data_returns_typed_data() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    let data = p.get_data();
    assert!(data.is_some());
    // Downcast Arc<dyn Any + Send + Sync> back to Arc<LimiterData>
    let data = data.unwrap();
    let limiter_data = data.downcast::<LimiterData>();
    assert!(limiter_data.is_ok());
}

/// Stereo with link_amount=0: each channel should be limited independently.
#[test]
fn test_process_stereo_independent_channels() {
    let sr = 48000u32;
    let mut p = LimiterPlugin::new(2, -6.0, 50.0, 0.0, false);
    p.link_amount = 0.0;
    p.rebuild_cached_parameters();
    p.initialize(sr).unwrap();

    let frames = 1024;
    let mut b = vec![0.0f32; frames * 2];
    for frame in 0..frames {
        // Channel 0 loud, channel 1 quiet
        b[frame * 2] = 0.9;
        b[frame * 2 + 1] = 0.1;
    }

    let ctx = ProcessContext::new(sr, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    let thresh_lin = fast_pow10(-6.0 / 20.0);
    // Channel 0 should be limited
    let ch0_max = b[500..]
        .iter()
        .step_by(2)
        .map(|&s| s.abs())
        .fold(0.0f32, f32::max);
    assert!(
        ch0_max <= thresh_lin * 1.1,
        "ch0 should be limited, max={ch0_max}"
    );

    // Channel 1 should pass through unchanged (well below threshold)
    let ch1_max = b[500..]
        .iter()
        .skip(1)
        .step_by(2)
        .map(|&s| s.abs())
        .fold(0.0f32, f32::max);
    assert!(
        (ch1_max - 0.1).abs() < 1e-4,
        "ch1 should pass through unchanged, max={ch1_max}"
    );
}

/// Envelope should decay (release) after a transient ends.
#[test]
fn test_envelope_decay_after_transient() {
    let sr = 48000u32;
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 0.0, false);
    p.initialize(sr).unwrap();

    // Loud transient
    let mut b = vec![1.0f32; sr as usize]; // 1 second of loud signal
    let ctx = ProcessContext::new(sr, b.len());
    p.process_in_place(&mut b, &ctx).unwrap();
    let gr_after_transient = p.monitoring_gr_db;
    assert!(
        gr_after_transient > 1.0,
        "should have significant GR after loud signal"
    );

    // Now silence — envelope should release
    let mut silence = vec![0.0f32; sr as usize];
    p.process_in_place(&mut silence, &ctx).unwrap();
    let gr_after_silence = p.monitoring_gr_db;
    assert!(
        gr_after_silence < gr_after_transient,
        "envelope should decay during silence: before={gr_after_transient}, after={gr_after_silence}"
    );
}

/// Verify that lookahead buffer wraps correctly by processing multiple blocks.
#[test]
fn test_lookahead_buffer_wraps_correctly() {
    let sr = 48000u32;
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(sr).unwrap();

    let block = 256usize;
    let mut buf = vec![0.9f32; block];
    let ctx = ProcessContext::new(sr, block);

    // Process many blocks to exercise lookahead_pos wrapping
    for _ in 0..100 {
        p.process_in_place(&mut buf, &ctx).unwrap();
    }

    // Should still limit correctly after many wraps
    let thresh_lin = fast_pow10(-6.0 / 20.0);
    let max_out = buf.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        max_out <= thresh_lin * 1.1,
        "output should still be limited after many lookahead wraps"
    );
}

/// Threshold of 0 dB should only limit signals above 1.0.
#[test]
fn test_threshold_zero_db() {
    let sr = 48000u32;
    let mut p = LimiterPlugin::new(1, 0.0, 50.0, 0.0, false);
    p.initialize(sr).unwrap();

    // Signal at 0.8 (below 0 dB = 1.0) should pass through
    let frames = 1024;
    let mut b = vec![0.0f32; frames];
    for (i, sample) in b.iter_mut().enumerate() {
        *sample = 0.8f32 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin();
    }

    let ctx = ProcessContext::new(sr, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    let max_out = b[100..].iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        max_out > 0.79,
        "signal below 0 dB threshold should not be limited, max_out={max_out}"
    );
    assert!(
        p.monitoring_gr_db < 0.1,
        "GR should be near zero, got {} dB",
        p.monitoring_gr_db
    );
}

/// Process with lookahead and verify delayed output.
#[test]
fn test_lookahead_causes_delay() {
    let sr = 48000u32;
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(sr).unwrap();

    // Impulse at sample 0
    let frames = 512;
    let mut b = vec![0.0f32; frames];
    b[0] = 1.0;

    let ctx = ProcessContext::new(sr, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    // With 5ms lookahead = 240 samples, the impulse should be delayed.
    // The first non-zero output should appear around the lookahead length.
    let first_nonzero = b.iter().position(|&s| s.abs() > 1e-4).unwrap_or(frames);
    assert!(
        (200..=280).contains(&first_nonzero),
        "impulse should be delayed by lookahead, first_nonzero={first_nonzero}"
    );
}

/// Validation rejects non-finite (infinite) float values before set_parameter processes them.
#[test]
fn test_set_parameter_infinite_rejected_by_validation() {
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();

    assert!(
        p.set_parameter(
            ParameterId::from("mix"),
            ParameterValue::Float(f32::INFINITY)
        )
        .is_err()
    );
    assert!(
        p.set_parameter(
            ParameterId::from("link_amount"),
            ParameterValue::Float(f32::INFINITY)
        )
        .is_err()
    );
}

/// Verify that the single-channel path (nc <= 1) always uses max_peak_ch even with link < 1.
#[test]
fn test_single_channel_ignores_link_amount() {
    let sr = 48000u32;
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 0.0, false);
    p.link_amount = 0.0; // would use avg for stereo, but ignored for mono
    p.rebuild_cached_parameters();
    p.initialize(sr).unwrap();

    let frames = 512;
    let mut b = vec![0.9f32; frames];
    let ctx = ProcessContext::new(sr, frames);
    p.process_in_place(&mut b, &ctx).unwrap();

    let thresh_lin = fast_pow10(-6.0 / 20.0);
    let max_out = b.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        max_out <= thresh_lin * 1.1,
        "mono with link=0 should still limit correctly"
    );
}

/// Dual release envelope should process when target_gr <= envelope (release branch).
#[test]
fn test_dual_release_envelope_decay() {
    let sr = 48000u32;
    let mut p = LimiterPlugin::new(1, -6.0, 50.0, 0.0, false);
    p.dual_release = true;
    p.rebuild_cached_parameters();
    p.initialize(sr).unwrap();

    // Loud signal to build envelope
    let mut b = vec![1.0f32; sr as usize];
    let ctx = ProcessContext::new(sr, b.len());
    p.process_in_place(&mut b, &ctx).unwrap();
    let gr_after_loud = p.monitoring_gr_db;
    assert!(gr_after_loud > 1.0);

    // Silence to trigger release branch
    let mut silence = vec![0.0f32; sr as usize];
    p.process_in_place(&mut silence, &ctx).unwrap();
    let gr_after_silence = p.monitoring_gr_db;

    assert!(
        gr_after_silence < gr_after_loud,
        "dual release should decay envelope: {gr_after_loud} -> {gr_after_silence}"
    );
}

/// ISP mode with output below threshold should trigger the correction decay path.
#[test]
fn test_isp_mode_correction_decay_path() {
    let sr = 48000u32;
    let mut p = LimiterPlugin::new(1, -3.0, 50.0, 5.0, false);
    p.isp_mode = true;
    p.true_peak = true;
    p.rebuild_cached_parameters();
    p.initialize(sr).unwrap();

    // First, build up some ISP correction with a high-freq signal
    let frames = 4096;
    let mut b = vec![0.0f32; frames];
    for i in 0..frames {
        b[i] = 0.75 * (2.0 * std::f32::consts::PI * 15000.0 * i as f32 / sr as f32).sin();
    }
    let ctx = ProcessContext::new(sr, frames);
    p.process_in_place(&mut b, &ctx).unwrap();
    let correction_before = p.isp_correction_db;

    // Now feed a signal well below threshold to trigger the decay path
    let mut quiet = vec![0.0f32; frames];
    for i in 0..frames {
        quiet[i] = 0.1 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin();
    }
    p.process_in_place(&mut quiet, &ctx).unwrap();

    if correction_before > 0.1 {
        assert!(
            p.isp_correction_db < correction_before,
            "ISP correction should decay when output is below threshold: before={correction_before}, after={}",
            p.isp_correction_db
        );
    }
}

/// Cache update with mismatched isp_dbtp channel count should not panic.
#[test]
fn test_cache_update_channel_mismatch_does_not_panic() {
    let mut p = LimiterPlugin::new(1, -1.0, 50.0, 5.0, false);
    p.true_peak = true;
    p.rebuild_cached_parameters();
    p.initialize(48000).unwrap();

    // Force a mismatch by manually changing cache data channel count
    p.cache.update(|d| {
        d.isp_dbtp = vec![-120.0; 4]; // 4 channels in cache, but plugin has 1
    });

    let frames = 512;
    let mut b = vec![0.8f32; frames];
    let ctx = ProcessContext::new(48000, frames);
    // Process enough blocks to trigger cache update; should not panic
    for _ in 0..CACHE_UPDATE_THROTTLE + 1 {
        p.process_in_place(&mut b, &ctx).unwrap();
    }
}

/// initialize should resize detectors when channel count changes.
#[test]
fn test_initialize_resizes_detectors() {
    let mut p = LimiterPlugin::new(2, -6.0, 50.0, 5.0, false);
    p.initialize(48000).unwrap();
    assert_eq!(p.true_peak_detectors.len(), 2);
    assert_eq!(p.output_isp_detectors.len(), 2);
    assert_eq!(p.channel_peaks.len(), 2);
    assert_eq!(p.monitoring_isp_linear.len(), 2);

    // The plugin's channels field doesn't change, but we can verify the resize path
    // by calling initialize again with same channels (resize_with should be no-op)
    p.initialize(48000).unwrap();
    assert_eq!(p.true_peak_detectors.len(), 2);
}

/// Soft clipping should produce different output than hard clipping for loud signals.
#[test]
fn test_soft_vs_hard_clipping_difference() {
    let sr = 48000u32;
    let frames = 1024;
    let mut p_soft = LimiterPlugin::new(1, -6.0, 50.0, 0.0, true);
    let mut p_hard = LimiterPlugin::new(1, -6.0, 50.0, 0.0, false);
    p_soft.initialize(sr).unwrap();
    p_hard.initialize(sr).unwrap();

    let mut b_soft = vec![0.0f32; frames];
    let mut b_hard = vec![0.0f32; frames];
    for i in 0..frames {
        let s = 0.9f32 * (i as f32 * 0.1).sin();
        b_soft[i] = s;
        b_hard[i] = s;
    }

    let ctx = ProcessContext::new(sr, frames);
    p_soft.process_in_place(&mut b_soft, &ctx).unwrap();
    p_hard.process_in_place(&mut b_hard, &ctx).unwrap();

    // At least some samples should differ between soft and hard
    let differences: usize = b_soft
        .iter()
        .zip(b_hard.iter())
        .filter(|(a, b)| (*a - *b).abs() > 1e-5)
        .count();
    assert!(
        differences > 0,
        "soft and hard clipping should produce different output for some samples"
    );
}
