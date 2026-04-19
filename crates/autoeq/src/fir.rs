//! FIR filter design and optimization
//!
//! Provides functionality to generate FIR filters matching a target frequency response,
//! with support for linear and minimum phase.
//!
//! This module wraps the core FIR design functions from `math_audio_iir_fir` and adds
//! convenience wrappers that work with the `Curve` type.

use crate::Curve;
use std::path::Path;

// Re-export core types from math-iir-fir
pub use math_audio_iir_fir::{
    FirDesignConfig, FirPhase, WindowType,
    generate_kirkeby_correction as generate_kirkeby_correction_raw, save_fir_to_wav,
};

/// Generate an FIR filter to match a target frequency response
///
/// This helper supports generic target matching for `FirPhase::Linear` and
/// `FirPhase::Minimum`. Use `generate_kirkeby_correction*` for Kirkeby
/// regularized inversion, which requires both a measurement and a target.
///
/// # Arguments
/// * `target_curve` - The target frequency response (magnitude only needed)
/// * `sample_rate` - Sample rate in Hz
/// * `n_taps` - Number of taps (coefficients) for the FIR filter
/// * `phase_type` - Desired phase characteristic
///
/// # Returns
/// * Vector of FIR coefficients
pub fn generate_fir_from_response(
    target_curve: &Curve,
    sample_rate: f64,
    n_taps: usize,
    phase_type: FirPhase,
) -> Vec<f64> {
    let config = FirDesignConfig {
        n_taps,
        sample_rate,
        phase: phase_type,
        ..Default::default()
    };

    // Convert Curve to raw arrays
    let freqs: Vec<f64> = target_curve.freq.to_vec();
    let magnitude_db: Vec<f64> = target_curve.spl.to_vec();

    math_audio_iir_fir::generate_fir_from_response(&freqs, &magnitude_db, &config)
}

/// Generate Kirkeby regularized FIR correction filter from Curve
///
/// # Arguments
/// * `measurement` - Measurement curve (SPL and optionally phase)
/// * `target` - Target curve (SPL)
/// * `sample_rate` - Sample rate in Hz
/// * `n_taps` - Number of taps
/// * `min_freq` - Minimum frequency for in-band regularization
/// * `max_freq` - Maximum frequency for in-band regularization
///
/// # Returns
/// * Vector of FIR coefficients
pub fn generate_kirkeby_correction(
    measurement: &Curve,
    target: &Curve,
    sample_rate: f64,
    n_taps: usize,
    min_freq: f64,
    max_freq: f64,
) -> Vec<f64> {
    generate_kirkeby_correction_with_phase(
        measurement,
        target,
        sample_rate,
        n_taps,
        min_freq,
        max_freq,
        false, // Default: magnitude-only correction
    )
}

/// Generate Kirkeby regularized FIR correction filter with optional excess phase correction
///
/// # Arguments
/// * `measurement` - Measurement curve (SPL and optionally phase)
/// * `target` - Target curve (SPL)
/// * `sample_rate` - Sample rate in Hz
/// * `n_taps` - Number of taps
/// * `min_freq` - Minimum frequency for in-band regularization
/// * `max_freq` - Maximum frequency for in-band regularization
/// * `correct_excess_phase` - Whether to correct excess phase (requires phase data in measurement)
///
/// # Returns
/// * Vector of FIR coefficients
pub fn generate_kirkeby_correction_with_phase(
    measurement: &Curve,
    target: &Curve,
    sample_rate: f64,
    n_taps: usize,
    min_freq: f64,
    max_freq: f64,
    correct_excess_phase: bool,
) -> Vec<f64> {
    generate_kirkeby_correction_with_smoothing(
        measurement,
        target,
        sample_rate,
        n_taps,
        min_freq,
        max_freq,
        correct_excess_phase,
        0.167, // Default 1/6 octave smoothing
    )
}

/// Generate Kirkeby regularized FIR correction filter with optional excess phase correction and smoothing
///
/// # Arguments
/// * `measurement` - Measurement curve (SPL and optionally phase)
/// * `target` - Target curve (SPL)
/// * `sample_rate` - Sample rate in Hz
/// * `n_taps` - Number of taps
/// * `min_freq` - Minimum frequency for in-band regularization
/// * `max_freq` - Maximum frequency for in-band regularization
/// * `correct_excess_phase` - Whether to correct excess phase (requires phase data in measurement)
/// * `phase_smoothing_octaves` - Phase smoothing width in octaves (0.0 to disable)
///
/// # Returns
/// * Vector of FIR coefficients
pub fn generate_kirkeby_correction_with_smoothing(
    measurement: &Curve,
    target: &Curve,
    sample_rate: f64,
    n_taps: usize,
    min_freq: f64,
    max_freq: f64,
    correct_excess_phase: bool,
    phase_smoothing_octaves: f64,
) -> Vec<f64> {
    let config = FirDesignConfig {
        n_taps,
        sample_rate,
        phase: FirPhase::Kirkeby,
        min_freq,
        max_freq,
        correct_excess_phase,
        phase_smoothing_octaves,
        ..Default::default()
    };

    let meas_freqs: Vec<f64> = measurement.freq.to_vec();
    let meas_db: Vec<f64> = measurement.spl.to_vec();
    let meas_phase: Option<Vec<f64>> = measurement.phase.as_ref().map(|p| p.to_vec());

    // Interpolate target to measurement frequencies if needed
    let target_db: Vec<f64> = if target.freq.len() == measurement.freq.len() {
        target.spl.to_vec()
    } else {
        // Interpolate target to measurement frequency grid
        let interpolated = crate::read::interpolate(&measurement.freq, target);
        interpolated.spl.to_vec()
    };

    generate_kirkeby_correction_raw(
        &meas_freqs,
        &meas_db,
        meas_phase.as_deref(),
        &target_db,
        &config,
    )
}

/// Save FIR coefficients to a WAV file (32-bit float mono)
///
/// Convenience wrapper that takes a Path reference.
pub fn save_fir_wav(
    coeffs: &[f64],
    sample_rate: u32,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    save_fir_to_wav(coeffs, sample_rate, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;
    use tempfile::TempDir;

    /// Helper to create a test curve with given frequencies and SPL values
    fn create_test_curve(freqs: &[f64], spl_values: &[f64]) -> Curve {
        Curve {
            freq: Array1::from(freqs.to_vec()),
            spl: Array1::from(spl_values.to_vec()),
            phase: None,
            ..Default::default()
        }
    }

    /// Create a flat response curve at given SPL level
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

    /// Compute energy in a specific portion of the signal
    fn compute_energy_in_range(coeffs: &[f64], start_fraction: f64, end_fraction: f64) -> f64 {
        let n = coeffs.len();
        let start = (n as f64 * start_fraction) as usize;
        let end = (n as f64 * end_fraction) as usize;
        coeffs[start..end].iter().map(|x| x * x).sum()
    }

    #[test]
    fn test_linear_phase_impulse_symmetry() {
        let sample_rate = 48000.0;
        let n_taps = 512;

        let target_curve = create_test_curve(
            &[20.0, 100.0, 1000.0, 5000.0, 20000.0],
            &[0.0, 2.0, 0.0, -1.0, -2.0],
        );

        let coeffs =
            generate_fir_from_response(&target_curve, sample_rate, n_taps, FirPhase::Linear);

        assert_eq!(coeffs.len(), n_taps);

        // Check that the energy is centered
        let (max_idx, _) = coeffs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .unwrap();

        let center = n_taps / 2;
        let tolerance = n_taps / 10;
        assert!(
            (max_idx as isize - center as isize).unsigned_abs() < tolerance,
            "Linear phase FIR peak should be near center. Peak at {}, center at {}",
            max_idx,
            center
        );
    }

    #[test]
    fn test_minimum_phase_energy_concentration() {
        let sample_rate = 48000.0;
        let n_taps = 1024;

        let target_curve = create_test_curve(
            &[20.0, 100.0, 500.0, 1000.0, 5000.0, 20000.0],
            &[-3.0, 0.0, 2.0, 0.0, -2.0, -5.0],
        );

        let coeffs =
            generate_fir_from_response(&target_curve, sample_rate, n_taps, FirPhase::Minimum);

        assert_eq!(coeffs.len(), n_taps);

        // For minimum phase, first half should have more energy than second half
        // (windowing affects the exact distribution)
        let first_half_energy = compute_energy_in_range(&coeffs, 0.0, 0.5);
        let second_half_energy = compute_energy_in_range(&coeffs, 0.5, 1.0);

        assert!(
            first_half_energy > second_half_energy,
            "Minimum phase should have more energy in first half: first={:.4}, second={:.4}",
            first_half_energy,
            second_half_energy
        );
    }

    #[test]
    fn test_flat_target_produces_near_impulse() {
        let sample_rate = 48000.0;
        let n_taps = 256;

        let target_curve = create_flat_curve(20.0, 20000.0, 100, 0.0);

        let coeffs =
            generate_fir_from_response(&target_curve, sample_rate, n_taps, FirPhase::Linear);

        assert_eq!(coeffs.len(), n_taps);

        let (max_idx, max_val) = coeffs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .unwrap();

        let center = n_taps / 2;
        assert!(
            (max_idx as isize - center as isize).abs() < 10,
            "Peak should be near center for linear phase"
        );

        assert!(*max_val > 0.0, "Peak coefficient should be positive");
    }

    #[test]
    fn test_save_fir_to_wav_creates_valid_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let wav_path = temp_dir.path().join("test_fir.wav");

        let coeffs: Vec<f64> = (0..256).map(|i| (i as f64 * 0.01).sin()).collect();

        let result = save_fir_wav(&coeffs, 48000, &wav_path);
        assert!(result.is_ok(), "save_fir_wav should succeed");
        assert!(wav_path.exists(), "WAV file should be created");

        let reader = hound::WavReader::open(&wav_path).expect("Should open WAV file");
        let spec = reader.spec();

        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 48000);
        assert_eq!(spec.bits_per_sample, 32);
        assert_eq!(reader.len() as usize, coeffs.len());
    }

    #[test]
    fn test_fir_phase_types_differ() {
        let sample_rate = 48000.0;
        let n_taps = 512;

        let target_curve = create_test_curve(
            &[20.0, 100.0, 1000.0, 10000.0, 20000.0],
            &[0.0, 3.0, 0.0, -3.0, -6.0],
        );

        let linear_coeffs =
            generate_fir_from_response(&target_curve, sample_rate, n_taps, FirPhase::Linear);
        let minimum_coeffs =
            generate_fir_from_response(&target_curve, sample_rate, n_taps, FirPhase::Minimum);

        assert_eq!(linear_coeffs.len(), minimum_coeffs.len());

        let sum_diff: f64 = linear_coeffs
            .iter()
            .zip(minimum_coeffs.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();

        assert!(
            sum_diff > 0.1,
            "Linear and minimum phase should produce different coefficients"
        );
    }

    #[test]
    fn test_kirkeby_correction() {
        let measurement = create_test_curve(
            &[20.0, 100.0, 500.0, 1000.0, 5000.0, 20000.0],
            &[75.0, 82.0, 80.0, 78.0, 72.0, 65.0],
        );
        let target = create_test_curve(
            &[20.0, 100.0, 500.0, 1000.0, 5000.0, 20000.0],
            &[80.0, 80.0, 80.0, 80.0, 80.0, 80.0],
        );

        let coeffs =
            generate_kirkeby_correction(&measurement, &target, 48000.0, 4096, 20.0, 1000.0);

        assert_eq!(coeffs.len(), 4096);
        assert!(coeffs.iter().any(|&x| x.abs() > 1e-10));
    }
}
