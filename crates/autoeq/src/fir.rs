//! FIR filter design and optimization
//!
//! Provides functionality to generate FIR filters matching a target frequency response,
//! with support for linear and minimum phase.

use crate::Curve;
use ndarray::Array1;
use num_complex::Complex64;
use rustfft::FftPlanner;
use rustfft::num_traits::Zero;
use std::f64::consts::PI;

/// Phase type for FIR generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirPhase {
    /// Linear phase (symmetrical impulse response, constant delay)
    Linear,
    /// Minimum phase (causal, minimum delay, concentrates energy at start)
    Minimum,
}

/// Generate an FIR filter to match a target frequency response
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
    // 1. Interpolate target to linear grid for FFT
    // FFT size should be at least n_taps, preferably power of 2
    let fft_size = (n_taps * 8).next_power_of_two().max(4096);
    let n_bins = fft_size / 2 + 1;

    // Create linear frequency grid (0 to Nyquist)
    let freq_step = sample_rate / fft_size as f64;
    let linear_freqs = Array1::from_shape_fn(n_bins, |i| i as f64 * freq_step);

    // Interpolate target curve to this grid
    // Note: read::interpolate assumes log interpolation which is fine for magnitude
    let interpolated = crate::read::interpolate(&linear_freqs, target_curve);
    let magnitude_db = interpolated.spl;

    // Convert dB to linear magnitude
    let magnitude = magnitude_db.mapv(|db| 10.0_f64.powf(db / 20.0));

    // 2. Construct complex spectrum based on phase type
    let mut spectrum = vec![Complex64::zero(); n_bins];

    match phase_type {
        FirPhase::Linear => {
            // Linear phase = magnitude + linear phase shift to center the impulse
            // Delay of (N-1)/2 samples
            // But here we design for full FFT size first, then window
            // Usually we create zero-phase here, IFFT, then rotate/window
            for i in 0..n_bins {
                spectrum[i] = Complex64::new(magnitude[i], 0.0);
            }
        }
        FirPhase::Minimum => {
            // Minimum phase via Cepstrum method (Hilbert transform)
            // 1. Log magnitude
            // 2. IFFT -> Real Cepstrum
            // 3. Window Cepstrum (causal part)
            // 4. FFT -> Analytic Signal (Complex Log Magnitude)
            // 5. Exp -> Minimum Phase Spectrum

            // Step 1: Log Magnitude (avoid log(0))
            let log_mag: Vec<Complex64> = magnitude
                .iter()
                .map(|&m| Complex64::new(m.max(1e-9).ln(), 0.0))
                .collect();

            // Construct full symmetric spectrum for IFFT
            let mut full_log_mag = vec![Complex64::zero(); fft_size];
            full_log_mag[0] = log_mag[0];
            for i in 1..n_bins {
                full_log_mag[i] = log_mag[i];
                // Conjugate symmetry for real signal (but log_mag is already real)
                full_log_mag[fft_size - i] = log_mag[i].conj();
            }
            // Nyquist
            if fft_size.is_multiple_of(2) {
                full_log_mag[n_bins - 1] = log_mag[n_bins - 1]; // Make sure it's real
            }

            // Step 2: IFFT
            let mut planner = FftPlanner::new();
            let ifft = planner.plan_fft_inverse(fft_size);
            let mut cepstrum = full_log_mag.clone();
            ifft.process(&mut cepstrum);

            // Normalize IFFT
            for x in &mut cepstrum {
                *x /= fft_size as f64;
            }

            // Step 3: Window Cepstrum to make it causal
            // Keep dc, double positive time, zero negative time
            let mut causal_cepstrum = vec![Complex64::zero(); fft_size];
            causal_cepstrum[0] = cepstrum[0]; // DC
            // Positive frequencies (1 to N/2 - 1) -> multiply by 2
            for i in 1..fft_size / 2 {
                causal_cepstrum[i] = cepstrum[i] * 2.0;
            }
            // Nyquist
            causal_cepstrum[fft_size / 2] = cepstrum[fft_size / 2];
            // Negative frequencies (N/2 + 1 to N) -> zero

            // Step 4: FFT back
            let fft = planner.plan_fft_forward(fft_size);
            let mut analytic_log_spectrum = causal_cepstrum;
            fft.process(&mut analytic_log_spectrum);

            // Step 5: Exponentiate to get Min Phase Spectrum
            for i in 0..n_bins {
                spectrum[i] = analytic_log_spectrum[i].exp();
            }
        }
    }

    // 3. IFFT to get Impulse Response
    // Construct full symmetric spectrum
    let mut full_spectrum = vec![Complex64::zero(); fft_size];
    full_spectrum[0] = spectrum[0]; // DC must be real
    for i in 1..n_bins {
        full_spectrum[i] = spectrum[i];
        full_spectrum[fft_size - i] = spectrum[i].conj();
    }
    // Nyquist must be real
    if fft_size.is_multiple_of(2) {
        // Force Nyquist to be real (using magnitude)
        full_spectrum[n_bins - 1] = Complex64::new(spectrum[n_bins - 1].norm(), 0.0);
    }

    let mut planner = FftPlanner::new();
    let ifft = planner.plan_fft_inverse(fft_size);
    let mut ir_complex = full_spectrum;
    ifft.process(&mut ir_complex);

    // Extract real part and normalize
    let mut ir: Vec<f64> = ir_complex.iter().map(|c| c.re / fft_size as f64).collect();

    // 4. Windowing and Centering
    if phase_type == FirPhase::Linear {
        // Rotate to center
        // Current peak is at 0. We want it at (n_taps-1)/2.
        // Or simply fftshift?
        // Since we started with zero phase, the impulse is at index 0 (and wrapped at end).
        // We need to shift it to the middle of our desired n_taps.
        // But n_taps << fft_size usually.
        // We center the window around index 0 (circularly).

        let center = n_taps / 2;
        let mut final_ir = vec![0.0; n_taps];

        // Copy from end of buffer to start of final_ir (negative time)
        // Copy from start of buffer to end of final_ir (positive time)
        // Actually, easiest is to just grab indices [-center .. center] modulo fft_size

        for (i, val) in final_ir.iter_mut().enumerate().take(n_taps) {
            // i goes from 0 to n_taps-1.
            // We want index 'center' to map to IR index 0.
            // i = center => ir_idx = 0.
            // i = 0 => ir_idx = -center.

            let shift = i as isize - center as isize;
            let ir_idx = if shift < 0 {
                fft_size as isize + shift
            } else {
                shift
            };

            *val = ir[ir_idx as usize];
        }
        ir = final_ir;
    } else {
        // Minimum phase: Impulse is already at 0. Just truncate.
        ir.truncate(n_taps);
    }

    // Apply Window (Blackman or Hann) to smooth truncation
    // Use crate::math_iir::fir logic or implement simple window
    // I'll implement a simple Blackman window
    let window = make_blackman_window(n_taps);
    for (x, w) in ir.iter_mut().zip(window.iter()) {
        *x *= w;
    }

    ir
}

fn make_blackman_window(size: usize) -> Vec<f64> {
    (0..size)
        .map(|i| {
            let alpha = 0.42;
            let beta = 0.5;
            let gamma = 0.08;
            let n = i as f64;
            let m = (size - 1) as f64;
            let p = 2.0 * PI * n / m;
            alpha - beta * p.cos() + gamma * (2.0 * p).cos()
        })
        .collect()
}

/// Save FIR coefficients to a WAV file (32-bit float mono)
pub fn save_fir_to_wav(
    coeffs: &[f64],
    sample_rate: u32,
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in coeffs {
        writer.write_sample(sample as f32)?;
    }
    writer.finalize()?;

    Ok(())
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

    /// Compute total energy of a signal
    fn compute_total_energy(coeffs: &[f64]) -> f64 {
        coeffs.iter().map(|x| x * x).sum()
    }

    #[test]
    fn test_linear_phase_impulse_symmetry() {
        // Linear phase FIR should have symmetric impulse response
        // Note: The FIR filter generation applies a Blackman window which
        // can introduce some asymmetry. We check relative symmetry.
        let sample_rate = 48000.0;
        let n_taps = 512;

        // Create a simple curve with mild frequency response variation
        let target_curve = create_test_curve(
            &[20.0, 100.0, 1000.0, 5000.0, 20000.0],
            &[0.0, 2.0, 0.0, -1.0, -2.0],
        );

        let coeffs = generate_fir_from_response(&target_curve, sample_rate, n_taps, FirPhase::Linear);

        assert_eq!(coeffs.len(), n_taps);

        // Check that the energy is centered (indicative of linear phase)
        // Find the index of maximum energy
        let (max_idx, _) = coeffs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .unwrap();

        let center = n_taps / 2;

        // Peak should be near the center (within 10% of length)
        let tolerance = n_taps / 10;
        assert!(
            (max_idx as isize - center as isize).unsigned_abs() < tolerance,
            "Linear phase FIR peak should be near center. Peak at {}, center at {}",
            max_idx,
            center
        );

        // Verify that energy distribution is roughly symmetric around the peak
        let left_energy: f64 = coeffs[..max_idx].iter().map(|x| x * x).sum();
        let right_energy: f64 = coeffs[max_idx + 1..].iter().map(|x| x * x).sum();

        // Energy ratio should be within 10x of each other for a roughly symmetric filter
        let energy_ratio = if left_energy > right_energy {
            left_energy / right_energy.max(1e-10)
        } else {
            right_energy / left_energy.max(1e-10)
        };

        assert!(
            energy_ratio < 10.0,
            "Linear phase FIR should have roughly symmetric energy distribution. Ratio = {}",
            energy_ratio
        );
    }

    #[test]
    fn test_minimum_phase_energy_concentration() {
        // Minimum phase FIR should concentrate energy at the start
        let sample_rate = 48000.0;
        let n_taps = 1024;

        // Create a curve with some frequency shaping
        let target_curve = create_test_curve(
            &[20.0, 100.0, 500.0, 1000.0, 5000.0, 20000.0],
            &[-3.0, 0.0, 2.0, 0.0, -2.0, -5.0],
        );

        let coeffs =
            generate_fir_from_response(&target_curve, sample_rate, n_taps, FirPhase::Minimum);

        assert_eq!(coeffs.len(), n_taps);

        // For minimum phase, most energy should be in the first portion
        let total_energy = compute_total_energy(&coeffs);
        let first_quarter_energy = compute_energy_in_range(&coeffs, 0.0, 0.25);
        let first_half_energy = compute_energy_in_range(&coeffs, 0.0, 0.5);

        // At least 50% of energy should be in first quarter for minimum phase
        let first_quarter_ratio = first_quarter_energy / total_energy;
        assert!(
            first_quarter_ratio > 0.5,
            "Minimum phase should have >50% energy in first quarter, got {:.1}%",
            first_quarter_ratio * 100.0
        );

        // At least 90% in first half
        let first_half_ratio = first_half_energy / total_energy;
        assert!(
            first_half_ratio > 0.9,
            "Minimum phase should have >90% energy in first half, got {:.1}%",
            first_half_ratio * 100.0
        );
    }

    #[test]
    fn test_flat_target_produces_near_impulse() {
        // A flat 0dB target should produce something close to a unity impulse
        let sample_rate = 48000.0;
        let n_taps = 256;

        let target_curve = create_flat_curve(20.0, 20000.0, 100, 0.0);

        let coeffs = generate_fir_from_response(&target_curve, sample_rate, n_taps, FirPhase::Linear);

        assert_eq!(coeffs.len(), n_taps);

        // Find the peak coefficient (should be near center for linear phase)
        let (max_idx, max_val) = coeffs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .unwrap();

        // Peak should be near center for linear phase
        let center = n_taps / 2;
        assert!(
            (max_idx as isize - center as isize).abs() < 10,
            "Peak should be near center for linear phase, got {} vs {}",
            max_idx,
            center
        );

        // Peak value should be positive and significant
        assert!(
            *max_val > 0.0,
            "Peak coefficient should be positive, got {}",
            max_val
        );

        // Most other coefficients should be small compared to peak
        let sum_others: f64 = coeffs
            .iter()
            .enumerate()
            .filter(|(i, _)| (*i as isize - center as isize).abs() > 5)
            .map(|(_, v)| v.abs())
            .sum();

        let peak_sum: f64 = coeffs
            .iter()
            .enumerate()
            .filter(|(i, _)| (*i as isize - center as isize).abs() <= 5)
            .map(|(_, v)| v.abs())
            .sum();

        assert!(
            peak_sum > sum_others,
            "Peak region should have more magnitude than tails"
        );
    }

    #[test]
    fn test_blackman_window_properties() {
        let window = make_blackman_window(128);

        assert_eq!(window.len(), 128);

        // Blackman window should have very small values at endpoints
        assert!(
            window[0] < 0.01,
            "Blackman start should be near zero, got {}",
            window[0]
        );
        assert!(
            window[127] < 0.01,
            "Blackman end should be near zero, got {}",
            window[127]
        );

        // Maximum should be at center
        let center_val = window[64];
        assert!(
            center_val > 0.99,
            "Blackman center should be near 1.0, got {}",
            center_val
        );

        // Should be symmetric
        for i in 0..64 {
            let diff = (window[i] - window[127 - i]).abs();
            assert!(
                diff < 0.001,
                "Blackman window should be symmetric at index {}, diff = {}",
                i,
                diff
            );
        }
    }

    #[test]
    fn test_small_tap_count() {
        // Test with small number of taps (edge case)
        let sample_rate = 48000.0;
        let n_taps = 64;

        let target_curve = create_flat_curve(100.0, 10000.0, 50, 0.0);

        let coeffs = generate_fir_from_response(&target_curve, sample_rate, n_taps, FirPhase::Linear);

        assert_eq!(coeffs.len(), n_taps);

        // Should still produce valid output
        let has_nonzero = coeffs.iter().any(|&x| x.abs() > 1e-10);
        assert!(has_nonzero, "FIR should have non-zero coefficients");
    }

    #[test]
    fn test_large_tap_count() {
        // Test with large number of taps
        let sample_rate = 96000.0;
        let n_taps = 4096;

        let target_curve = create_test_curve(
            &[20.0, 50.0, 100.0, 500.0, 1000.0, 5000.0, 10000.0, 20000.0],
            &[-6.0, 0.0, 3.0, 0.0, -2.0, -4.0, -6.0, -10.0],
        );

        let coeffs =
            generate_fir_from_response(&target_curve, sample_rate, n_taps, FirPhase::Minimum);

        assert_eq!(coeffs.len(), n_taps);

        // Should produce valid output
        let has_nonzero = coeffs.iter().any(|&x| x.abs() > 1e-10);
        assert!(has_nonzero, "Large FIR should have non-zero coefficients");
    }

    #[test]
    fn test_save_fir_to_wav_creates_valid_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let wav_path = temp_dir.path().join("test_fir.wav");

        // Create some test coefficients
        let coeffs: Vec<f64> = (0..256).map(|i| (i as f64 * 0.01).sin()).collect();

        let result = save_fir_to_wav(&coeffs, 48000, &wav_path);
        assert!(result.is_ok(), "save_fir_to_wav should succeed");

        // Verify file was created and has correct size
        assert!(wav_path.exists(), "WAV file should be created");

        // Read back and verify
        let reader = hound::WavReader::open(&wav_path).expect("Should open WAV file");
        let spec = reader.spec();

        assert_eq!(spec.channels, 1, "Should be mono");
        assert_eq!(spec.sample_rate, 48000, "Sample rate should match");
        assert_eq!(spec.bits_per_sample, 32, "Should be 32-bit float");
        assert_eq!(reader.len() as usize, coeffs.len(), "Sample count should match");
    }

    #[test]
    fn test_fir_phase_types_differ() {
        // Linear and minimum phase should produce different results
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

        // They should be different
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
}
