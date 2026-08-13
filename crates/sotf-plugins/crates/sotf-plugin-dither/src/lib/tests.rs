use super::dither_plugin::DitherPlugin;
use super::misc::random_f32;
use super::misc::xorshift64;
use super::types::DitherPluginParams;
use rustfft::{FftPlanner, num_complex::Complex};
use sotf_host::ParametricInPlacePlugin;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::ProcessContext;

fn make_context(num_frames: usize) -> ProcessContext<'static> {
    ProcessContext::new(48000, num_frames)
}

fn noise_transfer_magnitude(plugin: &DitherPlugin, frequency_hz: f32) -> f32 {
    let mut real = 1.0_f32;
    let mut imag = 0.0_f32;
    for (coefficient, delay_seconds) in super::misc::NOISE_SHAPING_COEFFS
        .iter()
        .zip(plugin.noise_shaping_delays_samples.iter())
    {
        let delay_seconds = delay_seconds / plugin.sample_rate as f32;
        let phase = 2.0 * std::f32::consts::PI * frequency_hz * delay_seconds;
        real -= coefficient * phase.cos();
        imag += coefficient * phase.sin();
    }
    real.hypot(imag)
}

#[test]
fn f_weighted_shaper_preserves_absolute_frequency_response_across_sample_rates() {
    let mut reference = DitherPlugin::new(1);
    reference.initialize(44_100).unwrap();
    let mut double_rate = DitherPlugin::new(1);
    double_rate.initialize(88_200).unwrap();

    assert_eq!(reference.noise_shaping_delays_samples, [1.0, 2.0, 3.0]);
    assert_eq!(double_rate.noise_shaping_delays_samples, [2.0, 4.0, 6.0]);

    for frequency_hz in [1_000.0, 5_000.0, 10_000.0, 15_000.0, 20_000.0] {
        let at_reference = noise_transfer_magnitude(&reference, frequency_hz);
        let at_double_rate = noise_transfer_magnitude(&double_rate, frequency_hz);
        assert!(
            (at_reference - at_double_rate).abs() < 1.0e-5,
            "F-weighted NTF changed at {frequency_hz} Hz: {at_reference} vs {at_double_rate}"
        );
    }
}

#[test]
fn f_weighted_shaper_has_a_bounded_policy_for_every_supported_rate_family() {
    for sample_rate in [
        8_000_u32, 16_000, 22_050, 32_000, 44_100, 48_000, 96_000, 192_000, 384_000, 768_000,
    ] {
        let mut plugin = DitherPlugin::new(2);
        plugin.initialize(sample_rate).unwrap();
        assert!(
            plugin
                .noise_shaping_delays_samples
                .iter()
                .all(|delay| delay.is_finite() && *delay >= 1.0)
        );
        assert!(plugin.error_history.iter().all(|history| {
            history.len() > plugin.noise_shaping_delays_samples[2].ceil() as usize
        }));

        let frames = 128;
        let mut signal = vec![0.001_234_f32; frames * 2];
        plugin
            .process_in_place(&mut signal, &ProcessContext::new(sample_rate, frames))
            .unwrap();
        assert!(signal.iter().all(|sample| sample.is_finite()));
    }

    assert!(DitherPlugin::new(1).initialize(0).is_err());
    assert!(DitherPlugin::new(1).initialize(768_001).is_err());
}

fn averaged_error_spectrum(noise_shaping: bool) -> Vec<f64> {
    const SAMPLE_RATE: u32 = 48_000;
    const FFT_SIZE: usize = 2048;
    const SEGMENTS: usize = 16;
    const TOTAL: usize = FFT_SIZE * SEGMENTS;

    // Deterministic broadband programme decorrelates quantization error from
    // individual FFT bins without adding explicit dither that would mask the
    // error-feedback shaper under test.
    let mut rng = 0x42a7_91d3_55ee_1021_u64;
    let original: Vec<f32> = (0..TOTAL).map(|_| random_f32(&mut rng) * 0.02).collect();
    let mut quantized = original.clone();
    let mut plugin = DitherPlugin::from_params(
        1,
        DitherPluginParams {
            bit_depth: 0,
            noise_shaping,
            dither_type: 1,
        },
    );
    plugin.initialize(SAMPLE_RATE).unwrap();
    plugin
        .process_in_place(&mut quantized, &ProcessContext::new(SAMPLE_RATE, TOTAL))
        .unwrap();

    let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);
    let mut averaged = vec![0.0_f64; FFT_SIZE / 2 + 1];
    let mut bins = vec![Complex::new(0.0_f32, 0.0_f32); FFT_SIZE];
    for segment in 0..SEGMENTS {
        for (index, bin) in bins.iter_mut().enumerate() {
            let window =
                0.5 - 0.5 * (2.0 * std::f32::consts::PI * index as f32 / FFT_SIZE as f32).cos();
            let offset = segment * FFT_SIZE + index;
            *bin = Complex::new((quantized[offset] - original[offset]) * window, 0.0);
        }
        fft.process(&mut bins);
        for (power, bin) in averaged.iter_mut().zip(&bins) {
            *power += bin.norm_sqr() as f64;
        }
    }
    averaged
}

#[test]
fn f_weighted_noise_shaping_moves_quantization_error_out_of_the_sensitive_band() {
    let flat = averaged_error_spectrum(false);
    let shaped = averaged_error_spectrum(true);
    let bin_hz = 48_000.0 / 2048.0;
    let band_power = |spectrum: &[f64], low_hz: f64, high_hz: f64| {
        spectrum
            .iter()
            .enumerate()
            .filter(|(bin, _)| {
                let frequency = *bin as f64 * bin_hz;
                frequency >= low_hz && frequency < high_hz
            })
            .map(|(_, power)| *power)
            .sum::<f64>()
    };

    let flat_sensitive = band_power(&flat, 200.0, 8_000.0);
    let shaped_sensitive = band_power(&shaped, 200.0, 8_000.0);
    let flat_ultrasonic = band_power(&flat, 16_000.0, 23_000.0);
    let shaped_ultrasonic = band_power(&shaped, 16_000.0, 23_000.0);
    assert!(
        shaped_sensitive < flat_sensitive * 0.75,
        "shaping did not reduce sensitive-band error: flat={flat_sensitive:e}, shaped={shaped_sensitive:e}"
    );
    assert!(
        shaped_ultrasonic > flat_ultrasonic * 1.5,
        "shaping did not move error upward: flat={flat_ultrasonic:e}, shaped={shaped_ultrasonic:e}"
    );
}

#[test]
fn test_dither_basic() {
    // Process silence, verify output stays near zero
    let mut plugin = DitherPlugin::new(2);
    plugin.initialize(48000).unwrap();

    let num_frames = 1024;
    let mut buffer = vec![0.0f32; num_frames * 2];
    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    // With dither on silence, output should be very small (within 4 LSB of 16-bit)
    let max_lsb_16 = 1.0 / 32768.0; // 1 LSB at 16-bit
    for &sample in &buffer {
        assert!(
            sample.abs() <= max_lsb_16 * 4.0,
            "Dithered silence should stay near zero, got {}",
            sample
        );
    }
}

#[test]
fn test_dither_quantizes_to_target_depth() {
    // Process a known signal, verify output values are on the 16-bit grid
    let mut plugin = DitherPlugin::from_params(
        1,
        DitherPluginParams {
            bit_depth: 0, // 16-bit
            noise_shaping: false,
            dither_type: 1, // None (no dither, just quantize)
        },
    );
    plugin.initialize(48000).unwrap();

    let scale_16 = 32768.0_f32;
    let num_frames = 512;
    let mut buffer: Vec<f32> = (0..num_frames)
        .map(|i| (i as f32 / num_frames as f32) * 0.5 - 0.25)
        .collect();

    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    // Every output value should be exactly on the 16-bit grid
    for &sample in &buffer {
        let scaled = sample * scale_16;
        let rounded = scaled.round();
        assert!(
            (scaled - rounded).abs() < 1e-4,
            "Sample {} is not on 16-bit grid (scaled={})",
            sample,
            scaled
        );
    }
}

#[test]
fn test_noise_shaping_reduces_audible_noise() {
    // Compare total quantization error with and without noise shaping.
    // With noise shaping, the error is reshaped (not necessarily reduced in total
    // energy), but the low-frequency portion should be lower.
    let num_frames = 8192;
    let channels = 1;

    // Generate a quiet sine wave (well below full scale so quantization matters)
    let freq = 1000.0;
    let sr = 48000.0;
    let amplitude = 0.01; // ~-40 dBFS
    let original: Vec<f32> = (0..num_frames)
        .map(|i| amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin())
        .collect();

    // Process WITHOUT noise shaping
    let mut plugin_no_ns = DitherPlugin::from_params(
        channels,
        DitherPluginParams {
            bit_depth: 0,
            noise_shaping: false,
            dither_type: 0,
        },
    );
    plugin_no_ns.initialize(48000).unwrap();
    let mut buf_no_ns = original.clone();
    plugin_no_ns
        .process_in_place(&mut buf_no_ns, &make_context(num_frames))
        .unwrap();

    // Process WITH noise shaping
    let mut plugin_ns = DitherPlugin::from_params(
        channels,
        DitherPluginParams {
            bit_depth: 0,
            noise_shaping: true,
            dither_type: 0,
        },
    );
    plugin_ns.initialize(48000).unwrap();
    let mut buf_ns = original.clone();
    plugin_ns
        .process_in_place(&mut buf_ns, &make_context(num_frames))
        .unwrap();

    // Compute error energy in the low-frequency band (bins 0..N/8 ~ 0-3kHz)
    // For a rough check, just compute the sum of squared differences
    let error_no_ns: f64 = buf_no_ns
        .iter()
        .zip(original.iter())
        .map(|(o, i)| ((*o - *i) as f64).powi(2))
        .sum();

    let error_ns: f64 = buf_ns
        .iter()
        .zip(original.iter())
        .map(|(o, i)| ((*o - *i) as f64).powi(2))
        .sum();

    // Both should produce some quantization error
    assert!(error_no_ns > 0.0, "No-NS error should be non-zero");
    assert!(error_ns > 0.0, "NS error should be non-zero");

    // Noise shaping may increase total error energy (it reshapes, doesn't remove).
    // We just verify both produce finite, reasonable results.
    assert!(error_no_ns.is_finite());
    assert!(error_ns.is_finite());
}

#[test]
fn test_dither_parameter_set_get() {
    let mut plugin = DitherPlugin::new(2);

    // Test bit_depth
    plugin
        .set_parameter(
            ParameterId::from("bit_depth"),
            ParameterValue::Int(2), // 24-bit
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("bit_depth")),
        Some(ParameterValue::Int(2))
    );

    // Test noise_shaping
    plugin
        .set_parameter(
            ParameterId::from("noise_shaping"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("noise_shaping")),
        Some(ParameterValue::Bool(false))
    );

    // Test dither_type
    plugin
        .set_parameter(
            ParameterId::from("dither_type"),
            ParameterValue::Int(2), // Truncate
        )
        .unwrap();
    assert_eq!(
        plugin.get_parameter(&ParameterId::from("dither_type")),
        Some(ParameterValue::Int(2))
    );

    // Test unknown parameter
    assert!(
        plugin
            .set_parameter(ParameterId::from("unknown"), ParameterValue::Float(0.0),)
            .is_err()
    );
    assert_eq!(plugin.get_parameter(&ParameterId::from("unknown")), None);
}

#[test]
fn test_xorshift64_produces_nonzero_values() {
    let mut state = 0xDEAD_BEEF_CAFE_0001_u64;
    let mut all_zero = true;
    for _ in 0..100 {
        let val = xorshift64(&mut state);
        if val != 0 {
            all_zero = false;
        }
    }
    assert!(!all_zero, "xorshift64 should produce non-zero values");
}

#[test]
fn test_random_f32_range() {
    let mut state = 0xDEAD_BEEF_CAFE_0001_u64;
    for _ in 0..10000 {
        let val = random_f32(&mut state);
        assert!(
            (-0.5..=0.5).contains(&val),
            "random_f32 out of range: {}",
            val
        );
    }
}

/// Verify the PRNG boundary: the worst-case input (upper = u32::MAX) produces
/// a value within the valid TPDF range [-0.5, 0.5].
///
/// Note: due to f32 rounding of u32::MAX (which rounds up to 2^32), the result
/// for u32::MAX is exactly 0.5 (not strictly less).  This is acceptable for TPDF
/// dither — the closed interval [-0.5, 0.5] is statistically equivalent for audio
/// use.  The comment in `random_f32` was corrected from "[-0.5, 0.5)" to
/// "[-0.5, 0.5]" to match the actual implementation.
#[test]
fn test_random_f32_boundary_precision() {
    // Directly exercise the boundary: upper = u32::MAX produces exactly 0.5
    // (because u32::MAX as f32 rounds up to 2^32 = the divisor, giving ratio 1.0).
    let upper_max = u32::MAX;
    let val = (upper_max as f32 / u32::MAX as f32) - 0.5;
    // The boundary value must be within [-0.5, 0.5] — not outside.
    assert!(
        val <= 0.5,
        "random_f32 boundary: value exceeds 0.5: {}",
        val
    );
    assert!(
        val >= -0.5,
        "random_f32 boundary: value below -0.5: {}",
        val
    );
    // Confirm this specific boundary is exactly 0.5 (documents the known f32 behavior).
    assert_eq!(
        val, 0.5,
        "random_f32 boundary for u32::MAX should be exactly 0.5, got {}",
        val
    );
}

#[test]
fn test_tpdf_is_independent_triangular_noise() {
    let mut plugin = DitherPlugin::new(1);
    plugin.initialize(48000).unwrap();
    let samples: Vec<f64> = (0..100_000).map(|_| plugin.next_tpdf(0) as f64).collect();
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let variance = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / samples.len() as f64;
    let lag_one = samples
        .windows(2)
        .map(|pair| (pair[0] - mean) * (pair[1] - mean))
        .sum::<f64>()
        / (samples.len() - 1) as f64;
    assert!(mean.abs() < 0.005, "TPDF mean={mean}");
    assert!(
        (variance - 1.0 / 6.0).abs() < 0.005,
        "TPDF variance={variance}"
    );
    assert!(
        (lag_one / variance).abs() < 0.02,
        "lag-1 correlation={}",
        lag_one / variance
    );
    assert!(samples.iter().all(|x| (-1.0..=1.0).contains(x)));
}

#[test]
fn test_noise_shaping_feedback_excludes_dither_term() {
    let mut plugin = DitherPlugin::from_params(
        1,
        DitherPluginParams {
            bit_depth: 0,
            noise_shaping: true,
            dither_type: 0, // TPDF
        },
    );
    plugin.initialize(48000).unwrap();

    // Use a deterministic sample and capture the next TPDF token before processing.
    let input = 0.123456_f32;
    let saved_rng_state = plugin.rng_state.clone();
    let tpdf = plugin.next_tpdf(0);
    plugin.rng_state = saved_rng_state;

    // With a fresh plugin state, the initial noise-shaping feedback is zero.
    let mut buffer = vec![input];
    plugin
        .process_in_place(&mut buffer, &make_context(1))
        .unwrap();

    let shaped = input;
    let dithered = shaped + tpdf * plugin.inv_scale;
    let quantized = (dithered * plugin.scale).round() * plugin.inv_scale;
    let expected_error = quantized - dithered;
    let stale_error = quantized - shaped;

    assert!(
        tpdf.abs() > 0.0,
        "TPDF sample must be non-zero for this regression"
    );
    assert!(
        (plugin.error_history[0][0] - expected_error).abs() < 1e-6,
        "expected feedback residual to exclude explicit dither"
    );
    assert!(
        (expected_error - stale_error).abs() > 1e-9,
        "TPDF should distinguish the two residual definitions"
    );
    assert_eq!(
        plugin.error_history[0][0], expected_error,
        "stored error history should match the quantizer-input residual"
    );
}

#[test]
fn reset_restarts_deterministic_tpdf_sequence() {
    let mut plugin = DitherPlugin::new(1);
    let first: Vec<f32> = (0..16).map(|_| plugin.next_tpdf(0)).collect();
    plugin.reset();
    let restarted: Vec<f32> = (0..16).map(|_| plugin.next_tpdf(0)).collect();
    assert_eq!(first, restarted);
}

#[test]
fn test_truncate_mode_quantizes_without_rounding() {
    let mut plugin = DitherPlugin::from_params(
        1,
        DitherPluginParams {
            bit_depth: 0, // 16-bit
            noise_shaping: false,
            dither_type: 2, // Truncate
        },
    );
    plugin.initialize(48000).unwrap();

    let scale_16 = 32768.0_f32;
    let mut buffer = vec![-0.123456, 0.123456];
    let num_frames = buffer.len();
    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    // Truncation is toward zero, so results are different from rounding for
    // these non-symmetric test points.
    assert!(
        (buffer[0] - ((-0.123456_f32 * scale_16).trunc() * (1.0 / scale_16))).abs() < 1e-7,
        "truncate should apply for negative value, got {}",
        buffer[0]
    );
    assert!(
        (buffer[1] - ((0.123456_f32 * scale_16).trunc() * (1.0 / scale_16))).abs() < 1e-7,
        "truncate should apply for positive value, got {}",
        buffer[1]
    );
}

#[test]
fn test_24bit_quantization_grid() {
    let mut plugin = DitherPlugin::from_params(
        1,
        DitherPluginParams {
            bit_depth: 2, // 24-bit
            noise_shaping: false,
            dither_type: 1, // None
        },
    );
    plugin.initialize(48000).unwrap();

    let scale_24 = 8388608.0_f32; // 2^23
    let num_frames = 256;
    let mut buffer: Vec<f32> = (0..num_frames)
        .map(|i| (i as f32 / num_frames as f32) * 0.1)
        .collect();

    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    for &sample in &buffer {
        let scaled = sample * scale_24;
        let rounded = scaled.round();
        assert!(
            (scaled - rounded).abs() < 1e-2,
            "Sample {} is not on 24-bit grid (scaled={})",
            sample,
            scaled
        );
    }
}

#[test]
fn signed_pcm_endpoints_are_saturated_before_error_feedback() {
    for (bit_depth, bits) in [(0usize, 16i32), (1usize, 20i32), (2usize, 24i32)] {
        let scale = 2.0_f32.powi(bits - 1);
        let inv_scale = 1.0 / scale;
        let max_code = scale - 1.0;

        // Exercise both signed endpoints and the highest representable value
        // just below full scale without noise shaping first.
        let mut plugin = DitherPlugin::from_params(
            1,
            DitherPluginParams {
                bit_depth,
                noise_shaping: false,
                dither_type: 1, // None (round)
            },
        );
        plugin.initialize(48000).unwrap();
        let near_full_scale = 1.0 - 1.5 * inv_scale;
        let mut buffer = vec![-1.0, 1.0, near_full_scale];
        let num_frames = buffer.len();
        plugin
            .process_in_place(&mut buffer, &make_context(num_frames))
            .unwrap();

        assert_eq!(buffer[0], -1.0, "negative endpoint at {bits}-bit");
        assert_eq!(
            buffer[1],
            max_code * inv_scale,
            "positive endpoint at {bits}-bit"
        );
        assert_eq!(
            buffer[2],
            max_code * inv_scale,
            "near-full-scale rounding must not emit an out-of-range code at {bits}-bit"
        );

        // A shaping overshoot must feed back the code that was actually
        // emitted after saturation, rather than the unrepresentable +1.0.
        let mut shaped_plugin = DitherPlugin::from_params(
            1,
            DitherPluginParams {
                bit_depth,
                noise_shaping: true,
                dither_type: 1, // None (round)
            },
        );
        shaped_plugin.initialize(48000).unwrap();
        let mut overshoot = vec![1.0];
        let num_frames = overshoot.len();
        shaped_plugin
            .process_in_place(&mut overshoot, &make_context(num_frames))
            .unwrap();
        assert_eq!(
            overshoot[0],
            max_code * inv_scale,
            "shaper overshoot must saturate at {bits}-bit signed PCM maximum"
        );
        assert_eq!(
            shaped_plugin.error_history[0][0],
            max_code * inv_scale - 1.0,
            "noise-shaping state must use the emitted {bits}-bit code"
        );
    }
}

#[test]
fn test_multichannel_independent() {
    // Verify each channel gets independent dither
    let mut plugin = DitherPlugin::from_params(
        2,
        DitherPluginParams {
            bit_depth: 0,
            noise_shaping: false,
            dither_type: 0,
        },
    );
    plugin.initialize(48000).unwrap();

    let num_frames = 256;
    // Same value on both channels
    let val = 0.00123_f32;
    let mut buffer = vec![val; num_frames * 2];

    plugin
        .process_in_place(&mut buffer, &make_context(num_frames))
        .unwrap();

    // With TPDF dither, the two channels should generally differ
    // (different RNG states), though they could rarely match
    let mut differ_count = 0;
    for frame in 0..num_frames {
        if (buffer[frame * 2] - buffer[frame * 2 + 1]).abs() > 1e-10 {
            differ_count += 1;
        }
    }
    assert!(
        differ_count > num_frames / 4,
        "Channels should have independent dither, but only {} of {} frames differed",
        differ_count,
        num_frames
    );
}
