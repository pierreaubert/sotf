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
