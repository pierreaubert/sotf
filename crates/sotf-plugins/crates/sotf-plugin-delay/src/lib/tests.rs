use super::allpass_state::AllpassState;
use super::delay_plugin::DelayPlugin;
use super::types::DelayPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;

#[test]
fn test_delay_basic() {
    let mut p = DelayPlugin::new(1, 10.0, 0.5, 0.5);
    p.initialize(48000).unwrap();
    let mut b = vec![1.0; 1000];
    p.process_in_place(&mut b, &ProcessContext::new(48000, 1000))
        .unwrap();
    assert!(b[999] != 1.0);
}

#[test]
fn test_lagrange4_exact_samples() {
    // When frac=0, Lagrange should return y_0 exactly
    let result = DelayPlugin::lagrange4(0.0, 1.0, 0.0, 0.0, 0.0);
    assert!((result - 1.0).abs() < 1e-6, "frac=0 should return y_0");

    // When frac=1, Lagrange should return y_1 exactly
    let result = DelayPlugin::lagrange4(0.0, 0.0, 1.0, 0.0, 1.0);
    assert!((result - 1.0).abs() < 1e-6, "frac=1 should return y_1");
}

#[test]
fn test_lagrange4_linear_signal() {
    // For a linear signal, any interpolation should be exact
    // y = [1, 2, 3, 4] at frac=0.5 should give 2.5
    let result = DelayPlugin::lagrange4(1.0, 2.0, 3.0, 4.0, 0.5);
    assert!(
        (result - 2.5).abs() < 1e-6,
        "Linear signal interpolation should be exact, got {}",
        result
    );
}

#[test]
fn test_lagrange4_quadratic_signal() {
    // For a quadratic signal y = x^2: at x=-1,0,1,2 => y=1,0,1,4
    // At x=0.5: y = 0.25
    let result = DelayPlugin::lagrange4(1.0, 0.0, 1.0, 4.0, 0.5);
    assert!(
        (result - 0.25).abs() < 1e-5,
        "Quadratic signal interpolation should be exact, got {}",
        result
    );
}

#[test]
fn test_lfo_modulation() {
    let mut p = DelayPlugin::new(1, 10.0, 0.0, 1.0);
    p.initialize(48000).unwrap();

    // Enable LFO
    p.set_parameter(ParameterId::from("lfo_rate_hz"), ParameterValue::Float(5.0))
        .unwrap();
    p.set_parameter(
        ParameterId::from("lfo_depth_ms"),
        ParameterValue::Float(2.0),
    )
    .unwrap();

    // Process an impulse and collect output
    let mut b = vec![0.0; 48000];
    b[0] = 1.0;
    p.process_in_place(&mut b, &ProcessContext::new(48000, 48000))
        .unwrap();

    // The delayed impulse should appear with time-varying position due to LFO
    // Find the peak in the output (after the initial impulse at sample 0)
    let delay_region_start = 300; // 10ms at 48kHz ~ 480 samples, look around there
    let delay_region_end = 700;
    let peak_val = b[delay_region_start..delay_region_end]
        .iter()
        .fold(0.0_f32, |a, &x| a.max(x.abs()));
    assert!(
        peak_val > 0.1,
        "Should have delayed signal in expected region"
    );
}

#[test]
fn test_effective_delay_samples_scales_depth_to_preserve_symmetry() {
    let mut p = DelayPlugin::new(1, 100.0, 0.0, 1.0);
    p.initialize(48000).unwrap();
    p.lfo_depth_ms = 10.0;
    p.delay_smoother.set_target(100.0 * 48.0);

    let base_delay = 100.0 * 48.0;
    let up = p.effective_delay_samples(base_delay, 1.0);
    let down = p.effective_delay_samples(base_delay, -1.0);
    let delta = up - base_delay;
    let neg_delta = down - base_delay;
    assert!(
        (delta.abs() - neg_delta.abs()).abs() < 1e-5,
        "LFO depth should scale symmetrically around base delay (up={up}, down={down})"
    );
    assert!(delta < 0.0 && neg_delta > 0.0 || delta > 0.0 && neg_delta < 0.0);
}

#[test]
fn test_effective_delay_samples_degenerate_near_min_delay() {
    let mut p = DelayPlugin::new(1, 0.0, 0.0, 1.0);
    p.initialize(48000).unwrap();
    p.lfo_depth_ms = 10.0;

    let base_delay = 1.0;
    assert!(
        p.effective_delay_samples(base_delay, 1.0).to_bits()
            == p.effective_delay_samples(base_delay, -1.0).to_bits(),
        "when near minimum delay, LFO depth should be reduced to avoid asymmetry"
    );
    assert_eq!(
        p.effective_delay_samples(base_delay, 1.0),
        base_delay,
        "degenerate lower-bound case should still satisfy interpolation guard"
    );
}

#[test]
fn test_allpass_feedback() {
    let mut p = DelayPlugin::new(1, 10.0, 0.5, 0.5);
    p.initialize(48000).unwrap();

    // Enable allpass feedback
    p.set_parameter(
        ParameterId::from("allpass_feedback"),
        ParameterValue::Bool(true),
    )
    .unwrap();

    let mut b = vec![0.0; 2000];
    b[0] = 1.0;
    p.process_in_place(&mut b, &ProcessContext::new(48000, 2000))
        .unwrap();

    // With feedback and allpass, we should see repeated taps with spectral coloring
    // Check that there is signal beyond the first delay tap
    let late_energy: f32 = b[960..2000].iter().map(|x| x * x).sum();
    assert!(
        late_energy > 1e-6,
        "Allpass feedback should produce signal in later taps"
    );
}

#[test]
fn test_allpass_state() {
    let mut ap = AllpassState::new(0.5);
    // Process a unit impulse
    let y0 = ap.process(1.0);
    let y1 = ap.process(0.0);
    let y2 = ap.process(0.0);

    // First-order allpass with coeff=0.5:
    // y[0] = 0.5*1 + 0 - 0.5*0 = 0.5
    assert!((y0 - 0.5).abs() < 1e-6, "y0={}", y0);
    // y[1] = 0.5*0 + 1 - 0.5*0.5 = 0.75
    assert!((y1 - 0.75).abs() < 1e-6, "y1={}", y1);
    // y[2] = 0.5*0 + 0 - 0.5*0.75 = -0.375
    assert!((y2 - (-0.375)).abs() < 1e-6, "y2={}", y2);
}

#[test]
fn test_delay_buffer_is_deinterleaved() {
    let mut p = DelayPlugin::new(2, 5.0, 0.0, 0.0);
    p.initialize(48_000).unwrap();
    p.reset();

    let mut buffer = vec![0.0f32; 2];
    buffer[0] = 1.23;
    buffer[1] = -0.77;

    p.process_in_place(&mut buffer, &ProcessContext::new(48_000, 1))
        .unwrap();

    // Deinterleaved layout stores channels in separate contiguous segments:
    // [ch0 samples..., ch1 samples..., ...].
    assert_eq!(p.buffer[0], 1.23);
    assert_eq!(p.buffer[p.max_samples], -0.77);
    assert_eq!(p.buffer[1], 0.0);
    assert_eq!(p.buffer[p.max_samples + 1], 0.0);
}

#[test]
fn test_from_params() {
    let params = DelayPluginParams {
        delay_ms: 50.0,
        feedback: 0.4,
        mix: 0.6,
        lfo_rate_hz: 3.0,
        lfo_depth_ms: 1.5,
        allpass_feedback: true,
        allpass_coeff: 0.5,
        channel_delays_ms: Vec::new(),
    };
    let p = DelayPlugin::from_params(2, params).unwrap();
    assert_eq!(p.delay_ms, 50.0);
    assert_eq!(p.lfo_rate_hz, 3.0);
    assert_eq!(p.lfo_depth_ms, 1.5);
    assert!(p.allpass_feedback);
    assert!(!p.is_per_channel());
}

#[test]
fn test_per_channel_construction() {
    let p = DelayPlugin::new_per_channel(vec![2.0, 5.0, 10.0]).unwrap();
    assert!(p.is_per_channel());
    assert_eq!(p.channels, 3);
    assert_eq!(p.channel_delays_ms, vec![2.0, 5.0, 10.0]);
}

#[test]
fn test_per_channel_from_params() {
    let params = DelayPluginParams {
        delay_ms: 100.0, // ignored when channel_delays_ms is non-empty
        feedback: 0.0,
        mix: 1.0,
        lfo_rate_hz: 0.0,
        lfo_depth_ms: 0.0,
        allpass_feedback: false,
        allpass_coeff: 0.5,
        channel_delays_ms: vec![1.0, 3.0, 7.0],
    };
    let p = DelayPlugin::from_params(3, params).unwrap();
    assert!(p.is_per_channel());
    assert_eq!(p.channel_delays_ms, vec![1.0, 3.0, 7.0]);
}

#[test]
fn test_per_channel_from_params_rejects_channels_mismatch() {
    let params = DelayPluginParams {
        delay_ms: 100.0,
        feedback: 0.0,
        mix: 1.0,
        lfo_rate_hz: 0.0,
        lfo_depth_ms: 0.0,
        allpass_feedback: false,
        allpass_coeff: 0.5,
        channel_delays_ms: vec![1.0, 3.0, 7.0],
    };
    // 3 per-channel delays but channels arg = 2: hard error.
    assert!(DelayPlugin::from_params(2, params).is_err());
}

#[test]
fn test_per_channel_delays_independent() {
    // Each channel should produce its delayed impulse at the right time.
    let sr = 48000u32;
    let delays_ms = vec![5.0, 10.0]; // 240 and 480 samples at 48kHz
    let mut p = DelayPlugin::new_per_channel(delays_ms.clone()).unwrap();
    // Per-channel mode constructor defaults to mix=1.0, feedback=0.
    p.initialize(sr).unwrap();
    // Snap smoothers to target so the impulse is delayed by the exact
    // configured amount instead of seeing the 50 ms smoother ramp.
    p.reset();

    let channels = 2;
    let num_frames = 1024;
    let mut buf = vec![0.0f32; num_frames * channels];
    // Interleaved impulse on both channels at frame 0
    buf[0] = 1.0;
    buf[1] = 1.0;

    p.process_in_place(&mut buf, &ProcessContext::new(sr, num_frames))
        .unwrap();

    // Skip frame 0 (carries the dry input contribution); find the peak in
    // each channel's tail. With smoothers snapped to target, the delayed
    // impulse should land within a handful of samples of the expected
    // position (Lagrange interpolation + integer floor introduce <2 sample
    // wiggle).
    let peak_ch0 = (10..num_frames)
        .max_by(|&a, &b| {
            buf[a * channels]
                .abs()
                .partial_cmp(&buf[b * channels].abs())
                .unwrap()
        })
        .unwrap();
    let peak_ch1 = (10..num_frames)
        .max_by(|&a, &b| {
            buf[a * channels + 1]
                .abs()
                .partial_cmp(&buf[b * channels + 1].abs())
                .unwrap()
        })
        .unwrap();

    assert!(
        (peak_ch0 as i32 - 240).abs() <= 2,
        "channel 0 delay peak at {peak_ch0}, expected near 240"
    );
    assert!(
        (peak_ch1 as i32 - 480).abs() <= 2,
        "channel 1 delay peak at {peak_ch1}, expected near 480"
    );
}

#[test]
fn test_parameter_getset() {
    let mut p = DelayPlugin::new(1, 100.0, 0.3, 0.5);
    p.initialize(48000).unwrap();

    // Set and get lfo_rate_hz
    p.set_parameter(ParameterId::from("lfo_rate_hz"), ParameterValue::Float(7.5))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("lfo_rate_hz")),
        Some(ParameterValue::Float(7.5))
    );

    // Set and get lfo_depth_ms
    p.set_parameter(
        ParameterId::from("lfo_depth_ms"),
        ParameterValue::Float(3.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("lfo_depth_ms")),
        Some(ParameterValue::Float(3.0))
    );

    // Set and get allpass_feedback
    p.set_parameter(
        ParameterId::from("allpass_feedback"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("allpass_feedback")),
        Some(ParameterValue::Bool(true))
    );
}

#[test]
fn test_mix_zero_equals_dry() {
    // mix=0.0 -> output equals input (dry only, no delayed signal)
    let mut p = DelayPlugin::new(1, 10.0, 0.0, 0.0); // mix=0
    p.initialize(48000).unwrap();

    let num_frames = 1000;
    let original: Vec<f32> = (0..num_frames).map(|i| (i as f32 * 0.1).sin()).collect();
    let mut buffer = original.clone();
    p.process_in_place(&mut buffer, &ProcessContext::new(48000, num_frames))
        .unwrap();

    // With mix=0, output = input * (1-0) + delayed * 0 = input
    for (i, (&out, &inp)) in buffer.iter().zip(original.iter()).enumerate() {
        assert!(
            (out - inp).abs() < 1e-6,
            "mix=0 should equal dry input at frame {}: out={}, in={}",
            i,
            out,
            inp
        );
    }
}

#[test]
fn test_mix_one_equals_delayed() {
    // mix=1.0 -> output equals delayed signal only (no dry signal)
    let sr = 48000;
    let delay_ms = 10.0;
    let delay_samples = (delay_ms / 1000.0 * sr as f32).round() as usize;
    let mut p = DelayPlugin::new(1, delay_ms, 0.0, 1.0); // mix=1, feedback=0
    p.initialize(sr).unwrap();

    // Create an impulse
    let num_frames = delay_samples + 200;
    let mut buffer = vec![0.0f32; num_frames];
    buffer[0] = 1.0; // impulse

    p.process_in_place(&mut buffer, &ProcessContext::new(sr, num_frames))
        .unwrap();

    // With mix=1 and feedback=0:
    // output = input * (1-1) + delayed * 1 = delayed only
    // Frame 0: no delay history, so delayed=0, output=0 (not the impulse!)
    assert!(
        buffer[0].abs() < 0.01,
        "mix=1 frame 0 should be ~0 (delayed only), got {}",
        buffer[0]
    );

    // The impulse should appear at the delay offset
    // Find the peak in output (should be at delay_samples)
    let peak_idx = buffer
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
        .unwrap()
        .0;
    assert!(
        (peak_idx as i32 - delay_samples as i32).unsigned_abs() <= 1,
        "mix=1 peak should be at delay offset {}, found at {}",
        delay_samples,
        peak_idx
    );
}

/// Verify that the mix smoother ramps per-sample rather than jumping block-constant.
///
/// With block-constant smoothing (the old bug), every sample in a block gets
/// the same final-step value, so there is no ramp visible within the block.
/// With per-sample smoothing, output is monotonically decreasing (output=1-mix,
/// mix increasing 0→1) within the block when the target just changed from 0 → 1.
#[test]
fn test_mix_smoother_per_sample_ramp() {
    // Setup: mix=0 initially, feedback=0, delay > block size so delayed=0 during block.
    // Input is a ramp signal so dry ≠ wet and we can observe the mix ramp.
    // Delay is 200ms (9600 samples) >> 64 frames, so the delay buffer only has
    // silence during the first block → delayed = 0.
    // output[n] = input[n] * (1 - mix[n]) + 0 * mix[n] = input[n] * (1 - mix[n])
    //
    // With mix ramping 0→1 per-sample:
    //   mix[0] ≈ 0 → mix[63] ≈ 0.23   (5ms/48kHz, 64 steps)
    //   output[0] ≈ input[0] * 1.0
    //   output[63] ≈ input[63] * 0.77
    //
    // If mix were block-constant (the bug), all 64 samples would use the same
    // final-block mix value, making output[n] = input[n] * constant.
    // We distinguish by computing the ratio output[n]/input[n] for each n.
    // Per-sample: ratio[n] strictly decreasing (1-mix[n] decreasing as mix grows).
    // Block-constant: ratio[n] == constant for all n (flat).
    let sr = 48000u32;
    let mut p = DelayPlugin::new(1, 200.0, 0.0, 0.0); // mix=0, delay=200ms, feedback=0
    p.initialize(sr).unwrap();

    // Jump mix target to 1.0
    p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(1.0))
        .unwrap();

    // Process 64 frames of a ramp signal (input[n] = n+1, all positive and distinct)
    let num_frames = 64usize;
    let input: Vec<f32> = (0..num_frames).map(|n| (n + 1) as f32).collect();
    let mut buf = input.clone();
    p.process_in_place(&mut buf, &ProcessContext::new(sr, num_frames))
        .unwrap();

    // Compute effective mix per sample: mix[n] = 1 - output[n]/input[n]
    let ratios: Vec<f32> = buf
        .iter()
        .zip(input.iter())
        .map(|(&out, &inp)| out / inp) // ratio = (1 - mix[n])
        .collect();

    // Per-sample smoothing: ratio must be strictly decreasing (mix is increasing).
    // Check first vs last: ratio[0] > ratio[63].
    assert!(
        ratios[0] > ratios[num_frames - 1],
        "mix smoother must ramp per-sample: ratio[0]={} should be > ratio[63]={}",
        ratios[0],
        ratios[num_frames - 1]
    );
    // First sample: mix≈0.004 (one step from 0 with 5ms/48kHz), ratio≈0.996
    assert!(
        ratios[0] > 0.99,
        "ratio[0] should be near 1 (mix just started ramping), got {}",
        ratios[0]
    );
    // Last sample: after 64 steps mix≈0.23, ratio≈0.77
    assert!(
        ratios[num_frames - 1] < 0.95,
        "ratio[63] should be < 0.95 (mix has ramped), got {}",
        ratios[num_frames - 1]
    );
}

/// Verify that the delay smoother advances exactly once per processed frame.
#[test]
fn test_delay_smoother_advances_once_per_frame() {
    let sr = 48000u32;
    let mut p = DelayPlugin::new(1, 100.0, 0.0, 1.0); // mix=1 to hear delay
    p.initialize(sr).unwrap();

    let mut expected = sotf_host::smoothing::Smoother::new(100.0 * 48.0, 50.0, sr);
    expected.set_target(200.0 * 48.0);

    p.set_parameter(ParameterId::from("delay_ms"), ParameterValue::Float(200.0))
        .unwrap();

    let num_frames = 64usize;
    let mut buf = vec![0.0f32; num_frames];
    p.process_in_place(&mut buf, &ProcessContext::new(sr, num_frames))
        .unwrap();

    for _ in 0..num_frames {
        expected.advance();
    }
    let actual = p.delay_smoother.current();
    let expected = expected.current();
    assert!(
        (actual - expected).abs() < 1e-4,
        "delay smoother should advance once per frame: actual={actual}, expected={expected}"
    );
}

#[test]
fn test_allpass_coeff_parameter_exists_and_affects_response() {
    // Regression: allpass coefficient was hardcoded to 0.5 with no user parameter.
    let mut p = DelayPlugin::new(1, 10.0, 0.5, 0.5);
    p.initialize(48000).unwrap();

    // The parameter must exist
    assert!(
        p.get_parameter(&ParameterId::from("allpass_coeff"))
            .is_some(),
        "allpass_coeff parameter should exist"
    );

    // Enable allpass feedback
    p.set_parameter(
        ParameterId::from("allpass_feedback"),
        ParameterValue::Bool(true),
    )
    .unwrap();

    // Process impulse with coeff=0.5 (default)
    let mut b1 = vec![0.0; 2000];
    b1[0] = 1.0;
    p.process_in_place(&mut b1, &ProcessContext::new(48000, 2000))
        .unwrap();

    // Change coefficient to 0.8
    p.set_parameter(
        ParameterId::from("allpass_coeff"),
        ParameterValue::Float(0.8),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("allpass_coeff")),
        Some(ParameterValue::Float(0.8))
    );

    // Process identical impulse with coeff=0.8
    let mut b2 = vec![0.0; 2000];
    b2[0] = 1.0;
    p.process_in_place(&mut b2, &ProcessContext::new(48000, 2000))
        .unwrap();

    // The outputs must differ because the allpass coefficient changed
    let diff: f32 = b1.iter().zip(b2.iter()).map(|(a, b)| (a - b).abs()).sum();
    assert!(
        diff > 1e-6,
        "different allpass coefficients should produce different outputs, diff={}",
        diff
    );
}

#[test]
fn test_parameter_validation() {
    let mut p = DelayPlugin::new(1, 100.0, 0.3, 0.5);
    p.initialize(48000).unwrap();

    // LFO rate out of range should fail
    assert!(p
        .set_parameter(
            ParameterId::from("lfo_rate_hz"),
            ParameterValue::Float(15.0)
        )
        .is_err());

    // LFO depth out of range should fail
    assert!(p
        .set_parameter(
            ParameterId::from("lfo_depth_ms"),
            ParameterValue::Float(10.0)
        )
        .is_err());

    // Wrong type should fail
    assert!(p
        .set_parameter(
            ParameterId::from("allpass_feedback"),
            ParameterValue::Float(1.0)
        )
        .is_err());
}

/// process_in_place smoke test with a known impulse and no feedback.
#[test]
fn test_process_in_place_impulse_known_delay() {
    let sr = 48000u32;
    let delay_ms = 5.0;
    let delay_samples = (delay_ms / 1000.0 * sr as f32).round() as usize;
    let mut p = DelayPlugin::new(1, delay_ms, 0.0, 1.0); // mix=1, feedback=0
    p.initialize(sr).unwrap();

    let num_frames = delay_samples + 100;
    let mut buffer = vec![0.0f32; num_frames];
    buffer[0] = 1.0;

    p.process_in_place(&mut buffer, &ProcessContext::new(sr, num_frames))
        .unwrap();

    // Frame 0: no delayed sample yet, so output should be ~0 (mix=1 means wet only)
    assert!(
        buffer[0].abs() < 0.01,
        "frame 0 should be ~0, got {}",
        buffer[0]
    );

    // The delayed impulse should appear near delay_samples.
    let peak_idx = buffer
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
        .unwrap()
        .0;
    assert!(
        (peak_idx as i32 - delay_samples as i32).abs() <= 1,
        "peak should be at {delay_samples}, found at {peak_idx}"
    );
    assert!(
        buffer[peak_idx] > 0.9,
        "delayed impulse should be close to 1.0"
    );
}

/// set_parameter smoke tests for the primary scalar parameters.
#[test]
fn test_set_parameter_smoke_known_values() {
    let mut p = DelayPlugin::new(1, 20.0, 0.0, 0.0);
    p.initialize(48000).unwrap();

    p.set_parameter(ParameterId::from("delay_ms"), ParameterValue::Float(123.0))
        .unwrap();
    assert!((p.delay_ms - 123.0).abs() < 1e-4);

    p.set_parameter(ParameterId::from("feedback"), ParameterValue::Float(0.75))
        .unwrap();
    assert!((p.feedback - 0.75).abs() < 1e-6);

    p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
        .unwrap();
    assert!((p.mix - 0.5).abs() < 1e-6);

    p.set_parameter(ParameterId::from("lfo_rate_hz"), ParameterValue::Float(2.5))
        .unwrap();
    assert!((p.lfo_rate_hz - 2.5).abs() < 1e-6);

    p.set_parameter(
        ParameterId::from("lfo_depth_ms"),
        ParameterValue::Float(1.25),
    )
    .unwrap();
    assert!((p.lfo_depth_ms - 1.25).abs() < 1e-6);

    p.set_parameter(
        ParameterId::from("allpass_feedback"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert!(p.allpass_feedback);

    p.set_parameter(
        ParameterId::from("allpass_coeff"),
        ParameterValue::Float(0.7),
    )
    .unwrap();
    assert!((p.allpass_coeff - 0.7).abs() < 1e-6);
}

/// set_parameter must reject non-finite float values.
#[test]
fn test_set_parameter_rejects_non_finite() {
    let mut p = DelayPlugin::new(1, 20.0, 0.0, 0.0);
    p.initialize(48000).unwrap();

    assert!(p
        .set_parameter(
            ParameterId::from("delay_ms"),
            ParameterValue::Float(f32::NAN)
        )
        .is_err());
    assert!(p
        .set_parameter(
            ParameterId::from("delay_ms"),
            ParameterValue::Float(f32::INFINITY)
        )
        .is_err());
}

/// process_in_place with zero frames returns 0 and leaves the buffer untouched.
#[test]
fn test_process_in_place_zero_frames() {
    let mut p = DelayPlugin::new(1, 10.0, 0.0, 0.0);
    p.initialize(48000).unwrap();
    let mut buffer = vec![0.1, 0.2, 0.3];
    let processed = p
        .process_in_place(&mut buffer, &ProcessContext::new(48000, 0))
        .unwrap();
    assert_eq!(processed, 0);
    assert_eq!(buffer, vec![0.1, 0.2, 0.3]);
}

/// get_parameter round-trips values set by set_parameter.
#[test]
fn test_get_parameter_round_trip() {
    let mut p = DelayPlugin::new(1, 20.0, 0.0, 0.0);
    p.initialize(48000).unwrap();

    p.set_parameter(ParameterId::from("delay_ms"), ParameterValue::Float(99.0))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("delay_ms")),
        Some(ParameterValue::Float(99.0))
    );

    p.set_parameter(ParameterId::from("feedback"), ParameterValue::Float(0.42))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("feedback")),
        Some(ParameterValue::Float(0.42))
    );

    p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.88))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("mix")),
        Some(ParameterValue::Float(0.88))
    );
}

/// process_in_place with a DC step produces a known attenuated delayed copy when mix=0.5.
#[test]
fn test_process_in_place_step_known_output() {
    let sr = 48000u32;
    let delay_ms = 1.0;
    let delay_samples = (delay_ms / 1000.0 * sr as f32).round() as usize;
    let mut p = DelayPlugin::new(1, delay_ms, 0.0, 0.5); // mix=0.5, feedback=0
    p.initialize(sr).unwrap();

    // Step input of 1.0; output at frame n should be 0.5*1 + 0.5*delayed[n].
    // Before the delayed step arrives, delayed[n] = 0, so output = 0.5.
    // After delay_samples, delayed[n] = 1.0, so output = 1.0.
    let num_frames = delay_samples + 10;
    let mut buffer = vec![1.0f32; num_frames];

    p.process_in_place(&mut buffer, &ProcessContext::new(sr, num_frames))
        .unwrap();

    // Before delay: output should be 0.5 (half dry, half silent wet)
    assert!((buffer[0] - 0.5).abs() < 1e-4);
    assert!((buffer[delay_samples - 1] - 0.5).abs() < 1e-4);
    // After delay settles: output should be 1.0
    assert!((buffer[num_frames - 1] - 1.0).abs() < 1e-4);
}

// -------------------------------------------------------------------------
// set_parameter focused tests (per-channel, allpass, clamping)
// -------------------------------------------------------------------------

#[test]
fn test_set_parameter_per_channel_delay_roundtrip() {
    let mut p = DelayPlugin::new_per_channel(vec![5.0, 10.0]).unwrap();
    p.initialize(48000).unwrap();

    p.set_parameter(ParameterId::from("delay_ms_0"), ParameterValue::Float(20.0))
        .unwrap();
    p.set_parameter(ParameterId::from("delay_ms_1"), ParameterValue::Float(30.0))
        .unwrap();

    assert_eq!(
        p.get_parameter(&ParameterId::from("delay_ms_0")),
        Some(ParameterValue::Float(20.0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("delay_ms_1")),
        Some(ParameterValue::Float(30.0))
    );
}

#[test]
fn test_set_parameter_per_channel_invalid_id_errors() {
    let mut p = DelayPlugin::new(2, 10.0, 0.0, 0.0);
    p.initialize(48000).unwrap();
    // Not in per-channel mode: delay_ms_0 is not a valid parameter
    assert!(p
        .set_parameter(ParameterId::from("delay_ms_0"), ParameterValue::Float(5.0))
        .is_err());
}

#[test]
fn test_set_parameter_allpass_feedback_false_resets_state() {
    let mut p = DelayPlugin::new(1, 10.0, 0.5, 0.5);
    p.initialize(48000).unwrap();

    // Enable allpass feedback and warm up state
    p.set_parameter(
        ParameterId::from("allpass_feedback"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    let mut b1 = vec![0.0; 2000];
    b1[0] = 1.0;
    p.process_in_place(&mut b1, &ProcessContext::new(48000, 2000))
        .unwrap();

    // Disable allpass feedback (should reset internal state)
    p.set_parameter(
        ParameterId::from("allpass_feedback"),
        ParameterValue::Bool(false),
    )
    .unwrap();

    let mut b2 = vec![0.0; 2000];
    b2[0] = 1.0;
    p.process_in_place(&mut b2, &ProcessContext::new(48000, 2000))
        .unwrap();

    let diff: f32 = b1.iter().zip(b2.iter()).map(|(a, b)| (a - b).abs()).sum();
    assert!(
        diff > 1e-6,
        "disabling allpass_feedback should reset state and change output, diff={}",
        diff
    );
}

#[test]
fn test_set_parameter_allpass_coeff_boundaries() {
    let mut p = DelayPlugin::new(1, 10.0, 0.0, 0.0);
    p.initialize(48000).unwrap();

    // Maximum valid value
    p.set_parameter(
        ParameterId::from("allpass_coeff"),
        ParameterValue::Float(0.99),
    )
    .unwrap();
    assert!(
        (p.allpass_coeff - 0.99).abs() < 1e-6,
        "allpass_coeff should accept 0.99, got {}",
        p.allpass_coeff
    );

    // Minimum valid value
    p.set_parameter(
        ParameterId::from("allpass_coeff"),
        ParameterValue::Float(0.0),
    )
    .unwrap();
    assert!(
        (p.allpass_coeff - 0.0).abs() < 1e-6,
        "allpass_coeff should accept 0.0, got {}",
        p.allpass_coeff
    );

    // Out of range should be rejected by validation
    assert!(p
        .set_parameter(
            ParameterId::from("allpass_coeff"),
            ParameterValue::Float(1.5)
        )
        .is_err());
    assert!(p
        .set_parameter(
            ParameterId::from("allpass_coeff"),
            ParameterValue::Float(-0.5)
        )
        .is_err());
}

#[test]
fn test_set_parameter_per_channel_delay_affects_processing() {
    let sr = 48000u32;
    let mut p = DelayPlugin::new_per_channel(vec![5.0, 10.0]).unwrap();
    p.initialize(sr).unwrap();
    p.reset();

    let num_frames = 1024;
    let mut buf = vec![0.0f32; num_frames * 2];
    buf[0] = 1.0;
    buf[1] = 1.0;

    p.process_in_place(&mut buf, &ProcessContext::new(sr, num_frames))
        .unwrap();

    // Change channel 0 delay from 5ms (240 samples) to 15ms (720 samples)
    p.set_parameter(ParameterId::from("delay_ms_0"), ParameterValue::Float(15.0))
        .unwrap();
    p.reset(); // snap smoother to new target

    let mut buf2 = vec![0.0f32; num_frames * 2];
    buf2[0] = 1.0;
    buf2[1] = 1.0;
    p.process_in_place(&mut buf2, &ProcessContext::new(sr, num_frames))
        .unwrap();

    let peak_ch0_before = (10..num_frames)
        .max_by(|&a, &b| buf[a * 2].abs().partial_cmp(&buf[b * 2].abs()).unwrap())
        .unwrap();
    let peak_ch0_after = (10..num_frames)
        .max_by(|&a, &b| buf2[a * 2].abs().partial_cmp(&buf2[b * 2].abs()).unwrap())
        .unwrap();

    assert!(
        (peak_ch0_before as i32 - 240).abs() <= 2,
        "before: peak should be near 240, got {}",
        peak_ch0_before
    );
    assert!(
        (peak_ch0_after as i32 - 720).abs() <= 2,
        "after: peak should be near 720, got {}",
        peak_ch0_after
    );
}

#[test]
fn test_set_parameter_per_channel_rejects_non_finite() {
    let mut p = DelayPlugin::new_per_channel(vec![5.0]).unwrap();
    p.initialize(48000).unwrap();

    assert!(p
        .set_parameter(
            ParameterId::from("delay_ms_0"),
            ParameterValue::Float(f32::NAN)
        )
        .is_err());
}
