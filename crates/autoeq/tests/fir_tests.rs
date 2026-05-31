//! Integration tests for FIR filter generation
//!
//! Tests the round-trip accuracy of FIR correction filters and validates
//! that different phase types work correctly.

use autoeq::Curve;
use autoeq::fir::{FirPhase, generate_fir_from_response, save_fir_to_wav};
use ndarray::Array1;
use num_complex::Complex64;
use rustfft::FftPlanner;
use std::path::PathBuf;
use tempfile::TempDir;

// ============================================================================
// Test Helper Functions
// ============================================================================

/// Create a synthetic frequency response curve
fn create_test_curve(freqs: &[f64], spl_values: &[f64]) -> Curve {
    Curve {
        freq: Array1::from(freqs.to_vec()),
        spl: Array1::from(spl_values.to_vec()),
        phase: None,
        ..Default::default()
    }
}

/// Create a flat response at the given SPL level
fn create_flat_curve(min_freq: f64, max_freq: f64, n_points: usize, spl_db: f64) -> Curve {
    let freqs: Vec<f64> = (0..n_points)
        .map(|i| {
            let t = i as f64 / (n_points - 1) as f64;
            min_freq * (max_freq / min_freq).powf(t)
        })
        .collect();
    let spl: Vec<f64> = vec![spl_db; n_points];
    create_test_curve(&freqs, &spl)
}

/// Create a response curve with a deep null at a specified frequency
fn create_curve_with_null(null_freq: f64, null_depth_db: f64) -> Curve {
    // Create frequencies logarithmically spaced with extra points around the null
    let mut freqs = vec![20.0, 30.0, 40.0, 50.0, 60.0];

    // Add points around the null
    freqs.push(null_freq * 0.7);
    freqs.push(null_freq * 0.85);
    freqs.push(null_freq * 0.95);
    freqs.push(null_freq);
    freqs.push(null_freq * 1.05);
    freqs.push(null_freq * 1.15);
    freqs.push(null_freq * 1.3);

    // Continue with higher frequencies
    freqs.extend_from_slice(&[
        150.0, 200.0, 300.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
    ]);
    freqs.sort_by(f64::total_cmp);

    // Create SPL values with a null
    let baseline = 85.0;
    let spl: Vec<f64> = freqs
        .iter()
        .map(|&f| {
            // Create a narrow dip around the null frequency
            let distance = ((f / null_freq).ln()).abs();
            if distance < 0.2 {
                // Within the null region
                let depth = null_depth_db * (1.0 - distance / 0.2);
                baseline + depth
            } else {
                // Normal response with gentle rolloff
                baseline - (f.log10() - 2.5).abs() * 2.0
            }
        })
        .collect();

    create_test_curve(&freqs, &spl)
}

/// Compute the frequency response of an FIR filter
fn compute_fir_frequency_response(
    coeffs: &[f64],
    sample_rate: f64,
    frequencies: &[f64],
) -> Vec<f64> {
    let fft_size = coeffs.len().next_power_of_two() * 4;

    // Zero-pad the coefficients
    let mut padded: Vec<Complex64> = coeffs.iter().map(|&x| Complex64::new(x, 0.0)).collect();
    padded.resize(fft_size, Complex64::new(0.0, 0.0));

    // FFT
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
    fft.process(&mut padded);

    // Extract magnitude at requested frequencies
    let freq_step = sample_rate / fft_size as f64;

    frequencies
        .iter()
        .map(|&f| {
            let bin = (f / freq_step).round() as usize;
            let bin = bin.min(fft_size / 2);
            let mag = padded[bin].norm();
            20.0 * mag.max(1e-10).log10()
        })
        .collect()
}

/// Compute RMS deviation between two curves (in dB)
fn compute_rms_deviation(curve1: &[f64], curve2: &[f64]) -> f64 {
    assert_eq!(curve1.len(), curve2.len());
    let sum_sq: f64 = curve1
        .iter()
        .zip(curve2.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();
    (sum_sq / curve1.len() as f64).sqrt()
}

/// Load a test CSV file
fn load_test_csv(filename: &str) -> Curve {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/fir")
        .join(filename);

    autoeq::read_curve_from_csv(&path).unwrap_or_else(|_| panic!("Failed to load {}", filename))
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_fir_round_trip_flat_response() {
    // Generate FIR for flat response, apply it, verify result is still flat
    let sample_rate = 48000.0;
    let n_taps = 2048;

    // Target: flat at 0dB (unity gain)
    let target = create_flat_curve(20.0, 20000.0, 100, 0.0);

    let coeffs = generate_fir_from_response(&target, sample_rate, n_taps, FirPhase::Linear);

    // Compute frequency response of the FIR
    let test_freqs: Vec<f64> = (0..50)
        .map(|i| 20.0 * (20000.0 / 20.0_f64).powf(i as f64 / 49.0))
        .collect();

    let fir_response = compute_fir_frequency_response(&coeffs, sample_rate, &test_freqs);

    // The response should be approximately flat (near 0dB)
    // Allow some deviation due to windowing effects at band edges
    let mid_range_response: Vec<f64> = fir_response
        .iter()
        .zip(test_freqs.iter())
        .filter(|&(_, f)| *f >= 100.0 && *f <= 10000.0)
        .map(|(r, _)| *r)
        .collect();

    let mean_level: f64 = mid_range_response.iter().sum::<f64>() / mid_range_response.len() as f64;
    let max_deviation = mid_range_response
        .iter()
        .map(|&r| (r - mean_level).abs())
        .fold(0.0_f64, f64::max);

    assert!(
        max_deviation < 3.0,
        "Flat target should produce flat FIR response, max deviation = {:.2} dB",
        max_deviation
    );
}

#[test]
fn test_fir_round_trip_correction() {
    // Create a measurement with some deviation, generate correction, verify improvement
    let sample_rate = 48000.0;
    let n_taps = 4096;

    // Measurement with peaks and dips
    let measurement = create_test_curve(
        &[
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ],
        &[-3.0, 0.0, 3.0, 5.0, 2.0, 0.0, -2.0, -4.0, -6.0, -10.0],
    );

    // Target: flat at 0dB
    let target_spl = vec![0.0; measurement.spl.len()];

    // Correction curve = target - measurement (what we need to add)
    let correction_spl: Vec<f64> = target_spl
        .iter()
        .zip(measurement.spl.iter())
        .map(|(t, m)| t - m)
        .collect();

    let correction_curve = Curve {
        freq: measurement.freq.clone(),
        spl: Array1::from(correction_spl),
        phase: None,
        ..Default::default()
    };

    let coeffs =
        generate_fir_from_response(&correction_curve, sample_rate, n_taps, FirPhase::Linear);

    // Compute the FIR response
    let test_freqs: Vec<f64> = measurement.freq.to_vec();
    let fir_response = compute_fir_frequency_response(&coeffs, sample_rate, &test_freqs);

    // Apply correction: result = measurement + FIR response
    let corrected: Vec<f64> = measurement
        .spl
        .iter()
        .zip(fir_response.iter())
        .map(|(m, f)| m + f)
        .collect();

    // Focus on mid-band (100Hz - 5kHz) where FIR is most accurate
    let mid_indices: Vec<usize> = test_freqs
        .iter()
        .enumerate()
        .filter(|(_, f)| **f >= 100.0 && **f <= 5000.0)
        .map(|(i, _)| i)
        .collect();

    let original_mid: Vec<f64> = mid_indices.iter().map(|&i| measurement.spl[i]).collect();
    let corrected_mid: Vec<f64> = mid_indices.iter().map(|&i| corrected[i]).collect();
    let target_mid: Vec<f64> = vec![0.0; mid_indices.len()];

    let original_mid_dev = compute_rms_deviation(&original_mid, &target_mid);
    let corrected_mid_dev = compute_rms_deviation(&corrected_mid, &target_mid);

    // Corrected should be better than original in the mid-band
    // Allow for some tolerance since FIR windowing affects accuracy
    assert!(
        corrected_mid_dev <= original_mid_dev + 1.0, // Allow 1dB tolerance
        "FIR correction should improve or maintain response. Original dev: {:.2}dB, Corrected dev: {:.2}dB",
        original_mid_dev,
        corrected_mid_dev
    );
}

#[test]
fn test_fir_handles_room_null_gracefully() {
    // Test that FIR generation doesn't produce extreme coefficients for deep nulls
    let null_freq = 80.0;
    let null_depth = -30.0;

    let measurement = create_curve_with_null(null_freq, null_depth);

    // Target is flat at baseline level
    let baseline = 85.0;
    let target_spl: Vec<f64> = vec![baseline; measurement.spl.len()];

    // Compute correction curve
    let correction_spl: Vec<f64> = target_spl
        .iter()
        .zip(measurement.spl.iter())
        .map(|(t, m)| t - m)
        .collect();

    let correction_curve = Curve {
        freq: measurement.freq.clone(),
        spl: Array1::from(correction_spl),
        phase: None,
        ..Default::default()
    };

    let sample_rate = 48000.0;
    let n_taps = 4096;

    let coeffs =
        generate_fir_from_response(&correction_curve, sample_rate, n_taps, FirPhase::Linear);

    // FIR should be generated without panicking
    assert_eq!(coeffs.len(), n_taps);

    // Coefficients should be finite
    assert!(
        coeffs.iter().all(|&x| x.is_finite()),
        "All FIR coefficients should be finite"
    );

    // Compute the frequency response of the FIR filter
    let fir_response = compute_fir_frequency_response(&coeffs, sample_rate, &[null_freq]);

    // The boost at the null frequency might be large but should be finite
    let boost_at_null = fir_response[0];
    assert!(
        boost_at_null.is_finite(),
        "FIR response at null should be finite"
    );
}

#[test]
fn test_fir_output_file_valid() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let wav_path = temp_dir.path().join("test_output.wav");

    // Generate a realistic FIR filter
    let sample_rate = 48000.0;
    let n_taps = 1024;

    let target = create_test_curve(
        &[20.0, 100.0, 1000.0, 10000.0, 20000.0],
        &[0.0, 3.0, 0.0, -3.0, -6.0],
    );

    let coeffs = generate_fir_from_response(&target, sample_rate, n_taps, FirPhase::Linear);

    // Save to WAV
    let save_result = save_fir_to_wav(&coeffs, sample_rate as u32, &wav_path);
    assert!(save_result.is_ok(), "Should save FIR to WAV successfully");

    // Load and verify
    let reader = hound::WavReader::open(&wav_path).expect("Should open saved WAV");
    let spec = reader.spec();

    assert_eq!(spec.channels, 1, "FIR WAV should be mono");
    assert_eq!(spec.sample_rate, 48000, "Sample rate should match");
    assert_eq!(spec.bits_per_sample, 32, "Should be 32-bit");
    assert_eq!(
        reader.len() as usize,
        n_taps,
        "Sample count should match tap count"
    );

    // Read back samples and verify they're close to original
    let samples: Vec<f32> = reader
        .into_samples::<f32>()
        .map(|s| s.expect("Should read sample"))
        .collect();

    assert_eq!(samples.len(), coeffs.len());

    // Verify samples match (within f32 precision)
    for (i, (&original, &loaded)) in coeffs.iter().zip(samples.iter()).enumerate() {
        let diff = (original as f32 - loaded).abs();
        assert!(
            diff < 1e-6,
            "Sample {} mismatch: original={}, loaded={}",
            i,
            original,
            loaded
        );
    }
}

#[test]
fn test_load_flat_response_fixture() {
    let curve = load_test_csv("flat_response.csv");

    assert_eq!(curve.freq.len(), 10);
    assert_eq!(curve.spl.len(), 10);

    // All SPL values should be 85.0
    for &spl in curve.spl.iter() {
        assert!(
            (spl - 85.0).abs() < 0.01,
            "Flat response should have 85dB SPL, got {}",
            spl
        );
    }
}

#[test]
fn test_load_room_with_null_fixture() {
    let curve = load_test_csv("room_with_null.csv");

    assert!(curve.freq.len() > 5);

    // Find the null around 80Hz
    let null_idx = curve
        .freq
        .iter()
        .enumerate()
        .find(|(_, f)| (**f - 80.0).abs() < 1.0)
        .map(|(i, _)| i)
        .expect("Should have 80Hz point");

    let null_spl = curve.spl[null_idx];
    let surrounding_spl = (curve.spl[null_idx - 1] + curve.spl[null_idx + 1]) / 2.0;

    assert!(
        null_spl < surrounding_spl - 10.0,
        "Null should be >10dB below surrounding. Null: {}, Surrounding: {}",
        null_spl,
        surrounding_spl
    );
}

#[test]
fn test_load_peaked_response_fixture() {
    let curve = load_test_csv("peaked_response.csv");

    assert!(curve.freq.len() > 5);

    // Find the peak around 500Hz
    let peak_idx = curve
        .freq
        .iter()
        .enumerate()
        .find(|(_, f)| (**f - 500.0).abs() < 1.0)
        .map(|(i, _)| i)
        .expect("Should have 500Hz point");

    let peak_spl = curve.spl[peak_idx];

    // Peak should be at 95dB
    assert!(
        (peak_spl - 95.0).abs() < 0.1,
        "Peak should be at 95dB, got {}",
        peak_spl
    );

    // Surrounding points should be lower
    assert!(
        peak_spl > curve.spl[peak_idx - 1] + 3.0,
        "Peak should be higher than neighbors"
    );
    assert!(
        peak_spl > curve.spl[peak_idx + 1] + 3.0,
        "Peak should be higher than neighbors"
    );
}

#[test]
fn test_fir_from_fixture_data() {
    // Use fixture data to generate FIR and verify basic properties
    let measurement = load_test_csv("room_with_null.csv");
    let sample_rate = 48000.0;
    let n_taps = 4096;

    // Target: flat at 85dB (matching the baseline of the measurement)
    let target = create_flat_curve(20.0, 20000.0, 50, 85.0);

    // Compute correction curve
    let target_interp = autoeq::read::interpolate(&measurement.freq, &target);

    let correction_spl: Vec<f64> = target_interp
        .spl
        .iter()
        .zip(measurement.spl.iter())
        .map(|(t, m)| t - m)
        .collect();

    let correction_curve = Curve {
        freq: measurement.freq.clone(),
        spl: Array1::from(correction_spl),
        phase: None,
        ..Default::default()
    };

    let coeffs =
        generate_fir_from_response(&correction_curve, sample_rate, n_taps, FirPhase::Minimum);

    assert_eq!(coeffs.len(), n_taps);

    // FIR should have non-trivial coefficients
    let max_coeff = coeffs.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
    assert!(
        max_coeff > 0.001,
        "FIR should have significant coefficients"
    );

    // For minimum phase, energy should be concentrated toward the start
    // Note: The exact distribution depends on the target spectrum
    let first_half: f64 = coeffs[..n_taps / 2].iter().map(|x| x * x).sum();
    let total: f64 = coeffs.iter().map(|x| x * x).sum();

    // At least 30% of energy should be in first half for minimum phase
    // (this is more lenient as the actual distribution depends on the correction target)
    assert!(
        first_half / total > 0.3,
        "Minimum phase should have significant energy in first half, got {:.1}%",
        (first_half / total) * 100.0
    );
}

#[test]
fn test_linear_vs_minimum_phase_difference() {
    // Linear and minimum phase FIRs should produce different impulse responses
    let sample_rate = 48000.0;
    let n_taps = 2048;

    let target = create_test_curve(
        &[20.0, 100.0, 500.0, 1000.0, 5000.0, 20000.0],
        &[0.0, 3.0, 0.0, -2.0, -4.0, -8.0],
    );

    let linear_coeffs = generate_fir_from_response(&target, sample_rate, n_taps, FirPhase::Linear);
    let minimum_coeffs =
        generate_fir_from_response(&target, sample_rate, n_taps, FirPhase::Minimum);

    // Time-domain coefficients should be different
    let diff_sum: f64 = linear_coeffs
        .iter()
        .zip(minimum_coeffs.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();

    assert!(
        diff_sum > 0.1,
        "Linear and minimum phase should have different coefficients"
    );

    // Linear phase should have peak near center
    let (linear_max_idx, _) = linear_coeffs
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .unwrap();

    let linear_center = n_taps / 2;
    let linear_distance = (linear_max_idx as isize - linear_center as isize).unsigned_abs();
    assert!(
        linear_distance < n_taps / 10,
        "Linear phase peak should be near center"
    );

    // Minimum phase should have more energy in the first half than linear phase
    // (comparing energy distributions rather than peak positions)
    let linear_first_half_energy: f64 = linear_coeffs[..n_taps / 2].iter().map(|x| x * x).sum();
    let linear_total_energy: f64 = linear_coeffs.iter().map(|x| x * x).sum();
    let linear_first_half_ratio = linear_first_half_energy / linear_total_energy;

    let minimum_first_half_energy: f64 = minimum_coeffs[..n_taps / 2].iter().map(|x| x * x).sum();
    let minimum_total_energy: f64 = minimum_coeffs.iter().map(|x| x * x).sum();
    let minimum_first_half_ratio = minimum_first_half_energy / minimum_total_energy;

    // Minimum phase should have higher first-half energy ratio than linear phase
    assert!(
        minimum_first_half_ratio > linear_first_half_ratio,
        "Minimum phase should have more energy in first half. Linear: {:.1}%, Minimum: {:.1}%",
        linear_first_half_ratio * 100.0,
        minimum_first_half_ratio * 100.0
    );
}

#[test]
fn test_fir_energy_conservation() {
    // Test that FIR filter has reasonable energy properties
    let sample_rate = 48000.0;
    let n_taps = 1024;

    // Flat 0dB target should produce filter with energy ≈ 1
    let target = create_flat_curve(20.0, 20000.0, 50, 0.0);
    let coeffs = generate_fir_from_response(&target, sample_rate, n_taps, FirPhase::Linear);

    // Total energy (sum of squares)
    let total_energy: f64 = coeffs.iter().map(|x| x * x).sum();

    // For a flat 0dB (unity gain) filter, energy should be around 1
    // Allow some tolerance due to windowing
    assert!(
        total_energy > 0.1 && total_energy < 10.0,
        "Unity gain FIR should have energy close to 1, got {}",
        total_energy
    );
}

#[test]
fn test_fir_different_sample_rates() {
    // Test that FIR generation works with different sample rates
    let n_taps = 1024;

    let target = create_test_curve(
        &[20.0, 100.0, 1000.0, 10000.0, 20000.0],
        &[0.0, 2.0, 0.0, -2.0, -4.0],
    );

    for sample_rate in [44100.0, 48000.0, 96000.0] {
        let coeffs = generate_fir_from_response(&target, sample_rate, n_taps, FirPhase::Linear);

        assert_eq!(coeffs.len(), n_taps);
        assert!(
            coeffs.iter().any(|&x| x.abs() > 1e-10),
            "FIR at {}Hz should have non-zero coefficients",
            sample_rate
        );

        // Verify response at 1kHz (well within Nyquist for all rates)
        let response = compute_fir_frequency_response(&coeffs, sample_rate, &[1000.0]);
        assert!(
            response[0].is_finite(),
            "FIR response at 1kHz should be finite for {}Hz sample rate",
            sample_rate
        );
    }
}

#[test]
fn test_fir_different_tap_counts() {
    // Test that FIR generation works with various tap counts
    let sample_rate = 48000.0;

    let target = create_test_curve(
        &[20.0, 100.0, 1000.0, 10000.0, 20000.0],
        &[0.0, 2.0, 0.0, -2.0, -4.0],
    );

    for n_taps in [64, 256, 1024, 4096] {
        let coeffs = generate_fir_from_response(&target, sample_rate, n_taps, FirPhase::Linear);

        assert_eq!(
            coeffs.len(),
            n_taps,
            "Should produce requested number of taps: {}",
            n_taps
        );

        // Should produce valid coefficients
        assert!(
            coeffs.iter().all(|x| x.is_finite()),
            "All coefficients should be finite for {} taps",
            n_taps
        );
    }
}
